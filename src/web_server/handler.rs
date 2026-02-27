//! HTTP handler and web UI for the nutrition server.
//!
//! Exposes a REST API (`/api/…`) and a mobile-responsive HTML web UI that
//! support read and write operations on the nutrition file passed to
//! `nutrition serve`.

use std::io::Write as IoWrite;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tower_http::services::ServeDir;
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

use axum::extract::Query;

use minijinja::{context, Environment};

// ── Application state ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub file_path: Arc<String>,
    /// Mutex that serialises all file-write operations to prevent corruption
    /// when concurrent requests attempt to modify the nutrition file.
    pub write_lock: Arc<tokio::sync::Mutex<()>>,

    // In-memory caching of the parsed nutrition file
    pub cache: Arc<tokio::sync::Mutex<Option<crate::ast::ast::Document>>>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_doc(state: &AppState) -> Result<crate::ast::ast::Document, (StatusCode, String)> {
    let tree = load_tree(Some(state.file_path.as_str()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e));

    match tree {
        Ok(t) => {
            // Update the cache with the newly loaded document.
            // Note: We can't use .lock().await here because this is a sync function.
            // The cache will be updated by the async handlers instead.
            Ok(t)
        },
        Err(e) => Err(e),
    }
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
struct Pagination {
    #[serde(default = "default_offset")]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_offset() -> usize { 0 }
fn default_limit() -> usize { 20 }

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

async fn api_list_ingredients(State(state): State<AppState>, Query(pagination): Query<Pagination>) -> Response {
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
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
        .skip(pagination.offset)
        .take(pagination.limit)
        .collect();
    (StatusCode::OK, Json(ingredients)).into_response()
}

async fn api_get_ingredient(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> Response {
    let alias = alias.to_lowercase();
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
        }
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
        Ok(()) => {
            state.cache.lock().await.take();
            (StatusCode::CREATED, Json(ingredient)).into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Recipes ──────────────────────────────────────────────────────────────

async fn api_list_recipes(State(state): State<AppState>, Query(pagination): Query<Pagination>) -> Response {
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
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
        .skip(pagination.offset)
        .take(pagination.limit)
        .collect();
    (StatusCode::OK, Json(recipes)).into_response()
}

async fn api_get_recipe(State(state): State<AppState>, Path(alias): Path<String>) -> Response {
    let alias = alias.to_lowercase();
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
        }
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
        Ok(()) => {
            state.cache.lock().await.take();
            (StatusCode::CREATED, Json(recipe)).into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Exercises ────────────────────────────────────────────────────────────

async fn api_list_exercises(State(state): State<AppState>, Query(pagination): Query<Pagination>) -> Response {
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
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
        .skip(pagination.offset)
        .take(pagination.limit)
        .collect();
    (StatusCode::OK, Json(exercises)).into_response()
}

async fn api_get_exercise(State(state): State<AppState>, Path(alias): Path<String>) -> Response {
    let alias = alias.to_lowercase();
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
        }
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
        Ok(()) => {
            state.cache.lock().await.take();
            (StatusCode::CREATED, Json(exercise)).into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── API: Days ─────────────────────────────────────────────────────────────────

async fn api_list_days(State(state): State<AppState>, Query(pagination): Query<Pagination>) -> Response {
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
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
        .skip(pagination.offset)
        .take(pagination.limit)
        .collect();
    (StatusCode::OK, Json(days)).into_response()
}

async fn api_get_day(State(state): State<AppState>, Path(date): Path<String>) -> Response {
    let cache = state.cache.lock().await;
    let doc = if let Some(doc) = &*cache {
        doc.clone()
    } else {
        drop(cache);
        match load_doc(&state) {
            Ok(d) => {
                state.cache.lock().await.replace(d.clone());
                d
            },
            Err((status, msg)) => {
                return (status, Json(serde_json::json!({ "error": msg }))).into_response()
            }
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
    let alias_lower = body.alias.to_lowercase();
    let entry = format!("@ate \"{}\"({})", alias_lower, qty.to_string());
    let _guard = state.write_lock.lock().await;
    match add_entry_to_day(&state.file_path, &date, &entry) {
        Ok(()) => {
            state.cache.lock().await.take();
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "date": date, "alias": alias_lower, "quantity": body.quantity })),
            )
                .into_response()
        },
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
    let entry = format!(
        "@exercised \"{}\"({})",
        alias_lower,
        qty.to_string()
    );
    let _guard = state.write_lock.lock().await;
    match add_entry_to_day(&state.file_path, &date, &entry) {
        Ok(()) => {
            state.cache.lock().await.take();
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "date": date, "alias": alias_lower, "quantity": body.quantity })),
            )
                .into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── Web UI pages ──────────────────────────────────────────────────────────────


fn render_template(env: &Environment, template: &str, active: &str) -> Result<Html<String>, (StatusCode, String)> {
    let tmpl = env
        .get_template(template)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let html = tmpl
        .render(context! { active => active })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Html(html))
}

async fn page_home(env: &Environment<'_>) -> Result<Html<String>, (StatusCode, String)> {
    render_template(env, "pages/home.html", "home")
}

async fn page_calendar(env: &Environment<'_>) -> Result<Html<String>, (StatusCode, String)> {
    render_template(env, "pages/calendar.html", "calendar")
}

async fn page_query(env: &Environment<'_>) -> Result<Html<String>, (StatusCode, String)> {
    render_template(env, "pages/query.html", "query")
}

async fn page_new_ingredient(env: &Environment<'_>) -> Result<Html<String>, (StatusCode, String)> {
    render_template(env, "pages/new_ingredient.html", "new_ingredient")
}

async fn page_new_recipe(env: &Environment<'_>) -> Result<Html<String>, (StatusCode, String)> {
    render_template(env, "pages/new_recipe.html", "new_recipe")
}

async fn page_new_exercise(env: &Environment<'_>) -> Result<Html<String>, (StatusCode, String)> {
    render_template(env, "pages/new_exercise.html", "new_exercise")
}

// ── run_server ────────────────────────────────────────────────────────────────

pub async fn run_server(
    host: String,
    port: u16,
    file_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut env = Environment::new();
    env.set_loader(minijinja::path_loader("src/web_server/templates"));
    let env = Arc::new(env);

    let state = AppState {
        file_path: Arc::new(file_path),
        write_lock: Arc::new(tokio::sync::Mutex::new(())),
        cache: Arc::new(tokio::sync::Mutex::new(None)),
    };

    let app = Router::new()
        // Web UI
        .route("/", get({
            let env = env.clone();
            move || {
                let env = env.clone();
                async move {
                    match page_home(&env).await {
                        Ok(html) => html.into_response(),
                        Err((status, msg)) => (status, msg).into_response(),
                    }
                }
            }
        }))
        .route("/calendar", get({
            let env = env.clone();
            move || {
                let env = env.clone();
                async move {
                    match page_calendar(&env).await {
                        Ok(html) => html.into_response(),
                        Err((status, msg)) => (status, msg).into_response(),
                    }
                }
            }
        }))
        .route("/query", get({
            let env = env.clone();
            move || {
                let env = env.clone();
                async move {
                    match page_query(&env).await {
                        Ok(html) => html.into_response(),
                        Err((status, msg)) => (status, msg).into_response(),
                    }
                }
            }
        }))
        .route("/ingredients/new", get({
            let env = env.clone();
            move || {
                let env = env.clone();
                async move {
                    match page_new_ingredient(&env).await {
                        Ok(html) => html.into_response(),
                        Err((status, msg)) => (status, msg).into_response(),
                    }
                }
            }
        }))
        .route("/recipes/new", get({
            let env = env.clone();
            move || {
                let env = env.clone();
                async move {
                    match page_new_recipe(&env).await {
                        Ok(html) => html.into_response(),
                        Err((status, msg)) => (status, msg).into_response(),
                    }
                }
            }
        }))
        .route("/exercises/new", get({
            let env = env.clone();
            move || {
                let env = env.clone();
                async move {
                    match page_new_exercise(&env).await {
                        Ok(html) => html.into_response(),
                        Err((status, msg)) => (status, msg).into_response(),
                    }
                }
            }
        }))
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
        .nest_service("/partials", ServeDir::new("src/web_server/templates/partials"))
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
