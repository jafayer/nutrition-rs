use crate::ast::ast::Exercise;
use crate::emitters::emitter::{CanEmit, format_quantity, quoted_string};

pub struct ExerciseEmitter;

const EXERCISE_KEYWORD: &str = "@exercise";

impl CanEmit<Exercise> for ExerciseEmitter {
    fn emit(&self, exercise: &Exercise) -> String {
        let mut output = String::new();

        output.push_str(EXERCISE_KEYWORD);

        for qty in &exercise.quantities {
            output.push('(');
            output.push_str(&format_quantity(qty));
            output.push(')');
        }
        output.push(' ');

        for alias in &exercise.aliases {
            quoted_string(&mut output, alias);
            output.push(' ');
        }

        if !exercise.properties.is_empty() {
            output.push('{');
            for prop in &exercise.properties {
                output.push('\n');
                output.push_str("    ");
                output.push_str(&prop.name);
                output.push_str(": ");
                output.push_str(&format_quantity(&prop.value));
            }
            output.push('\n');
            output.push('}');
        } else {
            output.push_str("{ }");
        }

        output.push('\n');
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ast::{Property, Quantity};
    use crate::emitters::emitter::CanEmit;
    use crate::parser::parser::parse;

    #[test]
    fn emit_exercise_with_properties() {
        let exercise = Exercise {
            aliases: vec!["running".to_string()],
            quantities: vec![Quantity {
                amount: 30.0,
                unit: Some("min".to_string()),
            }],
            properties: vec![Property {
                name: "calories".to_string(),
                value: Quantity {
                    amount: 300.0,
                    unit: Some("kcal".to_string()),
                },
            }],
        };
        let output = ExerciseEmitter.emit(&exercise);
        assert!(output.starts_with("@exercise"), "should start with @exercise");
        assert!(output.contains("\"running\""), "should contain alias");
        assert!(output.contains("30min"), "should contain quantity");
        assert!(output.contains("calories"), "should contain property name");
        // Verify the emitted text can be parsed back
        let doc = parse(&output).expect("should parse back");
        assert!(!doc.items.is_empty(), "parsed doc should have items");
    }

    #[test]
    fn emit_exercise_without_properties() {
        let exercise = Exercise {
            aliases: vec!["yoga".to_string()],
            quantities: vec![Quantity {
                amount: 1.0,
                unit: Some("hour".to_string()),
            }],
            properties: vec![],
        };
        let output = ExerciseEmitter.emit(&exercise);
        assert!(output.contains("{ }"), "empty properties should produce an empty block");
        let doc = parse(&output).expect("should parse back");
        assert!(!doc.items.is_empty());
    }
}
