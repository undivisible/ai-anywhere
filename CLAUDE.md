# CLAUDE.md

App authors write **Rust + `.crepus` only**. No hand-written JS.

## Scope

- `apps/ai-anywhere` — AI widget renderer: scans AI assistant pages for code blocks
- `apps/quicknote` — WASM-driven popup note-taker

## Dependencies

Framework crates live in `../anywhere/crates/`. Both repos must be checked out
side-by-side for Cargo path deps to resolve.

## Build

```bash
bash apps/ai-anywhere/scripts/build.sh
bash apps/quicknote/scripts/build.sh
```

## Rules

- All extension logic stays in Rust + `.crepus`.
- The JS-visible API surface is only `#[wasm_bindgen]`-exported functions.
- Do not add handwritten JavaScript inside `apps/`.
- Framework JS assets come from `../anywhere/crates/anywhere-webext/assets/`.
  Update the framework, not the apps, if browser bootstrap needs to change.

## WASM-driven popup protocol

Apps that export `render_popup(state: JsValue) -> Result<JsValue, JsValue>` get
the full WASM-driven popup loop from the framework `popup.js`:

1. `popup.js` calls `render_popup(storage_state)` on startup and after each action.
2. `render_popup` returns `{ html }` — the full popup innerHTML.
3. Click events on `[data-action]` elements call `handle_popup_action(action, data)`.
4. `handle_popup_action` returns `{ storage_op? }` — optional storage mutation.
5. `popup.js` applies the storage op and re-renders.

Supported `storage_op` types: `push`, `remove`, `set`.
