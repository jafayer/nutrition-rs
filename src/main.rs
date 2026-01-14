use nutrition_rs::{ast, cli::*};
use clap::Parser;
use std::fs;
use nutrition_rs::parser::parser::parser;
use nutrition_rs::lexer::lexer::Token;
use logos::Logos;
use chumsky::Parser as _ChumskyParser;

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

    let tokens: Vec<Token> = Token::lexer(&content).filter_map(Result::ok).collect();
    println!("Lexed tokens: {:#?}", tokens);
    let parse_result = parser().parse(tokens.as_slice()).into_result();

    match parse_result {
        Ok(ast) => {
            println!("Parsed AST: {:#?}", ast);
        }
        Err(errors) => {
            eprintln!("Parsing errors:");
            for e in errors {
                eprintln!("{:?}", e);
            }
        }
    }
}