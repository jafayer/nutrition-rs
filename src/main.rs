use nutrition_rs::{ast, cli::*};
use nutrition_rs::tree_sitter_ast::ast::parse;
use nutrition_rs::tree_sitter_ast::semantic::SemanticAnalyzer;
use clap::Parser;
use std::fs;


#[derive(Parser, Debug)]
#[command(name = "nutrition")]
#[command(about = "A nutrition tracking tool for the Nutrition spec", long_about = None)]
pub struct Cli {
    #[arg(
        short,
        long,
        help = "Path to input file to parse (or set via env: NUTRITION_DEFAULT_FILE)",
        env = env::DEFAULT_FILE_ENV_VAR,
        required = true,
    )]
    pub file: String,
}

fn main() {
    let cli = Cli::parse();

    let content = fs::read_to_string(&cli.file)
        .expect("Failed to read input file");
    println!("Parsing file: {}", &cli.file);

    let mut analyzer = SemanticAnalyzer::new();

    let tree_sat = parse(&content).expect("Failed to parse input file with Tree-sitter");
    let root_node = tree_sat.root_node();

    let document = analyzer.analyze(root_node, &content).expect("Failed to analyze semantic AST");
    println!("Semantic AST: {:#?}", document);

    println!("Recipes:");
    document.items.iter().for_each(|item| {
        if let crate::ast::ast::Item::Recipe(recipe) = item {
            let quantity = recipe.quantities.first().unwrap().amount;
            println!("- {} ({} servings)", recipe.aliases.join(", "), quantity);
        }
    });
}