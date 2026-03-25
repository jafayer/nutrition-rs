# Nutrition Language Support

Language support for `.nutrition` files with client-side features.

## Features

- Syntax highlighting for `.nutrition` files
- Clickable `!import "..."` paths for quick file navigation
- Find commands for ingredients, foods, recipes, and days across the current file and imported files
- Today command to jump to or create today's entry
- Document formatting with auto-indentation and block spacing

## Requirements

None. The extension is self-contained with no external dependencies.

## Extension Settings

This extension currently has no user-configurable settings.

## Building from Source

1. Install dependencies and compile:
   ```bash
   cd vscode-extension
   npm install
   npm run compile
   ```

2. Package the extension:
   ```bash
   npm run package
   ```

## Testing

Press F5 in VS Code to launch an Extension Development Host with the extension loaded.
