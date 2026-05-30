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

        #[arg(
            short = 'H',
            long,
            help = "Host to bind to",
            default_value = "127.0.0.1"
        )]
        host: String
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
        #[arg(help = "Optional date (e.g. '2026-01-01')")]
        date: Option<String>,

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

#[derive(Debug, Clone)]
enum ReportMode {
    Single { date: String },
    Range { start: String, end: String },
}

fn normalize_report_date(raw: Option<&str>, today: &str) -> String {
    match raw {
        Some("today") | None => today.to_string(),
        Some(value) => value.to_string(),
    }
}

fn resolve_report_mode(
    date: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    today: &str,
) -> Result<ReportMode, String> {
    if date.is_some() && (start.is_some() || end.is_some()) {
        return Err(
            "`report [date]` cannot be combined with `--start`/`--end`; use either single-day mode or range mode".to_string(),
        );
    }

    if let Some(single_date) = date {
        return Ok(ReportMode::Single {
            date: normalize_report_date(Some(single_date), today),
        });
    }

    if start.is_some() || end.is_some() {
        return Ok(ReportMode::Range {
            start: normalize_report_date(start, today),
            end: normalize_report_date(end, today),
        });
    }

    Ok(ReportMode::Single {
        date: today.to_string(),
    })
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

        Commands::Serve { port, host } => {
            let file = require_file(&cli.file);
            crate::web_server::handler::run_server(host, port, file).await.unwrap();
        }

        Commands::Query { name, quantity, json } => {
            let file = require_file(&cli.file);
            let (source, source_map, doc, diagnostics) = match file_loader::load_source_with_diagnostics(&file) {
                Ok(result) => result,
                Err(err) => {
                    eprintln!("Failed to load file '{}': {}", file, err);
                    std::process::exit(1);
                }
            };
            if !diagnostics.is_empty() {
                validate::render_parse_diagnostics_to_stderr(&file, &source, &source_map, &diagnostics);
            }
            let document = match doc {
                Some(d) => d,
                None => {
                    eprintln!("error: failed to parse '{}'", file);
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

        Commands::Report { date, start, end, ate_only, no_aggregate, json, trace } => {
            let file = require_file(&cli.file);
            let (source, source_map, doc, diagnostics) = match file_loader::load_source_with_diagnostics(&file) {
                Ok(result) => result,
                Err(err) => {
                    eprintln!("Failed to load file '{}': {}", file, err);
                    std::process::exit(1);
                }
            };
            if !diagnostics.is_empty() {
                validate::render_parse_diagnostics_to_stderr(&file, &source, &source_map, &diagnostics);
            }
            let document = match doc {
                Some(d) => d,
                None => {
                    eprintln!("error: failed to parse '{}'", file);
                    std::process::exit(1);
                }
            };

            let today = current_date_iso8601();
            let report_mode = match resolve_report_mode(
                date.as_deref(),
                start.as_deref(),
                end.as_deref(),
                &today,
            ) {
                Ok(mode) => mode,
                Err(message) => {
                    eprintln!("Report options error: {}", message);
                    std::process::exit(1);
                }
            };

            let (start_resolved, end_resolved) = match &report_mode {
                ReportMode::Single { date } => (date.as_str(), date.as_str()),
                ReportMode::Range { start, end } => (start.as_str(), end.as_str()),
            };

            if trace {
                let traces = compute_trace_report(&document, Some(start_resolved), Some(end_resolved));
                if traces.is_empty() {
                    match report_mode {
                        ReportMode::Single { .. } => {
                            println!("No @day entry found for the specified date.");
                        }
                        ReportMode::Range { .. } => {
                            println!("No @day entries found in the specified range.");
                        }
                    }
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
                match report_mode {
                    ReportMode::Single { .. } => {
                        println!("No @day entry found for the specified date.");
                    }
                    ReportMode::Range { .. } => {
                        println!("No @day entries found in the specified range.");
                    }
                }
                return;
            }

            let use_aggregate = matches!(report_mode, ReportMode::Range { .. })
                && start_resolved != end_resolved
                && !no_aggregate;

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

#[cfg(test)]
mod tests {
    use super::{resolve_report_mode, ReportMode};

    #[test]
    fn report_mode_defaults_to_single_today() {
        let mode = resolve_report_mode(None, None, None, "2026-02-26").unwrap();
        match mode {
            ReportMode::Single { date } => assert_eq!(date, "2026-02-26"),
            other => panic!("expected single mode, got: {other:?}"),
        }
    }

    #[test]
    fn report_mode_uses_positional_date() {
        let mode = resolve_report_mode(Some("2026-01-07"), None, None, "2026-02-26").unwrap();
        match mode {
            ReportMode::Single { date } => assert_eq!(date, "2026-01-07"),
            other => panic!("expected single mode, got: {other:?}"),
        }
    }

    #[test]
    fn report_mode_supports_range_with_partial_bounds() {
        let mode = resolve_report_mode(None, Some("2026-01-01"), None, "2026-02-26").unwrap();
        match mode {
            ReportMode::Range { start, end } => {
                assert_eq!(start, "2026-01-01");
                assert_eq!(end, "2026-02-26");
            }
            other => panic!("expected range mode, got: {other:?}"),
        }
    }

    #[test]
    fn report_mode_rejects_mixing_date_and_range_flags() {
        let error = resolve_report_mode(
            Some("2026-01-07"),
            Some("2026-01-01"),
            Some("2026-01-31"),
            "2026-02-26",
        )
        .expect_err("expected conflict error");
        assert!(error.contains("cannot be combined"));
    }
}