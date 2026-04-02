use std::collections::BTreeMap;

use anywhere_core::plugin::{FrontendRenderRequest, PluginHost};
use anywhere_core::widget::{extract_anywhere_widgets, extract_widget_specs};
use anywhere_crepuscularity::{plugin as crepuscularity_plugin, PLUGIN_ID as CREPUSCULARITY_PLUGIN_ID};
use crepuscularity_anywhere_webext::api::{BrowserProgram, JsExpr, MessagePayload, StorageArea};
use crepuscularity_anywhere_webext::manifest::{ExtensionApp, ManifestSpec};
use serde::Deserialize;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

#[derive(Debug, Deserialize)]
struct RenderRequest {
    entry: Option<String>,
    files: BTreeMap<String, String>,
    props: BTreeMap<String, Value>,
}

fn app_definition() -> ExtensionApp {
    ExtensionApp::new(
        "anywhere",
        ManifestSpec {
            name: "Anywhere".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Render AI-generated widgets from code blocks on any page.".to_string(),
        },
        CREPUSCULARITY_PLUGIN_ID,
    )
    .with_frontend(CREPUSCULARITY_PLUGIN_ID, "views/ui.crepus#Panel")
}

fn plugin_host() -> Result<PluginHost, JsValue> {
    let mut host = PluginHost::new();
    host.register_frontend(crepuscularity_plugin())
        .map_err(|err| JsValue::from_str(&err))?;
    Ok(host)
}

#[wasm_bindgen]
pub fn runtime_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen]
pub fn extract_specs(message: &str) -> Result<JsValue, JsValue> {
    let specs = extract_widget_specs(message);
    serde_wasm_bindgen::to_value(&specs).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
pub fn extract_widgets(message: &str) -> Result<JsValue, JsValue> {
    let widgets = extract_anywhere_widgets(message);
    serde_wasm_bindgen::to_value(&widgets).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
pub fn render_crepus(request: JsValue) -> Result<JsValue, JsValue> {
    let request: RenderRequest =
        serde_wasm_bindgen::from_value(request).map_err(|err| JsValue::from_str(&err.to_string()))?;
    let app = app_definition();
    let binding = app
        .default_frontend_binding()
        .ok_or_else(|| JsValue::from_str("no default frontend binding configured"))?;
    let plugin_request = FrontendRenderRequest {
        entry: request.entry.unwrap_or_else(|| binding.entry.clone()),
        files: request.files,
        props: request.props,
    };
    let host = plugin_host()?;
    let rendered = host
        .render_frontend(&binding.plugin_id, &plugin_request)
        .map_err(|err| JsValue::from_str(&err))?;
    serde_wasm_bindgen::to_value(&rendered).map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen]
pub fn render_frontend(request: JsValue) -> Result<JsValue, JsValue> {
    render_crepus(request)
}

#[wasm_bindgen]
pub fn app_manifest() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&app_definition()).map_err(|err| JsValue::from_str(&err.to_string()))
}

/// Returns a browser MV3 manifest.json string, generated entirely from Rust.
/// The build CLI calls this after compiling WASM to produce dist/manifest.json.
#[wasm_bindgen]
pub fn generate_manifest() -> String {
    app_definition().to_manifest_v3()
}

#[wasm_bindgen]
pub fn browser_program() -> String {
    BrowserProgram::new()
        .bind_storage("auto_render", StorageArea::Sync, "autoRender")
        .bind_runtime_message(
            "settings",
            MessagePayload::new().with_field("type", JsExpr::string("settings:get")),
        )
        .set_storage(StorageArea::Local, "aiAnywhereBooted", JsExpr::bool(true))
        .console_log([
            JsExpr::string("ai-anywhere booted"),
            JsExpr::var("settings"),
            JsExpr::var("auto_render"),
            JsExpr::Literal(json!({"framework":"anywhere","frontend_plugin":"crepuscularity"})),
        ])
        .emit_module()
}
