use clap::Parser;
use crate::ast::ast::{Ate, Day, DayItem, Exercised, Quantity};
use crate::emitters::day::DayEmitter;
use crate::emitters::emitter::CanEmit;

#[derive(Parser, Debug, Clone)]
#[clap(about = "Track day items (appends to or creates the @day block for a date)")]
pub struct TrackArgs {
    #[arg(
        short = 'd',
        long = "date",
        help = "Date to track for (e.g., '2026-01-01'). Defaults to today."
    )]
    pub date: Option<String>,

    #[arg(
        long = "meal-label",
        short = 'm',
        help = "Meal label (e.g., 'Breakfast')"
    )]
    pub meal_label: Vec<String>,

    #[arg(
        long = "ate",
        short = 'a',
        help = "Food eaten (e.g., 'Granola(100g)' or '\"Chana Masala\"(2 servings)')"
    )]
    pub ate: Vec<String>,

    #[arg(
        long = "exercised",
        short = 'e',
        help = "Exercise done (e.g., 'Running(30m)')"
    )]
    pub exercised: Vec<String>,
}

/// Parse an `@ate` item string in the format `"Alias"(quantity)` or `Alias(quantity)`.
pub fn parse_ate_item(s: &str) -> Result<DayItem, String> {
    let (alias, quantity) = parse_item_with_quantity(s)?;
    Ok(DayItem::Ate(Ate { food_alias: alias, quantity }))
}

/// Parse an `@exercised` item string in the format `"Alias"(quantity)` or `Alias(quantity)`.
pub fn parse_exercised_item(s: &str) -> Result<DayItem, String> {
    let (alias, quantity) = parse_item_with_quantity(s)?;
    Ok(DayItem::Exercised(Exercised { exercise_alias: alias, quantity }))
}

fn parse_item_with_quantity(s: &str) -> Result<(String, Quantity), String> {
    let s = s.trim();
    if let Some((alias_part, rest)) = s.split_once('(') {
        let alias = alias_part.trim().trim_matches('"').to_string();
        let quantity_str = rest.trim_end_matches(')').trim();
        let quantity = Quantity::from_string(quantity_str)
            .map_err(|e| format!("Invalid quantity in '{}': {}", s, e))?;
        Ok((alias, quantity))
    } else {
        Err(format!(
            "Invalid item format '{}': expected 'Alias(quantity)'",
            s
        ))
    }
}

/// Reconstruct ordered `DayItem`s from a raw args slice.
///
/// Scans `raw_args` left-to-right and produces items in the order the flags
/// appear.  Handles both `--flag value` and `--flag=value` forms.  Unknown
/// flags and positional arguments are silently skipped.
pub fn build_ordered_items<S: AsRef<str>>(raw_args: &[S]) -> Result<Vec<DayItem>, String> {
    let mut items = Vec::new();
    let mut i = 0;
    while i < raw_args.len() {
        let arg = raw_args[i].as_ref();
        if arg == "--meal-label" || arg == "-m" {
            i += 1;
            if let Some(value) = raw_args.get(i) {
                items.push(DayItem::Meal(value.as_ref().to_string()));
            } else {
                return Err("--meal-label requires a value".to_string());
            }
        } else if arg == "--ate" || arg == "-a" {
            i += 1;
            if let Some(value) = raw_args.get(i) {
                items.push(parse_ate_item(value.as_ref())?);
            } else {
                return Err("--ate requires a value".to_string());
            }
        } else if arg == "--exercised" || arg == "-e" {
            i += 1;
            if let Some(value) = raw_args.get(i) {
                items.push(parse_exercised_item(value.as_ref())?);
            } else {
                return Err("--exercised requires a value".to_string());
            }
        } else if let Some(value) = arg.strip_prefix("--meal-label=") {
            items.push(DayItem::Meal(value.to_string()));
        } else if let Some(value) = arg.strip_prefix("--ate=") {
            items.push(parse_ate_item(value)?);
        } else if let Some(value) = arg.strip_prefix("--exercised=") {
            items.push(parse_exercised_item(value)?);
        }
        i += 1;
    }
    Ok(items)
}

/// Emit `DayItem`s as indented markup lines (no surrounding block).
fn emit_day_items(items: &[DayItem]) -> String {
    let mut output = String::new();
    for item in items {
        match item {
            DayItem::Meal(label) => {
                output.push_str(&format!("    [{}]\n", label));
            }
            DayItem::Ate(ate) => {
                output.push_str(&format!(
                    "    @ate \"{}\"({})\n",
                    ate.food_alias,
                    ate.quantity.to_string()
                ));
            }
            DayItem::Exercised(ex) => {
                output.push_str(&format!(
                    "    @exercised \"{}\"({})\n",
                    ex.exercise_alias,
                    ex.quantity.to_string()
                ));
            }
        }
    }
    output
}

/// Apply tracked items to `content`.
///
/// - If a `@day "date"` block already exists, new items are appended to it
///   in-place (before the closing `}`).
/// - If no block exists for `date`, a new `@day` block is appended to the end
///   of the content.
pub fn apply_track_to_content(content: &str, date: &str, items: &[DayItem]) -> String {
    if items.is_empty() {
        return content.to_string();
    }

    let search = format!("@day \"{}\"", date);

    if let Some(day_start) = content.find(&search) {
        if let Some(brace_offset) = content[day_start..].find('{') {
            let brace_abs = day_start + brace_offset;
            // Walk brace depth to find the matching closing brace.
            // @day blocks contain no nested `{` in their item entries, but we
            // use depth counting to be safe.
            let mut depth = 0usize;
            let mut close_abs: Option<usize> = None;
            for (offset, c) in content[brace_abs..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            close_abs = Some(brace_abs + offset);
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(close) = close_abs {
                let items_text = emit_day_items(items);
                let mut result = String::new();
                result.push_str(&content[..close]);
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                result.push_str(&items_text);
                result.push_str(&content[close..]);
                return result;
            }
        }
    }

    // No existing block for this date – append a new one.
    let block = DayEmitter.emit(&Day {
        date: date.to_string(),
        items: items.to_vec(),
    });

    let mut result = content.to_string();
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    if !result.is_empty() {
        result.push('\n');
    }
    result.push_str(&block);
    result
}

/// Run the track command with injectable I/O.
///
/// `read_fn` is called to obtain the current file content; `write_fn` is
/// called with the updated content.  This design allows tests to supply
/// in-memory closures without touching the filesystem.
pub fn run_track_with_io<R, W>(
    read_fn: R,
    write_fn: W,
    date: &str,
    items: &[DayItem],
) -> Result<(), String>
where
    R: FnOnce() -> Result<String, String>,
    W: FnOnce(String) -> Result<(), String>,
{
    let content = read_fn()?;
    let updated = apply_track_to_content(&content, date, items);
    write_fn(updated)
}

/// Run the track command on a real file.
pub fn run_track_on_file(file_path: &str, date: &str, items: &[DayItem]) -> Result<(), String> {
    let path = file_path.to_string();
    run_track_with_io(
        || std::fs::read_to_string(&path).map_err(|e| format!("Failed to read '{}': {}", path, e)),
        |content| {
            std::fs::write(&path, content)
                .map_err(|e| format!("Failed to write '{}': {}", path, e))
        },
        date,
        items,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parser::parse;

    // -----------------------------------------------------------------------
    // parse_ate_item
    // -----------------------------------------------------------------------

    #[test]
    fn parse_ate_item_with_unquoted_alias_and_quantity() {
        let result = parse_ate_item("Granola(100g)").expect("should parse");
        match result {
            DayItem::Ate(ate) => {
                assert_eq!(ate.food_alias, "Granola");
                assert_eq!(ate.quantity.amount, 100.0);
                assert_eq!(ate.quantity.unit.as_deref(), Some("g"));
            }
            _ => panic!("expected Ate"),
        }
    }

    #[test]
    fn parse_ate_item_with_quoted_alias_and_quantity() {
        let result =
            parse_ate_item("\"Chana Masala\"(2 servings)").expect("should parse");
        match result {
            DayItem::Ate(ate) => {
                assert_eq!(ate.food_alias, "Chana Masala");
                assert_eq!(ate.quantity.amount, 2.0);
                assert_eq!(ate.quantity.unit.as_deref(), Some("servings"));
            }
            _ => panic!("expected Ate"),
        }
    }

    #[test]
    fn parse_ate_item_without_parens_returns_error() {
        let err = parse_ate_item("Granola")
            .expect_err("should error for missing quantity parens");
        assert!(
            err.contains("expected 'Alias(quantity)'"),
            "error was: {err}"
        );
    }

    #[test]
    fn parse_ate_item_with_invalid_quantity_returns_error() {
        let err = parse_ate_item("Food(notanumber)")
            .expect_err("should error for invalid quantity");
        assert!(err.contains("Invalid quantity"), "error was: {err}");
    }

    // -----------------------------------------------------------------------
    // parse_exercised_item
    // -----------------------------------------------------------------------

    #[test]
    fn parse_exercised_item_with_unquoted_alias_and_quantity() {
        let result = parse_exercised_item("Running(30m)").expect("should parse");
        match result {
            DayItem::Exercised(ex) => {
                assert_eq!(ex.exercise_alias, "Running");
                assert_eq!(ex.quantity.amount, 30.0);
                assert_eq!(ex.quantity.unit.as_deref(), Some("m"));
            }
            _ => panic!("expected Exercised"),
        }
    }

    #[test]
    fn parse_exercised_item_with_quoted_alias_and_quantity() {
        let result =
            parse_exercised_item("\"Weight Training\"(45 minutes)").expect("should parse");
        match result {
            DayItem::Exercised(ex) => {
                assert_eq!(ex.exercise_alias, "Weight Training");
                assert_eq!(ex.quantity.amount, 45.0);
                assert_eq!(ex.quantity.unit.as_deref(), Some("minutes"));
            }
            _ => panic!("expected Exercised"),
        }
    }

    #[test]
    fn parse_exercised_item_without_parens_returns_error() {
        let err = parse_exercised_item("Running")
            .expect_err("should error for missing quantity parens");
        assert!(
            err.contains("expected 'Alias(quantity)'"),
            "error was: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // build_ordered_items
    // -----------------------------------------------------------------------

    #[test]
    fn build_ordered_items_empty_args_returns_empty() {
        let items = build_ordered_items::<&str>(&[]).expect("should not error");
        assert!(items.is_empty());
    }

    #[test]
    fn build_ordered_items_single_meal_label() {
        let items =
            build_ordered_items(&["--meal-label", "Breakfast"]).expect("should parse");
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], DayItem::Meal(l) if l == "Breakfast"));
    }

    #[test]
    fn build_ordered_items_single_ate() {
        let items =
            build_ordered_items(&["--ate", "Granola(100g)"]).expect("should parse");
        assert_eq!(items.len(), 1);
        match &items[0] {
            DayItem::Ate(ate) => {
                assert_eq!(ate.food_alias, "Granola");
                assert_eq!(ate.quantity.amount, 100.0);
            }
            _ => panic!("expected Ate"),
        }
    }

    #[test]
    fn build_ordered_items_single_exercised() {
        let items =
            build_ordered_items(&["--exercised", "Running(30m)"]).expect("should parse");
        assert_eq!(items.len(), 1);
        match &items[0] {
            DayItem::Exercised(ex) => assert_eq!(ex.exercise_alias, "Running"),
            _ => panic!("expected Exercised"),
        }
    }

    #[test]
    fn build_ordered_items_preserves_meal_then_ate_order() {
        let items = build_ordered_items(&[
            "--meal-label", "Breakfast",
            "--ate", "Granola(100g)",
        ])
        .expect("should parse");
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], DayItem::Meal(_)));
        assert!(matches!(&items[1], DayItem::Ate(_)));
    }

    #[test]
    fn build_ordered_items_preserves_ate_then_meal_order() {
        let items = build_ordered_items(&[
            "--ate", "Granola(100g)",
            "--meal-label", "Lunch",
        ])
        .expect("should parse");
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], DayItem::Ate(_)));
        assert!(matches!(&items[1], DayItem::Meal(_)));
    }

    #[test]
    fn build_ordered_items_skips_unrecognized_flags_and_values() {
        let items = build_ordered_items(&[
            "track",
            "--date", "2026-01-01",
            "--file", "/path/to/file",
            "--meal-label", "Breakfast",
        ])
        .expect("should parse");
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], DayItem::Meal(l) if l == "Breakfast"));
    }

    #[test]
    fn build_ordered_items_handles_equals_form() {
        let items =
            build_ordered_items(&["--meal-label=Breakfast", "--ate=Granola(100g)"])
                .expect("should parse");
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], DayItem::Meal(l) if l == "Breakfast"));
        assert!(matches!(&items[1], DayItem::Ate(_)));
    }

    #[test]
    fn build_ordered_items_returns_error_for_invalid_ate() {
        let err = build_ordered_items(&["--ate", "Granola"])
            .expect_err("should error for missing qty parens");
        assert!(
            err.contains("expected 'Alias(quantity)'"),
            "error was: {err}"
        );
    }

    #[test]
    fn build_ordered_items_returns_error_for_invalid_exercised() {
        let err = build_ordered_items(&["--exercised", "Running"])
            .expect_err("should error");
        assert!(
            err.contains("expected 'Alias(quantity)'"),
            "error was: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // apply_track_to_content
    // -----------------------------------------------------------------------

    fn ate(alias: &str, qty: &str) -> DayItem {
        DayItem::Ate(Ate {
            food_alias: alias.to_string(),
            quantity: Quantity::from_string(qty).unwrap(),
        })
    }

    fn exercised(alias: &str, qty: &str) -> DayItem {
        DayItem::Exercised(Exercised {
            exercise_alias: alias.to_string(),
            quantity: Quantity::from_string(qty).unwrap(),
        })
    }

    fn meal(label: &str) -> DayItem {
        DayItem::Meal(label.to_string())
    }

    #[test]
    fn apply_track_to_empty_content_creates_day_block() {
        let items = vec![meal("Breakfast"), ate("Granola", "100g")];
        let result = apply_track_to_content("", "2026-01-01", &items);
        assert!(result.contains("@day \"2026-01-01\""));
        assert!(result.contains("[Breakfast]"));
        assert!(result.contains("@ate \"Granola\"(100g)"));
    }

    #[test]
    fn apply_track_appends_new_block_when_date_not_found() {
        let content =
            "@day \"2025-12-31\" {\n    [Dinner]\n    @ate \"Pizza\"(1 slice)\n}\n";
        let items = vec![meal("Breakfast"), ate("Granola", "100g")];
        let result = apply_track_to_content(content, "2026-01-01", &items);
        assert!(result.contains("@day \"2025-12-31\""));
        assert!(result.contains("@day \"2026-01-01\""));
        assert!(result.contains("[Breakfast]"));
    }

    #[test]
    fn apply_track_updates_existing_block_in_place() {
        let content =
            "@day \"2026-01-01\" {\n    [Breakfast]\n    @ate \"Oatmeal\"(1 cup)\n}\n";
        let items = vec![meal("Lunch"), ate("Sandwich", "1")];
        let result = apply_track_to_content(content, "2026-01-01", &items);
        // Exactly one block for this date
        assert_eq!(result.matches("@day \"2026-01-01\"").count(), 1);
        // Pre-existing content preserved
        assert!(result.contains("[Breakfast]"));
        assert!(result.contains("@ate \"Oatmeal\"(1 cup)"));
        // New items present
        assert!(result.contains("[Lunch]"));
        assert!(result.contains("@ate \"Sandwich\"(1)"));
    }

    #[test]
    fn apply_track_in_place_leaves_other_blocks_unchanged() {
        let content = concat!(
            "@day \"2026-01-01\" {\n    [Breakfast]\n}\n\n",
            "@day \"2026-01-02\" {\n    [Breakfast]\n}\n"
        );
        let items = vec![ate("Oatmeal", "1 cup")];
        let result = apply_track_to_content(content, "2026-01-01", &items);
        // Inserted item appears before second block
        let ate_pos = result.find("@ate \"Oatmeal\"").unwrap();
        let second_block_pos = result.find("@day \"2026-01-02\"").unwrap();
        assert!(ate_pos < second_block_pos);
        // Second block unchanged
        assert_eq!(result.matches("@day \"2026-01-02\"").count(), 1);
    }

    #[test]
    fn apply_track_with_no_items_is_a_no_op() {
        let content = "@day \"2026-01-01\" {\n    [Breakfast]\n}\n";
        let result = apply_track_to_content(content, "2026-01-01", &[]);
        assert_eq!(result, content);
    }

    #[test]
    fn apply_track_to_new_content_produces_parseable_markup() {
        let items = vec![meal("Breakfast"), ate("Granola", "100g"), exercised("Running", "30m")];
        let result = apply_track_to_content("", "2026-01-01", &items);
        parse(&result).expect("result should be valid markup");
    }

    #[test]
    fn apply_track_in_place_produces_parseable_markup_with_correct_item_count() {
        let content =
            "@day \"2026-01-01\" {\n    [Breakfast]\n}\n";
        let items = vec![meal("Lunch"), ate("Sandwich", "1")];
        let result = apply_track_to_content(content, "2026-01-01", &items);
        let doc = parse(&result).expect("result should be valid markup after update");
        assert_eq!(doc.items.len(), 1);
        match &doc.items[0] {
            crate::ast::ast::Item::Day(day) => {
                // [Breakfast] + [Lunch] + Sandwich
                assert_eq!(day.items.len(), 3);
            }
            _ => panic!("expected Day item"),
        }
    }

    // -----------------------------------------------------------------------
    // run_track_with_io
    // -----------------------------------------------------------------------

    #[test]
    fn run_track_with_io_passes_updated_content_to_write_fn() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let written = Rc::new(RefCell::new(String::new()));
        let written_clone = Rc::clone(&written);

        let items = vec![meal("Breakfast"), ate("Granola", "100g")];
        run_track_with_io(
            || Ok("".to_string()),
            |content| {
                *written_clone.borrow_mut() = content;
                Ok(())
            },
            "2026-01-01",
            &items,
        )
        .expect("should succeed");

        let result = written.borrow().clone();
        assert!(result.contains("@day \"2026-01-01\""));
        assert!(result.contains("[Breakfast]"));
    }

    #[test]
    fn run_track_with_io_propagates_read_error() {
        let items = vec![meal("Breakfast")];
        let err = run_track_with_io(
            || Err("file not found".to_string()),
            |_| Ok(()),
            "2026-01-01",
            &items,
        )
        .expect_err("should propagate read error");
        assert!(err.contains("file not found"));
    }

    #[test]
    fn run_track_with_io_propagates_write_error() {
        let items = vec![meal("Breakfast")];
        let err = run_track_with_io(
            || Ok("".to_string()),
            |_| Err("permission denied".to_string()),
            "2026-01-01",
            &items,
        )
        .expect_err("should propagate write error");
        assert!(err.contains("permission denied"));
    }
}
