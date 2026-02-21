use crate::ast::ast::Document;
use crate::tree_sitter_ast::ast::parse;
use std::fs;
use crate::cli::env::get_default_file_from_env;

pub fn load_tree(file_path: Option<&str>) -> Result<Document, String> {
    let path = match file_path {
        Some(path) => path.to_string(),
        None => match get_default_file_from_env() {
            Some(env_path) => env_path,
            None => return Err("No file path provided and no default file set in environment.".to_string()),
        },
    };

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read input file {}: {}", path, e))?;

    parse(&content)
        .ok_or_else(|| format!("Failed to parse input file {}", path))
}
