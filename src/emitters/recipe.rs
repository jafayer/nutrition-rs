use crate::ast::ast::Recipe;
use crate::emitters::emitter::{CanEmit, format_quantity, quoted_string};

pub struct RecipeEmitter;

const RECIPE_KEYWORD: &str = "@recipe";

impl CanEmit<Recipe> for RecipeEmitter {
    fn emit(&self, recipe: &Recipe) -> String {
        let mut output = String::new();

        // Emit aliases
        output.push_str(RECIPE_KEYWORD);
        for quantity in &recipe.quantities {
            output.push('(');
            output.push_str(&format_quantity(quantity));
            output.push(')');
        }
        output.push(' ');

        for alias in recipe.aliases.iter() {
            quoted_string(&mut output, alias);
            output.push(' ');
        }

        // start block
        output.push('{');

        if recipe.ingredients.is_empty() {
            // add empty ingredients list
            output.push_str(" }");
        } else {
            for ingredient in &recipe.ingredients {
                output.push('\n');
                output.push_str("    "); // indent

                // emit quantity
                quoted_string(&mut output, &ingredient.alias);
                output.push('(');
                output.push_str(&format_quantity(&ingredient.quantity));
                output.push(')');
            }
            output.push('\n');
            output.push('}'); // close block
        }

        output.push('\n');

        output
    }
}
