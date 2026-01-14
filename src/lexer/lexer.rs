use logos::Logos;
use std::ops::Range;

pub type Span = Range<usize>;
pub type SpannedToken = (Token, Span);

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
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    String(String),

    #[regex(r"[0-9]+\.[0-9]+|[0-9]+|\.[0-9]+", |lex| lex.slice().parse::<f64>().unwrap())]
    Number(f64),

    #[regex(r"true|false|True|False", |lex| lex.slice().to_lowercase() == "true")]
    Bool(bool),

    #[regex(r"[A-Za-z_][A-Za-z0-9_\-]*", |lex| lex.slice().to_string())]
    Identifier(String),

    // Meal label: [Breakfast]
    #[regex(r"\[[^\]\n]+\]", |lex| lex.slice().to_string())]
    MealLabel(String),

    // Comment
    #[regex(r"//[^\n]*", |lex| lex.slice().to_string(), allow_greedy = true)]
    Comment(String),

    #[regex(r"\n")]
    Newline,

    #[regex(r"[ \t\r]+", logos::skip)]
    Error,
}

// impl Token {
//     pub fn lex_with_spans(src: &str) -> Vec<SpannedToken> {
//         let mut lex = Token::lexer(src);
//         let mut out = Vec::new();
//         while let Some(tok) = lex.next() {
//             match tok {
//                 Ok(t) => {
//                     let span = lex.span(); // byte range in `src`
//                     out.push((t, span));
//                 }
//                 Err(_) => {
//                     // Skip errors for now
//                     continue;
//                 }
//             }
//         }
//         out
//     }
// }