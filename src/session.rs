//! Parser snapshots for editors: `parseSnapshot`, `reparse` and `ParserSession`.

use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn error(message: &str) -> JsValue {
    js_sys::Error::new(&format!("carve: {message}")).into()
}

fn type_error(message: &str) -> JsValue {
    js_sys::TypeError::new(&format!("carve: {message}")).into()
}

/// The tree as `parse` writes it, so a host can compare the two.
fn document_json(document: &carve::api::Document) -> Result<Value, JsValue> {
    let mut value = serde_json::to_value(document).map_err(|e| error(&e.to_string()))?;
    if let Some(map) = value.as_object_mut() {
        map.insert("type".into(), "document".into());
    }
    let tree = carve::api::decode_tree(&value.to_string()).map_err(|e| error(&e.to_string()))?;
    serde_json::from_str(&carve::api::encode_tree(&tree)).map_err(|e| error(&e.to_string()))
}

fn result_json(parse: &carve::IncrementalParse) -> Result<String, JsValue> {
    let changed: Vec<Value> = parse
        .changed_source
        .iter()
        .map(|range| json!([range.start, range.end]))
        .collect();
    let layout: Value = serde_json::from_str(&parse.source_layout_json).unwrap_or(Value::Null);
    Ok(json!({
        "source": parse.snapshot.source(),
        "document": document_json(&parse.document)?,
        "sourceLayout": layout,
        "changedSource": changed,
        "reusedPreviousTree": parse.reused_previous_tree,
        "parsedSourceBytes": parse.parsed_source_bytes,
    })
    .to_string())
}

/// A JSON array of `{ range: [start, end], replacement }` with UTF-8 byte offsets.
fn text_changes(input: &str) -> Result<Vec<carve::TextChange>, JsValue> {
    let value: Value = serde_json::from_str(input)
        .map_err(|e| type_error(&format!("`changes` is not JSON: {e}")))?;
    let array = value
        .as_array()
        .ok_or_else(|| type_error("`changes` must be a JSON array"))?;
    let mut changes = Vec::with_capacity(array.len());
    for (index, entry) in array.iter().enumerate() {
        let at = |what: &str| type_error(&format!("`changes[{index}]` {what}"));
        let range = entry
            .get("range")
            .and_then(Value::as_array)
            .filter(|r| r.len() == 2)
            .ok_or_else(|| at("needs a `range` of two byte offsets"))?;
        let offset = |slot: usize| {
            range[slot]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(|n| n as usize)
                .ok_or_else(|| {
                    at("range offsets must be non-negative integers no greater than 4294967295")
                })
        };
        let replacement = entry
            .get("replacement")
            .and_then(Value::as_str)
            .ok_or_else(|| at("needs a `replacement` string"))?;
        changes.push(carve::TextChange {
            range: offset(0)?..offset(1)?,
            replacement: replacement.to_string(),
        });
    }
    Ok(changes)
}

fn snapshot(source: &str) -> Result<carve::IncrementalParse, JsValue> {
    carve::parse_snapshot(source).map_err(|e| error(&e.to_string()))
}

/// Parse a document and keep what a later `reparse` needs. Returns JSON:
/// `{ source, document, sourceLayout, changedSource, reusedPreviousTree,
/// parsedSourceBytes }`.
#[wasm_bindgen(js_name = parseSnapshot)]
pub fn parse_snapshot(source: &str) -> Result<String, JsValue> {
    result_json(&snapshot(source)?)
}

/// Re-parse `source` with `changes` applied. Offsets are UTF-8 bytes; a
/// malformed change throws a `TypeError`.
#[wasm_bindgen(js_name = reparse)]
pub fn reparse(source: &str, changes: &str) -> Result<String, JsValue> {
    let changes = text_changes(changes)?;
    let next = carve::reparse(snapshot(source)?.snapshot, &changes)
        .map_err(|e| type_error(&e.to_string()))?;
    result_json(&next)
}

/// A parser snapshot retained between editor updates. Call `free()` when done.
#[wasm_bindgen]
pub struct ParserSession {
    parse: carve::IncrementalParse,
}

#[wasm_bindgen]
impl ParserSession {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<ParserSession, JsValue> {
        Ok(Self {
            parse: snapshot(source)?,
        })
    }

    /// The current result, with byte ranges and parser-work counters.
    pub fn snapshot(&self) -> Result<String, JsValue> {
        result_json(&self.parse)
    }

    /// Apply byte edits; a rejected edit leaves the session unchanged.
    pub fn edit(&mut self, changes: &str) -> Result<String, JsValue> {
        let changes = text_changes(changes)?;
        let next = carve::reparse(self.parse.snapshot.clone(), &changes)
            .map_err(|e| type_error(&e.to_string()))?;
        self.parse = next;
        self.snapshot()
    }
}
