//! Browser and Node binding for the Carve 0.2 JSON call boundary.

use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

#[cfg(feature = "source-patches")]
mod source_patch;

fn input_options(options: Option<JsValue>) -> Result<Value, JsValue> {
    match options {
        None => Ok(json!({})),
        Some(value) if value.is_null() || value.is_undefined() => Ok(json!({})),
        Some(value) => {
            let parsed: Value = serde_wasm_bindgen::from_value(value)
                .map_err(|e| js_sys::TypeError::new(&format!("invalid options: {e}")))?;
            if !parsed.is_object() {
                return Err(js_sys::TypeError::new("options must be an object").into());
            }
            Ok(parsed)
        }
    }
}

fn error_from_response(error: &Value) -> JsValue {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Carve call failed");
    let exception = js_sys::Error::new(message);
    for name in ["name", "category", "status"] {
        if let Some(value) = error.get(name) {
            let key = JsValue::from_str(name);
            let field = serde_wasm_bindgen::to_value(value).unwrap_or(JsValue::UNDEFINED);
            let _ = js_sys::Reflect::set(&exception, &key, &field);
        }
    }
    exception.into()
}

#[cfg(any(
    feature = "other-renderers",
    feature = "html-import",
    feature = "reports"
))]
fn unavailable(error: &JsValue) -> bool {
    js_sys::Reflect::get(error, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .is_some_and(|message| message.contains("not available in this engine yet"))
}

fn run(request: Value) -> Result<Value, JsValue> {
    let response = carve::api::json::call(&request.to_string());
    let result: Value = serde_json::from_str(&response)
        .map_err(|e| js_sys::Error::new(&format!("invalid engine response: {e}")))?;
    if let Some(error) = result.get("error") {
        return Err(error_from_response(error));
    }
    result
        .get("ok")
        .cloned()
        .ok_or_else(|| js_sys::Error::new("engine response has no result").into())
}

fn source_call(name: &str, source: &str, options: Option<JsValue>) -> Result<Value, JsValue> {
    run(json!({"call": name, "source": source, "options": input_options(options)?}))
}

#[cfg(any(feature = "ast-json", feature = "includes", feature = "lint"))]
fn field(value: Value, name: &str) -> Result<JsValue, JsValue> {
    value_to_js(value.get(name).unwrap_or(&Value::Null))
}

#[cfg(any(
    feature = "ast-json",
    feature = "reports",
    feature = "html-import",
    feature = "includes",
    feature = "lint"
))]
fn value_to_js(value: &Value) -> Result<JsValue, JsValue> {
    js_sys::JSON::parse(&value.to_string())
}

fn text_field(value: Value, name: &str) -> Result<String, JsValue> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| js_sys::Error::new(&format!("engine result has no {name}")).into())
}

/// Pass a 0.2 request to the Rust engine and return its JSON response unchanged.
/// This supports every engine call, including future calls at a newer pin.
#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = call)]
pub fn call(request: &str) -> String {
    carve::api::json::call(request)
}

/// Parse source to a 0.2 tree.
#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = parse)]
pub fn parse(source: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    field(source_call("parse", source, options)?, "tree")
}

#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = parseWithDiagnostics)]
pub fn parse_with_diagnostics(source: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    value_to_js(&source_call("parseWithDiagnostics", source, options)?)
}

#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = parseWithLayout)]
pub fn parse_with_layout(source: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    value_to_js(&source_call("parseWithLayout", source, options)?)
}

/// Render source to HTML. The engine's 0.2 default refuses raw HTML.
#[wasm_bindgen(js_name = render)]
pub fn render(source: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    text_field(source_call("render", source, options)?, "html")
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = renderWithDiagnostics)]
pub fn render_with_diagnostics(source: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    value_to_js(&source_call("renderWithDiagnostics", source, options)?)
}

#[cfg(feature = "lint")]
#[wasm_bindgen(js_name = lint)]
pub fn lint(source: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    field(source_call("lint", source, options)?, "diagnostics")
}

#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = renderTree)]
pub fn render_tree(tree: JsValue, options: Option<JsValue>) -> Result<String, JsValue> {
    let tree: Value = serde_wasm_bindgen::from_value(tree)
        .map_err(|e| js_sys::TypeError::new(&format!("invalid tree: {e}")))?;
    text_field(
        run(json!({"call": "render", "tree": tree, "options": input_options(options)?}))?,
        "html",
    )
}

#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = encodeTree)]
pub fn encode_tree(tree: JsValue) -> Result<String, JsValue> {
    let tree: Value = serde_wasm_bindgen::from_value(tree)
        .map_err(|e| js_sys::TypeError::new(&format!("invalid tree: {e}")))?;
    text_field(run(json!({"call": "encodeTree", "tree": tree}))?, "json")
}

#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = decodeTree)]
pub fn decode_tree(tree_json: &str) -> Result<JsValue, JsValue> {
    field(
        run(json!({"call": "decodeTree", "json": tree_json}))?,
        "tree",
    )
}

#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = migrateTree)]
pub fn migrate_tree(tree: JsValue) -> Result<JsValue, JsValue> {
    let tree: Value = serde_wasm_bindgen::from_value(tree)
        .map_err(|e| js_sys::TypeError::new(&format!("invalid tree: {e}")))?;
    value_to_js(&run(json!({"call": "migrateTree", "tree": tree}))?)
}

#[cfg(feature = "includes")]
#[wasm_bindgen(js_name = includeDependencies)]
pub fn include_dependencies(tree: JsValue, file: Option<String>) -> Result<JsValue, JsValue> {
    let tree: Value = serde_wasm_bindgen::from_value(tree)
        .map_err(|e| js_sys::TypeError::new(&format!("invalid tree: {e}")))?;
    field(
        run(json!({"call": "includeDependencies", "tree": tree, "file": file}))?,
        "dependencies",
    )
}

/// Route every output target through the 0.2 writer registry.
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = write)]
pub fn write(tree: JsValue, target: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    let tree: Value = serde_wasm_bindgen::from_value(tree)
        .map_err(|e| js_sys::TypeError::new(&format!("invalid tree: {e}")))?;
    let response = run(
        json!({"call": "write", "tree": tree, "target": target, "options": input_options(options)?}),
    );
    match response {
        Ok(value) => text_field(value, "output"),
        Err(error)
            if unavailable(&error) && ["carve", "markdown", "plain", "ansi"].contains(&target) =>
        {
            Err(js_sys::Error::new(&format!("{target} writer not available yet")).into())
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = writeWithDiagnostics)]
pub fn write_with_diagnostics(
    tree: JsValue,
    target: &str,
    options: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let tree: Value = serde_wasm_bindgen::from_value(tree)
        .map_err(|e| js_sys::TypeError::new(&format!("invalid tree: {e}")))?;
    match run(
        json!({"call": "writeWithDiagnostics", "tree": tree, "target": target, "options": input_options(options)?}),
    ) {
        Ok(value) => value_to_js(&value),
        Err(error) if unavailable(&error) => {
            Err(js_sys::Error::new(&format!("{target} writer not available yet")).into())
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = format)]
pub fn format(source: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    match source_call("format", source, options) {
        Ok(value) => text_field(value, "source"),
        Err(error) if unavailable(&error) => {
            Err(js_sys::Error::new("carve writer not available yet").into())
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = formatWithDiagnostics)]
pub fn format_with_diagnostics(source: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    match source_call("formatWithDiagnostics", source, options) {
        Ok(value) => value_to_js(&value),
        Err(error) if unavailable(&error) => {
            Err(js_sys::Error::new("carve writer not available yet").into())
        }
        Err(error) => Err(error),
    }
}

#[wasm_bindgen(js_name = toHtml)]
pub fn to_html(source: &str) -> Result<String, JsValue> {
    render(source, None)
}

#[wasm_bindgen(js_name = toHtmlWithOptions)]
pub fn to_html_with_options(source: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    render(source, options)
}

#[cfg(feature = "other-renderers")]
fn target_source(source: &str, target: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    let tree = source_call("parse", source, None)?;
    let response = run(
        json!({"call": "write", "tree": tree["tree"], "target": target, "options": input_options(options)?}),
    );
    match response {
        Ok(value) => text_field(value, "output"),
        Err(error) if unavailable(&error) => {
            Err(js_sys::Error::new(&format!("{target} writer not available yet")).into())
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toMarkdown)]
pub fn to_markdown(source: &str) -> Result<String, JsValue> {
    target_source(source, "markdown", None)
}
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toMarkdownWithOptions)]
pub fn to_markdown_with_options(source: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    target_source(source, "markdown", options)
}
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toPlainText)]
pub fn to_plain_text(source: &str) -> Result<String, JsValue> {
    target_source(source, "plain", None)
}
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toPlainTextWithOptions)]
pub fn to_plain_text_with_options(
    source: &str,
    options: Option<JsValue>,
) -> Result<String, JsValue> {
    target_source(source, "plain", options)
}
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toAnsi)]
pub fn to_ansi(source: &str) -> Result<String, JsValue> {
    target_source(source, "ansi", None)
}
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toAnsiWithOptions)]
pub fn to_ansi_with_options(source: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    target_source(source, "ansi", options)
}
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toCarve)]
pub fn to_carve(source: &str) -> Result<String, JsValue> {
    format(source, None)
}
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toCarveWithOptions)]
pub fn to_carve_with_options(source: &str, options: Option<JsValue>) -> Result<String, JsValue> {
    format(source, options)
}

#[cfg(all(feature = "source-patches", feature = "other-renderers"))]
#[wasm_bindgen(js_name = toCarvePatch)]
pub fn to_carve_patch(source: &str) -> Result<JsValue, JsValue> {
    let formatted = format(source, None)?;
    serde_wasm_bindgen::to_value(&source_patch::create(
        source,
        &formatted,
        source_patch::SourceEditKind::Formatting,
        "canonical-format",
    ))
    .map_err(|e| js_sys::Error::new(&e.to_string()).into())
}

/// The standard extensions the `extensions` option enables by name (C2-X-039).
/// Host extensions take host code, which the JSON boundary has no form for.
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = extensions)]
pub fn extensions() -> Result<JsValue, JsValue> {
    value_to_js(&json!([
        "carve.cite",
        "carve.imgFence.image",
        "carve.imgFence.img",
        "carve.tabs",
        "carve.wiki"
    ]))
}

#[cfg(feature = "html-import")]
#[wasm_bindgen(js_name = importDocument)]
pub fn import_document(source: &str, format: &str) -> Result<JsValue, JsValue> {
    let result = run(json!({"call": "importDocument", "source": source, "from": format}));
    match result {
        Ok(value) => value_to_js(&value),
        Err(error) if unavailable(&error) => Err(js_sys::Error::new(&format!(
            "importDocument not available yet for {format}"
        ))
        .into()),
        Err(error) => Err(error),
    }
}

#[cfg(feature = "source-patches")]
#[wasm_bindgen(js_name = createSourcePatch)]
pub fn create_source_patch(
    source: &str,
    replacement: &str,
    kind: &str,
    code: &str,
) -> Result<JsValue, JsValue> {
    let kind = match kind {
        "formatting" => source_patch::SourceEditKind::Formatting,
        "syntax-migration" => source_patch::SourceEditKind::SyntaxMigration,
        "quick-fix" => source_patch::SourceEditKind::QuickFix,
        "refactor" => source_patch::SourceEditKind::Refactor,
        _ => return Err(js_sys::TypeError::new("unknown source patch edit kind").into()),
    };
    if code.is_empty() {
        return Err(js_sys::TypeError::new("source patch code must not be empty").into());
    }
    serde_wasm_bindgen::to_value(&source_patch::create(source, replacement, kind, code))
        .map_err(|e| js_sys::Error::new(&e.to_string()).into())
}

#[cfg(feature = "source-patches")]
#[wasm_bindgen(js_name = applySourcePatch)]
pub fn apply_source_patch(source: &str, patch: JsValue) -> Result<String, JsValue> {
    let patch: source_patch::SourcePatch = serde_wasm_bindgen::from_value(patch)
        .map_err(|e| js_sys::TypeError::new(&format!("invalid source patch: {e}")))?;
    source_patch::apply(source, &patch).map_err(|e| js_sys::Error::new(&e.to_string()).into())
}

#[wasm_bindgen(js_name = version)]
pub fn version() -> String {
    "0.2 development".to_string()
}

#[cfg(all(test, feature = "ast-json"))]
mod tests {
    use super::*;
    #[test]
    fn json_boundary_renders_and_parses() {
        let html: Value =
            serde_json::from_str(&call(r#"{"call":"render","source":"a *b*"}"#)).unwrap();
        assert_eq!(html["ok"]["html"], "<p>a <strong>b</strong></p>");
        let tree: Value =
            serde_json::from_str(&call(r##"{"call":"parse","source":"# H"}"##)).unwrap();
        assert_eq!(tree["ok"]["tree"]["type"], "document");
    }
}
