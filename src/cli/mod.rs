pub mod env;
pub mod file_loader;
pub mod generate;
pub mod validate;

use clap::Parser;
use chrono::Local;

pub use env::DEFAULT_FILE_ENV_VAR;

// ---------------------------------------------------------------------------
// CLI type definitions
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "nutrition")]
#[command(about = "A nutrition tracking tool for the Nutrition spec", long_about = None)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub struct Cli {
    #[arg(
        short,
        long,
        help = "Path to input file to parse (or set via env: NUTRITION_DEFAULT_FILE)",
        env = env::DEFAULT_FILE_ENV_VAR,
        global = true
    )]
    pub file: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Validate a nutrition file and print any parse errors.
    Validate {
        #[arg(short, long, help = "Show the parse tree")]
        show_tree: bool,
    },

    /// Generate new markup.
    #[command(visible_aliases = ["gen", "g"])]
    Generate {
        #[command(subcommand)]
        generate_command: generate::GenerateCommands,
    },

    /// Run a web server that converts JSON to nutrition markup.
    #[cfg(feature = "runtime")]
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

        #[arg(long, help = "Output raw JSON instead of the nutrition-label display")]
        json: bool,
    },

    /// Compute daily nutrition reports from @day blocks.
    Report {
        #[arg(
            long,
            help = "Start date filter, inclusive (e.g. '2026-01-01'). Defaults to today."
        )]
        start: Option<String>,

        #[arg(
            long,
            help = "End date filter, inclusive (e.g. '2026-01-31' or 'today'). Defaults to today."
        )]
        end: Option<String>,

        #[arg(
            long,
            help = "Only show intake (exclude exercise / net computation)",
            default_value_t = false
        )]
        ate_only: bool,

        #[arg(
            long,
            help = "Show each day individually instead of aggregating over the date range"
        )]
        no_aggregate: bool,

        #[arg(long, help = "Output raw JSON instead of the nutrition-label display")]
        json: bool,

        #[arg(
            long,
            help = "Show per-entry nutrition trace tree (source contributions) instead of standard report"
        )]
        trace: bool,
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Return the file path from the option or exit with an error message.
pub fn require_file(file: &Option<String>) -> String {
    file.clone().unwrap_or_else(|| {
        eprintln!(
            "Missing required argument: --file <FILE> (or set {})",
            env::DEFAULT_FILE_ENV_VAR
        );
        std::process::exit(1);
    })
}

/// Print the AST document to stdout for debugging.
pub fn print_document(node: crate::ast::ast::Document) {
    println!("{:#?}", node);
}

/// Return the current local date as a `YYYY-MM-DD` string.
pub fn current_date_iso8601() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

/// Execute the parsed CLI command.  This is gated behind the `runtime`
/// feature because some sub-commands (`serve`, AI ingredient generation)
/// require an async runtime.
#[cfg(feature = "runtime")]
pub async fn run_cli(cli: Cli) {
    use crate::ast::ast::Quantity;
    use crate::display::{
        format_aggregated_report,
        format_daily_report,
        format_daily_trace_report,
        format_nutrition_report,
    };
    use crate::nutrition::{
        aggregate_reports,
        compute_report,
        compute_trace_report,
        query_nutrition,
        NutritionReport,
    };

    match cli.command {
        Commands::Validate { show_tree } => {
            let file = require_file(&cli.file);
            if let Err(code) = validate::run_validate(&file, show_tree) {
                std::process::exit(code);
            }
        }

        Commands::Generate { generate_command } => match generate_command {
            generate::GenerateCommands::Recipe(args) => {
                let output = args.emit();
                println!("\n{}", output);
            }

            generate::GenerateCommands::Ingredient(args) => {
                let output = args.emit_with_ai().await;
                println!("\n{}", output);
            }

            generate::GenerateCommands::Day(args) => {
                let output = args.emit();
                println!("\n{}", output);
            }
        },

        Commands::Serve { port } => {
            crate::web_server::handler::run_server(port).await.unwrap();
        }

        Commands::Query { name, quantity, json } => {
            let file = require_file(&cli.file);
            let document = match file_loader::load_tree(Some(&file)) {
                Ok(doc) => doc,
                Err(err) => {
                    eprintln!("Failed to load file '{}': {}", file, err);
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
                Ok(report) => {
                    if json {
                        println!("{}", report.to_json());
                    } else {
                        println!("{}", format_nutrition_report(&report));
                    }
                }
                Err(err) => {
                    eprintln!("Query failed: {}", err);
                    std::process::exit(1);
                }
            }
        }

        Commands::Report { start, end, ate_only, no_aggregate, json, trace } => {
            let file = require_file(&cli.file);
            let document = match file_loader::load_tree(Some(&file)) {
                Ok(doc) => doc,
                Err(err) => {
                    eprintln!("Failed to load file '{}': {}", file, err);
                    std::process::exit(1);
                }
            };

            let today = current_date_iso8601();
            let start_resolved = start
                .as_deref()
                .map(|s| if s == "today" { today.as_str() } else { s })
                .unwrap_or(today.as_str());
            let end_resolved = end
                .as_deref()
                .map(|e| if e == "today" { today.as_str() } else { e })
                .unwrap_or(today.as_str());

            if trace {
                let traces = compute_trace_report(&document, Some(start_resolved), Some(end_resolved));
                if traces.is_empty() {
                    println!("No @day entries found in the specified range.");
                    return;
                }

                if json {
                    println!("{}", serde_json::to_string_pretty(&traces).unwrap_or_default());
                } else {
                    for (idx, trace_report) in traces.iter().enumerate() {
                        if idx > 0 {
                            println!();
                        }
                        println!("{}", format_daily_trace_report(trace_report));
                    }
                }
                return;
            }

            let reports = compute_report(&document, Some(start_resolved), Some(end_resolved));

            if reports.is_empty() {
                println!("No @day entries found in the specified range.");
                return;
            }

            let is_range = start_resolved != end_resolved;
            let use_aggregate = is_range && !no_aggregate;

            if use_aggregate {
                let agg = aggregate_reports(&reports, start_resolved, end_resolved);
                if json {
                    if ate_only {
                        let output = serde_json::json!({
                            "start": agg.start,
                            "end": agg.end,
                            "days": agg.days,
                            "intake": agg.intake,
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
                    } else {
                        println!("{}", agg.to_json());
                    }
                } else if ate_only {
                    let label = format!("{} \u{2013} {}", agg.start, agg.end);
                    let intake_report = NutritionReport {
                        name: label,
                        quantity: Quantity { amount: agg.days as f64, unit: Some("days".to_string()) },
                        properties: agg.intake,
                    };
                    println!("{}", format_nutrition_report(&intake_report));
                } else {
                    println!("{}", format_aggregated_report(&agg));
                }
            } else {
                for report in &reports {
                    if json {
                        if ate_only {
                            let output = serde_json::json!({
                                "date": report.date,
                                "intake": report.intake,
                            });
                            println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
                        } else {
                            println!("{}", report.to_json());
                        }
                    } else if ate_only {
                        let intake_report = NutritionReport {
                            name: report.date.clone(),
                            quantity: Quantity { amount: 1.0, unit: Some("day".to_string()) },
                            properties: report.intake.clone(),
                        };
                        println!("{}", format_nutrition_report(&intake_report));
                    } else {
                        println!("{}", format_daily_report(report));
                    }
                }
            }
        }
    }
}