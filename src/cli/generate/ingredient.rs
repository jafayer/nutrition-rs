use crate::emitters::emitter::CanEmit;
use crate::emitters::ingredient::IngredientEmitter;
use crate::ast::ast::{Ingredient, Quantity, Property};
use clap::Parser;

#[cfg(feature = "runtime")]
use crate::emitters::emitter::CanEmitAI;

#[derive(Parser, Debug)]
#[clap(about = "Generate various outputs from nutrition data")]
pub enum GenerateCommands {
    #[clap(about = "Generate recipe emitter output")]
    Ingredient(IngredientGenerateArgs),
}

#[derive(Parser, Debug)]
#[clap(about = "Generate recipe emitter output")]
pub struct IngredientGenerateArgs {
    #[clap(short = 'q', long = "quantity", help = "Number of servings")]
    pub quantities: Vec<String>,
    #[clap(short = 'a', long = "alias", help = "Aliases for the recipe")]
    pub aliases: Vec<String>,
    #[clap(short = 'p', long = "property", help = "Properties for the recipe in the format 'label:quantity'")]
    pub properties: Vec<String>,
    #[clap(long = "ai", help = "Use AI to generate ingredient details")]
    pub ai: bool,
}

impl IngredientGenerateArgs {
    pub fn to_ingredient(&self) -> Ingredient {
        let quantities = self.quantities.iter().map(|q_str| {
            Quantity::from_string(q_str).unwrap()
        }).collect();

        let properties = self.properties.iter().map(|prop_str| {
            let parts: Vec<&str> = prop_str.splitn(2, ':').collect();
            let name = parts[0].trim().to_string();
            let value = Quantity::from_string(parts[1].trim()).unwrap();
            Property { name, value }
        }).collect();

        Ingredient {
            aliases: self.aliases.clone(),
            quantities,
            properties,
        }
    }

    pub fn emit(&self) -> String {
        let ingredient = self.to_ingredient();
        let emitter = IngredientEmitter;
        emitter.emit(&ingredient)
    }

    /// Emit the ingredient, optionally using the AI backend when `--ai` is
    /// passed.  Only available with the `runtime` feature.
    #[cfg(feature = "runtime")]
    pub async fn emit_with_ai(&self) -> String {
        let ingredient = self.to_ingredient();
        let emitter = IngredientEmitter;

        let processed_ingredient = if self.ai {
            match emitter.emit_ai(&ingredient).await {
                Ok(ai_ingredient) => ai_ingredient,
                Err(err) => return format!("AI emission failed: {}", err),
            }
        } else {
            ingredient
        };

        emitter.emit(&processed_ingredient)
    }
}