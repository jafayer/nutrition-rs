#[derive(Debug)]
pub struct Document {
    pub items: Vec<Item>,
}

#[derive(Debug)]
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
#[derive(Debug)]
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
}

/**
 * Property represents a named property with a quantity value.
 * For example, "calories: 200kcal" would have name "calories" and value Quantity { amount: 200, unit: Some("kcal") }.
 */
#[derive(Debug)]
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
}


/**
 * Ingredient represents a food ingredient with aliases, quantities, and optionally
 * properties.
 * For example, an ingredient could be "@ingredient(100g) "sugar" { calories: 387kcal }".
 */
#[derive(Debug)]
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

}


/**
 * An ingredient label is used inside a recipe or day block to represent
 * some quantity of an ingredient used. Its alias can refer to any of the
 * aliases used to identify an ingredient.
 * For example, in a recipe, you may have
 * "sugar"(100g)
 */
#[derive(Debug)]
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
#[derive(Debug)]
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
}

/**
 * Exercise represents a physical exercise with aliases, quantities, and optionally
 * properties.
 * For example, an exercise could be "@exercise(30min) "running" { calories: 300kcal }".
 */
#[derive(Debug)]
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
}

#[derive(Debug)]
pub struct Ate {
    pub food_alias: String,
    pub quantity: Quantity,
}

#[derive(Debug)]
pub struct Exercised {
    pub exercise_alias: String,
    pub quantity: Quantity,
}

#[derive(Debug)]
pub enum DayItem {
    Ate(Ate),
    Exercised(Exercised),
}

#[derive(Debug)]
pub struct Day {
    pub date: String,
    pub items: Vec<DayItem>,
}