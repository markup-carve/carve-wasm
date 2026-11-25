//! The ProseMirror / Tiptap bridge of the engine (`carve::api::prosemirror`):
//! a 0.2 tree to a ProseMirror document in the schema of the carve-grammars
//! Tiptap kit, and back.

use carve::api::prosemirror::{self as bridge, ProseMirrorError, ProseMirrorOptions};
use carve::api::{self, Options, WriteOptions};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn error(e: &ProseMirrorError) -> JsValue {
    let exception = js_sys::Error::new(&e.to_string());
    match e {
        ProseMirrorError::Carve(carve) => {
            exception.set_name(carve.spec_name());
            // The fields the JSON calls' errors carry (C2-T-040).
            let _ = js_sys::Reflect::set(&exception, &"category".into(), &carve.category().into());
            let _ = js_sys::Reflect::set(&exception, &"status".into(), &carve.status().into());
        }
        _ => exception.set_name("ProseMirrorError"),
    }
    exception.into()
}

fn read_options(value: Option<JsValue>) -> Result<ProseMirrorOptions, JsValue> {
    let value: Value = match value {
        Some(v) if !v.is_null() && !v.is_undefined() => serde_wasm_bindgen::from_value(v)
            .map_err(|e| js_sys::TypeError::new(&format!("invalid options: {e}")))?,
        _ => Value::Null,
    };
    ProseMirrorOptions::from_json(&value).map_err(|e| error(&e))
}

/// The ProseMirror document as JSON text: a string as it is, an object
/// through `JSON.stringify`.
fn doc_json(doc: &JsValue) -> Result<String, JsValue> {
    if let Some(text) = doc.as_string() {
        return Ok(text);
    }
    js_sys::JSON::stringify(doc)
        .map(String::from)
        .map_err(|_| js_sys::TypeError::new("invalid ProseMirror document").into())
}

/// A tree from Carve source (a string) or a 0.2 tree object.
fn tree(input: &JsValue) -> Result<api::Tree, JsValue> {
    if let Some(source) = input.as_string() {
        return api::parse(source.as_str(), &Options::default())
            .map_err(|e| error(&ProseMirrorError::Carve(e)));
    }
    let json = js_sys::JSON::stringify(input)
        .map(String::from)
        .map_err(|_| js_sys::TypeError::new("invalid tree"))?;
    api::decode_tree(&json).map_err(|e| error(&ProseMirrorError::Carve(e)))
}

fn tree_to_js(tree: &api::Tree) -> Result<JsValue, JsValue> {
    js_sys::JSON::parse(&api::encode_tree(tree))
}

/// The ProseMirror document of Carve source or a 0.2 tree:
/// `{ doc, json, blanked }`. `doc` goes to the editor's `setContent()`;
/// `blanked` lists the denied link and image destinations kept out of the
/// editor's attributes. `options`: `urlPolicy` and `mapping`.
#[wasm_bindgen(js_name = toProseMirror, unchecked_return_type = "ProseMirrorResult")]
pub fn to_prose_mirror(
    #[wasm_bindgen(unchecked_param_type = "string | object")] input: JsValue,
    options: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let options = read_options(options)?;
    let rendered = bridge::to_prosemirror_with(&tree(&input)?, &options).map_err(|e| error(&e))?;
    js_sys::JSON::parse(
        &json!({"doc": rendered.doc, "json": rendered.json, "blanked": rendered.blanked})
            .to_string(),
    )
}

/// The 0.2 tree of a ProseMirror document (the editor's `getJSON()`, as an
/// object or JSON text). A node or mark with no Carve node throws.
#[wasm_bindgen(js_name = fromProseMirror)]
pub fn from_prose_mirror(
    #[wasm_bindgen(unchecked_param_type = "string | object")] doc: JsValue,
    options: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let options = read_options(options)?;
    let saved =
        bridge::from_prosemirror_with_report(&doc_json(&doc)?, &options).map_err(|e| error(&e))?;
    tree_to_js(&saved.tree)
}

/// `fromProseMirror` with the report: `{ tree, origins, pointers, ignored }`.
/// `origins` gives, per top-level block, the index of the loaded block it
/// came from (`null` for one the editor added); `pointers` maps a tree path
/// to the JSON pointer of its editor node; `ignored` lists the attributes the
/// save did not read.
#[wasm_bindgen(
    js_name = fromProseMirrorWithReport,
    unchecked_return_type = "ProseMirrorImport"
)]
pub fn from_prose_mirror_with_report(
    #[wasm_bindgen(unchecked_param_type = "string | object")] doc: JsValue,
    options: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let options = read_options(options)?;
    let saved =
        bridge::from_prosemirror_with_report(&doc_json(&doc)?, &options).map_err(|e| error(&e))?;
    let ignored: Vec<Value> = saved
        .ignored
        .iter()
        .map(|i| json!({"path": i.path, "type": i.type_, "attribute": i.attribute, "value": i.value}))
        .collect();
    let tree: Value = serde_json::from_str(&api::encode_tree(&saved.tree))
        .map_err(|e| js_sys::Error::new(&e.to_string()))?;
    js_sys::JSON::parse(
        &json!({"tree": tree, "origins": saved.origins, "pointers": saved.pointers, "ignored": ignored})
            .to_string(),
    )
}

/// The canonical Carve source of a ProseMirror document.
#[wasm_bindgen(js_name = proseMirrorToCarve)]
pub fn prose_mirror_to_carve(
    #[wasm_bindgen(unchecked_param_type = "string | object")] doc: JsValue,
    options: Option<JsValue>,
) -> Result<String, JsValue> {
    let options = read_options(options)?;
    bridge::prosemirror_to_carve_with(&doc_json(&doc)?, &options, &WriteOptions::default())
        .map_err(|e| error(&e))
}

#[wasm_bindgen(typescript_custom_section)]
const TS_PROSEMIRROR: &str = r#"
export interface ProseMirrorMappingEntry {
  as: "span" | "div";
  class: string;
  attributes?: string[];
  content?: boolean;
  carve?: boolean;
}
export interface ProseMirrorOptions {
  urlPolicy?: { deny: string[] } | { allow: string[] };
  mapping?: {
    nodes?: Record<string, ProseMirrorMappingEntry>;
    marks?: Record<string, ProseMirrorMappingEntry>;
  };
}
export interface ProseMirrorResult {
  /** The ProseMirror `doc` node. */
  doc: object;
  /** The same, as JSON text. */
  json: string;
  /** The denied destinations kept out of the editor's attributes. */
  blanked: string[];
}
export interface ProseMirrorImport {
  tree: object;
  origins: (number | null)[];
  pointers: Record<string, string>;
  ignored: { path: string; type: string; attribute: string; value: unknown }[];
}
"#;
