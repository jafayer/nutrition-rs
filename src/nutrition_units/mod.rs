//! `nutrition-units` provides a runtime unit system for nutritional quantities.
//!
//! # Overview
//!
//! - [`NutritionQuantity`]: An amount paired with a unit string.
//! - [`UnitRegistry`]: Holds unit conversion factors (both built-in SI conversions
//!   and ingredient-scoped custom equivalencies).
//! - [`default_unit_for_property`]: Returns the canonical default unit for common
//!   nutritional property names (e.g. `"calories"` → `"kcal"`).
//! - [`ConversionError`]: Returned when two quantities cannot be reconciled.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Default units for well-known nutritional properties
// ---------------------------------------------------------------------------

/// Return the default unit string for a well-known nutritional property name.
///
/// ```
/// use nutrition_rs::nutrition_units::default_unit_for_property;
/// assert_eq!(default_unit_for_property("calories"), Some("kcal"));
/// assert_eq!(default_unit_for_property("protein"),  Some("g"));
/// assert_eq!(default_unit_for_property("unknown"),  None);
/// ```
pub fn default_unit_for_property(property: &str) -> Option<&'static str> {
    match property.to_lowercase().as_str() {
        "calories" | "energy" => Some("kcal"),
        "protein" | "fat" | "carbohydrates" | "carbs" | "fiber" | "sugar" | "sodium" => Some("g"),
        "cholesterol" => Some("mg"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// NutritionQuantity
// ---------------------------------------------------------------------------

/// A numeric amount paired with a unit string.
///
/// ```
/// use nutrition_rs::nutrition_units::NutritionQuantity;
/// let q = NutritionQuantity::new(100.0, "g");
/// assert_eq!(q.to_string(), "100g");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct NutritionQuantity {
    pub amount: f64,
    pub unit: String,
}

impl NutritionQuantity {
    /// Create a new quantity.
    pub fn new(amount: f64, unit: impl Into<String>) -> Self {
        NutritionQuantity {
            amount,
            unit: unit.into(),
        }
    }
}

impl std::fmt::Display for NutritionQuantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.amount, self.unit)
    }
}

// ---------------------------------------------------------------------------
// ConversionError
// ---------------------------------------------------------------------------

/// Error returned when a unit conversion is impossible given the current registry.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionError {
    pub from: String,
    pub to: String,
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot convert '{}' to '{}'", self.from, self.to)
    }
}

impl std::error::Error for ConversionError {}

// ---------------------------------------------------------------------------
// UnitRegistry
// ---------------------------------------------------------------------------

/// A registry of unit conversion factors.
///
/// The registry stores directed edges `(from_unit, to_unit) → factor` where
/// `1 from_unit = factor * to_unit`.  Reverse edges are stored automatically
/// so every `add_conversion` call creates a bidirectional relationship.
///
/// Built-in SI conversions (mass, volume, energy) are included by default via
/// [`UnitRegistry::with_si_defaults`].  Ingredient-scoped custom equivalencies
/// (e.g. `100 g = 1 cup` for chickpeas) can be added with
/// [`UnitRegistry::add_conversion`] or built directly from the ingredient's
/// declared quantities with [`UnitRegistry::from_ingredient_quantities`].
///
/// # Examples
///
/// ```
/// use nutrition_rs::nutrition_units::{NutritionQuantity, UnitRegistry};
///
/// let reg = UnitRegistry::with_si_defaults();
/// let kg = NutritionQuantity::new(1.0, "kg");
/// let grams = reg.convert(&kg, "g").unwrap();
/// assert_eq!(grams.amount, 1000.0);
/// ```
#[derive(Debug, Clone)]
pub struct UnitRegistry {
    /// `(from, to)` → factor such that `1 from = factor * to`
    conversions: HashMap<(String, String), f64>,
}

impl Default for UnitRegistry {
    fn default() -> Self {
        Self::with_si_defaults()
    }
}

impl UnitRegistry {
    /// Create an empty registry with no conversions.
    pub fn empty() -> Self {
        UnitRegistry {
            conversions: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with common SI-adjacent conversions for
    /// nutritional use (mass, volume, energy).
    pub fn with_si_defaults() -> Self {
        let mut reg = UnitRegistry::empty();

        // --- Mass (base: g) ---
        reg.add_conversion("kg", "g", 1_000.0);
        reg.add_conversion("mg", "g", 0.001);
        reg.add_conversion("lb", "g", 453.592);
        reg.add_conversion("oz", "g", 28.3495);

        // --- Volume (base: mL) ---
        reg.add_conversion("L", "mL", 1_000.0);
        reg.add_conversion("l", "mL", 1_000.0);
        reg.add_conversion("cup", "mL", 236.588);
        reg.add_conversion("cups", "mL", 236.588);
        reg.add_conversion("fl_oz", "mL", 29.5735);
        reg.add_conversion("floz", "mL", 29.5735);
        reg.add_conversion("tbsp", "mL", 14.7868);
        reg.add_conversion("tsp", "mL", 4.92892);

        // --- Energy (base: kcal) ---
        reg.add_conversion("cal", "kcal", 0.001);
        reg.add_conversion("kJ", "kcal", 0.239006);

        reg
    }

    /// Register a conversion: `1 from_unit = factor * to_unit`.
    ///
    /// The inverse (`1 to_unit = (1/factor) * from_unit`) is stored
    /// automatically.
    pub fn add_conversion(&mut self, from: &str, to: &str, factor: f64) {
        self.conversions
            .insert((from.to_string(), to.to_string()), factor);
        if factor != 0.0 {
            self.conversions
                .insert((to.to_string(), from.to_string()), 1.0 / factor);
        }
    }

    /// Build a registry from the list of quantities declared for a single
    /// ingredient (i.e. the parenthetical groups in `@ingredient(100g)(1 cup)`).
    ///
    /// Each quantity after the first is treated as equivalent to the first,
    /// establishing scoped conversions for that ingredient only.
    ///
    /// ```
    /// use nutrition_rs::nutrition_units::{NutritionQuantity, UnitRegistry};
    ///
    /// // @ingredient(100g)(1 cup) "chickpeas"
    /// let reg = UnitRegistry::from_ingredient_quantities(&[
    ///     (100.0, "g".to_string()),
    ///     (1.0,   "cup".to_string()),
    /// ]);
    ///
    /// let one_cup = NutritionQuantity::new(1.0, "cup");
    /// let grams = reg.convert(&one_cup, "g").unwrap();
    /// assert!((grams.amount - 100.0).abs() < 1e-9);
    /// ```
    pub fn from_ingredient_quantities(quantities: &[(f64, String)]) -> Self {
        let mut reg = UnitRegistry::with_si_defaults();
        if quantities.len() < 2 {
            return reg;
        }
        let (base_amount, base_unit) = &quantities[0];
        for (other_amount, other_unit) in quantities.iter().skip(1) {
            if *base_amount != 0.0 && *other_amount != 0.0 {
                reg.add_conversion(base_unit, other_unit, other_amount / base_amount);
            }
        }
        reg
    }

    /// Try to convert `qty` to `target_unit` using a breadth-first search
    /// through all known conversion edges (handles multi-hop paths).
    ///
    /// Returns `None` if no conversion path exists.
    pub fn convert(&self, qty: &NutritionQuantity, target_unit: &str) -> Option<NutritionQuantity> {
        if qty.unit == target_unit {
            return Some(qty.clone());
        }

        let mut visited: HashMap<String, f64> = HashMap::new();
        visited.insert(qty.unit.clone(), qty.amount);
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((qty.amount, qty.unit.clone()));

        while let Some((amount, unit)) = queue.pop_front() {
            for ((from, to), factor) in &self.conversions {
                if from == &unit && !visited.contains_key(to.as_str()) {
                    let new_amount = amount * factor;
                    if to == target_unit {
                        return Some(NutritionQuantity::new(new_amount, target_unit));
                    }
                    visited.insert(to.clone(), new_amount);
                    queue.push_back((new_amount, to.clone()));
                }
            }
        }

        None
    }

    /// Add two quantities, converting `b` into `a`'s unit when possible.
    ///
    /// If conversion is impossible in both directions a [`ConversionError`] is
    /// returned.
    ///
    /// ```
    /// use nutrition_rs::nutrition_units::{NutritionQuantity, UnitRegistry};
    ///
    /// let reg = UnitRegistry::with_si_defaults();
    /// let a = NutritionQuantity::new(500.0, "g");
    /// let b = NutritionQuantity::new(0.5, "kg");
    /// let sum = reg.add(&a, &b).unwrap();
    /// assert_eq!(sum.unit, "g");
    /// assert!((sum.amount - 1000.0).abs() < 1e-6);
    /// ```
    pub fn add(
        &self,
        a: &NutritionQuantity,
        b: &NutritionQuantity,
    ) -> Result<NutritionQuantity, ConversionError> {
        if a.unit == b.unit {
            return Ok(NutritionQuantity::new(a.amount + b.amount, &a.unit));
        }
        if let Some(b_conv) = self.convert(b, &a.unit) {
            return Ok(NutritionQuantity::new(a.amount + b_conv.amount, &a.unit));
        }
        if let Some(a_conv) = self.convert(a, &b.unit) {
            return Ok(NutritionQuantity::new(a_conv.amount + b.amount, &b.unit));
        }
        Err(ConversionError {
            from: b.unit.clone(),
            to: a.unit.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_units_for_common_properties() {
        assert_eq!(default_unit_for_property("calories"), Some("kcal"));
        assert_eq!(default_unit_for_property("protein"), Some("g"));
        assert_eq!(default_unit_for_property("fat"), Some("g"));
        assert_eq!(default_unit_for_property("carbohydrates"), Some("g"));
        assert_eq!(default_unit_for_property("carbs"), Some("g"));
        assert_eq!(default_unit_for_property("fiber"), Some("g"));
        assert_eq!(default_unit_for_property("sodium"), Some("g"));
        assert_eq!(default_unit_for_property("cholesterol"), Some("mg"));
        assert_eq!(default_unit_for_property("unknown"), None);
    }

    #[test]
    fn default_units_case_insensitive() {
        assert_eq!(default_unit_for_property("Calories"), Some("kcal"));
        assert_eq!(default_unit_for_property("PROTEIN"), Some("g"));
    }

    #[test]
    fn si_mass_conversions() {
        let reg = UnitRegistry::with_si_defaults();

        let kg = NutritionQuantity::new(1.0, "kg");
        let g = reg.convert(&kg, "g").unwrap();
        assert!((g.amount - 1000.0).abs() < 1e-6);

        let mg = NutritionQuantity::new(1000.0, "mg");
        let g2 = reg.convert(&mg, "g").unwrap();
        assert!((g2.amount - 1.0).abs() < 1e-6);
    }

    #[test]
    fn si_volume_conversions() {
        let reg = UnitRegistry::with_si_defaults();

        let cup = NutritionQuantity::new(1.0, "cup");
        let ml = reg.convert(&cup, "mL").unwrap();
        assert!((ml.amount - 236.588).abs() < 1e-3);
    }

    #[test]
    fn si_energy_conversions() {
        let reg = UnitRegistry::with_si_defaults();

        let kcal = NutritionQuantity::new(1.0, "kcal");
        let cal = reg.convert(&kcal, "cal").unwrap();
        assert!((cal.amount - 1000.0).abs() < 1e-3);
    }

    #[test]
    fn add_same_units() {
        let reg = UnitRegistry::with_si_defaults();
        let a = NutritionQuantity::new(100.0, "g");
        let b = NutritionQuantity::new(200.0, "g");
        let sum = reg.add(&a, &b).unwrap();
        assert_eq!(sum.unit, "g");
        assert!((sum.amount - 300.0).abs() < 1e-9);
    }

    #[test]
    fn add_with_si_conversion() {
        let reg = UnitRegistry::with_si_defaults();
        let a = NutritionQuantity::new(500.0, "g");
        let b = NutritionQuantity::new(0.5, "kg");
        let sum = reg.add(&a, &b).unwrap();
        assert_eq!(sum.unit, "g");
        assert!((sum.amount - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn add_incompatible_units_returns_error() {
        let reg = UnitRegistry::with_si_defaults();
        let a = NutritionQuantity::new(100.0, "g");
        let b = NutritionQuantity::new(1.0, "slice");
        assert!(reg.add(&a, &b).is_err());
    }

    #[test]
    fn ingredient_scoped_equivalency_100g_equals_1cup() {
        let reg = UnitRegistry::from_ingredient_quantities(&[
            (100.0, "g".to_string()),
            (1.0, "cup".to_string()),
        ]);

        let one_cup = NutritionQuantity::new(1.0, "cup");
        let grams = reg.convert(&one_cup, "g").unwrap();
        assert!((grams.amount - 100.0).abs() < 1e-9);

        let two_cups = NutritionQuantity::new(2.0, "cup");
        let grams2 = reg.convert(&two_cups, "g").unwrap();
        assert!((grams2.amount - 200.0).abs() < 1e-9);
    }

    #[test]
    fn ingredient_scoped_equivalency_1pie_equals_8slices() {
        let reg = UnitRegistry::from_ingredient_quantities(&[
            (1.0, "pie".to_string()),
            (8.0, "slices".to_string()),
        ]);

        let half_pie = NutritionQuantity::new(0.5, "pie");
        let slices = reg.convert(&half_pie, "slices").unwrap();
        assert!((slices.amount - 4.0).abs() < 1e-9);

        let two_slices = NutritionQuantity::new(2.0, "slices");
        let pies = reg.convert(&two_slices, "pie").unwrap();
        assert!((pies.amount - 0.25).abs() < 1e-9);
    }

    #[test]
    fn add_with_ingredient_scoped_equivalency() {
        let reg = UnitRegistry::from_ingredient_quantities(&[
            (100.0, "g".to_string()),
            (1.0, "cup".to_string()),
        ]);

        let a = NutritionQuantity::new(200.0, "g");
        let b = NutritionQuantity::new(1.0, "cup");
        let sum = reg.add(&a, &b).unwrap();
        assert_eq!(sum.unit, "g");
        assert!((sum.amount - 300.0).abs() < 1e-9);
    }

    #[test]
    fn multihop_conversion_mg_to_kg() {
        let reg = UnitRegistry::with_si_defaults();
        let mg = NutritionQuantity::new(1_000_000.0, "mg");
        let kg = reg.convert(&mg, "kg").unwrap();
        assert!((kg.amount - 1.0).abs() < 1e-6);
    }

    #[test]
    fn convert_same_unit_is_identity() {
        let reg = UnitRegistry::with_si_defaults();
        let q = NutritionQuantity::new(42.0, "g");
        let result = reg.convert(&q, "g").unwrap();
        assert_eq!(result, q);
    }

    #[test]
    fn convert_unknown_unit_returns_none() {
        let reg = UnitRegistry::with_si_defaults();
        let q = NutritionQuantity::new(1.0, "slice");
        assert!(reg.convert(&q, "g").is_none());
    }

    #[test]
    fn nutrition_quantity_display() {
        let q = NutritionQuantity::new(14.5, "g");
        assert_eq!(q.to_string(), "14.5g");
    }
}
