use std::collections::{BTreeMap, HashMap};

use crepuscularity_core::context::TemplateContext;
use crepuscularity_web::render_from_files;
use crepuscularity_webext::{
    build_frame_doc, json_to_template, BrowserProgram, JsExpr, MessagePayload, StorageArea,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// App-specific embedded assets (compiled into the WASM binary)
// ---------------------------------------------------------------------------

const POPUP_VIEW: &str = include_str!("../../views/popup.crepus");
const UI_VIEW: &str = include_str!("../../views/ui.crepus");

const POPUP_CSS: &str = r#"
*,*::before,*::after{box-sizing:border-box}
body{margin:0;min-width:300px;font-family:system-ui,"Segoe UI",sans-serif;background:#0f0f17;color:#e0e0f0}
.popup{display:flex;flex-direction:column;padding:14px 16px 16px;gap:10px}
.popup--help{padding:14px 16px 20px;gap:8px}
.popup__eyebrow{font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:.12em;color:#5a5a8a}
.popup__header{display:flex;align-items:center;justify-content:space-between}
.popup__title{margin:0;font-size:15px;font-weight:700;color:#e8e8ff}
.popup__copy{margin:0;font-size:12px;line-height:1.5;color:#8888aa}
.popup__toggle{display:flex;align-items:center;gap:8px;cursor:pointer;font-size:13px;color:#c0c0e0}
.popup__toggle input{accent-color:#7b7bde;width:15px;height:15px;cursor:pointer}
.popup__help-btn{background:none;border:1px solid #333355;border-radius:50%;width:22px;height:22px;color:#7777aa;font-size:13px;font-weight:700;cursor:pointer;line-height:1;padding:0;display:flex;align-items:center;justify-content:center;flex-shrink:0}
.popup__help-btn:hover{background:#1a1a2e;color:#9999cc}
.popup__nav{margin-bottom:2px}
.popup__back{background:none;border:none;color:#7b7bde;font-size:12px;cursor:pointer;padding:0;font-weight:600}
.popup__back:hover{color:#9999ff}
.popup__help-section{display:flex;flex-direction:column;gap:4px}
.popup__help-heading{margin:0;font-size:11px;font-weight:700;text-transform:uppercase;letter-spacing:.08em;color:#7b7bde}
.popup__prompt{margin:0;padding:10px 12px;background:#0a0a14;border:1px solid #222240;border-radius:6px;font-size:10.5px;line-height:1.55;color:#a0a0c8;white-space:pre-wrap;word-break:break-all;font-family:"IBM Plex Mono",ui-monospace,monospace;overflow-y:auto;max-height:320px}
.popup--help{overflow-y:auto;max-height:520px}
.popup__crepus-btn{width:100%;padding:8px 12px;background:#1a1a30;border:1px solid #333360;border-radius:7px;color:#9090dd;font-size:12px;font-weight:600;cursor:pointer;text-align:left}
.popup__crepus-btn:hover{background:#20203a;color:#b0b0ff}
"#;

/// All `.crepus` views embedded at compile time — no filesystem access at runtime.
fn embedded_files() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("views/ui.crepus".to_string(), UI_VIEW.to_string());
    map.insert("views/popup.crepus".to_string(), POPUP_VIEW.to_string());
    map
}

// ---------------------------------------------------------------------------
// Anywhere widget types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WidgetSpec {
    pub id: String,
    pub title: String,
    pub html: String,
    pub css: String,
    pub js: String,
    pub source: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UiLang {
    Crepus,
    Html,
    Builtin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScriptLang {
    Js,
    Mermaid,
    Latex,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnywhereUi {
    pub lang: UiLang,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnywhereScript {
    pub lang: ScriptLang,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnywhereWidget {
    pub id: Option<String>,
    pub widget_type: Option<String>,
    pub title: Option<String>,
    pub ui: Option<AnywhereUi>,
    pub script: Option<AnywhereScript>,
    pub data: Option<String>,
}

// ---------------------------------------------------------------------------
// WASM input types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RenderRequest {
    entry: String,
    #[serde(default)] files: HashMap<String, String>,
    #[serde(default)] props: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct FrameDocRequest {
    #[serde(default)] html: String,
    #[serde(default)] css: String,
    #[serde(default)] js: String,
    #[serde(default)] unocss: String,
}

#[derive(Deserialize)]
struct AnywhereFrameDocRequest {
    #[serde(default)] unocss: String,
    #[serde(flatten)] widget: AnywhereWidget,
}

// ---------------------------------------------------------------------------
// Anywhere widget parsing — <ai-anywhere> tags
// ---------------------------------------------------------------------------

fn parse_anywhere_tags(text: &str) -> Vec<AnywhereWidget> {
    let mut widgets = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        let remaining = &text[pos..];
        let Some(rel_start) = remaining.find("<ai-anywhere") else { break };
        let abs_start = pos + rel_start;
        let after_open = &text[abs_start + "<ai-anywhere".len()..];
        let Some(tag_close) = find_tag_close(after_open) else { pos = abs_start + 1; continue };
        let attrs = &after_open[..tag_close];
        let id = attr_value(attrs, "id");
        let widget_type = attr_value(attrs, "type");
        let title = attr_value(attrs, "title");
        let inner_start = abs_start + "<ai-anywhere".len() + tag_close + 1;
        let Some(inner_len) = find_close_tag(&text[inner_start..], "ai-anywhere") else {
            pos = abs_start + 1; continue
        };
        let inner = &text[inner_start..inner_start + inner_len];
        let ui = extract_section(inner, "anywhere-ui").map(|(a, c)| AnywhereUi {
            lang: match attr_value(&a, "lang").as_deref() {
                Some("html") => UiLang::Html,
                _ => UiLang::Crepus,
            },
            source: c,
        });
        let script = extract_section(inner, "anywhere-script").map(|(a, c)| AnywhereScript {
            lang: match attr_value(&a, "lang").as_deref() {
                Some("mermaid") => ScriptLang::Mermaid,
                Some("latex" | "katex" | "tex") => ScriptLang::Latex,
                _ => ScriptLang::Js,
            },
            source: c,
        });
        let data = extract_section(inner, "anywhere-data").map(|(_, c)| c);
        widgets.push(AnywhereWidget { id, widget_type, title, ui, script, data });
        pos = inner_start + inner_len + "</ai-anywhere>".len();
    }
    widgets
}

fn find_tag_close(s: &str) -> Option<usize> {
    let mut in_quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match (in_quote, c) {
            (None, '"' | '\'') => in_quote = Some(c),
            (Some(q), c) if c == q => in_quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

fn find_close_tag(s: &str, tag: &str) -> Option<usize> {
    let open_pat = format!("<{tag}");
    let close_pat = format!("</{tag}>");
    let mut depth = 1usize;
    let mut pos = 0;
    while pos < s.len() {
        let rest = &s[pos..];
        match (rest.find(&open_pat), rest.find(&close_pat)) {
            (None, None) => break,
            (None, Some(c)) => {
                depth -= 1;
                if depth == 0 { return Some(pos + c); }
                pos += c + close_pat.len();
            }
            (Some(o), None) => { depth += 1; pos += o + open_pat.len(); }
            (Some(o), Some(c)) => {
                if o < c { depth += 1; pos += o + open_pat.len(); }
                else {
                    depth -= 1;
                    if depth == 0 { return Some(pos + c); }
                    pos += c + close_pat.len();
                }
            }
        }
    }
    None
}

fn extract_section(s: &str, tag: &str) -> Option<(String, String)> {
    let open_pat = format!("<{tag}");
    let start = s.find(&open_pat)?;
    let after = &s[start + open_pat.len()..];
    let tag_end = find_tag_close(after)?;
    let attrs = after[..tag_end].trim().to_string();
    let content_start = start + open_pat.len() + tag_end + 1;
    let close_pat = format!("</{tag}>");
    let content_end = s[content_start..].find(&close_pat)?;
    Some((attrs, s[content_start..content_start + content_end].trim().to_string()))
}

fn attr_value(attrs: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pat = format!("{name}={quote}");
        if let Some(pos) = attrs.find(&pat) {
            let after = &attrs[pos + pat.len()..];
            if let Some(end) = after.find(quote) {
                return Some(after[..end].to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Anywhere widget parsing — legacy code-block format
// ---------------------------------------------------------------------------

fn extract_widget_specs(message: &str) -> Vec<WidgetSpec> {
    let fences = parse_fences(message);
    let mut specs = extract_json_specs(&fences);
    specs.extend(extract_triplet_specs(&fences));
    specs
}

struct Fence { lang: String, body: String }

fn parse_fences(message: &str) -> Vec<Fence> {
    let mut fences = Vec::new();
    let mut lines = message.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with("```") { continue; }
        let lang = trimmed.trim_start_matches("```").trim().to_lowercase();
        let mut body = Vec::new();
        for inner in lines.by_ref() {
            if inner.trim() == "```" { break; }
            body.push(inner);
        }
        fences.push(Fence { lang, body: body.join("\n") });
    }
    fences
}

fn extract_json_specs(fences: &[Fence]) -> Vec<WidgetSpec> {
    fences.iter().enumerate().filter_map(|(idx, f)| {
        if !matches!(f.lang.as_str(), "aiwidget" | "widget" | "widget-json") { return None; }
        #[derive(Deserialize)]
        struct Input {
            #[serde(default)] id: Option<String>,
            #[serde(default)] title: Option<String>,
            #[serde(default)] html: Option<String>,
            #[serde(default)] css: Option<String>,
            #[serde(default)] js: Option<String>,
        }
        let input: Input = serde_json::from_str(&f.body).ok()?;
        Some(WidgetSpec {
            id: input.id.unwrap_or_else(|| format!("widget-json-{idx}")),
            title: input.title.unwrap_or_else(|| format!("Widget {}", idx + 1)),
            html: input.html.unwrap_or_default(),
            css: input.css.unwrap_or_default(),
            js: input.js.unwrap_or_default(),
            source: format!("json:{idx}"),
            format: "json".to_string(),
        })
    }).collect()
}

fn extract_triplet_specs(fences: &[Fence]) -> Vec<WidgetSpec> {
    let mut out = Vec::new();
    let mut pending = WidgetSpec::default();
    let mut found = false;
    for (idx, f) in fences.iter().enumerate() {
        match f.lang.as_str() {
            "widget-html" | "aiwidget-html" => {
                if found && !pending.html.is_empty() {
                    out.push(finalize(&pending, out.len()));
                    pending = WidgetSpec::default();
                }
                pending.html = f.body.clone();
                pending.source = format!("fence:{idx}");
                pending.format = "triplet".to_string();
                found = true;
            }
            "widget-css" | "aiwidget-css" => { pending.css = f.body.clone(); found = true; }
            "widget-js" | "aiwidget-js" | "javascript" | "js" => {
                if found && pending.js.is_empty() && !pending.html.is_empty() {
                    pending.js = f.body.clone();
                }
            }
            _ => {}
        }
    }
    if found && !pending.html.is_empty() { out.push(finalize(&pending, out.len())); }
    out
}

fn finalize(spec: &WidgetSpec, idx: usize) -> WidgetSpec {
    let mut s = spec.clone();
    if s.id.is_empty() { s.id = format!("widget-triplet-{idx}"); }
    if s.title.is_empty() { s.title = format!("Widget {}", idx + 1); }
    s
}

// ---------------------------------------------------------------------------
// Anywhere widget frame doc
// ---------------------------------------------------------------------------

fn anywhere_frame_doc_parts(widget: &AnywhereWidget) -> (String, String, String) {
    let data: Value = widget.data
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({}));

    let mut html = String::new();
    let css = String::new();
    let mut js = String::new();

    if let Some(ui) = &widget.ui {
        match ui.lang {
            UiLang::Html => { html = ui.source.clone(); }
            UiLang::Crepus => {
                let mut files = HashMap::new();
                files.insert("__ai_widget__".to_string(), ui.source.clone());
                let mut ctx = TemplateContext::new();
                if let Value::Object(obj) = &data {
                    for (k, v) in obj {
                        ctx.set(k, json_to_template(v.clone()));
                    }
                }
                match render_from_files(&files, "__ai_widget__#Widget", &ctx) {
                    Ok(h) => html = h,
                    Err(e) => {
                        html = format!("<pre style=\"color:red\">Crepus render error: {e}</pre>");
                    }
                }
            }
            UiLang::Builtin => {}
        }
    }

    if let Some(script) = &widget.script {
        match script.lang {
            ScriptLang::Mermaid => {
                html = format!("<div class=\"mermaid\">{}</div>", script.source);
                js = "import('https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.esm.min.mjs').then(m=>m.default.initialize({startOnLoad:true}));".to_string();
            }
            ScriptLang::Latex => {
                html = format!("<div class=\"latex\">{}</div>", script.source);
                js = "import('https://cdn.jsdelivr.net/npm/katex/dist/katex.mjs').then(m=>{document.querySelectorAll('.latex').forEach(el=>m.default.render(el.textContent,el,{throwOnError:false}));});".to_string();
            }
            ScriptLang::Js | ScriptLang::Other => { js = script.source.clone(); }
        }
    }

    (html, css, js)
}

// ---------------------------------------------------------------------------
// WASM exports
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub fn runtime_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Render a `.crepus` template. Views in `views/` are embedded in the WASM
/// binary. The optional `files` map can supply additional or override templates.
/// Input: `{ entry: "path#Component", props: {...}, files?: {path: content} }`
/// Returns: `{ html: string }`
#[wasm_bindgen]
pub fn render_frontend(request: JsValue) -> Result<JsValue, JsValue> {
    let req: RenderRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut ctx = TemplateContext::new();
    for (k, v) in req.props {
        ctx.set(k, json_to_template(v));
    }
    let mut files = embedded_files();
    files.extend(req.files);
    let html = render_from_files(&files, &req.entry, &ctx)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&json!({ "html": html }))
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Build the srcdoc for a widget iframe.
/// Input: `{ html, css, js, unocss? }`  Returns: `{ srcdoc: string }`
#[wasm_bindgen]
pub fn render_frame_doc(request: JsValue) -> Result<JsValue, JsValue> {
    let req: FrameDocRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let srcdoc = build_frame_doc(
        &req.html, &req.css, &req.js, &req.unocss,
        "<p>Widget had no HTML payload.</p>",
    );
    serde_wasm_bindgen::to_value(&json!({ "srcdoc": srcdoc }))
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Build the srcdoc for an `<ai-anywhere>` widget iframe.
/// Input: widget object + optional `unocss` field.  Returns: `{ srcdoc: string }`
#[wasm_bindgen]
pub fn render_anywhere_frame_doc(request: JsValue) -> Result<JsValue, JsValue> {
    let req: AnywhereFrameDocRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let (html, css, js) = anywhere_frame_doc_parts(&req.widget);
    let srcdoc = build_frame_doc(&html, &css, &js, &req.unocss, "<p>Widget had no content.</p>");
    serde_wasm_bindgen::to_value(&json!({ "srcdoc": srcdoc }))
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse `<ai-anywhere>` widgets from an AI message.
#[wasm_bindgen]
pub fn extract_widgets(message: &str) -> Result<JsValue, JsValue> {
    let widgets = parse_anywhere_tags(message);
    serde_wasm_bindgen::to_value(&widgets)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse legacy ``` code-block widget specs from an AI message.
#[wasm_bindgen]
pub fn extract_specs(message: &str) -> Result<JsValue, JsValue> {
    let specs: Vec<WidgetSpec> = extract_widget_specs(message);
    serde_wasm_bindgen::to_value(&specs)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Render the settings popup from storage state.
/// Input: `{ enabled: bool, autoRender: bool }`  Returns: `{ html: string, css: string }`
#[wasm_bindgen]
pub fn render_popup(state: JsValue) -> Result<JsValue, JsValue> {
    let state: Value = serde_wasm_bindgen::from_value(state)
        .unwrap_or_else(|_| json!({}));
    let enabled = state.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let auto_render = state.get("autoRender").and_then(|v| v.as_bool()).unwrap_or(false);
    let show_help = state.get("showHelp").and_then(|v| v.as_bool()).unwrap_or(false);
    let show_crepus = state.get("showCrepus").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut ctx = TemplateContext::new();
    ctx.set("enabled", enabled);
    ctx.set("auto_render", auto_render);
    ctx.set("show_help", show_help);
    ctx.set("show_crepus", show_crepus);
    let files = embedded_files();
    let html = render_from_files(&files, "views/popup.crepus", &ctx)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&json!({ "html": html, "css": POPUP_CSS }))
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Route a popup action.  Returns `{ storage_op? }`.
#[wasm_bindgen]
pub fn handle_popup_action(action: &str, data: JsValue) -> Result<JsValue, JsValue> {
    let data: Value = serde_wasm_bindgen::from_value(data)
        .unwrap_or_else(|_| json!({}));
    let checked = data.get("checked").and_then(|v| v.as_str()) == Some("true");
    let storage_op = match action {
        "set-enabled"     => Some(json!({ "type": "set", "key": "enabled",    "value": checked, "area": "sync" })),
        "set-auto-render" => Some(json!({ "type": "set", "key": "autoRender", "value": checked, "area": "sync" })),
        "show-help"       => Some(json!({ "type": "set", "key": "showHelp",   "value": true  })),
        "hide-help"       => Some(json!({ "type": "set", "key": "showHelp",   "value": false })),
        "show-crepus"     => Some(json!({ "type": "set", "key": "showCrepus", "value": true  })),
        "hide-crepus"     => Some(json!({ "type": "set", "key": "showCrepus", "value": false })),
        _ => None,
    };
    serde_wasm_bindgen::to_value(&match storage_op {
        Some(op) => json!({ "storage_op": op }),
        None => json!({}),
    })
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// App metadata for the settings popup.
#[wasm_bindgen]
pub fn app_manifest() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&json!({
        "manifest": {
            "name": env!("CARGO_PKG_NAME"),
            "description": env!("CARGO_PKG_DESCRIPTION")
        }
    }))
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Browser API interaction program as a JSON data structure (CSP-safe).
#[wasm_bindgen]
pub fn browser_program_data() -> String {
    BrowserProgram::new()
        .bind_storage("auto_render", StorageArea::Sync, "autoRender")
        .bind_runtime_message(
            "settings",
            MessagePayload::new().with_field("type", JsExpr::string("settings:get")),
        )
        .set_storage(StorageArea::Local, "anywhereBooted", JsExpr::bool(true))
        .console_log([
            JsExpr::string("anywhere booted"),
            JsExpr::var("settings"),
            JsExpr::var("auto_render"),
            JsExpr::Literal(json!({ "framework": "crepuscularity", "product": "anywhere" })),
        ])
        .emit_data()
}

/// Browser API interaction program as a self-contained ES module string.
#[wasm_bindgen]
pub fn browser_program() -> String {
    BrowserProgram::new()
        .bind_storage("auto_render", StorageArea::Sync, "autoRender")
        .bind_runtime_message(
            "settings",
            MessagePayload::new().with_field("type", JsExpr::string("settings:get")),
        )
        .set_storage(StorageArea::Local, "anywhereBooted", JsExpr::bool(true))
        .console_log([
            JsExpr::string("anywhere booted"),
            JsExpr::var("settings"),
            JsExpr::var("auto_render"),
            JsExpr::Literal(json!({ "framework": "crepuscularity", "product": "anywhere" })),
        ])
        .emit_module()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_anywhere_tags ──────────────────────────────────────────────────

    #[test]
    fn parses_simple_anywhere_tag() {
        let input = r#"<ai-anywhere id="w1" type="chart" title="My Chart">
<anywhere-ui lang="crepus">div foo</anywhere-ui>
</ai-anywhere>"#;
        let widgets = parse_anywhere_tags(input);
        assert_eq!(widgets.len(), 1);
        let w = &widgets[0];
        assert_eq!(w.id.as_deref(), Some("w1"));
        assert_eq!(w.widget_type.as_deref(), Some("chart"));
        let ui = w.ui.as_ref().unwrap();
        assert_eq!(ui.lang, UiLang::Crepus);
    }

    #[test]
    fn parses_multiple_widgets() {
        let input = r#"<ai-anywhere id="a"></ai-anywhere> <ai-anywhere id="b"></ai-anywhere>"#;
        let widgets = parse_anywhere_tags(input);
        assert_eq!(widgets.len(), 2);
    }

    #[test]
    fn ignores_unclosed_tag() {
        assert!(parse_anywhere_tags("<ai-anywhere id=\"x\">no close").is_empty());
    }

    // ── extract_widget_specs ─────────────────────────────────────────────────

    #[test]
    fn extracts_json_widget_spec() {
        let input = "```aiwidget\n{\"title\":\"Foo\",\"html\":\"<b>hi</b>\"}\n```";
        let specs = extract_widget_specs(input);
        assert_eq!(specs[0].title, "Foo");
        assert_eq!(specs[0].html, "<b>hi</b>");
    }

    #[test]
    fn no_specs_from_plain_text() {
        assert!(extract_widget_specs("just text").is_empty());
    }

    // ── handle_popup_action (app logic) ─────────────────────────────────────

    fn call_action(action: &str, data: Value) -> Value {
        let checked = data.get("checked").and_then(|v| v.as_str()) == Some("true");
        let op = match action {
            "set-enabled"     => Some(json!({ "type": "set", "key": "enabled",    "value": checked, "area": "sync" })),
            "set-auto-render" => Some(json!({ "type": "set", "key": "autoRender", "value": checked, "area": "sync" })),
            "show-help"       => Some(json!({ "type": "set", "key": "showHelp",   "value": true  })),
            "hide-help"       => Some(json!({ "type": "set", "key": "showHelp",   "value": false })),
            "show-crepus"     => Some(json!({ "type": "set", "key": "showCrepus", "value": true  })),
            "hide-crepus"     => Some(json!({ "type": "set", "key": "showCrepus", "value": false })),
            _ => None,
        };
        match op { Some(op) => json!({ "storage_op": op }), None => json!({}) }
    }

    #[test]
    fn show_help_sets_true()  { assert_eq!(call_action("show-help", json!({}))["storage_op"]["value"], true); }
    #[test]
    fn hide_help_sets_false() { assert_eq!(call_action("hide-help", json!({}))["storage_op"]["value"], false); }
    #[test]
    fn set_enabled_reflects_checked() {
        let r = call_action("set-enabled", json!({ "checked": "true" }));
        assert_eq!(r["storage_op"]["value"], true);
        assert_eq!(r["storage_op"]["area"], "sync");
    }
    #[test]
    fn unknown_action_returns_empty() {
        assert!(call_action("bogus", json!({})).get("storage_op").is_none());
    }
}
