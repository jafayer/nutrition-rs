use nutrition_rs::tree_sitter_ast::ast::parse;
use nutrition_rs::ast::ast::{Item, Document};

fn print_document(doc: &Document, indent: usize) {
    let pad = " ".repeat(indent);
    for item in &doc.items {
        match item {
            Item::Ingredient(ing) => {
                println!("{}Ingredient: {:?}", pad, ing.aliases);
                println!("{}  quantities: {:?}", pad, ing.quantities);
                println!("{}  properties: {:?}", pad, ing.properties);
            }
            Item::Recipe(rec) => {
                println!("{}Recipe: {:?}", pad, rec.aliases);
                println!("{}  quantities: {:?}", pad, rec.quantities);
                println!("{}  ingredients: {:?}", pad, rec.ingredients);
            }
            Item::Exercise(ex) => {
                println!("{}Exercise: {:?}", pad, ex.aliases);
            }
            Item::Day(day) => {
                println!("{}Day: {}", pad, day.date);
                println!("{}  items: {:?}", pad, day.items);
            }
            Item::Ate(ate) => {
                println!("{}Ate: {} {:?}", pad, ate.food_alias, ate.quantity);
            }
            Item::Exercised(ex) => {
                println!("{}Exercised: {} {:?}", pad, ex.exercise_alias, ex.quantity);
            }
            Item::Comment(c) => {
                println!("{}Comment: {}", pad, c);
            }
            Item::Property(prop) => {
                println!("{}Property: {} {:?}", pad, prop.name, prop.value);
            }
        }
    }
}

fn main() {
    let source = r#"@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" {
    calories: 269
    protein: 14.5g
}"#;

    if let Some(doc) = parse(source) {
        println!("Document structure:");
        print_document(&doc, 0);
    } else {
        println!("Parse failed");
    }
}
