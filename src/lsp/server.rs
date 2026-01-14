use std::error::Error;
use std::collections::HashMap;

use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    ServerCapabilities,
    TextDocumentSyncCapability,
    TextDocumentSyncKind,
    CompletionOptions,
    HoverProviderCapability,
    OneOf,
    InitializeParams,
    Uri,
    notification::{Notification, DidOpenTextDocument, DidChangeTextDocument, PublishDiagnostics},
    request::{Completion, HoverRequest, GotoDefinition, Formatting, Request as _, SemanticTokensFullRequest},
    DidOpenTextDocumentParams,
    DidChangeTextDocumentParams,
    HoverParams,
    CompletionResponse,
    CompletionItem,
    CompletionItemKind,
    Hover,
    HoverContents,
    MarkedString,
    TextEdit,
    Range,
    Position,
    Diagnostic,
    DiagnosticSeverity,
    PublishDiagnosticsParams,
    SemanticTokensLegend,
    SemanticTokensOptions,
    SemanticTokensServerCapabilities,
    SemanticToken,
    SemanticTokensResult,
    SemanticTokens,
    SemanticTokensParams,
    SemanticTokenType,
};

use tree_sitter::{Parser, Tree};

use nutrition_rs::tree_sitter_ast::ast::get_language;
use nutrition_rs::tree_sitter_ast::semantic::SemanticAnalyzer;

// =====================================================================
// Document State Management
// =====================================================================

/// Represents the state of a single document
struct DocumentState {
    /// The text content of the document
    text: String,
    /// The parsed tree-sitter tree
    tree: Tree,
    /// The tree-sitter parser (kept for incremental parsing)
    parser: Parser,
}

impl DocumentState {
    /// Create a new document state with initial parsing
    fn new(text: String) -> Result<Self, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&get_language())
            .map_err(|e| format!("Failed to set language: {}", e))?;
        
        let tree = parser
            .parse(&text, None)
            .ok_or_else(|| "Failed to parse document".to_string())?;
        
        Ok(DocumentState {
            text,
            tree,
            parser,
        })
    }
    
    /// Update the document with new text, using incremental parsing
    fn update(&mut self, new_text: String) -> Result<(), String> {
        let old_tree = Some(&self.tree);
        
        self.tree = self.parser
            .parse(&new_text, old_tree)
            .ok_or_else(|| "Failed to parse document".to_string())?;
        
        self.text = new_text;
        Ok(())
    }
    
    /// Get a reference to the current tree
    fn tree(&self) -> &Tree {
        &self.tree
    }
    
    /// Get a reference to the current text
    fn text(&self) -> &str {
        &self.text
    }
    
    /// Analyze the document and return semantic information
    fn analyze(&self) -> Result<nutrition_rs::ast::ast::Document, String> {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer.analyze(self.tree.root_node(), &self.text)
    }
}

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    eprintln!("Starting custom LSP server...");

    // 1. Create the transport (stdio communication with editor)
    let (connection, io_threads) = Connection::stdio();

    // 2. Define server capabilities (what features you support)
    let server_capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL  // Get full document on each change
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![". ".to_string(), ":".to_string()]),
            .. Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf:: Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: vec![
                        SemanticTokenType::KEYWORD,
                        SemanticTokenType::STRING,
                        SemanticTokenType::NUMBER,
                        SemanticTokenType::PROPERTY,
                        SemanticTokenType::VARIABLE,
                        SemanticTokenType::TYPE,
                        SemanticTokenType::COMMENT,
                    ],
                    token_modifiers: vec![],
                },
                full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
                range: None,
                ..Default::default()
            }
        )),
        ..Default::default()
    };

    // 3. Perform LSP handshake
    let server_capabilities = serde_json::to_value(&server_capabilities)?;
    let initialization_params = connection.initialize(server_capabilities)?;
    
    // 4. Run the main event loop
    main_loop(connection, initialization_params)?;
    
    // 5. Clean shutdown
    io_threads.join()?;
    eprintln!("LSP server shut down successfully");
    
    Ok(())
}

// =====================================================================
// Main Event Loop
// =====================================================================

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let _params: InitializeParams = serde_json::from_value(params)?;
    
    // Server state: track open documents with their tree-sitter trees
    let mut document_map: HashMap<Uri, DocumentState> = HashMap::new();
    
    // Message loop
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                // Handle shutdown gracefully
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(&connection, &req, &document_map)?;
            }
            Message::Notification(not) => {
                handle_notification(&connection, &not, &mut document_map)?;
            }
            Message::Response(resp) => {
                eprintln!("Unexpected response: {:?}", resp);
            }
        }
    }
    
    Ok(())
}

// =====================================================================
// Notification Handlers (client -> server, no response expected)
// =====================================================================

fn handle_notification(
    conn: &Connection,
    notification: &lsp_server::Notification,
    document_map: &mut HashMap<Uri, DocumentState>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match notification.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = 
                serde_json::from_value(notification.params.clone())?;
            
            let uri = params.text_document.uri.clone();
            let text = params.text_document.text;
            
            eprintln!("Document opened: {:?}", uri);
            
            // Parse the document and store its state
            match DocumentState::new(text) {
                Ok(state) => {
                    document_map.insert(uri.clone(), state);
                    // Send diagnostics based on the parsed tree
                    send_diagnostics(conn, &uri, &document_map)?;
                }
                Err(e) => {
                    eprintln!("Failed to parse document {:?}: {}", uri, e);
                }
            }
        }
        
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams = 
                serde_json::from_value(notification.params.clone())?;
            
            let uri = params.text_document.uri.clone();
            
            // With FULL sync, we get the entire document
            if let Some(change) = params.content_changes.into_iter().next() {
                eprintln!("Document changed: {:?}", uri);
                
                // Update the existing document state or create new one
                if let Some(state) = document_map.get_mut(&uri) {
                    // Incremental parsing using the old tree
                    if let Err(e) = state.update(change.text) {
                        eprintln!("Failed to update document {:?}: {}", uri, e);
                    } else {
                        send_diagnostics(conn, &uri, &document_map)?;
                    }
                } else {
                    // Document not found, create new state
                    match DocumentState::new(change.text) {
                        Ok(state) => {
                            document_map.insert(uri.clone(), state);
                            send_diagnostics(conn, &uri, &document_map)?;
                        }
                        Err(e) => {
                            eprintln!("Failed to parse document {:?}: {}", uri, e);
                        }
                    }
                }
            }
        }
        
        _ => eprintln!("Unhandled notification: {}", notification.method),
    }
    
    Ok(())
}

// =====================================================================
// Request Handlers (client -> server, response required)
// =====================================================================

fn handle_request(
    conn: &Connection,
    request: &Request,
    _document_map: &HashMap<Uri, DocumentState>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    match request.method.as_str() {
        GotoDefinition::METHOD => {
            eprintln!("Go to definition requested");
            
            let params: lsp_types::GotoDefinitionParams = serde_json::from_value(request.params.clone())?;
            let uri = params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;
            
            let locations = if let Some(state) = _document_map.get(&uri) {
                find_definition(state, position, &uri)
            } else {
                vec![]
            };
            
            let response = Response::new_ok(
                request.id.clone(),
                lsp_types::GotoDefinitionResponse::Array(locations)
            );
            conn.sender.send(Message::Response(response))?;
        }
        
        Completion::METHOD => {
            eprintln!("Completion requested");
            
            // Provide some sample completions
            let items = vec![
                CompletionItem {
                    label: "hello_world".to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("A friendly function".to_string()),
                    documentation: Some(lsp_types::Documentation::String(
                        "Prints hello world".to_string()
                    )),
                    ..Default::default()
                },
                CompletionItem {
                    label: "my_variable".to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("A sample variable".to_string()),
                    ..Default::default()
                },
            ];
            
            let response = Response::new_ok(
                request.id.clone(),
                CompletionResponse::Array(items)
            );
            conn.sender.send(Message::Response(response))?;
        }
        
        HoverRequest::METHOD => {
            eprintln!("Hover requested");
            
            let params: HoverParams = serde_json::from_value(request.params.clone())?;
            let uri = params.text_document_position_params.text_document.uri;
            let position = params.text_document_position_params.position;
            
            let hover = if let Some(state) = _document_map.get(&uri) {
                get_hover_info(state, position)
            } else {
                None
            };
            
            let response = Response::new_ok(request.id.clone(), hover);
            conn.sender.send(Message::Response(response))?;
        }
        
        Formatting::METHOD => {
            eprintln!("Formatting requested");
            
            // For this example, just return empty (no changes)
            let edits: Vec<TextEdit> = vec![];
            let response = Response::new_ok(request.id.clone(), edits);
            conn.sender.send(Message::Response(response))?;
        }
        
        SemanticTokensFullRequest::METHOD => {
            eprintln!("Semantic tokens requested");
            
            let params: SemanticTokensParams = serde_json::from_value(request.params.clone())?;
            let uri = params.text_document.uri;
            
            let tokens = if let Some(state) = _document_map.get(&uri) {
                collect_semantic_tokens(state)
            } else {
                vec![]
            };
            
            let result = SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data: tokens,
            });
            
            let response = Response::new_ok(request.id.clone(), result);
            conn.sender.send(Message::Response(response))?;
        }
        
        _ => {
            eprintln!("Unhandled request: {}", request.method);
            let response = Response::new_err(
                request.id.clone(),
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("Method not found: {}", request.method),
            );
            conn.sender.send(Message::Response(response))?;
        }
    }
    
    Ok(())
}

// =====================================================================
// Diagnostics (server -> client notifications)
// =====================================================================

fn send_diagnostics(
    conn: &Connection,
    uri: &Uri,
    document_map: &HashMap<Uri, DocumentState>,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let diagnostics = if let Some(state) = document_map.get(uri) {
        // Use the tree-sitter tree to generate diagnostics
        collect_diagnostics(state)
    } else {
        vec![]
    };
    
    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics,
        version: None,
    };
    
    let notification = lsp_server::Notification::new(
        PublishDiagnostics::METHOD.to_string(),
        params,
    );
    
    conn.sender.send(Message::Notification(notification))?;
    Ok(())
}

/// Collect diagnostics from the tree-sitter parse tree
fn collect_diagnostics(state: &DocumentState) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let tree = state.tree();
    let source = state.text();
    
    // Check for parse errors
    if tree.root_node().has_error() {
        collect_error_nodes(tree.root_node(), source, &mut diagnostics);
    }
    
    // Optionally perform semantic analysis for additional diagnostics
    match state.analyze() {
        Ok(_doc) => {
            // Successfully analyzed - could add semantic warnings/info here
            // For example: check for undefined references, unused definitions, etc.
        }
        Err(e) => {
            // Semantic analysis failed - add a diagnostic
            diagnostics.push(Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(lsp_types::NumberOrString::String("E001".to_string())),
                source: Some("nutrition-lsp".to_string()),
                message: format!("Semantic analysis failed: {}", e),
                ..Default::default()
            });
        }
    }
    
    diagnostics
}

/// Recursively collect error nodes from the tree
fn collect_error_nodes(node: tree_sitter::Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.is_error() || node.is_missing() {
        let start = node.start_position();
        let end = node.end_position();
        
        let message = if node.is_missing() {
            format!("Expected: {}", node.kind())
        } else {
            let text = &source[node.start_byte()..node.end_byte()];
            format!("Syntax error: unexpected '{}'", text)
        };
        
        diagnostics.push(Diagnostic {
            range: Range::new(
                Position::new(start.row as u32, start.column as u32),
                Position::new(end.row as u32, end.column as u32),
            ),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(lsp_types::NumberOrString::String("E100".to_string())),
            source: Some("nutrition-lsp".to_string()),
            message,
            ..Default::default()
        });
    }
    
    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_error_nodes(child, source, diagnostics);
    }
}

// =====================================================================
// Semantic Tokens
// =====================================================================

/// Collect semantic tokens from the tree-sitter parse tree
fn collect_semantic_tokens(state: &DocumentState) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();
    let tree = state.tree();
    let source = state.text();
    
    collect_tokens_recursive(tree.root_node(), source, &mut tokens);
    
    // Convert to delta-encoded format required by LSP
    encode_semantic_tokens(tokens)
}

/// Recursively collect tokens from the tree
fn collect_tokens_recursive(
    node: tree_sitter::Node,
    source: &str,
    tokens: &mut Vec<(u32, u32, u32, u32)>, // (line, start_char, length, token_type)
) {
    const KEYWORD: u32 = 0;
    const STRING: u32 = 1;
    const NUMBER: u32 = 2;
    const PROPERTY: u32 = 3;
    const VARIABLE: u32 = 4;
    const TYPE: u32 = 5;
    const COMMENT: u32 = 6;
    
    let kind = node.kind();
    let start = node.start_position();
    let length = node.end_byte() - node.start_byte();
    
    // Check if this node should be highlighted
    let token_type = match kind {
        // @ keywords from grammar.js
        "unit_decl" | "property_decl" | "ingredient_decl" | "food_decl" |
        "recipe_decl" | "exercise_decl" | "day_decl" | "ate_entry" | "exercised_entry" => {
            // Highlight the @ prefix specifically
            if let Some(text) = source.get(node.start_byte()..node.start_byte() + 1) {
                if text == "@" {
                    // Only highlight @ keywords at the start of declarations
                    let keyword_end = node.start_byte() + kind.len() - 5; // Remove "_decl" or "_entry"
                    let keyword_text = source.get(node.start_byte()..keyword_end.min(node.end_byte()));
                    if let Some(kw) = keyword_text {
                        if kw.starts_with('@') {
                            let kw_len = kw.split_whitespace().next().unwrap_or(kw).len();
                            tokens.push((
                                start.row as u32,
                                start.column as u32,
                                kw_len as u32,
                                KEYWORD
                            ));
                        }
                    }
                }
            }
            None // Don't highlight the whole node, just the keyword
        }
        "comment" => Some(COMMENT),
        "string" => Some(STRING),
        "number" => Some(NUMBER),
        "identifier" => {
            // Check if this is a property name (preceded by whitespace and followed by ':')
            let parent = node.parent();
            if let Some(p) = parent {
                if p.kind() == "property_assignment" {
                    Some(PROPERTY)
                } else {
                    Some(VARIABLE)
                }
            } else {
                Some(VARIABLE)
            }
        }
        "unit_name" | "unit_token" => Some(TYPE),
        // Property keywords
        _ if kind == "Int" || kind == "Float" || kind == "Bool" => Some(TYPE),
        _ if kind == "true" || kind == "false" || kind == "True" || kind == "False" => Some(KEYWORD),
        _ => None,
    };
    
    if let Some(tt) = token_type {
        tokens.push((
            start.row as u32,
            start.column as u32,
            length as u32,
            tt
        ));
    }
    
    // Recursively process children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tokens_recursive(child, source, tokens);
    }
}

/// Convert tokens to delta-encoded format required by LSP
/// Each token is encoded as: [deltaLine, deltaStart, length, tokenType, tokenModifiers]
fn encode_semantic_tokens(mut tokens: Vec<(u32, u32, u32, u32)>) -> Vec<SemanticToken> {
    // Sort by position
    tokens.sort_by_key(|&(line, col, _, _)| (line, col));
    
    let mut result = Vec::new();
    let mut prev_line = 0;
    let mut prev_start = 0;
    
    for (line, start, length, token_type) in tokens {
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 {
            start - prev_start
        } else {
            start
        };
        
        result.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });
        
        prev_line = line;
        prev_start = start;
    }
    
    result
}

// =====================================================================
// Go to Definition
// =====================================================================

/// Find the definition location for a symbol at the given position
fn find_definition(state: &DocumentState, position: Position, uri: &Uri) -> Vec<lsp_types::Location> {
    let tree = state.tree();
    let source = state.text();
    
    // Convert LSP position to byte offset
    let byte_offset = match position_to_offset(source, position) {
        Some(offset) => offset,
        None => return vec![],
    };
    
    // Find the node at this position
    let node = match get_node_at_offset(tree.root_node(), byte_offset) {
        Some(n) => n,
        None => return vec![],
    };
    
    // Check if we're in a recipe_ingredient_line
    let ingredient_line = find_ancestor_of_kind(node, "recipe_ingredient_line");
    if let Some(line_node) = ingredient_line {
        // Extract the ingredient name/alias from this line
        if let Some(ingredient_name) = extract_ingredient_name_from_line(line_node, source) {
            // Search for matching ingredient declaration
            if let Some(location) = find_ingredient_declaration(tree.root_node(), &ingredient_name, source, uri) {
                return vec![location];
            }
        }
    }
    
    vec![]
}

/// Find an ancestor node with a specific kind
fn find_ancestor_of_kind<'a>(mut node: tree_sitter::Node<'a>, kind: &str) -> Option<tree_sitter::Node<'a>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

/// Extract the ingredient name/alias from a recipe_ingredient_line
fn extract_ingredient_name_from_line(node: tree_sitter::Node, source: &str) -> Option<String> {
    // Look for identifier or string nodes in the recipe_ingredient_line
    // The ingredient name is typically the first significant token after any quantity
    let mut cursor = node.walk();
    
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        
        // Skip quantity and unit tokens
        if kind == "number" || kind == "unit_token" || kind == "unit_name" {
            continue;
        }
        
        // Look for the ingredient identifier or string
        if kind == "identifier" || kind == "string" {
            let text = &source[child.start_byte()..child.end_byte()];
            // Remove quotes if it's a string
            let cleaned = text.trim_matches('"').trim_matches('\'');
            return Some(cleaned.to_string());
        }
        
        // Recursively check children
        if let Some(name) = extract_ingredient_name_from_line(child, source) {
            return Some(name);
        }
    }
    
    None
}

/// Find an ingredient declaration with matching alias
fn find_ingredient_declaration(
    node: tree_sitter::Node,
    ingredient_name: &str,
    source: &str,
    uri: &Uri,
) -> Option<lsp_types::Location> {
    if node.kind() == "ingredient_decl" {
        // ingredient_decl has repeat1($.string); treat any string literal as a valid alias/name
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                let alias_text = &source[child.start_byte()..child.end_byte()];
                let cleaned_alias = alias_text.trim_matches('"').trim_matches('\'');
                
                if cleaned_alias.eq_ignore_ascii_case(ingredient_name) {
                    // Found a match! Return the location of the ingredient_decl
                    let start = node.start_position();
                    let end = node.end_position();
                    
                    return Some(lsp_types::Location {
                        uri: uri.clone(),
                        range: Range::new(
                            Position::new(start.row as u32, start.column as u32),
                            Position::new(end.row as u32, end.column as u32),
                        ),
                    });
                }
            }
        }
    }
    
    // Recursively search children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(location) = find_ingredient_declaration(child, ingredient_name, source, uri) {
            return Some(location);
        }
    }
    
    None
}

// =====================================================================
// Hover Information
// =====================================================================

/// Get hover information for a position in the document
fn get_hover_info(state: &DocumentState, position: Position) -> Option<Hover> {
    let tree = state.tree();
    let source = state.text();
    
    // Convert LSP position to byte offset
    let byte_offset = position_to_offset(source, position)?;
    
    // Find the smallest node at this position
    let node = get_node_at_offset(tree.root_node(), byte_offset)?;
    
    let kind = node.kind();
    let start = node.start_position();
    let end = node.end_position();
    let text = &source[node.start_byte()..node.end_byte()];
    
    // Truncate text for display if too long
    let display_text = if text.len() > 100 {
        format!("{}...", &text[..97])
    } else {
        text.to_string()
    };
    
    // Build hover content with node information
    let mut content = format!("**Tree-sitter Node**\n\n");
    content.push_str(&format!("- **Kind**: `{}`\n", kind));
    content.push_str(&format!("- **Position**: {}:{} - {}:{}\n", 
        start.row, start.column, end.row, end.column));
    content.push_str(&format!("- **Bytes**: {} - {}\n", 
        node.start_byte(), node.end_byte()));
    
    if !display_text.is_empty() {
        content.push_str(&format!("- **Text**: `{}`\n", display_text.replace('\n', "\\n")));
    }
    
    // Add parent info
    if let Some(parent) = node.parent() {
        content.push_str(&format!("- **Parent**: `{}`\n", parent.kind()));
    }
    
    // Add children count
    let child_count = node.child_count();
    if child_count > 0 {
        content.push_str(&format!("- **Children**: {}\n", child_count));
        
        // List first few children
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).take(5).collect();
        if !children.is_empty() {
            content.push_str("  - ");
            let child_kinds: Vec<_> = children.iter().map(|c| format!("`{}`", c.kind())).collect();
            content.push_str(&child_kinds.join(", "));
            if child_count > 5 {
                content.push_str(&format!(", ... ({} more)", child_count - 5));
            }
            content.push('\n');
        }
    }
    
    // Add field name if this is a named field
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        for (index, child) in parent.children(&mut cursor).enumerate() {
            if child.id() == node.id() {
                if let Some(field_name) = parent.field_name_for_child(index as u32) {
                    content.push_str(&format!("- **Field**: `{}`\n", field_name));
                }
                break;
            }
        }
    }
    
    // Add node properties
    if node.is_named() {
        content.push_str("- **Named**: yes\n");
    }
    if node.is_missing() {
        content.push_str("- **Missing**: yes (parse error)\n");
    }
    if node.is_error() {
        content.push_str("- **Error**: yes\n");
    }
    
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(content)),
        range: Some(Range::new(
            Position::new(start.row as u32, start.column as u32),
            Position::new(end.row as u32, end.column as u32),
        )),
    })
}

/// Convert LSP Position to byte offset in source
fn position_to_offset(source: &str, position: Position) -> Option<usize> {
    let mut offset = 0;
    let mut current_line = 0;
    
    for line in source.lines() {
        if current_line == position.line as usize {
            // Found the target line
            let char_offset = position.character as usize;
            if char_offset <= line.len() {
                return Some(offset + char_offset);
            } else {
                return Some(offset + line.len());
            }
        }
        offset += line.len() + 1; // +1 for newline
        current_line += 1;
    }
    
    // Position is beyond the end of the file
    if current_line == position.line as usize {
        Some(offset)
    } else {
        None
    }
}

/// Find the smallest node that contains the given byte offset
fn get_node_at_offset(node: tree_sitter::Node, offset: usize) -> Option<tree_sitter::Node> {
    if offset < node.start_byte() || offset > node.end_byte() {
        return None;
    }
    
    // Check children to find a more specific node
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if offset >= child.start_byte() && offset <= child.end_byte() {
            // Recursively search in this child
            if let Some(found) = get_node_at_offset(child, offset) {
                return Some(found);
            }
            // If recursive search didn't find anything, return this child
            return Some(child);
        }
    }
    
    // No child contains this offset, return the current node
    Some(node)
}
