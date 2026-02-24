use logos::Logos;
use std::fmt;
use std::ops::Range;

pub type Span = Range<usize>;
pub type SpannedToken = (Token, Span);

fn unescape_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }

    out
}

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    #[token("@unit")]
    AtUnit,
    #[token("@property")]
    AtProperty,
    #[token("@ingredient")]
    AtIngredient,
    #[token("@food")]
    AtFood,
    #[token("@recipe")]
    AtRecipe,
    #[token("@exercise")]
    AtExercise,
    #[token("@day")]
    AtDay,
    #[token("@ate")]
    AtAte,
    #[token("@exercised")]
    AtExercised,

     // Punctuation
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token(":")] Colon,
    #[token(",")] Comma,
    #[token("=")] Equals,

    // Literals
    #[regex(r#""([^"\\]|\\.)*""#, |lex| unescape_string_literal(&lex.slice()[1..lex.slice().len()-1]))]
    String(String),

    #[regex(r"[0-9]+\.[0-9]+|[0-9]+|\.[0-9]+", |lex| lex.slice().parse::<f64>().unwrap())]
    Number(f64),

    #[regex(r"true|false|True|False", |lex| lex.slice().to_lowercase() == "true")]
    Bool(bool),

    // Meal label: [Breakfast] - put before Identifier to ensure priority
    #[regex(r"\[[^\]\n]+\]", |lex| lex.slice().to_string())]
    MealLabel(String),

    #[regex(r"[A-Za-z_][A-Za-z0-9_\-]*", |lex| lex.slice().to_string())]
    Identifier(String),

    // Comment
    #[regex(r"//[^\n]*", |lex| lex.slice().to_string(), allow_greedy = true)]
    Comment(String),

    #[regex(r"\n")]
    Newline,

    #[regex(r"[ \t\r]+", logos::skip)]
    Error,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::AtUnit => write!(f, "@unit"),
            Token::AtProperty => write!(f, "@property"),
            Token::AtIngredient => write!(f, "@ingredient"),
            Token::AtFood => write!(f, "@food"),
            Token::AtRecipe => write!(f, "@recipe"),
            Token::AtExercise => write!(f, "@exercise"),
            Token::AtDay => write!(f, "@day"),
            Token::AtAte => write!(f, "@ate"),
            Token::AtExercised => write!(f, "@exercised"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Equals => write!(f, "="),
            Token::String(s) => write!(f, "\"{}\"", s),
            Token::Number(n) => write!(f, "{}", n),
            Token::Bool(b) => write!(f, "{}", b),
            Token::Identifier(id) => write!(f, "{}", id),
            Token::MealLabel(l) => write!(f, "{}", l),
            // Comment tokens store the full `//…` text including the leading `//`.
            Token::Comment(c) => write!(f, "{}", c),
            Token::Newline => write!(f, "<newline>"),
            Token::Error => write!(f, "<error>"),
        }
    }
}