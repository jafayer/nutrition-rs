# Nutrition VS Code Extension

A Visual Studio Code extension providing language support for `.nutrition` files.

## Features

- **Syntax Highlighting** - Color coding for nutrition language keywords
- **Find Commands** - Quick navigation to ingredients, foods, recipes, and days
- **Today Command** - Jump to or create today's daily log
- **Document Formatting** - Auto-indentation and block spacing

## Project Structure

```
nutrition-rs/
├── src/
│   ├── main.rs                 # CLI binary
│   └── lsp/
│       └── server.rs          # LSP server binary (unused)
├── vscode-extension/
│   ├── src/
│   │   └── extension.ts       # Extension entry point
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
1. Install npm dependencies
2. Compile the TypeScript extension code

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
   - **Find Commands**: Use Ctrl+Shift+P to access "Nutrition: Find Ingredient/Food/Recipe/Day"
   - **Today Command**: Use Ctrl+Shift+P and run "Nutrition: Today" to jump to today's entry
   - **Formatting**: Use Shift+Alt+F to format the document with proper indentation and spacing

Package and install the extension:

```bash
cd vscode-extension
npm run package
code --install-extension nutrition-language-0.1.0.vsix
```

## Extension Configuration

The Watch Mode

For continuous compilation during development:

```bash
cd vscode-extension
npm run watch
```

### Extending Language Features

To add new features to the extension:

**In `vscode-extension/src/extension.ts`:**
- Add new commands via `vscode.commands.registerCommand()`
- Modify the formatting provider in `registerFormattingProvider()` to add formatting rules
- Use the document API to interact with editor content

## Troubleshooting

### Extension Not Loading

1. Check **View > Output > Extension Host**
2. Verify `.nutrition` files are recognized:
   - Bottom right of editor should show "Nutrition" as the language
   - If not, click the language selector and choose "Nutrition"

### No Diagnostics Appearing

1. Ensure the document is saved with `.nutrition` extension
2. Check LSP trace: Set `nutrition.trace.server` to `"verbose"`
3. View trace: **View > Output > Nutrition LSP Trace**

##m run package
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
Syntax Highlighting (via grammar)
Document Formatting
Find & Navigation Commands
```

The extension provides client-side features for editing `.nutrition` files with syntax highlighting, formatting, and navigation command