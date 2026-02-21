use chumsky::Parser as _;
use logos::Logos;

use crate::ast::ast::Document;
use crate::lexer::lexer::Token;
use crate::parser::parser::parser;

/// Parse a nutrition source string into a [`Document`] using the native
/// Chumsky parser.  Returns `None` if the input cannot be parsed.
pub fn parse(source: &str) -> Option<Document> {
    let tokens: Vec<Token> = Token::lexer(source).filter_map(Result::ok).collect();
    parser().parse(tokens.as_slice()).into_output()
}