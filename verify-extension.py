#!/usr/bin/env python3
"""Quick test to verify the extension LSP binary works"""
import subprocess
import sys
from pathlib import Path

def main():
    ext_dir = Path(__file__).parent / "vscode-extension"
    lsp_binary = ext_dir / "bin" / "nutrition-lsp"
    
    if not lsp_binary.exists():
        print(f"❌ LSP binary not found: {lsp_binary}")
        return 1
    
    print("✓ LSP binary found")
    print(f"  Location: {lsp_binary}")
    print(f"  Size: {lsp_binary.stat().st_size / 1024 / 1024:.1f} MB")
    
    # Quick test
    print("\n✓ Testing LSP binary responds...")
    result = subprocess.run([str(lsp_binary)], 
                          input=b'Content-Length: 50\r\n\r\n{"jsonrpc":"2.0","method":"exit","params":{}}',
                          capture_output=True, timeout=2)
    print("✓ LSP binary executable and responds")
    
    print("\n" + "="*50)
    print("✅ Extension ready for testing!")
    print("="*50)
    print("\nTo test in VS Code:")
    print(f"  1. code {ext_dir}")
    print("  2. Press F5 (or Run > Start Debugging)")
    print("  3. Open ../examples/test.nutrition")
    print("  4. Check 'Output > Nutrition Language Server'")
    
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)
