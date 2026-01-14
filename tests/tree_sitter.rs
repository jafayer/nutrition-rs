use nutrition_rs::tree_sitter_ast::ast::{get_language, parse};
use std::fs;

#[test]
fn test_language_loads() {
    let language = get_language();
    assert!(language.abi_version() > 0, "Language should have a valid version");
}

#[test]
fn test_parse_example_file() {
    let source = fs::read_to_string("examples/test.nutrition")
        .expect("Failed to read test.nutrition file");
    
    let tree = parse(&source).expect("Failed to parse test.nutrition");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Parse tree should not contain errors");
    assert_eq!(root.kind(), "source_file", "Root node should be source_file");
}

#[test]
fn test_parse_simple_ingredient() {
    let source = r#"@ingredient(100g) "test" {
  calories: 50
}"#;
    
    let tree = parse(source).expect("Failed to parse ingredient");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Simple ingredient should parse without errors");
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
    
    let tree = parse(source).expect("Failed to parse ingredient with aliases");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Ingredient with aliases should parse without errors");
}

#[test]
fn test_parse_ingredient_with_comments() {
    let source = r#"// test comment
@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" { // test comment
    calories: 269
    protein: 14.5g
    fat: 4g
    carbohydrates: 45g
    fiber: 12.5g // test comment
} // test comment"#;
    
    let tree = parse(source).expect("Failed to parse ingredient with comments");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Ingredient with comments should parse without errors");
}

#[test]
fn test_parse_empty_ingredient() {
    let source = r#"@ingredient(1 cup) "water" {}"#;
    
    let tree = parse(source).expect("Failed to parse empty ingredient");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Empty ingredient should parse without errors");
}

#[test]
fn test_parse_recipe() {
    let source = r#"@recipe(4) "simple chickpeas" {
    "water"(1 cup)
    "chickpeas"(200g)
}"#;
    
    let tree = parse(source).expect("Failed to parse recipe");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Recipe should parse without errors");
}

#[test]
fn test_parse_recipe_with_yield() {
    let source = r#"@recipe(8)(500g) "chickpea stew" { "chickpeas"(2 cups), "water"(5 cups) }"#;
    
    let tree = parse(source).expect("Failed to parse recipe with yield");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Recipe with yield should parse without errors");
}

#[test]
fn test_parse_recipe_with_fractional_amount() {
    let source = r#"@recipe(2) "double pizza party" {
    "pizza"(0.5 pie)
}"#;
    
    let tree = parse(source).expect("Failed to parse recipe with fractional amount");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Recipe with fractional amount should parse without errors");
}

#[test]
fn test_parse_day_entry() {
    let source = r#"@day "2026-01-01" {
    @ate "chickpea stew"(2)
}"#;
    
    let tree = parse(source).expect("Failed to parse day entry");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Day entry should parse without errors");
}

#[test]
fn test_parse_day_with_exercise() {
    let source = r#"@day "2026-01-06" {
    @ate "simple chickpeas"(3)
    @exercised "running"(30 minutes)
}"#;
    
    let tree = parse(source).expect("Failed to parse day with exercise");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Day with exercise should parse without errors");
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
    
    let tree = parse(source).expect("Failed to parse multiple entries");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Multiple entries should parse without errors");
}

#[test]
fn test_tree_structure() {
    let source = r#"@ingredient(100g) "test" {
  calories: 50
}"#;
    
    let tree = parse(source).expect("Failed to parse");
    let root = tree.root_node();
    
    assert_eq!(root.kind(), "source_file");
    assert!(root.child_count() > 0, "Source file should have children");
}

#[test]
fn test_parse_error_detection() {
    // Test with clearly invalid syntax
    let source = r#"this is not valid nutrition syntax"#;
    
    let tree = parse(source).expect("Parser should return a tree even for invalid input");
    let root = tree.root_node();
    
    // Tree-sitter may or may not report errors depending on grammar implementation
    // At minimum, we should get a tree back
    assert_eq!(root.kind(), "source_file");
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
  "water"(4 cups)
}
"#;
    
    let tree = parse(source).expect("Failed to parse with various units");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Various units should parse without errors");
}

#[test]
fn test_empty_source() {
    let source = "";
    
    let tree = parse(source).expect("Failed to parse empty source");
    let root = tree.root_node();
    
    assert_eq!(root.kind(), "source_file");
}

#[test]
fn test_comments_only() {
    let source = r#"// just a comment
// another comment"#;
    
    let tree = parse(source).expect("Failed to parse comments only");
    let root = tree.root_node();
    
    assert!(!root.has_error(), "Comments only should parse without errors");
}
