# ai-anywhere

Example apps built with the [anywhere](../anywhere) framework — a Rust-first toolkit
for browser extensions.

App authors write **Rust + `.crepus` templates only**. All JS bootstrap is
framework-owned and lives in `../anywhere/crates/anywhere-webext/assets/`.

## Apps

| App | Description |
|-----|-------------|
| `apps/ai-anywhere` | Scan AI assistant pages for widget code blocks and render them in sandboxed iframes |
| `apps/quicknote` | WASM-driven popup note-taker: add, list, and delete notes stored in `browser.storage.local` |

## Building

Prerequisites: Rust + `wasm32-unknown-unknown` target + `wasm-bindgen-cli`.

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli

# ai-anywhere
bash apps/ai-anywhere/scripts/build.sh

# quicknote
bash apps/quicknote/scripts/build.sh
```

Each build produces an unpacked extension at `apps/<name>/dist/unpacked/`
that can be loaded directly into Chrome or Firefox (Developer Mode).

## Dependency layout

```
/home/undivisible/
├── anywhere/          ← framework (crates/anywhere-*)
├── crepuscularity/    ← crepus DSL runtime (used by anywhere-crepuscularity)
└── ai-anywhere/       ← this repo (apps)
```

Both `anywhere` and `ai-anywhere` must be checked out side-by-side for the
workspace path dependencies to resolve.

## Architecture

Each app consists of:

- `runtime/src/lib.rs` — WASM entry points (`browser_program()`, `render_frontend()`,
  optional `render_popup()` + `handle_popup_action()`)
- `views/ui.crepus` — crepus component templates
- `extension/manifest.json` — MV3 extension manifest
- `extension/src/*.html` + `*.css` — HTML shells and styling (no JS)

The framework (`anywhere-webext`) supplies:
- `browser-shim.js` — unified Chrome/Firefox API bridge
- `background.js` — service worker with settings messaging
- `content.js` — WASM boot + widget scanning (for ai-anywhere style apps)
- `popup.js` — dual-mode: WASM-driven render loop or default settings UI
