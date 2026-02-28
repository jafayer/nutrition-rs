/// Tests that `DayEmitter` and `Day::to_string()` both correctly preserve
/// meal labels (e.g. `[Breakfast]`) in their emitted output, and that the
/// emitted text round-trips through the parser with the same item order.
use nutrition_rs::ast::ast::{Ate, Day, DayItem, Exercised, Item, Quantity};
use nutrition_rs::emitters::day::DayEmitter;
use nutrition_rs::emitters::emitter::CanEmit;
use nutrition_rs::parser::parser::parse;

// ── helpers ──────────────────────────────────────────────────────────────────

fn ate(alias: &str, amount: f64, unit: Option<&str>) -> DayItem {
    DayItem::Ate(Ate {
        food_alias: alias.to_string(),
        quantity: Quantity {
            amount,
            unit: unit.map(str::to_string),
        },
    })
}

fn exercised(alias: &str, amount: f64, unit: Option<&str>) -> DayItem {
    DayItem::Exercised(Exercised {
        exercise_alias: alias.to_string(),
        quantity: Quantity {
            amount,
            unit: unit.map(str::to_string),
        },
    })
}

fn meal(label: &str) -> DayItem {
    DayItem::Meal(label.to_string())
}

/// Parse the emitted text back and return the items from the single @day block.
fn roundtrip_items(emitted: &str) -> Vec<DayItem> {
    let doc = parse(emitted).expect("emitted text failed to parse");
    assert_eq!(doc.items.len(), 1, "expected exactly one item in document");
    match doc.items.into_iter().next().unwrap() {
        Item::Day(day) => day.items,
        other => panic!("expected Item::Day, got {:?}", other),
    }
}

// ── DayEmitter tests ─────────────────────────────────────────────────────────

#[test]
fn emitter_emits_single_meal_label() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![meal("Breakfast")],
    };
    let output = DayEmitter.emit(&day);
    assert!(
        output.contains("[Breakfast]"),
        "emitted text should contain '[Breakfast]', got:\n{output}"
    );
}

#[test]
fn emitter_meal_label_roundtrip() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![meal("Breakfast")],
    };
    let output = DayEmitter.emit(&day);
    let items = roundtrip_items(&output);
    assert_eq!(items.len(), 1);
    match &items[0] {
        DayItem::Meal(label) => assert_eq!(label, "Breakfast"),
        other => panic!("expected Meal, got {:?}", other),
    }
}

#[test]
fn emitter_preserves_meal_label_order_between_ate() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![
            meal("Breakfast"),
            ate("oatmeal", 1.0, Some("cup")),
            meal("Lunch"),
            ate("sandwich", 1.0, None),
            meal("Dinner"),
            ate("pasta", 1.0, Some("bowl")),
        ],
    };
    let output = DayEmitter.emit(&day);
    let items = roundtrip_items(&output);

    assert_eq!(items.len(), 6, "all 6 items should survive the roundtrip");
    assert!(matches!(&items[0], DayItem::Meal(l) if l == "Breakfast"));
    assert!(matches!(&items[1], DayItem::Ate(a) if a.food_alias == "oatmeal"));
    assert!(matches!(&items[2], DayItem::Meal(l) if l == "Lunch"));
    assert!(matches!(&items[3], DayItem::Ate(a) if a.food_alias == "sandwich"));
    assert!(matches!(&items[4], DayItem::Meal(l) if l == "Dinner"));
    assert!(matches!(&items[5], DayItem::Ate(a) if a.food_alias == "pasta"));
}

#[test]
fn emitter_preserves_meal_label_order_with_exercised() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![
            meal("Morning"),
            exercised("running", 30.0, Some("min")),
            meal("Breakfast"),
            ate("eggs", 2.0, None),
            exercised("yoga", 20.0, Some("min")),
        ],
    };
    let output = DayEmitter.emit(&day);
    let items = roundtrip_items(&output);

    assert_eq!(items.len(), 5);
    assert!(matches!(&items[0], DayItem::Meal(l) if l == "Morning"));
    assert!(matches!(&items[1], DayItem::Exercised(e) if e.exercise_alias == "running"));
    assert!(matches!(&items[2], DayItem::Meal(l) if l == "Breakfast"));
    assert!(matches!(&items[3], DayItem::Ate(a) if a.food_alias == "eggs"));
    assert!(matches!(&items[4], DayItem::Exercised(e) if e.exercise_alias == "yoga"));
}

#[test]
fn emitter_consecutive_meal_labels_roundtrip() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![
            meal("Breakfast"),
            meal("Lunch"),
            meal("Dinner"),
        ],
    };
    let output = DayEmitter.emit(&day);
    let items = roundtrip_items(&output);

    assert_eq!(items.len(), 3);
    assert!(matches!(&items[0], DayItem::Meal(l) if l == "Breakfast"));
    assert!(matches!(&items[1], DayItem::Meal(l) if l == "Lunch"));
    assert!(matches!(&items[2], DayItem::Meal(l) if l == "Dinner"));
}

#[test]
fn emitter_meal_label_with_spaces_roundtrip() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![meal("Early Morning Snack")],
    };
    let output = DayEmitter.emit(&day);
    let items = roundtrip_items(&output);

    assert_eq!(items.len(), 1);
    match &items[0] {
        DayItem::Meal(label) => assert_eq!(label, "Early Morning Snack"),
        other => panic!("expected Meal, got {:?}", other),
    }
}

#[test]
fn emitter_no_items_roundtrip() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![],
    };
    let output = DayEmitter.emit(&day);
    let items = roundtrip_items(&output);
    assert!(items.is_empty());
}

// ── Day::to_string tests ──────────────────────────────────────────────────────

#[test]
fn to_string_emits_meal_label_with_brackets() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![meal("Breakfast")],
    };
    let output = day.to_string();
    assert!(
        output.contains("[Breakfast]"),
        "Day::to_string() should contain '[Breakfast]', got:\n{output}"
    );
    // Must NOT use quoted-string syntax for meal labels
    assert!(
        !output.contains("\"Breakfast\""),
        "Day::to_string() must not emit a quoted string for a meal label"
    );
}

#[test]
fn to_string_meal_label_roundtrip() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![
            meal("Breakfast"),
            ate("oatmeal", 1.0, Some("cup")),
            meal("Lunch"),
            ate("salad", 1.0, Some("bowl")),
        ],
    };
    let output = day.to_string();
    let items = roundtrip_items(&output);

    assert_eq!(items.len(), 4, "all items should survive the roundtrip");
    assert!(matches!(&items[0], DayItem::Meal(l) if l == "Breakfast"));
    assert!(matches!(&items[1], DayItem::Ate(a) if a.food_alias == "oatmeal"));
    assert!(matches!(&items[2], DayItem::Meal(l) if l == "Lunch"));
    assert!(matches!(&items[3], DayItem::Ate(a) if a.food_alias == "salad"));
}

// ── Consistency between DayEmitter and Day::to_string ────────────────────────

#[test]
fn emitter_and_to_string_agree_on_meal_label_syntax() {
    let day = Day {
        date: "2026-01-01".to_string(),
        items: vec![
            meal("Breakfast"),
            ate("eggs", 2.0, None),
        ],
    };
    let via_emitter   = DayEmitter.emit(&day);
    let via_to_string = day.to_string();

    // Both should use [Breakfast] syntax
    assert!(via_emitter.contains("[Breakfast]"),   "DayEmitter should use bracket syntax");
    assert!(via_to_string.contains("[Breakfast]"), "Day::to_string should use bracket syntax");
}
