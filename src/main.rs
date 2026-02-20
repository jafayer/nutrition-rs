use clap::Parser;
use nutrition_rs::cli::{env, file_loader, generate};
use nutrition_rs::web_server::handler::run_server;
use nutrition_rs::nutrition::{query_nutrition, compute_report};
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

    /// Compute daily nutrition reports from @day blocks.
    Report {
        #[arg(
            long,
            help = "Start date filter, inclusive (e.g. '2026-01-01')"
        )]
        start: Option<String>,

        #[arg(
            long,
            help = "End date filter, inclusive (e.g. '2026-01-31' or 'today')"
        )]
        end: Option<String>,

        #[arg(
            long,
            help = "Only show intake (exclude exercise / net computation)",
            default_value_t = false
        )]
        ate_only: bool,
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

        Commands::Report { start, end, ate_only } => {
            let document = match file_loader::load_tree(Some(&cli.file)) {
                Ok(doc) => doc,
                Err(err) => {
                    eprintln!("Failed to load file '{}': {}", cli.file, err);
                    std::process::exit(1);
                }
            };

            // Resolve "today" alias to the current date (YYYY-MM-DD).
            let today = current_date_iso8601();
            let start_str = start.as_deref();
            let end_resolved = end.as_deref().map(|e| if e == "today" { today.as_str() } else { e });

            let reports = compute_report(&document, start_str, end_resolved);

            if reports.is_empty() {
                println!("No @day entries found in the specified range.");
            } else {
                for report in &reports {
                    if ate_only {
                        let output = serde_json::json!({
                            "date": report.date,
                            "intake": report.intake,
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
                    } else {
                        println!("{}", report.to_json());
                    }
                }
            }
        }
    }
}

/// Return the current UTC date as a `YYYY-MM-DD` string using only the
/// standard library (no external date crates required).
///
/// Note: this uses UTC time, so the date may differ from the local wall-clock
/// date near midnight depending on the system's timezone offset.
fn current_date_iso8601() -> String {
    // std::time gives us seconds since the Unix epoch; we convert manually.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Days since epoch.
    let days = secs / 86400;

    // Algorithm from http://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn print_document(node: nutrition_rs::ast::ast::Document) {
    println!("{:#?}", node);
}
