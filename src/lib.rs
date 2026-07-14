use wasm_bindgen::prelude::*;

/// A `name -> value` symbol map, already lowered out of JS.
type SymbolPairs = Vec<(String, String)>;

/// Lower a JS symbol map (a plain object or a `Map`) into owned
/// `(name, value)` pairs.
///
/// `undefined` / `null` means "no symbols". Names and values MUST both be
/// strings; anything else throws a JS `TypeError`, so a mistyped map fails fast
/// (matching the sibling bindings, which raise `TypeError` too) instead of
/// being silently dropped.
fn symbol_pairs(symbols: Option<js_sys::Object>) -> Result<SymbolPairs, JsValue> {
    let Some(obj) = symbols else {
        return Ok(SymbolPairs::new());
    };
    let value: JsValue = obj.clone().into();
    if value.is_null() || value.is_undefined() {
        return Ok(SymbolPairs::new());
    }

    // A `Map` keeps its data outside its own properties, so `Object.entries`
    // would see nothing - use the Map's own `entries()` iterator instead.
    let entries: js_sys::Array = match value.dyn_ref::<js_sys::Map>() {
        Some(map) => js_sys::Array::from(&map.entries().into()),
        None => js_sys::Object::entries(&obj),
    };

    let mut pairs = SymbolPairs::with_capacity(entries.length() as usize);
    for entry in entries.iter() {
        let pair: js_sys::Array = entry.into();
        let name = pair.get(0).as_string().ok_or_else(|| {
            JsValue::from(js_sys::TypeError::new(
                "carve: symbol names must be strings",
            ))
        })?;
        let value = pair.get(1).as_string().ok_or_else(|| {
            JsValue::from(js_sys::TypeError::new(&format!(
                "carve: symbol value for \"{name}\" must be a string"
            )))
        })?;
        pairs.push((name, value));
    }
    Ok(pairs)
}

/// Render with the core (no-extension) profile plus the given symbol map.
fn render_core(source: &str, symbols: &SymbolPairs) -> String {
    if symbols.is_empty() {
        return carve::to_html(source);
    }
    let mut options = carve::Options::new();
    for (name, value) in symbols {
        options = options.with_symbol(name.clone(), value.clone());
    }
    carve::to_html_with_options(source, &options)
}

/// Render with the demo-useful built-in extension set plus the given symbol map.
fn render_full(source: &str, symbols: &SymbolPairs) -> String {
    use carve::{
        Autolink, CarveExtension, Citations, CodeCallouts, Details, ExternalLinks, FencedRender,
        HeadingPermalinks, ListTable, MathBlock, Options, TabNormalize, Wikilinks,
    };
    // `Options` borrows each extension, so the owned boxes must outlive it;
    // they live in this frame, alongside the render call.
    let owned: Vec<Box<dyn CarveExtension>> = vec![
        Box::new(TabNormalize::new()),
        Box::new(Details::new()),
        Box::new(FencedRender::mermaid()),
        Box::new(Wikilinks::new()),
        Box::new(Autolink::new()),
        Box::new(ListTable::new()),
        Box::new(MathBlock::new()),
        Box::new(HeadingPermalinks::new()),
        Box::new(Citations::new()),
        Box::new(CodeCallouts::new()),
        Box::new(ExternalLinks::new()),
    ];
    let mut options = Options::new();
    for ext in &owned {
        options = options.with_extension(ext.as_ref());
    }
    for (name, value) in symbols {
        options = options.with_symbol(name.clone(), value.clone());
    }
    carve::to_html_with_options(source, &options)
}

#[wasm_bindgen(js_name = toHtml)]
pub fn to_html(source: &str) -> String {
    carve::to_html(source)
}

/// Render with the core profile and a **symbols map**: `{ rocket: "🚀" }` (a
/// plain object or a `Map`). A `:name:` symbol whose name is in the map renders
/// the mapped value; an unmapped `:name:` stays literal `:name:` text, and the
/// leading word-boundary guard still applies (`a:b:c`, `10:30:`,
/// `me@example.com` never become symbols).
///
/// Names and values must both be strings; a non-string value throws a JS
/// `TypeError`.
///
/// SECURITY: a mapped value is inserted as **TRUSTED RAW output in the target
/// format** - it is NOT escaped, the same trust class as the static `renderers`
/// map. So `{ b: "<b>x</b>" }` emits a real `<b>` element, not escaped text.
/// This is deliberate: processor configuration is trusted. NEVER build a
/// symbols map out of untrusted / user-supplied input.
#[wasm_bindgen(js_name = toHtmlWithSymbols)]
pub fn to_html_with_symbols(
    source: &str,
    symbols: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    Ok(render_core(source, &symbol_pairs(symbols)?))
}

/// Render with the demo-useful built-in Carve extensions enabled
/// (tab-normalize, details, Mermaid, wikilinks, autolink, list-table,
/// math-block, heading-permalinks, citations, code-callouts, external-links).
/// Lets the WASM engine match an extensions-on host (e.g. the docs Playground)
/// instead of the core-only `toHtml`.
///
/// Deliberately excludes table-of-contents (it auto-injects a TOC, which
/// clutters a preview). The code-group / tabs extensions are also absent
/// because carve-rs does not implement them.
///
/// The optional second argument is the same **symbols map** as
/// [`to_html_with_symbols`], with the same trusted-raw contract: mapped values
/// are emitted UNESCAPED, so never feed it untrusted input.
#[wasm_bindgen(js_name = toHtmlFull)]
pub fn to_html_full(source: &str, symbols: Option<js_sys::Object>) -> Result<String, JsValue> {
    Ok(render_full(source, &symbol_pairs(symbols)?))
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::{render_core, render_full, SymbolPairs};

    /// Build the lowered symbol map the JS bridge produces (the `js_sys`
    /// conversion itself only runs inside a JS host).
    fn symbols(pairs: &[(&str, &str)]) -> SymbolPairs {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn renders_html() {
        assert!(crate::to_html("# Hello").contains("<h1>Hello</h1>"));
    }

    #[test]
    fn full_enables_mermaid_extension() {
        let html = render_full("``` mermaid\ngraph TD; A-->B\n```\n", &SymbolPairs::new());
        assert!(html.contains("<pre class=\"mermaid\">"));
    }

    #[test]
    fn full_enables_list_table_extension() {
        let src = "{header-rows=1}\n::: list-table \"Quarterly results\"\n- - Region\n  - Notes\n- - EMEA\n  - Strong quarter.\n:::\n";
        let html = render_full(src, &SymbolPairs::new());
        assert!(html.contains("<table"), "expected a <table>, got: {html}");
        assert!(!html.contains("class=\"list-table\""));
    }

    #[test]
    fn full_enables_code_callouts_extension() {
        let src = "``` rust\nlet x = 1; // <1>\n```\n\n<1> Assign x.\n";
        let html = render_full(src, &SymbolPairs::new());
        assert!(
            html.contains("class=\"callout\""),
            "expected callout bubble, got: {html}"
        );
        assert!(
            html.contains("class=\"callouts\""),
            "expected callouts list, got: {html}"
        );
    }

    // The engine is tracked as a git dependency on carve-rs `main` with no
    // pinned rev, and Cargo.lock is not committed here — so these tests are
    // what actually holds the engine's current language surface in place.

    #[test]
    fn superscript_and_subscript_are_braced_only() {
        // Bare `^x^` / `,x,` are literal text; only the braced forms mark up.
        let html = crate::to_html("a ^2^ b and H,2,O");
        assert!(
            !html.contains("<sup>"),
            "bare ^x^ must stay literal, got: {html}"
        );
        assert!(
            !html.contains("<sub>"),
            "bare ,x, must stay literal, got: {html}"
        );

        let html = crate::to_html("x{^2^} and H{,2,}O");
        assert!(
            html.contains("<sup>2</sup>"),
            "expected a <sup>, got: {html}"
        );
        assert!(
            html.contains("<sub>2</sub>"),
            "expected a <sub>, got: {html}"
        );
    }

    #[test]
    fn symbol_inline_is_recognized_with_a_word_boundary_guard() {
        // An unmapped symbol renders its `:name:` source, but it is a real
        // Symbol node — attaching attributes proves it parsed as one.
        let html = crate::to_html(":smile:{.emoji}");
        assert!(
            html.contains("<span class=\"emoji\">:smile:</span>"),
            "expected a symbol span, got: {html}"
        );

        // The leading word-boundary guard keeps these literal (no span).
        let html = crate::to_html("a:b:c and 10:30: and me@example.com");
        assert!(
            !html.contains("<span"),
            "guarded colons must stay literal, got: {html}"
        );
    }

    #[test]
    fn mapped_symbol_renders_its_value() {
        let map = symbols(&[("rocket", "🚀")]);

        let html = render_core("Ship it :rocket:", &map);
        assert!(
            html.contains("Ship it 🚀"),
            "expected the mapped value, got: {html}"
        );
        assert!(
            !html.contains(":rocket:"),
            "a mapped name must not stay literal, got: {html}"
        );

        // The same map flows through the extensions-on entry point.
        let html = render_full("Ship it :rocket:", &map);
        assert!(
            html.contains("🚀"),
            "expected the mapped value, got: {html}"
        );
    }

    #[test]
    fn plus_one_is_a_valid_symbol_name() {
        let html = render_core("nice :+1:", &symbols(&[("+1", "👍")]));
        assert!(
            html.contains("nice 👍"),
            "expected :+1: to map, got: {html}"
        );
    }

    #[test]
    fn unmapped_symbol_stays_literal_with_a_map_active() {
        let html = render_core(":rocket: and :unmapped:", &symbols(&[("rocket", "🚀")]));
        assert!(
            html.contains("🚀"),
            "expected the mapped value, got: {html}"
        );
        assert!(
            html.contains(":unmapped:"),
            "an unmapped name must stay literal, got: {html}"
        );
    }

    #[test]
    fn word_boundary_guard_still_holds_with_a_map_active() {
        // Each of these names WOULD map if the guard were lost.
        let map = symbols(&[
            ("b", "MAPPED-B"),
            ("30", "MAPPED-30"),
            ("example", "MAPPED-EX"),
        ]);
        let html = render_core("a:b:c and 10:30: and me@example.com", &map);
        assert!(
            html.contains("a:b:c") && html.contains("10:30:") && html.contains("me@example.com"),
            "guarded colons must stay literal, got: {html}"
        );
        assert!(
            !html.contains("MAPPED-"),
            "no guarded run may map, got: {html}"
        );
    }

    #[test]
    fn mapped_value_is_trusted_raw_output_not_escaped() {
        // Documented contract: a symbol value is inserted RAW (same trust class
        // as the renderers map), so markup comes through as markup.
        let html = render_core(":bold:", &symbols(&[("bold", "<b>x</b>")]));
        assert!(
            html.contains("<b>x</b>"),
            "symbol value must be emitted raw, got: {html}"
        );
        assert!(
            !html.contains("&lt;b&gt;"),
            "symbol value must NOT be escaped, got: {html}"
        );
    }
}
