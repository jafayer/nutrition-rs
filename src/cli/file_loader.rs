use crate::ast::ast::{Document};
use crate::tree_sitter_ast::ast::{parse};
use crate::tree_sitter_ast::semantic::SemanticAnalyzer;
use std::fs;
use crate::cli::env::{get_default_file_from_env};

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

    let parsed = parse(&content, None)
        .ok_or_else(|| format!("Failed to parse input file {} with Tree-sitter", path))?;

    let mut analyzer = SemanticAnalyzer::new();
    let root_node = parsed.root_node();

    let document = analyzer.analyze(root_node, &content)
        .map_err(|e| format!("Failed to analyze semantic AST for file {}: {}", path, e))?;

    Ok(document)
}