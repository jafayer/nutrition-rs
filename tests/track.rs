use std::cell::RefCell;
use std::rc::Rc;

use nutrition_rs::ast::ast::{Ate, DayItem, Exercised, Item, Quantity};
use nutrition_rs::cli::track::{apply_track_to_content, build_ordered_items, run_track_with_io};
use nutrition_rs::parser::parser::parse;

fn make_ate(alias: &str, qty: &str) -> DayItem {
    DayItem::Ate(Ate {
        food_alias: alias.to_string(),
        quantity: Quantity::from_string(qty).unwrap(),
    })
}

fn make_exercised(alias: &str, qty: &str) -> DayItem {
    DayItem::Exercised(Exercised {
        exercise_alias: alias.to_string(),
        quantity: Quantity::from_string(qty).unwrap(),
    })
}

fn make_meal(label: &str) -> DayItem {
    DayItem::Meal(label.to_string())
}

// ---------------------------------------------------------------------------
// New day block creation
// ---------------------------------------------------------------------------

#[test]
fn track_creates_day_block_when_file_has_no_matching_date() {
    let items = vec![make_meal("Breakfast"), make_ate("Granola", "100g")];
    let result = apply_track_to_content("", "2026-01-01", &items);
    let doc = parse(&result).expect("result should be parseable");
    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            assert_eq!(day.items.len(), 2);
        }
        _ => panic!("expected Day"),
    }
}

#[test]
fn track_appends_block_after_existing_content() {
    let existing = "@day \"2025-12-31\" {\n    [Dinner]\n    @ate \"Pizza\"(1 slice)\n}\n";
    let items = vec![make_meal("Breakfast"), make_ate("Granola", "100g")];
    let result = apply_track_to_content(existing, "2026-01-01", &items);
    let doc = parse(&result).expect("result should be parseable");
    // Both day blocks present
    assert_eq!(doc.items.len(), 2);
    match &doc.items[1] {
        Item::Day(day) => assert_eq!(day.date, "2026-01-01"),
        _ => panic!("expected second Day"),
    }
}

// ---------------------------------------------------------------------------
// In-place update of existing day block
// ---------------------------------------------------------------------------

#[test]
fn track_updates_existing_day_block_in_place() {
    let content =
        "@day \"2026-01-01\" {\n    [Breakfast]\n    @ate \"Oatmeal\"(1 cup)\n}\n";
    let items = vec![make_meal("Lunch"), make_ate("Sandwich", "1")];
    let result = apply_track_to_content(content, "2026-01-01", &items);

    // Exactly one block for this date
    assert_eq!(result.matches("@day \"2026-01-01\"").count(), 1);

    let doc = parse(&result).expect("updated content should be parseable");
    assert_eq!(doc.items.len(), 1);
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            // [Breakfast] + Oatmeal + [Lunch] + Sandwich
            assert_eq!(day.items.len(), 4);
            assert!(matches!(&day.items[0], DayItem::Meal(l) if l == "Breakfast"));
            assert!(matches!(&day.items[1], DayItem::Ate(a) if a.food_alias == "Oatmeal"));
            assert!(matches!(&day.items[2], DayItem::Meal(l) if l == "Lunch"));
            assert!(matches!(&day.items[3], DayItem::Ate(a) if a.food_alias == "Sandwich"));
        }
        _ => panic!("expected Day"),
    }
}

#[test]
fn track_in_place_update_does_not_affect_other_day_blocks() {
    let content = concat!(
        "@day \"2026-01-01\" {\n    [Breakfast]\n}\n\n",
        "@day \"2026-01-02\" {\n    [Breakfast]\n}\n"
    );
    let items = vec![make_ate("Granola", "100g")];
    let result = apply_track_to_content(content, "2026-01-01", &items);

    let doc = parse(&result).expect("result should be parseable");
    assert_eq!(doc.items.len(), 2);

    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-01");
            assert_eq!(day.items.len(), 2); // [Breakfast] + Granola
        }
        _ => panic!("expected Day at index 0"),
    }
    match &doc.items[1] {
        Item::Day(day) => {
            assert_eq!(day.date, "2026-01-02");
            assert_eq!(day.items.len(), 1); // unchanged
        }
        _ => panic!("expected Day at index 1"),
    }
}

// ---------------------------------------------------------------------------
// Item ordering is preserved
// ---------------------------------------------------------------------------

#[test]
fn track_preserves_order_of_interleaved_items() {
    let items = vec![
        make_meal("Breakfast"),
        make_ate("Oatmeal", "1 cup"),
        make_exercised("Running", "30m"),
        make_meal("Lunch"),
        make_ate("Sandwich", "1"),
    ];
    let result = apply_track_to_content("", "2026-01-01", &items);
    let doc = parse(&result).expect("result should be parseable");
    match &doc.items[0] {
        Item::Day(day) => {
            assert_eq!(day.items.len(), 5);
            assert!(matches!(&day.items[0], DayItem::Meal(l) if l == "Breakfast"));
            assert!(matches!(&day.items[1], DayItem::Ate(a) if a.food_alias == "Oatmeal"));
            assert!(
                matches!(&day.items[2], DayItem::Exercised(e) if e.exercise_alias == "Running")
            );
            assert!(matches!(&day.items[3], DayItem::Meal(l) if l == "Lunch"));
            assert!(matches!(&day.items[4], DayItem::Ate(a) if a.food_alias == "Sandwich"));
        }
        _ => panic!("expected Day"),
    }
}

// ---------------------------------------------------------------------------
// build_ordered_items with full CLI args slice
// ---------------------------------------------------------------------------

#[test]
fn build_ordered_items_from_full_cli_args_preserves_interleaved_order() {
    let args: Vec<String> = [
        "nutrition",
        "track",
        "--meal-label",
        "Breakfast",
        "--ate",
        "Granola(100g)",
        "--exercised",
        "Running(30m)",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let items = build_ordered_items(&args).expect("should parse");
    assert_eq!(items.len(), 3);
    assert!(matches!(&items[0], DayItem::Meal(l) if l == "Breakfast"));
    assert!(matches!(&items[1], DayItem::Ate(a) if a.food_alias == "Granola"));
    assert!(matches!(&items[2], DayItem::Exercised(e) if e.exercise_alias == "Running"));
}

// ---------------------------------------------------------------------------
// File write via mocked I/O
// ---------------------------------------------------------------------------

#[test]
fn run_track_with_io_updates_content_in_place_for_existing_date() {
    let file_content = Rc::new(RefCell::new(
        "@day \"2026-01-01\" {\n    [Breakfast]\n}\n".to_string(),
    ));
    let file_write = Rc::clone(&file_content);

    let items = vec![make_ate("Granola", "100g")];
    run_track_with_io(
        || Ok(file_content.borrow().clone()),
        |updated| {
            *file_write.borrow_mut() = updated;
            Ok(())
        },
        "2026-01-01",
        &items,
    )
    .expect("should succeed");

    let result = file_content.borrow().clone();
    // Exactly one block, containing both original and new items
    assert_eq!(result.matches("@day \"2026-01-01\"").count(), 1);

    let doc = parse(&result).expect("written content should be parseable");
    match &doc.items[0] {
        Item::Day(day) => assert_eq!(day.items.len(), 2), // [Breakfast] + Granola
        _ => panic!("expected Day"),
    }
}

#[test]
fn run_track_with_io_appends_new_block_when_date_absent() {
    let initial = "@day \"2025-12-31\" {\n    [Dinner]\n}\n";
    let written = Rc::new(RefCell::new(String::new()));
    let written_clone = Rc::clone(&written);

    let items = vec![make_meal("Breakfast"), make_ate("Granola", "100g")];
    run_track_with_io(
        || Ok(initial.to_string()),
        |updated| {
            *written_clone.borrow_mut() = updated;
            Ok(())
        },
        "2026-01-01",
        &items,
    )
    .expect("should succeed");

    let result = written.borrow().clone();
    let doc = parse(&result).expect("written content should be parseable");
    assert_eq!(doc.items.len(), 2);
}

#[test]
fn run_track_with_io_written_content_is_in_place_not_appended() {
    // Verify that an existing block is modified at its original position,
    // not replaced by appending a duplicate block at the end.
    let initial = concat!(
        "@day \"2026-01-01\" {\n    [Breakfast]\n}\n\n",
        "@day \"2026-01-02\" {\n    [Breakfast]\n}\n"
    );
    let written = Rc::new(RefCell::new(String::new()));
    let written_clone = Rc::clone(&written);

    let items = vec![make_ate("Granola", "100g")];
    run_track_with_io(
        || Ok(initial.to_string()),
        |updated| {
            *written_clone.borrow_mut() = updated;
            Ok(())
        },
        "2026-01-01",
        &items,
    )
    .expect("should succeed");

    let result = written.borrow().clone();

    // Exactly one block per date (no duplicate appended)
    assert_eq!(result.matches("@day \"2026-01-01\"").count(), 1);
    assert_eq!(result.matches("@day \"2026-01-02\"").count(), 1);

    // The new item appears before the second day block
    let ate_pos = result.find("@ate \"Granola\"").unwrap();
    let second_block_pos = result.find("@day \"2026-01-02\"").unwrap();
    assert!(
        ate_pos < second_block_pos,
        "new item should be inside the first block, not after the second"
    );
}
