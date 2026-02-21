use std::fs::File;
use std::io::BufReader;

use crate::ast::ast::Document;
use crate::tree_sitter_ast::ast::parse_reader;
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
