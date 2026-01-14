use nutrition_rs::tree_sitter_ast::ast::parse;

fn print_tree(node: tree_sitter::Node, source: &str, indent: usize) {
    let name = node.kind();
    let text = if node.child_count() == 0 {
        let s = &source[node.start_byte()..node.end_byte()];
        format!(" '{}'", s.trim())
    } else {
        String::new()
    };
    println!("{}{}{}[{}..{}]", " ".repeat(indent), name, text, node.start_byte(), node.end_byte());
    
    for child in node.children(&mut node.walk()) {
        print_tree(child, source, indent + 2);
    }
}

fn main() {
    let source = r#"@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" {
    calories: 269
    protein: 14.5g
}"#;

    if let Some(tree) = parse(source) {
        let root = tree.root_node();
        println!("Tree structure:");
        print_tree(root, source, 0);
    } else {
        println!("Parse failed");
    }
}
