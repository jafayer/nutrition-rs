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
	// Split by top-level lines starting with '@' while preserving leading comment blocks
	let mut chunks: Vec<String> = Vec::new();
	let mut current = String::new();
	let mut brace_depth: i32 = 0;
	for line in input.lines() {
		let trimmed = line.trim_start();
		if brace_depth == 0 && trimmed.starts_with('@') {
			if !current.trim().is_empty() {
				chunks.push(current.clone());
				current.clear();
			}
		}
		current.push_str(line);
		current.push('\n');

		// Update brace depth after appending the line so lines with '{' start the block
		// and lines with '}' close it. This is a simple heuristic suitable for this test file.
		for ch in line.chars() {
			if ch == '{' {
				brace_depth += 1;
			} else if ch == '}' {
				brace_depth -= 1;
			}
		}
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

#[test]
fn parse_ingredient_block_with_escaped_quotes() {
    let doc = parse_document(r#"@ingredient(100g)(1cup)(164g) "Chickpeas, \"Garbanzo\" Beans" {
  calories: 164kcal
  protein: 8.9g
  fat: 2.6g
  carbohydrates: 27.4g
  fiber: 7.6g
  sugar: 4.8g
}"#);
    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Ingredient(ing) => {
            assert_eq!(ing.aliases, vec!["Chickpeas, \"Garbanzo\" Beans"]);
            assert_eq!(ing.quantities.len(), 3);
            assert_quantity(&ing.quantities[0], 100.0, Some("g"));
            assert_quantity(&ing.quantities[1], 1.0, Some("cup"));
            assert_quantity(&ing.quantities[2], 164.0, Some("g"));
            assert_eq!(ing.properties.len(), 6);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}
// ---------------------------------------------------------------------------
// Error-reporting and recovery tests
// ---------------------------------------------------------------------------
use nutrition_rs::parser::parser::parse_with_errors;

#[test]
fn parse_with_errors_returns_empty_errors_for_valid_input() {
    let source = r#"@ingredient(100g) "chickpeas" { calories: 269 }"#;
    let (doc, errors) = parse_with_errors(source);
    assert!(doc.is_some(), "valid input should produce a document");
    assert!(errors.is_empty(), "valid input should have no errors");
}

#[test]
fn parse_with_errors_returns_error_for_invalid_input() {
    // Completely invalid top-level content.
    let source = "this is definitely not valid nutrition syntax @@@";
    let (_, errors) = parse_with_errors(source);
    assert!(!errors.is_empty(), "invalid input should report at least one error");
}

#[test]
fn parse_with_errors_recovers_valid_items_after_bad_one() {
    // The first ingredient block is missing its closing `}`, making it
    // malformed.  The second ingredient is valid and should still be parsed.
    let source = r#"@ingredient(100g) "bad" {
    calories: 50
@ingredient(100g) "good" {
    calories: 60
}"#;
    let (doc, errors) = parse_with_errors(source);
    assert!(doc.is_some(), "should produce a partial document after recovery");
    assert!(!errors.is_empty(), "should record an error for the malformed item");
    let doc = doc.unwrap();
    let valid_ingredients: Vec<_> = doc
        .items
        .iter()
        .filter_map(|i| {
            if let Item::Ingredient(ing) = i {
                Some(ing)
            } else {
                None
            }
        })
        .collect();
    assert!(
        !valid_ingredients.is_empty(),
        "at least the good ingredient should be recovered"
    );
}

#[test]
fn parse_with_errors_error_messages_include_line_number() {
    let source = "// line 1 comment\n@ingredient MISSING_PARENS_AND_BLOCK\n// another comment";
    let (_, errors) = parse_with_errors(source);
    if !errors.is_empty() {
        // At least one error should mention a line number.
        let has_line_ref = errors.iter().any(|e| e.starts_with("line "));
        assert!(has_line_ref, "error messages should reference a line number, got: {:?}", errors);
    }
}

#[test]
fn parse_with_errors_empty_input_returns_empty_document() {
    let (doc, errors) = parse_with_errors("");
    assert!(doc.is_some(), "empty input should return Some(Document)");
    assert!(errors.is_empty(), "empty input should have no errors");
    assert!(doc.unwrap().items.is_empty());
}

#[test]
fn parse_with_errors_multiple_bad_items_all_reported() {
    // Two malformed declarations and one valid one.
    let source = r#"@ingredient BROKEN_1
@ingredient(100g) "good" { calories: 50 }
@recipe BROKEN_2"#;
    let (doc, errors) = parse_with_errors(source);
    assert!(doc.is_some());
    // Both bad items should generate errors.
    assert!(errors.len() >= 2, "expected at least 2 errors, got: {:?}", errors);
}

// ---------------------------------------------------------------------------
// parse_with_diagnostics tests
// ---------------------------------------------------------------------------
use nutrition_rs::parser::parser::parse_with_diagnostics;

#[test]
fn diagnostics_empty_for_valid_source() {
    let (doc, diags) = parse_with_diagnostics(
        r#"@ingredient(100g) "chickpeas" { calories: 269 }"#,
    );
    assert!(doc.is_some());
    assert!(diags.is_empty(), "valid source should produce no diagnostics");
}

#[test]
fn diagnostics_carry_byte_span_into_source() {
    // The @ingredient keyword starts at byte 0.
    let source = "@ingredient MISSING_BLOCK";
    let (_, diags) = parse_with_diagnostics(source);
    assert!(!diags.is_empty(), "malformed declaration should produce a diagnostic");
    let d = &diags[0];
    // The span must be within the source string.
    assert!(d.byte_span.start < source.len());
    assert!(d.byte_span.end <= source.len());
}

#[test]
fn diagnostics_declaration_kind_matches_keyword() {
    let source = "@recipe MISSING_BLOCK";
    let (_, diags) = parse_with_diagnostics(source);
    assert!(!diags.is_empty());
    assert_eq!(diags[0].declaration_kind, "@recipe");
}

#[test]
fn diagnostics_recover_valid_item_after_bad_one() {
    let source = r#"@ingredient(100g) "bad" {
    calories: 50
@ingredient(100g) "good" {
    calories: 60
}"#;
    let (doc, diags) = parse_with_diagnostics(source);
    assert!(doc.is_some(), "should produce partial document");
    assert!(!diags.is_empty(), "should have at least one diagnostic");
    let doc = doc.unwrap();
    let ingredients: Vec<_> = doc.items.iter()
        .filter_map(|i| if let Item::Ingredient(ing) = i { Some(ing) } else { None })
        .collect();
    assert!(!ingredients.is_empty(), "valid ingredient should be recovered");
}

#[test]
fn diagnostics_empty_source_no_diagnostics() {
    let (doc, diags) = parse_with_diagnostics("");
    assert!(doc.is_some());
    assert!(diags.is_empty());
    assert!(doc.unwrap().items.is_empty());
}

// ---------------------------------------------------------------------------
// note_span / fine-grained diagnostic tests
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_day_block_flags_unexpected_string() {
    // "chickpeas"(1 cup) is not valid in a @day block.
    let source = r#"@day "2026-01-01" {
  @ate "chickpea stew"(2)
  "chickpeas"(1 cup)
}"#;
    let (doc, diags) = parse_with_diagnostics(source);
    assert!(doc.is_some() || !diags.is_empty());
    assert!(!diags.is_empty(), "should report an error");

    let d = &diags[0];
    // The note_span should point inside the block (after the opening brace),
    // not at the declaration header (byte 0).
    let note = d.note_span.as_ref().expect("note_span should be set");
    assert!(note.start > 0, "note should not point to the header");

    // The note message should mention "chickpeas".
    let msg = d.note_message.as_deref().expect("note_message should be set");
    assert!(
        msg.contains("chickpeas"),
        "note message should mention 'chickpeas', got: {msg}"
    );
    assert!(
        msg.contains("@day"),
        "note message should mention '@day', got: {msg}"
    );
}

#[test]
fn diagnostic_day_block_note_span_within_source() {
    let source = r#"@day "2026-01-01" {
  @ate "chickpea stew"(2)
  "chickpeas"(1 cup)
}"#;
    let (_, diags) = parse_with_diagnostics(source);
    assert!(!diags.is_empty());
    let d = &diags[0];
    if let Some(ref note_span) = d.note_span {
        assert!(
            note_span.start <= source.len(),
            "note_span.start out of bounds"
        );
        assert!(
            note_span.end <= source.len(),
            "note_span.end out of bounds"
        );
        // The note span should cover "chickpeas"(1 cup) — verify start byte
        // points into the word "chickpeas" (after the opening `"`).
        let snippet = &source[note_span.clone()];
        assert!(
            snippet.contains("chickpeas"),
            "note span should cover 'chickpeas', got: {snippet:?}"
        );
    }
}

#[test]
fn diagnostic_ingredient_block_flags_unexpected_string() {
    // A string literal ("chickpeas") is not a valid property in an ingredient block.
    let source = r#"@ingredient(100g) "oil" {
  calories: 900
  "not a property"
}"#;
    let (_, diags) = parse_with_diagnostics(source);
    assert!(!diags.is_empty(), "should report an error");
    let d = &diags[0];
    let note = d.note_span.as_ref().expect("note_span should be set");
    assert!(note.start > 0, "note should not point to the header");
    let msg = d.note_message.as_deref().expect("note_message should be set");
    assert!(
        msg.contains("@ingredient"),
        "note should mention @ingredient, got: {msg}"
    );
}

#[test]
fn diagnostic_header_only_when_no_brace() {
    // No `{` means no body to scan — note_span should be None.
    let source = "@ingredient MISSING_EVERYTHING";
    let (_, diags) = parse_with_diagnostics(source);
    assert!(!diags.is_empty());
    // note_span should be None since there's no body.
    assert!(
        diags[0].note_span.is_none(),
        "no brace → no note_span expected"
    );
}
