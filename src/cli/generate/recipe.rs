
use crate::emitters::emitter::CanEmit;
use crate::emitters::recipe::RecipeEmitter;
use crate::ast::ast::{Recipe, Quantity, IngredientLabel};
use clap::Parser;
use std::io::{self, Read};


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
    #[clap(long = "quantity", short='q', required = true)]
    pub quantities: Vec<String>,

    #[clap(long = "alias", short='a', required = true)]
    pub aliases: Vec<String>,

    #[clap(long = "ingredient", conflicts_with = "from_cook")]
    pub ingredients: Vec<String>,

    #[clap(
        long = "from-cook",
        help = "Read ingredients from stdin (Cooklang shopping list format)",
        conflicts_with = "ingredients"
    )]
    pub from_cook: bool,
}

impl RecipeGenerateArgs {
    pub fn to_recipe(&self) -> Recipe {
        let quantities = self.quantities.iter().map(|q_str| {
            parse_quantity(q_str)
        }).collect();

        let ingredients = if self.from_cook {
            self.parse_cook_ingredients()
        } else {
            self.ingredients.iter().map(|ing_str| {
                let parts: Vec<&str> = ing_str.split('(').collect();
                let alias = parts[0].to_string();
                let quantity = parse_quantity(parts[1].trim_end_matches(')'));
                IngredientLabel { alias, quantity }
            }).collect()
        };

        Recipe {
            quantities,
            aliases: self.aliases.clone(),
            ingredients,
        }
    }

    fn parse_cook_ingredients(&self) -> Vec<IngredientLabel> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).expect("Failed to read from stdin");

        parse_cook_ingredients_from_str(&input)
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
    let unit = if unit_str.is_empty() { None } else { Some(unit_str) };

    Quantity { amount, unit }
}

fn parse_cook_ingredient_line(line: &str) -> Option<IngredientLabel> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Skip section headers like: [other]
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return None;
    }

    let mut quantity_start_idx: Option<usize> = None;
    let mut prev: Option<char> = None;
    for (idx, ch) in trimmed.char_indices() {
        if (ch.is_ascii_digit() || ch == '.') && prev.is_some_and(|c| c.is_whitespace()) {
            quantity_start_idx = Some(idx);
            break;
        }
        prev = Some(ch);
    }

    let quantity_start_idx = quantity_start_idx?;
    let alias = trimmed[..quantity_start_idx].trim().to_string();
    if alias.is_empty() {
        return None;
    }

    let quantity_raw = trimmed[quantity_start_idx..].trim();
    let quantity = parse_cook_quantity(quantity_raw)?;

    Some(IngredientLabel { alias, quantity })
}

fn parse_cook_ingredients_from_str(input: &str) -> Vec<IngredientLabel> {
    input
        .lines()
        .filter_map(parse_cook_ingredient_line)
        .collect()
}

fn parse_cook_quantity(quantity_raw: &str) -> Option<Quantity> {
    let tokens: Vec<&str> = quantity_raw.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let amount = parse_numeric_token(tokens[0])?;

    // Keep only parser-safe unit tokens (identifiers). Stop at the first additional numeric token.
    let mut unit_parts: Vec<String> = Vec::new();
    for token in tokens.iter().skip(1) {
        let cleaned = token.trim_matches(',');
        if cleaned.is_empty() {
            continue;
        }

        if parse_numeric_token(cleaned).is_some() {
            break;
        }

        if !is_identifier_like(cleaned) {
            break;
        }

        unit_parts.push(cleaned.to_string());
    }

    let unit = if unit_parts.is_empty() {
        None
    } else {
        Some(unit_parts.join(" "))
    };

    Some(Quantity { amount, unit })
}

fn parse_numeric_token(token: &str) -> Option<f64> {
    token.trim_matches(',').parse::<f64>().ok()
}

fn is_identifier_like(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::{parse_cook_ingredient_line, parse_cook_ingredients_from_str};

    #[test]
    fn cook_section_header_is_ignored() {
        assert!(parse_cook_ingredient_line("[other]").is_none());
    }

    #[test]
    fn cook_line_with_simple_quantity_parses() {
        let parsed = parse_cook_ingredient_line("frozen corn           2 cup").expect("expected ingredient");
        assert_eq!(parsed.alias, "frozen corn");
        assert_eq!(parsed.quantity.amount, 2.0);
        assert_eq!(parsed.quantity.unit.as_deref(), Some("cup"));
    }

    #[test]
    fn cook_line_with_extra_numeric_segments_is_still_parseable() {
        let parsed = parse_cook_ingredient_line("black beans           2 15 oz can").expect("expected ingredient");
        assert_eq!(parsed.alias, "black beans");
        assert_eq!(parsed.quantity.amount, 2.0);
        assert_eq!(parsed.quantity.unit.as_deref(), None);
    }

    #[test]
    fn cook_line_with_multiple_measurements_keeps_first_unit() {
        let parsed = parse_cook_ingredient_line("cilantro              2 tbsp, 1").expect("expected ingredient");
        assert_eq!(parsed.alias, "cilantro");
        assert_eq!(parsed.quantity.amount, 2.0);
        assert_eq!(parsed.quantity.unit.as_deref(), Some("tbsp"));
    }

    #[test]
    fn cook_line_without_quantity_is_ignored() {
        assert!(parse_cook_ingredient_line("ground black pepper").is_none());
    }

    #[test]
    fn cook_aisle_headers_are_ignored_in_full_input() {
        let input = r#"[dairy]
Milk       1 cup

[pantry]
Crackers   3 biscuits
"#;

        let parsed = parse_cook_ingredients_from_str(input);
        assert_eq!(parsed.len(), 2);

        assert_eq!(parsed[0].alias, "Milk");
        assert_eq!(parsed[0].quantity.amount, 1.0);
        assert_eq!(parsed[0].quantity.unit.as_deref(), Some("cup"));

        assert_eq!(parsed[1].alias, "Crackers");
        assert_eq!(parsed[1].quantity.amount, 3.0);
        assert_eq!(parsed[1].quantity.unit.as_deref(), Some("biscuits"));
    }
}