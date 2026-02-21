use nutrition_rs::emitters::ingredient::IngredientEmitter;
use nutrition_rs::emitters::emitter::CanEmit;
use nutrition_rs::ast::ast::{Ingredient, Quantity, Property};
use nutrition_rs::tree_sitter_ast::ast::parse;

#[test]
fn test_emit_ingredient_with_properties() {
    let ingredient = Ingredient {
        quantities: vec![
            Quantity { amount: 100.0, unit: Some("g".to_string()) },
        ],
        aliases: vec![
            "sugar".to_string(),
        ],
        properties: vec![
            Property {
                name: "calories".to_string(),
                value: Quantity { amount: 387.0, unit: Some("kcal".to_string()) },
            }
        ],
    };

    let emitter = IngredientEmitter;
    let output = emitter.emit(&ingredient);

    // print output for debugging
    println!("Emitted Ingredient:\n{}", output);

    // parse the emitted output to ensure it's valid
    let doc = parse(&output).expect("Failed to parse emitted ingredient");
    assert!(!doc.items.is_empty(), "Parsed document should not be empty");
}

#[test]
fn test_emit_ingredient_without_properties() {
    let ingredient = Ingredient {
        quantities: vec![
            Quantity { amount: 50.0, unit: Some("ml".to_string()) },
        ],
        aliases: vec![
            "olive oil".to_string(),
        ],
        properties: vec![],
    };

    let emitter = IngredientEmitter;
    let output = emitter.emit(&ingredient);

    // print output for debugging
    println!("Emitted Ingredient:\n{}", output);

    // parse the emitted output to ensure it's valid
    let doc = parse(&output).expect("Failed to parse emitted ingredient");
    assert!(!doc.items.is_empty(), "Parsed document should not be empty");
}
