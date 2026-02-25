use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use logos::Logos;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ast::ast::{Day, Document, Ingredient, Quantity, Recipe};
use crate::emitters::day::DayEmitter;
use crate::emitters::emitter::CanEmit;
use crate::emitters::ingredient::IngredientEmitter;
use crate::emitters::recipe::RecipeEmitter;
use crate::lexer::lexer::Token;
use crate::nutrition::{compute_daily_report, compute_report, query_nutrition};
use crate::nutrition_units::{NutritionQuantity, UnitRegistry, default_unit_for_property};
use crate::parser::parser::{parse, parse_with_errors};

#[derive(Serialize)]
struct FfiResponse<T: Serialize> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Serialize)]
struct LexedToken {
    kind: String,
    text: String,
    span_start: usize,
    span_end: usize,
}

#[derive(Serialize)]
struct ParsedWithErrors {
    document: Option<Document>,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct UnitQuantityDto {
    amount: f64,
    unit: String,
}

fn to_json_ptr<T: Serialize>(value: &FfiResponse<T>) -> *mut c_char {
    match serde_json::to_string(value) {
        Ok(json) => string_to_ptr(&json),
        Err(err) => {
            let fallback = format!(
                "{{\"ok\":false,\"data\":null,\"error\":\"failed to serialize response: {}\"}}",
                err
            );
            string_to_ptr(&fallback)
        }
    }
}

fn ok_ptr<T: Serialize>(data: T) -> *mut c_char {
    to_json_ptr(&FfiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
}

fn err_ptr(message: impl Into<String>) -> *mut c_char {
    to_json_ptr::<serde_json::Value>(&FfiResponse {
        ok: false,
        data: None,
        error: Some(message.into()),
    })
}

fn string_to_ptr(value: &str) -> *mut c_char {
    let sanitized = value.replace('\0', "\\u0000");
    match CString::new(sanitized) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => CString::new(
            "{\"ok\":false,\"data\":null,\"error\":\"internal ffi string encoding error\"}",
        )
        .expect("static ffi fallback string must not contain null bytes")
        .into_raw(),
    }
}

fn ptr_to_string(ptr: *const c_char, name: &str) -> Result<String, String> {
    if ptr.is_null() {
        return Err(format!("{name} must not be null"));
    }

    let cstr = unsafe { CStr::from_ptr(ptr) };
    let text = cstr
        .to_str()
        .map_err(|_| format!("{name} must be valid UTF-8"))?;
    Ok(text.to_string())
}

fn parse_json_input<T: DeserializeOwned>(input: &str, name: &str) -> Result<T, String> {
    serde_json::from_str(input).map_err(|e| format!("invalid {name} JSON: {e}"))
}

fn token_kind(token: &Token) -> &'static str {
    match token {
        Token::AtUnit => "AtUnit",
        Token::AtProperty => "AtProperty",
        Token::AtIngredient => "AtIngredient",
        Token::AtFood => "AtFood",
        Token::AtRecipe => "AtRecipe",
        Token::AtExercise => "AtExercise",
        Token::AtDay => "AtDay",
        Token::AtAte => "AtAte",
        Token::AtExercised => "AtExercised",
        Token::LBrace => "LBrace",
        Token::RBrace => "RBrace",
        Token::LParen => "LParen",
        Token::RParen => "RParen",
        Token::Colon => "Colon",
        Token::Comma => "Comma",
        Token::Equals => "Equals",
        Token::String(_) => "String",
        Token::Number(_) => "Number",
        Token::Bool(_) => "Bool",
        Token::MealLabel(_) => "MealLabel",
        Token::Identifier(_) => "Identifier",
        Token::Comment(_) => "Comment",
        Token::Newline => "Newline",
        Token::Error => "Error",
    }
}

fn to_unit_dto(quantity: NutritionQuantity) -> UnitQuantityDto {
    UnitQuantityDto {
        amount: quantity.amount,
        unit: quantity.unit,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_lex(source: *const c_char) -> *mut c_char {
    let source = match ptr_to_string(source, "source") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let mut lexer = Token::lexer(&source);
    let mut tokens: Vec<LexedToken> = Vec::new();

    while let Some(next) = lexer.next() {
        let span = lexer.span();
        let text = lexer.slice().to_string();

        match next {
            Ok(token) => tokens.push(LexedToken {
                kind: token_kind(&token).to_string(),
                text,
                span_start: span.start,
                span_end: span.end,
            }),
            Err(_) => tokens.push(LexedToken {
                kind: "Error".to_string(),
                text,
                span_start: span.start,
                span_end: span.end,
            }),
        }
    }

    ok_ptr(tokens)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_parse(source: *const c_char) -> *mut c_char {
    let source = match ptr_to_string(source, "source") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    match parse(&source) {
        Some(document) => ok_ptr(document),
        None => err_ptr("failed to parse source"),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_parse_with_errors(source: *const c_char) -> *mut c_char {
    let source = match ptr_to_string(source, "source") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let (document, errors) = parse_with_errors(&source);
    ok_ptr(ParsedWithErrors { document, errors })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_emit_ingredient(
    ingredient_json: *const c_char,
) -> *mut c_char {
    let ingredient_json = match ptr_to_string(ingredient_json, "ingredient_json") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let ingredient: Ingredient = match parse_json_input(&ingredient_json, "ingredient") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let emitted = IngredientEmitter.emit(&ingredient);
    ok_ptr(emitted)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_emit_recipe(recipe_json: *const c_char) -> *mut c_char {
    let recipe_json = match ptr_to_string(recipe_json, "recipe_json") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let recipe: Recipe = match parse_json_input(&recipe_json, "recipe") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let emitted = RecipeEmitter.emit(&recipe);
    ok_ptr(emitted)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_emit_day(day_json: *const c_char) -> *mut c_char {
    let day_json = match ptr_to_string(day_json, "day_json") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let day: Day = match parse_json_input(&day_json, "day") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let emitted = DayEmitter.emit(&day);
    ok_ptr(emitted)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_default_unit(property_name: *const c_char) -> *mut c_char {
    let property_name = match ptr_to_string(property_name, "property_name") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let unit = default_unit_for_property(&property_name).map(str::to_string);
    ok_ptr(unit)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_unit_convert(
    amount: f64,
    from_unit: *const c_char,
    to_unit: *const c_char,
) -> *mut c_char {
    let from_unit = match ptr_to_string(from_unit, "from_unit") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };
    let to_unit = match ptr_to_string(to_unit, "to_unit") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let reg = UnitRegistry::with_si_defaults();
    let quantity = NutritionQuantity::new(amount, from_unit);
    match reg.convert(&quantity, &to_unit) {
        Some(converted) => ok_ptr(to_unit_dto(converted)),
        None => err_ptr(format!(
            "cannot convert '{}' to '{}'",
            quantity.unit, to_unit
        )),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_unit_add(
    a_amount: f64,
    a_unit: *const c_char,
    b_amount: f64,
    b_unit: *const c_char,
) -> *mut c_char {
    let a_unit = match ptr_to_string(a_unit, "a_unit") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };
    let b_unit = match ptr_to_string(b_unit, "b_unit") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let reg = UnitRegistry::with_si_defaults();
    let a = NutritionQuantity::new(a_amount, a_unit);
    let b = NutritionQuantity::new(b_amount, b_unit);

    match reg.add(&a, &b) {
        Ok(sum) => ok_ptr(to_unit_dto(sum)),
        Err(err) => err_ptr(err.to_string()),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_query_nutrition(
    document_json: *const c_char,
    alias: *const c_char,
    requested_quantity_json: *const c_char,
) -> *mut c_char {
    let document_json = match ptr_to_string(document_json, "document_json") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };
    let alias = match ptr_to_string(alias, "alias") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let document: Document = match parse_json_input(&document_json, "document") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let requested_quantity = if requested_quantity_json.is_null() {
        None
    } else {
        let raw = match ptr_to_string(requested_quantity_json, "requested_quantity_json") {
            Ok(v) => v,
            Err(e) => return err_ptr(e),
        };
        match parse_json_input::<Quantity>(&raw, "requested_quantity") {
            Ok(q) => Some(q),
            Err(e) => return err_ptr(e),
        }
    };

    let requested_quantity_ref = requested_quantity.as_ref();
    match query_nutrition(&document, &alias, requested_quantity_ref) {
        Ok(report) => ok_ptr(report),
        Err(err) => err_ptr(err),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_compute_daily_report(
    document_json: *const c_char,
    day_json: *const c_char,
) -> *mut c_char {
    let document_json = match ptr_to_string(document_json, "document_json") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };
    let day_json = match ptr_to_string(day_json, "day_json") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let document: Document = match parse_json_input(&document_json, "document") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };
    let day: Day = match parse_json_input(&day_json, "day") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let report = compute_daily_report(&document, &day);
    ok_ptr(report)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nutrition_ffi_compute_report(
    document_json: *const c_char,
    start_date: *const c_char,
    end_date: *const c_char,
) -> *mut c_char {
    let document_json = match ptr_to_string(document_json, "document_json") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };
    let document: Document = match parse_json_input(&document_json, "document") {
        Ok(v) => v,
        Err(e) => return err_ptr(e),
    };

    let start = if start_date.is_null() {
        None
    } else {
        match ptr_to_string(start_date, "start_date") {
            Ok(v) => Some(v),
            Err(e) => return err_ptr(e),
        }
    };

    let end = if end_date.is_null() {
        None
    } else {
        match ptr_to_string(end_date, "end_date") {
            Ok(v) => Some(v),
            Err(e) => return err_ptr(e),
        }
    };

    let reports = compute_report(&document, start.as_deref(), end.as_deref());
    ok_ptr(reports)
}
