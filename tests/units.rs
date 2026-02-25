//! Integration tests for the in-crate `nutrition_units` module with parsed nutrition DSL output.
//!
//! These tests validate that ingredient declarations are correctly translated
//! into `UnitRegistry` instances and that arithmetic across units works as
//! expected on real parsed data.

use chumsky::Parser;
use logos::Logos;

use nutrition_rs::ast::ast::Item;
use nutrition_rs::lexer::lexer::Token;
use nutrition_rs::nutrition_units::{NutritionQuantity, UnitRegistry, default_unit_for_property};
use nutrition_rs::parser::parser::parser;

fn lex_and_parse(input: &str) -> nutrition_rs::ast::ast::Document {
    let tokens: Vec<Token> = Token::lexer(input).filter_map(Result::ok).collect();
    parser()
        .parse(tokens.as_slice())
        .into_output()
        .expect("failed to parse input")
}

/// Build a `UnitRegistry` from the quantities declared on a parsed ingredient.
fn registry_for_ingredient(ingredient: &nutrition_rs::ast::ast::Ingredient) -> UnitRegistry {
    let pairs: Vec<(f64, String)> = ingredient
        .quantities
        .iter()
        .map(|q| (q.amount, q.unit.clone().unwrap_or_default()))
        .collect();
    UnitRegistry::from_ingredient_quantities(&pairs)
}

// ---------------------------------------------------------------------------
// Requirement 1 – sensible default units for nutritional properties
// ---------------------------------------------------------------------------

#[test]
fn req1_default_units_for_common_properties() {
    assert_eq!(default_unit_for_property("calories"), Some("kcal"));
    assert_eq!(default_unit_for_property("protein"), Some("g"));
    assert_eq!(default_unit_for_property("fat"), Some("g"));
    assert_eq!(default_unit_for_property("carbohydrates"), Some("g"));
    assert_eq!(default_unit_for_property("fiber"), Some("g"));
    assert_eq!(default_unit_for_property("cholesterol"), Some("mg"));
}

#[test]
fn req1_parsed_properties_respect_declared_units() {
    let doc = lex_and_parse(
        r#"@ingredient(100g) "sugar" {
    calories: 387kcal
    protein: 14.5g
}"#,
    );
    match &doc.items[0] {
        Item::Ingredient(ing) => {
            let calories_prop = &ing.properties[0];
            assert_eq!(calories_prop.name, "calories");
            assert_eq!(calories_prop.value.unit.as_deref(), Some("kcal"));

            let protein_prop = &ing.properties[1];
            assert_eq!(protein_prop.name, "protein");
            assert_eq!(protein_prop.value.unit.as_deref(), Some("g"));
        }
        _ => panic!("expected ingredient"),
    }
}

// ---------------------------------------------------------------------------
// Requirement 2 – new units can be created from the DSL
// ---------------------------------------------------------------------------

#[test]
fn req2_custom_unit_slice_from_dsl() {
    // "@ingredient(1 pie)(8 slices)" introduces "slice" and "pie" as new units
    let doc = lex_and_parse(r#"@ingredient(1 pie)(8 slices) "pizza" { calories: 285kcal }"#);
    match &doc.items[0] {
        Item::Ingredient(ing) => {
            let reg = registry_for_ingredient(ing);
            // "slices" is now a known unit – we can convert to "pie"
            let two_slices = NutritionQuantity::new(2.0, "slices");
            let as_pie = reg.convert(&two_slices, "pie").unwrap();
            assert!((as_pie.amount - 0.25).abs() < 1e-9);
        }
        _ => panic!("expected ingredient"),
    }
}

// ---------------------------------------------------------------------------
// Requirement 3 – equivalencies can be created from the DSL
// ---------------------------------------------------------------------------

#[test]
fn req3_equivalency_1pie_equals_8slices() {
    let doc = lex_and_parse(r#"@ingredient(1 pie)(8 slices) "pizza" {}"#);
    match &doc.items[0] {
        Item::Ingredient(ing) => {
            let reg = registry_for_ingredient(ing);

            // 8 slices → 1 pie
            let eight_slices = NutritionQuantity::new(8.0, "slices");
            let pie = reg.convert(&eight_slices, "pie").unwrap();
            assert!((pie.amount - 1.0).abs() < 1e-9);

            // 1 pie → 8 slices
            let one_pie = NutritionQuantity::new(1.0, "pie");
            let slices = reg.convert(&one_pie, "slices").unwrap();
            assert!((slices.amount - 8.0).abs() < 1e-9);
        }
        _ => panic!("expected ingredient"),
    }
}

#[test]
fn req3_equivalency_100g_equals_1cup() {
    let doc = lex_and_parse(r#"@ingredient(100g)(1 cup) "chickpeas" {}"#);
    match &doc.items[0] {
        Item::Ingredient(ing) => {
            let reg = registry_for_ingredient(ing);

            let one_cup = NutritionQuantity::new(1.0, "cup");
            let grams = reg.convert(&one_cup, "g").unwrap();
            assert!((grams.amount - 100.0).abs() < 1e-9);
        }
        _ => panic!("expected ingredient"),
    }
}

// ---------------------------------------------------------------------------
// Requirement 4 – equivalencies are scoped to ingredients
// ---------------------------------------------------------------------------

#[test]
fn req4_cup_to_gram_for_chickpeas_does_not_hold_for_water() {
    let chickpeas_src = r#"@ingredient(100g)(1 cup) "chickpeas" {}"#;
    let water_src = r#"@ingredient(1 cup) "water" {}"#;

    let chickpea_doc = lex_and_parse(chickpeas_src);
    let water_doc = lex_and_parse(water_src);

    let chickpea_ing = match &chickpea_doc.items[0] {
        Item::Ingredient(ing) => ing,
        _ => panic!("expected ingredient"),
    };
    let water_ing = match &water_doc.items[0] {
        Item::Ingredient(ing) => ing,
        _ => panic!("expected ingredient"),
    };

    let chickpea_reg = registry_for_ingredient(chickpea_ing);
    let water_reg = registry_for_ingredient(water_ing);

    // For chickpeas: 1 cup = 100 g
    let one_cup = NutritionQuantity::new(1.0, "cup");
    let chickpea_grams = chickpea_reg.convert(&one_cup, "g").unwrap();
    assert!((chickpea_grams.amount - 100.0).abs() < 1e-9);

    // For water: no custom cup→g relationship (only SI defaults apply;
    // water has no declared gram equivalency, so this must fail)
    let water_cup = NutritionQuantity::new(1.0, "cup");
    // cup → mL is a built-in SI conversion; cup → g is NOT (no custom equiv for water)
    assert!(
        water_reg.convert(&water_cup, "g").is_none(),
        "cup→g should not be defined for water without an explicit equivalency"
    );
}

// ---------------------------------------------------------------------------
// Requirement 5 – arithmetic across declared equivalencies
// ---------------------------------------------------------------------------

#[test]
fn req5_add_grams_and_kilograms_via_si() {
    let reg = UnitRegistry::with_si_defaults();
    let a = NutritionQuantity::new(500.0, "g");
    let b = NutritionQuantity::new(0.5, "kg");
    let sum = reg.add(&a, &b).unwrap();
    assert_eq!(sum.unit, "g");
    assert!((sum.amount - 1000.0).abs() < 1e-6);
}

#[test]
fn req5_add_cups_and_grams_for_chickpeas() {
    // @ingredient(100g)(1 cup) → 1 cup = 100 g
    let doc = lex_and_parse(r#"@ingredient(100g)(1 cup) "chickpeas" {}"#);
    match &doc.items[0] {
        Item::Ingredient(ing) => {
            let reg = registry_for_ingredient(ing);
            let a = NutritionQuantity::new(200.0, "g");
            let b = NutritionQuantity::new(1.0, "cup"); // = 100 g
            let sum = reg.add(&a, &b).unwrap();
            assert_eq!(sum.unit, "g");
            assert!((sum.amount - 300.0).abs() < 1e-9);
        }
        _ => panic!("expected ingredient"),
    }
}

#[test]
fn req5_add_slices_and_pies_for_pizza() {
    // @ingredient(1 pie)(8 slices)
    let doc = lex_and_parse(r#"@ingredient(1 pie)(8 slices) "pizza" {}"#);
    match &doc.items[0] {
        Item::Ingredient(ing) => {
            let reg = registry_for_ingredient(ing);
            let a = NutritionQuantity::new(2.0, "slices");
            let b = NutritionQuantity::new(0.5, "pie"); // = 4 slices
            let sum = reg.add(&a, &b).unwrap();
            assert_eq!(sum.unit, "slices");
            assert!((sum.amount - 6.0).abs() < 1e-9);
        }
        _ => panic!("expected ingredient"),
    }
}

#[test]
fn req5_add_incompatible_units_returns_error() {
    let reg = UnitRegistry::with_si_defaults();
    let protein_g = NutritionQuantity::new(10.0, "g");
    let calories_kcal = NutritionQuantity::new(100.0, "kcal");
    assert!(reg.add(&protein_g, &calories_kcal).is_err());
}

// ---------------------------------------------------------------------------
// Full pipeline – parse realistic ingredient declarations and build registries
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_chickpeas_ingredient() {
    let input = r#"@ingredient(100g)(1 cup) "chickpeas" "garbanzo beans" {
    calories: 269kcal
    protein: 14.5g
    fat: 4g
    carbohydrates: 45g
    fiber: 12.5g
}"#;

    let doc = lex_and_parse(input);
    assert_eq!(doc.items.len(), 1);

    let chickpeas = match &doc.items[0] {
        Item::Ingredient(ing) => ing,
        _ => panic!("expected ingredient"),
    };

    // Verify aliases parsed correctly
    assert!(chickpeas.aliases.contains(&"chickpeas".to_string()));

    // Build registry from declared quantities and verify cup→g conversion
    let reg = registry_for_ingredient(chickpeas);
    let one_cup = NutritionQuantity::new(1.0, "cup");
    let grams = reg.convert(&one_cup, "g").unwrap();
    assert!((grams.amount - 100.0).abs() < 1e-9);

    // Verify declared properties have expected units
    let calories_prop = chickpeas
        .properties
        .iter()
        .find(|p| p.name == "calories")
        .expect("calories property missing");
    assert_eq!(calories_prop.value.unit.as_deref(), Some("kcal"));
    assert_eq!(default_unit_for_property("calories"), Some("kcal"));
}

#[test]
fn full_pipeline_pizza_ingredient() {
    let input = r#"@ingredient(1 pie)(8 slices) "pizza" {
    calories: 285kcal
    protein: 12g
}"#;

    let doc = lex_and_parse(input);
    assert_eq!(doc.items.len(), 1);

    let pizza = match &doc.items[0] {
        Item::Ingredient(ing) => ing,
        _ => panic!("expected ingredient"),
    };

    let reg = registry_for_ingredient(pizza);

    // 4 slices = 0.5 pie
    let four_slices = NutritionQuantity::new(4.0, "slices");
    let pies = reg.convert(&four_slices, "pie").unwrap();
    assert!((pies.amount - 0.5).abs() < 1e-9);

    // Can add slices + pies
    let a = NutritionQuantity::new(2.0, "slices");
    let b = NutritionQuantity::new(0.5, "pie"); // = 4 slices
    let sum = reg.add(&a, &b).unwrap();
    assert_eq!(sum.unit, "slices");
    assert!((sum.amount - 6.0).abs() < 1e-9);
}
