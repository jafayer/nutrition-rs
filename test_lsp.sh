#!/bin/bash

# Test script for the Nutrition LSP server
# This sends a basic initialize request and shutdown sequence

echo "Building LSP server..."
cargo build --bin nutrition-lsp

echo -e "\nTesting LSP server..."

# Create a test initialization message
cat << 'EOF' | ./target/debug/nutrition-lsp
Content-Length: 246

{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":"file:///tmp","capabilities":{},"trace":"off","workspaceFolders":[{"uri":"file:///tmp","name":"test"}]}}
Content-Length: 53

{"jsonrpc":"2.0","method":"initialized","params":{}}
Content-Length: 45

{"jsonrpc":"2.0","id":2,"method":"shutdown"}
Content-Length: 46

{"jsonrpc":"2.0","method":"exit","params":{}}
EOF

echo -e "\nLSP test completed!"
