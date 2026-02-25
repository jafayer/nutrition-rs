use nutrition_rs::ast::ast::Item;
use nutrition_rs::cli::file_loader::load_tree;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn make_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("nutrition-rs-import-tests-{nanos}"));
    fs::create_dir_all(&dir).expect("failed to create temp test directory");
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directory");
    }
    fs::write(path, content).expect("failed to write test file");
}

#[test]
fn load_tree_resolves_recursive_imports() {
    let root_dir = make_temp_dir();

    let leaf = root_dir.join("leaf.nutrition");
    write_file(
        &leaf,
        r#"@ingredient(10g) "leaf" {
    calories: 10
}
"#,
    );

    let child = root_dir.join("nested").join("child.nutrition");
    write_file(
        &child,
        r#"!import "../leaf.nutrition"
@ingredient(20g) "child" {
    calories: 20
}
"#,
    );

    let root = root_dir.join("root.nutrition");
    write_file(
        &root,
        r#"!import "nested/child.nutrition"
@ingredient(30g) "root" {
    calories: 30
}
"#,
    );

    let doc = load_tree(Some(root.to_str().expect("utf-8 path")))
        .expect("recursive import expansion should parse");

    let aliases: Vec<String> = doc
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Ingredient(ingredient) => ingredient.aliases.first().cloned(),
            _ => None,
        })
        .collect();

    assert_eq!(aliases, vec!["leaf", "child", "root"]);

    fs::remove_dir_all(&root_dir).expect("failed to clean temp directory");
}

#[test]
fn load_tree_reports_cycle_for_recursive_imports() {
    let root_dir = make_temp_dir();

    let a = root_dir.join("a.nutrition");
    let b = root_dir.join("b.nutrition");

    write_file(&a, "!import \"b.nutrition\"\n");
    write_file(&b, "!import \"a.nutrition\"\n");

    let error = load_tree(Some(a.to_str().expect("utf-8 path")))
        .expect_err("import cycle should return an error");

    assert!(
        error.contains("Cyclic !import detected"),
        "unexpected error: {error}"
    );

    fs::remove_dir_all(&root_dir).expect("failed to clean temp directory");
}
