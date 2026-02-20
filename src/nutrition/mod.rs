//! Compute nutritional information for ingredients, recipes, and days.
//!
//! This module provides the [`NutritionReport`] type and functions for
//! scaling and summing nutritional properties using the `nutrition-units`
//! unit registry.

use crate::ast::ast::{Document, Ingredient, Item, Property, Quantity, Recipe};
use nutrition_units::{NutritionQuantity, UnitRegistry};
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
// Internal helpers
// ---------------------------------------------------------------------------

/// Scale every property in `properties` by `scale`.
fn scale_properties(properties: &[Property], scale: f64) -> Vec<Property> {
    properties
        .iter()
        .map(|prop| Property {
            name: prop.name.clone(),
            value: Quantity {
                amount: prop.value.amount * scale,
                unit: prop.value.unit.clone(),
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
            let unit = prop.value.unit.clone().unwrap_or_default();
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
