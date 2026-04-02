# anywhere

The main Anywhere AI extension product - browser extensions built with
crepuscularity-anywhere plugin.

App authors write **Rust + `.crepus` templates only**. All JS bootstrap is
framework-owned and lives in the crepuscularity-anywhere-webext crate.

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

## Architecture

Each app consists of:

- `runtime/src/lib.rs` — WASM entry points
- `views/ui.crepus` — crepus component templates
- Build scripts generate manifest.json from TOML config

The framework supplies browser bootstrap JavaScript.
