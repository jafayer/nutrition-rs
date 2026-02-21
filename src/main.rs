use clap::Parser;
use nutrition_rs::cli::{env, file_loader, generate};
use nutrition_rs::web_server::handler::run_server;
use nutrition_rs::nutrition::{query_nutrition, compute_report, aggregate_reports};
use nutrition_rs::display::{format_nutrition_report, format_daily_report, format_aggregated_report};
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
        global = true
    )]
    pub file: Option<String>,

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
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { show_tree } => {
            use ariadne::{Color, Label, Report, ReportKind, Source};

            let file = require_file(&cli.file);
            let (source, document, diagnostics) =
                match file_loader::load_source_with_diagnostics(&file) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                };

            // Render each diagnostic as a rich ariadne report with source
            // context, arrows, and colour highlighting.
            for diag in &diagnostics {
                let mut report =
                    Report::build(ReportKind::Error, file.as_str(), diag.byte_span.start)
                        .with_message(&diag.message)
                        .with_label(
                            Label::new((file.as_str(), diag.byte_span.clone()))
                                .with_message(format!(
                                    "this {} could not be parsed",
                                    diag.declaration_kind
                                ))
                                .with_color(Color::Red),
                        );

                // If we know the specific token that caused the failure,
                // add a second label pointing directly at it.
                if let (Some(note_span), Some(note_msg)) =
                    (&diag.note_span, &diag.note_message)
                {
                    report = report.with_label(
                        Label::new((file.as_str(), note_span.clone()))
                            .with_message(note_msg)
                            .with_color(Color::Yellow),
                    );
                }

                report
                    .with_help(help_for_kind(diag.declaration_kind))
                    .finish()
                    .eprint((file.as_str(), Source::from(&source)))
                    .unwrap();
            }

            match document {
                Some(doc) if diagnostics.is_empty() => {
                    let item_count = doc
                        .items
                        .iter()
                        .filter(|i| {
                            !matches!(i, nutrition_rs::ast::ast::Item::Comment(_))
                        })
                        .count();
                    println!("✓ '{}' is valid ({} item(s)).", file, item_count);
                    if show_tree {
                        print_document(doc);
                    }
                }
                Some(doc) => {
                    let recovered = doc
                        .items
                        .iter()
                        .filter(|i| {
                            !matches!(i, nutrition_rs::ast::ast::Item::Comment(_))
                        })
                        .count();
                    eprintln!(
                        "✗ '{}' has {} parse error(s); {} item(s) recovered.",
                        file,
                        diagnostics.len(),
                        recovered,
                    );
                    if show_tree {
                        print_document(doc);
                    }
                    std::process::exit(1);
                }
                None => {
                    eprintln!("✗ '{}' could not be parsed.", file);
                    std::process::exit(1);
                }
            }
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

        Commands::Report { start, end, ate_only, no_aggregate, json } => {
            let file = require_file(&cli.file);
            let document = match file_loader::load_tree(Some(&file)) {
                Ok(doc) => doc,
                Err(err) => {
                    eprintln!("Failed to load file '{}': {}", file, err);
                    std::process::exit(1);
                }
            };

            // Resolve "today" alias to the current date (YYYY-MM-DD).
            // Both start and end default to today when not provided.
            let today = current_date_iso8601();
            let start_resolved = start
                .as_deref()
                .map(|s| if s == "today" { today.as_str() } else { s })
                .unwrap_or(today.as_str());
            let end_resolved = end
                .as_deref()
                .map(|e| if e == "today" { today.as_str() } else { e })
                .unwrap_or(today.as_str());

            let reports = compute_report(&document, Some(start_resolved), Some(end_resolved));

            if reports.is_empty() {
                println!("No @day entries found in the specified range.");
                return;
            }

            // Aggregate when the range spans more than one date and --no-aggregate
            // was not requested.
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
                    use nutrition_rs::nutrition::NutritionReport;
                    use nutrition_rs::ast::ast::Quantity;
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
                        use nutrition_rs::nutrition::NutritionReport;
                        use nutrition_rs::ast::ast::Quantity;
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

fn require_file(file: &Option<String>) -> String {
    file.clone().unwrap_or_else(|| {
        eprintln!(
            "Missing required argument: --file <FILE> (or set {})",
            env::DEFAULT_FILE_ENV_VAR
        );
        std::process::exit(1);
    })
}

fn print_document(node: nutrition_rs::ast::ast::Document) {
    println!("{:#?}", node);
}

/// Return a declaration-specific help message for ariadne's `with_help`.
fn help_for_kind(kind: &str) -> &'static str {
    match kind {
        "@day" => {
            "@day blocks may only contain `@ate` and `@exercised` entries"
        }
        "@ingredient" | "@food" => {
            "ingredients must have at least one quantity, one alias, and a `{ property: value }` body"
        }
        "@recipe" => {
            "recipes must have at least one quantity, one alias, and a body with `\"alias\"(quantity)` entries"
        }
        "@exercise" => {
            "exercises must have at least one quantity, one alias, and a `{ property: value }` body"
        }
        _ => "check that all required fields are present and the block is closed with `}`",
    }
}
