//! Nutrition-label-style display formatting for nutrition reports.
//!
//! This module converts [`NutritionReport`] and [`DailyNutritionReport`] values
//! into an attractive, human-readable box-drawing representation that resembles a
//! standard nutrition facts label.
//!
//! Pass `--json` on the CLI to skip this display and emit raw JSON instead.

use crate::ast::ast::Property;
use crate::nutrition::{AggregatedReport, DailyNutritionReport, NutritionReport};

// ── Layout constants ──────────────────────────────────────────────────────────

/// Width of the inner content area (excluding the two `│` border characters).
const W: usize = 44;

// ── Border / separator helpers ────────────────────────────────────────────────

fn top_border() -> String {
    format!("┌{}┐", "─".repeat(W))
}

fn bottom_border() -> String {
    format!("└{}┘", "─".repeat(W))
}

/// A divider with a centred label, e.g. `├──── Intake ─────────────────────┤`.
fn section_divider(label: &str) -> String {
    let inner = format!(" {} ", label);
    let inner_len = inner.len();
    if inner_len >= W {
        return format!("├{}┤", &inner[..W]);
    }
    let remaining = W - inner_len;
    let left = remaining / 2;
    let right = remaining - left;
    format!("├{}{}{}┤", "─".repeat(left), inner, "─".repeat(right))
}

/// A centred text row, e.g. `│        Nutrition Facts         │`.
fn center_row(text: &str) -> String {
    let text_chars: Vec<char> = text.chars().collect();
    let text_len = text_chars.len();
    if text_len >= W {
        let truncated: String = text_chars[..W].iter().collect();
        return format!("│{}│", truncated);
    }
    let remaining = W - text_len;
    let left = remaining / 2;
    let right = remaining - left;
    format!("│{}{}{}│", " ".repeat(left), text, " ".repeat(right))
}

/// A bold separator row used for section emphasis.
fn thick_divider() -> String {
    format!("╞{}╡", "═".repeat(W))
}

// ── Value formatting helpers ──────────────────────────────────────────────────

/// Format a floating-point amount: integers render without a decimal point;
/// values with a fractional part render with one decimal place.
fn fmt_amount(amount: f64) -> String {
    if (amount - amount.round()).abs() < 0.005 {
        format!("{}", amount.round() as i64)
    } else {
        format!("{:.1}", amount)
    }
}

/// Format a `Property` as a single content row:
/// `│  name                     123 kcal  │`
fn property_row(prop: &Property) -> String {
    let unit_str = prop.value.unit.as_deref().unwrap_or("");
    let value_str = if unit_str.is_empty() {
        fmt_amount(prop.value.amount)
    } else {
        format!("{} {}", fmt_amount(prop.value.amount), unit_str)
    };
    // name truncated / padded to 18 chars; value right-aligned in 22 chars
    let name_display: String = prop.name.chars().take(18).collect();
    // inner = 2 + 18 + 22 + 2 = 44 = W
    format!("│  {:<18}{:>22}  │", name_display, value_str)
}

// ── Property sort order ───────────────────────────────────────────────────────

/// Sort properties with common macro-nutrients first (calories, protein, fat,
/// carbohydrates, …), then remaining entries alphabetically.
fn sorted_props(props: &[Property]) -> Vec<&Property> {
    const PRIORITY: &[&str] = &[
        "calories",
        "protein",
        "fat",
        "saturated fat",
        "trans fat",
        "carbohydrates",
        "carbs",
        "fiber",
        "sugar",
        "sodium",
        "cholesterol",
    ];

    let mut sorted: Vec<&Property> = props.iter().collect();
    sorted.sort_by(|a, b| {
        let ai = PRIORITY.iter().position(|&p| p == a.name.as_str());
        let bi = PRIORITY.iter().position(|&p| p == b.name.as_str());
        match (ai, bi) {
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });
    sorted
}

// ── Public formatters ─────────────────────────────────────────────────────────

/// Render a [`NutritionReport`] as a nutrition-label-style box.
///
/// ```text
/// ┌────────────────────────────────────────────┐
/// │             Nutrition Facts                │
/// │          chickpeas  ·  200 g               │
/// ╞════════════════════════════════════════════╡
/// │  calories                       538 kcal  │
/// │  protein                          29 g    │
/// └────────────────────────────────────────────┘
/// ```
pub fn format_nutrition_report(report: &NutritionReport) -> String {
    let qty_str = match &report.quantity.unit {
        Some(u) => format!("{} {}", fmt_amount(report.quantity.amount), u),
        None => fmt_amount(report.quantity.amount),
    };
    let subtitle = format!("{}  ·  {}", report.name, qty_str);

    let mut lines = Vec::new();
    lines.push(top_border());
    lines.push(center_row("Nutrition Facts"));
    lines.push(center_row(&subtitle));
    lines.push(thick_divider());

    let sorted = sorted_props(&report.properties);
    if sorted.is_empty() {
        lines.push(center_row("(no nutritional data)"));
    } else {
        for prop in sorted {
            lines.push(property_row(prop));
        }
    }

    lines.push(bottom_border());
    lines.join("\n")
}

/// Render a [`DailyNutritionReport`] as a nutrition-label-style box with
/// Intake / Exercise / Net sections.
///
/// ```text
/// ┌────────────────────────────────────────────┐
/// │          Daily Nutrition Report            │
/// │               2026-01-01                   │
/// ├──────────────── Intake ────────────────────┤
/// │  calories                       538 kcal  │
/// ├─────────────── Exercise ───────────────────┤
/// │  calories                       200 kcal  │
/// ├──────────────── Net ───────────────────────┤
/// │  calories                       338 kcal  │
/// └────────────────────────────────────────────┘
/// ```
pub fn format_daily_report(report: &DailyNutritionReport) -> String {
    let mut lines = Vec::new();
    lines.push(top_border());
    lines.push(center_row("Daily Nutrition Report"));
    lines.push(center_row(&report.date));

    // ── Intake ──
    lines.push(section_divider("Intake"));
    let intake = sorted_props(&report.intake);
    if intake.is_empty() {
        lines.push(center_row("(no intake recorded)"));
    } else {
        for prop in intake {
            lines.push(property_row(prop));
        }
    }

    // ── Exercise (omitted when empty) ──
    if !report.exercise.is_empty() {
        lines.push(section_divider("Exercise"));
        for prop in sorted_props(&report.exercise) {
            lines.push(property_row(prop));
        }
    }

    // ── Net ──
    lines.push(section_divider("Net"));
    let net = sorted_props(&report.net);
    if net.is_empty() {
        lines.push(center_row("(no data)"));
    } else {
        for prop in net {
            lines.push(property_row(prop));
        }
    }

    lines.push(bottom_border());
    lines.join("\n")
}

/// Render an [`AggregatedReport`] as a nutrition-label-style box showing the
/// date range, number of days, and per-day averages for Intake / Exercise / Net.
///
/// ```text
/// ┌────────────────────────────────────────────┐
/// │       Average Daily Nutrition Report       │
/// │          2026-01-01 – 2026-01-31           │
/// │               2 days of data               │
/// ├────────────────── Intake ──────────────────┤
/// │  calories                           190.3  │
/// ├─────────────────── Net ────────────────────┤
/// │  calories                           190.3  │
/// └────────────────────────────────────────────┘
/// ```
pub fn format_aggregated_report(report: &AggregatedReport) -> String {
    let mut lines = Vec::new();
    lines.push(top_border());
    lines.push(center_row("Average Daily Nutrition Report"));
    let range_str = if report.start == report.end {
        report.start.clone()
    } else {
        format!("{} \u{2013} {}", report.start, report.end)
    };
    lines.push(center_row(&range_str));
    let days_str = if report.days == 1 {
        "1 day of data".to_string()
    } else {
        format!("{} days of data", report.days)
    };
    lines.push(center_row(&days_str));

    // ── Intake ──
    lines.push(section_divider("Intake"));
    let intake = sorted_props(&report.intake);
    if intake.is_empty() {
        lines.push(center_row("(no intake recorded)"));
    } else {
        for prop in intake {
            lines.push(property_row(prop));
        }
    }

    // ── Exercise (omitted when empty) ──
    if !report.exercise.is_empty() {
        lines.push(section_divider("Exercise"));
        for prop in sorted_props(&report.exercise) {
            lines.push(property_row(prop));
        }
    }

    // ── Net ──
    lines.push(section_divider("Net"));
    let net = sorted_props(&report.net);
    if net.is_empty() {
        lines.push(center_row("(no data)"));
    } else {
        for prop in net {
            lines.push(property_row(prop));
        }
    }

    lines.push(bottom_border());
    lines.join("\n")
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ast::{Property, Quantity};
    use crate::nutrition::{DailyNutritionReport, NutritionReport};

    fn make_prop(name: &str, amount: f64, unit: Option<&str>) -> Property {
        Property {
            name: name.to_string(),
            value: Quantity {
                amount,
                unit: unit.map(str::to_string),
            },
        }
    }

    #[test]
    fn format_nutrition_report_contains_header_and_properties() {
        let report = NutritionReport {
            name: "chickpeas".to_string(),
            quantity: Quantity { amount: 200.0, unit: Some("g".to_string()) },
            properties: vec![
                make_prop("calories", 538.0, Some("kcal")),
                make_prop("protein", 29.0, Some("g")),
            ],
        };
        let output = format_nutrition_report(&report);
        assert!(output.contains("Nutrition Facts"), "should contain header");
        assert!(output.contains("chickpeas"), "should contain ingredient name");
        assert!(output.contains("538 kcal"), "should contain calorie value");
        assert!(output.contains("29 g"), "should contain protein value");
        // calories should appear before protein (priority ordering)
        let cal_pos = output.find("calories").unwrap();
        let pro_pos = output.find("protein").unwrap();
        assert!(cal_pos < pro_pos, "calories should come before protein");
    }

    #[test]
    fn format_nutrition_report_no_properties_shows_placeholder() {
        let report = NutritionReport {
            name: "water".to_string(),
            quantity: Quantity { amount: 1.0, unit: Some("cup".to_string()) },
            properties: vec![],
        };
        let output = format_nutrition_report(&report);
        assert!(output.contains("no nutritional data"), "placeholder for empty properties");
    }

    #[test]
    fn format_daily_report_shows_all_sections() {
        let report = DailyNutritionReport {
            date: "2026-01-01".to_string(),
            intake: vec![make_prop("calories", 538.0, Some("kcal"))],
            exercise: vec![make_prop("calories", 200.0, Some("kcal"))],
            net: vec![make_prop("calories", 338.0, Some("kcal"))],
        };
        let output = format_daily_report(&report);
        assert!(output.contains("Daily Nutrition Report"), "header present");
        assert!(output.contains("2026-01-01"), "date present");
        assert!(output.contains("Intake"), "intake section present");
        assert!(output.contains("Exercise"), "exercise section present");
        assert!(output.contains("Net"), "net section present");
        assert!(output.contains("538 kcal"), "intake calories present");
        assert!(output.contains("200 kcal"), "exercise calories present");
        assert!(output.contains("338 kcal"), "net calories present");
    }

    #[test]
    fn format_daily_report_omits_exercise_section_when_empty() {
        let report = DailyNutritionReport {
            date: "2026-01-01".to_string(),
            intake: vec![make_prop("calories", 300.0, Some("kcal"))],
            exercise: vec![],
            net: vec![make_prop("calories", 300.0, Some("kcal"))],
        };
        let output = format_daily_report(&report);
        assert!(!output.contains("Exercise"), "exercise section should be omitted when empty");
    }

    #[test]
    fn all_rows_have_same_display_width() {
        // Every line in the output should have the same printed width.
        let report = NutritionReport {
            name: "test".to_string(),
            quantity: Quantity { amount: 100.0, unit: Some("g".to_string()) },
            properties: vec![
                make_prop("calories", 100.0, Some("kcal")),
                make_prop("protein", 5.5, Some("g")),
            ],
        };
        let output = format_nutrition_report(&report);
        let widths: Vec<usize> = output.lines().map(|l| l.chars().count()).collect();
        let first = widths[0];
        for (i, &w) in widths.iter().enumerate() {
            assert_eq!(w, first, "line {} has width {} but expected {}", i, w, first);
        }
    }
}
