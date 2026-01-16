pub mod recipe;
pub mod ingredient;
use crate::cli::generate::recipe::RecipeGenerateArgs;
use crate::cli::generate::ingredient::IngredientGenerateArgs;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about = "Generate various outputs from nutrition data")]
pub enum GenerateCommands {
    #[clap(about = "Generate recipe emitter output")]
    Recipe(RecipeGenerateArgs),
    Ingredient(IngredientGenerateArgs),
}