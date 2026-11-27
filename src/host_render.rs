//! `toHtmlWithRenderers`: JS renderers and transforms on the engine host.
//! The JSON boundary has no form for host code (C2-T-010), so this call builds
//! the host itself, as the native Python and Ruby bindings do.

use carve::api::{
    self, Channel, DocumentTransform, Extension, ExtensionDeclaration, HeadingIds, Mode, Profile,
    RenderContext, Renderer, SoftBreak, Tree, UrlPolicy,
};
use js_sys::{Function, Object, Reflect};
use serde_json::Value;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

/// A JS function the engine calls during the render.
struct HostFunction(Function);

// SAFETY: wasm32-unknown-unknown without atomics runs on one thread, so the
// function is never touched from another. The engine requires the bounds.
unsafe impl Send for HostFunction {}
unsafe impl Sync for HostFunction {}

fn host_error(why: impl ToString) -> api::Error {
    api::Error::Option {
        option: "host".into(),
        message: why.to_string(),
    }
}

fn js_message(error: &JsValue) -> String {
    Reflect::get(error, &"message".into())
        .ok()
        .and_then(|m| m.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "the callback threw".into())
}

fn to_js(value: &Value) -> Result<JsValue, JsValue> {
    js_sys::JSON::parse(&value.to_string())
}

fn from_js(value: &JsValue) -> Result<Value, JsValue> {
    let text = js_sys::JSON::stringify(value)?
        .as_string()
        .ok_or_else(|| JsValue::from(js_sys::TypeError::new("value has no JSON form")))?;
    serde_json::from_str(&text).map_err(|e| js_sys::TypeError::new(&e.to_string()).into())
}

impl Renderer for HostFunction {
    fn render(&self, node: &Value, _cx: &RenderContext<'_>) -> Result<String, api::Error> {
        let arg = to_js(node).map_err(|e| host_error(js_message(&e)))?;
        let result = self
            .0
            .call1(&JsValue::NULL, &arg)
            .map_err(|e| host_error(js_message(&e)))?;
        result
            .as_string()
            .ok_or_else(|| host_error("a renderer must return a string"))
    }
}

impl DocumentTransform for HostFunction {
    fn transform(&self, view: Tree) -> Result<Tree, api::Error> {
        let value: Value = serde_json::from_str(&api::encode_tree(&view)).map_err(host_error)?;
        let arg = to_js(&value).map_err(|e| host_error(js_message(&e)))?;
        let result = self
            .0
            .call1(&JsValue::NULL, &arg)
            .map_err(|e| host_error(js_message(&e)))?;
        let next = from_js(&result).map_err(|e| host_error(js_message(&e)))?;
        api::decode_tree(&next.to_string())
    }
}

fn option_error(message: impl AsRef<str>) -> JsValue {
    let error = js_sys::TypeError::new(&format!("invalid options: {}", message.as_ref()));
    let _ = Reflect::set(&error, &"name".into(), &"CarveOptionError".into());
    let _ = Reflect::set(&error, &"category".into(), &"usage".into());
    error.into()
}

fn text<'a>(key: &str, value: &'a Value) -> Result<&'a str, JsValue> {
    value
        .as_str()
        .ok_or_else(|| option_error(format!("{key} must be text")))
}

fn flag(key: &str, value: &Value) -> Result<bool, JsValue> {
    value
        .as_bool()
        .ok_or_else(|| option_error(format!("{key} must be a boolean")))
}

fn strings(key: &str, value: &Value) -> Result<Vec<String>, JsValue> {
    value
        .as_array()
        .ok_or_else(|| option_error(format!("{key} must be a list")))?
        .iter()
        .map(|item| text(key, item).map(str::to_string))
        .collect()
}

fn string_map(key: &str, value: &Value) -> Result<BTreeMap<String, String>, JsValue> {
    value
        .as_object()
        .ok_or_else(|| option_error(format!("{key} must be an object")))?
        .iter()
        .map(|(name, item)| Ok((name.clone(), text(key, item)?.to_string())))
        .collect()
}

fn count(key: &str, value: &Value, max: u64) -> Result<u64, JsValue> {
    value
        .as_u64()
        .filter(|n| *n <= max)
        .ok_or_else(|| option_error(format!("{key} must be a count from 0 to {max}")))
}

/// The render options this call reads, from the JSON part of `options`.
fn read_options(map: &serde_json::Map<String, Value>) -> Result<api::Options, JsValue> {
    let mut out = api::Options::default();
    for (key, item) in map {
        match key.as_str() {
            "mode" => {
                out.mode = match text(key, item)? {
                    "interactive" => Mode::Interactive,
                    "static" => Mode::Static,
                    _ => return Err(option_error("mode must be interactive or static")),
                }
            }
            "headingIds" => {
                out.heading_ids = HeadingIds::from_name(text(key, item)?)
                    .ok_or_else(|| option_error("invalid headingIds"))?
            }
            "profile" => {
                out.profile = Profile::from_name(text(key, item)?)
                    .ok_or_else(|| option_error("invalid profile"))?
            }
            "softBreak" => {
                out.soft_break = SoftBreak::from_name(text(key, item)?)
                    .ok_or_else(|| option_error("invalid softBreak"))?
            }
            "sections" => out.sections = flag(key, item)?,
            "rawHtml" => out.raw_html = flag(key, item)?,
            "sourceLines" => out.source_lines = flag(key, item)?,
            "smartTypography" => out.smart_typography = flag(key, item)?,
            "roundtrip" => out.roundtrip = flag(key, item)?,
            "strictLosses" => out.strict_losses = flag(key, item)?,
            "symbols" => out.symbols = string_map(key, item)?,
            "labels" => out.labels.extend(string_map(key, item)?),
            "allowLosses" => out.allow_losses = strings(key, item)?,
            "headingShift" => out.heading_shift = count(key, item, 5)? as u8,
            "maxLosses" => out.max_losses = count(key, item, u64::from(u32::MAX))? as usize,
            "urlPolicy" => {
                let policy = item.as_object().filter(|p| p.len() == 1).ok_or_else(|| {
                    option_error("urlPolicy must have exactly one of allow or deny")
                })?;
                out.url_policy = if let Some(deny) = policy.get("deny") {
                    UrlPolicy::Deny(strings(key, deny)?)
                } else if let Some(allow) = policy.get("allow") {
                    UrlPolicy::Allow(strings(key, allow)?)
                } else {
                    return Err(option_error("urlPolicy must have allow or deny"));
                };
            }
            "social" => {
                let social = string_map(key, item)?;
                if let Some(name) = social
                    .keys()
                    .find(|name| !matches!(name.as_str(), "mentionUrl" | "tagUrl"))
                {
                    return Err(option_error(format!("unknown social field {name}")));
                }
                out.social = Some(carve::options::Social {
                    mention_url: social.get("mentionUrl").cloned(),
                    tag_url: social.get("tagUrl").cloned(),
                });
            }
            "extensions" => {}
            "includes" => {
                return Err(option_error(
                    "includes takes a resolver function, or an object with resolver",
                ))
            }
            _ => {
                return Err(option_error(format!(
                    "toHtmlWithRenderers does not read option {key}"
                )))
            }
        }
    }
    Ok(out)
}

fn function(value: &JsValue) -> Option<Function> {
    value.dyn_ref::<Function>().cloned()
}

fn take(options: &Object, key: &str) -> Result<JsValue, JsValue> {
    let value = Reflect::get(options, &key.into())?;
    Reflect::delete_property(options, &key.into())?;
    Ok(value)
}

/// A standard renderer selection: `"standard"` or `{ standard: { mode } }`.
fn standard_selection(value: &JsValue) -> Result<Option<bool>, JsValue> {
    if value.as_string().as_deref() == Some("standard") {
        return Ok(Some(false));
    }
    if value.is_object() && function(value).is_none() {
        let standard = Reflect::get(value, &"standard".into())?;
        if !standard.is_undefined() {
            let mode = if standard.is_object() {
                Reflect::get(&standard, &"mode".into())?.as_string()
            } else {
                None
            };
            return Ok(Some(mode.as_deref() == Some("aria")));
        }
    }
    Ok(None)
}

fn host_renderers(name: &str, value: &JsValue) -> Result<Extension, JsValue> {
    let mut version = "1".to_string();
    let mut renderers: Vec<(Channel, Box<dyn Renderer>)> = Vec::new();
    if let Some(callback) = function(value) {
        renderers.push((
            Channel::Interactive,
            Box::new(HostFunction(callback.clone())),
        ));
        renderers.push((Channel::BuildTime, Box::new(HostFunction(callback))));
    } else if value.is_object() {
        for key in Object::keys(value.unchecked_ref::<Object>()).iter() {
            let key = key.as_string().unwrap_or_default();
            let item = Reflect::get(value, &key.as_str().into())?;
            let channel = match key.as_str() {
                "version" => {
                    version = item.as_string().ok_or_else(|| {
                        option_error(format!("renderers: {name}: version must be text"))
                    })?;
                    continue;
                }
                "interactive" => Channel::Interactive,
                "static" => Channel::BuildTime,
                _ => {
                    return Err(option_error(format!(
                        "renderers: {name}: a channel is interactive or static"
                    )))
                }
            };
            let callback = function(&item).ok_or_else(|| {
                option_error(format!("renderers: {name}: {key} must be a function"))
            })?;
            renderers.push((channel, Box::new(HostFunction(callback))));
        }
    } else {
        return Err(option_error(format!(
            "renderers: {name} must be \"standard\", a function, or {{ interactive, static }}"
        )));
    }
    Ok(Extension {
        declaration: ExtensionDeclaration {
            name: name.to_string(),
            version,
            required: false,
            slots: Vec::new(),
        },
        renderers,
        transform: None,
    })
}

fn engine_error(error: api::Error) -> JsValue {
    let exception = js_sys::Error::new(&error.to_string());
    let _ = Reflect::set(&exception, &"name".into(), &error.spec_name().into());
    let _ = Reflect::set(&exception, &"category".into(), &error.category().into());
    let _ = Reflect::set(&exception, &"status".into(), &error.status().into());
    exception.into()
}

#[wasm_bindgen(typescript_custom_section)]
const TS_HOST_RENDER: &str = r#"
/** A host renderer: one function for both channels, or one per channel. */
export type HostRenderer =
  | ((node: any) => string)
  | { interactive?: (node: any) => string; static?: (node: any) => string; version?: string };
export interface HostRenderOptions {
  [option: string]: unknown;
  renderers?: Record<string, "standard" | { standard: { mode?: string } } | HostRenderer>;
  transforms?: Array<(tree: any) => any>;
}
export interface HostRenderResult {
  html: string;
  diagnostics: any[];
  suppressedDiagnostics: number;
}
export function toHtmlWithRenderers(source: string, options?: HostRenderOptions | null): HostRenderResult;
"#;

/// Render with host renderers and transforms. Returns `{ html, diagnostics,
/// suppressedDiagnostics }`.
#[wasm_bindgen(js_name = toHtmlWithRenderers, skip_typescript)]
pub fn to_html_with_renderers(source: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    let options = match options {
        Some(value) if value.is_object() => value,
        Some(value) if !(value.is_null() || value.is_undefined()) => {
            return Err(option_error("options must be an object"))
        }
        _ => Object::new().into(),
    };
    let (options, includes) = crate::host_includes::split(options)?;
    let options = Object::assign(&Object::new(), options.unchecked_ref::<Object>());
    let renderers = take(&options, "renderers")?;
    let transforms = take(&options, "transforms")?;
    let json: Value =
        serde_wasm_bindgen::from_value(options.into()).map_err(|e| option_error(e.to_string()))?;
    let map = json
        .as_object()
        .ok_or_else(|| option_error("options must be an object"))?;
    let mut opts = read_options(map)?;
    opts.includes = includes;

    let mut selected: BTreeMap<String, bool> = BTreeMap::new();
    let mut standard = serde_json::Map::new();
    let mut host_extensions = Vec::new();
    if renderers.is_object() {
        for name in Object::keys(renderers.unchecked_ref::<Object>()).iter() {
            let name = name.as_string().unwrap_or_default();
            let value = Reflect::get(&renderers, &name.as_str().into())?;
            if let Some(aria) = standard_selection(&value)? {
                standard.insert(
                    name.clone(),
                    from_js(&value).map_err(|e| option_error(js_message(&e)))?,
                );
                selected.insert(name, aria);
            } else if crate::STANDARD_EXTENSIONS.contains(&name.as_str()) {
                return Err(option_error(format!(
                    "renderers: {name}: a standard extension takes \"standard\"; host renderers render host extensions (C2-T-010)"
                )));
            } else {
                host_extensions.push(host_renderers(&name, &value)?);
            }
        }
    } else if !renderers.is_undefined() && !renderers.is_null() {
        return Err(option_error("renderers must be an object"));
    }

    // The engine's own option reader checks the extension names and the
    // standard selections, so this call refuses what `render` refuses.
    let check = serde_json::json!({
        "call": "render",
        "source": "",
        "options": {"extensions": map.get("extensions").cloned().unwrap_or(Value::Array(Vec::new())), "renderers": standard},
    });
    crate::run(check)?;

    let mut host = api::Carve::new();
    if let Some(names) = map.get("extensions") {
        for name in strings("extensions", names)? {
            match selected.get(&name) {
                Some(true) if name == "carve.codeGroup" => host
                    .register(api::extensions::code_group(
                        api::extensions::CodeGroupOptions {
                            mode: api::extensions::CodeGroupMode::Aria,
                            ..Default::default()
                        },
                    ))
                    .map_err(|e| option_error(e.to_string()))?,
                aria => {
                    host.enable_standard(&name, aria.is_some())
                        .map_err(|e| option_error(e.to_string()))?;
                    if aria == Some(&true) {
                        host.enable_standard_tabs_renderer("aria")
                            .map_err(|e| option_error(e.to_string()))?;
                    }
                }
            }
        }
    }
    for extension in host_extensions {
        host.register_extension(extension)
            .map_err(|e| option_error(e.to_string()))?;
    }
    if !transforms.is_undefined() && !transforms.is_null() {
        let list = transforms
            .dyn_ref::<js_sys::Array>()
            .ok_or_else(|| option_error("transforms must be a list of functions"))?;
        for (index, item) in list.iter().enumerate() {
            let callback = function(&item)
                .ok_or_else(|| option_error("transforms must be a list of functions"))?;
            host.register_extension(Extension {
                declaration: ExtensionDeclaration {
                    name: format!("host.transform.{index}"),
                    version: "1".into(),
                    required: false,
                    slots: Vec::new(),
                },
                renderers: Vec::new(),
                transform: Some(Box::new(HostFunction(callback))),
            })
            .map_err(|e| option_error(e.to_string()))?;
        }
    }

    let rendered = host
        .render_with_diagnostics(source, &opts)
        .map_err(engine_error)?;
    let result = serde_json::json!({
        "html": rendered.html,
        "diagnostics": rendered.diagnostics,
        "suppressedDiagnostics": rendered.suppressed_diagnostics,
    });
    to_js(&result)
}
