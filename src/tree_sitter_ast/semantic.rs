use crate::ast::ast::*;
use std::collections::HashMap;
use tree_sitter::Node;

/// Semantic analyzer that converts tree-sitter parse trees into semantic ASTs
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

    /// Analyze a tree-sitter parse tree and build a semantic AST
    pub fn analyze(&mut self, root: Node, source: &str) -> Result<Document, String> {
        let mut items = Vec::new();

        // First pass: collect all definitions (ingredients, recipes, exercises, days)
        self.collect_definitions(root, source)?;

        // Second pass: build the document with resolved references
        for child in root.children(&mut root.walk()) {
            if let Ok(item) = self.parse_item(child, source) {
                items.push(item);
            }
        }

        Ok(Document { items })
    }

    /// First pass: collect all ingredient, recipe, exercise, and day definitions
    fn collect_definitions(&mut self, node: Node, source: &str) -> Result<(), String> {
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "ingredient_decl" => {
                    if let Ok(ingredient) = self.parse_ingredient_def(child, source) {
                        for alias in &ingredient.aliases {
                            self.ingredients.insert(alias.clone(), ingredient.clone());
                        }
                    }
                }
                "recipe_decl" => {
                    if let Ok(recipe) = self.parse_recipe_def(child, source) {
                        for alias in &recipe.aliases {
                            self.recipes.insert(alias.clone(), recipe.clone());
                        }
                    }
                }
                "exercise_decl" => {
                    if let Ok(exercise) = self.parse_exercise_def(child, source) {
                        for alias in &exercise.aliases {
                            self.exercises.insert(alias.clone(), exercise.clone());
                        }
                    }
                }
                "day_decl" => {
                    if let Ok(day) = self.parse_day(child, source) {
                        self.days.push(day);
                    }
                }
                _ => {
                    // Recursively collect from nested nodes
                    let _ = self.collect_definitions(child, source);
                }
            }
        }
        Ok(())
    }

    fn parse_item(&mut self, node: Node, source: &str) -> Result<Item, String> {
        match node.kind() {
            "ingredient_decl" => Ok(Item::Ingredient(self.parse_ingredient_def(node, source)?)),
            "recipe_decl" => Ok(Item::Recipe(self.parse_recipe_def(node, source)?)),
            "exercise_decl" => Ok(Item::Exercise(self.parse_exercise_def(node, source)?)),
            "day_decl" => Ok(Item::Day(self.parse_day(node, source)?)),
            "ate_entry" => Ok(Item::Ate(self.parse_ate(node, source)?)),
            "exercised_entry" => Ok(Item::Exercised(self.parse_exercised(node, source)?)),
            "comment" => {
                let text = source[node.start_byte()..node.end_byte()].to_string();
                Ok(Item::Comment(text))
            }
            _ => Err(format!("Unknown item type: {}", node.kind())),
        }
    }

    fn parse_ingredient_def(&self, node: Node, source: &str) -> Result<Ingredient, String> {
        let mut aliases = Vec::new();
        let mut quantities = Vec::new();
        let mut properties = Vec::new();

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "string" => {
                    let text = self.extract_string_value(child, source);
                    aliases.push(text);
                }
                "paren_quantity" => {
                    if let Ok(qty) = self.parse_paren_quantity(child, source) {
                        quantities.push(qty);
                    }
                }
                "block" => {
                    // Parse properties from the block
                    for block_child in child.children(&mut child.walk()) {
                        if block_child.kind() == "block_item" {
                            for item_child in block_child.children(&mut block_child.walk()) {
                                if item_child.kind() == "property_assignment" {
                                    if let Ok(prop) = self.parse_property(item_child, source) {
                                        properties.push(prop);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if aliases.is_empty() {
            return Err("Ingredient must have at least one alias".to_string());
        }

        Ok(Ingredient {
            aliases,
            quantities,
            properties,
        })
    }

    fn parse_recipe_def(&self, node: Node, source: &str) -> Result<Recipe, String> {
        let mut aliases = Vec::new();
        let mut quantities = Vec::new();
        let mut ingredients = Vec::new();

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "string" => {
                    let text = self.extract_string_value(child, source);
                    aliases.push(text);
                }
                "paren_quantity" => {
                    if let Ok(qty) = self.parse_paren_quantity(child, source) {
                        quantities.push(qty);
                    }
                }
                "recipe_item" => {
                    // Parse recipe_ingredient_line from recipe_item
                    for item_child in child.children(&mut child.walk()) {
                        if item_child.kind() == "recipe_ingredient_line" {
                            if let Ok(ing) = self.parse_recipe_ingredient_line(item_child, source) {
                                ingredients.push(ing);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if aliases.is_empty() {
            return Err("Recipe must have at least one alias".to_string());
        }

        Ok(Recipe {
            aliases,
            quantities,
            ingredients,
        })
    }

    fn parse_exercise_def(&self, node: Node, source: &str) -> Result<Exercise, String> {
        let mut aliases = Vec::new();
        let mut quantities = Vec::new();
        let mut properties = Vec::new();

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "string" => {
                    let text = self.extract_string_value(child, source);
                    aliases.push(text);
                }
                "paren_quantity" => {
                    if let Ok(qty) = self.parse_paren_quantity(child, source) {
                        quantities.push(qty);
                    }
                }
                "block" => {
                    // Parse properties from the block
                    for block_child in child.children(&mut child.walk()) {
                        if block_child.kind() == "block_item" {
                            for item_child in block_child.children(&mut block_child.walk()) {
                                if item_child.kind() == "property_assignment" {
                                    if let Ok(prop) = self.parse_property(item_child, source) {
                                        properties.push(prop);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if aliases.is_empty() {
            return Err("Exercise must have at least one alias".to_string());
        }

        Ok(Exercise {
            aliases,
            quantities,
            properties,
        })
    }

    fn parse_day(&self, node: Node, source: &str) -> Result<Day, String> {
        let mut date = String::new();
        let mut items = Vec::new();

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "string" => {
                    if date.is_empty() {
                        date = self.extract_string_value(child, source);
                    }
                }
                "day_item" => {
                    // Parse day_item to find ate_entry or exercised_entry
                    for item_child in child.children(&mut child.walk()) {
                        if item_child.kind() == "ate_entry" {
                            if let Ok(ate) = self.parse_ate(item_child, source) {
                                items.push(DayItem::Ate(ate));
                            }
                        } else if item_child.kind() == "exercised_entry" {
                            if let Ok(exercised) = self.parse_exercised(item_child, source) {
                                items.push(DayItem::Exercised(exercised));
                            }
                        }
                    }
                }
                "ate_entry" => {
                    if let Ok(ate) = self.parse_ate(child, source) {
                        items.push(DayItem::Ate(ate));
                    }
                }
                "exercised_entry" => {
                    if let Ok(exercised) = self.parse_exercised(child, source) {
                        items.push(DayItem::Exercised(exercised));
                    }
                }
                _ => {}
            }
        }

        if date.is_empty() {
            return Err("Day must have a date".to_string());
        }

        Ok(Day { date, items })
    }

    fn parse_ate(&self, node: Node, source: &str) -> Result<Ate, String> {
        let mut food_alias = String::new();
        let mut quantity = Quantity {
            amount: 1.0,
            unit: None,
        };

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "string" => {
                    if food_alias.is_empty() {
                        food_alias = self.extract_string_value(child, source);
                    }
                }
                "paren_quantity" => {
                    if let Ok(qty) = self.parse_paren_quantity(child, source) {
                        quantity = qty;
                    }
                }
                _ => {}
            }
        }

        if food_alias.is_empty() {
            return Err("Ate must reference a food".to_string());
        }

        Ok(Ate {
            food_alias,
            quantity,
        })
    }

    fn parse_exercised(&self, node: Node, source: &str) -> Result<Exercised, String> {
        let mut exercise_alias = String::new();
        let mut quantity = Quantity {
            amount: 1.0,
            unit: None,
        };

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "string" => {
                    if exercise_alias.is_empty() {
                        exercise_alias = self.extract_string_value(child, source);
                    }
                }
                "paren_quantity" => {
                    if let Ok(qty) = self.parse_paren_quantity(child, source) {
                        quantity = qty;
                    }
                }
                _ => {}
            }
        }

        if exercise_alias.is_empty() {
            return Err("Exercised must reference an exercise".to_string());
        }

        Ok(Exercised {
            exercise_alias,
            quantity,
        })
    }

    fn parse_recipe_ingredient_line(
        &self,
        node: Node,
        source: &str,
    ) -> Result<IngredientLabel, String> {
        // recipe_ingredient_line: $ => seq($.string, $.paren_quantity),
        let mut alias = String::new();
        let mut quantity = Quantity {
            amount: 1.0,
            unit: None,
        };

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "string" => {
                    if alias.is_empty() {
                        alias = self.extract_string_value(child, source);
                    }
                }
                "paren_quantity" => {
                    if let Ok(qty) = self.parse_paren_quantity(child, source) {
                        quantity = qty;
                    }
                }
                _ => {}
            }
        }

        if alias.is_empty() {
            return Err("Recipe ingredient line must have an alias".to_string());
        }

        Ok(IngredientLabel { alias, quantity })
    }

    fn parse_paren_quantity(&self, node: Node, source: &str) -> Result<Quantity, String> {
        // paren_quantity: $ => seq('(', $.number, optional($.unit_token), ')')
        let mut amount = 1.0;
        let mut unit = None;

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "number" => {
                    let text = source[child.start_byte()..child.end_byte()].trim();
                    if let Ok(num) = text.parse::<f64>() {
                        amount = num;
                    }
                }
                "unit_token" => {
                    // unit_token can be a string or identifier
                    for unit_child in child.children(&mut child.walk()) {
                        if unit_child.kind() == "identifier" || unit_child.kind() == "string" {
                            unit = Some(self.extract_string_value(unit_child, source));
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Quantity { amount, unit })
    }

    fn parse_quantity(&self, node: Node, source: &str) -> Result<Quantity, String> {
        let text = source[node.start_byte()..node.end_byte()].trim();
        let mut amount = 1.0;
        let mut unit = None;

        // Parse amount and unit from text like "100g", "1 cup", "0.5 pie"
        let parts: Vec<&str> = text.split_whitespace().collect();
        if !parts.is_empty() {
            if let Ok(num) = parts[0].parse::<f64>() {
                amount = num;
                if parts.len() > 1 {
                    unit = Some(parts[1..].join(" "));
                }
            } else if let Ok(num) = self.parse_numeric_with_unit(parts[0]) {
                amount = num.0;
                unit = Some(num.1);
            }
        }

        Ok(Quantity { amount, unit })
    }

    fn parse_property(&self, node: Node, source: &str) -> Result<Property, String> {
        let mut name = String::new();
        let mut value = Quantity {
            amount: 0.0,
            unit: None,
        };

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "identifier" => {
                    if name.is_empty() {
                        name = source[child.start_byte()..child.end_byte()].to_string();
                    }
                }
                // The grammar uses a `value` node wrapping `number_with_unit`.
                "value" => {
                    if let Ok(qty) = self.parse_value(child, source) {
                        value = qty;
                    }
                }
                // Legacy fallback kept for any grammar variants that emit `quantity` directly.
                "quantity" => {
                    if let Ok(qty) = self.parse_quantity(child, source) {
                        value = qty;
                    }
                }
                _ => {}
            }
        }

        if name.is_empty() {
            return Err("Property must have a name".to_string());
        }

        Ok(Property { name, value })
    }

    /// Parse a `value` node (grammar rule: `value: number_with_unit | bool | string`).
    fn parse_value(&self, node: Node, source: &str) -> Result<Quantity, String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "number_with_unit" {
                return self.parse_number_with_unit(child, source);
            }
        }
        // Fallback: try to parse the raw text.
        self.parse_quantity(node, source)
    }

    /// Parse a `number_with_unit` node (grammar rule: `seq(number, optional(unit_token))`).
    fn parse_number_with_unit(&self, node: Node, source: &str) -> Result<Quantity, String> {
        let mut amount = 0.0;
        let mut unit: Option<String> = None;

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "number" => {
                    let text = source[child.start_byte()..child.end_byte()].trim();
                    if let Ok(num) = text.parse::<f64>() {
                        amount = num;
                    }
                }
                "unit_token" => {
                    // unit_token is a string or identifier
                    for unit_child in child.children(&mut child.walk()) {
                        if unit_child.kind() == "identifier" || unit_child.kind() == "string" {
                            unit = Some(self.extract_string_value(unit_child, source));
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Quantity { amount, unit })
    }

    fn extract_string_value(&self, node: Node, source: &str) -> String {
        let text = source[node.start_byte()..node.end_byte()].trim();
        // Remove quotes
        if text.starts_with('"') && text.ends_with('"') {
            text[1..text.len() - 1].to_string()
        } else {
            text.to_string()
        }
    }

    /// Parse a numeric value with attached unit (e.g., "100g" -> (100.0, "g"))
    fn parse_numeric_with_unit(&self, text: &str) -> Result<(f64, String), String> {
        let mut amount_str = String::new();
        let mut unit_str = String::new();
        let mut found_letter = false;

        for ch in text.chars() {
            if ch.is_numeric() || ch == '.' {
                if !found_letter {
                    amount_str.push(ch);
                } else {
                    return Err("Invalid number format".to_string());
                }
            } else {
                found_letter = true;
                unit_str.push(ch);
            }
        }

        if amount_str.is_empty() || unit_str.is_empty() {
            return Err("Invalid format".to_string());
        }

        let amount = amount_str
            .parse::<f64>()
            .map_err(|_| "Failed to parse amount".to_string())?;
        Ok((amount, unit_str))
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

    /// Calculate nutritional properties for a recipe by resolving ingredient references
    pub fn calculate_recipe_properties(&self, recipe: &Recipe) -> Result<Vec<Property>, String> {
        let mut totals: HashMap<String, f64> = HashMap::new();

        for ingredient_ref in &recipe.ingredients {
            let ingredient = self.get_ingredient(&ingredient_ref.alias)
                .ok_or_else(|| format!("Unknown ingredient: {}", ingredient_ref.alias))?;

            // Calculate scaling factor based on ingredient quantity
            let scale = if !ingredient.quantities.is_empty() {
                let ingredient_qty = &ingredient.quantities[0];
                ingredient_ref.quantity.amount / ingredient_qty.amount
            } else {
                1.0
            };

            // Accumulate properties
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

impl Clone for Ingredient {
    fn clone(&self) -> Self {
        Ingredient {
            aliases: self.aliases.clone(),
            quantities: self.quantities.clone(),
            properties: self.properties.clone(),
        }
    }
}

impl Clone for Quantity {
    fn clone(&self) -> Self {
        Quantity {
            amount: self.amount,
            unit: self.unit.clone(),
        }
    }
}

impl Clone for Property {
    fn clone(&self) -> Self {
        Property {
            name: self.name.clone(),
            value: self.value.clone(),
        }
    }
}

impl Clone for Recipe {
    fn clone(&self) -> Self {
        Recipe {
            aliases: self.aliases.clone(),
            quantities: self.quantities.clone(),
            ingredients: self.ingredients.clone(),
        }
    }
}

impl Clone for IngredientLabel {
    fn clone(&self) -> Self {
        IngredientLabel {
            alias: self.alias.clone(),
            quantity: self.quantity.clone(),
        }
    }
}

impl Clone for Exercise {
    fn clone(&self) -> Self {
        Exercise {
            aliases: self.aliases.clone(),
            quantities: self.quantities.clone(),
            properties: self.properties.clone(),
        }
    }
}
