pub mod day;
pub mod ingredient;
pub mod recipe;
use crate::cli::generate::day::DayGenerateArgs;
use crate::cli::generate::ingredient::IngredientGenerateArgs;
use crate::cli::generate::recipe::RecipeGenerateArgs;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about = "Generate various outputs from nutrition data")]
pub enum GenerateCommands {
    #[clap(about = "Generate recipe markup", visible_aliases = ["rec", "r"])]
    Recipe(RecipeGenerateArgs),
    #[clap(about = "Generate ingredient markup", visible_aliases = ["ing", "i"])]
    Ingredient(IngredientGenerateArgs),
    #[clap(about = "Generate day markup", visible_aliases = ["d"])]
    Day(DayGenerateArgs),
}
