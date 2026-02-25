use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use nutrition_rs::ffi::{
    nutrition_ffi_emit_ingredient, nutrition_ffi_free_string, nutrition_ffi_parse,
    nutrition_ffi_parse_with_errors, nutrition_ffi_unit_convert,
};
use serde_json::Value;

fn call_ffi_json(ptr: *mut c_char) -> Value {
    assert!(!ptr.is_null(), "ffi returned null pointer");

    let json_str = unsafe { CStr::from_ptr(ptr as *const c_char) }
        .to_str()
        .expect("ffi returned non-utf8 json")
        .to_string();

    unsafe {
        nutrition_ffi_free_string(ptr);
    }

    serde_json::from_str(&json_str).expect("ffi response was not valid json")
}

#[test]
fn ffi_parse_happy_path_returns_ok() {
    let input = CString::new(
        "@ingredient(100g) \"oats\" {\n  calories: 389\n  protein: 17g\n}\n",
    )
    .expect("valid c string");

    let response = call_ffi_json(unsafe { nutrition_ffi_parse(input.as_ptr()) });

    assert_eq!(response["ok"], Value::Bool(true));
    assert!(response["data"]["items"].is_array());
    assert!(response["error"].is_null());
}

#[test]
fn ffi_parse_null_pointer_returns_error() {
    let response = call_ffi_json(unsafe { nutrition_ffi_parse(std::ptr::null()) });

    assert_eq!(response["ok"], Value::Bool(false));
    assert!(response["data"].is_null());
    assert!(response["error"]
        .as_str()
        .expect("error string expected")
        .contains("must not be null"));
}

#[test]
fn ffi_parse_with_errors_reports_partial_errors() {
    let input = CString::new(
        "@ingredient(100g) \"oats\" {\n  calories: 389\n}\n@day \"2026-01-01\" {\n  \"oops\"(1)\n}\n",
    )
    .expect("valid c string");

    let response = call_ffi_json(unsafe { nutrition_ffi_parse_with_errors(input.as_ptr()) });

    assert_eq!(response["ok"], Value::Bool(true));
    assert!(response["data"]["errors"].is_array());
    let errors = response["data"]["errors"].as_array().expect("errors array");
    assert!(!errors.is_empty());
}

#[test]
fn ffi_unit_convert_handles_success_and_failure() {
    let from = CString::new("kg").expect("valid c string");
    let to = CString::new("g").expect("valid c string");
    let ok_response = call_ffi_json(unsafe { nutrition_ffi_unit_convert(1.0, from.as_ptr(), to.as_ptr()) });
    assert_eq!(ok_response["ok"], Value::Bool(true));
    assert_eq!(ok_response["data"]["unit"], Value::String("g".to_string()));

    let bad_from = CString::new("unknown_a").expect("valid c string");
    let bad_to = CString::new("unknown_b").expect("valid c string");
    let err_response = call_ffi_json(unsafe {
        nutrition_ffi_unit_convert(1.0, bad_from.as_ptr(), bad_to.as_ptr())
    });
    assert_eq!(err_response["ok"], Value::Bool(false));
    assert!(err_response["error"].as_str().is_some());
}

#[test]
fn ffi_emit_ingredient_invalid_json_returns_error() {
    let invalid = CString::new("{not valid json").expect("valid c string");
    let response = call_ffi_json(unsafe { nutrition_ffi_emit_ingredient(invalid.as_ptr()) });

    assert_eq!(response["ok"], Value::Bool(false));
    assert!(response["error"]
        .as_str()
        .expect("error string expected")
        .contains("invalid ingredient JSON"));
}

#[test]
fn ffi_free_string_accepts_null() {
    unsafe {
        nutrition_ffi_free_string(std::ptr::null_mut());
    }
}
