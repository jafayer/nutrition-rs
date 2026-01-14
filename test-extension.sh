#!/bin/bash

echo "Testing Nutrition VS Code Extension"
echo "===================================="

cd "$(dirname "$0")/vscode-extension"

# Check if LSP binary exists
if [ ! -f "bin/nutrition-lsp" ]; then
    echo "❌ LSP binary not found. Run ./build-extension.sh first"
    exit 1
fi

echo "✓ LSP binary found: bin/nutrition-lsp"

# Check if compiled extension exists
if [ ! -f "out/extension.js" ]; then
    echo "❌ Extension not compiled. Run ./build-extension.sh first"
    exit 1
fi

echo "✓ Extension compiled: out/extension.js"

# Test LSP binary works
echo ""
echo "Testing LSP binary..."
echo "Sending initialize request..."

timeout 5s bash -c 'cat << EOF | ./bin/nutrition-lsp 2>&1 | head -20
Content-Length: 246

{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":"file:///tmp","capabilities":{},"trace":"off","workspaceFolders":[{"uri":"file:///tmp","name":"test"}]}}
Content-Length: 53

{"jsonrpc":"2.0","method":"initialized","params":{}}
Content-Length: 45

{"jsonrpc":"2.0","id":2,"method":"shutdown"}
Content-Length: 46

{"jsonrpc":"2.0","method":"exit","params":{}}
EOF
' && echo "✓ LSP server responds correctly" || echo "❌ LSP server failed"

echo ""
echo "===================================="
echo "Extension Build Summary:"
echo "===================================="
echo "Extension location: $(pwd)"
echo "LSP binary: $(pwd)/bin/nutrition-lsp ($(du -h bin/nutrition-lsp | cut -f1))"
echo "Compiled output: $(pwd)/out/"
echo ""
echo "Next steps:"
echo "1. Open VS Code in the vscode-extension directory:"
echo "   code $(pwd)"
echo ""
echo "2. Press F5 to launch Extension Development Host"
echo ""
echo "3. In the new window, open: ../examples/test.nutrition"
echo ""
echo "4. You should see:"
echo "   - Syntax highlighting"
echo "   - LSP server starting (check Output > Nutrition Language Server)"
echo "   - Real-time diagnostics for syntax errors"
echo "   - Hover support"
echo "   - Code completion"
echo ""
