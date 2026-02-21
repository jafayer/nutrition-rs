use nutrition_rs::tree_sitter_ast::ast::parse;
use nutrition_rs::ast::ast::{Item, Document};

fn print_document(doc: &Document) {
    for item in &doc.items {
        match item {
            Item::Ingredient(ing) => {
                println!("Ingredient: {:?}", ing.aliases);
                println!("  quantities: {:?}", ing.quantities);
                println!("  properties: {:?}", ing.properties);
            }
            Item::Recipe(rec) => {
                println!("Recipe: {:?}", rec.aliases);
            }
            Item::Exercise(ex) => {
                println!("Exercise: {:?}", ex.aliases);
            }
            Item::Day(day) => {
                println!("Day: {}", day.date);
            }
            Item::Ate(ate) => {
                println!("Ate: {}", ate.food_alias);
            }
            Item::Exercised(ex) => {
                println!("Exercised: {}", ex.exercise_alias);
            }
            Item::Comment(c) => {
                println!("Comment: {}", c);
            }
        }
    }
}

fn main() {
    let source = r#"@ingredient(100g) "test" {
  calories: 50
}"#;

    if let Some(doc) = parse(source) {
        println!("Document structure:");
        print_document(&doc);
    } else {
        println!("Parse failed");
    }
}
