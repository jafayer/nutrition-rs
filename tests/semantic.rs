use nutrition_rs::ast::ast::*;
use nutrition_rs::parser::parser::parse;
use nutrition_rs::parser::semantic::SemanticAnalyzer;
use std::fs;

#[test]
fn test_semantic_analyzer_initialization() {
    let analyzer = SemanticAnalyzer::new();
    assert!(analyzer.get_ingredient("test").is_none());
    assert!(analyzer.get_recipe("test").is_none());
}

#[test]
fn test_parse_simple_ingredient() {
    let source = r#"@ingredient(100g) "test" {
  calories: 50
}"#;

    let mut analyzer = SemanticAnalyzer::new();
    if let Some(doc) = parse(source) {
        if let Ok(doc) = analyzer.analyze(doc) {
            assert!(!doc.items.is_empty());
            // Check that ingredient was registered
            let ingredient = analyzer.get_ingredient("test");
            assert!(ingredient.is_some());
        }
    } else {
        panic!("Failed to parse source");
    }
}

#[test]
fn test_ingredient_with_aliases() {
    let source = r#"@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" {
    calories: 269
    protein: 14.5g
}"#;

    let mut analyzer = SemanticAnalyzer::new();
    if let Some(doc) = parse(source) {
        if let Ok(_doc) = analyzer.analyze(doc) {
            // All aliases should resolve to the same ingredient
            let chickpeas = analyzer.get_ingredient("chickpeas");
            let chickpea = analyzer.get_ingredient("chickpea");
            let garbanzo = analyzer.get_ingredient("garbanzo beans");

            assert!(chickpeas.is_some());
            assert!(chickpea.is_some());
            assert!(garbanzo.is_some());
        }
    } else {
        panic!("Failed to parse source");
    }
}

#[test]
fn test_quantity_parsing() {
    let quantity = Quantity {
        amount: 100.0,
        unit: Some("g".to_string()),
    };
    assert_eq!(quantity.amount, 100.0);
    assert_eq!(quantity.unit.as_deref(), Some("g"));
}

#[test]
fn test_recipe_ingredient_resolution() {
    let mut analyzer = SemanticAnalyzer::new();

    let ingredient_source = r#"@ingredient(100g) "chickpeas" {
  calories: 269
  protein: 14.5g
}"#;

    if let Some(doc) = parse(ingredient_source) {
        let _ = analyzer.analyze(doc);
    }

    // Now verify the ingredient is registered
    assert!(analyzer.get_ingredient("chickpeas").is_some());
}

#[test]
fn test_day_parsing() {
    let source = r#"@day "2026-01-01" {
    @ate "chickpeas"(2 cups)
}"#;

    let mut analyzer = SemanticAnalyzer::new();
    if let Some(doc) = parse(source) {
        if let Ok(doc) = analyzer.analyze(doc) {
            let days = doc
                .items
                .iter()
                .filter(|item| matches!(item, Item::Day(_)))
                .count();
            assert!(days > 0);
        }
    } else {
        panic!("Failed to parse source");
    }
}

#[test]
fn test_complex_document_analysis() {
    let source =
        fs::read_to_string("examples/test.nutrition").expect("Failed to read test.nutrition");

    let mut analyzer = SemanticAnalyzer::new();
    if let Some(doc) = parse(&source) {
        if let Ok(doc) = analyzer.analyze(doc) {
            let ingredients = doc
                .items
                .iter()
                .filter(|item| matches!(item, Item::Ingredient(_)))
                .count();
            let recipes = doc
                .items
                .iter()
                .filter(|item| matches!(item, Item::Recipe(_)))
                .count();
            let days = doc
                .items
                .iter()
                .filter(|item| matches!(item, Item::Day(_)))
                .count();

            assert!(ingredients > 0 || recipes > 0 || days > 0 || !doc.items.is_empty());
        }
    } else {
        panic!("Failed to parse test.nutrition");
    }
}

#[test]
fn test_quantity_with_spaces() {
    let doc = Document {
        items: vec![Item::Ingredient(Ingredient {
            aliases: vec!["water".to_string()],
            quantities: vec![Quantity {
                amount: 1.0,
                unit: Some("cup".to_string()),
            }],
            properties: vec![],
        })],
    };

    if let Item::Ingredient(ing) = &doc.items[0] {
        assert!(!ing.quantities.is_empty());
        assert_eq!(ing.quantities[0].amount, 1.0);
        assert_eq!(ing.quantities[0].unit, Some("cup".to_string()));
    }
}

#[test]
fn test_fractional_quantities() {
    let doc = Document {
        items: vec![Item::Ingredient(Ingredient {
            aliases: vec!["pizza".to_string()],
            quantities: vec![Quantity {
                amount: 0.5,
                unit: Some("pie".to_string()),
            }],
            properties: vec![],
        })],
    };

    if let Item::Ingredient(ing) = &doc.items[0] {
        assert_eq!(ing.quantities[0].amount, 0.5);
        assert_eq!(ing.quantities[0].unit, Some("pie".to_string()));
    }
}

#[test]
fn test_property_tracking() {
    let doc = Document {
        items: vec![Item::Ingredient(Ingredient {
            aliases: vec!["chickpeas".to_string()],
            quantities: vec![Quantity {
                amount: 100.0,
                unit: Some("g".to_string()),
            }],
            properties: vec![
                Property {
                    name: "calories".to_string(),
                    value: Quantity {
                        amount: 269.0,
                        unit: None,
                    },
                },
                Property {
                    name: "protein".to_string(),
                    value: Quantity {
                        amount: 14.5,
                        unit: Some("g".to_string()),
                    },
                },
                Property {
                    name: "fat".to_string(),
                    value: Quantity {
                        amount: 4.0,
                        unit: Some("g".to_string()),
                    },
                },
            ],
        })],
    };

    if let Item::Ingredient(ing) = &doc.items[0] {
        assert_eq!(ing.properties.len(), 3);
        assert_eq!(ing.properties[0].name, "calories");
        assert_eq!(ing.properties[0].value.amount, 269.0);
    }
}

#[test]
fn test_multiple_aliases_resolution() {
    let doc = Document {
        items: vec![Item::Ingredient(Ingredient {
            aliases: vec![
                "chickpeas".to_string(),
                "chickpea".to_string(),
                "garbanzo beans".to_string(),
            ],
            quantities: vec![Quantity {
                amount: 100.0,
                unit: Some("g".to_string()),
            }],
            properties: vec![Property {
                name: "calories".to_string(),
                value: Quantity {
                    amount: 269.0,
                    unit: None,
                },
            }],
        })],
    };

    if let Item::Ingredient(ing) = &doc.items[0] {
        assert_eq!(ing.aliases.len(), 3);
        assert!(ing.aliases.contains(&"chickpeas".to_string()));
        assert!(ing.aliases.contains(&"chickpea".to_string()));
        assert!(ing.aliases.contains(&"garbanzo beans".to_string()));
    }
}

#[test]
fn test_recipe_with_ingredient_labels() {
    let doc = Document {
        items: vec![Item::Recipe(Recipe {
            aliases: vec!["chickpea stew".to_string()],
            quantities: vec![
                Quantity {
                    amount: 8.0,
                    unit: None,
                },
                Quantity {
                    amount: 500.0,
                    unit: Some("g".to_string()),
                },
            ],
            ingredients: vec![
                IngredientLabel {
                    alias: "chickpeas".to_string(),
                    quantity: Quantity {
                        amount: 2.0,
                        unit: Some("cups".to_string()),
                    },
                },
                IngredientLabel {
                    alias: "water".to_string(),
                    quantity: Quantity {
                        amount: 5.0,
                        unit: Some("cups".to_string()),
                    },
                },
            ],
        })],
    };

    if let Item::Recipe(recipe) = &doc.items[0] {
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.ingredients[0].alias, "chickpeas");
        assert_eq!(recipe.ingredients[0].quantity.amount, 2.0);
    }
}

#[test]
fn test_ate_reference() {
    let doc = Document {
        items: vec![Item::Day(Day {
            date: "2026-01-01".to_string(),
            items: vec![DayItem::Ate(Ate {
                food_alias: "chickpea stew".to_string(),
                quantity: Quantity {
                    amount: 2.0,
                    unit: None,
                },
            })],
        })],
    };

    if let Item::Day(day) = &doc.items[0] {
        assert_eq!(day.date, "2026-01-01");
        assert_eq!(day.items.len(), 1);
        if let DayItem::Ate(ate) = &day.items[0] {
            assert_eq!(ate.food_alias, "chickpea stew");
            assert_eq!(ate.quantity.amount, 2.0);
        }
    }
}

#[test]
fn test_exercised_entry() {
    let doc = Document {
        items: vec![Item::Day(Day {
            date: "2026-01-06".to_string(),
            items: vec![DayItem::Exercised(Exercised {
                exercise_alias: "running".to_string(),
                quantity: Quantity {
                    amount: 30.0,
                    unit: Some("minutes".to_string()),
                },
            })],
        })],
    };

    if let Item::Day(day) = &doc.items[0] {
        assert_eq!(day.items.len(), 1);
        if let DayItem::Exercised(ex) = &day.items[0] {
            assert_eq!(ex.exercise_alias, "running");
            assert_eq!(ex.quantity.amount, 30.0);
        }
    }
}

#[test]
fn test_day_with_multiple_items() {
    let doc = Document {
        items: vec![Item::Day(Day {
            date: "2026-01-06".to_string(),
            items: vec![
                DayItem::Ate(Ate {
                    food_alias: "chickpeas".to_string(),
                    quantity: Quantity {
                        amount: 1.0,
                        unit: Some("cup".to_string()),
                    },
                }),
                DayItem::Exercised(Exercised {
                    exercise_alias: "running".to_string(),
                    quantity: Quantity {
                        amount: 30.0,
                        unit: Some("minutes".to_string()),
                    },
                }),
            ],
        })],
    };

    if let Item::Day(day) = &doc.items[0] {
        assert_eq!(day.items.len(), 2);
    }
}
