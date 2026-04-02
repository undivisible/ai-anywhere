// as-hello: minimal anywhere-compatible AssemblyScript runtime.
//
// Build: asc assembly/index.ts --outFile build/runtime_bg.wasm --exportRuntime
//
// The --exportRuntime flag exports __getString / __newString so the
// framework's runtime-as-adapter.js can transfer strings across the boundary.
//
// JSON is assembled as raw strings to avoid a library dependency. For a
// production app use `json-as` (npm i json-as) and its @json decorator.

// ---------------------------------------------------------------------------
// Tiny JSON helpers (no library dependency)
// ---------------------------------------------------------------------------
function escapeJson(s: string): string {
  let out = "";
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c === 34) { out += '\\"'; }
    else if (c === 92) { out += "\\\\"; }
    else if (c === 10) { out += "\\n"; }
    else if (c === 13) { out += "\\r"; }
    else if (c === 9)  { out += "\\t"; }
    else { out += s.charAt(i); }
  }
  return out;
}

function jsonStr(s: string): string {
  return '"' + escapeJson(s) + '"';
}

function jsonObj(pairs: string[][]): string {
  const fields = pairs.map<string>((p) => jsonStr(p[0]) + ":" + p[1]);
  return "{" + fields.join(",") + "}";
}

// ---------------------------------------------------------------------------
// App metadata
// ---------------------------------------------------------------------------
export function runtime_version(): string {
  return "0.1.0-as";
}

export function app_manifest(): string {
  return jsonObj([
    ["id",  jsonStr("as-hello")],
    ["manifest", jsonObj([
      ["name",        jsonStr("as-hello")],
      ["version",     jsonStr("0.1.0")],
      ["description", jsonStr("AssemblyScript anywhere runtime example")],
    ])],
  ]);
}

// ---------------------------------------------------------------------------
// Browser program: runs once at content-script boot.
// The framework eval's the returned ES module string and calls runBrowserProgram.
// ---------------------------------------------------------------------------
export function browser_program(): string {
  return `
export async function runBrowserProgram(api) {
  console.log("[as-hello] booted", { runtime: "assemblyscript", framework: "anywhere" });
  await api.storage.local.set({ asHelloBooted: true });
}
`;
}

// ---------------------------------------------------------------------------
// WASM-driven popup: called by popup.js with the full storage.local state.
// Returns { html, css } — popup.js injects css into <head> and sets innerHTML.
// ---------------------------------------------------------------------------
export function render_popup(_stateJson: string): string {
  const html =
    '<div class="ah-popup">' +
      '<div class="ah-header">' +
        '<span class="ah-brand">as-hello</span>' +
        '<span class="ah-sub">AssemblyScript + anywhere</span>' +
      '</div>' +
      '<div class="ah-body">' +
        '<p>Hello from AssemblyScript!</p>' +
        '<p>This popup is rendered entirely from WASM — no hand-written JS, HTML, or CSS files in the app.</p>' +
        '<p class="ah-note">Runtime: <code>assemblyscript</code></p>' +
      '</div>' +
    '</div>';

  const css =
    "body{margin:0;min-width:260px;font-family:system-ui,sans-serif;background:#0f172a;color:#e2e8f0}" +
    ".ah-popup{display:flex;flex-direction:column}" +
    ".ah-header{padding:12px 16px;background:#1e293b;border-bottom:1px solid #334155;display:flex;flex-direction:column;gap:3px}" +
    ".ah-brand{font-size:13px;font-weight:700;text-transform:uppercase;letter-spacing:.08em;color:#7c3aed}" +
    ".ah-sub{font-size:11px;color:#64748b}" +
    ".ah-body{padding:14px 16px;font-size:13px;line-height:1.55;color:#94a3b8}" +
    ".ah-body p{margin:0 0 8px}" +
    ".ah-note{margin-top:10px!important;padding:8px 10px;background:#1e293b;border-radius:6px;color:#7c3aed;font-size:12px}" +
    "code{font-family:monospace;font-size:11px}";

  return jsonObj([
    ["html", jsonStr(html)],
    ["css",  jsonStr(css)],
  ]);
}
