use clap::Parser;
use nutrition_rs::cli::{env, file_loader, generate};
use nutrition_rs::web_server::handler::run_server;
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
                println!("{}", output);
            }

            generate::GenerateCommands::Ingredient(args) => {
                let output = args.emit();
                println!("{}", output);
            }

            generate::GenerateCommands::Day(args) => {
                let output = args.emit();
                println!("{}", output);
            }
        },
        Commands::Serve { port } => {
            run_server(port).await.unwrap();
        }
    }
}

fn print_document(node: nutrition_rs::ast::ast::Document) {
    println!("{:#?}", node);
}
