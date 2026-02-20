//! DuckDB in-process SQL support for nutrition data.
//!
//! This module builds an in-memory DuckDB database from a [`Document`] and
//! exposes a single entry-point, [`run_sql_query`], for executing arbitrary
//! SQL against it.
//!
//! # Schema
//!
//! Core tables:
//! - `ingredients`       – one row per ingredient definition
//! - `ingredient_aliases`– many-to-one mapping from alias to ingredient
//! - `ingredient_quantities` – declared quantity equivalencies per ingredient
//! - `ingredient_properties` – named property values per ingredient
//! - `recipes`           – one row per recipe definition
//! - `recipe_aliases`    – many-to-one mapping from alias to recipe
//! - `recipe_quantities` – declared serving-size quantities per recipe
//! - `recipe_ingredients`– (recipe_id, ingredient_alias, amount, unit)
//! - `exercises`         – one row per exercise definition
//! - `exercise_aliases`  – many-to-one mapping from alias to exercise
//! - `exercise_quantities`– declared quantity equivalencies per exercise
//! - `exercise_properties`– named property values per exercise
//! - `days`              – one row per @day block
//! - `day_ate`           – (day_id, food_alias, amount, unit)
//! - `day_exercised`     – (day_id, exercise_alias, amount, unit)
//!
//! Computed views:
//! - `day_nutrition`     – per-day, per-food intake resolved to ingredient/recipe properties
//! - `day_summary`       – per-day roll-up: total calories in, calories burned, net calories

use crate::ast::ast::{Day, Document, Exercise, Ingredient, Item, Recipe};
use duckdb::{Connection, Error as DuckdbError};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build an in-memory DuckDB database from `document`, execute `query`, and
/// return a human-readable result string (tab-separated columns, newline-separated rows).
///
/// Returns an error string on database or query failures.
pub fn run_sql_query(document: &Document, query: &str) -> Result<String, String> {
    let conn = build_database(document).map_err(|e| format!("Database error: {e}"))?;
    execute_query(&conn, query).map_err(|e| format!("Query error: {e}"))
}

// ---------------------------------------------------------------------------
// Database construction
// ---------------------------------------------------------------------------

fn build_database(document: &Document) -> Result<Connection, DuckdbError> {
    let conn = Connection::open_in_memory()?;
    create_schema(&conn)?;
    populate_tables(&conn, document)?;
    create_views(&conn)?;
    Ok(conn)
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

fn create_schema(conn: &Connection) -> Result<(), DuckdbError> {
    conn.execute_batch("
        CREATE TABLE ingredients (
            id      INTEGER PRIMARY KEY,
            name    TEXT NOT NULL
        );

        CREATE TABLE ingredient_aliases (
            ingredient_id INTEGER NOT NULL REFERENCES ingredients(id),
            alias         TEXT    NOT NULL
        );

        CREATE TABLE ingredient_quantities (
            ingredient_id INTEGER NOT NULL REFERENCES ingredients(id),
            amount        DOUBLE  NOT NULL,
            unit          TEXT    NOT NULL
        );

        CREATE TABLE ingredient_properties (
            ingredient_id INTEGER NOT NULL REFERENCES ingredients(id),
            name          TEXT    NOT NULL,
            amount        DOUBLE  NOT NULL,
            unit          TEXT    NOT NULL
        );

        CREATE TABLE recipes (
            id   INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE recipe_aliases (
            recipe_id INTEGER NOT NULL REFERENCES recipes(id),
            alias     TEXT    NOT NULL
        );

        CREATE TABLE recipe_quantities (
            recipe_id INTEGER NOT NULL REFERENCES recipes(id),
            amount    DOUBLE  NOT NULL,
            unit      TEXT    NOT NULL
        );

        CREATE TABLE recipe_ingredients (
            recipe_id        INTEGER NOT NULL REFERENCES recipes(id),
            ingredient_alias TEXT    NOT NULL,
            amount           DOUBLE  NOT NULL,
            unit             TEXT    NOT NULL
        );

        CREATE TABLE exercises (
            id   INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE exercise_aliases (
            exercise_id INTEGER NOT NULL REFERENCES exercises(id),
            alias       TEXT    NOT NULL
        );

        CREATE TABLE exercise_quantities (
            exercise_id INTEGER NOT NULL REFERENCES exercises(id),
            amount      DOUBLE  NOT NULL,
            unit        TEXT    NOT NULL
        );

        CREATE TABLE exercise_properties (
            exercise_id INTEGER NOT NULL REFERENCES exercises(id),
            name        TEXT    NOT NULL,
            amount      DOUBLE  NOT NULL,
            unit        TEXT    NOT NULL
        );

        CREATE TABLE days (
            id   INTEGER PRIMARY KEY,
            date TEXT NOT NULL
        );

        CREATE TABLE day_ate (
            day_id INTEGER NOT NULL REFERENCES days(id),
            food_alias TEXT   NOT NULL,
            amount     DOUBLE NOT NULL,
            unit       TEXT   NOT NULL
        );

        CREATE TABLE day_exercised (
            day_id         INTEGER NOT NULL REFERENCES days(id),
            exercise_alias TEXT    NOT NULL,
            amount         DOUBLE  NOT NULL,
            unit           TEXT    NOT NULL
        );
    ")
}

// ---------------------------------------------------------------------------
// Population
// ---------------------------------------------------------------------------

fn populate_tables(conn: &Connection, document: &Document) -> Result<(), DuckdbError> {
    let mut ingredient_id: i64 = 0;
    let mut recipe_id: i64 = 0;
    let mut exercise_id: i64 = 0;
    let mut day_id: i64 = 0;

    for item in &document.items {
        match item {
            Item::Ingredient(ing) => {
                insert_ingredient(conn, &mut ingredient_id, ing)?;
            }
            Item::Recipe(rec) => {
                insert_recipe(conn, &mut recipe_id, rec)?;
            }
            Item::Exercise(ex) => {
                insert_exercise(conn, &mut exercise_id, ex)?;
            }
            Item::Day(day) => {
                insert_day(conn, &mut day_id, day)?;
            }
            // Ate / Exercised at top level are treated as properties, not in-day entries.
            Item::Ate(_) | Item::Exercised(_) | Item::Property(_) | Item::Comment(_) => {}
        }
    }
    Ok(())
}

fn insert_ingredient(
    conn: &Connection,
    next_id: &mut i64,
    ing: &Ingredient,
) -> Result<(), DuckdbError> {
    *next_id += 1;
    let id = *next_id;
    let name = ing.aliases.first().cloned().unwrap_or_default();

    conn.execute(
        "INSERT INTO ingredients (id, name) VALUES (?, ?)",
        duckdb::params![id, name],
    )?;

    for alias in &ing.aliases {
        conn.execute(
            "INSERT INTO ingredient_aliases (ingredient_id, alias) VALUES (?, ?)",
            duckdb::params![id, alias],
        )?;
    }

    for qty in &ing.quantities {
        let unit = qty.unit.clone().unwrap_or_default();
        conn.execute(
            "INSERT INTO ingredient_quantities (ingredient_id, amount, unit) VALUES (?, ?, ?)",
            duckdb::params![id, qty.amount, unit],
        )?;
    }

    for prop in &ing.properties {
        let unit = prop.value.unit.clone().unwrap_or_default();
        conn.execute(
            "INSERT INTO ingredient_properties (ingredient_id, name, amount, unit) VALUES (?, ?, ?, ?)",
            duckdb::params![id, prop.name, prop.value.amount, unit],
        )?;
    }

    Ok(())
}

fn insert_recipe(
    conn: &Connection,
    next_id: &mut i64,
    rec: &Recipe,
) -> Result<(), DuckdbError> {
    *next_id += 1;
    let id = *next_id;
    let name = rec.aliases.first().cloned().unwrap_or_default();

    conn.execute(
        "INSERT INTO recipes (id, name) VALUES (?, ?)",
        duckdb::params![id, name],
    )?;

    for alias in &rec.aliases {
        conn.execute(
            "INSERT INTO recipe_aliases (recipe_id, alias) VALUES (?, ?)",
            duckdb::params![id, alias],
        )?;
    }

    for qty in &rec.quantities {
        let unit = qty.unit.clone().unwrap_or_default();
        conn.execute(
            "INSERT INTO recipe_quantities (recipe_id, amount, unit) VALUES (?, ?, ?)",
            duckdb::params![id, qty.amount, unit],
        )?;
    }

    for ing_label in &rec.ingredients {
        let unit = ing_label.quantity.unit.clone().unwrap_or_default();
        conn.execute(
            "INSERT INTO recipe_ingredients (recipe_id, ingredient_alias, amount, unit) VALUES (?, ?, ?, ?)",
            duckdb::params![id, ing_label.alias, ing_label.quantity.amount, unit],
        )?;
    }

    Ok(())
}

fn insert_exercise(
    conn: &Connection,
    next_id: &mut i64,
    ex: &Exercise,
) -> Result<(), DuckdbError> {
    *next_id += 1;
    let id = *next_id;
    let name = ex.aliases.first().cloned().unwrap_or_default();

    conn.execute(
        "INSERT INTO exercises (id, name) VALUES (?, ?)",
        duckdb::params![id, name],
    )?;

    for alias in &ex.aliases {
        conn.execute(
            "INSERT INTO exercise_aliases (exercise_id, alias) VALUES (?, ?)",
            duckdb::params![id, alias],
        )?;
    }

    for qty in &ex.quantities {
        let unit = qty.unit.clone().unwrap_or_default();
        conn.execute(
            "INSERT INTO exercise_quantities (exercise_id, amount, unit) VALUES (?, ?, ?)",
            duckdb::params![id, qty.amount, unit],
        )?;
    }

    for prop in &ex.properties {
        let unit = prop.value.unit.clone().unwrap_or_default();
        conn.execute(
            "INSERT INTO exercise_properties (exercise_id, name, amount, unit) VALUES (?, ?, ?, ?)",
            duckdb::params![id, prop.name, prop.value.amount, unit],
        )?;
    }

    Ok(())
}

fn insert_day(
    conn: &Connection,
    next_id: &mut i64,
    day: &Day,
) -> Result<(), DuckdbError> {
    use crate::ast::ast::DayItem;

    *next_id += 1;
    let id = *next_id;

    conn.execute(
        "INSERT INTO days (id, date) VALUES (?, ?)",
        duckdb::params![id, day.date],
    )?;

    for day_item in &day.items {
        match day_item {
            DayItem::Ate(ate) => {
                let unit = ate.quantity.unit.clone().unwrap_or_default();
                conn.execute(
                    "INSERT INTO day_ate (day_id, food_alias, amount, unit) VALUES (?, ?, ?, ?)",
                    duckdb::params![id, ate.food_alias, ate.quantity.amount, unit],
                )?;
            }
            DayItem::Exercised(ex) => {
                let unit = ex.quantity.unit.clone().unwrap_or_default();
                conn.execute(
                    "INSERT INTO day_exercised (day_id, exercise_alias, amount, unit) VALUES (?, ?, ?, ?)",
                    duckdb::params![id, ex.exercise_alias, ex.quantity.amount, unit],
                )?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

fn create_views(conn: &Connection) -> Result<(), DuckdbError> {
    // ---------------------------------------------------------------------------
    // ingredient_nutrition_per_100g
    //   Flatten ingredient properties to (ingredient_name, property, amount_per_100g)
    //   for easy scaling when computing how much was eaten.
    // ---------------------------------------------------------------------------
    conn.execute_batch("
        CREATE VIEW ingredient_nutrition_per_100g AS
        SELECT
            ia.alias                          AS food_alias,
            ip.name                           AS property,
            ip.amount                         AS prop_amount,
            ip.unit                           AS prop_unit,
            COALESCE(iq.amount, 100.0)        AS base_amount,
            COALESCE(iq.unit,  'g')           AS base_unit
        FROM ingredient_properties ip
        JOIN ingredients            i  ON i.id  = ip.ingredient_id
        JOIN ingredient_aliases     ia ON ia.ingredient_id = i.id
        LEFT JOIN (
            SELECT ingredient_id, amount, unit
            FROM ingredient_quantities
            WHERE (ingredient_id, amount) IN (
                SELECT ingredient_id, MIN(amount) FROM ingredient_quantities GROUP BY ingredient_id
            )
        ) iq ON iq.ingredient_id = i.id;
    ")?;

    // ---------------------------------------------------------------------------
    // recipe_nutrition_flat
    //   Resolve recipe ingredients → ingredient properties.
    //   Assumes recipe is used with the same unit as declared in recipe_ingredients.
    // ---------------------------------------------------------------------------
    conn.execute_batch("
        CREATE VIEW recipe_nutrition_flat AS
        SELECT
            ra.alias                        AS food_alias,
            ip.name                         AS property,
            -- scale: (label amount / base amount) * ingredient property amount
            (ri.amount
             / COALESCE(iq_base.amount, 100.0))
            * ip.amount                     AS prop_amount,
            ip.unit                         AS prop_unit,
            rq.amount                       AS recipe_base_amount,
            COALESCE(rq.unit, '')           AS recipe_base_unit
        FROM recipe_ingredients         ri
        JOIN recipes                    r   ON r.id  = ri.recipe_id
        JOIN recipe_aliases             ra  ON ra.recipe_id = r.id
        JOIN ingredient_aliases         ia  ON ia.alias = ri.ingredient_alias
        JOIN ingredients                i   ON i.id  = ia.ingredient_id
        JOIN ingredient_properties      ip  ON ip.ingredient_id = i.id
        LEFT JOIN (
            SELECT ingredient_id, amount, unit
            FROM ingredient_quantities
            WHERE (ingredient_id, amount) IN (
                SELECT ingredient_id, MIN(amount) FROM ingredient_quantities GROUP BY ingredient_id
            )
        ) iq_base ON iq_base.ingredient_id = i.id
        LEFT JOIN (
            SELECT recipe_id, amount, unit
            FROM recipe_quantities
            WHERE (recipe_id, amount) IN (
                SELECT recipe_id, MIN(amount) FROM recipe_quantities GROUP BY recipe_id
            )
        ) rq ON rq.recipe_id = r.id;
    ")?;

    // ---------------------------------------------------------------------------
    // day_ate_nutrition
    //   For each (day, food) entry, resolve the property values scaled to the
    //   consumed quantity.  Works for both plain ingredients and recipes.
    // ---------------------------------------------------------------------------
    conn.execute_batch("
        CREATE VIEW day_ate_nutrition AS
        -- From ingredients
        SELECT
            d.date,
            da.food_alias,
            n.property,
            (da.amount / n.base_amount) * n.prop_amount AS amount,
            n.prop_unit                                  AS unit
        FROM day_ate           da
        JOIN days              d  ON d.id = da.day_id
        JOIN ingredient_nutrition_per_100g n ON n.food_alias = da.food_alias

        UNION ALL

        -- From recipes (scale by requested amount / recipe base amount)
        SELECT
            d.date,
            da.food_alias,
            rn.property,
            (da.amount / NULLIF(rn.recipe_base_amount, 0)) * rn.prop_amount AS amount,
            rn.prop_unit                                                      AS unit
        FROM day_ate      da
        JOIN days         d  ON d.id = da.day_id
        JOIN recipe_nutrition_flat rn ON rn.food_alias = da.food_alias;
    ")?;

    // ---------------------------------------------------------------------------
    // day_exercised_calories
    //   Calories burned per (day, exercise) entry.
    // ---------------------------------------------------------------------------
    conn.execute_batch("
        CREATE VIEW day_exercised_calories AS
        SELECT
            d.date,
            de.exercise_alias,
            (de.amount / COALESCE(eq_base.amount, 1.0)) * ep.amount AS calories_burned,
            ep.unit
        FROM day_exercised de
        JOIN days          d   ON d.id = de.day_id
        JOIN exercise_aliases ea  ON ea.alias = de.exercise_alias
        JOIN exercises        ex  ON ex.id = ea.exercise_id
        JOIN exercise_properties ep ON ep.exercise_id = ex.id AND LOWER(ep.name) IN ('calories', 'energy')
        LEFT JOIN (
            SELECT exercise_id, amount, unit
            FROM exercise_quantities
            WHERE (exercise_id, amount) IN (
                SELECT exercise_id, MIN(amount) FROM exercise_quantities GROUP BY exercise_id
            )
        ) eq_base ON eq_base.exercise_id = ex.id;
    ")?;

    // ---------------------------------------------------------------------------
    // days  (view) – per-day summary rolling up calories in / out
    // ---------------------------------------------------------------------------
    conn.execute_batch("
        CREATE VIEW day_summary AS
        SELECT
            date,
            SUM(CASE WHEN LOWER(property) IN ('calories', 'energy') THEN amount ELSE 0 END)
                AS calories_in,
            COALESCE((
                SELECT SUM(calories_burned)
                FROM day_exercised_calories dec2
                WHERE dec2.date = dan.date
            ), 0.0) AS calories_burned,
            SUM(CASE WHEN LOWER(property) IN ('calories', 'energy') THEN amount ELSE 0 END)
            - COALESCE((
                SELECT SUM(calories_burned)
                FROM day_exercised_calories dec2
                WHERE dec2.date = dan.date
            ), 0.0) AS net_calories
        FROM day_ate_nutrition dan
        GROUP BY date;
    ")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Query execution
// ---------------------------------------------------------------------------

/// Execute a SQL statement and format results as a tab-separated table.
fn execute_query(conn: &Connection, query: &str) -> Result<String, DuckdbError> {
    let mut stmt = conn.prepare(query)?;
    let mut rows_iter = stmt.query([])?;

    // Column info is available after query() has been called.
    let column_count = rows_iter.as_ref().map(|s| s.column_count()).unwrap_or(0);
    let column_names: Vec<String> = (0..column_count)
        .map(|i| {
            rows_iter
                .as_ref()
                .and_then(|s| s.column_name(i).ok())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("col{i}"))
        })
        .collect();

    let mut output = String::new();

    // Header
    output.push_str(&column_names.join("\t"));
    output.push('\n');

    // Rows
    while let Some(row) = rows_iter.next()? {
        let mut cols: Vec<String> = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let val: String = format_value(row.get_unwrap::<_, duckdb::types::Value>(i));
            cols.push(val);
        }
        output.push_str(&cols.join("\t"));
        output.push('\n');
    }

    Ok(output)
}

/// Format a DuckDB [`Value`] as a human-readable string.
fn format_value(v: duckdb::types::Value) -> String {
    use duckdb::types::Value;
    match v {
        Value::Null => "NULL".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::TinyInt(n) => n.to_string(),
        Value::SmallInt(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::BigInt(n) => n.to_string(),
        Value::HugeInt(n) => n.to_string(),
        Value::UTinyInt(n) => n.to_string(),
        Value::USmallInt(n) => n.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::UBigInt(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Decimal(d) => d.to_string(),
        Value::Text(s) => s,
        Value::Enum(s) => s,
        Value::Blob(b) => format!("<blob {} bytes>", b.len()),
        Value::Date32(d) => d.to_string(),
        Value::Time64(_, t) => t.to_string(),
        Value::Timestamp(_, t) => t.to_string(),
        Value::Interval { months, days, nanos } => {
            format!("{}mo {}d {}ns", months, days, nanos)
        }
        Value::List(items) => {
            let parts: Vec<String> = items.into_iter().map(format_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Array(items) => {
            let parts: Vec<String> = items.into_iter().map(format_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Struct(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v.clone())))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Map(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", format_value(k.clone()), format_value(v.clone())))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Union(inner) => format_value(*inner),
    }
}
