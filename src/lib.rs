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
fn render_core(source: &str, symbols: &SymbolPairs, sections: bool) -> String {
    if symbols.is_empty() && sections {
        return carve::to_html(source);
    }
    let mut options = carve::Options::new().with_sections(sections);
    for (name, value) in symbols {
        options = options.with_symbol(name.clone(), value.clone());
    }
    carve::to_html_with_options(source, &options)
}

/// The preview extension set behind `full: true`.
///
/// These are registry keys, resolved through the engine rather than
/// constructed here, so a rename upstream fails
/// `preview_set_names_are_all_registered` instead of silently dropping an
/// extension from every `full` render.
///
/// It is a curated subset on purpose, not "everything registered". Extensions
/// that rewrite a document whether or not it asks - `heading-numbers` numbers
/// every heading, `table-of-contents` injects a TOC - would change the output
/// of a preview that never opted in. Everything here only acts on syntax the
/// document actually contains.
const PREVIEW_EXTENSIONS: &[&str] = &[
    "tab-normalize",
    "details",
    "fenced-render",
    "wikilinks",
    "autolink",
    "list-table",
    "math-block",
    "heading-permalinks",
    "citations",
    "code-callouts",
    "external-links",
    "code-group",
    "tabs",
];

/// Build owned extension instances for the given registry keys.
///
/// Unknown keys are skipped rather than erroring: the caller-facing entry
/// points validate names and report them, while the internal preview set is
/// covered by a test.
fn build_extensions(keys: &[String]) -> Vec<Box<dyn carve::CarveExtension>> {
    keys.iter()
        .filter_map(|key| carve::extensions::registry::by_key(key))
        .collect()
}

/// Render with the named extensions plus the given symbol map.
fn render_with_extensions(
    source: &str,
    keys: &[String],
    symbols: &SymbolPairs,
    sections: bool,
) -> String {
    // `Options` borrows each extension, so the owned boxes must outlive it;
    // they live in this frame, alongside the render call.
    let owned = build_extensions(keys);
    let mut options = carve::Options::new().with_sections(sections);
    for ext in &owned {
        options = options.with_extension(ext.as_ref());
    }
    for (name, value) in symbols {
        options = options.with_symbol(name.clone(), value.clone());
    }
    carve::to_html_with_options(source, &options)
}

/// Render with the preview extension set plus the given symbol map.
fn render_full(source: &str, symbols: &SymbolPairs, sections: bool) -> String {
    let keys: Vec<String> = PREVIEW_EXTENSIONS
        .iter()
        .map(|k| (*k).to_string())
        .collect();
    render_with_extensions(source, &keys, symbols, sections)
}

/// Every extension name this build accepts, in registry order.
///
/// Taken from the engine, so a new extension is reachable as soon as the pin
/// moves. Nothing here lists names.
#[wasm_bindgen(js_name = extensions)]
pub fn extensions() -> Vec<String> {
    carve::extensions::registry::keys()
        .map(str::to_string)
        .collect()
}

#[wasm_bindgen(js_name = toHtml)]
pub fn to_html(source: &str) -> String {
    carve::to_html(source)
}

#[wasm_bindgen(js_name = toMarkdown)]
pub fn to_markdown(source: &str) -> String {
    carve::to_markdown(source)
}

#[wasm_bindgen(js_name = toPlainText)]
pub fn to_plain_text(source: &str) -> String {
    carve::to_plain_text(source)
}

#[wasm_bindgen(js_name = toAnsi)]
pub fn to_ansi(source: &str) -> String {
    carve::to_ansi(source)
}

#[wasm_bindgen(js_name = toCarve)]
pub fn to_carve(source: &str) -> String {
    carve::to_carve(source)
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
    Ok(render_core(source, &symbol_pairs(symbols)?, true))
}

/// Render with the preview extension set enabled (`PREVIEW_EXTENSIONS`), so the
/// WASM engine matches an extensions-on host such as the docs Playground rather
/// than the core-only `toHtml`.
///
/// The set is curated, not "everything the engine has": `heading-numbers` and
/// `table-of-contents` rewrite a document that never asked for it, which is
/// wrong for a preview. Callers who want an exact set pass `extensions` to
/// `toHtmlWithOptions`, and `extensions()` reports what this build accepts.
///
/// The optional second argument is the same **symbols map** as
/// [`to_html_with_symbols`], with the same trusted-raw contract: mapped values
/// are emitted UNESCAPED, so never feed it untrusted input.
#[wasm_bindgen(js_name = toHtmlFull)]
pub fn to_html_full(source: &str, symbols: Option<js_sys::Object>) -> Result<String, JsValue> {
    Ok(render_full(source, &symbol_pairs(symbols)?, true))
}

/// Parse Carve source and return its AST as a JSON string.
///
/// The PART 12 exchange shape (https://markup-carve.github.io/carve/ast-json):
/// the same tree every Carve engine publishes, so a consumer written against
/// one implementation reads another's output. The root carries exactly `type`,
/// `children` and `srcByteLength`; frontmatter and footnote definitions are
/// block nodes inside `children`, not root fields.
///
/// Returns a STRING rather than a JS object: the caller runs `JSON.parse`, which
/// is what a browser does natively and faster than building the object graph
/// across the wasm boundary one property at a time. It also keeps the bytes
/// available for a caller that stores or forwards them.
///
/// Position tracking is on for this entry point and nowhere else. PART 12 §4
/// lets an engine gate tracking behind a parse option but requires the
/// serialized form to carry it, and rendering would pay for spans nobody reads.
#[wasm_bindgen(js_name = parseJson)]
pub fn parse_json(source: &str) -> String {
    let mut options = carve::Options::new();
    options.positions = true;
    carve::to_json(&carve::parse_with_options(source, &options))
}

fn html_import_mode(value: Option<String>) -> Result<carve::HtmlImportMode, JsValue> {
    match value.as_deref().unwrap_or("safe") {
        "safe" => Ok(carve::HtmlImportMode::Safe),
        "semantic" => Ok(carve::HtmlImportMode::Semantic),
        "roundtrip" => Ok(carve::HtmlImportMode::Roundtrip),
        other => Err(JsValue::from(js_sys::TypeError::new(&format!(
            "carve: unknown HTML import mode `{other}`"
        )))),
    }
}

fn html_import_report_json(report: &carve::HtmlImportReport) -> String {
    let mode = match report.mode {
        carve::HtmlImportMode::Safe => "safe",
        carve::HtmlImportMode::Semantic => "semantic",
        carve::HtmlImportMode::Roundtrip => "roundtrip",
    };
    let diagnostics = report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let code = match diagnostic.code {
                carve::HtmlImportDiagnosticCode::ElementDropped => "element-dropped",
                carve::HtmlImportDiagnosticCode::ElementUnwrapped => "element-unwrapped",
                carve::HtmlImportDiagnosticCode::AttributeDropped => "attribute-dropped",
                carve::HtmlImportDiagnosticCode::StyleUnmapped => "style-unmapped",
                carve::HtmlImportDiagnosticCode::TableDegraded => "table-degraded",
                carve::HtmlImportDiagnosticCode::RawPreserved => "raw-preserved",
                // Added by the engine after the previous pin. The match is
                // deliberately exhaustive rather than a `_` arm: relaying a new
                // code under a guessed spelling, or dropping it, is worse than
                // failing to build, and this is the only place that would
                // notice. Spellings copied from `report_vocabulary!` in
                // carve-rs `src/html_import.rs`, which is what the spec's
                // resources/html-import-schema.json admits.
                carve::HtmlImportDiagnosticCode::StructureUnspellable => "structure-unspellable",
                carve::HtmlImportDiagnosticCode::StructureSplit => "structure-split",
                carve::HtmlImportDiagnosticCode::EncodingAssumed => "encoding-assumed",
                carve::HtmlImportDiagnosticCode::DiagnosticsTruncated => "diagnostics-truncated",
            };
            let severity = match diagnostic.severity {
                carve::HtmlImportSeverity::Info => "info",
                carve::HtmlImportSeverity::Warning => "warning",
                carve::HtmlImportSeverity::Error => "error",
            };
            format!(
                "{{\"code\":\"{code}\",\"message\":{:?},\"severity\":\"{severity}\"{}}}",
                diagnostic.message,
                diagnostic
                    .path
                    .as_ref()
                    .map(|path| format!(",\"path\":{path:?}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"mode\":\"{mode}\",\"adapter\":\"generic\",\"diagnostics\":[{diagnostics}]}}")
}

/// Import HTML through the Rust HTML5 DOM and canonical Carve writer.
///
/// Returns `{ value, report }`; `report.diagnostics` makes every lossy import
/// decision observable. `roundtrip` is only safe for Carve-produced HTML.
#[wasm_bindgen(js_name = htmlToCarve)]
pub fn html_to_carve(source: &str, mode: Option<String>) -> Result<JsValue, JsValue> {
    let options = carve::HtmlImportOptions {
        mode: html_import_mode(mode)?,
        ..Default::default()
    };
    let result = carve::html_to_carve(source, &options)
        .map_err(|error| JsValue::from_str(&format!("carve: HTML import failed: {error:?}")))?;
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("value"),
        &JsValue::from_str(&result.value),
    )?;
    let report = js_sys::JSON::parse(&html_import_report_json(&result.report))?;
    js_sys::Reflect::set(&object, &JsValue::from_str("report"), &report)?;
    Ok(object.into())
}

#[wasm_bindgen(js_name = fromHtml)]
pub fn from_html(source: &str, mode: Option<String>) -> Result<JsValue, JsValue> {
    html_to_carve(source, mode)
}

#[wasm_bindgen(js_name = fromMarkdown)]
pub fn from_markdown(source: &str) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("value"),
        &JsValue::from_str(&carve::markdown_to_carve(source)),
    )?;
    let report = js_sys::JSON::parse(r#"{"sourceFormat":"markdown","diagnostics":[]}"#)?;
    js_sys::Reflect::set(&object, &JsValue::from_str("report"), &report)?;
    Ok(object.into())
}

/// Read one boolean field out of a JS options object.
///
/// Absent, `undefined` and `null` all mean "not set", so a caller can pass a
/// partially-filled object. A present-but-non-boolean value throws a JS
/// `TypeError` rather than being coerced: `{ sections: "false" }` is a mistake
/// worth surfacing, and JS truthiness would read that string as `true` - the
/// opposite of what was written.
fn bool_field(options: &js_sys::Object, key: &str) -> Result<Option<bool>, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str(key))?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    value.as_bool().map(Some).ok_or_else(|| {
        JsValue::from(js_sys::TypeError::new(&format!(
            "carve: `{key}` must be a boolean"
        )))
    })
}

/// Render with an options object, the general form of the three shorthands
/// above.
///
/// ```js
/// toHtmlWithOptions('# A\n\np\n', { sections: false })
/// // '<h1 id="A">A</h1>\n<p>p</p>'
///
/// toHtmlWithOptions(src, { sections: false, symbols: { rocket: '🚀' }, full: true })
/// ```
///
/// Every field is optional:
///
/// * `sections` (default `true`) - wrap each top-level heading, and the content
///   following it up to the next same-or-shallower heading, in a
///   `<section id="…">` (spec PART 9 §13). `false` renders headings flat with
///   the id back on the `<h*>` and the former section children as siblings.
///   For a host whose CSS or JS assumes rendered blocks are direct children of
///   the content container - the `.stack > * + *` spacing idiom,
///   `:first-child`, `nth-child()` counting, `element.children` walks - the
///   wrapper is the one output change a clean source migration still breaks.
/// * `symbols` - the same map as [`to_html_with_symbols`], with the same
///   TRUSTED-RAW contract: mapped values are emitted UNESCAPED, so never build
///   it from untrusted input.
/// * `extensions` - an array of extension names to enable, e.g.
///   `["glossary", "table-of-contents"]`. `extensions()` reports what this
///   build accepts. An unknown name throws: a silently ignored extension would
///   render as missing behavior that looks like a Carve bug. Takes precedence
///   over `full`.
/// * `full` (default `false`) - enable the preview extension set instead of
///   rendering core-only.
///
/// An unrecognized key is ignored: the object is configuration, and a caller
/// who mistypes one deserves the render to still work. A wrong TYPE on a key
/// that is recognized does throw, because that changes behavior silently.
///
/// Turning sections off changes nothing else. Ids, collision dedup, `</#id>`
/// crossrefs, implicit `[Heading][]` references and heading numbering all
/// resolve against the slug rather than the element carrying it, and the
/// endnotes `<section role="doc-endnotes">` is a separate construct that is
/// still emitted.
#[wasm_bindgen(js_name = toHtmlWithOptions)]
pub fn to_html_with_options(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    let Some(options) = options else {
        return Ok(carve::to_html(source));
    };
    let value: JsValue = options.clone().into();
    if value.is_null() || value.is_undefined() {
        return Ok(carve::to_html(source));
    }

    let sections = bool_field(&options, "sections")?.unwrap_or(true);
    let full = bool_field(&options, "full")?.unwrap_or(false);
    let named = extension_names_field(&options)?;
    // A wrong-typed `symbols` must THROW, not quietly render without symbols:
    // `dyn_into().ok()` would turn `{ symbols: "rocket" }` into `None` and lose
    // the caller's map with no signal. Absent / null / undefined still mean
    // "no symbols".
    let symbols = js_sys::Reflect::get(&options, &JsValue::from_str("symbols"))?;
    let symbols = if symbols.is_undefined() || symbols.is_null() {
        None
    } else {
        Some(symbols.dyn_into::<js_sys::Object>().map_err(|_| {
            JsValue::from(js_sys::TypeError::new(
                "carve: `symbols` must be an object or a Map",
            ))
        })?)
    };
    let pairs = symbol_pairs(symbols)?;

    Ok(match (named, full) {
        // An explicit list wins over the preview set: a caller who names
        // extensions has said exactly what they want.
        (Some(keys), _) => render_with_extensions(source, &keys, &pairs, sections),
        (None, true) => render_full(source, &pairs, sections),
        (None, false) => render_core(source, &pairs, sections),
    })
}

/// Read the `extensions` option: absent, or an array of registry names.
///
/// An unknown name THROWS rather than being skipped. A mistyped extension is
/// not configuration noise - the render would silently lack the behavior the
/// caller asked for, and the output would look like a Carve bug.
fn extension_names_field(options: &js_sys::Object) -> Result<Option<Vec<String>>, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str("extensions"))?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    let array = value.dyn_into::<js_sys::Array>().map_err(|_| {
        JsValue::from(js_sys::TypeError::new(
            "carve: `extensions` must be an array of extension names",
        ))
    })?;
    let mut keys = Vec::with_capacity(array.length() as usize);
    for entry in array.iter() {
        let name = entry.as_string().ok_or_else(|| {
            JsValue::from(js_sys::TypeError::new(
                "carve: each entry in `extensions` must be a string",
            ))
        })?;
        // Registry keys are kebab-case; accept snake_case too, the way the
        // Python and Ruby bindings do.
        let key = name.trim().to_ascii_lowercase().replace('_', "-");
        if carve::extensions::registry::by_key(&key).is_none() {
            return Err(JsValue::from(js_sys::TypeError::new(&format!(
                "carve: unknown extension \"{name}\" (see extensions())"
            ))));
        }
        keys.push(key);
    }
    Ok(Some(keys))
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        extensions, html_import_report_json, render_core, render_full, render_with_extensions,
        SymbolPairs, PREVIEW_EXTENSIONS,
    };

    /// Build the lowered symbol map the JS bridge produces (the `js_sys`
    /// conversion itself only runs inside a JS host).
    fn symbols(pairs: &[(&str, &str)]) -> SymbolPairs {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn html_import_report_is_json() {
        let result =
            carve::html_to_carve("<p onclick=\"x()\">safe</p>", &Default::default()).unwrap();
        let report = html_import_report_json(&result.report);
        assert!(report.contains("\"attribute-dropped\""));
        assert!(report.contains("\"mode\":\"safe\""));
    }

    // PART 9 §13: the wrapper is on by default and `sections: false` removes it,
    // putting the id back on the <h*>. A heading inside a container is not
    // wrapped either way, which is the shape the flat form matches.
    #[test]
    fn sections_off_emits_no_wrapper() {
        let none = SymbolPairs::new();
        assert_eq!(
            render_core("# A\n\np\n", &none, true),
            "<section id=\"A\">\n  <h1>A</h1>\n  <p>p</p>\n</section>"
        );
        assert_eq!(
            render_core("# A\n\np\n", &none, false),
            "<h1 id=\"A\">A</h1>\n<p>p</p>"
        );
    }

    #[test]
    fn sections_off_leaves_container_headings_alone() {
        let none = SymbolPairs::new();
        let src = "> # Quoted\n>\n> Quoted body.\n";
        assert_eq!(
            render_core(src, &none, false),
            render_core(src, &none, true)
        );
    }

    // The flag composes with the other two axes rather than being exclusive
    // with them, which the symbols-empty fast path inside render_core makes
    // easy to get wrong.
    #[test]
    fn sections_off_composes_with_symbols_and_extensions() {
        let map = symbols(&[("rocket", "🚀")]);
        let core = render_core("# A\n\n:rocket:\n", &map, false);
        assert!(core.starts_with("<h1 id=\"A\">A</h1>"), "{core}");
        assert!(core.contains('🚀'), "{core}");
        assert!(!core.contains("<section"), "{core}");

        let full = render_full("# A\n\n:rocket:\n", &map, false);
        assert!(full.contains('🚀'), "{full}");
        assert!(!full.contains("<section"), "{full}");
    }

    #[test]
    fn renders_html() {
        assert!(crate::to_html("# Hello").contains("<h1>Hello</h1>"));
    }

    #[test]
    fn marker_attributes_do_not_move_the_content_column() {
        let source = "-{title=\"😀\"} [x] a\n  # h\n";
        let html = crate::to_html(source);
        assert!(html.contains("<h1 id=\"h\">h</h1>"), "{html}");
        let canonical = crate::to_carve(source);
        assert_eq!(canonical, "-{title=😀} [x] a\n  # h\n");
        assert_eq!(crate::to_html(&canonical), html);
    }

    #[test]
    fn exposes_every_core_render_target() {
        let source = "# Hello\n\nBody\n";
        assert!(crate::to_markdown(source).contains("# Hello"));
        assert!(crate::to_plain_text(source).contains("Hello"));
        assert!(crate::to_ansi(source).contains("Hello"));
        assert_eq!(crate::to_carve("# Hello\n\n\nBody"), source);
    }

    #[test]
    fn full_enables_mermaid_extension() {
        let html = render_full(
            "``` mermaid\ngraph TD; A-->B\n```\n",
            &SymbolPairs::new(),
            true,
        );
        // The hydration element carries the accessible name the engine gives a
        // diagram fence (PART 9 §16a, carve-rs #1187): an image with no name is
        // skipped by a reader entirely, so the role and the label are written
        // together. Asserted whole rather than by class alone - a substring that
        // stops at the class would pass again if the name were dropped.
        assert!(
            html.contains("<pre class=\"mermaid\" role=\"img\" aria-label=\"mermaid\">"),
            "expected the named mermaid hydration element, got: {html}"
        );
    }

    #[test]
    fn full_enables_list_table_extension() {
        let src = "{header-rows=1}\n::: list-table \"Quarterly results\"\n- - Region\n  - Notes\n- - EMEA\n  - Strong quarter.\n:::\n";
        let html = render_full(src, &SymbolPairs::new(), true);
        assert!(html.contains("<table"), "expected a <table>, got: {html}");
        assert!(!html.contains("class=\"list-table\""));
    }

    #[test]
    fn preview_set_names_are_all_registered() {
        // The preview set is the one place a name is still written down here.
        // If the engine renames or drops one, that extension would silently
        // stop applying to every `full` render; this fails instead.
        for key in PREVIEW_EXTENSIONS {
            assert!(
                carve::extensions::registry::by_key(key).is_some(),
                "preview set names {key:?}, which the engine does not register"
            );
        }
    }

    #[test]
    fn extensions_reports_what_the_engine_registers() {
        let names = extensions();
        // Reachable now, and unreachable before this binding read the registry:
        // there was no name-based entry point at all.
        for expected in ["glossary", "index", "table-of-contents", "heading-numbers"] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn named_extensions_render() {
        let src = "# Heading\n";
        let keys = vec!["heading-permalinks".to_string()];
        let html = render_with_extensions(src, &keys, &SymbolPairs::new(), true);
        assert!(html.contains("class=\"permalink\""), "got: {html}");
    }

    #[test]
    fn an_unnamed_render_is_unaffected_by_the_registry() {
        // Core stays core: reading the registry must not enable anything.
        let src = "# Heading\n";
        let core = render_core(src, &SymbolPairs::new(), true);
        assert!(!core.contains("class=\"permalink\""), "got: {core}");
    }

    #[test]
    fn full_enables_code_callouts_extension() {
        let src = "``` rust\nlet x = 1; // <1>\n```\n\n<1> Assign x.\n";
        let html = render_full(src, &SymbolPairs::new(), true);
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

        let html = render_core("Ship it :rocket:", &map, true);
        assert!(
            html.contains("Ship it 🚀"),
            "expected the mapped value, got: {html}"
        );
        assert!(
            !html.contains(":rocket:"),
            "a mapped name must not stay literal, got: {html}"
        );

        // The same map flows through the extensions-on entry point.
        let html = render_full("Ship it :rocket:", &map, true);
        assert!(
            html.contains("🚀"),
            "expected the mapped value, got: {html}"
        );
    }

    #[test]
    fn plus_one_is_a_valid_symbol_name() {
        let html = render_core("nice :+1:", &symbols(&[("+1", "👍")]), true);
        assert!(
            html.contains("nice 👍"),
            "expected :+1: to map, got: {html}"
        );
    }

    #[test]
    fn unmapped_symbol_stays_literal_with_a_map_active() {
        let html = render_core(
            ":rocket: and :unmapped:",
            &symbols(&[("rocket", "🚀")]),
            true,
        );
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
        let html = render_core("a:b:c and 10:30: and me@example.com", &map, true);
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
        let html = render_core(":bold:", &symbols(&[("bold", "<b>x</b>")]), true);
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
