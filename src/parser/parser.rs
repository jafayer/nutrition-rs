use chumsky::{prelude::*};

use crate::{lexer::lexer::Token, ast::ast::*};

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
        .then(parse_identifier().or_not())
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
        .then(
            parse_string()
                .repeated()
                .at_least(1)
                .collect()
        )
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
                .then_ignore(just(Token::RBrace))
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
                .then_ignore(just(Token::RParen))
        )
        .map(|(alias, quantity)| IngredientLabel { alias, quantity })
}

fn parse_recipe_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    just(Token::AtRecipe)
        .ignore_then(parse_quantities_in_parens())
        .then(parse_string())
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
                .then_ignore(just(Token::RBrace))
        )
        .map(|((quantities, aliases), ingredients)| {
            Item::Recipe(Recipe {
                aliases: vec![aliases],
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
        )
        .map(|(food_alias, quantity)| Ate {
            food_alias,
            quantity,
        })
}

fn parse_exercised_item<'a>() -> impl Parser<'a, &'a [Token], Exercised> + Clone {
    just(Token::AtExercised)
        .ignore_then(parse_string())
        .then(
            just(Token::LParen)
                .ignore_then(parse_quantity())
                .then_ignore(just(Token::RParen))
        )
        .map(|(exercise_alias, quantity)| Exercised {
            exercise_alias,
            quantity,
        })
}

fn parse_day_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    just(Token::AtDay)
        .ignore_then(parse_string())
        .then(
            just(Token::LBrace)
                .ignore_then(skip_block_ws())
                .ignore_then(
                    parse_ate_item()
                        .map(DayItem::Ate)
                        .or(parse_exercised_item().map(DayItem::Exercised))
                        .separated_by(block_separator())
                        .allow_trailing()
                        .collect(),
                )
                .then_ignore(skip_block_ws())
                .then_ignore(just(Token::RBrace))
        )
        .map(|(date, items)| {
            Item::Day(Day { date, items })
        })
}

fn parse_exercise_item<'a>() -> impl Parser<'a, &'a [Token], Item> + Clone {
    just(Token::AtExercise)
        .ignore_then(parse_quantities_in_parens())
        .then(
            parse_string()
                .repeated()
                .at_least(1)
                .collect()
        )
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
                .then_ignore(just(Token::RBrace))
        )
        .map(|((quantities, aliases), properties)| {
            Item::Exercise(Exercise {
                aliases,
                quantities,
                properties,
            })
        })
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
    // Top-level items can be separated by one or more newlines and inline comments
    let top_sep = any()
        .filter(|tok| matches!(tok, Token::Newline | Token::Comment(_)))
        .repeated()
        .at_least(1)
        .ignored();

    parse_item()
        .separated_by(top_sep)
        .at_least(0)
        .allow_trailing()
        .collect()
        .then_ignore(end())
        .map(|items| Document { items })
}