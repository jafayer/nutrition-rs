//! HTTP handler and web UI for the nutrition server.
//!
//! Exposes a REST API (`/api/…`) and a mobile-responsive HTML web UI that
//! support read and write operations on the nutrition file passed to
//! `nutrition serve`.

use std::collections::HashSet;
use std::io::Write as IoWrite;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use minijinja::context;
use serde::{Deserialize, Serialize};

use crate::ast::ast::{DayItem, Document, Exercise, Ingredient, Item, Quantity, Recipe};
use crate::cli::file_loader::load_tree;
use crate::emitters::emitter::CanEmit;
use crate::emitters::exercise::ExerciseEmitter;
use crate::emitters::ingredient::IngredientEmitter;
use crate::emitters::recipe::RecipeEmitter;
use crate::nutrition::{compute_daily_report, query_nutrition, NutritionReport};

// ── Application state ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub file_path: Arc<String>,
    /// Serialises all file-write operations to prevent concurrent corruption.
    pub write_lock: Arc<tokio::sync::Mutex<()>>,
    /// RwLock: concurrent reads never block each other; writes are exclusive.
    pub cache: Arc<tokio::sync::RwLock<Option<Document>>>,
    /// Minijinja environment shared across all page handlers.
    pub env: Arc<minijinja::Environment<'static>>,
}

// ── Cache helpers ─────────────────────────────────────────────────────────────

/// Return the cached document, or parse the file and populate the cache.
///
/// Uses double-checked locking: a read lock is tried first so that warm-cache
/// calls never take an exclusive lock.  Only when the cache is cold does it
/// fall through to the write lock (which re-checks after acquiring to avoid
/// a double-parse under concurrent cold requests).
async fn get_doc(state: &AppState) -> Result<Document, (StatusCode, String)> {
    // Fast path – read lock, returns immediately when cache is warm.
    {
        let r = state.cache.read().await;
        if let Some(doc) = r.as_ref() {
            return Ok(doc.clone());
        }
    }
    // Slow path – write lock + double-check.
    let mut w = state.cache.write().await;
    if let Some(doc) = w.as_ref() {
        return Ok(doc.clone());
    }
    let doc = load_tree(Some(state.file_path.as_str()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    *w = Some(doc.clone());
    Ok(doc)
}

/// After a write, reload the document from disk into the cache so that the
/// very next request sees the updated content without a cold miss.
async fn reload_cache(state: &AppState) {
    let mut w = state.cache.write().await;
    match load_tree(Some(state.file_path.as_str())) {
        Ok(doc) => *w = Some(doc),
        Err(_) => *w = None, // fall back to cold reload on next request
    }
}

// ── Fuzzy search ──────────────────────────────────────────────────────────────

/// Score how well `needle` fuzzy-matches `haystack` (case-insensitive).
///
/// Scoring bands (higher = better match):
///   1000 – exact
///    900 – prefix
///  < 800 – substring (earlier position = higher score)
///  < 700 – subsequence (closer length = higher score)
///   None – no match
fn fuzzy_score(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    if h == n {
        return Some(1000);
    }
    if h.starts_with(&n) {
        return Some(900);
    }
    if let Some(pos) = h.find(&n) {
        return Some(800usize.saturating_sub(pos * 5));
    }
    // Subsequence check: every char of n must appear in h in order.
    let mut h_iter = h.chars();
    for nc in n.chars() {
        if h_iter.find(|&hc| hc == nc).is_none() {
            return None;
        }
    }
    let gap = h.chars().count().saturating_sub(n.chars().count());
    Some(700usize.saturating_sub(gap * 5))
}

/// Best fuzzy score across all `aliases` for `query`.
/// Returns `Some(0)` for an empty query so that all items match.
fn best_alias_score(aliases: &[String], query: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    aliases.iter().filter_map(|a| fuzzy_score(a, query)).max()
}

// ── Date helper (no external crate required) ──────────────────────────────────

/// Current date in `YYYY-MM-DD` format, derived from the UTC Unix timestamp.
fn today_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Civil-calendar algorithm (Gregorian) from Howard Hinnant
    // https://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let z = (secs / 86400) as u32 + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

// ── File helpers ──────────────────────────────────────────────────────────────

fn append_to_file(file_path: &str, text: &str) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(file_path)
        .map_err(|e| format!("Failed to open file '{}': {}", file_path, e))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write to file '{}': {}", file_path, e))
}

/// Add a single `entry` line to the `@day "date"` block.
///
/// If the block already exists the entry is inserted before its closing `}`.
/// If no such block exists a new one is appended to the file.
fn add_entry_to_day(file_path: &str, date: &str, entry: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read '{}': {}", file_path, e))?;

    let day_marker = format!("@day \"{}\"", date);

    let new_content = if let Some(day_pos) = content.find(&day_marker) {
        let after_day = &content[day_pos..];
        let brace_offset = after_day
            .find('{')
            .ok_or_else(|| format!("Malformed @day block for '{}'", date))?;
        let abs_open = day_pos + brace_offset;

        let mut depth: i32 = 0;
        let mut close_byte = None;
        for (i, ch) in content[abs_open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close_byte = Some(abs_open + i);
                        break;
                    }
                }
                _ => {}
            }
        }

        let close = close_byte
            .ok_or_else(|| format!("No closing brace for @day '{}'", date))?;

        let before = &content[..close];
        let after = &content[close..];
        let sep = if before.ends_with('\n') { "" } else { "\n" };
        format!("{}{}    {}\n{}", before, sep, entry, after)
    } else {
        let new_block = format!("\n@day \"{}\" {{\n    {}\n}}\n", date, entry);
        format!("{}{}", content, new_block)
    };

    std::fs::write(file_path, new_content)
        .map_err(|e| format!("Failed to write '{}': {}", file_path, e))
}

// ── Alias collectors ──────────────────────────────────────────────────────────

fn all_food_aliases(doc: &Document) -> Vec<String> {
    let mut aliases: Vec<String> = doc
        .items
        .iter()
        .flat_map(|item| match item {
            Item::Ingredient(i) => i.aliases.clone(),
            Item::Recipe(r) => r.aliases.clone(),
            _ => vec![],
        })
        .collect();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn all_exercise_aliases(doc: &Document) -> Vec<String> {
    let mut aliases: Vec<String> = doc
        .items
        .iter()
        .flat_map(|item| match item {
            Item::Exercise(e) => e.aliases.clone(),
            _ => vec![],
        })
        .collect();
    aliases.sort();
    aliases.dedup();
    aliases
}

// ── Page render helper ────────────────────────────────────────────────────────

fn render_page<S: serde::Serialize>(state: &AppState, template: &str, ctx: S) -> Response {
    match state.env.get_template(template) {
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Ok(tmpl) => match tmpl.render(ctx) {
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            Ok(html) => Html(html).into_response(),
        },
    }
}

// ── Shared day-item serialiser ────────────────────────────────────────────────

fn serialise_day_items(day: &crate::ast::ast::Day) -> Vec<serde_json::Value> {
    day.items
        .iter()
        .map(|di| match di {
            DayItem::Ate(ate) => serde_json::json!({
                "type": "ate",
                "alias": ate.food_alias,
                "quantity": ate.quantity,
            }),
            DayItem::Exercised(ex) => serde_json::json!({
                "type": "exercised",
                "alias": ex.exercise_alias,
                "quantity": ex.quantity,
            }),
            DayItem::Meal(label) => serde_json::json!({
                "type": "meal",
                "label": label,
            }),
        })
        .collect()
}

/// Sort a `Vec<Property>` by a priority list (calories, protein, …).
fn sort_intake(mut props: Vec<crate::ast::ast::Property>) -> Vec<crate::ast::ast::Property> {
    const PRIORITY: &[&str] = &[
        "calories", "protein", "fat", "carbohydrates", "carbs", "fiber", "sugar",
    ];
    props.sort_by(|a, b| {
        let ai = PRIORITY.iter().position(|&p| p == a.name.as_str());
        let bi = PRIORITY.iter().position(|&p| p == b.name.as_str());
        match (ai, bi) {
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });
    props
}

// ── Request / response types ──────────────────────────────────────────────────

/// Query params accepted by all list endpoints.  The optional `q` field
/// enables fuzzy search; results are ranked by relevance when it is set.
#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    #[serde(default = "default_offset")]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_offset() -> usize {
    0
}
/// API list endpoints return up to 50 items by default (machine consumers).
fn default_limit() -> usize {
    50
}

#[derive(Deserialize)]
struct QueryPageParams {
    q: Option<String>,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_page_size")]
    limit: usize,
}

/// The HTML query page shows 20 items per page (human-readable page size).
fn default_page_size() -> usize {
    20
}

#[derive(Deserialize)]
struct LogAteRequest {
    alias: String,
    quantity: String,
}

#[derive(Deserialize)]
struct LogExercisedRequest {
    alias: String,
    quantity: String,
}

#[derive(Serialize)]
struct DayListEntry {
    date: String,
    items_count: usize,
}

#[derive(Serialize)]
struct DayDetailResponse {
    date: String,
    items: Vec<serde_json::Value>,
    report: crate::nutrition::DailyNutritionReport,
}

#[derive(Serialize)]
struct IngredientDetail {
    ingredient: Ingredient,
    nutrition: Option<NutritionReport>,
}

#[derive(Serialize)]
struct RecipeDetail {
    recipe: Recipe,
    nutrition: Option<NutritionReport>,
}

#[derive(Serialize)]
struct ExerciseDetail {
    exercise: Exercise,
}

// ── API: Ingredients ──────────────────────────────────────────────────────────

async fn api_list_ingredients(
    State(state): State<AppState>,
    Query(sq): Query<SearchQuery>,
) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let q = sq.q.as_deref().unwrap_or("").trim();
    let mut scored: Vec<(usize, &Ingredient)> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Ingredient(ing) = item {
                best_alias_score(&ing.aliases, q).map(|sc| (sc, ing))
            } else {
                None
            }
        })
        .collect();
    if !q.is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }
    let total = scored.len();
    let items: Vec<&Ingredient> = scored
        .into_iter()
        .skip(sq.offset)
        .take(sq.limit)
        .map(|(_, i)| i)
        .collect();
    (
        StatusCode::OK,
        [("X-Total-Count", total.to_string())],
        Json(items),
    )
        .into_response()
}

async fn api_get_ingredient(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> Response {
    let alias = alias.to_lowercase();
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let ingredient = doc.items.iter().find_map(|item| {
        if let Item::Ingredient(ing) = item {
            if ing.aliases.iter().any(|a| a.to_lowercase() == alias) {
                return Some(ing.clone());
            }
        }
        None
    });
    match ingredient {
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("No ingredient named '{}'", alias) })),
        )
            .into_response(),
        Some(ing) => {
            let nutrition = query_nutrition(&doc, &alias, None).ok();
            (StatusCode::OK, Json(IngredientDetail { ingredient: ing, nutrition })).into_response()
        }
    }
}

async fn api_create_ingredient(
    State(state): State<AppState>,
    Json(ingredient): Json<Ingredient>,
) -> Response {
    let text = IngredientEmitter.emit(&ingredient);
    let _guard = state.write_lock.lock().await;
    match append_to_file(&state.file_path, &format!("\n{}", text)) {
        Ok(()) => {
            reload_cache(&state).await;
            (StatusCode::CREATED, Json(ingredient)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Recipes ──────────────────────────────────────────────────────────────

async fn api_list_recipes(
    State(state): State<AppState>,
    Query(sq): Query<SearchQuery>,
) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let q = sq.q.as_deref().unwrap_or("").trim();
    let mut scored: Vec<(usize, &Recipe)> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Recipe(r) = item {
                best_alias_score(&r.aliases, q).map(|sc| (sc, r))
            } else {
                None
            }
        })
        .collect();
    if !q.is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }
    let total = scored.len();
    let items: Vec<&Recipe> = scored
        .into_iter()
        .skip(sq.offset)
        .take(sq.limit)
        .map(|(_, r)| r)
        .collect();
    (
        StatusCode::OK,
        [("X-Total-Count", total.to_string())],
        Json(items),
    )
        .into_response()
}

async fn api_get_recipe(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> Response {
    let alias = alias.to_lowercase();
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let recipe = doc.items.iter().find_map(|item| {
        if let Item::Recipe(r) = item {
            if r.aliases.iter().any(|a| a.to_lowercase() == alias) {
                return Some(r.clone());
            }
        }
        None
    });
    match recipe {
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("No recipe named '{}'", alias) })),
        )
            .into_response(),
        Some(rec) => {
            let nutrition = query_nutrition(&doc, &alias, None).ok();
            (StatusCode::OK, Json(RecipeDetail { recipe: rec, nutrition })).into_response()
        }
    }
}

async fn api_create_recipe(
    State(state): State<AppState>,
    Json(recipe): Json<Recipe>,
) -> Response {
    // Validate that every ingredient alias referenced in the recipe exists.
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let known: HashSet<String> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Ingredient(ing) = item {
                Some(ing.aliases.iter().map(|a| a.to_lowercase()))
            } else {
                None
            }
        })
        .flatten()
        .collect();
    for ing_label in &recipe.ingredients {
        if !known.contains(&ing_label.alias.to_lowercase()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Unknown ingredient: '{}'", ing_label.alias)
                })),
            )
                .into_response();
        }
    }

    let text = RecipeEmitter.emit(&recipe);
    let _guard = state.write_lock.lock().await;
    match append_to_file(&state.file_path, &format!("\n{}", text)) {
        Ok(()) => {
            reload_cache(&state).await;
            (StatusCode::CREATED, Json(recipe)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Exercises ────────────────────────────────────────────────────────────

async fn api_list_exercises(
    State(state): State<AppState>,
    Query(sq): Query<SearchQuery>,
) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let q = sq.q.as_deref().unwrap_or("").trim();
    let mut scored: Vec<(usize, &Exercise)> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Exercise(ex) = item {
                best_alias_score(&ex.aliases, q).map(|sc| (sc, ex))
            } else {
                None
            }
        })
        .collect();
    if !q.is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0));
    }
    let total = scored.len();
    let items: Vec<&Exercise> = scored
        .into_iter()
        .skip(sq.offset)
        .take(sq.limit)
        .map(|(_, ex)| ex)
        .collect();
    (
        StatusCode::OK,
        [("X-Total-Count", total.to_string())],
        Json(items),
    )
        .into_response()
}

async fn api_get_exercise(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> Response {
    let alias = alias.to_lowercase();
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let exercise = doc.items.iter().find_map(|item| {
        if let Item::Exercise(ex) = item {
            if ex.aliases.iter().any(|a| a.to_lowercase() == alias) {
                return Some(ex.clone());
            }
        }
        None
    });
    match exercise {
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("No exercise named '{}'", alias) })),
        )
            .into_response(),
        Some(ex) => (StatusCode::OK, Json(ExerciseDetail { exercise: ex })).into_response(),
    }
}

async fn api_create_exercise(
    State(state): State<AppState>,
    Json(exercise): Json<Exercise>,
) -> Response {
    let text = ExerciseEmitter.emit(&exercise);
    let _guard = state.write_lock.lock().await;
    match append_to_file(&state.file_path, &format!("\n{}", text)) {
        Ok(()) => {
            reload_cache(&state).await;
            (StatusCode::CREATED, Json(exercise)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Days ─────────────────────────────────────────────────────────────────

async fn api_list_days(
    State(state): State<AppState>,
    Query(sq): Query<SearchQuery>,
) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let mut days: Vec<DayListEntry> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Day(day) = item {
                Some(DayListEntry {
                    date: day.date.clone(),
                    items_count: day.items.len(),
                })
            } else {
                None
            }
        })
        .collect();
    // Newest first.
    days.sort_by(|a, b| b.date.cmp(&a.date));
    let total = days.len();
    let days: Vec<DayListEntry> = days.into_iter().skip(sq.offset).take(sq.limit).collect();
    (
        StatusCode::OK,
        [("X-Total-Count", total.to_string())],
        Json(days),
    )
        .into_response()
}

async fn api_get_day(
    State(state): State<AppState>,
    Path(date): Path<String>,
) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, Json(serde_json::json!({ "error": m }))).into_response(),
    };
    let day = doc.items.iter().find_map(|item| {
        if let Item::Day(d) = item {
            if d.date == date {
                return Some(d.clone());
            }
        }
        None
    });
    match day {
        None => {
            let empty = crate::nutrition::DailyNutritionReport {
                date: date.clone(),
                intake: vec![],
                exercise: vec![],
                net: vec![],
            };
            (
                StatusCode::OK,
                Json(DayDetailResponse { date, items: vec![], report: empty }),
            )
                .into_response()
        }
        Some(day) => {
            let report = compute_daily_report(&doc, &day);
            let items = serialise_day_items(&day);
            (
                StatusCode::OK,
                Json(DayDetailResponse { date: day.date, items, report }),
            )
                .into_response()
        }
    }
}

async fn api_log_ate(
    State(state): State<AppState>,
    Path(date): Path<String>,
    Json(body): Json<LogAteRequest>,
) -> Response {
    let qty = match Quantity::from_string(&body.quantity) {
        Ok(q) => q,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
        }
    };
    let alias_lower = body.alias.to_lowercase();
    let entry = format!("@ate \"{}\"({})", alias_lower, qty.to_string());
    let _guard = state.write_lock.lock().await;
    match add_entry_to_day(&state.file_path, &date, &entry) {
        Ok(()) => {
            reload_cache(&state).await;
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "date": date,
                    "alias": alias_lower,
                    "quantity": body.quantity,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn api_log_exercised(
    State(state): State<AppState>,
    Path(date): Path<String>,
    Json(body): Json<LogExercisedRequest>,
) -> Response {
    let qty = match Quantity::from_string(&body.quantity) {
        Ok(q) => q,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
        }
    };
    let alias_lower = body.alias.to_lowercase();
    let entry = format!("@exercised \"{}\"({})", alias_lower, qty.to_string());
    let _guard = state.write_lock.lock().await;
    match add_entry_to_day(&state.file_path, &date, &entry) {
        Ok(()) => {
            reload_cache(&state).await;
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "date": date,
                    "alias": alias_lower,
                    "quantity": body.quantity,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── Web UI pages ──────────────────────────────────────────────────────────────

async fn page_home(State(state): State<AppState>) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, m).into_response(),
    };
    let today = today_utc();
    let day = doc.items.iter().find_map(|item| {
        if let Item::Day(d) = item {
            if d.date == today {
                return Some(d.clone());
            }
        }
        None
    });
    let (items, intake) = if let Some(ref day) = day {
        let report = compute_daily_report(&doc, day);
        (serialise_day_items(day), sort_intake(report.intake))
    } else {
        (vec![], vec![])
    };
    let food_aliases = all_food_aliases(&doc);
    let exercise_aliases = all_exercise_aliases(&doc);
    render_page(
        &state,
        "pages/home.html",
        context! {
            active => "home",
            today_date => today,
            items => items,
            intake => intake,
            food_aliases => food_aliases,
            exercise_aliases => exercise_aliases,
        },
    )
}

async fn page_calendar(State(state): State<AppState>) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, m).into_response(),
    };
    let today = today_utc();
    let mut days: Vec<serde_json::Value> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Day(day) = item {
                Some(serde_json::json!({
                    "date": day.date,
                    "items_count": day.items.len(),
                    "is_today": day.date == today,
                }))
            } else {
                None
            }
        })
        .collect();
    // Newest first.
    days.sort_by(|a, b| {
        b["date"].as_str().unwrap_or("").cmp(a["date"].as_str().unwrap_or(""))
    });
    render_page(
        &state,
        "pages/calendar.html",
        context! {
            active => "calendar",
            days => days,
        },
    )
}

async fn page_calendar_day(
    State(state): State<AppState>,
    Path(date): Path<String>,
) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, m).into_response(),
    };
    let day = doc.items.iter().find_map(|item| {
        if let Item::Day(d) = item {
            if d.date == date {
                return Some(d.clone());
            }
        }
        None
    });
    let (items, intake, exercise, net) = if let Some(ref day) = day {
        let report = compute_daily_report(&doc, day);
        (
            serialise_day_items(day),
            sort_intake(report.intake),
            report.exercise,
            report.net,
        )
    } else {
        (vec![], vec![], vec![], vec![])
    };
    render_page(
        &state,
        "pages/calendar_day.html",
        context! {
            active => "calendar",
            date => date,
            items => items,
            intake => intake,
            exercise => exercise,
            net => net,
        },
    )
}

async fn page_query(
    State(state): State<AppState>,
    Query(params): Query<QueryPageParams>,
) -> Response {
    let doc = match get_doc(&state).await {
        Ok(d) => d,
        Err((s, m)) => return (s, m).into_response(),
    };
    let q = params.q.as_deref().unwrap_or("").trim().to_string();
    let offset = params.offset;
    let limit = params.limit;

    // Collect and score foods (ingredients + recipes combined).
    let mut foods: Vec<(usize, serde_json::Value)> = doc
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Ingredient(ing) => best_alias_score(&ing.aliases, &q).map(|sc| {
                (
                    sc,
                    serde_json::json!({
                        "item_type": "ingredient",
                        "primary": ing.aliases.first().cloned().unwrap_or_default(),
                        "aliases_rest": ing.aliases.get(1..).unwrap_or(&[]),
                        "quantity": ing.quantities.first(),
                    }),
                )
            }),
            Item::Recipe(rec) => best_alias_score(&rec.aliases, &q).map(|sc| {
                (
                    sc,
                    serde_json::json!({
                        "item_type": "recipe",
                        "primary": rec.aliases.first().cloned().unwrap_or_default(),
                        "aliases_rest": rec.aliases.get(1..).unwrap_or(&[]),
                        "quantity": rec.quantities.first(),
                    }),
                )
            }),
            _ => None,
        })
        .collect();
    if !q.is_empty() {
        foods.sort_by(|a, b| b.0.cmp(&a.0));
    }
    let food_total = foods.len();
    let foods: Vec<serde_json::Value> = foods
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, v)| v)
        .collect();

    // Exercises.
    let mut exercises: Vec<(usize, serde_json::Value)> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Exercise(ex) = item {
                best_alias_score(&ex.aliases, &q).map(|sc| {
                    (
                        sc,
                        serde_json::json!({
                            "primary": ex.aliases.first().cloned().unwrap_or_default(),
                            "aliases_rest": ex.aliases.get(1..).unwrap_or(&[]),
                            "quantity": ex.quantities.first(),
                        }),
                    )
                })
            } else {
                None
            }
        })
        .collect();
    if !q.is_empty() {
        exercises.sort_by(|a, b| b.0.cmp(&a.0));
    }
    let ex_total = exercises.len();
    let exercises: Vec<serde_json::Value> = exercises
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(_, v)| v)
        .collect();

    let has_prev = offset > 0;
    let has_next_food = offset + limit < food_total;
    let has_next_ex = offset + limit < ex_total;
    let prev_offset = offset.saturating_sub(limit);
    let next_offset = offset + limit;
    let showing_from = if food_total == 0 { 0 } else { offset + 1 };
    let showing_to_food = (offset + foods.len()).min(food_total);
    let showing_to_ex = (offset + exercises.len()).min(ex_total);

    render_page(
        &state,
        "pages/query.html",
        context! {
            active => "query",
            q => q,
            foods => foods,
            exercises => exercises,
            food_total => food_total,
            ex_total => ex_total,
            offset => offset,
            limit => limit,
            has_prev => has_prev,
            has_next_food => has_next_food,
            has_next_ex => has_next_ex,
            prev_offset => prev_offset,
            next_offset => next_offset,
            showing_from => showing_from,
            showing_to_food => showing_to_food,
            showing_to_ex => showing_to_ex,
            food_aliases => all_food_aliases(&doc),
        },
    )
}

async fn page_new_ingredient(State(state): State<AppState>) -> Response {
    render_page(
        &state,
        "pages/new_ingredient.html",
        context! { active => "new_ingredient" },
    )
}

async fn page_new_recipe(State(state): State<AppState>) -> Response {
    let food_aliases = match get_doc(&state).await {
        Ok(doc) => all_food_aliases(&doc),
        Err(_) => vec![],
    };
    render_page(
        &state,
        "pages/new_recipe.html",
        context! {
            active => "new_recipe",
            food_aliases => food_aliases,
        },
    )
}

async fn page_new_exercise(State(state): State<AppState>) -> Response {
    render_page(
        &state,
        "pages/new_exercise.html",
        context! { active => "new_exercise" },
    )
}

// ── run_server ────────────────────────────────────────────────────────────────

/// Build the minijinja environment, embedding all templates at compile time
/// so the deployed binary is self-contained.
fn build_env() -> minijinja::Environment<'static> {
    let mut env = minijinja::Environment::new();
    // Custom filter: format a float cleanly (drop .0 for whole numbers).
    // Threshold: values within 0.005 of an integer are displayed as integers.
    const WHOLE_THRESHOLD: f64 = 0.005;
    env.add_filter("fmt_num", |val: f64| -> String {
        if !val.is_finite() {
            return val.to_string();
        }
        if (val - val.round()).abs() < WHOLE_THRESHOLD {
            format!("{}", val.round() as i64)
        } else {
            format!("{:.1}", val)
        }
    });
    env.add_template("base.html", include_str!("templates/base.html"))
        .unwrap();
    env.add_template(
        "pages/home.html",
        include_str!("templates/pages/home.html"),
    )
    .unwrap();
    env.add_template(
        "pages/calendar.html",
        include_str!("templates/pages/calendar.html"),
    )
    .unwrap();
    env.add_template(
        "pages/calendar_day.html",
        include_str!("templates/pages/calendar_day.html"),
    )
    .unwrap();
    env.add_template(
        "pages/query.html",
        include_str!("templates/pages/query.html"),
    )
    .unwrap();
    env.add_template(
        "pages/new_ingredient.html",
        include_str!("templates/pages/new_ingredient.html"),
    )
    .unwrap();
    env.add_template(
        "pages/new_recipe.html",
        include_str!("templates/pages/new_recipe.html"),
    )
    .unwrap();
    env.add_template(
        "pages/new_exercise.html",
        include_str!("templates/pages/new_exercise.html"),
    )
    .unwrap();
    env
}

pub async fn run_server(
    host: String,
    port: u16,
    file_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        file_path: Arc::new(file_path),
        write_lock: Arc::new(tokio::sync::Mutex::new(())),
        cache: Arc::new(tokio::sync::RwLock::new(None)),
        env: Arc::new(build_env()),
    };

    let app = Router::new()
        // Web UI
        .route("/", get(page_home))
        .route("/calendar", get(page_calendar))
        .route("/calendar/{date}", get(page_calendar_day))
        .route("/query", get(page_query))
        .route("/ingredients/new", get(page_new_ingredient))
        .route("/recipes/new", get(page_new_recipe))
        .route("/exercises/new", get(page_new_exercise))
        // Static assets embedded in the binary
        .route("/partials/common.css", get(serve_css))
        .route("/partials/common.js", get(serve_js))
        // API: ingredients
        .route(
            "/api/ingredients",
            get(api_list_ingredients).post(api_create_ingredient),
        )
        .route("/api/ingredients/{alias}", get(api_get_ingredient))
        // API: recipes
        .route(
            "/api/recipes",
            get(api_list_recipes).post(api_create_recipe),
        )
        .route("/api/recipes/{alias}", get(api_get_recipe))
        // API: exercises
        .route(
            "/api/exercises",
            get(api_list_exercises).post(api_create_exercise),
        )
        .route("/api/exercises/{alias}", get(api_get_exercise))
        // API: days
        .route("/api/days", get(api_list_days))
        .route("/api/days/{date}", get(api_get_day))
        .route("/api/days/{date}/ate", post(api_log_ate))
        .route("/api/days/{date}/exercised", post(api_log_exercised))
        .with_state(state);

    let ip = host
        .parse::<IpAddr>()
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let addr = SocketAddr::from((ip, port));
    println!("Serving nutrition tracker on http://{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Static asset handlers ─────────────────────────────────────────────────────

async fn serve_css() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("templates/partials/common.css"),
    )
}

async fn serve_js() -> impl IntoResponse {
    (
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/javascript",
            ),
        ],
        include_str!("templates/partials/common.js"),
    )
}
