#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
APP_DIR="$REPO_ROOT/apps/as-hello"
EXT_DIR="$APP_DIR/extension"
DIST_DIR="$APP_DIR/dist/unpacked"

# Framework assets: try sibling ../anywhere checkout first, then fall back to
# ANYWHERE_DIR env var. The assets are framework-owned JS/HTML/CSS.
ANYWHERE_DIR="${ANYWHERE_DIR:-$REPO_ROOT/../anywhere}"
if [ ! -d "$ANYWHERE_DIR/crates/anywhere-webext/assets" ]; then
  echo "Error: cannot find anywhere-webext assets. Set ANYWHERE_DIR or check out"
  echo "       https://github.com/semitechnological/anywhere next to this repo."
  exit 1
fi
ASSETS_DIR="$ANYWHERE_DIR/crates/anywhere-webext/assets"

# --- Install dependencies if needed -------------------------------------
if [ ! -d "$APP_DIR/node_modules" ]; then
  echo "Installing AssemblyScript..."
  npm install --prefix "$APP_DIR"
fi

# --- Compile AssemblyScript ---------------------------------------------
mkdir -p "$APP_DIR/build"
"$APP_DIR/node_modules/.bin/asc" \
  "$APP_DIR/assembly/index.ts" \
  --outFile "$APP_DIR/build/runtime_bg.wasm" \
  --exportRuntime \
  --optimizeLevel 2

# --- Assemble dist ------------------------------------------------------
rm -rf "$DIST_DIR/src" "$DIST_DIR/vendor"
mkdir -p "$DIST_DIR/src" "$DIST_DIR/vendor"

cp "$EXT_DIR/manifest.json" "$DIST_DIR/manifest.json"

# runtime.js = the AS adapter (wraps raw WASM in the wasm-bindgen interface)
cp "$ASSETS_DIR/runtime-as-adapter.js" "$DIST_DIR/vendor/runtime.js"
cp "$APP_DIR/build/runtime_bg.wasm"    "$DIST_DIR/vendor/runtime_bg.wasm"

# Framework bootstrap — same files as every other anywhere app
cp "$ASSETS_DIR/browser-shim.js" "$DIST_DIR/src/browser-shim.js"
cp "$ASSETS_DIR/background.js"   "$DIST_DIR/src/background.js"
cp "$ASSETS_DIR/popup.js"        "$DIST_DIR/src/popup.js"
cp "$ASSETS_DIR/popup.html"      "$DIST_DIR/src/popup.html"
cp "$ASSETS_DIR/popup.css"       "$DIST_DIR/src/popup.css"

printf 'Built as-hello at %s\n' "$DIST_DIR"
