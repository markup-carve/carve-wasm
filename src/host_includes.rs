//! The `includes` option with a host resolver, which the JSON boundary has no form for.

use carve::api::{Cause, Includes, Request};
use js_sys::{Function, Object, Reflect, Uint8Array};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// A JS function held by the engine's resolver.
struct HostFunction(Function);

// SAFETY: wasm32-unknown-unknown without atomics runs on one thread, so the
// function is never touched from another. The engine requires the bounds.
unsafe impl Send for HostFunction {}
unsafe impl Sync for HostFunction {}

/// Splits a host resolver off `options`: `includes` given as a function, or as
/// an object whose `resolver` is one. Returns the options without `includes`
/// and the engine value, or the options unchanged when there is no resolver.
pub fn split(options: JsValue) -> Result<(JsValue, Option<Includes>), JsValue> {
    if !options.is_object() {
        return Ok((options, None));
    }
    let value = Reflect::get(&options, &"includes".into())?;
    let (resolver, config) = if let Some(function) = value.dyn_ref::<Function>() {
        (function.clone(), None)
    } else if value.is_object() {
        match Reflect::get(&value, &"resolver".into())?.dyn_into::<Function>() {
            Ok(function) => (function, Some(value)),
            Err(_) => return Ok((options, None)),
        }
    } else {
        return Ok((options, None));
    };

    let mut includes = Includes::new(resolver_of(resolver));
    if let Some(config) = config {
        let keys = Object::keys(config.unchecked_ref::<Object>());
        for key in keys.iter() {
            let key = key.as_string().unwrap_or_default();
            let item = Reflect::get(&config, &key.as_str().into())?;
            match key.as_str() {
                "resolver" => {}
                "file" if item.is_string() => includes.file = item.as_string(),
                "file" if item.is_undefined() => {}
                "depth" => includes.depth = limit(&item)?,
                "bytes" => includes.bytes = limit(&item)?,
                _ => {
                    return Err(js_sys::TypeError::new(&format!(
                        "invalid options: includes has an unknown or invalid field {key}"
                    ))
                    .into())
                }
            }
        }
    }

    let rest = Object::assign(&Object::new(), options.unchecked_ref::<Object>());
    Reflect::delete_property(&rest, &"includes".into())?;
    Ok((rest.into(), Some(includes)))
}

fn limit(value: &JsValue) -> Result<usize, JsValue> {
    value
        .as_f64()
        .filter(|n| n.fract() == 0.0 && *n >= 0.0 && *n <= u32::MAX.into())
        .map(|n| n as usize)
        .ok_or_else(|| {
            js_sys::TypeError::new("invalid options: an includes limit is not a count").into()
        })
}

fn resolver_of(function: Function) -> carve::api::Resolver {
    let host = HostFunction(function);
    Arc::new(move |request: &Request<'_>| answer(&host.0, request))
}

/// Calls the host for one directive. A throw or an answer of the wrong shape
/// counts as `unreadable`.
fn answer(function: &Function, request: &Request<'_>) -> Result<(Vec<u8>, String), Cause> {
    let argument = Object::new();
    let set = |key: &str, value: JsValue| {
        let _ = Reflect::set(&argument, &key.into(), &value);
    };
    set("path", request.path.into());
    if let Some(section) = request.section {
        set("section", section.into());
    }
    set(
        "options",
        js_sys::JSON::parse(&request.options.to_string()).unwrap_or(JsValue::NULL),
    );
    if let Some(from) = request.from {
        set("from", from.into());
    }
    let result = function
        .call1(&JsValue::NULL, &argument)
        .map_err(|_| Cause::Unreadable)?;
    if !result.is_object() {
        return Err(Cause::Unreadable);
    }
    let field = |key: &str| Reflect::get(&result, &key.into()).unwrap_or(JsValue::UNDEFINED);
    if let Some(error) = field("error").as_string() {
        return Err(match error.as_str() {
            "not_found" => Cause::NotFound,
            "outside_root" => Cause::OutsideRoot,
            "non_text" => Cause::NonText,
            _ => Cause::Unreadable,
        });
    }
    let id = field("id").as_string().ok_or(Cause::Unreadable)?;
    let text = field("text");
    let bytes = if let Some(text) = text.as_string() {
        text.into_bytes()
    } else if let Some(bytes) = text.dyn_ref::<Uint8Array>() {
        bytes.to_vec()
    } else {
        return Err(Cause::Unreadable);
    };
    Ok((bytes, id))
}
