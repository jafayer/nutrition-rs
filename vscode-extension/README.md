# Nutrition Language Support

Language support for Nutrition files with LSP-powered features.

## Features

- Syntax highlighting for `.nutrition` files
- Real-time syntax error detection
- Code completion
- Hover information
- Go to definition
- Document formatting

## Requirements

The extension bundles the `nutrition-lsp` language server.

## Extension Settings

This extension contributes the following settings:

- `nutrition.lsp.path`: Path to the nutrition-lsp executable (leave empty to use bundled version)
- `nutrition.trace.server`: Enable LSP communication tracing for debugging

## Building from Source

1. Build the LSP server:
   ```bash
   cd .. && cargo build --release --bin nutrition-lsp
   ```

2. Copy the LSP binary to the extension:
   ```bash
   mkdir -p bin
   cp ../target/release/nutrition-lsp bin/
   ```

3. Install dependencies and compile:
   ```bash
   npm install
   npm run compile
   ```

4. Package the extension:
   ```bash
   npm run package
   ```

## Testing

Press F5 in VS Code to launch an Extension Development Host with the extension loaded.
