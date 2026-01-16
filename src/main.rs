use nutrition_rs::cli::{env, file_loader, generate};
use clap::Parser;


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

    #[command(subcommand)]
    pub command: Commands,


}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    Validate {
        #[arg(short, long, help = "Show the parse tree")]
        show_tree: bool,
    },

    Generate {
        #[command(subcommand)]
        generate_command: generate::GenerateCommands,
    }


}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { show_tree } => {
            file_loader::load_tree(Some(&cli.file))
                .map(|document| {
                    println!("File '{}' is valid.", cli.file);
                    if show_tree {
                        print_document(document);
                    }
                })
                .unwrap_or_else(|err| {
                    eprintln!("Validation failed for file '{}': {}", cli.file, err);
                });
        }

        Commands::Generate { generate_command } => {
            match generate_command {
                generate::GenerateCommands::Recipe(args) => {
                    let output = args.emit();
                    println!("{}", output);
                }

                generate::GenerateCommands::Ingredient(args) => {
                    let output = args.emit();
                    println!("{}", output);
                }
            }
        }
    }
}

fn print_document(node: nutrition_rs::ast::ast::Document) {
    println!("{:#?}", node);
}