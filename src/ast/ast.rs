use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub items: Vec<Item>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Item {
    Property(Property),
    Ingredient(Ingredient),
    // Food(Food),
    Recipe(Recipe),
    Exercise(Exercise),
    Day(Day),
    // Meal(Meal),
    Ate(Ate),
    Exercised(Exercised),
    Comment(String),
}


/**
 * Quantity represents a numeric amount with an optional unit.
 * For example, "100g" would have amount 100 and unit "g".
 * There is an optional space between amount and unit in the string representation.
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct Quantity {
    pub amount: f64,
    pub unit: Option<String>,
}

impl Quantity {
    pub fn to_string(&self) -> String {
        match &self.unit {
            Some(unit) => format!("{}{}", self.amount, unit),
            None => format!("{}", self.amount),
        }
    }

    pub fn from_string(s: &str) -> Result<Self, String> {
        let mut chars = s.trim().chars().peekable();
        let mut amount_str = String::new();

        // Parse the numeric prefix (amount)
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '.' {
                amount_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        if amount_str.is_empty() {
            return Err(format!("No numeric amount found in quantity '{}'.", s));
        }

        let amount = amount_str
            .parse::<f64>()
            .map_err(|e| format!("Invalid numeric amount in '{}': {}", s, e))?;

        // Whatever remains (after trimming) is the unit; this avoids leading spaces becoming part of the unit.
        let rest: String = chars.collect();
        let unit_trimmed = rest.trim();
        let unit = if unit_trimmed.is_empty() {
            None
        } else {
            Some(unit_trimmed.to_string())
        };

        Ok(Quantity { amount, unit })
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        if let Some(obj) = value.as_object() {
            let amount = obj.get("amount")
                .and_then(|v| v.as_f64())
                .ok_or("Missing or invalid 'amount' field in Quantity JSON.")?;
            let unit = obj.get("unit")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(Quantity { amount, unit })
        } else {
            Err("Expected a JSON object for Quantity.".to_string())
        }
    }
}

/**
 * Property represents a named property with a quantity value.
 * For example, "calories: 200kcal" would have name "calories" and value Quantity { amount: 200, unit: Some("kcal") }.
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: Quantity,
}

impl Property {
    pub fn to_string(&self) -> String {
        format!("{}: {}", self.name, self.value.to_string())
    }

    pub fn from_string(s: &str) -> Self {
        let parts: Vec<&str> = s.split(':').collect();
        let name = parts[0].trim().to_string();
        let value = Quantity::from_string(parts[1].trim()).unwrap();
        Property { name, value }
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        if let Some(obj) = value.as_object() {
            let name = obj.get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing or invalid 'name' field in Property JSON.")?
                .to_string();
            let value_field = obj.get("value")
                .ok_or("Missing 'value' field in Property JSON.")?;
            let value = Quantity::from_json(value_field)?;
            Ok(Property { name, value })
        } else {
            Err("Expected a JSON object for Property.".to_string())
        }
    }
}


/**
 * Ingredient represents a food ingredient with aliases, quantities, and optionally
 * properties.
 * For example, an ingredient could be "@ingredient(100g) "sugar" { calories: 387kcal }".
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct Ingredient {
    pub aliases: Vec<String>,
    pub quantities: Vec<Quantity>,
    pub properties: Vec<Property>,
}

impl Ingredient {
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str("@ingredient");
        for quantity in &self.quantities {
            result.push('(');
            result.push_str(&quantity.to_string());
            result.push(')');
        }
        result.push(' ');
        for alias in &self.aliases {
            result.push('"');
            result.push_str(alias);
            result.push('"');
            result.push(' ');
        }
        if !self.properties.is_empty() {
            result.push('{');
            for property in &self.properties {
                result.push('\n');
                result.push_str("    "); // indent
                result.push_str(&property.to_string());
            }
            result.push('\n');
            result.push('}'); // close block
        } else {
            result.push_str(" { }");
        }
        result.push('\n');
        result
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        if let Some(obj) = value.as_object() {
            let aliases = obj.get("aliases")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'aliases' field in Ingredient JSON.")?
                .iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect();

            let quantities = obj.get("quantities")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'quantities' field in Ingredient JSON.")?
                .iter()
                .map(Quantity::from_json)
                .collect::<Result<Vec<Quantity>, String>>()?;

            let properties = obj.get("properties")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'properties' field in Ingredient JSON.")?
                .iter()
                .map(Property::from_json)
                .collect::<Result<Vec<Property>, String>>()?;

            Ok(Ingredient { aliases, quantities, properties })
        } else {
            Err("Expected a JSON object for Ingredient.".to_string())
        }
    }

}


/**
 * An ingredient label is used inside a recipe or day block to represent
 * some quantity of an ingredient used. Its alias can refer to any of the
 * aliases used to identify an ingredient.
 * For example, in a recipe, you may have
 * "sugar"(100g)
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct IngredientLabel {
    pub alias: String,
    pub quantity: Quantity,
}

impl IngredientLabel {
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        result.push('"');
        result.push_str(&self.alias);
        result.push('"');
        result.push('(');
        result.push_str(&self.quantity.to_string());
        result.push(')');
        result
    }
}

/**
 * Recipe represents a recipe with aliases, quantities, and ingredients.
 * For example, a recipe could be "@recipe(4 servings) "Pancakes" { "flour"(200g) "milk"(300ml) }".
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub aliases: Vec<String>,
    pub quantities: Vec<Quantity>,
    pub ingredients: Vec<IngredientLabel>,
}

impl Recipe {
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str("@recipe");
        for quantity in &self.quantities {
            result.push('(');
            result.push_str(&quantity.to_string());
            result.push(')');
        }
        result.push(' ');
        for alias in &self.aliases {
            result.push('"');
            result.push_str(alias);
            result.push('"');
            result.push(' ');
        }
        result.push('{');
        if self.ingredients.is_empty() {
            result.push_str(" }");
        } else {
            for ingredient in &self.ingredients {
                result.push('\n');
                result.push_str("    "); // indent
                result.push_str(&ingredient.to_string());
            }
            result.push('\n');
            result.push('}'); // close block
        }
        result.push('\n');
        result
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        if let Some(obj) = value.as_object() {
            let aliases = obj.get("aliases")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'aliases' field in Recipe JSON.")?
                .iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect();

            let quantities = obj.get("quantities")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'quantities' field in Recipe JSON.")?
                .iter()
                .map(Quantity::from_json)
                .collect::<Result<Vec<Quantity>, String>>()?;

            let ingredients = obj.get("ingredients")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'ingredients' field in Recipe JSON.")?
                .iter()
                .map(|ing_val| {
                    if let Some(ing_obj) = ing_val.as_object() {
                        let alias = ing_obj.get("alias")
                            .and_then(|v| v.as_str())
                            .ok_or("Missing or invalid 'alias' field in IngredientLabel JSON.")?
                            .to_string();
                        let quantity_field = ing_obj.get("quantity")
                            .ok_or("Missing 'quantity' field in IngredientLabel JSON.")?;
                        let quantity = Quantity::from_json(quantity_field)?;
                        Ok(IngredientLabel { alias, quantity })
                    } else {
                        Err("Expected a JSON object for IngredientLabel.".to_string())
                    }
                })
                .collect::<Result<Vec<IngredientLabel>, String>>()?;

            Ok(Recipe { aliases, quantities, ingredients })
        } else {
            Err("Expected a JSON object for Recipe.".to_string())
        }
    }
}

/**
 * Exercise represents a physical exercise with aliases, quantities, and optionally
 * properties.
 * For example, an exercise could be "@exercise(30min) "running" { calories: 300kcal }".
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct Exercise {
    pub aliases: Vec<String>,
    pub quantities: Vec<Quantity>,
    pub properties: Vec<Property>,
}

impl Exercise {
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str("@exercise");
        for quantity in &self.quantities {
            result.push('(');
            result.push_str(&quantity.to_string());
            result.push(')');
        }
        result.push(' ');
        for alias in &self.aliases {
            result.push('"');
            result.push_str(alias);
            result.push('"');
            result.push(' ');
        }
        if !self.properties.is_empty() {
            result.push('{');
            for property in &self.properties {
                result.push('\n');
                result.push_str("    "); // indent
                result.push_str(&property.to_string());
            }
            result.push('\n');
            result.push('}'); // close block
        } else {
            result.push_str(" { }");
        }
        result.push('\n');
        result
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        if let Some(obj) = value.as_object() {
            let aliases = obj.get("aliases")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'aliases' field in Exercise JSON.")?
                .iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect();

            let quantities = obj.get("quantities")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'quantities' field in Exercise JSON.")?
                .iter()
                .map(Quantity::from_json)
                .collect::<Result<Vec<Quantity>, String>>()?;

            let properties = obj.get("properties")
                .and_then(|v| v.as_array())
                .ok_or("Missing or invalid 'properties' field in Exercise JSON.")?
                .iter()
                .map(Property::from_json)
                .collect::<Result<Vec<Property>, String>>()?;

            Ok(Exercise { aliases, quantities, properties })
        } else {
            Err("Expected a JSON object for Exercise.".to_string())
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ate {
    pub food_alias: String,
    pub quantity: Quantity,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Exercised {
    pub exercise_alias: String,
    pub quantity: Quantity,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DayItem {
    Ate(Ate),
    Exercised(Exercised),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Day {
    pub date: String,
    pub items: Vec<DayItem>,
}

impl Day {
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str("@day ");
        result.push('"');
        result.push_str(&self.date);
        result.push('"');
        result.push_str(" {\n");
        for item in &self.items {
            match item {
                DayItem::Ate(ate) => {
                    result.push_str("    @ate \"");
                    result.push_str(&ate.food_alias);
                    result.push_str("\"(");
                    result.push_str(&ate.quantity.to_string());
                    result.push_str(")\n");
                }
                DayItem::Exercised(exercised) => {
                    result.push_str("    @exercised \"");
                    result.push_str(&exercised.exercise_alias);
                    result.push_str("\"(");
                    result.push_str(&exercised.quantity.to_string());
                    result.push_str(")\n");
                }
            }
        }
        result.push_str("}\n");
        result
    }
}