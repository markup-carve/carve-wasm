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

/// The render switches the JS options object exposes.
///
/// One struct rather than a parameter per switch: they all default one way,
/// they would all sit last in three near-identical helper signatures, and a
/// transposed pair of bools would compile, render wrongly, and read correctly
/// at the call site.
#[derive(Debug, Clone, PartialEq)]
struct RenderConfig {
    sections: bool,
    raw_html: bool,
    source_lines: bool,
    positions: bool,
    lowercase_heading_ids: bool,
    ascii_heading_ids: carve::AsciiHeadingIds,
    smart_typography: carve::SmartTypographyMode,
    mode: carve::Mode,
    profile: Option<carve::Profile>,
    profile_base_host: Option<String>,
    /// Ordered rather than a map so a render is reproducible from the object
    /// the caller passed, in the order they wrote it.
    labels: Vec<(String, String)>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            sections: true,
            raw_html: true,
            source_lines: false,
            positions: false,
            lowercase_heading_ids: false,
            ascii_heading_ids: carve::AsciiHeadingIds::default(),
            smart_typography: carve::SmartTypographyMode::default(),
            mode: carve::Mode::default(),
            profile: None,
            profile_base_host: None,
            labels: Vec::new(),
        }
    }
}

impl RenderConfig {
    fn apply<'a>(&self, options: carve::Options<'a>) -> carve::Options<'a> {
        let mut options = options
            .with_sections(self.sections)
            .with_raw_html(self.raw_html)
            .with_source_lines(self.source_lines)
            .with_positions(self.positions)
            .with_lowercase_heading_ids(self.lowercase_heading_ids)
            .with_ascii_heading_ids(self.ascii_heading_ids)
            .with_mode(self.mode);
        // No `with_` builder for this one upstream; the field is public.
        options.smart_typography = self.smart_typography;
        if let Some(profile) = &self.profile {
            options = options.with_profile(profile.clone());
        }
        if let Some(host) = &self.profile_base_host {
            options = options.with_profile_base_host(host.clone());
        }
        for (key, value) in &self.labels {
            options = options.with_label(key.clone(), value.clone());
        }
        options
    }
}

/// Render with the core (no-extension) profile plus the given symbol map.
fn render_core(
    source: &str,
    symbols: &SymbolPairs,
    config: &RenderConfig,
) -> Result<String, carve::ProfileViolationError> {
    // `carve::to_html` takes a layout fast path the options form does not, so
    // the default render keeps it. The guard is one equality against `Default`
    // rather than a list of fields, because a list is what a new switch gets
    // left out of.
    if symbols.is_empty() && *config == RenderConfig::default() {
        return Ok(carve::to_html(source));
    }
    let mut options = config.apply(carve::Options::new());
    for (name, value) in symbols {
        options = options.with_symbol(name.clone(), value.clone());
    }
    carve::try_to_html_with_options(source, &options)
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
    config: &RenderConfig,
) -> Result<String, carve::ProfileViolationError> {
    // `Options` borrows each extension, so the owned boxes must outlive it;
    // they live in this frame, alongside the render call.
    let owned = build_extensions(keys);
    let mut options = config.apply(carve::Options::new());
    for ext in &owned {
        options = options.with_extension(ext.as_ref());
    }
    for (name, value) in symbols {
        options = options.with_symbol(name.clone(), value.clone());
    }
    carve::try_to_html_with_options(source, &options)
}

/// Render with the preview extension set plus the given symbol map.
fn render_full(
    source: &str,
    symbols: &SymbolPairs,
    config: &RenderConfig,
) -> Result<String, carve::ProfileViolationError> {
    let keys: Vec<String> = PREVIEW_EXTENSIONS
        .iter()
        .map(|k| (*k).to_string())
        .collect();
    render_with_extensions(source, &keys, symbols, config)
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

#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toMarkdown)]
pub fn to_markdown(source: &str) -> String {
    carve::to_markdown(source)
}

#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toPlainText)]
pub fn to_plain_text(source: &str) -> String {
    carve::to_plain_text(source)
}

#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toAnsi)]
pub fn to_ansi(source: &str) -> String {
    carve::to_ansi(source)
}

#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toCarve)]
pub fn to_carve(source: &str) -> String {
    carve::to_carve(source)
}

#[cfg(feature = "reports")]
fn render_report_to_js(result: carve::RenderResult<String>) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(&object, &"value".into(), &result.value.into())?;
    js_sys::Reflect::set(
        &object,
        &"totalLosses".into(),
        &(result.total_losses as f64).into(),
    )?;
    js_sys::Reflect::set(&object, &"truncated".into(), &result.truncated.into())?;
    let losses = js_sys::Array::new();
    for loss in result.losses {
        let item = js_sys::Object::new();
        js_sys::Reflect::set(&item, &"code".into(), &loss.code.into())?;
        js_sys::Reflect::set(&item, &"format".into(), &loss.format.into())?;
        js_sys::Reflect::set(&item, &"target".into(), &loss.target.as_str().into())?;
        js_sys::Reflect::set(&item, &"nodeType".into(), &loss.node_type.as_str().into())?;
        js_sys::Reflect::set(&item, &"message".into(), &loss.message.into())?;
        if let Some(pos) = loss.pos {
            let value = js_sys::Object::new();
            for (key, number) in [
                ("startLine", pos.start_line),
                ("endLine", pos.end_line),
                ("startColumn", pos.start_column),
                ("endColumn", pos.end_column),
                ("startOffset", pos.start_offset),
                ("endOffset", pos.end_offset),
            ] {
                js_sys::Reflect::set(&value, &key.into(), &(number as f64).into())?;
            }
            js_sys::Reflect::set(&item, &"pos".into(), &value)?;
        }
        losses.push(&item);
    }
    js_sys::Reflect::set(&object, &"losses".into(), &losses)?;
    Ok(object.into())
}

#[cfg(feature = "reports")]
fn checked_options(strict: Option<bool>, maximum: Option<u32>) -> carve::CheckedRenderOptions {
    carve::CheckedRenderOptions {
        strict: strict.unwrap_or(false),
        max_losses: maximum.map_or(carve::DEFAULT_MAX_RENDER_LOSSES, |value| value as usize),
    }
}

#[cfg(feature = "reports")]
fn checked_result(
    result: Result<carve::RenderResult<String>, carve::RenderLossError>,
) -> Result<JsValue, JsValue> {
    match result {
        Ok(result) => render_report_to_js(result),
        Err(error) => {
            let message = error.to_string();
            let report = carve::RenderResult {
                value: String::new(),
                losses: error.losses,
                total_losses: error.total_losses,
                truncated: error.truncated,
            };
            let exception = js_sys::Error::new(&message);
            js_sys::Reflect::set(&exception, &"name".into(), &"RenderLossError".into())?;
            let encoded = render_report_to_js(report)?;
            for key in ["losses", "totalLosses", "truncated"] {
                js_sys::Reflect::set(
                    &exception,
                    &key.into(),
                    &js_sys::Reflect::get(&encoded, &key.into())?,
                )?;
            }
            Err(exception.into())
        }
    }
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = toHtmlWithReport)]
pub fn to_html_with_report(
    source: &str,
    strict: Option<bool>,
    maximum: Option<u32>,
) -> Result<JsValue, JsValue> {
    checked_result(carve::to_html_with_report(
        source,
        checked_options(strict, maximum),
    ))
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = toMarkdownWithReport)]
pub fn to_markdown_with_report(
    source: &str,
    strict: Option<bool>,
    maximum: Option<u32>,
) -> Result<JsValue, JsValue> {
    checked_result(carve::to_markdown_with_report(
        source,
        checked_options(strict, maximum),
    ))
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = toPlainTextWithReport)]
pub fn to_plain_text_with_report(
    source: &str,
    strict: Option<bool>,
    maximum: Option<u32>,
) -> Result<JsValue, JsValue> {
    checked_result(carve::to_plain_text_with_report(
        source,
        checked_options(strict, maximum),
    ))
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = toAnsiWithReport)]
pub fn to_ansi_with_report(
    source: &str,
    strict: Option<bool>,
    maximum: Option<u32>,
) -> Result<JsValue, JsValue> {
    checked_result(carve::to_ansi_with_report(
        source,
        checked_options(strict, maximum),
    ))
}

#[cfg(feature = "reports")]
#[wasm_bindgen(js_name = toCarveWithReport)]
pub fn to_carve_with_report(
    source: &str,
    strict: Option<bool>,
    maximum: Option<u32>,
) -> Result<JsValue, JsValue> {
    checked_result(carve::to_carve_with_report(
        source,
        checked_options(strict, maximum),
    ))
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
    render_core(source, &symbol_pairs(symbols)?, &RenderConfig::default())
        .map_err(profile_violation_error)
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
    render_full(source, &symbol_pairs(symbols)?, &RenderConfig::default())
        .map_err(profile_violation_error)
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
#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = parseJson)]
pub fn parse_json(source: &str) -> String {
    let mut options = carve::Options::new();
    options.positions = true;
    carve::to_json(&carve::parse_with_options(source, &options))
}

#[cfg(feature = "html-import")]
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

#[cfg(feature = "html-import")]
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
                carve::HtmlImportDiagnosticCode::AttributePreserved => "attribute-preserved",
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
#[cfg(feature = "html-import")]
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

#[cfg(feature = "html-import")]
#[wasm_bindgen(js_name = fromHtml)]
pub fn from_html(source: &str, mode: Option<String>) -> Result<JsValue, JsValue> {
    html_to_carve(source, mode)
}

#[cfg(feature = "markdown-import")]
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

/// Turn a profile rejection into a JS `Error` a caller can act on.
///
/// The message is the engine's, and `violations` carries them one per entry so
/// a host can report which construct was refused without parsing prose. The
/// `name` is set so `error.name === 'ProfileViolationError'` works the way it
/// does in carve-js.
fn profile_violation_error(error: carve::ProfileViolationError) -> JsValue {
    let js_error = js_sys::Error::new(&error.to_string());
    js_error.set_name("ProfileViolationError");
    let violations = js_sys::Array::new();
    for violation in &error.violations {
        violations.push(&JsValue::from_str(&violation.message()));
    }
    // Best effort: a failed property set must not mask the rejection itself.
    let _ = js_sys::Reflect::set(
        &js_error,
        &JsValue::from_str("violations"),
        &violations.into(),
    );
    js_error.into()
}

/// Structural types for the entry points that return objects.
///
/// wasm-bindgen types a `JsValue` return as `any`, which hands a TypeScript
/// caller nothing. These are declared here and referenced by
/// `unchecked_return_type` on the functions below.
#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
export interface LintWarning {
  /** 1-based line number. */
  line: number;
  /** 1-based column number. */
  column: number;
  /** Stable rule id, shared with carve-js and carve-php. */
  rule: string;
  message: string;
  /** 0-based BYTE offset into the source, inclusive. */
  start: number;
  /** 0-based BYTE offset into the source, exclusive. */
  end: number;
}

export interface Stamp {
  /** The spec version the document was last processed under. */
  version: string;
  /** The engine that wrote the marker, when it recorded one. */
  generatedBy: string | null;
}
"#;

/// A thrown JS `Error`, not a thrown string.
///
/// `JsValue::from_str` throws the string itself, so `error.message` is
/// undefined in the catch block and a host's normal error handling misses it.
/// The older entry points in this file still do that; new ones do not.
fn js_error(message: String) -> JsValue {
    js_sys::Error::new(&message).into()
}

/// Render an AST-JSON document (PART 12) to HTML.
///
/// The other half of `parseJson`. A host that reads the tree in a browser does
/// it to CHANGE something, and until now there was no way to render the result:
/// the binding could serialize a tree out and not take one back.
///
/// Takes the same options object as [`to_html_with_options`], so an edited tree
/// renders under the profile, labels and switches the host already configured.
#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = astJsonToHtml)]
pub fn ast_json_to_html(json: &str, options: Option<js_sys::Object>) -> Result<String, JsValue> {
    let doc = carve::from_json(json)
        .map_err(|error| js_error(format!("carve: invalid AST JSON: {error:?}")))?;
    let Some(request) = RenderRequest::read(options)? else {
        return carve::render_html(&doc)
            .map_err(|error| js_error(format!("carve: render refused: {error:?}")));
    };
    let owned = request.extension_boxes();
    let engine_options = request.engine_options(&owned);
    // Through the engine's own preparation, not straight into the renderer.
    // `render_html_with_options` renders a tree AS GIVEN: it applies neither the
    // profile filter nor the `before_render` hooks, both of which live in this
    // step. Skipping it would make one options object mean two different things
    // - a profile that filters on the source path and does nothing here, and
    // extensions that transform there and are ignored here.
    let prepared =
        carve::prepare_document_for_render(doc, &engine_options, engine_options.mode, true)
            .map_err(profile_violation_error)?;
    carve::render_html_with_options(&prepared, &engine_options)
        .map_err(|error| js_error(format!("carve: render refused: {error:?}")))
}

/// Render an AST-JSON document (PART 12) back to canonical Carve source.
///
/// The round trip a host needs to SAVE an edited tree, rather than only display
/// it. A tree holding something no Carve source can spell is refused rather
/// than written approximately.
#[cfg(all(feature = "ast-json", feature = "other-renderers"))]
#[wasm_bindgen(js_name = astJsonToCarve)]
pub fn ast_json_to_carve(json: &str) -> Result<String, JsValue> {
    let doc = carve::from_json(json)
        .map_err(|error| js_error(format!("carve: invalid AST JSON: {error:?}")))?;
    carve::render_carve(&doc)
        .map_err(|error| js_error(format!("carve: cannot write this tree: {error:?}")))
}

/// Lint a document for the degradations PART 15 describes.
///
/// Returns an array of `{ line, column, rule, message, start, end }`. The rule
/// ids are shared with carve-js and carve-php, so the same trigger reports the
/// same id everywhere. Offsets are BYTE offsets into the source, matching the
/// engine.
///
/// Built as JS objects rather than a JSON string: a message carries arbitrary
/// document text, and hand-rolled JSON escaping is where that goes wrong.
#[cfg(feature = "lint")]
#[wasm_bindgen(js_name = lintCarve, unchecked_return_type = "LintWarning[]")]
pub fn lint_carve(source: &str) -> Result<JsValue, JsValue> {
    let warnings = js_sys::Array::new();
    for warning in carve::lint_carve(source) {
        let entry = js_sys::Object::new();
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("line"),
            &JsValue::from_f64(warning.line as f64),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("column"),
            &JsValue::from_f64(warning.column as f64),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("rule"),
            &JsValue::from_str(warning.rule),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("message"),
            &JsValue::from_str(&warning.message),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("start"),
            &JsValue::from_f64(warning.start as f64),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("end"),
            &JsValue::from_f64(warning.end as f64),
        )?;
        warnings.push(&entry.into());
    }
    Ok(warnings.into())
}

/// Read a document's provenance marker: `{ version, generatedBy }`, or `null`.
#[cfg(feature = "stamp")]
#[wasm_bindgen(js_name = readStamp, unchecked_return_type = "Stamp | null")]
pub fn read_stamp(source: &str) -> Result<JsValue, JsValue> {
    let Some(stamp) = carve::read_stamp(source) else {
        return Ok(JsValue::NULL);
    };
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("version"),
        &JsValue::from_str(&stamp.version),
    )?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("generatedBy"),
        &stamp
            .generated_by
            .as_deref()
            .map(JsValue::from_str)
            .unwrap_or(JsValue::NULL),
    )?;
    Ok(object.into())
}

/// Whether a document was last processed under an older spec version than
/// `currentVersion`. An unstamped document counts as needing review.
#[cfg(feature = "stamp")]
#[wasm_bindgen(js_name = needsReview)]
pub fn needs_review(source: &str, current_version: &str) -> bool {
    carve::needs_review(source, current_version)
}

/// Convert Djot source to Carve.
///
/// Carve diverges from Djot deliberately - the emphasis delimiters are swapped,
/// among others - so a Djot document is not Carve source and pasting one in
/// renders wrongly rather than failing.
#[cfg(feature = "other-imports")]
#[wasm_bindgen(js_name = fromDjot)]
pub fn from_djot(source: &str) -> String {
    carve::djot_to_carve(source)
}

/// Convert BBCode source to Carve.
///
/// Rejects input past the engine's `BBCODE_MAX_INPUT_LENGTH` rather than
/// working on it: the importer's cost is superlinear in places, and a browser
/// host cannot afford to find that out on the main thread.
#[cfg(feature = "other-imports")]
#[wasm_bindgen(js_name = fromBbcode)]
pub fn from_bbcode(source: &str) -> Result<String, JsValue> {
    carve::bbcode_to_carve(source)
        .map_err(|error| js_error(format!("carve: BBCode import failed: {error:?}")))
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
///
/// toHtmlWithOptions(untrusted, { rawHtml: false })
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
/// * `rawHtml` (default `true`) - render an explicit passthrough - the `=html`
///   raw block and the `` `…`{=html} `` inline raw span - as markup. `false`
///   emits it as escaped text instead, the same switch carve-js spells
///   `allowRawHtml`. A host that renders a document it did not author (a shared
///   link, a comment field, anything a reader supplies) wants `false`: without
///   it a passthrough is a way to run script on the host's origin.
/// * `profile` - one of `"full"`, `"article"`, `"comment"`, `"minimal"`. The
///   rest of the untrusted-input story: input length, denied constructs, link
///   policy. A document the profile REJECTS throws a `ProfileViolationError`
///   carrying `violations`, rather than resolving to an empty string.
/// * `profileBaseHost` - the host counted as internal when the profile's link
///   policy distinguishes internal from external links.
/// * `mode` - `"interactive"` (default) or `"static"`, the self-contained form
///   for print, PDF and archival: no client scripts.
/// * `sourceLine` (default `false`) - stamp top-level blocks with
///   `data-source-line`, for editor preview scroll-sync.
/// * `positions` (default `false`) - keep source offsets on the nodes.
/// * `labels` - override the engine-written strings (admonition names, the
///   endnotes heading, backlink text) for a page that is not in English. These
///   are TEXT and are escaped where they land, unlike `symbols`.
/// * `smartTypography` - `"glyph"` (default) resolves `...` to an ellipsis,
///   `"source"` keeps the author's run.
/// * `lowercaseHeadingIds` (default `false`) and `asciiHeadingIds`
///   (`"off"` (default), `"fold"`, `"strict"`) - the slug policy, for a host
///   whose anchors have to match another generator's.
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
    let Some(request) = RenderRequest::read(options)? else {
        return Ok(carve::to_html(source));
    };
    request.render(source).map_err(profile_violation_error)
}

/// One options object, parsed once.
///
/// Read as a whole rather than field by field at each entry point: a second
/// reader is a second place for a key to be spelled differently, or left out.
struct RenderRequest {
    config: RenderConfig,
    symbols: SymbolPairs,
    named: Option<Vec<String>>,
    full: bool,
}

impl RenderRequest {
    /// `None` when the caller passed nothing at all, which is the engine's own
    /// default render and takes its fast path.
    fn read(options: Option<js_sys::Object>) -> Result<Option<Self>, JsValue> {
        let Some(options) = options else {
            return Ok(None);
        };
        let value: JsValue = options.clone().into();
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }

        let config = RenderConfig {
            sections: bool_field(&options, "sections")?.unwrap_or(true),
            raw_html: bool_field(&options, "rawHtml")?.unwrap_or(true),
            source_lines: bool_field(&options, "sourceLine")?.unwrap_or(false),
            positions: bool_field(&options, "positions")?.unwrap_or(false),
            lowercase_heading_ids: bool_field(&options, "lowercaseHeadingIds")?.unwrap_or(false),
            ascii_heading_ids: ascii_heading_ids_field(&options)?,
            smart_typography: smart_typography_field(&options)?,
            mode: mode_field(&options)?,
            profile: profile_field(&options)?,
            profile_base_host: string_field(&options, "profileBaseHost")?,
            labels: string_map_field(&options, "labels")?,
        };
        let full = bool_field(&options, "full")?.unwrap_or(false);
        let named = extension_names_field(&options)?;
        // A wrong-typed `symbols` must THROW, not quietly render without
        // symbols: `dyn_into().ok()` would turn `{ symbols: "rocket" }` into
        // `None` and lose the caller's map with no signal. Absent / null /
        // undefined still mean "no symbols".
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

        Ok(Some(Self {
            config,
            symbols: symbol_pairs(symbols)?,
            named,
            full,
        }))
    }

    fn render(&self, source: &str) -> Result<String, carve::ProfileViolationError> {
        match (&self.named, self.full) {
            // An explicit list wins over the preview set: a caller who names
            // extensions has said exactly what they want.
            (Some(keys), _) => render_with_extensions(source, keys, &self.symbols, &self.config),
            (None, true) => render_full(source, &self.symbols, &self.config),
            (None, false) => render_core(source, &self.symbols, &self.config),
        }
    }

    /// The engine options this request describes, for an entry point that
    /// takes a TREE and so cannot go through the source-rendering helpers.
    fn engine_options<'a>(
        &'a self,
        owned: &'a [Box<dyn carve::CarveExtension>],
    ) -> carve::Options<'a> {
        let mut options = self.config.apply(carve::Options::new());
        for ext in owned {
            options = options.with_extension(ext.as_ref());
        }
        for (name, value) in &self.symbols {
            options = options.with_symbol(name.clone(), value.clone());
        }
        options
    }

    /// The extension boxes this request needs, owned by the caller's frame
    /// because `Options` borrows them.
    fn extension_boxes(&self) -> Vec<Box<dyn carve::CarveExtension>> {
        match (&self.named, self.full) {
            (Some(keys), _) => build_extensions(keys),
            (None, true) => build_extensions(
                &PREVIEW_EXTENSIONS
                    .iter()
                    .map(|k| (*k).to_string())
                    .collect::<Vec<_>>(),
            ),
            (None, false) => Vec::new(),
        }
    }
}

/// Read one string field out of a JS options object.
fn string_field(options: &js_sys::Object, key: &str) -> Result<Option<String>, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str(key))?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    value.as_string().map(Some).ok_or_else(|| {
        JsValue::from(js_sys::TypeError::new(&format!(
            "carve: `{key}` must be a string"
        )))
    })
}

/// Read a name-to-string map, in the order the caller wrote it.
///
/// Unlike `symbols`, these values are TEXT: the engine escapes a label where it
/// lands, so a host may feed this from a translation catalog.
fn string_map_field(options: &js_sys::Object, key: &str) -> Result<Vec<(String, String)>, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str(key))?;
    if value.is_undefined() || value.is_null() {
        return Ok(Vec::new());
    }
    let object = value.dyn_into::<js_sys::Object>().map_err(|_| {
        JsValue::from(js_sys::TypeError::new(&format!(
            "carve: `{key}` must be an object"
        )))
    })?;
    let mut pairs = Vec::new();
    for entry in js_sys::Object::entries(&object).iter() {
        let entry: js_sys::Array = entry.into();
        let name = entry.get(0).as_string().ok_or_else(|| {
            JsValue::from(js_sys::TypeError::new(&format!(
                "carve: every key in `{key}` must be a string"
            )))
        })?;
        let text = entry.get(1).as_string().ok_or_else(|| {
            JsValue::from(js_sys::TypeError::new(&format!(
                "carve: every value in `{key}` must be a string"
            )))
        })?;
        pairs.push((name, text));
    }
    Ok(pairs)
}

/// Read a string field and map it through `accept`, naming the alternatives in
/// the error the way the sibling bindings do.
fn enum_field<T>(
    options: &js_sys::Object,
    key: &str,
    accepted: &str,
    accept: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, JsValue> {
    let Some(name) = string_field(options, key)? else {
        return Ok(None);
    };
    accept(&name).map(Some).ok_or_else(|| {
        JsValue::from(js_sys::TypeError::new(&format!(
            "carve: unknown `{key}` {name:?} (supported: {accepted})"
        )))
    })
}

fn mode_field(options: &js_sys::Object) -> Result<carve::Mode, JsValue> {
    Ok(enum_field(
        options,
        "mode",
        "\"interactive\", \"static\"",
        |name| match name {
            "interactive" => Some(carve::Mode::Interactive),
            "static" => Some(carve::Mode::Static),
            _ => None,
        },
    )?
    .unwrap_or_default())
}

fn smart_typography_field(options: &js_sys::Object) -> Result<carve::SmartTypographyMode, JsValue> {
    Ok(enum_field(
        options,
        "smartTypography",
        "\"glyph\", \"source\"",
        |name| match name {
            "glyph" => Some(carve::SmartTypographyMode::Glyph),
            "source" => Some(carve::SmartTypographyMode::Source),
            _ => None,
        },
    )?
    .unwrap_or_default())
}

fn ascii_heading_ids_field(options: &js_sys::Object) -> Result<carve::AsciiHeadingIds, JsValue> {
    Ok(enum_field(
        options,
        "asciiHeadingIds",
        "\"off\", \"fold\", \"strict\"",
        |name| match name {
            "off" => Some(carve::AsciiHeadingIds::Off),
            "fold" => Some(carve::AsciiHeadingIds::Fold),
            "strict" => Some(carve::AsciiHeadingIds::Strict),
            _ => None,
        },
    )?
    .unwrap_or_default())
}

/// Read the `profile` option: one of the engine's four presets, by name.
///
/// Named rather than constructed, matching carve-rb. A profile assembled field
/// by field across the wasm boundary would be a second way to spell a security
/// posture, and the presets are what the spec and the other bindings describe.
fn profile_field(options: &js_sys::Object) -> Result<Option<carve::Profile>, JsValue> {
    enum_field(
        options,
        "profile",
        "\"full\", \"article\", \"comment\", \"minimal\"",
        |name| match name {
            "full" => Some(carve::Profile::full()),
            "article" => Some(carve::Profile::article()),
            "comment" => Some(carve::Profile::comment()),
            "minimal" => Some(carve::Profile::minimal()),
            _ => None,
        },
    )
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
        extensions, render_core, render_full, render_with_extensions, RenderConfig, SymbolPairs,
        PREVIEW_EXTENSIONS,
    };

    /// Sections off, everything else at its default.
    fn no_sections() -> RenderConfig {
        RenderConfig {
            sections: false,
            ..RenderConfig::default()
        }
    }

    #[cfg(feature = "html-import")]
    use super::html_import_report_json;

    /// Build the lowered symbol map the JS bridge produces (the `js_sys`
    /// conversion itself only runs inside a JS host).
    fn symbols(pairs: &[(&str, &str)]) -> SymbolPairs {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[cfg(feature = "html-import")]
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
            render_core("# A\n\np\n", &none, &RenderConfig::default()).unwrap(),
            "<section id=\"A\">\n  <h1>A</h1>\n  <p>p</p>\n</section>"
        );
        assert_eq!(
            render_core("# A\n\np\n", &none, &no_sections()).unwrap(),
            "<h1 id=\"A\">A</h1>\n<p>p</p>"
        );
    }

    #[test]
    fn sections_off_leaves_container_headings_alone() {
        let none = SymbolPairs::new();
        let src = "> # Quoted\n>\n> Quoted body.\n";
        assert_eq!(
            render_core(src, &none, &no_sections()).unwrap(),
            render_core(src, &none, &RenderConfig::default()).unwrap()
        );
    }

    // The flag composes with the other two axes rather than being exclusive
    // with them, which the symbols-empty fast path inside render_core makes
    // easy to get wrong.
    #[test]
    fn sections_off_composes_with_symbols_and_extensions() {
        let map = symbols(&[("rocket", "🚀")]);
        let core = render_core("# A\n\n:rocket:\n", &map, &no_sections()).unwrap();
        assert!(core.starts_with("<h1 id=\"A\">A</h1>"), "{core}");
        assert!(core.contains('🚀'), "{core}");
        assert!(!core.contains("<section"), "{core}");

        let full = render_full("# A\n\n:rocket:\n", &map, &no_sections()).unwrap();
        assert!(full.contains('🚀'), "{full}");
        assert!(!full.contains("<section"), "{full}");
    }

    // A passthrough is the one construct that can put author-controlled markup
    // on the host's origin, so the switch has to reach BOTH spellings - the
    // block and the inline span - and it has to survive the extension and
    // symbol paths, which build their options separately.
    #[test]
    fn raw_html_off_escapes_both_passthrough_spellings() {
        let none = SymbolPairs::new();
        let src = "```=html\n<img src=x onerror=alert(1)>\n```\n\nan `<b>x</b>`{=html} span\n";
        let config = RenderConfig {
            raw_html: false,
            ..RenderConfig::default()
        };

        let on = render_core(src, &none, &RenderConfig::default()).unwrap();
        assert!(on.contains("<img src=x onerror=alert(1)>"), "{on}");
        assert!(on.contains("<b>x</b>"), "{on}");

        let off = render_core(src, &none, &config).unwrap();
        assert!(off.contains("&lt;img src=x onerror=alert(1)&gt;"), "{off}");
        assert!(!off.contains("<img src=x"), "{off}");
        assert!(off.contains("&lt;b&gt;x&lt;/b&gt;"), "{off}");
    }

    #[test]
    fn raw_html_off_composes_with_sections_symbols_and_extensions() {
        let map = symbols(&[("rocket", "\u{1f680}")]);
        let src = "# A\n\n:rocket:\n\n```=html\n<b>raw</b>\n```\n";
        let config = RenderConfig {
            sections: false,
            raw_html: false,
            ..RenderConfig::default()
        };

        let core = render_core(src, &map, &config).unwrap();
        assert!(core.contains('\u{1f680}'), "{core}");
        assert!(!core.contains("<section"), "{core}");
        assert!(core.contains("&lt;b&gt;raw&lt;/b&gt;"), "{core}");

        let full = render_full(src, &map, &config).unwrap();
        assert!(full.contains("&lt;b&gt;raw&lt;/b&gt;"), "{full}");
    }

    // The symbols map keeps its TRUSTED-RAW contract either way: `rawHtml` is
    // about the document's passthrough, not about what the host configured.
    #[test]
    fn raw_html_off_leaves_the_symbol_map_trusted() {
        let map = symbols(&[("bold", "<b>x</b>")]);
        let config = RenderConfig {
            raw_html: false,
            ..RenderConfig::default()
        };
        let html = render_core(":bold:", &map, &config).unwrap();
        assert!(html.contains("<b>x</b>"), "{html}");
    }

    // The profile is the other half of rendering a document from a stranger:
    // `rawHtml: false` stops a passthrough, the profile caps size and denies
    // constructs. The helper has to REPORT a rejection - the infallible engine
    // entry point turns one into an empty string, which a caller cannot tell
    // from a document that rendered to nothing.
    #[test]
    fn a_profile_rejection_is_an_error_not_an_empty_string() {
        let config = RenderConfig {
            profile: Some(carve::Profile::minimal()),
            ..RenderConfig::default()
        };
        let src = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let rejected = render_core(src, &SymbolPairs::new(), &config);
        match rejected {
            Err(error) => assert!(!error.violations.is_empty()),
            Ok(html) => assert!(
                !html.is_empty(),
                "a rejection must not reach the caller as an empty string"
            ),
        }
    }

    #[test]
    fn a_profile_renders_what_it_allows() {
        let config = RenderConfig {
            profile: Some(carve::Profile::full()),
            ..RenderConfig::default()
        };
        let html = render_core("# A\n\np\n", &SymbolPairs::new(), &config).unwrap();
        assert!(html.contains("<h1"), "{html}");
    }

    #[test]
    fn source_lines_and_positions_are_opt_in() {
        let none = SymbolPairs::new();
        let src = "# A\n\np\n";
        assert!(!render_core(src, &none, &RenderConfig::default())
            .unwrap()
            .contains("data-source-line"));
        let config = RenderConfig {
            source_lines: true,
            ..RenderConfig::default()
        };
        assert!(render_core(src, &none, &config)
            .unwrap()
            .contains("data-source-line"));
    }

    // The `labels` map is the engine's i18n seam (PART 9 §16a): admonition
    // names, the endnotes heading, backlink text. A non-English page renders
    // English furniture without it.
    #[test]
    fn labels_replace_the_generated_string() {
        let none = SymbolPairs::new();
        let src = "::: note\nbody\n:::\n";
        let english = render_core(src, &none, &RenderConfig::default()).unwrap();
        assert!(english.contains("Note"), "{english}");

        let config = RenderConfig {
            labels: vec![("admonitionNote".to_string(), "Hinweis".to_string())],
            ..RenderConfig::default()
        };
        let german = render_core(src, &none, &config).unwrap();
        assert!(german.contains("Hinweis"), "{german}");
    }

    #[test]
    fn smart_typography_source_keeps_the_authors_run() {
        let none = SymbolPairs::new();
        let config = RenderConfig {
            smart_typography: carve::SmartTypographyMode::Source,
            ..RenderConfig::default()
        };
        let src = "a...b\n";
        let glyph = render_core(src, &none, &RenderConfig::default()).unwrap();
        let source = render_core(src, &none, &config).unwrap();
        assert_ne!(glyph, source, "{glyph} vs {source}");
        assert!(source.contains("a...b"), "{source}");
    }

    #[test]
    fn the_heading_id_policy_is_configurable() {
        let none = SymbolPairs::new();
        let src = "# Grüße Alle\n";
        let plain = render_core(src, &none, &RenderConfig::default()).unwrap();
        assert!(plain.contains("Grüße"), "{plain}");

        let config = RenderConfig {
            lowercase_heading_ids: true,
            ascii_heading_ids: carve::AsciiHeadingIds::Strict,
            ..RenderConfig::default()
        };
        let folded = render_core(src, &none, &config).unwrap();
        assert!(folded.contains("id=\"grusse-alle\""), "{folded}");
    }

    #[test]
    fn renders_html() {
        assert!(crate::to_html("# Hello").contains("<h1>Hello</h1>"));
    }

    #[cfg(feature = "other-renderers")]
    #[test]
    fn marker_attributes_do_not_move_the_content_column() {
        let source = "-{title=\"😀\"} [x] a\n  # h\n";
        let html = crate::to_html(source);
        assert!(html.contains("<h1 id=\"h\">h</h1>"), "{html}");
        let canonical = crate::to_carve(source);
        assert_eq!(canonical, "-{title=😀} [x] a\n  # h\n");
        assert_eq!(crate::to_html(&canonical), html);
    }

    #[cfg(feature = "other-renderers")]
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
            &RenderConfig::default(),
        )
        .unwrap();
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
        let html = render_full(src, &SymbolPairs::new(), &RenderConfig::default()).unwrap();
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
        let html =
            render_with_extensions(src, &keys, &SymbolPairs::new(), &RenderConfig::default())
                .unwrap();
        assert!(html.contains("class=\"permalink\""), "got: {html}");
    }

    #[test]
    fn an_unnamed_render_is_unaffected_by_the_registry() {
        // Core stays core: reading the registry must not enable anything.
        let src = "# Heading\n";
        let core = render_core(src, &SymbolPairs::new(), &RenderConfig::default()).unwrap();
        assert!(!core.contains("class=\"permalink\""), "got: {core}");
    }

    #[test]
    fn full_enables_code_callouts_extension() {
        let src = "``` rust\nlet x = 1; // <1>\n```\n\n<1> Assign x.\n";
        let html = render_full(src, &SymbolPairs::new(), &RenderConfig::default()).unwrap();
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

        let html = render_core("Ship it :rocket:", &map, &RenderConfig::default()).unwrap();
        assert!(
            html.contains("Ship it 🚀"),
            "expected the mapped value, got: {html}"
        );
        assert!(
            !html.contains(":rocket:"),
            "a mapped name must not stay literal, got: {html}"
        );

        // The same map flows through the extensions-on entry point.
        let html = render_full("Ship it :rocket:", &map, &RenderConfig::default()).unwrap();
        assert!(
            html.contains("🚀"),
            "expected the mapped value, got: {html}"
        );
    }

    #[test]
    fn plus_one_is_a_valid_symbol_name() {
        let html = render_core(
            "nice :+1:",
            &symbols(&[("+1", "👍")]),
            &RenderConfig::default(),
        )
        .unwrap();
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
            &RenderConfig::default(),
        )
        .unwrap();
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
        let html = render_core(
            "a:b:c and 10:30: and me@example.com",
            &map,
            &RenderConfig::default(),
        )
        .unwrap();
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
        let html = render_core(
            ":bold:",
            &symbols(&[("bold", "<b>x</b>")]),
            &RenderConfig::default(),
        )
        .unwrap();
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
