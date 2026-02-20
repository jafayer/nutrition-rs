//! Integration tests for the SQL module.

use nutrition_rs::ast::ast::{
    Ate, Day, DayItem, Document, Exercised, Exercise, Ingredient, IngredientLabel, Item, Property,
    Quantity, Recipe,
};
use nutrition_rs::sql::run_sql_query;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_document() -> Document {
    let flour = Ingredient {
        aliases: vec!["flour".to_string()],
        quantities: vec![Quantity { amount: 100.0, unit: Some("g".to_string()) }],
        properties: vec![
            Property {
                name: "calories".to_string(),
                value: Quantity { amount: 364.0, unit: Some("kcal".to_string()) },
            },
            Property {
                name: "protein".to_string(),
                value: Quantity { amount: 10.0, unit: Some("g".to_string()) },
            },
        ],
    };

    let water = Ingredient {
        aliases: vec!["water".to_string()],
        quantities: vec![Quantity { amount: 1.0, unit: Some("cup".to_string()) }],
        properties: vec![
            Property {
                name: "calories".to_string(),
                value: Quantity { amount: 0.0, unit: Some("kcal".to_string()) },
            },
        ],
    };

    let bread = Recipe {
        aliases: vec!["bread".to_string()],
        quantities: vec![Quantity { amount: 1.0, unit: None }],
        ingredients: vec![
            IngredientLabel {
                alias: "flour".to_string(),
                quantity: Quantity { amount: 200.0, unit: Some("g".to_string()) },
            },
        ],
    };

    let running = Exercise {
        aliases: vec!["running".to_string()],
        quantities: vec![Quantity { amount: 30.0, unit: Some("min".to_string()) }],
        properties: vec![
            Property {
                name: "calories".to_string(),
                value: Quantity { amount: 300.0, unit: Some("kcal".to_string()) },
            },
        ],
    };

    let day1 = Day {
        date: "2026-01-01".to_string(),
        items: vec![
            DayItem::Ate(Ate {
                food_alias: "flour".to_string(),
                quantity: Quantity { amount: 200.0, unit: Some("g".to_string()) },
            }),
            DayItem::Exercised(Exercised {
                exercise_alias: "running".to_string(),
                quantity: Quantity { amount: 30.0, unit: Some("min".to_string()) },
            }),
        ],
    };

    Document {
        items: vec![
            Item::Ingredient(flour),
            Item::Ingredient(water),
            Item::Recipe(bread),
            Item::Exercise(running),
            Item::Day(day1),
        ],
    }
}

// ---------------------------------------------------------------------------
// Schema tests
// ---------------------------------------------------------------------------

#[test]
fn ingredients_table_populated() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT name FROM ingredients ORDER BY name").unwrap();
    assert!(result.contains("flour"), "expected 'flour' in: {result}");
    assert!(result.contains("water"), "expected 'water' in: {result}");
}

#[test]
fn ingredient_aliases_populated() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT alias FROM ingredient_aliases ORDER BY alias").unwrap();
    assert!(result.contains("flour"), "expected 'flour' alias");
    assert!(result.contains("water"), "expected 'water' alias");
}

#[test]
fn ingredient_properties_populated() {
    let doc = make_document();
    let result = run_sql_query(
        &doc,
        "SELECT name, amount, unit FROM ingredient_properties WHERE name = 'calories' ORDER BY amount",
    )
    .unwrap();
    // flour: 364 kcal, water: 0 kcal
    assert!(result.contains("364"), "expected 364 in: {result}");
    assert!(result.contains("0"), "expected 0 in: {result}");
}

#[test]
fn recipes_table_populated() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT name FROM recipes").unwrap();
    assert!(result.contains("bread"), "expected 'bread' in: {result}");
}

#[test]
fn exercises_table_populated() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT name FROM exercises").unwrap();
    assert!(result.contains("running"), "expected 'running' in: {result}");
}

#[test]
fn days_table_populated() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT date FROM days").unwrap();
    assert!(result.contains("2026-01-01"), "expected date in: {result}");
}

#[test]
fn day_ate_table_populated() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT food_alias, amount, unit FROM day_ate").unwrap();
    assert!(result.contains("flour"), "expected 'flour' in: {result}");
    assert!(result.contains("200"), "expected amount 200 in: {result}");
}

#[test]
fn day_exercised_table_populated() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT exercise_alias FROM day_exercised").unwrap();
    assert!(result.contains("running"), "expected 'running' in: {result}");
}

// ---------------------------------------------------------------------------
// View tests
// ---------------------------------------------------------------------------

#[test]
fn day_ate_nutrition_view_resolves_ingredients() {
    let doc = make_document();
    // 200g flour = 2 × 364 kcal = 728 kcal
    let result = run_sql_query(
        &doc,
        "SELECT SUM(amount) FROM day_ate_nutrition WHERE LOWER(property) = 'calories'",
    )
    .unwrap();
    assert!(result.contains("728"), "expected 728 kcal in: {result}");
}

#[test]
fn day_exercised_calories_view() {
    let doc = make_document();
    // 30 min running at base 30 min → 300 kcal burned
    let result =
        run_sql_query(&doc, "SELECT SUM(calories_burned) FROM day_exercised_calories").unwrap();
    assert!(result.contains("300"), "expected 300 calories_burned in: {result}");
}

#[test]
fn day_summary_net_calories() {
    let doc = make_document();
    // calories_in = 728, calories_burned = 300, net = 428
    let result = run_sql_query(
        &doc,
        "SELECT calories_in, calories_burned, net_calories FROM day_summary WHERE date = '2026-01-01'",
    )
    .unwrap();
    assert!(result.contains("728"), "expected 728 calories_in in: {result}");
    assert!(result.contains("300"), "expected 300 calories_burned in: {result}");
    assert!(result.contains("428"), "expected 428 net_calories in: {result}");
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[test]
fn invalid_sql_returns_error() {
    let doc = make_document();
    let result = run_sql_query(&doc, "SELECT * FROM nonexistent_table");
    assert!(result.is_err(), "expected error for invalid SQL");
}
