use std::path::{Path, PathBuf};
use std::collections::HashMap;

use crate::ast::ast::Document;
use crate::lexer::lexer::Token;
use crate::parser::parser::{parse, parse_with_diagnostics, parse_with_errors};
use crate::cli::env::get_default_file_from_env;
use logos::Logos;

#[derive(Clone, Debug)]
pub struct SourceSegment {
    pub generated: std::ops::Range<usize>,
    pub origin_file: String,
    pub origin: std::ops::Range<usize>,
}

#[derive(Clone, Debug)]
pub struct OriginSpan {
    pub file: String,
    pub span: std::ops::Range<usize>,
}

#[derive(Clone, Debug)]
pub struct ExpandedSourceMap {
    pub segments: Vec<SourceSegment>,
    pub sources: HashMap<String, String>,
}

impl ExpandedSourceMap {
    pub fn map_generated_span(&self, span: &std::ops::Range<usize>) -> Option<OriginSpan> {
        let generated_start = span.start;
        let segment = self
            .segments
            .iter()
            .find(|seg| generated_start >= seg.generated.start && generated_start < seg.generated.end)?;

        let offset_in_segment = generated_start.saturating_sub(segment.generated.start);
        let mut origin_start = segment.origin.start.saturating_add(offset_in_segment);
        if origin_start >= segment.origin.end {
            origin_start = segment.origin.end.saturating_sub(1);
        }

        let requested_len = span.end.saturating_sub(span.start).max(1);
        let available_len = segment.origin.end.saturating_sub(origin_start).max(1);
        let mapped_len = requested_len.min(available_len);
        let origin_end = origin_start.saturating_add(mapped_len);

        Some(OriginSpan {
            file: segment.origin_file.clone(),
            span: origin_start..origin_end,
        })
    }

    pub fn source_for_file(&self, file: &str) -> Option<&str> {
        self.sources.get(file).map(String::as_str)
    }
}

#[derive(Clone, Debug)]
struct ExpandedSource {
    source: String,
    source_map: ExpandedSourceMap,
}

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

fn append_segment(
    out_source: &mut String,
    segments: &mut Vec<SourceSegment>,
    origin_file: &str,
    origin_span: std::ops::Range<usize>,
    text: &str,
) {
    if text.is_empty() {
        return;
    }

    let generated_start = out_source.len();
    out_source.push_str(text);
    let generated_end = out_source.len();

    if generated_end > generated_start {
        segments.push(SourceSegment {
            generated: generated_start..generated_end,
            origin_file: origin_file.to_string(),
            origin: origin_span,
        });
    }
}

fn expand_imports_recursive(path: &Path, stack: &mut Vec<PathBuf>) -> Result<ExpandedSource, String> {
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

    let canonical_str = canonical.display().to_string();
    let mut expanded = String::new();
    let mut segments: Vec<SourceSegment> = Vec::new();
    let mut sources: HashMap<String, String> = HashMap::new();
    sources.insert(canonical_str.clone(), source.clone());

    let mut source_cursor = 0usize;
    for segment in source.split_inclusive('\n') {
        let segment_start = source_cursor;
        let segment_end = source_cursor + segment.len();
        source_cursor = segment_end;

        let line = segment.strip_suffix('\n').unwrap_or(segment);
        if let Some(import_path) = detect_import_path(line) {
            let target = resolve_import_target(&canonical, &import_path)?;
            let imported = expand_imports_recursive(&target, stack)?;

            let imported_offset = expanded.len();
            expanded.push_str(&imported.source);
            for seg in imported.source_map.segments {
                segments.push(SourceSegment {
                    generated: (seg.generated.start + imported_offset)..(seg.generated.end + imported_offset),
                    origin_file: seg.origin_file,
                    origin: seg.origin,
                });
            }
            for (path, src) in imported.source_map.sources {
                sources.entry(path).or_insert(src);
            }

            if !imported.source.ends_with('\n') {
                append_segment(
                    &mut expanded,
                    &mut segments,
                    &canonical_str,
                    segment_start..segment_end.min(segment_start + 1),
                    "\n",
                );
            }
        } else {
            append_segment(
                &mut expanded,
                &mut segments,
                &canonical_str,
                segment_start..segment_end,
                segment,
            );
        }
    }

    stack.pop();

    Ok(ExpandedSource {
        source: expanded,
        source_map: ExpandedSourceMap { segments, sources },
    })
}

fn load_expanded_source(path: &str) -> Result<ExpandedSource, String> {
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

    let expanded = load_expanded_source(&path)?;
    parse(&expanded.source).ok_or_else(|| format!("Failed to parse input file {}", path))
}

/// Parse a file and return both the (possibly partial) [`Document`] and any
/// human-readable parse error messages.  Uses error recovery so that a
/// malformed declaration does not prevent subsequent items from being parsed.
pub fn load_tree_with_errors(file_path: &str) -> (Option<Document>, Vec<String>) {
    let source = match load_expanded_source(file_path) {
        Ok(s) => s.source,
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
        ExpandedSourceMap,
        Option<crate::ast::ast::Document>,
        Vec<crate::parser::parser::ParseDiagnostic>,
    ),
    String,
> {
    let expanded = load_expanded_source(path)?;
    let (doc, diagnostics) = parse_with_diagnostics(&expanded.source);
    Ok((expanded.source, expanded.source_map, doc, diagnostics))
}
