use chumsky::Parser;
use logos::Logos;

use nutrition_rs::ast::ast::{DayItem, Document, Item};
use nutrition_rs::lexer::lexer::Token;
use nutrition_rs::parser::parser::parser;

fn lex(input: &str) -> Vec<Token> {
    Token::lexer(input).filter_map(Result::ok).collect()
}

fn parse_document(input: &str) -> Document {
    let tokens = lex(input);
    parser()
        .parse(tokens.as_slice())
        .into_output()
        .expect("parser returned an error")
}

// ---------------------------------------------------------------------------
// Empty @day blocks
// ---------------------------------------------------------------------------

#[test]
fn parses_empty_day_block() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            assert_eq!(day.items.len(), 0);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_day_block_with_only_newlines() {
    let doc = parse_document(
        r#"@day "2026-01-01" {


}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            assert_eq!(day.items.len(), 0);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_day_block_with_only_comment() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  // just a comment
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            // Comments inside @day blocks are consumed by block_separator, not stored as items
            assert_eq!(day.items.len(), 0);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_day_block_with_multiple_comments() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  // comment 1
  // comment 2
  // comment 3
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            assert_eq!(day.items.len(), 0);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Meal labels only
// ---------------------------------------------------------------------------

#[test]
fn parses_day_block_with_single_meal_label() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Meal(label) => assert_eq!(label, "Breakfast"),
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_day_block_with_multiple_meal_labels() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  [Lunch]
  [Dinner]
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            assert_eq!(day.items.len(), 3);
            match &day.items[0] {
                DayItem::Meal(label) => assert_eq!(label, "Breakfast"),
                other => panic!("unexpected day item: {other:?}"),
            }
            match &day.items[1] {
                DayItem::Meal(label) => assert_eq!(label, "Lunch"),
                other => panic!("unexpected day item: {other:?}"),
            }
            match &day.items[2] {
                DayItem::Meal(label) => assert_eq!(label, "Dinner"),
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_meal_label_with_spaces() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Early Morning Snack]
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Meal(label) => assert_eq!(label, "Early Morning Snack"),
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Meal labels with comments
// ---------------------------------------------------------------------------

#[test]
fn parses_meal_label_followed_by_comment() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  // no breakfast today
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Meal(label) => assert_eq!(label, "Breakfast"),
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_comment_before_meal_label() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  // starting the day
  [Breakfast]
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Meal(label) => assert_eq!(label, "Breakfast"),
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_multiple_meal_labels_with_comments_between() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  // transition to lunch
  [Lunch]
  // and dinner
  [Dinner]
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 3);
            match &day.items[0] {
                DayItem::Meal(label) => assert_eq!(label, "Breakfast"),
                other => panic!("unexpected day item: {other:?}"),
            }
            match &day.items[1] {
                DayItem::Meal(label) => assert_eq!(label, "Lunch"),
                other => panic!("unexpected day item: {other:?}"),
            }
            match &day.items[2] {
                DayItem::Meal(label) => assert_eq!(label, "Dinner"),
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// @ate entries
// ---------------------------------------------------------------------------

#[test]
fn parses_single_ate_entry() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @ate "oatmeal"(1 cup)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Ate(ate) => {
                    assert_eq!(ate.food_alias, "oatmeal");
                    assert_eq!(ate.quantity.amount, 1.0);
                    assert_eq!(ate.quantity.unit.as_deref(), Some("cup"));
                }
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_multiple_ate_entries() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @ate "oatmeal"(1 cup)
  @ate "banana"(1)
  @ate "coffee"(2 cups)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 3);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_ate_with_multi_word_unit() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @ate "veggie nugs"(2 pieces)
  @ate "ginger beer"(12 fl oz)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 2);
            match &day.items[0] {
                DayItem::Ate(ate) => {
                    assert_eq!(ate.food_alias, "veggie nugs");
                    assert_eq!(ate.quantity.amount, 2.0);
                    assert_eq!(ate.quantity.unit.as_deref(), Some("pieces"));
                }
                other => panic!("unexpected day item: {other:?}"),
            }
            match &day.items[1] {
                DayItem::Ate(ate) => {
                    assert_eq!(ate.food_alias, "ginger beer");
                    assert_eq!(ate.quantity.amount, 12.0);
                    assert_eq!(ate.quantity.unit.as_deref(), Some("fl oz"));
                }
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// @exercised entries
// ---------------------------------------------------------------------------

#[test]
fn parses_single_exercised_entry() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @exercised "running"(30 minutes)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Exercised(exercised) => {
                    assert_eq!(exercised.exercise_alias, "running");
                    assert_eq!(exercised.quantity.amount, 30.0);
                    assert_eq!(exercised.quantity.unit.as_deref(), Some("minutes"));
                }
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_multiple_exercised_entries() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @exercised "running"(30 minutes)
  @exercised "yoga"(15 minutes)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 2);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Combinations of meal labels and @ate
// ---------------------------------------------------------------------------

#[test]
fn parses_meal_label_with_ate_entries() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  @ate "oatmeal"(1 cup)
  @ate "banana"(1)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 3);
            match &day.items[0] {
                DayItem::Meal(label) => assert_eq!(label, "Breakfast"),
                other => panic!("unexpected day item: {other:?}"),
            }
            match &day.items[1] {
                DayItem::Ate(_) => {}
                other => panic!("expected Ate, got: {other:?}"),
            }
            match &day.items[2] {
                DayItem::Ate(_) => {}
                other => panic!("expected Ate, got: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_multiple_meal_labels_with_ate_entries() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  @ate "oatmeal"(1 cup)
  [Lunch]
  @ate "sandwich"(1)
  [Dinner]
  @ate "pasta"(1 bowl)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 6);
            assert!(matches!(day.items[0], DayItem::Meal(_)));
            assert!(matches!(day.items[1], DayItem::Ate(_)));
            assert!(matches!(day.items[2], DayItem::Meal(_)));
            assert!(matches!(day.items[3], DayItem::Ate(_)));
            assert!(matches!(day.items[4], DayItem::Meal(_)));
            assert!(matches!(day.items[5], DayItem::Ate(_)));
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Combinations with comments
// ---------------------------------------------------------------------------

#[test]
fn parses_meal_label_with_comment_then_ate() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  // had a light breakfast
  @ate "toast"(2 slices)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 2);
            assert!(matches!(day.items[0], DayItem::Meal(_)));
            assert!(matches!(day.items[1], DayItem::Ate(_)));
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_comment_after_ate_before_meal_label() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @ate "oatmeal"(1 cup)
  // moving to lunch
  [Lunch]
  @ate "sandwich"(1)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 3);
            assert!(matches!(day.items[0], DayItem::Ate(_)));
            assert!(matches!(day.items[1], DayItem::Meal(_)));
            assert!(matches!(day.items[2], DayItem::Ate(_)));
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_meal_label_with_comment_and_no_ate() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  // skipped breakfast
  [Lunch]
  @ate "salad"(1 bowl)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 3);
            assert!(matches!(day.items[0], DayItem::Meal(_)));
            assert!(matches!(day.items[1], DayItem::Meal(_)));
            assert!(matches!(day.items[2], DayItem::Ate(_)));
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Combinations with @exercised
// ---------------------------------------------------------------------------

#[test]
fn parses_meal_label_with_exercised() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Morning]
  @exercised "running"(30 minutes)
  [Breakfast]
  @ate "oatmeal"(1 cup)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 4);
            assert!(matches!(day.items[0], DayItem::Meal(_)));
            assert!(matches!(day.items[1], DayItem::Exercised(_)));
            assert!(matches!(day.items[2], DayItem::Meal(_)));
            assert!(matches!(day.items[3], DayItem::Ate(_)));
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_ate_and_exercised_mixed() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @ate "breakfast"(1)
  @exercised "running"(30 minutes)
  @ate "lunch"(1)
  @exercised "yoga"(15 minutes)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 4);
            assert!(matches!(day.items[0], DayItem::Ate(_)));
            assert!(matches!(day.items[1], DayItem::Exercised(_)));
            assert!(matches!(day.items[2], DayItem::Ate(_)));
            assert!(matches!(day.items[3], DayItem::Exercised(_)));
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Complex combinations
// ---------------------------------------------------------------------------

#[test]
fn parses_complex_day_with_all_elements() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  // morning routine
  [Morning Exercise]
  @exercised "running"(30 minutes)
  // breakfast time
  [Breakfast]
  @ate "oatmeal"(1 cup)
  @ate "banana"(1)
  // midday
  [Lunch]
  @ate "sandwich"(1)
  @ate "apple"(1)
  // afternoon workout
  [Afternoon]
  @exercised "yoga"(20 minutes)
  // dinner
  [Dinner]
  @ate "pasta"(1 bowl)
  @ate "salad"(1 bowl)
  // evening snack
  [Snack]
  @ate "nuts"(0.25 cup)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            // 5 meal labels + 2 exercised + 8 ate = 15 items
            assert_eq!(day.items.len(), 15);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_meal_label_empty_then_another_with_ate() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  // skipped
  [Lunch]
  @ate "sandwich"(1)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 3);
            assert!(matches!(day.items[0], DayItem::Meal(_)));
            assert!(matches!(day.items[1], DayItem::Meal(_)));
            assert!(matches!(day.items[2], DayItem::Ate(_)));
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_multiple_comments_between_items() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  // comment 1
  // comment 2
  @ate "oatmeal"(1 cup)
  // comment 3
  // comment 4
  // comment 5
  [Lunch]
  @ate "sandwich"(1)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 4);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_ate_without_explicit_quantity() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @ate "pizza"
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Ate(ate) => {
                    assert_eq!(ate.food_alias, "pizza");
                    // Default quantity is 1.0 with no unit
                    assert_eq!(ate.quantity.amount, 1.0);
                    assert_eq!(ate.quantity.unit, None);
                }
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_exercised_without_explicit_quantity() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @exercised "running"
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 1);
            match &day.items[0] {
                DayItem::Exercised(exercised) => {
                    assert_eq!(exercised.exercise_alias, "running");
                    assert_eq!(exercised.quantity.amount, 1.0);
                    assert_eq!(exercised.quantity.unit, None);
                }
                other => panic!("unexpected day item: {other:?}"),
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Edge cases and stress tests
// ---------------------------------------------------------------------------

#[test]
fn parses_meal_labels_back_to_back() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  [Lunch]
  [Dinner]
  [Snack]
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 4);
            for item in &day.items {
                assert!(matches!(item, DayItem::Meal(_)));
            }
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_comments_only_between_all_items() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  // start
  [Breakfast]
  // between
  @ate "oatmeal"(1 cup)
  // more
  @exercised "running"(30 minutes)
  // end
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 3);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_many_ate_entries_in_one_meal() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  [Breakfast]
  @ate "oatmeal"(1 cup)
  @ate "banana"(1)
  @ate "coffee"(2 cups)
  @ate "orange juice"(1 cup)
  @ate "toast"(2 slices)
  @ate "butter"(1 tbsp)
  @ate "jam"(1 tbsp)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 8); // 1 meal label + 7 ate entries
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}

#[test]
fn parses_interleaved_everything() {
    let doc = parse_document(
        r#"@day "2026-01-01" {
  @ate "early snack"(1)
  // comment
  [Breakfast]
  @ate "oatmeal"(1 cup)
  @exercised "stretching"(10 minutes)
  // another comment
  @ate "banana"(1)
  [Lunch]
  // skipped lunch
  [Afternoon]
  @exercised "running"(30 minutes)
  // comment
  [Dinner]
  @ate "pasta"(1 bowl)
  @ate "salad"(1 bowl)
  @exercised "walking"(15 minutes)
}"#,
    );

    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert!(day.items.len() >= 10);
        }
        other => panic!("unexpected item parsed: {other:?}"),
    }
}
