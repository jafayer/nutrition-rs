use chumsky::prelude::*;
use logos::Logos;
use std::io::BufRead;

use crate::{ast::ast::*, lexer::lexer::Token};

// Inside brace-delimited blocks, allow both newlines and inline comments to be skipped.
fn skip_block_ws<'a>() -> impl Parser<'a, &'a [Token], ()> + Clone {
    any()
        .filter(|tok| matches!(tok, Token::Newline | Token::Comment(_)))
        .repeated()
        .ignored()
}

// Separator inside blocks: commas, newlines, and inline comments.
fn block_separator<'a>() -> impl Parser<'a, &'a [Token], ()> + Clone {
    any()
        .filter(|tok| matches!(tok, Token::Comma | Token::Newline | Token::Comment(_)))
        .repeated()
        .at_least(1)
        .ignored()
}

fn parse_number<'a>() -> impl Parser<'a, &'a [Token], f64> + Clone {
    select! { Token::Number(n) => n }
}

fn parse_string<'a>() -> impl Parser<'a, &'a [Token], String> + Clone {
    select! { Token::String(s) => s }
}

fn parse_identifier<'a>() -> impl Parser<'a, &'a [Token], String> + Clone {
    select! { Token::Identifier(id) => id }
}

fn parse_quantity<'a>() -> impl Parser<'a, &'a [Token], Quantity> + Clone {
    parse_number()
        .then(
            parse_identifier()
                .repeated()
                .at_least(1)
                .collect::<Vec<String>>()
                .map(|parts| Some(parts.join(" "))),
        )
        .or(parse_number().map(|n| (n, None)))
        .map(|(amount, unit)| Quantity { amount, unit })
}

fn parse_quantities_in_parens<'a>() -> impl Parser<'a, &'a [Token], Vec<Quantity>> + Clone {
    just(Token::LParen)
        .ignore_then(parse_quantity())
        .then_ignore(just(Token::RParen))
        .repeated()
        .collect()
}

fn parse_property<'a>() -> impl Parser<'a, &'a [Token], Property> + Clone {
    parse_identifier()
        .then_ignore(just(Token::Colon))
        .then(parse_quantity())
        .map(|(name, value)| Property { name, value })
}

fn parse_ingredient_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    just(Token::AtIngredient)
        .or(just(Token::AtFood))
        .ignore_then(parse_quantities_in_parens())
        .then(parse_string().repeated().at_least(1).collect())
        .then(
            just(Token::LBrace)
                .ignore_then(skip_block_ws())
                .ignore_then(
                    parse_property()
                        .separated_by(block_separator())
                        .allow_trailing()
                        .collect()
                        .or_not()
                        .map(|opt| opt.unwrap_or_default()),
                )
                .then_ignore(skip_block_ws())
                .then_ignore(just(Token::RBrace)),
        )
        .map(|((quantities, aliases), properties)| {
            Item::Ingredient(Ingredient {
                aliases,
                quantities,
                properties,
            })
        })
}

fn parse_ingredient_label<'a>() -> impl Parser<'a, &'a [Token], IngredientLabel> + Clone {
    parse_string()
        .then(
            just(Token::LParen)
                .ignore_then(parse_quantity())
                .then_ignore(just(Token::RParen)),
        )
        .map(|(alias, quantity)| IngredientLabel { alias, quantity })
}

fn parse_recipe_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    just(Token::AtRecipe)
        .ignore_then(parse_quantities_in_parens())
        .then(parse_string().repeated().at_least(1).collect())
        .then(
            just(Token::LBrace)
                .ignore_then(skip_block_ws())
                .ignore_then(
                    parse_ingredient_label()
                        .separated_by(block_separator())
                        .allow_trailing()
                        .collect(),
                )
                .then_ignore(skip_block_ws())
                .then_ignore(just(Token::RBrace)),
        )
        .map(|((quantities, aliases), ingredients)| {
            Item::Recipe(Recipe {
                aliases,
                quantities,
                ingredients,
            })
        })
}

fn parse_ate_item<'a>() -> impl Parser<'a, &'a [Token], Ate> + Clone {
    just(Token::AtAte)
        .ignore_then(parse_string())
        .then(
            just(Token::LParen)
                .ignore_then(parse_quantity())
                .then_ignore(just(Token::RParen))
                .or_not(),
        )
        .map(|(food_alias, quantity)| Ate {
            food_alias,
            quantity: quantity.unwrap_or(Quantity {
                amount: 1.0,
                unit: None,
            }),
        })
}

fn parse_exercised_item<'a>() -> impl Parser<'a, &'a [Token], Exercised> + Clone {
    just(Token::AtExercised)
        .ignore_then(parse_string())
        .then(
            just(Token::LParen)
                .ignore_then(parse_quantity())
                .then_ignore(just(Token::RParen))
                .or_not(),
        )
        .map(|(exercise_alias, quantity)| Exercised {
            exercise_alias,
            quantity: quantity.unwrap_or(Quantity {
                amount: 1.0,
                unit: None,
            }),
        })
}

fn parse_day_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    just(Token::AtDay)
        .ignore_then(parse_string())
        .then(
            just(Token::LBrace)
                .ignore_then(skip_block_ws())
                .ignore_then(
                    parse_meal_label()
                        .or(parse_ate_item().map(DayItem::Ate))
                        .or(parse_exercised_item().map(DayItem::Exercised))
                        .separated_by(block_separator())
                        .allow_trailing()
                        .collect(),
                )
                .then_ignore(skip_block_ws())
                .then_ignore(just(Token::RBrace)),
        )
        .map(|(date, items)| Item::Day(Day { date, items }))
}

fn parse_exercise_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    just(Token::AtExercise)
        .ignore_then(parse_quantities_in_parens())
        .then(parse_string().repeated().at_least(1).collect())
        .then(
            just(Token::LBrace)
                .ignore_then(skip_block_ws())
                .ignore_then(
                    parse_property()
                        .separated_by(block_separator())
                        .allow_trailing()
                        .collect()
                        .or_not()
                        .map(|opt| opt.unwrap_or_default()),
                )
                .then_ignore(skip_block_ws())
                .then_ignore(just(Token::RBrace)),
        )
        .map(|((quantities, aliases), properties)| {
            Item::Exercise(Exercise {
                aliases,
                quantities,
                properties,
            })
        })
}

fn parse_meal_label<'a>() -> impl Parser<'a, &'a [Token], DayItem> + Clone {
    select! { Token::MealLabel(label) => {
        // Extract the label text without the brackets
        let trimmed = label.trim_start_matches('[').trim_end_matches(']').to_string();
        DayItem::Meal(trimmed)
    }}
}

fn parse_comment<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    select! { Token::Comment(c) => Item::Comment(c) }
}

fn parse_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    parse_ingredient_item()
        .or(parse_recipe_item())
        .or(parse_exercise_item())
        .or(parse_day_item())
        .or(parse_comment())
}

pub fn parser<'a>() -> impl Parser<'a, &'a [Token], Document> + Clone {
    // Newlines used as whitespace between top-level items
    let newlines = just(Token::Newline).repeated().ignored();

    // Each item is optionally preceded by newlines and followed by newlines.
    // Comments are parsed as first-class items rather than consumed as
    // whitespace, so that "// comment\n// comment" yields two Comment items.
    newlines
        .clone()
        .ignore_then(
            parse_item()
                .then_ignore(newlines)
                .repeated()
                .at_least(0)
                .collect(),
        )
        .then_ignore(end())
        .map(|items| Document { items })
}
/// Parse a nutrition source string into a [`Document`].
/// Returns `Some(Document)` if parsing succeeds, or `None` if the input
/// cannot be parsed.
pub fn parse(source: &str) -> Option<Document> {
    let tokens: Vec<Token> = Token::lexer(source).filter_map(Result::ok).collect();
    parser().parse(tokens.as_slice()).into_output()
}

/// Parse a nutrition document from any [`BufRead`] source (e.g. a
/// `BufReader<File>`) without loading the entire contents into memory at once.
///
/// The source is consumed one line at a time: each line is lexed
/// individually and the resulting tokens are accumulated.  A
/// [`Token::Newline`] is appended after every line so the parser sees the
/// same token stream it would see when parsing a full string.  Because
/// every [`Token`] variant owns its data, the line buffer can be dropped
/// immediately after lexing.
///
/// # Limitations
/// All tokens in the Nutrition language are single-line (comments are
/// `//[^\n]*` and string literals do not support embedded literal newlines),
/// so line-by-line lexing is equivalent to lexing the full source at once.
/// An extra [`Token::Newline`] is appended after the last line regardless of
/// whether the source ends with a newline; the parser consumes trailing
/// newlines gracefully via its `repeated()` skip rules, so this has no
/// observable effect on the resulting [`Document`].
pub fn parse_reader<R: BufRead>(reader: R) -> Option<Document> {
    let mut tokens: Vec<Token> = Vec::new();
    for line_result in reader.lines() {
        let line = line_result.ok()?;
        tokens.extend(Token::lexer(&line).filter_map(Result::ok));
        tokens.push(Token::Newline);
    }
    parser().parse(tokens.as_slice()).into_output()
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Error-recovery parser and reporting helpers
// ---------------------------------------------------------------------------

/// Returns the human-readable name of the declaration that starts with `tok`.
fn declaration_name(tok: &Token) -> &'static str {
    match tok {
        Token::AtIngredient | Token::AtFood => "@ingredient",
        Token::AtRecipe => "@recipe",
        Token::AtExercise => "@exercise",
        Token::AtDay => "@day",
        Token::AtAte => "@ate",
        Token::AtExercised => "@exercised",
        Token::Comment(_) => "comment",
        _ => "declaration",
    }
}

/// Returns `true` for tokens that begin a top-level declaration.
fn is_top_level_start(tok: &Token) -> bool {
    matches!(
        tok,
        Token::AtIngredient
            | Token::AtFood
            | Token::AtRecipe
            | Token::AtExercise
            | Token::AtDay
            | Token::Comment(_)
    )
}

/// Build a `token_index → 1-based line number` mapping.
///
/// Each [`Token::Newline`] increments the line counter for subsequent tokens.
fn token_line_map(tokens: &[Token]) -> Vec<usize> {
    let mut map = Vec::with_capacity(tokens.len());
    let mut line = 1usize;
    for tok in tokens {
        map.push(line);
        if matches!(tok, Token::Newline) {
            line += 1;
        }
    }
    map
}

/// Split a flat token slice into per-declaration chunks.
///
/// Splitting rules:
/// * `@ingredient`, `@food`, `@recipe`, `@exercise`, `@day` **always** start a
///   new chunk, even when the brace depth is > 0.  These keywords cannot
///   legitimately appear inside a `{…}` block, so seeing one at depth > 0
///   signals an unclosed block from the previous declaration.  Force-splitting
///   here allows the subsequent declaration to be parsed correctly.
/// * `@ate` and `@exercised` are **not** split points because they can appear
///   legitimately inside a `@day { … }` block.
/// * Comments at brace depth 0 form their own single-token chunks.
/// * A chunk that ends with a `}` that brings depth back to 0 is closed there.
///
/// Returns `(start, end)` index pairs (exclusive end) into `tokens`.
fn split_chunks(tokens: &[Token]) -> Vec<(usize, usize)> {
    let mut chunks: Vec<(usize, usize)> = Vec::new();
    let mut chunk_start: Option<usize> = None;
    let mut brace_depth = 0usize;

    for (i, tok) in tokens.iter().enumerate() {
        match tok {
            Token::LBrace => brace_depth += 1,
            Token::RBrace => {
                brace_depth = brace_depth.saturating_sub(1);
                // Closing the outermost brace ends this chunk.
                if brace_depth == 0 {
                    if let Some(start) = chunk_start {
                        chunks.push((start, i + 1));
                        chunk_start = None;
                    }
                }
            }
            // These keywords CANNOT appear inside blocks → always force a split.
            Token::AtIngredient
            | Token::AtFood
            | Token::AtRecipe
            | Token::AtExercise
            | Token::AtDay => {
                if let Some(start) = chunk_start {
                    chunks.push((start, i));
                }
                // Reset brace depth: the previous block was unclosed.
                brace_depth = 0;
                chunk_start = Some(i);
            }
            // Comments at depth 0 are standalone single-token chunks.
            Token::Comment(_) if brace_depth == 0 => {
                if let Some(start) = chunk_start {
                    chunks.push((start, i));
                }
                chunks.push((i, i + 1));
                chunk_start = None;
            }
            _ => {}
        }
    }

    // Any tokens after the last closed block form a trailing chunk.
    if let Some(start) = chunk_start {
        if start < tokens.len() {
            chunks.push((start, tokens.len()));
        }
    }

    // Drop chunks that contain nothing but newline tokens.
    chunks.retain(|(s, e)| tokens[*s..*e].iter().any(|t| !matches!(t, Token::Newline)));

    chunks
}

/// Parse a token chunk produced by [`split_chunks`] and return the items it
/// contains, or an error string describing what failed.
fn parse_chunk(chunk: &[Token], start_line: usize) -> Result<Vec<Item>, String> {
    // Append a trailing Newline so `parser()` sees a clean end-of-stream.
    let mut padded: Vec<Token> = chunk.to_vec();
    padded.push(Token::Newline);
    match parser().parse(padded.as_slice()).into_output() {
        Some(doc) => Ok(doc.items),
        None => {
            // Build a descriptive message from the declaration keyword.
            let decl = chunk
                .iter()
                .find(|t| is_top_level_start(t))
                .map(declaration_name)
                .unwrap_or("declaration");
            Err(format!("line {start_line}: malformed {decl}"))
        }
    }
}

/// Parse a nutrition source string, returning both the (possibly partial)
/// [`Document`] and a list of human-readable error messages.
///
/// Recovery is performed at declaration-boundary level: when one declaration
/// fails to parse, its error is recorded and parsing continues with the next
/// declaration.  This ensures a single malformed block does not prevent the
/// rest of the file from being processed.
///
/// Returns `(None, errors)` only when the input is so badly formed that not
/// even a partial document could be produced.
pub fn parse_with_errors(source: &str) -> (Option<Document>, Vec<String>) {
    let tokens: Vec<Token> = Token::lexer(source).filter_map(Result::ok).collect();
    if tokens.is_empty() {
        return (Some(Document { items: vec![] }), vec![]);
    }
    let line_map = token_line_map(&tokens);
    let chunks = split_chunks(&tokens);

    // If the file has non-whitespace content but no recognisable declarations,
    // report that nothing was understood rather than silently returning empty.
    let has_content = tokens.iter().any(|t| !matches!(t, Token::Newline));
    if has_content && chunks.is_empty() {
        return (None, vec!["no recognizable declarations found".to_string()]);
    }

    let mut items: Vec<Item> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (start, end) in &chunks {
        let chunk = &tokens[*start..*end];
        let start_line = line_map.get(*start).copied().unwrap_or(1);
        match parse_chunk(chunk, start_line) {
            Ok(chunk_items) => items.extend(chunk_items),
            Err(e) => errors.push(e),
        }
    }

    if items.is_empty() && !errors.is_empty() {
        (None, errors)
    } else {
        (Some(Document { items }), errors)
    }
}

/// Parse a nutrition document from a [`BufRead`] source with full error
/// reporting and per-declaration recovery.
///
/// Each line is lexed individually (line buffers are dropped after lexing) and
/// a [`Token::Newline`] is injected between lines to preserve correct
/// whitespace semantics.  Line numbers are tracked during streaming so that
/// error messages reference the correct source line.
pub fn parse_reader_with_errors<R: BufRead>(reader: R) -> (Option<Document>, Vec<String>) {
    let mut tokens: Vec<Token> = Vec::new();
    let mut line_numbers: Vec<usize> = Vec::new();
    // Starts at 0; the loop increments it to 1 before processing the first line,
    // so the first real line gets line number 1.
    let mut line_num = 0usize;

    for line_result in reader.lines() {
        line_num += 1;
        match line_result {
            Ok(line) => {
                let line_tokens: Vec<Token> = Token::lexer(&line).filter_map(Result::ok).collect();
                let count = line_tokens.len();
                line_numbers.extend(std::iter::repeat(line_num).take(count));
                tokens.extend(line_tokens);
                tokens.push(Token::Newline);
                line_numbers.push(line_num);
            }
            Err(e) => {
                return (None, vec![format!("IO error reading line {line_num}: {e}")]);
            }
        }
    }

    if tokens.is_empty() {
        return (Some(Document { items: vec![] }), vec![]);
    }

    let has_content = tokens.iter().any(|t| !matches!(t, Token::Newline));
    let chunks = split_chunks(&tokens);
    if has_content && chunks.is_empty() {
        return (None, vec!["no recognizable declarations found".to_string()]);
    }
    let mut items: Vec<Item> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (start, end) in &chunks {
        let chunk = &tokens[*start..*end];
        let start_line = line_numbers.get(*start).copied().unwrap_or(1);
        match parse_chunk(chunk, start_line) {
            Ok(chunk_items) => items.extend(chunk_items),
            Err(e) => errors.push(e),
        }
    }

    if items.is_empty() && !errors.is_empty() {
        (None, errors)
    } else {
        (Some(Document { items }), errors)
    }
}

// ---------------------------------------------------------------------------
// Structured diagnostics with byte-offset spans (for ariadne rendering)
// ---------------------------------------------------------------------------

/// A parse error carrying both the span of the failing declaration **header**
/// and (when discoverable) the span of the specific token inside the block
/// that caused the failure.  Both spans reference byte offsets in the original
/// source string, enabling rich diagnostic rendering (arrows, source snippets,
/// colour highlighting) via `ariadne`.
pub struct ParseDiagnostic {
    /// Short top-level message (e.g. `"malformed @day declaration"`).
    pub message: String,
    /// Byte span of the declaration header line (primary label).
    pub byte_span: std::ops::Range<usize>,
    /// The declaration keyword (e.g. `"@day"`).
    pub declaration_kind: &'static str,
    /// Byte span of the specific token (or expression) that caused the failure,
    /// when it can be determined.  If `None`, only the header is highlighted.
    pub note_span: Option<std::ops::Range<usize>>,
    /// Human-readable note to show at `note_span`,
    /// e.g. `"unexpected \"chickpeas\" in @day block"`.
    pub note_message: Option<String>,
}

/// Lex `source` and return the tokens paired with their byte spans.
///
/// `Error` tokens (unrecognised characters) are discarded, matching the
/// behaviour of the other `parse*` helpers in this module.
fn lex_with_spans(source: &str) -> (Vec<Token>, Vec<std::ops::Range<usize>>) {
    let mut tokens = Vec::new();
    let mut spans: Vec<std::ops::Range<usize>> = Vec::new();
    let mut lex = Token::lexer(source);
    while let Some(result) = lex.next() {
        if let Ok(tok) = result {
            spans.push(lex.span());
            tokens.push(tok);
        }
    }
    (tokens, spans)
}

/// Compute the byte span covering from the token at `tok_idx` in the chunk to
/// the end of its line (i.e. up to and including the last non-Newline token on
/// the same line).  This highlights the whole offending expression rather than
/// just its first token.
fn note_byte_span(
    chunk: &[Token],
    tok_idx: usize,
    chunk_global_start: usize,
    spans: &[std::ops::Range<usize>],
) -> Option<std::ops::Range<usize>> {
    if chunk.is_empty() {
        return None;
    }
    let clamped = tok_idx.min(chunk.len() - 1);
    let global_start_idx = chunk_global_start + clamped;
    let note_start = spans.get(global_start_idx)?.start;

    // Scan forward until we hit a Newline to find the line end.
    let last_on_line = chunk[clamped..]
        .iter()
        .enumerate()
        .take_while(|(_, t)| !matches!(t, Token::Newline))
        .last()
        .map(|(offset, _)| clamped + offset)
        .unwrap_or(clamped);

    let note_end = spans
        .get(chunk_global_start + last_on_line)
        .map(|s| s.end)
        .unwrap_or(note_start + 1);

    Some(note_start..note_end)
}

/// Scan the body of a block (tokens after `{`) for the first token that cannot
/// legitimately appear at the top level of that block type.
///
/// Returns the byte span and an explanatory message, or `None` if no
/// unexpected token is found (e.g. the error is in the header rather than
/// the body).
///
/// This scan is intentionally simple: it looks for tokens that can *start* a
/// valid body entry for the given declaration kind.  Tokens that are part of
/// an already-started entry (strings after `@ate`, numbers inside parens, etc.)
/// are skipped so only truly out-of-place tokens are flagged.
fn find_unexpected_in_body(
    chunk: &[Token],
    chunk_global_start: usize,
    spans: &[std::ops::Range<usize>],
    decl: &str,
) -> Option<(std::ops::Range<usize>, String)> {
    // Find the opening `{` of the block body.
    let body_start = chunk.iter().position(|t| matches!(t, Token::LBrace))?;
    let body = &chunk[body_start + 1..];
    if body.is_empty() {
        return None;
    }

    let mut i = 0usize;
    while i < body.len() {
        let tok = &body[i];
        let chunk_idx = body_start + 1 + i;

        let is_valid_entry_start = match decl {
            "@day" => matches!(
                tok,
                Token::AtAte
                    | Token::AtExercised
                    | Token::Newline
                    | Token::Comment(_)
                    | Token::Comma
                    | Token::RBrace
            ),
            "@ingredient" | "@food" | "@exercise" => matches!(
                tok,
                Token::Identifier(_)
                    | Token::Newline
                    | Token::Comment(_)
                    | Token::Comma
                    | Token::RBrace
            ),
            "@recipe" => matches!(
                tok,
                Token::String(_)
                    | Token::Newline
                    | Token::Comment(_)
                    | Token::Comma
                    | Token::RBrace
            ),
            _ => true,
        };

        if !is_valid_entry_start {
            let note_span = note_byte_span(chunk, chunk_idx, chunk_global_start, spans)?;
            let expected_hint = match decl {
                "@day" => "`@ate`, `@exercised`, or `}`",
                "@ingredient" | "@food" | "@exercise" => {
                    "a property name (e.g. `calories: 100`), or `}`"
                }
                "@recipe" => "an ingredient alias (e.g. `\"chickpeas\"(200g)`), or `}`",
                _ => "a valid entry, or `}`",
            };
            return Some((
                note_span,
                format!("unexpected `{tok}` in {decl} block — expected {expected_hint}"),
            ));
        }

        // Skip ahead past the current valid entry so that tokens that are
        // *part* of a valid entry are not re-inspected at the top level.
        match tok {
            Token::AtAte | Token::AtExercised => {
                i += 1; // skip @ate/@exercised keyword
                // skip alias string
                if i < body.len() && matches!(body[i], Token::String(_)) {
                    i += 1;
                }
                // skip optional (quantity)
                if i < body.len() && matches!(body[i], Token::LParen) {
                    while i < body.len() && !matches!(body[i], Token::RParen) {
                        i += 1;
                    }
                    if i < body.len() {
                        i += 1; // consume RParen
                    }
                }
            }
            Token::Identifier(_) => {
                // property: identifier colon number [unit]
                i += 1;
                if i < body.len() && matches!(body[i], Token::Colon) {
                    i += 1;
                }
                if i < body.len() && matches!(body[i], Token::Number(_)) {
                    i += 1;
                }
                if i < body.len() && matches!(body[i], Token::Identifier(_)) {
                    i += 1;
                }
            }
            Token::String(_) => {
                // ingredient label: "alias"(quantity)
                i += 1;
                if i < body.len() && matches!(body[i], Token::LParen) {
                    while i < body.len() && !matches!(body[i], Token::RParen) {
                        i += 1;
                    }
                    if i < body.len() {
                        i += 1; // consume RParen
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    None
}

/// Parse a nutrition source string, returning both the (possibly partial)
/// [`Document`] and a list of [`ParseDiagnostic`] values that carry the
/// byte-span information needed for rich diagnostic rendering.
///
/// For each failing declaration, the token body is scanned to find the first
/// token that cannot appear at the top level of that block type (e.g.
/// `"chickpeas"(1 cup)` inside a `@day` block that only accepts `@ate` and
/// `@exercised` entries).  This token's byte span is stored in
/// [`ParseDiagnostic::note_span`] so callers can render two ariadne labels:
/// one on the declaration header and one on the specific offending token.
pub fn parse_with_diagnostics(source: &str) -> (Option<Document>, Vec<ParseDiagnostic>) {
    let (tokens, spans) = lex_with_spans(source);
    if tokens.is_empty() {
        return (Some(Document { items: vec![] }), vec![]);
    }

    let has_content = tokens.iter().any(|t| !matches!(t, Token::Newline));
    let chunks = split_chunks(&tokens);

    if has_content && chunks.is_empty() {
        return (
            None,
            vec![ParseDiagnostic {
                message: "no recognizable declarations found".to_string(),
                byte_span: 0..source.len().max(1),
                declaration_kind: "declaration",
                note_span: None,
                note_message: None,
            }],
        );
    }

    let line_map = token_line_map(&tokens);
    let mut items: Vec<Item> = Vec::new();
    let mut diagnostics: Vec<ParseDiagnostic> = Vec::new();

    for (start, end) in &chunks {
        let chunk = &tokens[*start..*end];

        // Header span: first line of the declaration only.
        let byte_start = spans.get(*start).map(|s| s.start).unwrap_or(0);
        let byte_end = spans
            .get(end.saturating_sub(1))
            .map(|s| s.end)
            .unwrap_or(byte_start + 1);
        let first_newline_byte = chunk
            .iter()
            .enumerate()
            .find(|(_, t)| matches!(t, Token::Newline))
            .and_then(|(i, _)| spans.get(*start + i))
            .map(|s| s.start)
            .unwrap_or(byte_end);
        let header_span = byte_start..first_newline_byte;

        let decl = chunk
            .iter()
            .find(|t| is_top_level_start(t))
            .map(declaration_name)
            .unwrap_or("declaration");

        let start_line = line_map.get(*start).copied().unwrap_or(1);
        match parse_chunk(chunk, start_line) {
            Ok(chunk_items) => items.extend(chunk_items),
            Err(_) => {
                // Try to locate the specific offending token in the body.
                let (note_span, note_message) =
                    find_unexpected_in_body(chunk, *start, &spans, decl)
                        .map(|(s, m)| (Some(s), Some(m)))
                        .unwrap_or((None, None));

                diagnostics.push(ParseDiagnostic {
                    message: format!("malformed {} declaration", decl),
                    byte_span: header_span,
                    declaration_kind: decl,
                    note_span,
                    note_message,
                });
            }
        }
    }

    if items.is_empty() && !diagnostics.is_empty() {
        (None, diagnostics)
    } else {
        (Some(Document { items }), diagnostics)
    }
}
