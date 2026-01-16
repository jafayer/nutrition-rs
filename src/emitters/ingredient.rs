use crate::ast::ast::Ingredient;
use crate::emitters::emitter::{CanEmit, quoted_string, format_quantity};

pub struct IngredientEmitter;

const INGREDIENT_KEYWORD: &str = "@ingredient";

impl CanEmit<Ingredient> for IngredientEmitter {
    fn emit(&self, ingredient: &Ingredient) -> String {
        let mut output = String::new();
        
        // Emit ingredient keyword
        output.push_str(INGREDIENT_KEYWORD);
        
        // Emit quantity
        for quantity in &ingredient.quantities {
            output.push('(');
            output.push_str(&format_quantity(quantity));
            output.push(')');
        }
        output.push(' ');

        // Emit label
        for alias in &ingredient.aliases {
            quoted_string(&mut output, alias);
            output.push(' ');
        }
        
        
        // Emit properties if any
        if !ingredient.properties.is_empty() {
            output.push('{');
            for property in &ingredient.properties {
                output.push('\n');
                output.push_str("    "); // indent
                output.push_str(&property.name); // property names are NOT quoted
                output.push_str(": ");
                output.push_str(&format_quantity(&property.value));
            }
            output.push('\n');
            output.push('}'); // close block
        } else {
            // add empty properties list
            output.push_str(" { }");
        }

        output.push('\n');
        
        output
    }
}