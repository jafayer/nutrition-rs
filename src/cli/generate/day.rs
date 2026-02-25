use crate::ast::ast::{Ate, Day, DayItem, Exercised};
use crate::emitters::day::DayEmitter;
use crate::emitters::emitter::CanEmit;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about = "Generate day emitter output")]
pub enum GenerateCommands {
    #[clap(about = "Generate day emitter output")]
    Day(DayGenerateArgs),
}

#[derive(Parser, Debug)]
#[clap(about = "Generate day emitter output")]
pub struct DayGenerateArgs {
    #[clap(
        short = 'd',
        long = "date",
        help = "Date for the day entry (e.g., '2024-06-15')"
    )]
    pub date: String,
    #[clap(
        short = 'a',
        long = "ate",
        help = "Food eaten in the format '\"chana masala\"(2 servings)'"
    )]
    pub ate: Vec<String>,
    #[clap(
        short = 'e',
        long = "exercised",
        help = "Exercise performed in the format '\"cycling\"(15m)'"
    )]
    pub exercised: Vec<String>,
}

impl DayGenerateArgs {
    pub fn to_day(&self) -> Day {
        let mut items: Vec<DayItem> = Vec::new();

        for ate_str in &self.ate {
            // Parse ate string
            if let Some((food_alias, quantity_str)) = ate_str.split_once('(') {
                let food_alias = food_alias.trim().trim_matches('"').to_string();
                let quantity_str = quantity_str.trim_end_matches(')').trim();
                let quantity = crate::ast::ast::Quantity::from_string(quantity_str).unwrap();

                let ate = Ate {
                    food_alias,
                    quantity,
                };
                items.push(DayItem::Ate(ate));
            }
        }

        for exercised_str in &self.exercised {
            // Parse exercised string
            if let Some((exercise_alias, quantity_str)) = exercised_str.split_once('(') {
                let exercise_alias = exercise_alias.trim().trim_matches('"').to_string();
                let quantity_str = quantity_str.trim_end_matches(')').trim();
                let quantity = crate::ast::ast::Quantity::from_string(quantity_str).unwrap();

                let exercised = Exercised {
                    exercise_alias,
                    quantity,
                };
                items.push(DayItem::Exercised(exercised));
            }
        }

        Day {
            date: self.date.clone(),
            items,
        }
    }

    pub fn emit(&self) -> String {
        let day = self.to_day();
        let emitter = DayEmitter;
        emitter.emit(&day)
    }
}
