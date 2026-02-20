use chumsky::Parser;
use logos::Logos;

use nutrition_rs::ast::ast::{DayItem, Document, Item, Quantity};
use nutrition_rs::lexer::lexer::Token;
use nutrition_rs::parser::parser::parser;

fn lex(input: &str) -> Vec<Token> {
	Token::lexer(input).filter_map(Result::ok).collect()
}

fn parse_document(input: &str) -> Document {
	let tokens = lex(input);
	parser()
		.parse(tokens.as_slice())
		.into_output()
		.expect("parser returned an error")
}

fn assert_quantity(quantity: &Quantity, expected_amount: f64, expected_unit: Option<&str>) {
	assert_eq!(quantity.amount, expected_amount);
	assert_eq!(quantity.unit.as_deref(), expected_unit);
}

#[test]
fn parses_basic_ingredient() {
	let doc = parse_document(
		r#"@ingredient(100g)(1 cup) "chickpeas" "garbanzo" {
	calories: 269
	protein: 14.5g
}"#,
	);

	assert_eq!(doc.items.len(), 1);

	match &doc.items[0] {
		Item::Ingredient(ingredient) => {
			assert_eq!(ingredient.aliases, vec!["chickpeas", "garbanzo"]);
			assert_eq!(ingredient.quantities.len(), 2);
			assert_quantity(&ingredient.quantities[0], 100.0, Some("g"));
			assert_quantity(&ingredient.quantities[1], 1.0, Some("cup"));

			assert_eq!(ingredient.properties.len(), 2);
			assert_eq!(ingredient.properties[0].name, "calories");
			assert_quantity(&ingredient.properties[0].value, 269.0, None);
			assert_eq!(ingredient.properties[1].name, "protein");
			assert_quantity(&ingredient.properties[1].value, 14.5, Some("g"));
		}
		other => panic!("unexpected item parsed: {other:?}"),
	}
}

#[test]
fn parses_recipe_with_ingredient_labels() {
	let doc = parse_document(
		r#"@recipe(4)(500g) "chickpea stew" {
	"chickpeas"(2 cups), "water"(3 cups)
}"#,
	);

	assert_eq!(doc.items.len(), 1);

	match &doc.items[0] {
		Item::Recipe(recipe) => {
			assert_eq!(recipe.aliases, vec!["chickpea stew"]);
			assert_eq!(recipe.quantities.len(), 2);
			assert_quantity(&recipe.quantities[0], 4.0, None);
			assert_quantity(&recipe.quantities[1], 500.0, Some("g"));

			assert_eq!(recipe.ingredients.len(), 2);
			assert_eq!(recipe.ingredients[0].alias, "chickpeas");
			assert_quantity(&recipe.ingredients[0].quantity, 2.0, Some("cups"));
			assert_eq!(recipe.ingredients[1].alias, "water");
			assert_quantity(&recipe.ingredients[1].quantity, 3.0, Some("cups"));
		}
		other => panic!("unexpected item parsed: {other:?}"),
	}
}

#[test]
fn parses_day_with_ate_and_exercised() {
	let doc = parse_document(
		r#"@day "2026-01-06" {
	@ate "simple chickpeas"(3)
	@exercised "running"(30 minutes)
}"#,
	);

	assert_eq!(doc.items.len(), 1);

	match &doc.items[0] {
		Item::Day(day) => {
			assert_eq!(day.date, "2026-01-06");
			assert_eq!(day.items.len(), 2);

			match &day.items[0] {
				DayItem::Ate(ate) => {
					assert_eq!(ate.food_alias, "simple chickpeas");
					assert_quantity(&ate.quantity, 3.0, None);
				}
				other => panic!("unexpected first day item: {other:?}"),
			}

			match &day.items[1] {
				DayItem::Exercised(exercised) => {
					assert_eq!(exercised.exercise_alias, "running");
					assert_quantity(&exercised.quantity, 30.0, Some("minutes"));
				}
				other => panic!("unexpected second day item: {other:?}"),
			}
		}
		other => panic!("unexpected item parsed: {other:?}"),
	}
}

#[test]
fn parses_comment_as_item() {
	let doc = parse_document("// top level comment");

	assert_eq!(doc.items.len(), 1);

	match &doc.items[0] {
		Item::Comment(text) => assert_eq!(text, "// top level comment"),
		other => panic!("unexpected item parsed: {other:?}"),
	}
}

#[test]
fn parses_exercise_block() {
	let doc = parse_document(
		r#"@exercise(30 min) "running" {
	calories: 300kcal
}"#,
	);

	assert_eq!(doc.items.len(), 1);

	match &doc.items[0] {
		Item::Exercise(ex) => {
			assert_eq!(ex.aliases, vec!["running"]);
			assert_eq!(ex.quantities.len(), 1);
			assert_eq!(ex.quantities[0].amount, 30.0);
			assert_eq!(ex.quantities[0].unit.as_deref(), Some("min"));
			assert_eq!(ex.properties.len(), 1);
			assert_eq!(ex.properties[0].name, "calories");
			assert_eq!(ex.properties[0].value.amount, 300.0);
			assert_eq!(ex.properties[0].value.unit.as_deref(), Some("kcal"));
		}
		other => panic!("unexpected item parsed: {other:?}"),
	}
}

#[test]
fn parse_example_document() {
    // load examples/test.nutrition and parse
    let input = std::fs::read_to_string("examples/test.nutrition").expect("failed to read example file");
    let doc = parse_document(&input);
    assert!(!doc.items.is_empty(), "parsed document should not be empty");
}

#[test]
fn parse_example_items_individually() {
	// Helps isolate which section of the example fails
	let input = std::fs::read_to_string("examples/test.nutrition").expect("failed to read example file");
	// Split by lines starting with '@' while preserving leading comment blocks
	let mut chunks: Vec<String> = Vec::new();
	let mut current = String::new();
	for line in input.lines() {
		let trimmed = line.trim_start();
		if trimmed.starts_with('@') {
			if !current.trim().is_empty() {
				chunks.push(current.clone());
				current.clear();
			}
		}
		current.push_str(line);
		current.push('\n');
	}
	if !current.trim().is_empty() {
		chunks.push(current);
	}

	assert!(!chunks.is_empty(), "no chunks found in example");

	for (i, chunk) in chunks.iter().enumerate() {
		let res = std::panic::catch_unwind(|| parse_document(chunk));
		match res {
			Ok(doc) => assert!(doc.items.len() >= 1, "chunk {} parsed to no items", i + 1),
			Err(_) => panic!("failed to parse chunk {}:\n{}", i + 1, chunk),
		}
	}
}