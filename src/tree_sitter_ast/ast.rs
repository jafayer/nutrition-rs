use tree_sitter::Parser;

pub fn get_language() -> tree_sitter::Language {
    tree_sitter_nutrition::LANGUAGE.into()
}

pub fn parse(source: &str, old_tree: Option<&tree_sitter::Tree>) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&get_language())
        .expect("Error loading Nutrition grammar");
    parser.parse(source, old_tree)
}