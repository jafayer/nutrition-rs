use std::fs::File;
use std::io::BufReader;

use crate::ast::ast::Document;
use crate::cli::env::get_default_file_from_env;
use crate::parser::parser::{parse_reader, parse_reader_with_errors};

pub fn load_tree(file_path: Option<&str>) -> Result<Document, String> {
    let path = match file_path {
        Some(path) => path.to_string(),
        None => match get_default_file_from_env() {
            Some(env_path) => env_path,
            None => {
                return Err(
                    "No file path provided and no default file set in environment.".to_string(),
                );
            }
        },
    };

    let file =
        File::open(&path).map_err(|e| format!("Failed to open input file {}: {}", path, e))?;
    let reader = BufReader::new(file);

    parse_reader(reader).ok_or_else(|| format!("Failed to parse input file {}", path))
}

/// Parse a file and return both the (possibly partial) [`Document`] and any
/// human-readable parse error messages.  Uses error recovery so that a
/// malformed declaration does not prevent subsequent items from being parsed.
pub fn load_tree_with_errors(file_path: &str) -> (Option<Document>, Vec<String>) {
    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            return (
                None,
                vec![format!("Failed to open file '{}': {}", file_path, e)],
            );
        }
    };
    let reader = BufReader::new(file);
    parse_reader_with_errors(reader)
}

/// Read a file fully into memory and parse it with structured byte-span
/// diagnostics suitable for rich rendering (e.g. via `ariadne`).
///
/// Returns `(source_text, document, diagnostics)`.  `document` may be a
/// partial result when some declarations failed to parse but others succeeded.
pub fn load_source_with_diagnostics(
    path: &str,
) -> Result<
    (
        String,
        Option<crate::ast::ast::Document>,
        Vec<crate::parser::parser::ParseDiagnostic>,
    ),
    String,
> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;
    let (doc, diagnostics) = crate::parser::parser::parse_with_diagnostics(&source);
    Ok((source, doc, diagnostics))
}
