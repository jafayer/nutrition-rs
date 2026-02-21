use crate::ast::ast::*;
use std::collections::HashMap;

/// Semantic analyzer that indexes a parsed [`Document`] for alias-based lookups
/// and optional nutritional calculations.
pub struct SemanticAnalyzer {
    /// Map of ingredient aliases to their definitions
    ingredients: HashMap<String, Ingredient>,
    /// Map of recipe aliases to their definitions
    recipes: HashMap<String, Recipe>,
    /// Map of exercise aliases to their definitions
    exercises: HashMap<String, Exercise>,
    // Ordered list of days
    days: Vec<Day>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        SemanticAnalyzer {
            ingredients: HashMap::new(),
            recipes: HashMap::new(),
            exercises: HashMap::new(),
            days: Vec::new(),
        }
    }

    /// Index the items in `document` for alias-based lookups and return it.
    /// This method always succeeds and returns `Ok(document)`.
    pub fn analyze(&mut self, document: Document) -> Result<Document, String> {
        for item in &document.items {
            match item {
                Item::Ingredient(ing) => {
                    for alias in &ing.aliases {
                        self.ingredients.insert(alias.clone(), ing.clone());
                    }
                }
                Item::Recipe(rec) => {
                    for alias in &rec.aliases {
                        self.recipes.insert(alias.clone(), rec.clone());
                    }
                }
                Item::Exercise(ex) => {
                    for alias in &ex.aliases {
                        self.exercises.insert(alias.clone(), ex.clone());
                    }
                }
                Item::Day(day) => {
                    self.days.push(day.clone());
                }
                _ => {}
            }
        }
        Ok(document)
    }

    /// Get a resolved ingredient by alias
    pub fn get_ingredient(&self, alias: &str) -> Option<&Ingredient> {
        self.ingredients.get(alias)
    }

    /// Get a resolved recipe by alias
    pub fn get_recipe(&self, alias: &str) -> Option<&Recipe> {
        self.recipes.get(alias)
    }

    /// Get a resolved exercise by alias
    pub fn get_exercise(&self, alias: &str) -> Option<&Exercise> {
        self.exercises.get(alias)
    }

    /// Calculate nutritional properties for a recipe by resolving ingredient
    /// references.
    pub fn calculate_recipe_properties(&self, recipe: &Recipe) -> Result<Vec<Property>, String> {
        let mut totals: HashMap<String, f64> = HashMap::new();

        for ingredient_ref in &recipe.ingredients {
            let ingredient = self.get_ingredient(&ingredient_ref.alias)
                .ok_or_else(|| format!("Unknown ingredient: {}", ingredient_ref.alias))?;

            let scale = if !ingredient.quantities.is_empty() {
                let ingredient_qty = &ingredient.quantities[0];
                ingredient_ref.quantity.amount / ingredient_qty.amount
            } else {
                1.0
            };

            for prop in &ingredient.properties {
                let scaled_value = prop.value.amount * scale;
                *totals.entry(prop.name.clone()).or_insert(0.0) += scaled_value;
            }
        }

        let properties = totals
            .into_iter()
            .map(|(name, value)| Property {
                name,
                value: Quantity {
                    amount: value,
                    unit: None,
                },
            })
            .collect();

        Ok(properties)
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
