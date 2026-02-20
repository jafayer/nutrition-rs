//! Integration tests for the nutrition computation module.

use nutrition_rs::ast::ast::{Ate, Day, DayItem, Document, Exercise, Exercised, Ingredient, IngredientLabel, Item, Property, Quantity, Recipe};
use nutrition_rs::nutrition::{
    compute_daily_report, compute_ingredient_nutrition, compute_recipe_nutrition,
    compute_report, query_nutrition,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ingredient(alias: &str, base_amount: f64, base_unit: &str, props: Vec<(&str, f64, Option<&str>)>) -> Ingredient {
    Ingredient {
        aliases: vec![alias.to_string()],
        quantities: vec![Quantity {
            amount: base_amount,
            unit: Some(base_unit.to_string()),
        }],
        properties: props
            .into_iter()
            .map(|(name, amount, unit)| Property {
                name: name.to_string(),
                value: Quantity {
                    amount,
                    unit: unit.map(|u| u.to_string()),
                },
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Ingredient scaling
// ---------------------------------------------------------------------------

#[test]
fn scale_ingredient_same_unit_doubles_properties() {
    // 100 g → 269 kcal; request 200 g → 538 kcal
    let ing = make_ingredient("chickpeas", 100.0, "g", vec![("calories", 269.0, Some("kcal"))]);
    let req = Quantity { amount: 200.0, unit: Some("g".to_string()) };
    let report = compute_ingredient_nutrition(&ing, Some(&req));

    assert_eq!(report.name, "chickpeas");
    assert_eq!(report.quantity.amount, 200.0);
    assert_eq!(report.properties.len(), 1);
    let cal = &report.properties[0];
    assert_eq!(cal.name, "calories");
    assert!((cal.value.amount - 538.0).abs() < 1e-6);
    assert_eq!(cal.value.unit.as_deref(), Some("kcal"));
}

#[test]
fn scale_ingredient_no_quantity_is_identity() {
    let ing = make_ingredient("sugar", 100.0, "g", vec![("calories", 387.0, Some("kcal"))]);
    let report = compute_ingredient_nutrition(&ing, None);

    assert_eq!(report.properties[0].value.amount, 387.0);
}

#[test]
fn scale_ingredient_cross_unit_via_equivalency() {
    // @ingredient(100g)(1 cup) "chickpeas" → 1 cup = 100 g
    let mut ing = make_ingredient("chickpeas", 100.0, "g", vec![("calories", 269.0, Some("kcal"))]);
    ing.quantities.push(Quantity { amount: 1.0, unit: Some("cup".to_string()) });

    // Request 2 cups → scale = 2 (because 1 cup = 100 g ≡ base, so 2 cups = 200 g = 2×)
    let req = Quantity { amount: 2.0, unit: Some("cup".to_string()) };
    let report = compute_ingredient_nutrition(&ing, Some(&req));

    assert!((report.properties[0].value.amount - 538.0).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// Recipe computation
// ---------------------------------------------------------------------------

fn make_document(items: Vec<Item>) -> Document {
    Document { items }
}

#[test]
fn recipe_nutrition_sums_ingredient_properties() {
    // @ingredient(100g) "flour" { calories: 364kcal }
    // @ingredient(100g) "milk"  { calories: 42kcal }
    // @recipe(4) "pancakes" { "flour"(200g) "milk"(300g) }
    let flour = make_ingredient("flour", 100.0, "g", vec![("calories", 364.0, Some("kcal"))]);
    let milk = make_ingredient("milk", 100.0, "g", vec![("calories", 42.0, Some("kcal"))]);

    let recipe = Recipe {
        aliases: vec!["pancakes".to_string()],
        quantities: vec![Quantity { amount: 4.0, unit: None }],
        ingredients: vec![
            IngredientLabel {
                alias: "flour".to_string(),
                quantity: Quantity { amount: 200.0, unit: Some("g".to_string()) },
            },
            IngredientLabel {
                alias: "milk".to_string(),
                quantity: Quantity { amount: 300.0, unit: Some("g".to_string()) },
            },
        ],
    };

    let doc = make_document(vec![
        Item::Ingredient(flour),
        Item::Ingredient(milk),
        Item::Recipe(recipe.clone()),
    ]);

    let report = compute_recipe_nutrition(&doc, &recipe, None).unwrap();

    // flour: 200/100 * 364 = 728; milk: 300/100 * 42 = 126; total = 854 kcal
    let cal = report.properties.iter().find(|p| p.name == "calories").unwrap();
    assert!((cal.value.amount - 854.0).abs() < 1e-6);
    assert_eq!(cal.value.unit.as_deref(), Some("kcal"));
}

#[test]
fn recipe_nutrition_with_requested_quantity_scales_result() {
    // Same recipe as above but request 2 servings (of 4) → halve
    let flour = make_ingredient("flour", 100.0, "g", vec![("calories", 364.0, Some("kcal"))]);
    let milk = make_ingredient("milk", 100.0, "g", vec![("calories", 42.0, Some("kcal"))]);

    let recipe = Recipe {
        aliases: vec!["pancakes".to_string()],
        quantities: vec![Quantity { amount: 4.0, unit: None }],
        ingredients: vec![
            IngredientLabel {
                alias: "flour".to_string(),
                quantity: Quantity { amount: 200.0, unit: Some("g".to_string()) },
            },
            IngredientLabel {
                alias: "milk".to_string(),
                quantity: Quantity { amount: 300.0, unit: Some("g".to_string()) },
            },
        ],
    };

    let doc = make_document(vec![
        Item::Ingredient(flour),
        Item::Ingredient(milk),
        Item::Recipe(recipe.clone()),
    ]);

    let req = Quantity { amount: 2.0, unit: None };
    let report = compute_recipe_nutrition(&doc, &recipe, Some(&req)).unwrap();

    // 854 * (2/4) = 427 kcal
    let cal = report.properties.iter().find(|p| p.name == "calories").unwrap();
    assert!((cal.value.amount - 427.0).abs() < 1e-6);
}

#[test]
fn recipe_unknown_ingredient_returns_error() {
    let recipe = Recipe {
        aliases: vec!["mystery".to_string()],
        quantities: vec![],
        ingredients: vec![IngredientLabel {
            alias: "unicorn dust".to_string(),
            quantity: Quantity { amount: 1.0, unit: None },
        }],
    };
    let doc = make_document(vec![Item::Recipe(recipe.clone())]);

    assert!(compute_recipe_nutrition(&doc, &recipe, None).is_err());
}

// ---------------------------------------------------------------------------
// JSON output
// ---------------------------------------------------------------------------

#[test]
fn nutrition_report_to_json_is_valid_json() {
    let ing = make_ingredient("sugar", 100.0, "g", vec![("calories", 387.0, Some("kcal"))]);
    let report = compute_ingredient_nutrition(&ing, None);
    let json_str = report.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("should be valid JSON");
    assert_eq!(parsed["name"], "sugar");
    assert!(parsed["properties"].is_array());
}

// ---------------------------------------------------------------------------
// query_nutrition
// ---------------------------------------------------------------------------

#[test]
fn query_nutrition_finds_ingredient() {
    let ing = make_ingredient("chickpeas", 100.0, "g", vec![("calories", 269.0, Some("kcal"))]);
    let doc = make_document(vec![Item::Ingredient(ing)]);
    let report = query_nutrition(&doc, "chickpeas", None).unwrap();
    assert_eq!(report.name, "chickpeas");
}

#[test]
fn query_nutrition_finds_recipe() {
    let flour = make_ingredient("flour", 100.0, "g", vec![("calories", 364.0, Some("kcal"))]);
    let recipe = Recipe {
        aliases: vec!["bread".to_string()],
        quantities: vec![Quantity { amount: 1.0, unit: None }],
        ingredients: vec![IngredientLabel {
            alias: "flour".to_string(),
            quantity: Quantity { amount: 100.0, unit: Some("g".to_string()) },
        }],
    };
    let doc = make_document(vec![Item::Ingredient(flour), Item::Recipe(recipe)]);
    let report = query_nutrition(&doc, "bread", None).unwrap();
    assert_eq!(report.name, "bread");
}

#[test]
fn query_nutrition_unknown_alias_returns_error() {
    let doc = make_document(vec![]);
    assert!(query_nutrition(&doc, "nope", None).is_err());
}

// ---------------------------------------------------------------------------
// Daily report computation
// ---------------------------------------------------------------------------

fn make_exercise(alias: &str, base_amount: f64, base_unit: &str, props: Vec<(&str, f64, Option<&str>)>) -> Exercise {
    Exercise {
        aliases: vec![alias.to_string()],
        quantities: vec![Quantity {
            amount: base_amount,
            unit: Some(base_unit.to_string()),
        }],
        properties: props
            .into_iter()
            .map(|(name, amount, unit)| Property {
                name: name.to_string(),
                value: Quantity {
                    amount,
                    unit: unit.map(|u| u.to_string()),
                },
            })
            .collect(),
    }
}

#[test]
fn daily_report_sums_ate_intake() {
    // 100g chickpeas = 269 kcal; eat 200g → 538 kcal
    let ing = make_ingredient("chickpeas", 100.0, "g", vec![("calories", 269.0, Some("kcal"))]);
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![DayItem::Ate(Ate {
            food_alias: "chickpeas".to_string(),
            quantity: Quantity { amount: 200.0, unit: Some("g".to_string()) },
        })],
    };
    let doc = make_document(vec![Item::Ingredient(ing), Item::Day(day.clone())]);
    let report = compute_daily_report(&doc, &day);

    assert_eq!(report.date, "2026-01-01");
    let cal = report.intake.iter().find(|p| p.name == "calories").unwrap();
    assert!((cal.value.amount - 538.0).abs() < 1e-6);
    // No exercises → exercise list is empty, net equals intake.
    assert!(report.exercise.is_empty());
    let net_cal = report.net.iter().find(|p| p.name == "calories").unwrap();
    assert!((net_cal.value.amount - 538.0).abs() < 1e-6);
}

#[test]
fn daily_report_subtracts_exercise_calories() {
    // eat 500 kcal, burn 200 kcal running → net 300 kcal
    let ing = make_ingredient("rice", 100.0, "g", vec![("calories", 130.0, Some("kcal"))]);
    // 100g rice = 130 kcal; eating ~385g → 500 kcal (approx)
    let ex = make_exercise("running", 30.0, "min", vec![("calories", 200.0, Some("kcal"))]);
    let day = Day {
        date: "2026-02-01".to_string(),
        items: vec![
            DayItem::Ate(Ate {
                food_alias: "rice".to_string(),
                quantity: Quantity { amount: 100.0, unit: Some("g".to_string()) },
            }),
            DayItem::Exercised(Exercised {
                exercise_alias: "running".to_string(),
                quantity: Quantity { amount: 30.0, unit: Some("min".to_string()) },
            }),
        ],
    };
    let doc = make_document(vec![
        Item::Ingredient(ing),
        Item::Exercise(ex),
        Item::Day(day.clone()),
    ]);
    let report = compute_daily_report(&doc, &day);

    let intake_cal = report.intake.iter().find(|p| p.name == "calories").unwrap();
    assert!((intake_cal.value.amount - 130.0).abs() < 1e-6);

    let ex_cal = report.exercise.iter().find(|p| p.name == "calories").unwrap();
    assert!((ex_cal.value.amount - 200.0).abs() < 1e-6);

    let net_cal = report.net.iter().find(|p| p.name == "calories").unwrap();
    // 130 - 200 = -70 kcal
    assert!((net_cal.value.amount - (-70.0)).abs() < 1e-6);
}

#[test]
fn compute_report_filters_by_date_range() {
    let ing = make_ingredient("apple", 1.0, "unit", vec![("calories", 80.0, Some("kcal"))]);
    let day1 = Day {
        date: "2026-01-01".to_string(),
        items: vec![DayItem::Ate(Ate {
            food_alias: "apple".to_string(),
            quantity: Quantity { amount: 1.0, unit: Some("unit".to_string()) },
        })],
    };
    let day2 = Day {
        date: "2026-01-15".to_string(),
        items: vec![DayItem::Ate(Ate {
            food_alias: "apple".to_string(),
            quantity: Quantity { amount: 2.0, unit: Some("unit".to_string()) },
        })],
    };
    let day3 = Day {
        date: "2026-02-01".to_string(),
        items: vec![DayItem::Ate(Ate {
            food_alias: "apple".to_string(),
            quantity: Quantity { amount: 3.0, unit: Some("unit".to_string()) },
        })],
    };
    let doc = make_document(vec![
        Item::Ingredient(ing),
        Item::Day(day1),
        Item::Day(day2),
        Item::Day(day3),
    ]);

    // Filter to January only.
    let reports = compute_report(&doc, Some("2026-01-01"), Some("2026-01-31"));
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].date, "2026-01-01");
    assert_eq!(reports[1].date, "2026-01-15");
}

#[test]
fn compute_report_no_filter_returns_all_days() {
    let ing = make_ingredient("apple", 1.0, "unit", vec![("calories", 80.0, Some("kcal"))]);
    let day1 = Day { date: "2026-01-01".to_string(), items: vec![] };
    let day2 = Day { date: "2026-03-01".to_string(), items: vec![] };
    let doc = make_document(vec![
        Item::Ingredient(ing),
        Item::Day(day1),
        Item::Day(day2),
    ]);
    let reports = compute_report(&doc, None, None);
    assert_eq!(reports.len(), 2);
}

#[test]
fn daily_report_unknown_food_skipped_gracefully() {
    // If the food alias doesn't exist in the document, we get an empty report.
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![DayItem::Ate(Ate {
            food_alias: "unicorn_dust".to_string(),
            quantity: Quantity { amount: 10.0, unit: Some("g".to_string()) },
        })],
    };
    let doc = make_document(vec![Item::Day(day.clone())]);
    let report = compute_daily_report(&doc, &day);
    assert!(report.intake.is_empty());
    assert!(report.exercise.is_empty());
}
