use clap::Parser;
use nutrition_rs::cli::{env, file_loader, generate};
use nutrition_rs::web_server::handler::run_server;
use nutrition_rs::nutrition::query_nutrition;
use nutrition_rs::ast::ast::Quantity;
use tokio;

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
    },

    Serve {
        #[arg(
            short,
            long,
            help = "Port to run the server on",
            default_value_t = 8080
        )]
        port: u16,
    },

    /// Display nutritional data for a named ingredient or recipe.
    Query {
        #[arg(short, long, help = "Name or alias of the ingredient or recipe to query")]
        name: String,

        #[arg(
            short,
            long,
            help = "Quantity to scale the result to (e.g. '200g', '2 servings')"
        )]
        quantity: Option<String>,
    },
}

#[tokio::main]
async fn main() {
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

        Commands::Generate { generate_command } => match generate_command {
            generate::GenerateCommands::Recipe(args) => {
                let output = args.emit();
                println!("\n{}", output);
            }

            generate::GenerateCommands::Ingredient(args) => {
                let output = args.emit();
                println!("\n{}", output.await);
            }

            generate::GenerateCommands::Day(args) => {
                let output = args.emit();
                println!("\n{}", output);
            }
        },
        Commands::Serve { port } => {
            run_server(port).await.unwrap();
        }

        Commands::Query { name, quantity } => {
            let document = match file_loader::load_tree(Some(&cli.file)) {
                Ok(doc) => doc,
                Err(err) => {
                    eprintln!("Failed to load file '{}': {}", cli.file, err);
                    std::process::exit(1);
                }
            };

            let requested_quantity = quantity
                .as_deref()
                .map(|q| Quantity::from_string(q))
                .transpose()
                .unwrap_or_else(|err| {
                    eprintln!("Invalid quantity '{}': {}", quantity.as_deref().unwrap_or(""), err);
                    std::process::exit(1);
                });

            match query_nutrition(&document, &name, requested_quantity.as_ref()) {
                Ok(report) => println!("{}", report.to_json()),
                Err(err) => {
                    eprintln!("Query failed: {}", err);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn print_document(node: nutrition_rs::ast::ast::Document) {
    println!("{:#?}", node);
}
