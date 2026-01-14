#!/bin/bash
set -e

echo "================================"
echo "Building Nutrition VS Code Extension"
echo "================================"

cd "$(dirname "$0")/vscode-extension"

# Step 1: Build the LSP server
echo -e "\n[1/4] Building LSP server..."
cd ..
cargo build --release --bin nutrition-lsp
cd vscode-extension

# Step 2: Copy LSP binary to extension
echo -e "\n[2/4] Copying LSP binary..."
mkdir -p bin
cp ../target/release/nutrition-lsp bin/
chmod +x bin/nutrition-lsp
echo "  ✓ LSP binary copied to bin/nutrition-lsp"

# Step 3: Install npm dependencies
echo -e "\n[3/4] Installing npm dependencies..."
npm install

# Step 4: Compile TypeScript
echo -e "\n[4/4] Compiling TypeScript..."
npm run compile

echo -e "\n================================"
echo "✅ Build complete!"
echo "================================"
echo ""
echo "To test the extension:"
echo "  1. Open the vscode-extension folder in VS Code"
echo "  2. Press F5 to launch Extension Development Host"
echo "  3. Open a .nutrition file to activate the extension"
echo ""
echo "To package for distribution:"
echo "  cd vscode-extension && npm run package"
echo ""
