use chumsky::Parser as _;
use logos::Logos;
use std::io::BufRead;

use crate::ast::ast::Document;
use crate::lexer::lexer::Token;
use crate::parser::parser::parser;

/// Parse a nutrition source string into a [`Document`] using the native
/// Chumsky parser.  Returns `None` if the input cannot be parsed.
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