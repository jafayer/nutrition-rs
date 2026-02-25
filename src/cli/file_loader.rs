use std::path::{Path, PathBuf};

use crate::ast::ast::Document;
use crate::lexer::lexer::Token;
use crate::parser::parser::{parse, parse_with_diagnostics, parse_with_errors};
use crate::cli::env::get_default_file_from_env;
use logos::Logos;

fn detect_import_path(line: &str) -> Option<String> {
    let tokens: Vec<Token> = Token::lexer(line).filter_map(Result::ok).collect();
    match tokens.as_slice() {
        [Token::ImportDirective, Token::String(path)] => Some(path.clone()),
        [Token::ImportDirective, Token::String(path), Token::Comment(_)] => Some(path.clone()),
        _ => None,
    }
}

fn resolve_import_target(current_file: &Path, raw_path: &str) -> Result<PathBuf, String> {
    let base_dir = current_file
        .parent()
        .ok_or_else(|| format!("Failed to resolve parent directory for '{}'", current_file.display()))?;

    let candidate = Path::new(raw_path);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    };

    std::fs::canonicalize(&joined).map_err(|e| {
        format!(
            "Failed to resolve !import '{}' from '{}': {}",
            raw_path,
            current_file.display(),
            e
        )
    })
}

fn format_import_cycle(stack: &[PathBuf], repeated: &Path) -> String {
    let mut chain: Vec<String> = stack.iter().map(|p| p.display().to_string()).collect();
    chain.push(repeated.display().to_string());
    chain.join(" -> ")
}

fn expand_imports_recursive(path: &Path, stack: &mut Vec<PathBuf>) -> Result<String, String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| format!("Failed to resolve file '{}': {}", path.display(), e))?;

    if stack.contains(&canonical) {
        return Err(format!(
            "Cyclic !import detected: {}",
            format_import_cycle(stack, &canonical)
        ));
    }

    stack.push(canonical.clone());
    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("Failed to read file '{}': {}", canonical.display(), e))?;

    let mut expanded = String::new();
    for segment in source.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        if let Some(import_path) = detect_import_path(line) {
            let target = resolve_import_target(&canonical, &import_path)?;
            let imported = expand_imports_recursive(&target, stack)?;
            expanded.push_str(&imported);
            if !imported.ends_with('\n') {
                expanded.push('\n');
            }
        } else {
            expanded.push_str(segment);
        }
    }

    stack.pop();
    Ok(expanded)
}

fn load_expanded_source(path: &str) -> Result<String, String> {
    let mut stack = Vec::new();
    expand_imports_recursive(Path::new(path), &mut stack)
}

pub fn load_tree(file_path: Option<&str>) -> Result<Document, String> {
    let path = match file_path {
        Some(path) => path.to_string(),
        None => match get_default_file_from_env() {
            Some(env_path) => env_path,
            None => return Err("No file path provided and no default file set in environment.".to_string()),
        },
    };

    let source = load_expanded_source(&path)?;
    parse(&source).ok_or_else(|| format!("Failed to parse input file {}", path))
}

/// Parse a file and return both the (possibly partial) [`Document`] and any
/// human-readable parse error messages.  Uses error recovery so that a
/// malformed declaration does not prevent subsequent items from being parsed.
pub fn load_tree_with_errors(file_path: &str) -> (Option<Document>, Vec<String>) {
    let source = match load_expanded_source(file_path) {
        Ok(s) => s,
        Err(e) => return (None, vec![e]),
    };
    parse_with_errors(&source)
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
    let source = load_expanded_source(path)?;
    let (doc, diagnostics) = parse_with_diagnostics(&source);
    Ok((source, doc, diagnostics))
}
