use crate::ast::ast::{IngredientLabel, Quantity, Recipe};
use crate::emitters::emitter::CanEmit;
use crate::emitters::recipe::RecipeEmitter;
use clap::Parser;

// usage nutrition gen recipe \
//    --quantity 200g \
//    --quantity 1cup \
//    --alias "chickpeas" \
//   --alias "chickpea" \
//   --alias "garbanzo beans" \
//   --ingredient "chickpeas"(200g) \
//   --ingredient "olive oil"(100ml)

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct RecipeGenerateArgs {
    #[clap(long = "quantity", short = 'q', required = true)]
    pub quantities: Vec<String>,

    #[clap(long = "alias", short = 'a', required = true)]
    pub aliases: Vec<String>,

    #[clap(long = "ingredient")]
    pub ingredients: Vec<String>,
}

impl RecipeGenerateArgs {
    pub fn to_recipe(&self) -> Recipe {
        let quantities = self
            .quantities
            .iter()
            .map(|q_str| parse_quantity(q_str))
            .collect();

        let ingredients = self
            .ingredients
            .iter()
            .map(|ing_str| {
                let parts: Vec<&str> = ing_str.split('(').collect();
                let alias = parts[0].to_string();
                let quantity = parse_quantity(parts[1].trim_end_matches(')'));
                IngredientLabel { alias, quantity }
            })
            .collect();

        Recipe {
            quantities,
            aliases: self.aliases.clone(),
            ingredients,
        }
    }

    pub fn emit(&self) -> String {
        let recipe = self.to_recipe();
        let emitter = RecipeEmitter;
        emitter.emit(&recipe)
    }

    pub fn print(&self) {
        let output = self.emit();
        println!("{}", output);
    }
}

fn parse_quantity(q_str: &str) -> Quantity {
    // Simple parser for quantity strings like "200g" or "1cup"
    let mut amount_str = String::new();
    let mut unit_str = String::new();

    for c in q_str.chars() {
        if c.is_digit(10) || c == '.' {
            amount_str.push(c);
        } else {
            unit_str.push(c);
        }
    }

    let amount: f64 = amount_str.parse().unwrap_or(0.0);
    let unit = if unit_str.is_empty() {
        None
    } else {
        Some(unit_str)
    };

    Quantity { amount, unit }
}
