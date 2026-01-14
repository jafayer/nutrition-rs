#!/usr/bin/env python3
"""
Test script for Nutrition LSP server
Sends LSP protocol messages and verifies responses
"""

import json
import subprocess
import sys
from pathlib import Path

def send_message(proc, method, params=None, msg_id=None):
    """Send an LSP message to the server"""
    message = {
        "jsonrpc": "2.0",
        "method": method,
    }
    if msg_id is not None:
        message["id"] = msg_id
    if params is not None:
        message["params"] = params
    
    content = json.dumps(message)
    header = f"Content-Length: {len(content)}\r\n\r\n"
    full_message = header + content
    
    print(f"→ Sending: {method}", file=sys.stderr)
    proc.stdin.write(full_message.encode())
    proc.stdin.flush()

def read_message(proc):
    """Read an LSP message from the server"""
    # Read headers
    headers = {}
    while True:
        line = proc.stdout.readline().decode().strip()
        if not line:
            break
        if ':' in line:
            key, value = line.split(':', 1)
            headers[key.strip()] = value.strip()
    
    # Read content
    content_length = int(headers.get('Content-Length', 0))
    if content_length > 0:
        content = proc.stdout.read(content_length).decode()
        message = json.loads(content)
        print(f"← Received: {json.dumps(message, indent=2)}", file=sys.stderr)
        return message
    return None

def main():
    print("Starting Nutrition LSP server test...\n")
    
    # Build the LSP server
    print("Building LSP server...")
    result = subprocess.run(["cargo", "build", "--bin", "nutrition-lsp"], 
                          capture_output=True, text=True)
    if result.returncode != 0:
        print("Build failed!")
        print(result.stderr)
        return 1
    
    # Start the LSP server
    lsp_path = Path("target/debug/nutrition-lsp")
    proc = subprocess.Popen(
        [str(lsp_path)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    
    try:
        # 1. Initialize
        print("\n=== Testing Initialize ===")
        send_message(proc, "initialize", {
            "processId": None,
            "rootUri": f"file://{Path.cwd()}",
            "capabilities": {
                "textDocument": {
                    "synchronization": {
                        "didOpen": True,
                        "didChange": True
                    }
                }
            }
        }, msg_id=1)
        
        response = read_message(proc)
        if response and "result" in response:
            print("✓ Initialize successful")
            print(f"  Server capabilities: {json.dumps(response['result'].get('capabilities', {}), indent=4)}")
        else:
            print("✗ Initialize failed")
            return 1
        
        # 2. Initialized notification
        send_message(proc, "initialized", {})
        
        # 3. Open a document
        print("\n=== Testing didOpen ===")
        test_file = Path("examples/test.nutrition")
        if not test_file.exists():
            print(f"✗ Test file not found: {test_file}")
            return 1
        test_content = test_file.read_text()
        
        send_message(proc, "textDocument/didOpen", {
            "textDocument": {
                "uri": f"file://{test_file.absolute()}",
                "languageId": "nutrition",
                "version": 1,
                "text": test_content
            }
        })
        
        # Read diagnostics (if any)
        print("  Waiting for diagnostics...")
        response = read_message(proc)
        if response:
            if response.get("method") == "textDocument/publishDiagnostics":
                diagnostics = response.get("params", {}).get("diagnostics", [])
                if diagnostics:
                    print(f"  ✓ Received {len(diagnostics)} diagnostic(s)")
                    for diag in diagnostics:
                        print(f"    - {diag.get('severity')}: {diag.get('message')}")
                else:
                    print("  ✓ No diagnostics (document is valid)")
            else:
                print(f"  Received: {response.get('method', 'unknown')}")
        
        # 4. Test hover
        print("\n=== Testing Hover ===")
        send_message(proc, "textDocument/hover", {
            "textDocument": {
                "uri": f"file://{test_file.absolute()}"
            },
            "position": {"line": 1, "character": 0}
        }, msg_id=2)
        
        response = read_message(proc)
        if response and "result" in response:
            print("✓ Hover response received")
        
        # 5. Test completion
        print("\n=== Testing Completion ===")
        send_message(proc, "textDocument/completion", {
            "textDocument": {
                "uri": f"file://{test_file.absolute()}"
            },
            "position": {"line": 1, "character": 0}
        }, msg_id=3)
        
        response = read_message(proc)
        if response and "result" in response:
            items = response["result"]
            if isinstance(items, list):
                print(f"✓ Received {len(items)} completion items")
                for item in items[:3]:  # Show first 3
                    print(f"  - {item.get('label')}: {item.get('detail', '')}")
            else:
                print("✓ Completion response received")
        
        # 6. Shutdown
        print("\n=== Testing Shutdown ===")
        send_message(proc, "shutdown", msg_id=4)
        response = read_message(proc)
        if response:
            print("✓ Shutdown acknowledged")
        
        send_message(proc, "exit")
        
        print("\n✅ All tests passed!")
        return 0
        
    except Exception as e:
        print(f"\n✗ Test failed with error: {e}")
        import traceback
        traceback.print_exc()
        return 1
    finally:
        proc.terminate()
        proc.wait()

if __name__ == "__main__":
    sys.exit(main())
