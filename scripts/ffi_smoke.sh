#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[ffi-smoke] Building release library..."
cargo build --release

LIB_DIR="target/release"
EXAMPLE_SRC="examples/ffi_example.c"
EXAMPLE_BIN="examples/ffi_example"

echo "[ffi-smoke] Compiling C example..."
cc -o "$EXAMPLE_BIN" "$EXAMPLE_SRC" -L "$LIB_DIR" -lnutrition_rs

echo "[ffi-smoke] Running C example..."
OUTPUT="$(LD_LIBRARY_PATH="$LIB_DIR" "$EXAMPLE_BIN")"
echo "$OUTPUT"

if ! grep -q '"ok":true' <<<"$OUTPUT"; then
  echo "[ffi-smoke] ERROR: expected successful JSON envelope with \"ok\":true" >&2
  exit 1
fi

echo "[ffi-smoke] PASS"
