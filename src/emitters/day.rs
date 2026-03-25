use crate::ast::ast::{Day, DayItem};
use crate::emitters::emitter::{CanEmit, quoted_string};

pub struct DayEmitter;

const DAY_KEYWORD: &str = "@day";

impl CanEmit<Day> for DayEmitter {
    fn emit(&self, day: &Day) -> String {
        let mut output = String::new();
        
        // Emit day keyword
        output.push_str(DAY_KEYWORD);
        output.push(' ');

        // Emit date
        quoted_string(&mut output, &day.date);
        output.push(' ');

        // start block
        output.push('{');

        // Emit ate entries
        for item in &day.items {
            match item {
                DayItem::Ate(ate) => {
                    output.push('\n');
                    output.push_str("    "); // indent
                    output.push_str("@ate ");
                    quoted_string(&mut output, &ate.food_alias);
                    output.push('(');
                    output.push_str(&ate.quantity.to_string());
                    output.push(')');
                }

                DayItem::Exercised(exercised) => {
                    output.push('\n');
                    output.push_str("    "); // indent
                    output.push_str("@exercised ");
                    quoted_string(&mut output, &exercised.exercise_alias);
                    output.push('(');
                    output.push_str(&exercised.quantity.to_string());
                    output.push(')');
                }

                
                DayItem::Meal(meal_label) => {
                    output.push('\n');
                    output.push_str("    ");
                    output.push('[');
                    output.push_str(meal_label);
                    output.push(']');
                }
            }
        }

        output.push('\n');
        output.push('}'); // close block
        output.push('\n');

        output
    }
}