#!/bin/bash
set -e

echo "================================"
echo "Building Nutrition VS Code Extension"
echo "================================"

cd "$(dirname "$0")/vscode-extension"

# Step 1: Install npm dependencies
echo -e "\n[1/2] Installing npm dependencies..."
npm install

# Step 2: Compile TypeScript
echo -e "\n[2/2] Compiling TypeScript..."
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
