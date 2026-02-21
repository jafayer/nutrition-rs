use nutrition_rs::parser::parser::parse;
use std::fs;

#[test]
fn test_parse_example_file() {
    let source = fs::read_to_string("examples/test.nutrition")
        .expect("Failed to read test.nutrition file");

    let doc = parse(&source).expect("Failed to parse test.nutrition");
    assert!(!doc.items.is_empty(), "Parsed document should not be empty");
}

#[test]
fn test_parse_simple_ingredient() {
    let source = r#"@ingredient(100g) "test" {
  calories: 50
}"#;

    let doc = parse(source).expect("Failed to parse ingredient");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_ingredient_with_aliases() {
    let source = r#"@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" {
    calories: 269
    protein: 14.5g
    fat: 4g
    carbohydrates: 45g
    fiber: 12.5g
}"#;

    let doc = parse(source).expect("Failed to parse ingredient with aliases");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_ingredient_with_comments() {
    let source = r#"// test comment
@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" {
    calories: 269
    protein: 14.5g
    fat: 4g
    carbohydrates: 45g
    fiber: 12.5g
}"#;

    let doc = parse(source).expect("Failed to parse ingredient with comments");
    assert!(!doc.items.is_empty(), "Ingredient with comments should parse");
}

#[test]
fn test_parse_empty_ingredient() {
    let source = r#"@ingredient(1 cup) "water" {}"#;

    let doc = parse(source).expect("Failed to parse empty ingredient");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_recipe() {
    let source = r#"@recipe(4) "simple chickpeas" {
    "water"(1 cup)
    "chickpeas"(200g)
}"#;

    let doc = parse(source).expect("Failed to parse recipe");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_recipe_with_yield() {
    let source = r#"@recipe(8)(500g) "chickpea stew" { "chickpeas"(2 cups), "water"(5 cups) }"#;

    let doc = parse(source).expect("Failed to parse recipe with yield");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_recipe_with_fractional_amount() {
    let source = r#"@recipe(2) "double pizza party" {
    "pizza"(0.5 pie)
}"#;

    let doc = parse(source).expect("Failed to parse recipe with fractional amount");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_day_entry() {
    let source = r#"@day "2026-01-01" {
    @ate "chickpea stew"(2)
}"#;

    let doc = parse(source).expect("Failed to parse day entry");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_day_with_exercise() {
    let source = r#"@day "2026-01-06" {
    @ate "simple chickpeas"(3)
    @exercised "running"(30 minutes)
}"#;

    let doc = parse(source).expect("Failed to parse day with exercise");
    assert_eq!(doc.items.len(), 1);
}

#[test]
fn test_parse_multiple_entries() {
    let source = r#"
@ingredient(100g) "test1" {
  calories: 50
}

@ingredient(100g) "test2" {
  calories: 60
}

@recipe(1) "test recipe" {
    "test1"(50g)
    "test2"(50g)
}
"#;

    let doc = parse(source).expect("Failed to parse multiple entries");
    assert_eq!(doc.items.len(), 3);
}

#[test]
fn test_parse_with_various_units() {
    let source = r#"
@ingredient(1 pie)(8 slices) "pizza" {
  calories: 285
  protein: 12g
  fat: 10g
  carbohydrates: 36g
}

@recipe(2) "pizza party" {
  "pizza"(2 slices)
}
"#;

    let doc = parse(source).expect("Failed to parse with various units");
    assert_eq!(doc.items.len(), 2);
}

#[test]
fn test_empty_source() {
    let source = "";

    let doc = parse(source).expect("Failed to parse empty source");
    assert!(doc.items.is_empty());
}

#[test]
fn test_comments_only() {
    let source = r#"// just a comment
// another comment"#;

    let doc = parse(source).expect("Failed to parse comments only");
    assert_eq!(doc.items.len(), 2);
}

#[test]
fn test_invalid_syntax_returns_none() {
    // Clearly invalid syntax – parser should return None
    let source = r#"this is not valid nutrition syntax @@@"#;

    let result = parse(source);
    assert!(result.is_none(), "Invalid syntax should return None");
}
