//! HTTP handler and web UI for the nutrition server.
//!
//! Exposes a REST API (`/api/…`) and a mobile-responsive HTML web UI that
//! support read and write operations on the nutrition file passed to
//! `nutrition serve`.

use std::io::Write as IoWrite;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::ast::ast::{DayItem, Exercise, Ingredient, Item, Quantity, Recipe};
use crate::cli::file_loader::load_tree;
use crate::emitters::emitter::CanEmit;
use crate::emitters::ingredient::IngredientEmitter;
use crate::emitters::recipe::RecipeEmitter;
use crate::nutrition::{compute_daily_report, query_nutrition, NutritionReport};

// ── Application state ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub file_path: Arc<String>,
    /// Mutex that serialises all file-write operations to prevent corruption
    /// when concurrent requests attempt to modify the nutrition file.
    pub write_lock: Arc<tokio::sync::Mutex<()>>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_doc(state: &AppState) -> Result<crate::ast::ast::Document, (StatusCode, String)> {
    load_tree(Some(state.file_path.as_str()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

/// Append `text` to the file at `file_path`.
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
        // Find the opening brace of this @day block.
        let after_day = &content[day_pos..];
        let brace_offset = after_day
            .find('{')
            .ok_or_else(|| format!("Malformed @day block for '{}'", date))?;
        let abs_open = day_pos + brace_offset;

        // Count braces to find the matching closing brace (simple, ignores
        // strings/comments — sufficient for the @day grammar which has no
        // nested blocks).
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
        // No @day block for this date — append a new one.
        let new_block = format!("\n@day \"{}\" {{\n    {}\n}}\n", date, entry);
        format!("{}{}", content, new_block)
    };

    std::fs::write(file_path, new_content)
        .map_err(|e| format!("Failed to write '{}': {}", file_path, e))
}

/// Serialise an [`Exercise`] to its text representation.
fn exercise_to_text(exercise: &Exercise) -> String {
    let mut s = String::from("@exercise");
    for qty in &exercise.quantities {
        s.push('(');
        s.push_str(&qty.to_string());
        s.push(')');
    }
    s.push(' ');
    for alias in &exercise.aliases {
        s.push('"');
        s.push_str(alias);
        s.push('"');
        s.push(' ');
    }
    if !exercise.properties.is_empty() {
        s.push('{');
        for prop in &exercise.properties {
            s.push('\n');
            s.push_str("    ");
            s.push_str(&prop.name);
            s.push_str(": ");
            s.push_str(&prop.value.to_string());
        }
        s.push('\n');
        s.push('}');
    } else {
        s.push_str("{ }");
    }
    s.push('\n');
    s
}

// ── Request / response types ──────────────────────────────────────────────────

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

async fn api_list_ingredients(State(state): State<AppState>) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    };
    let ingredients: Vec<&Ingredient> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Ingredient(ing) = item {
                Some(ing)
            } else {
                None
            }
        })
        .collect();
    (StatusCode::OK, Json(ingredients)).into_response()
}

async fn api_get_ingredient(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    };
    let ingredient = doc.items.iter().find_map(|item| {
        if let Item::Ingredient(ing) = item {
            if ing.aliases.iter().any(|a| a == &alias) {
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
            let detail = IngredientDetail {
                ingredient: ing,
                nutrition,
            };
            (StatusCode::OK, Json(detail)).into_response()
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
        Ok(()) => (StatusCode::CREATED, Json(ingredient)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Recipes ──────────────────────────────────────────────────────────────

async fn api_list_recipes(State(state): State<AppState>) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    };
    let recipes: Vec<&Recipe> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Recipe(r) = item {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    (StatusCode::OK, Json(recipes)).into_response()
}

async fn api_get_recipe(State(state): State<AppState>, Path(alias): Path<String>) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    };
    let recipe = doc.items.iter().find_map(|item| {
        if let Item::Recipe(r) = item {
            if r.aliases.iter().any(|a| a == &alias) {
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
            let detail = RecipeDetail {
                recipe: rec,
                nutrition,
            };
            (StatusCode::OK, Json(detail)).into_response()
        }
    }
}

async fn api_create_recipe(
    State(state): State<AppState>,
    Json(recipe): Json<Recipe>,
) -> Response {
    let text = RecipeEmitter.emit(&recipe);
    let _guard = state.write_lock.lock().await;
    match append_to_file(&state.file_path, &format!("\n{}", text)) {
        Ok(()) => (StatusCode::CREATED, Json(recipe)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Exercises ────────────────────────────────────────────────────────────

async fn api_list_exercises(State(state): State<AppState>) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    };
    let exercises: Vec<&Exercise> = doc
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Exercise(ex) = item {
                Some(ex)
            } else {
                None
            }
        })
        .collect();
    (StatusCode::OK, Json(exercises)).into_response()
}

async fn api_get_exercise(State(state): State<AppState>, Path(alias): Path<String>) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    };
    let exercise = doc.items.iter().find_map(|item| {
        if let Item::Exercise(ex) = item {
            if ex.aliases.iter().any(|a| a == &alias) {
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
        Some(ex) => {
            let detail = ExerciseDetail { exercise: ex };
            (StatusCode::OK, Json(detail)).into_response()
        }
    }
}

async fn api_create_exercise(
    State(state): State<AppState>,
    Json(exercise): Json<Exercise>,
) -> Response {
    let text = exercise_to_text(&exercise);
    let _guard = state.write_lock.lock().await;
    match append_to_file(&state.file_path, &format!("\n{}", text)) {
        Ok(()) => (StatusCode::CREATED, Json(exercise)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Days ─────────────────────────────────────────────────────────────────

async fn api_list_days(State(state): State<AppState>) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    };
    let days: Vec<DayListEntry> = doc
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
    (StatusCode::OK, Json(days)).into_response()
}

async fn api_get_day(State(state): State<AppState>, Path(date): Path<String>) -> Response {
    let doc = match load_doc(&state) {
        Ok(d) => d,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
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
            // Return empty report for dates with no @day block yet.
            let empty = crate::nutrition::DailyNutritionReport {
                date: date.clone(),
                intake: vec![],
                exercise: vec![],
                net: vec![],
            };
            let detail = DayDetailResponse {
                date,
                items: vec![],
                report: empty,
            };
            (StatusCode::OK, Json(detail)).into_response()
        }
        Some(day) => {
            let report = compute_daily_report(&doc, &day);
            let items: Vec<serde_json::Value> = day
                .items
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
                .collect();
            let detail = DayDetailResponse {
                date: day.date,
                items,
                report,
            };
            (StatusCode::OK, Json(detail)).into_response()
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
    let entry = format!("@ate \"{}\"({})", body.alias, qty.to_string());
    let _guard = state.write_lock.lock().await;
    match add_entry_to_day(&state.file_path, &date, &entry) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "date": date, "alias": body.alias, "quantity": body.quantity })),
        )
            .into_response(),
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
    let entry = format!(
        "@exercised \"{}\"({})",
        body.alias,
        qty.to_string()
    );
    let _guard = state.write_lock.lock().await;
    match add_entry_to_day(&state.file_path, &date, &entry) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "date": date, "alias": body.alias, "quantity": body.quantity })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── Web UI pages ──────────────────────────────────────────────────────────────

/// Inject the shared CSS and JavaScript into an HTML page template.
fn with_css(template: &str) -> String {
    template
        .replace("COMMON_CSS_PLACEHOLDER", COMMON_CSS)
        .replace("COMMON_JS_PLACEHOLDER", COMMON_JS)
}

async fn page_home() -> Html<String> {
    Html(with_css(HOME_PAGE))
}

async fn page_calendar() -> Html<String> {
    Html(with_css(CALENDAR_PAGE))
}

async fn page_query() -> Html<String> {
    Html(with_css(QUERY_PAGE))
}

async fn page_new_ingredient() -> Html<String> {
    Html(with_css(NEW_INGREDIENT_PAGE))
}

async fn page_new_recipe() -> Html<String> {
    Html(with_css(NEW_RECIPE_PAGE))
}

async fn page_new_exercise() -> Html<String> {
    Html(with_css(NEW_EXERCISE_PAGE))
}

// ── run_server ────────────────────────────────────────────────────────────────

pub async fn run_server(
    port: u16,
    file_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        file_path: Arc::new(file_path),
        write_lock: Arc::new(tokio::sync::Mutex::new(())),
    };

    let app = Router::new()
        // Web UI
        .route("/", get(page_home))
        .route("/calendar", get(page_calendar))
        .route("/query", get(page_query))
        .route("/ingredients/new", get(page_new_ingredient))
        .route("/recipes/new", get(page_new_recipe))
        .route("/exercises/new", get(page_new_exercise))
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

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Serving nutrition tracker on http://127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Shared CSS ────────────────────────────────────────────────────────────────

// ── Shared JavaScript utilities ───────────────────────────────────────────────

/// JavaScript utilities shared across all web UI pages, injected via
/// `COMMON_JS_PLACEHOLDER` in each HTML template.
const COMMON_JS: &str = r##"
function fmt(amount, unit) {
  const a = Math.abs(amount - Math.round(amount)) < 0.005 ? Math.round(amount) : amount.toFixed(1);
  return unit ? `${a} ${unit}` : `${a}`;
}

function parseQuantityStr(s) {
  s = (s || '').trim();
  const m = s.match(/^([0-9]+(?:\.[0-9]+)?)\s*(.*)$/);
  if (!m) return null;
  return { amount: parseFloat(m[1]), unit: m[2].trim() || null };
}

function addRemoveHandlers() {
  document.querySelectorAll('.remove-btn').forEach(btn => {
    btn.onclick = () => {
      const list = btn.closest('.repeatable-list');
      if (list.querySelectorAll('.repeatable-item').length > 1) btn.closest('.repeatable-item').remove();
    };
  });
}
"##;

const COMMON_CSS: &str = r##"
:root {
  --bg: #0f0f14;
  --surface: #1a1a2e;
  --surface2: #22223a;
  --border: #2e2e50;
  --accent: #6c63ff;
  --accent2: #a78bfa;
  --text: #e0e0f0;
  --text-dim: #8888aa;
  --success: #4ade80;
  --danger: #f87171;
  --warning: #fbbf24;
  --radius: 10px;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: var(--bg);
  color: var(--text);
  min-height: 100vh;
  font-size: 16px;
  line-height: 1.5;
}
a { color: var(--accent2); text-decoration: none; }
a:hover { text-decoration: underline; }
nav {
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  padding: 0.75rem 1rem;
  display: flex;
  align-items: center;
  gap: 1rem;
  flex-wrap: wrap;
  position: sticky;
  top: 0;
  z-index: 10;
}
.nav-brand {
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--accent2);
  white-space: nowrap;
}
.nav-links {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}
.nav-links a {
  color: var(--text-dim);
  padding: 0.25rem 0.6rem;
  border-radius: 6px;
  font-size: 0.9rem;
  transition: background 0.15s, color 0.15s;
}
.nav-links a:hover, .nav-links a.active {
  background: var(--surface2);
  color: var(--text);
  text-decoration: none;
}
.nav-links a.cta {
  background: var(--accent);
  color: #fff;
}
.nav-links a.cta:hover { background: var(--accent2); }
main {
  max-width: 840px;
  margin: 0 auto;
  padding: 1.5rem 1rem 3rem;
}
h1 { font-size: 1.6rem; margin-bottom: 0.5rem; }
h2 { font-size: 1.25rem; margin-bottom: 0.75rem; color: var(--accent2); }
h3 { font-size: 1.05rem; margin-bottom: 0.5rem; color: var(--text-dim); font-weight: 600; }
.card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 1rem 1.25rem;
  margin-bottom: 1rem;
}
.card h2 { margin-bottom: 0.5rem; }
.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 0.75rem;
  margin-top: 0.5rem;
}
.stat-card {
  background: var(--surface2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 0.75rem;
  text-align: center;
}
.stat-value {
  font-size: 1.5rem;
  font-weight: 700;
  color: var(--accent2);
}
.stat-label {
  font-size: 0.8rem;
  color: var(--text-dim);
  margin-top: 0.1rem;
}
form { display: flex; flex-direction: column; gap: 0.75rem; }
.form-row { display: flex; gap: 0.5rem; flex-wrap: wrap; }
.form-row input, .form-row select { flex: 1; min-width: 120px; }
label { font-size: 0.875rem; color: var(--text-dim); margin-bottom: 0.1rem; display: block; }
input, select, textarea {
  background: var(--surface2);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text);
  padding: 0.5rem 0.75rem;
  font-size: 0.95rem;
  width: 100%;
  outline: none;
  transition: border-color 0.15s;
}
input:focus, select:focus, textarea:focus {
  border-color: var(--accent);
}
input::placeholder { color: var(--text-dim); }
button, .btn {
  background: var(--accent);
  color: #fff;
  border: none;
  border-radius: 6px;
  padding: 0.55rem 1.1rem;
  font-size: 0.95rem;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s;
  white-space: nowrap;
}
button:hover, .btn:hover { background: var(--accent2); }
button.secondary {
  background: var(--surface2);
  color: var(--text);
  border: 1px solid var(--border);
}
button.secondary:hover { background: var(--border); }
button.danger { background: var(--danger); }
.log-list { list-style: none; }
.log-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0;
  border-bottom: 1px solid var(--border);
  font-size: 0.95rem;
}
.log-item:last-child { border-bottom: none; }
.log-item .alias { font-weight: 600; }
.log-item .qty { color: var(--text-dim); }
.log-item .cal { color: var(--accent2); margin-left: auto; font-size: 0.85rem; }
.badge {
  display: inline-block;
  padding: 0.1rem 0.5rem;
  border-radius: 20px;
  font-size: 0.75rem;
  font-weight: 600;
}
.badge-ate { background: #1e3a2a; color: var(--success); }
.badge-exercised { background: #2a1e3a; color: var(--accent2); }
.badge-meal { background: #2a2a1e; color: var(--warning); }
.alert {
  padding: 0.75rem 1rem;
  border-radius: var(--radius);
  font-size: 0.9rem;
  margin-top: 0.5rem;
}
.alert-success { background: #1e3a2a; color: var(--success); border: 1px solid #2d5a40; }
.alert-error { background: #3a1e1e; color: var(--danger); border: 1px solid #5a2d2d; }
.repeatable-list { display: flex; flex-direction: column; gap: 0.5rem; }
.repeatable-item { display: flex; gap: 0.5rem; align-items: center; }
.repeatable-item input { flex: 1; }
.section-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
  border-bottom: 1px solid var(--border);
  padding-bottom: 0.4rem;
}
.empty-state {
  color: var(--text-dim);
  text-align: center;
  padding: 2rem;
  font-style: italic;
}
.day-link {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.65rem 0.9rem;
  background: var(--surface2);
  border-radius: 6px;
  margin-bottom: 0.4rem;
  color: var(--text);
  transition: background 0.15s;
}
.day-link:hover { background: var(--border); text-decoration: none; }
.day-link .day-date { font-weight: 600; }
.day-link .day-meta { color: var(--text-dim); font-size: 0.85rem; }
@media (max-width: 500px) {
  .stats-grid { grid-template-columns: repeat(2, 1fr); }
  .form-row { flex-direction: column; }
  nav { padding: 0.5rem; }
}
"##;

// ── HTML page: Home ───────────────────────────────────────────────────────────

const HOME_PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Nutrition Tracker</title>
  <style>COMMON_CSS_PLACEHOLDER</style>
  <script>COMMON_JS_PLACEHOLDER</script>
</head>
<body>
<nav>
  <div class="nav-brand">🥗 Nutrition</div>
  <div class="nav-links">
    <a href="/" class="active">Today</a>
    <a href="/calendar">Calendar</a>
    <a href="/query">Search</a>
    <a href="/ingredients/new" class="cta">+ Food</a>
    <a href="/recipes/new" class="cta">+ Recipe</a>
    <a href="/exercises/new" class="cta">+ Exercise</a>
  </div>
</nav>
<main>
  <div style="display:flex;align-items:baseline;gap:0.75rem;margin-bottom:1rem;">
    <h1>Today</h1>
    <span id="today-date" style="color:var(--text-dim);font-size:1rem;"></span>
  </div>

  <div class="card" id="stats-card">
    <h2>📊 Nutrition Summary</h2>
    <div id="stats-content">
      <p class="empty-state">Loading…</p>
    </div>
  </div>

  <div class="card">
    <h2>🍽 Log Food</h2>
    <form id="log-food-form">
      <div class="form-row">
        <div style="flex:2;min-width:160px;">
          <label for="food-alias">Food / Recipe name</label>
          <input type="text" id="food-alias" placeholder="e.g. chickpeas" list="food-options" autocomplete="off" required>
          <datalist id="food-options"></datalist>
        </div>
        <div style="flex:1;min-width:110px;">
          <label for="food-qty">Quantity</label>
          <input type="text" id="food-qty" placeholder="e.g. 200g" required>
        </div>
        <div style="display:flex;align-items:flex-end;">
          <button type="submit">Log</button>
        </div>
      </div>
      <div id="food-msg"></div>
    </form>
  </div>

  <div class="card">
    <h2>🏃 Log Exercise</h2>
    <form id="log-exercise-form">
      <div class="form-row">
        <div style="flex:2;min-width:160px;">
          <label for="exercise-alias">Exercise name</label>
          <input type="text" id="exercise-alias" placeholder="e.g. running" list="exercise-options" autocomplete="off" required>
          <datalist id="exercise-options"></datalist>
        </div>
        <div style="flex:1;min-width:110px;">
          <label for="exercise-qty">Duration / Quantity</label>
          <input type="text" id="exercise-qty" placeholder="e.g. 30 min" required>
        </div>
        <div style="display:flex;align-items:flex-end;">
          <button type="submit">Log</button>
        </div>
      </div>
      <div id="exercise-msg"></div>
    </form>
  </div>

  <div class="card">
    <h2>📋 Today's Log</h2>
    <div id="today-log">
      <p class="empty-state">Loading…</p>
    </div>
  </div>
</main>
<script>
const todayDate = new Date().toISOString().slice(0, 10);
document.getElementById('today-date').textContent = new Date().toLocaleDateString(undefined, { weekday:'long', year:'numeric', month:'long', day:'numeric' });

async function loadDay() {
  const res = await fetch(`/api/days/${todayDate}`);
  const data = await res.json();

  // Stats
  const statsEl = document.getElementById('stats-content');
  const intake = data.report && data.report.intake ? data.report.intake : [];
  const net    = data.report && data.report.net    ? data.report.net    : [];

  if (intake.length === 0) {
    statsEl.innerHTML = '<p class="empty-state">No food logged yet today.</p>';
  } else {
    const priority = ['calories','protein','fat','carbohydrates','carbs','fiber','sugar'];
    const sorted = [...intake].sort((a,b) => {
      const ai = priority.indexOf(a.name), bi = priority.indexOf(b.name);
      if (ai>=0 && bi>=0) return ai-bi;
      if (ai>=0) return -1;
      if (bi>=0) return 1;
      return a.name.localeCompare(b.name);
    });
    statsEl.innerHTML = `<div class="stats-grid">${sorted.map(p =>
      `<div class="stat-card"><div class="stat-value">${fmt(p.value.amount, p.value.unit)}</div><div class="stat-label">${p.name}</div></div>`
    ).join('')}</div>`;
  }

  // Log
  const logEl = document.getElementById('today-log');
  const items = data.items || [];
  if (items.length === 0) {
    logEl.innerHTML = '<p class="empty-state">Nothing logged yet.</p>';
  } else {
    logEl.innerHTML = `<ul class="log-list">${items.map(item => {
      if (item.type === 'ate') {
        return `<li class="log-item"><span class="badge badge-ate">ate</span><span class="alias">${item.alias}</span><span class="qty">${fmt(item.quantity.amount, item.quantity.unit)}</span></li>`;
      } else if (item.type === 'exercised') {
        return `<li class="log-item"><span class="badge badge-exercised">exercised</span><span class="alias">${item.alias}</span><span class="qty">${fmt(item.quantity.amount, item.quantity.unit)}</span></li>`;
      } else {
        return `<li class="log-item"><span class="badge badge-meal">meal</span><span class="alias">${item.label}</span></li>`;
      }
    }).join('')}</ul>`;
  }
}

async function loadAutocomplete() {
  const [ingRes, exRes] = await Promise.all([
    fetch('/api/ingredients'),
    fetch('/api/exercises')
  ]);
  const [ings, exs] = await Promise.all([ingRes.json(), exRes.json()]);

  // Also load recipes for food autocomplete
  const recRes = await fetch('/api/recipes');
  const recs = await recRes.json();

  const foodOptions = document.getElementById('food-options');
  const exOptions   = document.getElementById('exercise-options');

  const foodAliases = new Set();
  (ings || []).forEach(ing => (ing.aliases||[]).forEach(a => foodAliases.add(a)));
  (recs || []).forEach(rec => (rec.aliases||[]).forEach(a => foodAliases.add(a)));
  foodOptions.innerHTML = [...foodAliases].map(a => `<option value="${a}">`).join('');

  const exAliases = new Set();
  (exs || []).forEach(ex => (ex.aliases||[]).forEach(a => exAliases.add(a)));
  exOptions.innerHTML = [...exAliases].map(a => `<option value="${a}">`).join('');
}

function showMsg(id, msg, isError) {
  const el = document.getElementById(id);
  el.innerHTML = `<div class="alert ${isError ? 'alert-error' : 'alert-success'}">${msg}</div>`;
  setTimeout(() => el.innerHTML = '', 3000);
}

document.getElementById('log-food-form').addEventListener('submit', async e => {
  e.preventDefault();
  const alias = document.getElementById('food-alias').value.trim();
  const qty   = document.getElementById('food-qty').value.trim();
  const res = await fetch(`/api/days/${todayDate}/ate`, {
    method: 'POST',
    headers: {'Content-Type':'application/json'},
    body: JSON.stringify({ alias, quantity: qty })
  });
  if (res.ok) {
    showMsg('food-msg', `Logged ${alias} (${qty})`, false);
    document.getElementById('food-alias').value = '';
    document.getElementById('food-qty').value = '';
    loadDay();
  } else {
    const err = await res.json();
    showMsg('food-msg', err.error || 'Error logging food', true);
  }
});

document.getElementById('log-exercise-form').addEventListener('submit', async e => {
  e.preventDefault();
  const alias = document.getElementById('exercise-alias').value.trim();
  const qty   = document.getElementById('exercise-qty').value.trim();
  const res = await fetch(`/api/days/${todayDate}/exercised`, {
    method: 'POST',
    headers: {'Content-Type':'application/json'},
    body: JSON.stringify({ alias, quantity: qty })
  });
  if (res.ok) {
    showMsg('exercise-msg', `Logged ${alias} (${qty})`, false);
    document.getElementById('exercise-alias').value = '';
    document.getElementById('exercise-qty').value = '';
    loadDay();
  } else {
    const err = await res.json();
    showMsg('exercise-msg', err.error || 'Error logging exercise', true);
  }
});

loadDay();
loadAutocomplete();
</script>
</body>
</html>"##;

// ── HTML page: Calendar ───────────────────────────────────────────────────────

const CALENDAR_PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Calendar – Nutrition Tracker</title>
  <style>COMMON_CSS_PLACEHOLDER</style>
  <script>COMMON_JS_PLACEHOLDER</script>
</head>
<body>
<nav>
  <div class="nav-brand">🥗 Nutrition</div>
  <div class="nav-links">
    <a href="/">Today</a>
    <a href="/calendar" class="active">Calendar</a>
    <a href="/query">Search</a>
    <a href="/ingredients/new" class="cta">+ Food</a>
    <a href="/recipes/new" class="cta">+ Recipe</a>
    <a href="/exercises/new" class="cta">+ Exercise</a>
  </div>
</nav>
<main>
  <h1>📅 Calendar</h1>
  <p style="color:var(--text-dim);margin-bottom:1rem;">All logged days, most recent first.</p>
  <div id="days-list"><p class="empty-state">Loading…</p></div>
</main>
<script>
async function loadCalendar() {
  const res = await fetch('/api/days');
  const days = await res.json();
  const el = document.getElementById('days-list');
  if (!days || days.length === 0) {
    el.innerHTML = '<p class="empty-state">No days logged yet.</p>';
    return;
  }
  const sorted = [...days].sort((a,b) => b.date.localeCompare(a.date));
  const today = new Date().toISOString().slice(0,10);

  // Fetch today's report for highlighting
  el.innerHTML = sorted.map(d => {
    const isToday = d.date === today;
    const todayBadge = isToday ? ' <span class="badge badge-ate">today</span>' : '';
    const entriesStr = `${d.items_count} entr${d.items_count === 1 ? 'y' : 'ies'}`;
    return `<a href="/calendar/${d.date}" class="day-link">
      <span class="day-date">${d.date}${todayBadge}</span>
      <span class="day-meta">${entriesStr}</span>
    </a>`;
  }).join('');
}

loadCalendar();
</script>
</body>
</html>"##;

// ── HTML page: Query ──────────────────────────────────────────────────────────

const QUERY_PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Search – Nutrition Tracker</title>
  <style>COMMON_CSS_PLACEHOLDER</style>
  <script>COMMON_JS_PLACEHOLDER</script>
</head>
<body>
<nav>
  <div class="nav-brand">🥗 Nutrition</div>
  <div class="nav-links">
    <a href="/">Today</a>
    <a href="/calendar">Calendar</a>
    <a href="/query" class="active">Search</a>
    <a href="/ingredients/new" class="cta">+ Food</a>
    <a href="/recipes/new" class="cta">+ Recipe</a>
    <a href="/exercises/new" class="cta">+ Exercise</a>
  </div>
</nav>
<main>
  <h1>🔍 Search</h1>

  <div class="card">
    <h2>Foods &amp; Recipes</h2>
    <form id="food-search-form">
      <div class="form-row">
        <input type="text" id="food-query" placeholder="Name or alias…" list="all-food-options" autocomplete="off" required>
        <datalist id="all-food-options"></datalist>
        <button type="submit">Search</button>
      </div>
    </form>
    <div id="food-result" style="margin-top:1rem;"></div>
  </div>

  <div class="card">
    <h2>Exercises</h2>
    <form id="ex-search-form">
      <div class="form-row">
        <input type="text" id="ex-query" placeholder="Exercise name…" list="all-ex-options" autocomplete="off" required>
        <datalist id="all-ex-options"></datalist>
        <button type="submit">Search</button>
      </div>
    </form>
    <div id="ex-result" style="margin-top:1rem;"></div>
  </div>

  <div class="card">
    <h2>All Foods &amp; Recipes</h2>
    <div id="all-foods"><p class="empty-state">Loading…</p></div>
  </div>

  <div class="card">
    <h2>All Exercises</h2>
    <div id="all-exercises"><p class="empty-state">Loading…</p></div>
  </div>
</main>
<script>
function renderNutrition(report) {
  if (!report || !report.properties || report.properties.length === 0) return '<em>No nutritional data.</em>';
  const priority = ['calories','protein','fat','carbohydrates','carbs','fiber','sugar','sodium'];
  const sorted = [...report.properties].sort((a,b) => {
    const ai = priority.indexOf(a.name), bi = priority.indexOf(b.name);
    if (ai>=0 && bi>=0) return ai-bi;
    if (ai>=0) return -1; if (bi>=0) return 1;
    return a.name.localeCompare(b.name);
  });
  return `<div class="stats-grid">${sorted.map(p =>
    `<div class="stat-card"><div class="stat-value">${fmt(p.value.amount, p.value.unit)}</div><div class="stat-label">${p.name}</div></div>`
  ).join('')}</div><p style="color:var(--text-dim);font-size:0.8rem;margin-top:0.5rem;">per ${fmt(report.quantity.amount, report.quantity.unit)}</p>`;
}

async function loadAll() {
  const [ingRes, recRes, exRes] = await Promise.all([
    fetch('/api/ingredients'), fetch('/api/recipes'), fetch('/api/exercises')
  ]);
  const [ings, recs, exs] = await Promise.all([ingRes.json(), recRes.json(), exRes.json()]);

  const foodOpts = document.getElementById('all-food-options');
  const exOpts   = document.getElementById('all-ex-options');

  const foodAliases = new Set();
  (ings||[]).forEach(i => (i.aliases||[]).forEach(a => foodAliases.add(a)));
  (recs||[]).forEach(r => (r.aliases||[]).forEach(a => foodAliases.add(a)));
  foodOpts.innerHTML = [...foodAliases].map(a => `<option value="${a}">`).join('');

  const exAliases = new Set();
  (exs||[]).forEach(e => (e.aliases||[]).forEach(a => exAliases.add(a)));
  exOpts.innerHTML = [...exAliases].map(a => `<option value="${a}">`).join('');

  // List all foods
  const foodsEl = document.getElementById('all-foods');
  const allFoods = [
    ...(ings||[]).map(i => ({ type:'ingredient', primary: (i.aliases||[])[0]||'?', aliases: i.aliases||[], qty: i.quantities&&i.quantities[0] })),
    ...(recs||[]).map(r => ({ type:'recipe',     primary: (r.aliases||[])[0]||'?', aliases: r.aliases||[], qty: r.quantities&&r.quantities[0] }))
  ];
  if (allFoods.length === 0) {
    foodsEl.innerHTML = '<p class="empty-state">None defined yet.</p>';
  } else {
    foodsEl.innerHTML = `<ul class="log-list">${allFoods.map(f => {
      const qtyStr = f.qty ? fmt(f.qty.amount, f.qty.unit) : '';
      const badge = f.type === 'recipe' ? '<span class="badge badge-meal" style="margin-right:0.3rem">recipe</span>' : '';
      return `<li class="log-item">${badge}<a href="#" class="alias food-link" data-alias="${f.primary}">${f.primary}</a><span class="qty">${f.aliases.slice(1).join(', ')}</span><span class="cal">${qtyStr}</span></li>`;
    }).join('')}</ul>`;
  }

  // List all exercises
  const exsEl = document.getElementById('all-exercises');
  if (!exs || exs.length === 0) {
    exsEl.innerHTML = '<p class="empty-state">None defined yet.</p>';
  } else {
    exsEl.innerHTML = `<ul class="log-list">${exs.map(e => {
      const primary = (e.aliases||[])[0]||'?';
      const qtyStr = e.quantities&&e.quantities[0] ? fmt(e.quantities[0].amount, e.quantities[0].unit) : '';
      return `<li class="log-item"><a href="#" class="alias ex-link" data-alias="${primary}">${primary}</a><span class="qty">${(e.aliases||[]).slice(1).join(', ')}</span><span class="cal">${qtyStr}</span></li>`;
    }).join('')}</ul>`;
  }

  document.querySelectorAll('.food-link').forEach(el => {
    el.addEventListener('click', async ev => {
      ev.preventDefault();
      await searchFood(ev.target.dataset.alias);
      document.getElementById('food-query').value = ev.target.dataset.alias;
    });
  });
  document.querySelectorAll('.ex-link').forEach(el => {
    el.addEventListener('click', async ev => {
      ev.preventDefault();
      await searchExercise(ev.target.dataset.alias);
      document.getElementById('ex-query').value = ev.target.dataset.alias;
    });
  });
}

async function searchFood(alias) {
  const res = await fetch(`/api/ingredients/${encodeURIComponent(alias)}`);
  const resultEl = document.getElementById('food-result');
  if (res.ok) {
    const data = await res.json();
    const qtyStr = data.ingredient.quantities.map(q => fmt(q.amount, q.unit)).join(' / ');
    resultEl.innerHTML = `<div class="card" style="margin:0">
      <h3>${data.ingredient.aliases.join(' / ')} <span style="font-weight:400;color:var(--text-dim)">(${qtyStr})</span></h3>
      ${renderNutrition(data.nutrition)}
    </div>`;
  } else {
    // Try as recipe
    const rres = await fetch(`/api/recipes/${encodeURIComponent(alias)}`);
    if (rres.ok) {
      const data = await rres.json();
      const qtyStr = data.recipe.quantities.map(q => fmt(q.amount, q.unit)).join(' / ');
      resultEl.innerHTML = `<div class="card" style="margin:0">
        <h3>${data.recipe.aliases.join(' / ')} <span style="font-weight:400;color:var(--text-dim)">(${qtyStr})</span></h3>
        ${renderNutrition(data.nutrition)}
      </div>`;
    } else {
      resultEl.innerHTML = `<div class="alert alert-error">No ingredient or recipe named "${alias}" found.</div>`;
    }
  }
}

async function searchExercise(alias) {
  const res = await fetch(`/api/exercises/${encodeURIComponent(alias)}`);
  const resultEl = document.getElementById('ex-result');
  if (res.ok) {
    const data = await res.json();
    const qtyStr = data.exercise.quantities.map(q => fmt(q.amount, q.unit)).join(' / ');
    const propsHtml = data.exercise.properties.length > 0
      ? `<div class="stats-grid">${data.exercise.properties.map(p =>
          `<div class="stat-card"><div class="stat-value">${fmt(p.value.amount, p.value.unit)}</div><div class="stat-label">${p.name}</div></div>`
        ).join('')}</div><p style="color:var(--text-dim);font-size:0.8rem;margin-top:0.5rem;">per ${qtyStr}</p>`
      : '<em>No properties defined.</em>';
    resultEl.innerHTML = `<div class="card" style="margin:0"><h3>${data.exercise.aliases.join(' / ')}</h3>${propsHtml}</div>`;
  } else {
    resultEl.innerHTML = `<div class="alert alert-error">No exercise named "${alias}" found.</div>`;
  }
}

document.getElementById('food-search-form').addEventListener('submit', async e => {
  e.preventDefault();
  await searchFood(document.getElementById('food-query').value.trim());
});

document.getElementById('ex-search-form').addEventListener('submit', async e => {
  e.preventDefault();
  await searchExercise(document.getElementById('ex-query').value.trim());
});

loadAll();
</script>
</body>
</html>"##;

// ── HTML page: New Ingredient ─────────────────────────────────────────────────

const NEW_INGREDIENT_PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>New Food – Nutrition Tracker</title>
  <style>COMMON_CSS_PLACEHOLDER</style>
  <script>COMMON_JS_PLACEHOLDER</script>
</head>
<body>
<nav>
  <div class="nav-brand">🥗 Nutrition</div>
  <div class="nav-links">
    <a href="/">Today</a>
    <a href="/calendar">Calendar</a>
    <a href="/query">Search</a>
    <a href="/ingredients/new" class="cta active">+ Food</a>
    <a href="/recipes/new" class="cta">+ Recipe</a>
    <a href="/exercises/new" class="cta">+ Exercise</a>
  </div>
</nav>
<main>
  <h1>🥦 New Food / Ingredient</h1>
  <p style="color:var(--text-dim);margin-bottom:1rem;">Define a new ingredient with its nutritional data.</p>

  <div class="card">
    <form id="ing-form">
      <div>
        <div class="section-title"><h3>Names / Aliases</h3></div>
        <p style="color:var(--text-dim);font-size:0.85rem;margin-bottom:0.5rem;">Add one or more names (e.g. "chickpeas", "garbanzo beans").</p>
        <div class="repeatable-list" id="aliases-list">
          <div class="repeatable-item">
            <input type="text" placeholder="e.g. chickpeas" required>
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-alias" style="margin-top:0.5rem;">+ Add alias</button>
      </div>

      <div>
        <div class="section-title"><h3>Serving Sizes</h3></div>
        <p style="color:var(--text-dim);font-size:0.85rem;margin-bottom:0.5rem;">e.g. "100g" or "1 cup"</p>
        <div class="repeatable-list" id="quantities-list">
          <div class="repeatable-item">
            <input type="text" placeholder="e.g. 100g" required>
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-quantity" style="margin-top:0.5rem;">+ Add serving size</button>
      </div>

      <div>
        <div class="section-title"><h3>Nutritional Properties</h3></div>
        <p style="color:var(--text-dim);font-size:0.85rem;margin-bottom:0.5rem;">e.g. calories: 269, protein: 14.5g</p>
        <div class="repeatable-list" id="props-list">
          <div class="repeatable-item">
            <input type="text" placeholder="Property (e.g. calories)" style="flex:1.2">
            <input type="text" placeholder="Value (e.g. 269kcal)" style="flex:1">
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-prop" style="margin-top:0.5rem;">+ Add property</button>
      </div>

      <div id="form-msg"></div>
      <button type="submit" style="align-self:flex-start;">Save Ingredient</button>
    </form>
  </div>
</main>
<script>
addRemoveHandlers();

document.getElementById('add-alias').onclick = () => {
  const item = document.createElement('div');
  item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="e.g. garbanzo beans"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('aliases-list').appendChild(item);
  addRemoveHandlers();
};

document.getElementById('add-quantity').onclick = () => {
  const item = document.createElement('div');
  item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="e.g. 1 cup"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('quantities-list').appendChild(item);
  addRemoveHandlers();
};

document.getElementById('add-prop').onclick = () => {
  const item = document.createElement('div');
  item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="Property (e.g. protein)" style="flex:1.2"><input type="text" placeholder="Value (e.g. 14.5g)" style="flex:1"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('props-list').appendChild(item);
  addRemoveHandlers();
};

document.getElementById('ing-form').addEventListener('submit', async e => {
  e.preventDefault();
  const msgEl = document.getElementById('form-msg');

  const aliases = [...document.getElementById('aliases-list').querySelectorAll('input')].map(i => i.value.trim()).filter(Boolean);
  const quantities = [...document.getElementById('quantities-list').querySelectorAll('input')].map(i => parseQuantityStr(i.value)).filter(Boolean);

  const propItems = [...document.getElementById('props-list').querySelectorAll('.repeatable-item')];
  const properties = propItems.map(item => {
    const inputs = item.querySelectorAll('input');
    const name = inputs[0].value.trim();
    const val  = parseQuantityStr(inputs[1].value);
    if (!name || !val) return null;
    return { name, value: val };
  }).filter(Boolean);

  if (aliases.length === 0) { msgEl.innerHTML = '<div class="alert alert-error">At least one alias is required.</div>'; return; }
  if (quantities.length === 0) { msgEl.innerHTML = '<div class="alert alert-error">At least one serving size is required.</div>'; return; }

  const body = { aliases, quantities, properties };
  const res = await fetch('/api/ingredients', {
    method: 'POST',
    headers: {'Content-Type':'application/json'},
    body: JSON.stringify(body)
  });
  if (res.ok) {
    msgEl.innerHTML = `<div class="alert alert-success">✓ Ingredient "${aliases[0]}" saved! <a href="/query">Search</a> or <a href="/">Log food</a></div>`;
    e.target.reset();
  } else {
    const err = await res.json();
    msgEl.innerHTML = `<div class="alert alert-error">${err.error || 'Error saving ingredient'}</div>`;
  }
});
</script>
</body>
</html>"##;

// ── HTML page: New Recipe ─────────────────────────────────────────────────────

const NEW_RECIPE_PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>New Recipe – Nutrition Tracker</title>
  <style>COMMON_CSS_PLACEHOLDER</style>
  <script>COMMON_JS_PLACEHOLDER</script>
</head>
<body>
<nav>
  <div class="nav-brand">🥗 Nutrition</div>
  <div class="nav-links">
    <a href="/">Today</a>
    <a href="/calendar">Calendar</a>
    <a href="/query">Search</a>
    <a href="/ingredients/new" class="cta">+ Food</a>
    <a href="/recipes/new" class="cta active">+ Recipe</a>
    <a href="/exercises/new" class="cta">+ Exercise</a>
  </div>
</nav>
<main>
  <h1>📖 New Recipe</h1>
  <p style="color:var(--text-dim);margin-bottom:1rem;">Combine ingredients into a named recipe.</p>

  <div class="card">
    <form id="recipe-form">
      <div>
        <div class="section-title"><h3>Recipe Name(s)</h3></div>
        <div class="repeatable-list" id="aliases-list">
          <div class="repeatable-item">
            <input type="text" placeholder="e.g. chickpea stew" required>
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-alias" style="margin-top:0.5rem;">+ Add alias</button>
      </div>

      <div>
        <div class="section-title"><h3>Yield / Servings</h3></div>
        <p style="color:var(--text-dim);font-size:0.85rem;margin-bottom:0.5rem;">How much does this recipe make? e.g. "4 servings", "500g"</p>
        <div class="repeatable-list" id="quantities-list">
          <div class="repeatable-item">
            <input type="text" placeholder="e.g. 4 servings" required>
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-quantity" style="margin-top:0.5rem;">+ Add quantity</button>
      </div>

      <div>
        <div class="section-title"><h3>Ingredients</h3></div>
        <p style="color:var(--text-dim);font-size:0.85rem;margin-bottom:0.5rem;">Use ingredient aliases already defined in your file.</p>
        <div class="repeatable-list" id="ings-list">
          <div class="repeatable-item">
            <input type="text" placeholder="Ingredient alias" list="ing-options" style="flex:1.5" autocomplete="off">
            <input type="text" placeholder="Quantity (e.g. 200g)" style="flex:1">
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <datalist id="ing-options"></datalist>
        <button type="button" class="secondary" id="add-ing" style="margin-top:0.5rem;">+ Add ingredient</button>
      </div>

      <div id="form-msg"></div>
      <button type="submit" style="align-self:flex-start;">Save Recipe</button>
    </form>
  </div>
</main>
<script>
async function loadIngredientOptions() {
  const res = await fetch('/api/ingredients');
  const ings = await res.json();
  const opts = document.getElementById('ing-options');
  const aliases = new Set();
  (ings||[]).forEach(i => (i.aliases||[]).forEach(a => aliases.add(a)));
  opts.innerHTML = [...aliases].map(a => `<option value="${a}">`).join('');
}
loadIngredientOptions();

addRemoveHandlers();

document.getElementById('add-alias').onclick = () => {
  const item = document.createElement('div'); item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="e.g. stew"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('aliases-list').appendChild(item); addRemoveHandlers();
};
document.getElementById('add-quantity').onclick = () => {
  const item = document.createElement('div'); item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="e.g. 8 servings"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('quantities-list').appendChild(item); addRemoveHandlers();
};
document.getElementById('add-ing').onclick = () => {
  const item = document.createElement('div'); item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="Ingredient alias" list="ing-options" style="flex:1.5" autocomplete="off"><input type="text" placeholder="Quantity (e.g. 100g)" style="flex:1"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('ings-list').appendChild(item); addRemoveHandlers();
};

document.getElementById('recipe-form').addEventListener('submit', async e => {
  e.preventDefault();
  const msgEl = document.getElementById('form-msg');

  const aliases = [...document.getElementById('aliases-list').querySelectorAll('input')].map(i => i.value.trim()).filter(Boolean);
  const quantities = [...document.getElementById('quantities-list').querySelectorAll('input')].map(i => parseQuantityStr(i.value)).filter(Boolean);

  const ingItems = [...document.getElementById('ings-list').querySelectorAll('.repeatable-item')];
  const ingredients = ingItems.map(item => {
    const inputs = item.querySelectorAll('input');
    const alias = inputs[0].value.trim();
    const qty = parseQuantityStr(inputs[1].value);
    if (!alias || !qty) return null;
    return { alias, quantity: qty };
  }).filter(Boolean);

  if (aliases.length === 0) { msgEl.innerHTML = '<div class="alert alert-error">At least one name is required.</div>'; return; }
  if (quantities.length === 0) { msgEl.innerHTML = '<div class="alert alert-error">At least one serving quantity is required.</div>'; return; }

  const body = { aliases, quantities, ingredients };
  const res = await fetch('/api/recipes', {
    method: 'POST',
    headers: {'Content-Type':'application/json'},
    body: JSON.stringify(body)
  });
  if (res.ok) {
    msgEl.innerHTML = `<div class="alert alert-success">✓ Recipe "${aliases[0]}" saved! <a href="/query">Search</a> or <a href="/">Log food</a></div>`;
    e.target.reset();
  } else {
    const err = await res.json();
    msgEl.innerHTML = `<div class="alert alert-error">${err.error || 'Error saving recipe'}</div>`;
  }
});
</script>
</body>
</html>"##;

// ── HTML page: New Exercise ───────────────────────────────────────────────────

const NEW_EXERCISE_PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>New Exercise – Nutrition Tracker</title>
  <style>COMMON_CSS_PLACEHOLDER</style>
  <script>COMMON_JS_PLACEHOLDER</script>
</head>
<body>
<nav>
  <div class="nav-brand">🥗 Nutrition</div>
  <div class="nav-links">
    <a href="/">Today</a>
    <a href="/calendar">Calendar</a>
    <a href="/query">Search</a>
    <a href="/ingredients/new" class="cta">+ Food</a>
    <a href="/recipes/new" class="cta">+ Recipe</a>
    <a href="/exercises/new" class="cta active">+ Exercise</a>
  </div>
</nav>
<main>
  <h1>🏋️ New Exercise</h1>
  <p style="color:var(--text-dim);margin-bottom:1rem;">Define a new exercise type with calories burned per duration.</p>

  <div class="card">
    <form id="ex-form">
      <div>
        <div class="section-title"><h3>Exercise Name(s)</h3></div>
        <div class="repeatable-list" id="aliases-list">
          <div class="repeatable-item">
            <input type="text" placeholder="e.g. running" required>
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-alias" style="margin-top:0.5rem;">+ Add alias</button>
      </div>

      <div>
        <div class="section-title"><h3>Reference Duration</h3></div>
        <p style="color:var(--text-dim);font-size:0.85rem;margin-bottom:0.5rem;">e.g. "30 min" or "1 hour"</p>
        <div class="repeatable-list" id="quantities-list">
          <div class="repeatable-item">
            <input type="text" placeholder="e.g. 30 min" required>
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-quantity" style="margin-top:0.5rem;">+ Add duration</button>
      </div>

      <div>
        <div class="section-title"><h3>Properties (calories burned, etc.)</h3></div>
        <p style="color:var(--text-dim);font-size:0.85rem;margin-bottom:0.5rem;">e.g. calories: 300kcal (per reference duration above)</p>
        <div class="repeatable-list" id="props-list">
          <div class="repeatable-item">
            <input type="text" placeholder="Property (e.g. calories)" style="flex:1.2">
            <input type="text" placeholder="Value (e.g. 300kcal)" style="flex:1">
            <button type="button" class="secondary remove-btn" title="Remove">✕</button>
          </div>
        </div>
        <button type="button" class="secondary" id="add-prop" style="margin-top:0.5rem;">+ Add property</button>
      </div>

      <div id="form-msg"></div>
      <button type="submit" style="align-self:flex-start;">Save Exercise</button>
    </form>
  </div>
</main>
<script>
addRemoveHandlers();

document.getElementById('add-alias').onclick = () => {
  const item = document.createElement('div'); item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="e.g. jogging"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('aliases-list').appendChild(item); addRemoveHandlers();
};
document.getElementById('add-quantity').onclick = () => {
  const item = document.createElement('div'); item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="e.g. 1 hour"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('quantities-list').appendChild(item); addRemoveHandlers();
};
document.getElementById('add-prop').onclick = () => {
  const item = document.createElement('div'); item.className = 'repeatable-item';
  item.innerHTML = '<input type="text" placeholder="Property (e.g. calories)" style="flex:1.2"><input type="text" placeholder="Value (e.g. 250kcal)" style="flex:1"><button type="button" class="secondary remove-btn" title="Remove">✕</button>';
  document.getElementById('props-list').appendChild(item); addRemoveHandlers();
};

document.getElementById('ex-form').addEventListener('submit', async e => {
  e.preventDefault();
  const msgEl = document.getElementById('form-msg');

  const aliases = [...document.getElementById('aliases-list').querySelectorAll('input')].map(i => i.value.trim()).filter(Boolean);
  const quantities = [...document.getElementById('quantities-list').querySelectorAll('input')].map(i => parseQuantityStr(i.value)).filter(Boolean);

  const propItems = [...document.getElementById('props-list').querySelectorAll('.repeatable-item')];
  const properties = propItems.map(item => {
    const inputs = item.querySelectorAll('input');
    const name = inputs[0].value.trim();
    const val  = parseQuantityStr(inputs[1].value);
    if (!name || !val) return null;
    return { name, value: val };
  }).filter(Boolean);

  if (aliases.length === 0) { msgEl.innerHTML = '<div class="alert alert-error">At least one name is required.</div>'; return; }
  if (quantities.length === 0) { msgEl.innerHTML = '<div class="alert alert-error">At least one duration is required.</div>'; return; }

  const body = { aliases, quantities, properties };
  const res = await fetch('/api/exercises', {
    method: 'POST',
    headers: {'Content-Type':'application/json'},
    body: JSON.stringify(body)
  });
  if (res.ok) {
    msgEl.innerHTML = `<div class="alert alert-success">✓ Exercise "${aliases[0]}" saved! <a href="/">Log exercise</a></div>`;
    e.target.reset();
  } else {
    const err = await res.json();
    msgEl.innerHTML = `<div class="alert alert-error">${err.error || 'Error saving exercise'}</div>`;
  }
});
</script>
</body>
</html>"##;
