#!/usr/bin/env python3
"""
Test semantic tokens response from LSP server
"""

import json
import subprocess
import sys
from pathlib import Path

def send_message(proc, method, params=None, msg_id=None):
    message = {"jsonrpc": "2.0", "method": method}
    if msg_id is not None:
        message["id"] = msg_id
    if params is not None:
        message["params"] = params
    
    content = json.dumps(message)
    header = f"Content-Length: {len(content)}\r\n\r\n"
    proc.stdin.write((header + content).encode())
    proc.stdin.flush()

def read_message(proc):
    headers = {}
    while True:
        line = proc.stdout.readline().decode().strip()
        if not line:
            break
        if ':' in line:
            key, value = line.split(':', 1)
            headers[key.strip()] = value.strip()
    
    content_length = int(headers.get('Content-Length', 0))
    if content_length > 0:
        content = proc.stdout.read(content_length).decode()
        return json.loads(content)
    return None

def main():
    print("Testing Semantic Tokens Support\n")
    
    lsp_path = Path("target/release/nutrition-lsp")
    if not lsp_path.exists():
        lsp_path = Path("target/debug/nutrition-lsp")
    
    proc = subprocess.Popen([str(lsp_path)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    
    try:
        # Initialize
        send_message(proc, "initialize", {
            "processId": None,
            "rootUri": f"file://{Path.cwd()}",
            "capabilities": {
                "textDocument": {
                    "semanticTokens": {
                        "requests": {"full": True},
                        "tokenTypes": ["keyword", "string", "number", "property", "variable", "type", "comment"],
                        "tokenModifiers": []
                    }
                }
            }
        }, msg_id=1)
        
        response = read_message(proc)
        if response and "result" in response:
            caps = response["result"]["capabilities"]
            if "semanticTokensProvider" in caps:
                print("✅ Server supports semantic tokens!")
                legend = caps["semanticTokensProvider"]["legend"]
                print(f"\n📋 Token Types:")
                for i, tt in enumerate(legend["tokenTypes"]):
                    print(f"  {i}: {tt}")
            else:
                print("❌ Server does not support semantic tokens")
                return 1
        
        send_message(proc, "initialized", {})
        
        # Open test file
        test_file = Path("examples/test.nutrition")
        if not test_file.exists():
            print(f"\n❌ Test file not found: {test_file}")
            return 1
        
        content = test_file.read_text()
        print(f"\n📄 Testing with: {test_file}")
        print(f"   ({len(content)} bytes, {content.count(chr(10))} lines)")
        
        send_message(proc, "textDocument/didOpen", {
            "textDocument": {
                "uri": f"file://{test_file.absolute()}",
                "languageId": "nutrition",
                "version": 1,
                "text": content
            }
        })
        
        # Consume diagnostics
        read_message(proc)
        
        # Request semantic tokens
        print("\n🎨 Requesting semantic tokens...")
        send_message(proc, "textDocument/semanticTokens/full", {
            "textDocument": {
                "uri": f"file://{test_file.absolute()}"
            }
        }, msg_id=2)
        
        response = read_message(proc)
        if response and "result" in response:
            tokens = response["result"].get("data", [])
            print(f"✅ Received {len(tokens)} semantic tokens!")
            
            if tokens:
                print(f"\n🔍 Sample tokens (first 5):")
                token_names = ["keyword", "string", "number", "property", "variable", "type", "comment"]
                for i, token in enumerate(tokens[:5]):
                    token_type = token_names[token["tokenType"]] if token["tokenType"] < len(token_names) else "unknown"
                    print(f"  {i+1}. Line {token['deltaLine']}, Col {token['deltaStart']}, "
                          f"Len {token['length']}, Type: {token_type}")
        else:
            print("❌ No semantic tokens received")
            return 1
        
        # Shutdown
        send_message(proc, "shutdown", msg_id=3)
        read_message(proc)
        send_message(proc, "exit")
        
        print("\n✅ Semantic highlighting test passed!")
        print("\nNext: Test in VS Code Extension Development Host (F5)")
        return 0
        
    except Exception as e:
        print(f"\n❌ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return 1
    finally:
        proc.terminate()
        proc.wait()

if __name__ == "__main__":
    sys.exit(main())
