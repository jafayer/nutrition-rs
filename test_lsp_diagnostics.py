#!/usr/bin/env python3
"""
Test LSP server with invalid syntax to verify diagnostics
"""

import json
import subprocess
import sys
from pathlib import Path

def send_message(proc, method, params=None, msg_id=None):
    """Send an LSP message to the server"""
    message = {"jsonrpc": "2.0", "method": method}
    if msg_id is not None:
        message["id"] = msg_id
    if params is not None:
        message["params"] = params
    
    content = json.dumps(message)
    header = f"Content-Length: {len(content)}\r\n\r\n"
    full_message = header + content
    
    proc.stdin.write(full_message.encode())
    proc.stdin.flush()

def read_message(proc):
    """Read an LSP message from the server"""
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
    print("Testing LSP with invalid syntax...\n")
    
    lsp_path = Path("target/debug/nutrition-lsp")
    proc = subprocess.Popen([str(lsp_path)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    
    try:
        # Initialize
        send_message(proc, "initialize", {"processId": None, "rootUri": f"file://{Path.cwd()}", "capabilities": {}}, msg_id=1)
        read_message(proc)
        send_message(proc, "initialized", {})
        
        # Test with invalid syntax
        print("Testing with invalid syntax...")
        invalid_content = """
ingredient broken test {
    calories: "not a number"
}

recipe $$$ invalid name {
    servings: -1
}
"""
        
        send_message(proc, "textDocument/didOpen", {
            "textDocument": {
                "uri": "file:///tmp/test_invalid.nutrition",
                "languageId": "nutrition",
                "version": 1,
                "text": invalid_content
            }
        })
        
        response = read_message(proc)
        if response and response.get("method") == "textDocument/publishDiagnostics":
            diagnostics = response.get("params", {}).get("diagnostics", [])
            print(f"\n✓ Received {len(diagnostics)} diagnostic(s):")
            for i, diag in enumerate(diagnostics, 1):
                severity = {1: "ERROR", 2: "WARNING", 3: "INFO", 4: "HINT"}.get(diag.get("severity"), "UNKNOWN")
                line = diag.get("range", {}).get("start", {}).get("line", 0)
                message = diag.get("message", "")
                print(f"  {i}. [{severity}] Line {line}: {message}")
        else:
            print("✗ No diagnostics received")
        
        # Shutdown
        send_message(proc, "shutdown", msg_id=2)
        read_message(proc)
        send_message(proc, "exit")
        
        print("\n✅ Diagnostic test complete!")
        return 0
        
    except Exception as e:
        print(f"\n✗ Test failed: {e}")
        return 1
    finally:
        proc.terminate()
        proc.wait()

if __name__ == "__main__":
    sys.exit(main())
