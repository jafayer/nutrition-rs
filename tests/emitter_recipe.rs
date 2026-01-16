use nutrition_rs::emitters::recipe::RecipeEmitter;
use nutrition_rs::emitters::emitter::CanEmit;
use nutrition_rs::ast::ast::{Recipe, Quantity, IngredientLabel};
use nutrition_rs::tree_sitter_ast::ast::parse;

#[test]
fn test_emit_recipe_with_ingredients() {
    let recipe = Recipe {
        quantities: vec![
            Quantity { amount: 200.0, unit: Some("g".to_string()) },
            Quantity { amount: 1.0, unit: Some("cup".to_string()) },
        ],
        aliases: vec![
            "chickpeas".to_string(),
            "chickpea".to_string(),
            "garbanzo beans".to_string(),
        ],
        ingredients: vec![
            IngredientLabel {
                alias: "chickpeas".to_string(),
                quantity: Quantity { amount: 200.0, unit: Some("g".to_string()) },
            }
        ],
    };

    let emitter = RecipeEmitter;
    let output = emitter.emit(&recipe);

    // print output for debugging
    println!("Emitted Recipe:\n{}", output);

    // parse the emitted output to ensure it's valid
    let tree = parse(&output, None).expect("Failed to parse emitted recipe");
    assert!(tree.root_node().has_error() == false, "Parsed tree has errors");
}

#[test]
fn test_emit_recipe_without_ingredients() {
    let recipe = Recipe {
        quantities: vec![
            Quantity { amount: 100.0, unit: Some("ml".to_string()) },
        ],
        aliases: vec![
            "olive oil".to_string(),
        ],
        ingredients: vec![],
    };

    let emitter = RecipeEmitter;
    let output = emitter.emit(&recipe);

    // print output for debugging
    println!("Emitted Recipe:\n{}", output);

    // parse the emitted output to ensure it's valid
    let tree = parse(&output, None).expect("Failed to parse emitted recipe");
    assert!(tree.root_node().has_error() == false, "Parsed tree has errors");
}