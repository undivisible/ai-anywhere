use std::collections::BTreeMap;

use anywhere_core::plugin::{FrontendRenderRequest, PluginHost, PluginId};
use anywhere_crepuscularity::{plugin as crepuscularity_plugin, PLUGIN_ID as CREPUSCULARITY_PLUGIN_ID};
use anywhere_webext::api::{BrowserProgram, JsExpr, StorageArea};
use anywhere_webext::manifest::{ExtensionApp, ManifestSpec};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn app_definition() -> ExtensionApp {
    ExtensionApp::new(
        "quicknote",
        ManifestSpec {
            name: "quicknote".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Take quick notes on any page, stored locally in your browser.".to_string(),
        },
        CREPUSCULARITY_PLUGIN_ID,
    )
    .with_frontend(CREPUSCULARITY_PLUGIN_ID, "views/ui.crepus#NoteList")
}

fn plugin_host() -> Result<PluginHost, JsValue> {
    let mut host = PluginHost::new();
    host.register_frontend(crepuscularity_plugin())
        .map_err(|err| JsValue::from_str(&err))?;
    Ok(host)
}

fn render_entry(
    entry: &str,
    files: BTreeMap<String, String>,
    props: BTreeMap<String, Value>,
) -> Result<JsValue, JsValue> {
    let request = FrontendRenderRequest { entry: entry.to_string(), files, props };
    let host = plugin_host()?;
    let plugin_id = PluginId::new(CREPUSCULARITY_PLUGIN_ID);
    let rendered = host
        .render_frontend(&plugin_id, &request)
        .map_err(|err| JsValue::from_str(&err))?;
    serde_wasm_bindgen::to_value(&rendered).map_err(|err| JsValue::from_str(&err.to_string()))
}

// The note list template is compiled into the binary so the popup needs no
// network fetch. Editing the template requires a WASM rebuild.
const UI_CREPUS: &str = include_str!("../../views/ui.crepus");

fn template_files() -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    files.insert("views/ui.crepus".to_string(), UI_CREPUS.to_string());
    files
}

#[wasm_bindgen]
pub fn runtime_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen]
pub fn app_manifest() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&app_definition()).map_err(|err| JsValue::from_str(&err.to_string()))
}

/// Called by the framework `popup.js` on startup and after every action.
/// `state` is the raw `browser.storage.local` contents as a JS object.
/// Returns `{ html }` for the full popup body.
#[wasm_bindgen]
pub fn render_popup(state: JsValue) -> Result<JsValue, JsValue> {
    let state_map: BTreeMap<String, Value> =
        serde_wasm_bindgen::from_value(state).unwrap_or_default();

    let notes: Vec<Value> = state_map
        .get("quicknotes")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let note_count = notes.len();
    let empty = notes.is_empty();

    let mut props: BTreeMap<String, Value> = BTreeMap::new();
    props.insert("notes".to_string(), json!(notes));
    props.insert("note_count".to_string(), json!(note_count));
    props.insert("empty".to_string(), json!(empty));

    render_entry("views/ui.crepus#NoteList", template_files(), props)
}

/// Called by `popup.js` event delegation on `[data-action]` clicks.
/// Returns `{ storage_op? }` — an optional storage mutation for `popup.js` to apply.
///
/// Supported actions:
/// - `add-note`    data: { text: String }
///   → `{ storage_op: { type: "push", key: "quicknotes", item: { text } } }`
/// - `delete-note` data: { id: String }
///   → `{ storage_op: { type: "remove", key: "quicknotes", id } }`
/// - `clear-notes`
///   → `{ storage_op: { type: "set", key: "quicknotes", value: [] } }`
#[wasm_bindgen]
pub fn handle_popup_action(action: &str, data: JsValue) -> Result<JsValue, JsValue> {
    let data_map: BTreeMap<String, Value> =
        serde_wasm_bindgen::from_value(data).unwrap_or_default();

    let response = match action {
        "add-note" => {
            let text = data_map
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if text.is_empty() {
                json!({ "noop": true })
            } else {
                json!({
                    "storage_op": {
                        "type": "push",
                        "key": "quicknotes",
                        "item": { "text": text }
                    }
                })
            }
        }
        "delete-note" => {
            let id = data_map
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            json!({ "storage_op": { "type": "remove", "key": "quicknotes", "id": id } })
        }
        "clear-notes" => {
            json!({ "storage_op": { "type": "set", "key": "quicknotes", "value": [] } })
        }
        _ => json!({ "noop": true }),
    };

    serde_wasm_bindgen::to_value(&response).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
pub fn browser_program() -> String {
    BrowserProgram::new()
        .bind_storage("notes", StorageArea::Local, "quicknotes")
        .set_storage(StorageArea::Local, "quicknoteBooted", JsExpr::bool(true))
        .console_log([
            JsExpr::string("quicknote booted"),
            JsExpr::var("notes"),
            JsExpr::Literal(json!({ "framework": "anywhere", "app": "quicknote" })),
        ])
        .emit_module()
}
