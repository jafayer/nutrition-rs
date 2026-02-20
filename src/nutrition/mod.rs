//! Compute nutritional information for ingredients, recipes, and days.
//!
//! This module provides the [`NutritionReport`] type and functions for
//! scaling and summing nutritional properties using the `nutrition-units`
//! unit registry.

use crate::ast::ast::{Day, DayItem, Document, Exercise, Ingredient, Item, Property, Quantity, Recipe};
use nutrition_units::{default_unit_for_property, NutritionQuantity, UnitRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// NutritionReport
// ---------------------------------------------------------------------------

/// The computed nutritional breakdown for a single item at a specific quantity.
#[derive(Debug, Serialize, Deserialize)]
pub struct NutritionReport {
    /// Primary name / alias of the ingredient or recipe.
    pub name: String,
    /// The quantity this report was computed for.
    pub quantity: Quantity,
    /// Aggregated nutritional properties (scaled and summed).
    pub properties: Vec<Property>,
}

impl NutritionReport {
    /// Serialize this report to a pretty-printed JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ---------------------------------------------------------------------------
// DailyNutritionReport
// ---------------------------------------------------------------------------

/// The computed nutritional breakdown for a single day.
#[derive(Debug, Serialize, Deserialize)]
pub struct DailyNutritionReport {
    /// The date this report covers (as stored in the `@day` block).
    pub date: String,
    /// Summed nutritional properties from all `@ate` entries.
    pub intake: Vec<Property>,
    /// Summed nutritional properties burned via all `@exercised` entries.
    pub exercise: Vec<Property>,
    /// Net properties: intake minus exercise (per matching property).
    pub net: Vec<Property>,
}

impl DailyNutritionReport {
    /// Serialize this report to a pretty-printed JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Return the effective unit string for a property value.
///
/// If the declared unit is non-empty it is returned as-is.  Otherwise the
/// canonical default for the property name is used (e.g. `"calories"` →
/// `"kcal"`, `"protein"` → `"g"`).  This normalises unitless declarations
/// like `calories: 269` so they are compatible with explicit-unit declarations
/// like `calories: 300kcal` in arithmetic operations.
fn resolve_unit(prop_name: &str, declared_unit: Option<&str>) -> String {
    match declared_unit {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => default_unit_for_property(prop_name)
            .unwrap_or("")
            .to_string(),
    }
}

/// Scale every property in `properties` by `scale`.
fn scale_properties(properties: &[Property], scale: f64) -> Vec<Property> {
    properties
        .iter()
        .map(|prop| Property {
            name: prop.name.clone(),
            value: Quantity {
                amount: prop.value.amount * scale,
                unit: {
                    let u = resolve_unit(&prop.name, prop.value.unit.as_deref());
                    if u.is_empty() { None } else { Some(u) }
                },
            },
        })
        .collect()
}

/// Sum multiple property lists using unit-aware addition (SI defaults).
///
/// Properties with the same name but incompatible units are kept separate with
/// their unit appended to the name (e.g. `"calories_kcal"` vs `"calories_cal"`).
fn sum_properties(all_properties: Vec<Vec<Property>>) -> Vec<Property> {
    let reg = UnitRegistry::with_si_defaults();
    // Accumulate totals as NutritionQuantity keyed by property name.
    let mut totals: HashMap<String, NutritionQuantity> = HashMap::new();

    for properties in all_properties {
        for prop in properties {
            let unit = resolve_unit(&prop.name, prop.value.unit.as_deref());
            let nq = NutritionQuantity::new(prop.value.amount, unit);

            match totals.get(&prop.name) {
                Some(existing) => match reg.add(existing, &nq) {
                    Ok(sum) => {
                        totals.insert(prop.name.clone(), sum);
                    }
                    Err(_) => {
                        // Incompatible units – store under a disambiguated key so that
                        // e.g. "calories_kcal" and "calories_cal" are tracked separately.
                        // `or_insert_with` initialises a zero accumulator on the first
                        // encounter; subsequent encounters add to the same accumulator.
                        let key = if nq.unit.is_empty() {
                            prop.name.clone()
                        } else {
                            format!("{}_{}", prop.name, nq.unit)
                        };
                        let entry = totals.entry(key).or_insert_with(|| NutritionQuantity::new(0.0, &nq.unit));
                        entry.amount += nq.amount;
                    }
                },
                None => {
                    totals.insert(prop.name.clone(), nq);
                }
            }
        }
    }

    totals
        .into_iter()
        .map(|(name, nq)| Property {
            name,
            value: Quantity {
                amount: nq.amount,
                unit: if nq.unit.is_empty() { None } else { Some(nq.unit) },
            },
        })
        .collect()
}

/// Compute the scale factor needed to go from `ingredient_quantities` (the
/// base declaration) to `requested` (the desired quantity).
///
/// Uses the ingredient's own unit registry (built from all declared quantity
/// equivalencies) for cross-unit conversions.
fn compute_scale(ingredient_quantities: &[Quantity], requested: &Quantity) -> f64 {
    if ingredient_quantities.is_empty() {
        // No declared base quantity – treat the base as 1 (unitless), so the
        // scale equals the requested amount directly.
        return requested.amount;
    }

    let base = &ingredient_quantities[0];
    let base_unit = base.unit.as_deref().unwrap_or("");
    let req_unit = requested.unit.as_deref().unwrap_or("");

    if base_unit == req_unit {
        // Same unit – straightforward ratio.
        return requested.amount / base.amount;
    }

    // Build a registry from the ingredient's own declared quantities so that
    // ingredient-scoped equivalencies (e.g. 100g = 1 cup for chickpeas) are
    // available.
    let pairs: Vec<(f64, String)> = ingredient_quantities
        .iter()
        .map(|q| (q.amount, q.unit.clone().unwrap_or_default()))
        .collect();
    let reg = UnitRegistry::from_ingredient_quantities(&pairs);
    let req_nq = NutritionQuantity::new(requested.amount, req_unit);

    if let Some(converted) = reg.convert(&req_nq, base_unit) {
        converted.amount / base.amount
    } else {
        // Fallback: treat as a plain numeric ratio.
        requested.amount / base.amount
    }
}

/// Subtract `exercise` properties from `intake` properties using unit-aware
/// arithmetic.  Properties that appear only in intake are kept as-is;
/// properties that appear only in exercise are carried over with a negated
/// amount.
fn subtract_properties(intake: &[Property], exercise: &[Property]) -> Vec<Property> {
    let reg = UnitRegistry::with_si_defaults();
    let mut result: HashMap<String, NutritionQuantity> = HashMap::new();

    // Seed with intake values.
    for prop in intake {
        let unit = resolve_unit(&prop.name, prop.value.unit.as_deref());
        let nq = NutritionQuantity::new(prop.value.amount, unit);
        result.insert(prop.name.clone(), nq);
    }

    // Subtract exercise values.
    for prop in exercise {
        let unit = resolve_unit(&prop.name, prop.value.unit.as_deref());
        let nq = NutritionQuantity::new(prop.value.amount, &unit);
        match result.get(&prop.name) {
            Some(existing) => {
                // Try unit-aware subtraction: existing - exercise.
                let neg = NutritionQuantity::new(-nq.amount, &nq.unit);
                match reg.add(existing, &neg) {
                    Ok(diff) => { result.insert(prop.name.clone(), diff); }
                    Err(_) => {
                        // Incompatible units – subtract numerically.
                        let mut entry = existing.clone();
                        entry.amount -= nq.amount;
                        result.insert(prop.name.clone(), entry);
                    }
                }
            }
            None => {
                // Exercise-only property – net is negative.
                result.insert(prop.name.clone(), NutritionQuantity::new(-nq.amount, &nq.unit));
            }
        }
    }

    result
        .into_iter()
        .map(|(name, nq)| Property {
            name,
            value: Quantity {
                amount: nq.amount,
                unit: if nq.unit.is_empty() { None } else { Some(nq.unit) },
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compute the nutritional information for a single `ingredient`, optionally
/// scaled to `requested_quantity`.
///
/// If `requested_quantity` is `None` the ingredient's base quantity (first
/// declared quantity) is used – i.e. no scaling is applied.
pub fn compute_ingredient_nutrition(
    ingredient: &Ingredient,
    requested_quantity: Option<&Quantity>,
) -> NutritionReport {
    let scale = match requested_quantity {
        None => 1.0,
        Some(req) => compute_scale(&ingredient.quantities, req),
    };

    let name = ingredient.aliases.first().cloned().unwrap_or_default();
    let quantity = requested_quantity
        .cloned()
        .or_else(|| ingredient.quantities.first().cloned())
        .unwrap_or(Quantity {
            amount: 1.0,
            unit: None,
        });

    let properties = scale_properties(&ingredient.properties, scale);

    NutritionReport {
        name,
        quantity,
        properties,
    }
}

/// Compute the nutritional information for a `recipe` by resolving its
/// ingredient references from `document`, optionally scaled to
/// `requested_quantity`.
///
/// Returns an error if any ingredient referenced by the recipe cannot be
/// found in `document`.
pub fn compute_recipe_nutrition(
    document: &Document,
    recipe: &Recipe,
    requested_quantity: Option<&Quantity>,
) -> Result<NutritionReport, String> {
    // Build an ingredient lookup from the document.
    let mut ingredients: HashMap<String, &Ingredient> = HashMap::new();
    for item in &document.items {
        if let Item::Ingredient(ing) = item {
            for alias in &ing.aliases {
                ingredients.insert(alias.clone(), ing);
            }
        }
    }

    // Determine how much to scale the whole recipe.
    let recipe_scale = match requested_quantity {
        None => 1.0,
        Some(req) => compute_scale(&recipe.quantities, req),
    };

    // Compute scaled properties for each ingredient in the recipe.
    let mut all_properties: Vec<Vec<Property>> = Vec::new();

    for ing_label in &recipe.ingredients {
        let ingredient = ingredients
            .get(&ing_label.alias)
            .ok_or_else(|| format!("Unknown ingredient: '{}'", ing_label.alias))?;

        // Scale factor = (label quantity / ingredient base quantity) * recipe scale
        let ing_scale = if ingredient.quantities.is_empty() {
            ing_label.quantity.amount * recipe_scale
        } else {
            compute_scale(&ingredient.quantities, &ing_label.quantity) * recipe_scale
        };

        let scaled = scale_properties(&ingredient.properties, ing_scale);
        all_properties.push(scaled);
    }

    let properties = sum_properties(all_properties);

    let name = recipe.aliases.first().cloned().unwrap_or_default();
    let quantity = requested_quantity
        .cloned()
        .or_else(|| recipe.quantities.first().cloned())
        .unwrap_or(Quantity {
            amount: 1.0,
            unit: None,
        });

    Ok(NutritionReport {
        name,
        quantity,
        properties,
    })
}

/// Look up an ingredient or recipe by `alias` in `document` and compute its
/// nutrition, optionally scaled to `requested_quantity`.
///
/// Ingredients are checked before recipes.  Returns an error if the alias is
/// not found.
pub fn query_nutrition(
    document: &Document,
    alias: &str,
    requested_quantity: Option<&Quantity>,
) -> Result<NutritionReport, String> {
    // Try ingredients first.
    for item in &document.items {
        if let Item::Ingredient(ing) = item {
            if ing.aliases.iter().any(|a| a == alias) {
                return Ok(compute_ingredient_nutrition(ing, requested_quantity));
            }
        }
    }

    // Then try recipes.
    for item in &document.items {
        if let Item::Recipe(recipe) = item {
            if recipe.aliases.iter().any(|a| a == alias) {
                return compute_recipe_nutrition(document, recipe, requested_quantity);
            }
        }
    }

    Err(format!("No ingredient or recipe named '{alias}' found in document."))
}

/// Compute the nutritional properties burned by a single exercise at the given
/// quantity.
fn compute_exercise_nutrition(
    exercise: &Exercise,
    requested_quantity: Option<&Quantity>,
) -> Vec<Property> {
    let scale = match requested_quantity {
        None => 1.0,
        Some(req) => compute_scale(&exercise.quantities, req),
    };
    scale_properties(&exercise.properties, scale)
}

/// Compute the daily nutrition report for `day` by resolving all `@ate` and
/// `@exercised` entries against the definitions in `document`.
///
/// Unrecognised food/exercise aliases are silently skipped so that partial
/// data still produces a useful report.
pub fn compute_daily_report(document: &Document, day: &Day) -> DailyNutritionReport {
    // Build lookup tables for ingredients, recipes, and exercises.
    let mut ingredients: HashMap<String, &Ingredient> = HashMap::new();
    let mut recipes: HashMap<String, &Recipe> = HashMap::new();
    let mut exercises: HashMap<String, &Exercise> = HashMap::new();

    for item in &document.items {
        match item {
            Item::Ingredient(ing) => {
                for alias in &ing.aliases {
                    ingredients.insert(alias.clone(), ing);
                }
            }
            Item::Recipe(rec) => {
                for alias in &rec.aliases {
                    recipes.insert(alias.clone(), rec);
                }
            }
            Item::Exercise(ex) => {
                for alias in &ex.aliases {
                    exercises.insert(alias.clone(), ex);
                }
            }
            _ => {}
        }
    }

    // Accumulate intake and exercise properties separately.
    let mut intake_all: Vec<Vec<Property>> = Vec::new();
    let mut exercise_all: Vec<Vec<Property>> = Vec::new();

    for day_item in &day.items {
        match day_item {
            DayItem::Ate(ate) => {
                if let Some(ing) = ingredients.get(&ate.food_alias) {
                    let report = compute_ingredient_nutrition(ing, Some(&ate.quantity));
                    intake_all.push(report.properties);
                } else if let Some(rec) = recipes.get(&ate.food_alias) {
                    if let Ok(report) = compute_recipe_nutrition(document, rec, Some(&ate.quantity)) {
                        intake_all.push(report.properties);
                    }
                }
                // Unrecognised alias: skip gracefully.
            }
            DayItem::Exercised(exercised) => {
                if let Some(ex) = exercises.get(&exercised.exercise_alias) {
                    let props = compute_exercise_nutrition(ex, Some(&exercised.quantity));
                    exercise_all.push(props);
                }
                // Unrecognised exercise alias: skip gracefully.
            }
        }
    }

    let intake = sum_properties(intake_all);
    let exercise = sum_properties(exercise_all);
    let net = subtract_properties(&intake, &exercise);

    DailyNutritionReport {
        date: day.date.clone(),
        intake,
        exercise,
        net,
    }
}

/// Compute daily nutrition reports for every `@day` block whose date falls
/// within `[start, end]` (inclusive, lexicographic comparison on ISO-8601
/// date strings).  Pass `None` for either bound to leave that side open.
pub fn compute_report(
    document: &Document,
    start: Option<&str>,
    end: Option<&str>,
) -> Vec<DailyNutritionReport> {
    document
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Day(day) = item {
                let in_range = start.map_or(true, |s| day.date.as_str() >= s)
                    && end.map_or(true, |e| day.date.as_str() <= e);
                if in_range {
                    Some(compute_daily_report(document, day))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// AggregatedReport
// ---------------------------------------------------------------------------

/// An aggregated (averaged) nutrition report across multiple days.
#[derive(Debug, Serialize, Deserialize)]
pub struct AggregatedReport {
    /// Start of the date range (inclusive).
    pub start: String,
    /// End of the date range (inclusive).
    pub end: String,
    /// Number of days that had `@day` entries in this range.
    pub days: usize,
    /// Per-day average of intake properties.
    pub intake: Vec<Property>,
    /// Per-day average of exercise properties.
    pub exercise: Vec<Property>,
    /// Per-day average of net properties (intake − exercise).
    pub net: Vec<Property>,
}

impl AggregatedReport {
    /// Serialize this report to a pretty-printed JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Average a list of property lists across `n` days.
///
/// Each property value is summed then divided by `n`, rounded to the nearest
/// integer for "calorie-class" unitless properties and to one decimal place
/// for everything else.  The rounding is purely cosmetic – the stored `f64`
/// carries the full precision.
fn average_properties(all: Vec<Vec<Property>>, n: usize) -> Vec<Property> {
    if n == 0 {
        return Vec::new();
    }
    let summed = sum_properties(all);
    let divisor = n as f64;
    summed
        .into_iter()
        .map(|mut prop| {
            prop.value.amount /= divisor;
            prop
        })
        .collect()
}

/// Aggregate a slice of [`DailyNutritionReport`]s into a single
/// [`AggregatedReport`] by averaging each property across all days.
///
/// `start` and `end` are the human-readable date range strings shown in the
/// display (e.g. `"2026-01-01"` / `"2026-01-31"`).
pub fn aggregate_reports(
    reports: &[DailyNutritionReport],
    start: &str,
    end: &str,
) -> AggregatedReport {
    let n = reports.len();

    let intake_all: Vec<Vec<Property>> = reports.iter().map(|r| r.intake.clone()).collect();
    let exercise_all: Vec<Vec<Property>> = reports.iter().map(|r| r.exercise.clone()).collect();
    let net_all: Vec<Vec<Property>> = reports.iter().map(|r| r.net.clone()).collect();

    AggregatedReport {
        start: start.to_string(),
        end: end.to_string(),
        days: n,
        intake: average_properties(intake_all, n),
        exercise: average_properties(exercise_all, n),
        net: average_properties(net_all, n),
    }
}
