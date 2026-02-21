use std::fs::File;
use std::io::BufReader;

use crate::ast::ast::Document;
use crate::parser::parser::{parse_reader, parse_reader_with_errors};
use crate::cli::env::get_default_file_from_env;

pub fn load_tree(file_path: Option<&str>) -> Result<Document, String> {
    let path = match file_path {
        Some(path) => path.to_string(),
        None => match get_default_file_from_env() {
            Some(env_path) => env_path,
            None => return Err("No file path provided and no default file set in environment.".to_string()),
        },
    };

    let file = File::open(&path)
        .map_err(|e| format!("Failed to open input file {}: {}", path, e))?;
    let reader = BufReader::new(file);

    parse_reader(reader)
        .ok_or_else(|| format!("Failed to parse input file {}", path))
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
            )
        }
    };
    let reader = BufReader::new(file);
    parse_reader_with_errors(reader)
}
