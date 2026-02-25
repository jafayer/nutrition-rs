use ariadne::{Color, Label, Report, ReportKind, Source};
use logos::Logos;

use crate::ast::ast::{Document, Item};
use crate::cli::file_loader;
use crate::lexer::lexer::Token;

fn normalize_label(label: &str) -> String {
    label.trim().to_lowercase()
}

struct SemanticDiagnostic {
    message: String,
    byte_span: std::ops::Range<usize>,
    note_message: String,
}

enum ReferenceKind {
    RecipeIngredient,
    DayAte,
    DayExercised,
}

struct ReferenceUse {
    kind: ReferenceKind,
    alias: String,
    span: std::ops::Range<usize>,
}

/// Return a declaration-specific help message for ariadne's `with_help`.
fn help_for_kind(kind: &str) -> &'static str {
    match kind {
        "@day" => {
            "@day blocks may only contain `@ate`, `@exercised`, and `[MealLabel]` entries"
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

fn skip_layout(tokens: &[Token], mut i: usize) -> usize {
    while i < tokens.len() {
        if matches!(tokens[i], Token::Newline | Token::Comment(_) | Token::Comma) {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn lex_with_spans(source: &str) -> (Vec<Token>, Vec<std::ops::Range<usize>>) {
    let mut tokens = Vec::new();
    let mut spans = Vec::new();
    let mut lex = Token::lexer(source);

    while let Some(result) = lex.next() {
        if let Ok(tok) = result {
            spans.push(lex.span());
            tokens.push(tok);
        }
    }

    (tokens, spans)
}

fn collect_reference_uses(source: &str) -> Vec<ReferenceUse> {
    let (tokens, spans) = lex_with_spans(source);
    let mut refs: Vec<ReferenceUse> = Vec::new();
    let mut i = 0usize;

    while i < tokens.len() {
        match &tokens[i] {
            Token::AtRecipe => {
                while i < tokens.len() && !matches!(tokens[i], Token::LBrace) {
                    i += 1;
                }
                if i >= tokens.len() {
                    break;
                }

                i += 1;
                let mut depth = 1usize;
                while i < tokens.len() && depth > 0 {
                    match &tokens[i] {
                        Token::LBrace => depth += 1,
                        Token::RBrace => depth = depth.saturating_sub(1),
                        Token::String(alias) if depth == 1 => {
                            if let Some(span) = spans.get(i) {
                                refs.push(ReferenceUse {
                                    kind: ReferenceKind::RecipeIngredient,
                                    alias: alias.clone(),
                                    span: span.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            Token::AtDay => {
                while i < tokens.len() && !matches!(tokens[i], Token::LBrace) {
                    i += 1;
                }
                if i >= tokens.len() {
                    break;
                }

                i += 1;
                let mut depth = 1usize;
                while i < tokens.len() && depth > 0 {
                    match &tokens[i] {
                        Token::LBrace => depth += 1,
                        Token::RBrace => depth = depth.saturating_sub(1),
                        Token::AtAte if depth == 1 => {
                            let j = skip_layout(&tokens, i + 1);
                            if j < tokens.len() {
                                if let Token::String(alias) = &tokens[j] {
                                    if let Some(span) = spans.get(j) {
                                        refs.push(ReferenceUse {
                                            kind: ReferenceKind::DayAte,
                                            alias: alias.clone(),
                                            span: span.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        Token::AtExercised if depth == 1 => {
                            let j = skip_layout(&tokens, i + 1);
                            if j < tokens.len() {
                                if let Token::String(alias) = &tokens[j] {
                                    if let Some(span) = spans.get(j) {
                                        refs.push(ReferenceUse {
                                            kind: ReferenceKind::DayExercised,
                                            alias: alias.clone(),
                                            span: span.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    refs
}

fn validate_semantics(document: &Document, source: &str) -> Vec<SemanticDiagnostic> {
    let mut ingredient_or_recipe_aliases = std::collections::HashSet::<String>::new();
    let mut exercise_aliases = std::collections::HashSet::<String>::new();

    for item in &document.items {
        match item {
            Item::Ingredient(ingredient) => {
                for alias in &ingredient.aliases {
                    ingredient_or_recipe_aliases.insert(normalize_label(alias));
                }
            }
            Item::Recipe(recipe) => {
                for alias in &recipe.aliases {
                    ingredient_or_recipe_aliases.insert(normalize_label(alias));
                }
            }
            Item::Exercise(exercise) => {
                for alias in &exercise.aliases {
                    exercise_aliases.insert(normalize_label(alias));
                }
            }
            _ => {}
        }
    }

    let mut diagnostics = Vec::new();

    for usage in collect_reference_uses(source) {
        let alias_key = normalize_label(&usage.alias);
        match usage.kind {
            ReferenceKind::RecipeIngredient if !ingredient_or_recipe_aliases.contains(&alias_key) => {
                diagnostics.push(SemanticDiagnostic {
                    message: "invalid recipe ingredient reference".to_string(),
                    byte_span: usage.span,
                    note_message: format!(
                        "unknown ingredient/recipe alias '{}' referenced in @recipe",
                        usage.alias
                    ),
                });
            }
            ReferenceKind::DayAte if !ingredient_or_recipe_aliases.contains(&alias_key) => {
                diagnostics.push(SemanticDiagnostic {
                    message: "invalid @ate reference".to_string(),
                    byte_span: usage.span,
                    note_message: format!(
                        "unknown ingredient/recipe alias '{}' referenced by @ate",
                        usage.alias
                    ),
                });
            }
            ReferenceKind::DayExercised if !exercise_aliases.contains(&alias_key) => {
                diagnostics.push(SemanticDiagnostic {
                    message: "invalid @exercised reference".to_string(),
                    byte_span: usage.span,
                    note_message: format!(
                        "unknown exercise alias '{}' referenced by @exercised",
                        usage.alias
                    ),
                });
            }
            _ => {}
        }
    }

    diagnostics
}

pub fn run_validate(file: &str, show_tree: bool) -> Result<(), i32> {
    let (source, document, diagnostics) = match file_loader::load_source_with_diagnostics(file) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("error: {}", e);
            return Err(1);
        }
    };

    for diag in &diagnostics {
        let mut report =
            Report::build(ReportKind::Error, file, diag.byte_span.start)
                .with_message(&diag.message)
                .with_label(
                    Label::new((file, diag.byte_span.clone()))
                        .with_message(format!("this {} could not be parsed", diag.declaration_kind))
                        .with_color(Color::Red),
                );

        if let (Some(note_span), Some(note_msg)) = (&diag.note_span, &diag.note_message) {
            report = report.with_label(
                Label::new((file, note_span.clone()))
                    .with_message(note_msg)
                    .with_color(Color::Yellow),
            );
        }

        report
            .with_help(help_for_kind(diag.declaration_kind))
            .finish()
            .eprint((file, Source::from(&source)))
            .unwrap();
    }

    match document {
        Some(doc) if diagnostics.is_empty() => {
            let semantic_diagnostics = validate_semantics(&doc, &source);

            for diag in &semantic_diagnostics {
                Report::build(ReportKind::Error, file, diag.byte_span.start)
                    .with_message(&diag.message)
                    .with_label(
                        Label::new((file, diag.byte_span.clone()))
                            .with_message(&diag.note_message)
                            .with_color(Color::Red),
                    )
                    .with_help("ensure referenced aliases are defined by @ingredient/@food/@recipe or @exercise before use")
                    .finish()
                    .eprint((file, Source::from(&source)))
                    .unwrap();
            }

            if semantic_diagnostics.is_empty() {
                let item_count = doc
                    .items
                    .iter()
                    .filter(|i| !matches!(i, Item::Comment(_)))
                    .count();
                println!("✓ '{}' is valid ({} item(s)).", file, item_count);
                if show_tree {
                    super::print_document(doc);
                }
                Ok(())
            } else {
                eprintln!(
                    "✗ '{}' has {} semantic error(s).",
                    file,
                    semantic_diagnostics.len()
                );
                if show_tree {
                    super::print_document(doc);
                }
                Err(1)
            }
        }
        Some(doc) => {
            let recovered = doc
                .items
                .iter()
                .filter(|i| !matches!(i, Item::Comment(_)))
                .count();
            eprintln!(
                "✗ '{}' has {} parse error(s); {} item(s) recovered.",
                file,
                diagnostics.len(),
                recovered,
            );
            if show_tree {
                super::print_document(doc);
            }
            Err(1)
        }
        None => {
            eprintln!("✗ '{}' could not be parsed.", file);
            Err(1)
        }
    }
}

#[cfg(test)]
mod tests {
        use super::validate_semantics;
        use crate::parser::parser::parse;

        #[test]
        fn semantic_validate_reports_unknown_recipe_ingredient_alias() {
                let source = r#"@recipe(1) "bad recipe" {
    "missing ingredient"(1 cup)
}"#;

                let doc = parse(source).expect("source should parse syntactically");
                let diagnostics = validate_semantics(&doc, source);

                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].message, "invalid recipe ingredient reference");
                assert!(diagnostics[0]
                        .note_message
                        .contains("unknown ingredient/recipe alias 'missing ingredient'"));
        }

        #[test]
        fn semantic_validate_reports_unknown_day_ate_alias() {
                let source = r#"@day "2026-01-01" {
    @ate "missing food"(2)
}"#;

                let doc = parse(source).expect("source should parse syntactically");
                let diagnostics = validate_semantics(&doc, source);

                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].message, "invalid @ate reference");
                assert!(diagnostics[0]
                        .note_message
                        .contains("unknown ingredient/recipe alias 'missing food'"));
        }

        #[test]
        fn semantic_validate_reports_unknown_day_exercised_alias() {
                let source = r#"@day "2026-01-01" {
    @exercised "missing exercise"(30 min)
}"#;

                let doc = parse(source).expect("source should parse syntactically");
                let diagnostics = validate_semantics(&doc, source);

                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].message, "invalid @exercised reference");
                assert!(diagnostics[0]
                        .note_message
                        .contains("unknown exercise alias 'missing exercise'"));
        }

        #[test]
        fn semantic_validate_allows_resolved_references() {
                let source = r#"@ingredient(100g) "known ingredient" {
    calories: 200
}

@exercise(30 min) "known exercise" {
    calories: 100kcal
}

@recipe(1) "good recipe" {
    "known ingredient"(50g)
}

@day "2026-01-01" {
    @ate "good recipe"(1)
    @exercised "known exercise"(30 min)
}"#;

                let doc = parse(source).expect("source should parse syntactically");
                let diagnostics = validate_semantics(&doc, source);

                assert!(diagnostics.is_empty());
        }

    #[test]
    fn semantic_validate_allows_resolved_references_case_insensitively() {
        let source = r#"@ingredient(100g) "Known Ingredient" {
    calories: 200
}

@exercise(30 min) "Known Exercise" {
    calories: 100kcal
}

@recipe(1) "good recipe" {
    "KNOWN INGREDIENT"(50g)
}

@day "2026-01-01" {
    @ate "Good Recipe"(1)
    @exercised "known exercise"(30 min)
}"#;

        let doc = parse(source).expect("source should parse syntactically");
        let diagnostics = validate_semantics(&doc, source);

        assert!(diagnostics.is_empty());
    }
}
