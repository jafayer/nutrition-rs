# Nutrition VS Code Extension

A Visual Studio Code extension providing language support for `.nutrition` files.

## Features

- **Syntax Highlighting** - Color coding for nutrition language keywords
- **Real-time Diagnostics** - Immediate feedback on syntax errors via tree-sitter parsing
- **Code Completion** - Context-aware suggestions (ready to extend)
- **Hover Information** - Quick documentation on hover (ready to extend)
- **Go to Definition** - Navigate to definitions (ready to extend)
- **Incremental Parsing** - Fast, efficient re-parsing using stateful tree-sitter trees

## Project Structure

```
nutrition-rs/
├── src/
│   ├── main.rs                 # CLI binary
│   └── lsp/
│       └── server.rs          # LSP server binary
├── vscode-extension/
│   ├── src/
│   │   └── extension.ts       # Extension entry point
│   ├── bin/
│   │   └── nutrition-lsp      # Bundled LSP binary
│   ├── syntaxes/
│   │   └── nutrition.tmLanguage.json
│   └── package.json
└── build-extension.sh         # Build script
```

## Building

Run the build script from the project root:

```bash
./build-extension.sh
```

This will:
1. Build the `nutrition-lsp` binary in release mode
2. Copy it to `vscode-extension/bin/`
3. Install npm dependencies
4. Compile the TypeScript extension code

## Testing the Extension

### Method 1: Run in Development Mode

1. Open the extension directory in VS Code:
   ```bash
   code vscode-extension/
   ```

2. Press **F5** (or Run > Start Debugging)
   - This launches a new "Extension Development Host" window
   - The extension is automatically loaded

3. In the new window:
   - Open `../examples/test.nutrition`
   - The LSP server should start automatically
   - Check **View > Output** and select "Nutrition Language Server" from the dropdown

4. Test features:
   - **Syntax Errors**: Modify the file to introduce errors
   - **Hover**: Hover over keywords
   - **Completion**: Type and trigger completion (Ctrl+Space)

### Method 2: Install Locally

Package and install the extension:

```bash
cd vscode-extension
npm run package
code --install-extension nutrition-language-0.1.0.vsix
```

## Extension Configuration

The extension can be configured in VS Code settings:

- **nutrition.lsp.path**: Custom path to the LSP server binary (defaults to bundled version)
- **nutrition.trace.server**: Enable LSP communication tracing for debugging
  - `off` (default)
  - `messages`
  - `verbose`

Access via: Code > Settings > Extensions > Nutrition Language Server

## Development

### Watch Mode

For continuous compilation during development:

```bash
cd vscode-extension
npm run watch
```

### LSP Server Development

The LSP server maintains stateful parse trees for each document:

- **DocumentState**: Holds text, tree-sitter tree, and parser
- **Incremental Parsing**: Reuses old trees for efficient updates
- **Diagnostics**: Automatically reports syntax errors from parse tree

To modify the LSP server:
1. Edit `src/lsp/server.rs`
2. Rebuild: `cargo build --release --bin nutrition-lsp`
3. Copy to extension: `cp target/release/nutrition-lsp vscode-extension/bin/`
4. Reload the Extension Development Host window (Ctrl+R)

### Extending Language Features

The extension uses the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/):

**In `src/lsp/server.rs`:**
- Modify `handle_request()` for new request types
- Modify `collect_diagnostics()` for semantic analysis
- Access parsed tree via `DocumentState.tree()`
- Access semantic AST via `DocumentState.analyze()`

**In `vscode-extension/src/extension.ts`:**
- Configuration changes (rarely needed, LSP handles most features automatically)

## Troubleshooting

### LSP Server Not Starting

1. Check the Output panel: **View > Output > Nutrition Language Server**
2. Verify the binary exists: `ls -lh vscode-extension/bin/nutrition-lsp`
3. Test the binary directly: `./vscode-extension/bin/nutrition-lsp`

### Extension Not Loading

1. Check **View > Output > Extension Host**
2. Verify `.nutrition` files are recognized:
   - Bottom right of editor should show "Nutrition" as the language
   - If not, click the language selector and choose "Nutrition"

### No Diagnostics Appearing

1. Ensure the document is saved with `.nutrition` extension
2. Check LSP trace: Set `nutrition.trace.server` to `"verbose"`
3. View trace: **View > Output > Nutrition LSP Trace**

## Distribution

To create a `.vsix` package for distribution:

```bash
cd vscode-extension
npm run package
```

This creates `nutrition-language-0.1.0.vsix` which can be:
- Installed locally: `code --install-extension nutrition-language-0.1.0.vsix`
- Shared with others
- Published to VS Code Marketplace (requires publisher account)

## Architecture

```
User types in .nutrition file
        ↓
VS Code Editor
        ↓
Extension (extension.ts)
        ↓
Language Client (vscode-languageclient)
        ↓
[JSON-RPC over stdio]
        ↓
LSP Server (nutrition-lsp binary)
        ↓
Tree-sitter Parser → DocumentState
        ↓
Semantic Analyzer
        ↓
Diagnostics/Completions/Hover → Back to VS Code
```

The extension acts as a bridge between VS Code and the Rust-based LSP server, which does the heavy lifting of parsing and analysis.
