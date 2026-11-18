#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRIPLE="${1:-$(rustc -vV | sed -n 's|host: ||p')}"

# Determine target directory and normalize Windows backslashes if present
RAW_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
TARGET_DIR="${RAW_TARGET_DIR//\\//}"

EXT=""
if [[ "$TRIPLE" == *"windows"* ]] || [[ "$TRIPLE" == *"msvc"* ]]; then
  EXT=".exe"
fi

echo "==> Building notare-mcp for target: $TRIPLE"
cargo build --release -p notare-mcp --target "$TRIPLE"

SRC_BIN="$TARGET_DIR/$TRIPLE/release/notare-mcp$EXT"
if [[ ! -f "$SRC_BIN" ]]; then
  # Fallback if cargo did not use target subdirectory
  SRC_BIN="$TARGET_DIR/release/notare-mcp$EXT"
fi

if [[ ! -f "$SRC_BIN" ]]; then
  echo "Error: built binary not found at $SRC_BIN" >&2
  exit 1
fi

DEST_DIR="$ROOT/apps/desktop/src-tauri/binaries"
mkdir -p "$DEST_DIR"
DEST_BIN="$DEST_DIR/notare-mcp-$TRIPLE$EXT"

echo "==> Copying $SRC_BIN to $DEST_BIN"
cp "$SRC_BIN" "$DEST_BIN"
chmod +x "$DEST_BIN" 2>/dev/null || true

echo "==> Sidecar successfully built and staged at: $DEST_BIN"
