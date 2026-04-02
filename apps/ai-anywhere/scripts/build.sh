#!/usr/bin/env bash
# Preferred: use the anywhere CLI.
#   cargo install --path path/to/anywhere/crates/anywhere-cli
#   anywhere build --app .
#
# This script is a fallback for environments without the CLI installed.
set -euo pipefail

REAL_SCRIPT="$(readlink -f "${BASH_SOURCE[0]}")"
APP_DIR="$(cd "$(dirname "$REAL_SCRIPT")/.." && pwd)"

if command -v anywhere &>/dev/null; then
  exec anywhere build --app "$APP_DIR"
fi

# --- Fallback: manual build (mirrors what `anywhere build` does) --------
RUNTIME_DIR="$APP_DIR/runtime"
DIST_DIR="$APP_DIR/dist/unpacked"
VIEWS_DIR="$APP_DIR/views"

WEBEXT_SRC="$(cargo metadata \
    --manifest-path "$RUNTIME_DIR/Cargo.toml" \
    --format-version 1 \
  | python3 -c "
import json, sys
meta = json.load(sys.stdin)
pkg = next(p for p in meta['packages'] if p['name'] == 'anywhere-webext')
print(pkg['manifest_path'].removesuffix('/Cargo.toml'))
")"
ASSETS_DIR="$WEBEXT_SRC/assets"

WORKSPACE_ROOT="$(cargo metadata \
    --manifest-path "$RUNTIME_DIR/Cargo.toml" \
    --format-version 1 \
  | python3 -c "import json,sys; print(json.load(sys.stdin)['workspace_root'])")"

mkdir -p "$DIST_DIR"

cargo build --manifest-path "$RUNTIME_DIR/Cargo.toml" \
  --target wasm32-unknown-unknown --release

VENDOR_TMP="$(mktemp -d)"
trap 'rm -rf "$VENDOR_TMP"' EXIT

wasm-bindgen \
  --target web \
  --out-dir "$VENDOR_TMP" \
  "$WORKSPACE_ROOT/target/wasm32-unknown-unknown/release/ai_anywhere_runtime.wasm"

DIST_VENDOR="$DIST_DIR/vendor"
mkdir -p "$DIST_VENDOR"
cp "$VENDOR_TMP/ai_anywhere_runtime.js"      "$DIST_VENDOR/runtime.js"
cp "$VENDOR_TMP/ai_anywhere_runtime_bg.wasm" "$DIST_VENDOR/runtime_bg.wasm"

rm -rf "$DIST_DIR/src" "$DIST_DIR/views"
mkdir -p "$DIST_DIR/src" "$DIST_DIR/views"

cp "$ASSETS_DIR/browser-shim.js" "$DIST_DIR/src/browser-shim.js"
cp "$ASSETS_DIR/background.js"   "$DIST_DIR/src/background.js"
cp "$ASSETS_DIR/content.js"      "$DIST_DIR/src/content.js"
cp "$ASSETS_DIR/popup.js"        "$DIST_DIR/src/popup.js"
cp "$ASSETS_DIR/popup.html"      "$DIST_DIR/src/popup.html"
cp "$ASSETS_DIR/popup.css"       "$DIST_DIR/src/popup.css"
cp "$ASSETS_DIR/content.css"     "$DIST_DIR/src/content.css"

cp -r "$VIEWS_DIR/." "$DIST_DIR/views/"

# Generate manifest.json from anywhere.toml + Cargo.toml version
python3 - "$APP_DIR" <<'PY'
import sys, json, re, pathlib
app = pathlib.Path(sys.argv[1])
# Parse version from runtime/Cargo.toml (simple regex, no TOML parser needed)
cargo_src = (app / "runtime/Cargo.toml").read_text()
version = re.search(r'^version\s*=\s*"([^"]+)"', cargo_src, re.M).group(1)
# Parse anywhere.toml with python tomllib (3.11+) or tomli
try:
    import tomllib
    cfg = tomllib.loads((app / "anywhere.toml").read_text())
except ImportError:
    import tomli as tomllib
    cfg = tomllib.loads((app / "anywhere.toml").read_text())
app_sec = cfg.get("app", {})
perms_sec = cfg.get("permissions", {})
host = perms_sec.get("host", ["<all_urls>"])
browser = ["storage"] + [p for p in perms_sec.get("browser", []) if p != "storage"]
manifest = {
    "manifest_version": 3,
    "name": app_sec.get("name", ""),
    "version": version,
    "description": app_sec.get("description", ""),
    "permissions": browser,
    "host_permissions": host,
    "background": {"service_worker": "src/background.js", "type": "module"},
    "action": {"default_popup": "src/popup.html", "default_title": app_sec.get("name", "")},
    "content_scripts": [{"matches": host, "js": ["src/content.js"], "css": ["src/content.css"], "run_at": "document_idle"}],
    "web_accessible_resources": [{"resources": ["vendor/runtime.js", "vendor/runtime_bg.wasm", "src/*", "views/*"], "matches": ["<all_urls>"]}],
}
(app / "dist/unpacked/manifest.json").write_text(json.dumps(manifest, indent=2))
PY

printf 'Built ai-anywhere at %s\n' "$DIST_DIR"
