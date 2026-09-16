use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

#[cfg(feature = "source-patches")]
mod source_patch;

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
    /// The host's own URL templates for `@mention` and `#tag`. Absent, both
    /// render as inert spans; present, the token name is percent-encoded into
    /// `{name}` (`{user}` too, for a mention) and the result is sanitized.
    /// Trusted configuration, not document content.
    mention_url: Option<String>,
    tag_url: Option<String>,
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
            mention_url: None,
            tag_url: None,
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
        if let Some(template) = &self.mention_url {
            options = options.with_mention_url(template.clone());
        }
        if let Some(template) = &self.tag_url {
            options = options.with_tag_url(template.clone());
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

/// One of the engine's fallible `try_to_*_with_options` entry points.
///
/// The five targets differ only in which of these is called, so the options an
/// entry point was handed are assembled once and the target is a parameter.
type TargetRender = fn(&str, &carve::Options<'_>) -> Result<String, carve::ProfileViolationError>;

/// Render to one target with the named extensions plus the given symbol map.
fn render_target(
    source: &str,
    keys: &[String],
    symbols: &SymbolPairs,
    config: &RenderConfig,
    render: TargetRender,
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
    render(source, &options)
}

/// Render to HTML with the named extensions plus the given symbol map.
fn render_with_extensions(
    source: &str,
    keys: &[String],
    symbols: &SymbolPairs,
    config: &RenderConfig,
) -> Result<String, carve::ProfileViolationError> {
    render_target(
        source,
        keys,
        symbols,
        config,
        carve::try_to_html_with_options,
    )
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

#[cfg(feature = "source-patches")]
fn edit_kind(kind: &str) -> Result<source_patch::SourceEditKind, JsValue> {
    match kind {
        "formatting" => Ok(source_patch::SourceEditKind::Formatting),
        "syntax-migration" => Ok(source_patch::SourceEditKind::SyntaxMigration),
        "quick-fix" => Ok(source_patch::SourceEditKind::QuickFix),
        "refactor" => Ok(source_patch::SourceEditKind::Refactor),
        _ => Err(js_sys::TypeError::new("carve: unknown source patch edit kind").into()),
    }
}

#[cfg(feature = "source-patches")]
/// Build the smallest single UTF-8 byte-range replacement between two sources.
#[wasm_bindgen(js_name = createSourcePatch, unchecked_return_type = "SourcePatch")]
pub fn create_source_patch(
    source: &str,
    replacement: &str,
    kind: &str,
    code: &str,
) -> Result<JsValue, JsValue> {
    if code.is_empty() {
        return Err(js_sys::TypeError::new("carve: source patch code must not be empty").into());
    }
    serde_wasm_bindgen::to_value(&source_patch::create(
        source,
        replacement,
        edit_kind(kind)?,
        code,
    ))
    .map_err(|error| js_error(format!("carve: cannot create source patch: {error}")))
}

#[cfg(feature = "source-patches")]
/// Preview canonical formatting as a source-preserving patch.
#[wasm_bindgen(js_name = toCarvePatch, unchecked_return_type = "SourcePatch")]
pub fn to_carve_patch(source: &str) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&source_patch::create(
        source,
        &carve::to_carve(source),
        source_patch::SourceEditKind::Formatting,
        "canonical-format",
    ))
    .map_err(|error| js_error(format!("carve: cannot create formatting patch: {error}")))
}

#[cfg(feature = "source-patches")]
#[wasm_bindgen(js_name = applySourcePatch)]
/// Apply a trusted patch after its source length and fingerprint still match.
pub fn apply_source_patch(source: &str, patch: JsValue) -> Result<String, JsValue> {
    let patch =
        serde_wasm_bindgen::from_value::<source_patch::SourcePatch>(patch).map_err(|error| {
            js_sys::TypeError::new(&format!("carve: invalid source patch: {error}"))
        })?;
    source_patch::apply(source, &patch)
        .map_err(|error| js_error(format!("carve: cannot apply source patch: {error}")))
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

/// Serialize the tree with the same options object as
/// [`to_html_with_options`], so a host can export the tree of an untrusted
/// document under the `profile` it renders that document with.
///
/// `positions` is not read: every serialized tree this binding produces carries
/// them, as in [`parse_json`].
#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = parseJsonWithOptions)]
pub fn parse_json_with_options(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    let Some(mut request) = RenderRequest::read(options)? else {
        return Ok(parse_json(source));
    };
    request.config.positions = true;
    render_target(
        source,
        &request.extension_keys(),
        &request.symbols,
        &request.config,
        carve::try_to_json_with_options,
    )
    .map_err(profile_violation_error)
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

/// Import HTML through the Rust HTML5 DOM and canonical Carve writer.
///
/// Returns `{ value, report }`; `report.diagnostics` makes every lossy import
/// decision observable. `roundtrip` is only safe for Carve-produced HTML.
#[cfg(feature = "html-import")]
#[wasm_bindgen(js_name = htmlToCarve, unchecked_return_type = "MigrationResult")]
pub fn html_to_carve(source: &str, mode: Option<String>) -> Result<JsValue, JsValue> {
    let options = carve::HtmlImportOptions {
        mode: html_import_mode(mode)?,
        ..Default::default()
    };
    let result = carve::migrate_html(source, &options)
        .map_err(|error| JsValue::from_str(&format!("carve: HTML import failed: {error:?}")))?;
    migration_result_to_js(result)
}

/// Import HTML straight to the tree, as AST JSON.
///
/// `htmlToCarve` writes Carve SOURCE, so a host that wanted the tree had to
/// re-parse what it had just written - two parses of one document to get one
/// answer. This is the same importer without that round trip.
///
/// ```js
/// const { value, report } = htmlToAst('<p>Hello <b>world</b></p>', 'safe')
/// astJsonToHtml(value)
/// ```
///
/// Returns the SAME `{ value, report }` shape `htmlToCarve` does, with `value`
/// holding the tree instead of the source, in the same fidelity vocabulary. A
/// third result shape for one importer is what that avoids.
///
/// The REPORT IS NOT ALWAYS IDENTICAL, and the difference is the point. A loss
/// only a WRITER takes is not reported here, because no writer ran (PART 12
/// §16): a `<figure>` wrapping a table, or a table with an explicit
/// head/body/foot grouping, has no Carve spelling, so `htmlToCarve` reports
/// `structure-unspellable` and this does not - the tree keeps the thing that
/// would have been lost.
///
/// `mode` is spelled as it is there: `safe` (the default), `semantic`, and
/// trusted-only `roundtrip`.
#[cfg(all(feature = "html-import", feature = "ast-json"))]
#[wasm_bindgen(js_name = htmlToAst, unchecked_return_type = "MigrationResult")]
pub fn html_to_ast(source: &str, mode: Option<String>) -> Result<JsValue, JsValue> {
    let options = carve::HtmlImportOptions {
        mode: html_import_mode(mode)?,
        ..Default::default()
    };
    let result = carve::html_to_ast(source, &options)
        .map_err(|error| js_error(format!("carve: HTML import failed: {error:?}")))?;
    // Through the same adapter `migrate_html` uses on the source result, so the
    // report is built once rather than described twice.
    migration_result_to_js(carve::MigrationResult {
        value: carve::to_json(&result.value),
        report: carve::MigrationReport {
            schema_version: 2,
            source_format: carve::SourceFormat::Html,
            mode: Some(result.report.mode),
            adapter: Some(result.report.adapter),
            diagnostics: result
                .report
                .diagnostics
                .into_iter()
                .map(|diagnostic| carve::MigrationDiagnostic {
                    code: diagnostic.code.as_str().to_owned(),
                    message: diagnostic.message,
                    severity: diagnostic.severity,
                    fidelity: diagnostic.fidelity,
                    confidence: diagnostic.confidence,
                    path: diagnostic.path,
                })
                .collect(),
        },
    })
}

#[cfg(any(
    feature = "html-import",
    feature = "markdown-import",
    feature = "other-imports"
))]
fn migration_result_to_js(result: carve::MigrationResult) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("value"),
        &JsValue::from_str(&result.value),
    )?;
    let report = js_sys::Object::new();
    js_sys::Reflect::set(
        &report,
        &JsValue::from_str("schemaVersion"),
        &JsValue::from_f64(result.report.schema_version as f64),
    )?;
    js_sys::Reflect::set(
        &report,
        &JsValue::from_str("sourceFormat"),
        &JsValue::from_str(result.report.source_format.as_str()),
    )?;
    if let Some(mode) = result.report.mode {
        js_sys::Reflect::set(
            &report,
            &JsValue::from_str("mode"),
            &JsValue::from_str(mode.as_str()),
        )?;
    }
    if let Some(adapter) = result.report.adapter {
        js_sys::Reflect::set(
            &report,
            &JsValue::from_str("adapter"),
            &JsValue::from_str(adapter.as_str()),
        )?;
    }
    let diagnostics = js_sys::Array::new();
    for item in result.report.diagnostics {
        let diagnostic = js_sys::Object::new();
        for (key, value) in [
            ("code", item.code.as_str()),
            ("message", item.message.as_str()),
            ("severity", item.severity.as_str()),
            ("fidelity", item.fidelity.as_str()),
            ("confidence", item.confidence.as_str()),
        ] {
            js_sys::Reflect::set(
                &diagnostic,
                &JsValue::from_str(key),
                &JsValue::from_str(value),
            )?;
        }
        if let Some(path) = item.path {
            js_sys::Reflect::set(
                &diagnostic,
                &JsValue::from_str("path"),
                &JsValue::from_str(&path),
            )?;
        }
        diagnostics.push(&diagnostic);
    }
    js_sys::Reflect::set(&report, &JsValue::from_str("diagnostics"), &diagnostics)?;
    js_sys::Reflect::set(&object, &JsValue::from_str("report"), &report)?;
    Ok(object.into())
}

#[cfg(feature = "html-import")]
#[wasm_bindgen(js_name = fromHtml, unchecked_return_type = "MigrationResult")]
pub fn from_html(source: &str, mode: Option<String>) -> Result<JsValue, JsValue> {
    html_to_carve(source, mode)
}

#[cfg(feature = "markdown-import")]
#[wasm_bindgen(js_name = fromMarkdown, unchecked_return_type = "MigrationResult")]
pub fn from_markdown(source: &str) -> Result<JsValue, JsValue> {
    migration_result_to_js(carve::migrate_markdown(source))
}

/// Import Markdown straight to the tree, as AST JSON.
///
/// `fromMarkdown` writes Carve SOURCE, so a host that wanted the tree had to
/// re-parse what it had just written. This is the same importer without that
/// round trip. The losses `fromMarkdown` reports are not repeated here; a host
/// that needs them calls that one.
#[cfg(all(feature = "markdown-import", feature = "ast-json"))]
#[wasm_bindgen(js_name = markdownToAstJson)]
pub fn markdown_to_ast_json(source: &str) -> String {
    carve::to_json(&carve::markdown_to_ast(source))
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

export interface ProfileViolation {
  /** Canonical node type that was disallowed. */
  nodeType: string;
  /** Machine reason: element_not_allowed | max_nesting_exceeded | link_not_allowed | image_not_allowed. */
  reason: string;
  /** The profile's own wording for the denial, when it carries one. */
  reasonDescription: string | null;
  /** The engine's prose, the same text a thrown ProfileViolationError lists. */
  message: string;
}
export interface ProfileFilterResult {
  /** The filtered tree, as AST JSON. */
  json: string;
  violations: ProfileViolation[];
}

export interface Stamp {
  /** The spec version the document was last processed under. */
  version: string;
  /** The engine that wrote the marker, when it recorded one. */
  generatedBy: string | null;
}

export type MigrationFidelity = "preserved" | "normalized" | "degraded" | "dropped";
export type MigrationConfidence = "exact" | "inferred" | "fallback";
export interface MigrationDiagnostic {
  code: string;
  message: string;
  severity: "info" | "warning" | "error";
  fidelity: MigrationFidelity;
  confidence: MigrationConfidence;
  path?: string;
}
export interface MigrationReport {
  schemaVersion: 2;
  sourceFormat: string;
  mode?: string;
  adapter?: string;
  diagnostics: MigrationDiagnostic[];
}
export interface MigrationResult { value: string; report: MigrationReport; }

export type SourceEditKind = "formatting" | "syntax-migration" | "quick-fix" | "refactor";
export interface SourceEdit {
  /** Inclusive UTF-8 byte offset. */
  start: number;
  /** Exclusive UTF-8 byte offset. */
  end: number;
  replacement: string;
  kind: SourceEditKind;
  code: string;
}
export interface SourceSuggestion extends SourceEdit { message: string; }
export interface SourcePatch {
  version: 1;
  sourceFingerprint: string;
  sourceBytes: number;
  edits: SourceEdit[];
  unresolved: SourceSuggestion[];
}

export interface StaticRendererError {
  /** The renderer that failed. Only "math" is bound. */
  renderer: "math";
  /** True for display math. */
  display: boolean;
  /** The source the renderer was handed. */
  source: string;
  message: string;
}
export interface StaticRenderResult {
  html: string;
  /** Empty unless a render callback threw or returned a non-string. */
  rendererErrors: StaticRendererError[];
}

export interface AccessibilityDiagnostic {
  /** Stable rule id, e.g. a11y/image-alt. */
  rule: string;
  severity: "warning" | "error";
  message: string;
  /** 0-based BYTE offset into the source, or null when the node carried none. */
  startOffset: number | null;
  endOffset: number | null;
}

export interface SanitizeSvgOptions {
  allowStyle?: boolean;
  allowLinks?: boolean;
  allowAnimation?: boolean;
  allowExternalImages?: boolean;
}
export interface SanitizeResult {
  /** Meaningful only when ok is true. */
  svg: string;
  /** False when the input was not a single well-formed <svg> root. */
  ok: boolean;
}

export interface ParsedLocator {
  label: string | null;
  value: string | null;
  suffixText: string | null;
}

export interface ProseMirrorResult {
  /** The ProseMirror document, JSON-encoded. */
  json: string;
  /** Carve node type -> why its content is gone. */
  dropped: Record<string, string>;
  /** Carve node type -> why its node type is gone while its text survives. */
  degraded: Record<string, string>;
}
"#;

/// A thrown JS `Error`, not a thrown string.
///
/// `JsValue::from_str` throws the string itself, so `error.message` is
/// undefined in the catch block and a host's normal error handling misses it.
/// The older entry points in this file still do that; new ones do not.
///
/// Every caller sits behind a feature gate, so this list is the union of
/// theirs, and a new gated entry point that throws has to widen it. Forgetting
/// fails the build for that selection rather than degrading quietly. CI lints
/// the two selections that ship - default, and the rendering-only
/// `--no-default-features` the docs Playground is built from.
#[cfg(any(
    feature = "ast-json",
    feature = "other-imports",
    feature = "prosemirror",
    feature = "source-patches"
))]
fn js_error(message: String) -> JsValue {
    js_sys::Error::new(&message).into()
}

/// The PART 12 §13 source-layout sidecar for a document.
///
/// Separate from `parseJson`, which carries semantic positions on the nodes.
/// This is the byte-exact record of the SOURCE: its line endings, whether it
/// had a BOM, and a path-addressed byte span per node - what a host needs to
/// write an edit back into the file it came from.
#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = parseSourceLayoutJson)]
pub fn parse_source_layout_json(source: &str) -> String {
    carve::parse_with_source_layout(source).1
}

/// Expand `{{ path }}` include directives (spec PART 9 §19) through a JS
/// resolver, and hand back the expanded tree as AST JSON.
///
/// The resolver MUST BE SYNCHRONOUS, which decides who can use this. A browser
/// host that would resolve a path by `fetch` cannot: `fetch` is a Promise and
/// wasm-bindgen cannot await across the call. A host whose files are already in
/// memory - an editor with its open buffers, a bundler, a VFS, a test harness -
/// resolves from that map and is exactly who this is for.
///
/// ```js
/// const files = new Map([['child.crv', 'Included body.\n']])
/// expandIncludes('Before.\n\n{{ child.crv }}\n', {
///   resolve: (path) => files.get(path) ?? null,
/// })
/// // { json, warnings: [], suppressedWarnings: 0, dependencies: […],
/// //   chargedBytes: 15, resolverErrors: [] }
/// ```
///
/// The resolver returns the child's source as a string, or
/// `{ source, id }` when it can name the file canonically - the id is what the
/// cycle guard compares, so two spellings of one file defeat it without one.
/// `null` refuses the directive as `not-found`; `{ denial }` refuses it in one
/// of the classes §19 names.
///
/// An `async` resolver returns a Promise, which is not a string and cannot be
/// awaited. That is a per-document failure reported in `resolverErrors`,
/// matching what an `async` `renderers.math` gets from
/// [`to_html_with_renderers`]: the directive stays literal and the caller is
/// told why, rather than the value being swallowed or stringified.
///
/// The BUDGETS are part of the contract, not a detail: `maxBytes` defaults to
/// `max(1 MB, 8 x source bytes)`, `maxDepth` to 16, `maxResolverCalls` to 1000
/// and `maxWarnings` to 100. They bound what a document of directives can make
/// a host do, and lowering them is how a host serving untrusted documents keeps
/// that bounded.
///
/// The tree carries no positions. Expansion merges nodes from several files,
/// and a span on a node that came from a child would point into a source the
/// caller did not pass.
#[cfg(feature = "includes")]
#[wasm_bindgen(js_name = expandIncludes, unchecked_return_type = "IncludeExpansion")]
pub fn expand_includes(source: &str, options: js_sys::Object) -> Result<JsValue, JsValue> {
    let request = IncludeRequest::read(&options)?;
    let failures: Rc<RefCell<Vec<ResolverFailure>>> = Rc::default();
    let resolver = js_resolver(request.resolve.clone(), Rc::clone(&failures));

    let owned = build_extensions(&request.extensions);
    let mut include_options = carve::IncludeOptions::new().with_resolver(&resolver);
    for ext in &owned {
        include_options = include_options.with_extension(ext.as_ref());
    }
    if let Some(path) = &request.source_path {
        include_options = include_options.with_source_path(path.clone());
    }
    if let Some(depth) = request.max_depth {
        include_options = include_options.with_max_depth(depth);
    }
    if let Some(bytes) = request.max_bytes {
        include_options = include_options.with_max_bytes(bytes);
    }
    if let Some(calls) = request.max_resolver_calls {
        include_options = include_options.with_max_resolver_calls(calls);
    }
    if let Some(warnings) = request.max_warnings {
        include_options = include_options.with_max_warnings(warnings);
    }

    let mut parse_options = carve::Options::new();
    for ext in &owned {
        parse_options = parse_options.with_extension(ext.as_ref());
    }
    let doc = carve::parse_with_options(source, &parse_options);
    let result = carve::expand_includes(doc, source, &include_options);

    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &"json".into(),
        &JsValue::from_str(&carve::to_json(&result.doc)),
    )?;
    let warnings = js_sys::Array::new();
    for warning in &result.warnings {
        let item = js_sys::Object::new();
        js_sys::Reflect::set(&item, &"rule".into(), &warning.rule.as_str().into())?;
        js_sys::Reflect::set(&item, &"message".into(), &warning.message.as_str().into())?;
        js_sys::Reflect::set(
            &item,
            &"file".into(),
            &match &warning.file {
                Some(file) => JsValue::from_str(file),
                None => JsValue::NULL,
            },
        )?;
        warnings.push(&item);
    }
    js_sys::Reflect::set(&object, &"warnings".into(), &warnings)?;
    js_sys::Reflect::set(
        &object,
        &"suppressedWarnings".into(),
        &(result.suppressed_warnings as f64).into(),
    )?;
    let dependencies = js_sys::Array::new();
    for dependency in &result.dependencies {
        let item = js_sys::Object::new();
        js_sys::Reflect::set(&item, &"id".into(), &dependency.id.as_str().into())?;
        js_sys::Reflect::set(&item, &"resolved".into(), &dependency.resolved.into())?;
        js_sys::Reflect::set(
            &item,
            &"denial".into(),
            &match dependency.denial {
                Some(denial) => JsValue::from_str(denial.as_str()),
                None => JsValue::NULL,
            },
        )?;
        dependencies.push(&item);
    }
    js_sys::Reflect::set(&object, &"dependencies".into(), &dependencies)?;
    js_sys::Reflect::set(
        &object,
        &"chargedBytes".into(),
        &(result.charged_bytes as f64).into(),
    )?;
    let errors = js_sys::Array::new();
    for failure in failures.borrow().iter() {
        let item = js_sys::Object::new();
        js_sys::Reflect::set(&item, &"path".into(), &failure.path.as_str().into())?;
        js_sys::Reflect::set(&item, &"message".into(), &failure.message.as_str().into())?;
        errors.push(&item);
    }
    js_sys::Reflect::set(&object, &"resolverErrors".into(), &errors)?;
    Ok(object.into())
}

/// One resolver call the binding could not turn into an answer.
///
/// Shared by `expandIncludes` and `mergeAst`: both hand a conflict or a path to
/// a host callback that may return something unusable, and both report it
/// rather than throwing.
#[cfg(any(feature = "includes", feature = "ast-merge"))]
struct ResolverFailure {
    path: String,
    message: String,
}

/// The `expandIncludes` options object, parsed once.
#[cfg(feature = "includes")]
struct IncludeRequest {
    resolve: js_sys::Function,
    source_path: Option<String>,
    extensions: Vec<String>,
    max_depth: Option<usize>,
    max_bytes: Option<usize>,
    max_resolver_calls: Option<usize>,
    max_warnings: Option<usize>,
}

#[cfg(feature = "includes")]
impl IncludeRequest {
    fn read(options: &js_sys::Object) -> Result<Self, JsValue> {
        let resolve = js_sys::Reflect::get(options, &JsValue::from_str("resolve"))?;
        // Required rather than optional. With no resolver the engine's pass is
        // a no-op that leaves every directive literal, and an entry point whose
        // zero-config form silently does nothing is a trap.
        let resolve = resolve.dyn_into::<js_sys::Function>().map_err(|_| {
            type_error(
                "carve: `resolve` must be a function `(path, ctx) => source`. Without one every \
                 directive stays literal, so there would be nothing to expand",
            )
        })?;
        Ok(Self {
            resolve,
            source_path: string_field(options, "sourcePath")?,
            extensions: extension_names_field(options)?.unwrap_or_default(),
            max_depth: size_field(options, "maxDepth")?,
            max_bytes: size_field(options, "maxBytes")?,
            max_resolver_calls: size_field(options, "maxResolverCalls")?,
            max_warnings: size_field(options, "maxWarnings")?,
        })
    }
}

/// Read a non-negative integer budget.
#[cfg(feature = "includes")]
fn size_field(options: &js_sys::Object, key: &str) -> Result<Option<usize>, JsValue> {
    let value = js_sys::Reflect::get(options, &JsValue::from_str(key))?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    let number = value.as_f64().filter(|n| n.is_finite() && *n >= 0.0);
    number
        .map(|n| n as usize)
        .map(Some)
        .ok_or_else(|| type_error(&format!("carve: `{key}` must be a non-negative number")))
}

/// The engine's resolver, over a JS callback.
#[cfg(feature = "includes")]
fn js_resolver(
    resolve: js_sys::Function,
    failures: Rc<RefCell<Vec<ResolverFailure>>>,
) -> impl Fn(&str, &carve::IncludeContext<'_>) -> Result<carve::IncludeResolved, carve::IncludeDenial>
{
    move |path: &str, ctx: &carve::IncludeContext<'_>| {
        let record = |message: String| {
            failures.borrow_mut().push(ResolverFailure {
                path: path.to_string(),
                message,
            });
            carve::IncludeDenial::Unresolved
        };
        let context = match include_context(ctx) {
            Ok(context) => context,
            Err(error) => return Err(record(format!("`resolve` context: {error:?}"))),
        };
        let value = match resolve.call2(&JsValue::NULL, &JsValue::from_str(path), &context) {
            Ok(value) => value,
            Err(error) => {
                return Err(record(format!(
                    "`resolve` threw: {}",
                    describe_throw(&error)
                )))
            }
        };
        resolved_from_js(value).map_err(|message| match message {
            // A refusal the host meant, not a failure of the binding.
            Ok(denial) => denial,
            Err(message) => record(message),
        })
    }
}

/// The `ctx` argument one resolver call gets.
#[cfg(feature = "includes")]
fn include_context(ctx: &carve::IncludeContext<'_>) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &"sourcePath".into(),
        &match ctx.source_path {
            Some(path) => JsValue::from_str(path),
            None => JsValue::NULL,
        },
    )?;
    let stack = js_sys::Array::new();
    for entry in ctx.stack {
        stack.push(&JsValue::from_str(entry));
    }
    js_sys::Reflect::set(&object, &"stack".into(), &stack)?;
    js_sys::Reflect::set(&object, &"depth".into(), &(ctx.depth as f64).into())?;
    Ok(object.into())
}

/// What one resolver call returned.
///
/// `Err(Ok(denial))` is a refusal the host asked for; `Err(Err(message))` is a
/// value the binding could not read, which is reported as well as refused.
#[cfg(feature = "includes")]
#[allow(clippy::result_large_err, clippy::type_complexity)]
fn resolved_from_js(
    value: JsValue,
) -> Result<carve::IncludeResolved, Result<carve::IncludeDenial, String>> {
    if value.is_undefined() || value.is_null() {
        return Err(Ok(carve::IncludeDenial::NotFound));
    }
    if let Some(source) = value.as_string() {
        return Ok(carve::IncludeResolved::from(source));
    }
    // Named before the object branch. A Promise IS an object, so it would
    // otherwise be reported as one missing a `source` key - and an `async`
    // resolver is the mistake this reporting exists for.
    if value.is_instance_of::<js_sys::Promise>() {
        return Err(Err(format!(
            "`resolve` returned {}",
            describe_value(&value)
        )));
    }
    let Some(object) = value.dyn_ref::<js_sys::Object>() else {
        return Err(Err(format!(
            "`resolve` returned {}, not a string, an object or null",
            describe_value(&value)
        )));
    };
    let denial = js_sys::Reflect::get(object, &JsValue::from_str("denial"))
        .ok()
        .and_then(|d| d.as_string());
    if let Some(denial) = denial {
        return match denial.as_str() {
            "outside-root" => Err(Ok(carve::IncludeDenial::OutsideRoot)),
            "not-found" => Err(Ok(carve::IncludeDenial::NotFound)),
            "no-root" => Err(Ok(carve::IncludeDenial::NoRoot)),
            "include-denied" => Err(Ok(carve::IncludeDenial::Denied)),
            "include-unresolved" => Err(Ok(carve::IncludeDenial::Unresolved)),
            other => Err(Err(format!(
                "`resolve` returned an unknown denial {other:?} (supported: \"outside-root\", \
                 \"not-found\", \"no-root\", \"include-denied\", \"include-unresolved\")"
            ))),
        };
    }
    let source = js_sys::Reflect::get(object, &JsValue::from_str("source"))
        .ok()
        .and_then(|s| s.as_string());
    let Some(source) = source else {
        return Err(Err(format!(
            "`resolve` returned {} with no `source` string and no `denial`",
            describe_value(&value)
        )));
    };
    let id = js_sys::Reflect::get(object, &JsValue::from_str("id"))
        .ok()
        .and_then(|i| i.as_string());
    Ok(match id {
        Some(id) => carve::IncludeResolved::with_id(source, id),
        None => carve::IncludeResolved::from(source),
    })
}

/// Parse a document and keep what a later [`reparse`] needs.
///
/// Returns one JSON object: `{ source, document, sourceLayout, changedSource,
/// reusedPreviousTree }`. `document` is the PART 12 tree, `sourceLayout` the
/// byte-exact record [`parse_source_layout_json`] produces, and
/// `changedSource` the byte ranges this parse covered - the whole document, on
/// a first parse.
///
/// The snapshot crosses as JSON rather than as a handle a host has to `free()`.
/// Every other entry point here is a pure function that owns nothing, and the
/// engine's own snapshot holds only the source, so there is no tree being kept
/// alive in wasm memory for a handle to point at.
#[cfg(all(feature = "ast-json", feature = "incremental"))]
#[wasm_bindgen(js_name = parseSnapshot)]
pub fn parse_snapshot(source: &str) -> String {
    incremental_json(&carve::parse_snapshot(source), source)
}

/// Re-parse `source` with `changes` applied.
///
/// ```js
/// const first = JSON.parse(parseSnapshot('# One\n'))
/// const next = JSON.parse(reparse(first.source, '[{"range":[2,5],"replacement":"Two"}]'))
/// ```
///
/// `changes` is a JSON array of `{ range: [start, end], replacement }`.
///
/// THE OFFSETS ARE UTF-8 BYTE OFFSETS, which is what `parseJson` positions and
/// `createSourcePatch` ranges already mean. A browser editor counts UTF-16 code
/// units, so a host holding a `selectionStart` converts before calling - an
/// offset that lands inside a multi-byte character is refused rather than
/// guessed at.
///
/// A malformed change THROWS: overlapping ranges, an end past the source, and a
/// range that splits a code point are all a broken caller contract, not a
/// result the caller asked for.
///
/// `reusedPreviousTree` says whether the parse reused any of the previous one.
/// The pinned engine always reports `false` - it validates and applies the
/// edits and then parses the whole source - so a host should read this as the
/// engine's own answer rather than assume work was saved.
#[cfg(all(feature = "ast-json", feature = "incremental"))]
#[wasm_bindgen(js_name = reparse)]
pub fn reparse(source: &str, changes: &str) -> Result<String, JsValue> {
    let changes = text_changes_from_json(changes)?;
    let previous = carve::parse_snapshot(source);
    let result = carve::reparse(previous.snapshot, &changes)
        .map_err(|error| type_error(&format!("carve: {error}")))?;
    let applied = result.snapshot.source().to_string();
    Ok(incremental_json(&result, &applied))
}

/// The one JSON object both incremental entry points return.
#[cfg(all(feature = "ast-json", feature = "incremental"))]
fn incremental_json(parse: &carve::IncrementalParse, source: &str) -> String {
    let changed: Vec<serde_json::Value> = parse
        .changed_source
        .iter()
        .map(|range| serde_json::json!([range.start, range.end]))
        .collect();
    // The two engine strings are already JSON, so they are spliced in as values
    // rather than re-encoded as strings a caller would have to parse twice.
    let document: serde_json::Value =
        serde_json::from_str(&carve::to_json(&parse.document)).unwrap_or(serde_json::Value::Null);
    let layout: serde_json::Value =
        serde_json::from_str(&parse.source_layout_json).unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "source": source,
        "document": document,
        "sourceLayout": layout,
        "changedSource": changed,
        "reusedPreviousTree": parse.reused_previous_tree,
    })
    .to_string()
}

/// Read the `changes` argument: a JSON array of `{ range: [start, end],
/// replacement }`.
#[cfg(all(feature = "ast-json", feature = "incremental"))]
fn text_changes_from_json(input: &str) -> Result<Vec<carve::TextChange>, JsValue> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|error| type_error(&format!("carve: `changes` is not JSON: {error}")))?;
    let array = value
        .as_array()
        .ok_or_else(|| type_error("carve: `changes` must be a JSON array"))?;
    let mut changes = Vec::with_capacity(array.len());
    for (index, entry) in array.iter().enumerate() {
        let at = |what: &str| type_error(&format!("carve: `changes[{index}]` {what}"));
        let range = entry
            .get("range")
            .and_then(|r| r.as_array())
            .filter(|r| r.len() == 2)
            .ok_or_else(|| at("needs a `range` of two byte offsets"))?;
        let offset = |slot: usize| {
            range[slot]
                .as_u64()
                .map(|n| n as usize)
                .ok_or_else(|| at("range offsets must be non-negative integers"))
        };
        let replacement = entry
            .get("replacement")
            .and_then(|r| r.as_str())
            .ok_or_else(|| at("needs a `replacement` string"))?;
        changes.push(carve::TextChange {
            range: offset(0)?..offset(1)?,
            replacement: replacement.to_string(),
        });
    }
    Ok(changes)
}

/// The difference between two PART 12 trees, as patch JSON.
///
/// ```js
/// const patch = createAstPatch(parseJson('# One\n'), parseJson('# Two\n'))
/// applyAstPatch(parseJson('# One\n'), patch) // the '# Two' tree
/// ```
///
/// Trees and patches both cross as JSON strings, matching `parseJson` and
/// `astJsonToHtml`. A patch carries node payloads, so it carries arbitrary
/// document text - the reason `parseJson` chose a string in the first place.
///
/// The engine's `ast_patch_to_json` and `ast_patch_from_json` are not bound
/// separately. With the patch crossing as JSON they ARE the encoding of this
/// pair, and a host calling them would be converting JSON it already holds.
///
/// The operations are position-independent `{ op, path, value }`, so a patch
/// stays meaningful against a tree that moved underneath it - unlike a source
/// patch, which `createSourcePatch` fingerprints against staleness.
#[cfg(feature = "ast-patches")]
#[wasm_bindgen(js_name = createAstPatch)]
pub fn create_ast_patch(before: &str, after: &str) -> Result<String, JsValue> {
    let before = ast_from_json(before, "before")?;
    let after = ast_from_json(after, "after")?;
    let operations = carve::create_ast_patch(&before, &after).map_err(ast_patch_error)?;
    carve::ast_patch_to_json(&operations).map_err(ast_patch_error)
}

/// Replay patch JSON onto a PART 12 tree, returning the result as AST JSON.
#[cfg(feature = "ast-patches")]
#[wasm_bindgen(js_name = applyAstPatch)]
pub fn apply_ast_patch(ast: &str, patch: &str) -> Result<String, JsValue> {
    let document = ast_from_json(ast, "ast")?;
    let operations = carve::ast_patch_from_json(patch).map_err(ast_patch_error)?;
    let patched = carve::apply_ast_patch(&document, &operations).map_err(ast_patch_error)?;
    Ok(carve::to_json(&patched))
}

/// The same difference with its inverse and both fingerprints, as one JSON
/// object: `{ forward, inverse, beforeFingerprint, afterFingerprint }`.
///
/// This is the shape an undo step wants. THE STACK IS THE HOST'S: every entry
/// point in this package is a pure function that owns nothing, so there is no
/// history kept here to undo against, and a browser host already has one - a
/// key handler, a toolbar, an editor's own history plugin.
///
/// What the pair adds over two `createAstPatch` calls is the precondition.
/// [`apply_reversible_ast_patch`] refuses a tree whose fingerprint is not the
/// one the patch was made against, so an undo cannot be replayed onto a
/// document that has moved on.
#[cfg(feature = "ast-patches")]
#[wasm_bindgen(js_name = createReversibleAstPatch)]
pub fn create_reversible_ast_patch(before: &str, after: &str) -> Result<String, JsValue> {
    let before = ast_from_json(before, "before")?;
    let after = ast_from_json(after, "after")?;
    let patch = carve::create_reversible_ast_patch(&before, &after).map_err(ast_patch_error)?;
    let forward = carve::ast_patch_to_json(&patch.forward).map_err(ast_patch_error)?;
    let inverse = carve::ast_patch_to_json(&patch.inverse).map_err(ast_patch_error)?;
    let encode = |json: &str| -> serde_json::Value {
        serde_json::from_str(json).unwrap_or(serde_json::Value::Null)
    };
    Ok(serde_json::json!({
        "forward": encode(&forward),
        "inverse": encode(&inverse),
        "beforeFingerprint": patch.before_fingerprint,
        "afterFingerprint": patch.after_fingerprint,
    })
    .to_string())
}

/// Replay a reversible patch, forward or inverted.
///
/// The fingerprint is checked first: a tree that is not the one this patch was
/// made against is refused rather than half-patched. That is a broken caller
/// contract, so it throws.
#[cfg(feature = "ast-patches")]
#[wasm_bindgen(js_name = applyReversibleAstPatch)]
pub fn apply_reversible_ast_patch(
    ast: &str,
    patch: &str,
    inverse: Option<bool>,
) -> Result<String, JsValue> {
    let document = ast_from_json(ast, "ast")?;
    let patch = reversible_patch_from_json(patch)?;
    let patched = carve::apply_reversible_ast_patch(&document, &patch, inverse.unwrap_or(false))
        .map_err(ast_patch_error)?;
    Ok(carve::to_json(&patched))
}

/// Read one AST-JSON argument, naming which one when it is bad.
#[cfg(feature = "ast-patches")]
fn ast_from_json(json: &str, which: &str) -> Result<carve::Document, JsValue> {
    carve::from_json(json)
        .map_err(|error| js_error(format!("carve: invalid `{which}` AST JSON: {error:?}")))
}

#[cfg(feature = "ast-patches")]
fn ast_patch_error(error: carve::AstPatchError) -> JsValue {
    js_error(format!("carve: {error}"))
}

/// Read `{ forward, inverse, beforeFingerprint, afterFingerprint }` back.
#[cfg(feature = "ast-patches")]
fn reversible_patch_from_json(json: &str) -> Result<carve::ReversibleAstPatch, JsValue> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| type_error(&format!("carve: `patch` is not JSON: {error}")))?;
    let side = |key: &str| -> Result<Vec<carve::AstPatchOperation>, JsValue> {
        let operations = value
            .get(key)
            .ok_or_else(|| type_error(&format!("carve: `patch` needs a `{key}` operation list")))?;
        carve::ast_patch_from_json(&operations.to_string()).map_err(ast_patch_error)
    };
    let fingerprint = |key: &str| -> Result<String, JsValue> {
        value
            .get(key)
            .and_then(|f| f.as_str())
            .map(str::to_string)
            .ok_or_else(|| type_error(&format!("carve: `patch` needs a `{key}` string")))
    };
    Ok(carve::ReversibleAstPatch {
        forward: side("forward")?,
        inverse: side("inverse")?,
        before_fingerprint: fingerprint("beforeFingerprint")?,
        after_fingerprint: fingerprint("afterFingerprint")?,
    })
}

/// Three-way merge over PART 12 trees, with conflicts as a VALUE.
///
/// ```js
/// const { ok, ast, conflicts } = JSON.parse(mergeAst(base, ours, theirs))
/// ```
///
/// Returns `{ ok, ast, conflicts, resolverErrors }` as one JSON string. A
/// conflict does NOT throw: it is a result the caller asked for, and two people
/// editing one document is the ordinary case rather than a broken contract.
/// `ok` is false, `ast` is null, and `conflicts` says where and why.
///
/// The `{ ok, ast, conflicts }` shape and the three `reason` names -
/// `both-changed`, `delete-edit`, `concurrent-sequence-edit` - are carve-js's,
/// so a host merging with either engine reads one contract. The Rust engine
/// carries no `deleted` flags on a conflict, so that optional carve-js field is
/// absent here rather than guessed at.
///
/// `options.resolve` is an optional `(conflict) => 'base' | 'ours' | 'theirs' |
/// { value } | null`, answering conflicts while the merge runs; an answer of
/// `null` leaves one unresolved. IT MUST BE SYNCHRONOUS. A resolver that asks a
/// server, or asks the user, returns a Promise the merge cannot await - that is
/// reported in `resolverErrors` and the conflict is left unresolved, the same
/// treatment an `async` `renderers.math` gets.
#[cfg(feature = "ast-merge")]
#[wasm_bindgen(js_name = mergeAst)]
pub fn merge_ast(
    base: &str,
    ours: &str,
    theirs: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    let base = ast_from_json(base, "base")?;
    let ours = ast_from_json(ours, "ours")?;
    let theirs = ast_from_json(theirs, "theirs")?;
    let resolve = merge_resolver_field(options.as_ref())?;
    let failures: Rc<RefCell<Vec<ResolverFailure>>> = Rc::default();

    let merge_error =
        |error: carve::AstJsonError| js_error(format!("carve: merge failed: {error:?}"));
    let result = match &resolve {
        None => carve::merge_ast(&base, &ours, &theirs).map_err(merge_error)?,
        Some(resolve) => {
            let failures = Rc::clone(&failures);
            carve::merge_ast_with_resolver(&base, &ours, &theirs, |conflict| {
                js_resolution(resolve, conflict, &failures)
            })
            .map_err(merge_error)?
        }
    };

    let (ok, ast, conflicts) = match result {
        carve::MergeResult::Merged(document) => (
            true,
            serde_json::from_str(&carve::to_json(&document)).unwrap_or(serde_json::Value::Null),
            Vec::new(),
        ),
        carve::MergeResult::Conflicts(conflicts) => (
            false,
            serde_json::Value::Null,
            conflicts.iter().map(conflict_to_json).collect(),
        ),
    };
    let errors: Vec<serde_json::Value> = failures
        .borrow()
        .iter()
        .map(|failure| serde_json::json!({ "path": failure.path, "message": failure.message }))
        .collect();
    Ok(serde_json::json!({
        "ok": ok,
        "ast": ast,
        "conflicts": conflicts,
        "resolverErrors": errors,
    })
    .to_string())
}

/// One conflict, in carve-js's field names.
#[cfg(feature = "ast-merge")]
fn conflict_to_json(conflict: &carve::MergeConflict) -> serde_json::Value {
    // Each side is a JSON-encoded value or absent, and an absent side is `null`
    // - a field one side deleted has no value to report.
    let side = |value: &Option<String>| -> serde_json::Value {
        value
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or(serde_json::Value::Null)
    };
    serde_json::json!({
        "path": conflict.path,
        "reason": match conflict.reason {
            carve::MergeConflictReason::BothChanged => "both-changed",
            carve::MergeConflictReason::DeleteEdit => "delete-edit",
            carve::MergeConflictReason::ConcurrentSequenceEdit => "concurrent-sequence-edit",
        },
        "base": side(&conflict.base),
        "ours": side(&conflict.ours),
        "theirs": side(&conflict.theirs),
    })
}

/// Read `options.resolve`.
#[cfg(feature = "ast-merge")]
fn merge_resolver_field(
    options: Option<&js_sys::Object>,
) -> Result<Option<js_sys::Function>, JsValue> {
    let Some(options) = options else {
        return Ok(None);
    };
    let value = js_sys::Reflect::get(options, &JsValue::from_str("resolve"))?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    Ok(Some(value.dyn_into::<js_sys::Function>().map_err(
        |_| type_error("carve: `resolve` must be a function `(conflict) => resolution`"),
    )?))
}

/// Ask the JS resolver about one conflict.
#[cfg(feature = "ast-merge")]
fn js_resolution(
    resolve: &js_sys::Function,
    conflict: &carve::MergeConflict,
    failures: &Rc<RefCell<Vec<ResolverFailure>>>,
) -> Option<carve::MergeResolution> {
    let record = |message: String| {
        failures.borrow_mut().push(ResolverFailure {
            path: conflict.path.clone(),
            message,
        });
        None
    };
    let argument = match js_sys::JSON::parse(&conflict_to_json(conflict).to_string()) {
        Ok(value) => value,
        Err(error) => return record(format!("`resolve` argument: {}", describe_throw(&error))),
    };
    let value = match resolve.call1(&JsValue::NULL, &argument) {
        Ok(value) => value,
        Err(error) => return record(format!("`resolve` threw: {}", describe_throw(&error))),
    };
    if value.is_undefined() || value.is_null() {
        return None;
    }
    if let Some(name) = value.as_string() {
        return match name.as_str() {
            "base" => Some(carve::MergeResolution::Base),
            "ours" => Some(carve::MergeResolution::Ours),
            "theirs" => Some(carve::MergeResolution::Theirs),
            other => record(format!(
                "`resolve` returned {other:?} (supported: \"base\", \"ours\", \"theirs\", \
                 {{ value }}, null)"
            )),
        };
    }
    // Named before the object branch: a Promise IS an object, and an `async`
    // resolver is the mistake this reporting exists for.
    if value.is_instance_of::<js_sys::Promise>() {
        return record(format!("`resolve` returned {}", describe_value(&value)));
    }
    let Some(object) = value.dyn_ref::<js_sys::Object>() else {
        return record(format!(
            "`resolve` returned {}, not a side name, a {{ value }} or null",
            describe_value(&value)
        ));
    };
    let replacement = js_sys::Reflect::get(object, &JsValue::from_str("value")).ok();
    let Some(replacement) = replacement.filter(|v| !v.is_undefined()) else {
        return record("`resolve` returned an object with no `value`".to_string());
    };
    match js_sys::JSON::stringify(&replacement) {
        Ok(encoded) => Some(carve::MergeResolution::Value(String::from(encoded))),
        Err(error) => record(format!(
            "`resolve` returned a `value` that is not JSON: {}",
            describe_throw(&error)
        )),
    }
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

/// The tree-taking half of `toMarkdown` and its two siblings.
///
/// `plain` is the engine's options-free renderer, `render` the one that reads
/// them, mirroring the pair [`render_with_options`] holds for the source-taking
/// entry points.
#[cfg(all(feature = "ast-json", feature = "other-renderers"))]
fn ast_json_to_text(
    json: &str,
    options: Option<js_sys::Object>,
    plain: fn(&carve::Document) -> Result<String, carve::RenderDepthError>,
    render: fn(&carve::Document, &carve::Options<'_>) -> Result<String, carve::RenderDepthError>,
) -> Result<String, JsValue> {
    let doc = carve::from_json(json)
        .map_err(|error| js_error(format!("carve: invalid AST JSON: {error:?}")))?;
    let Some(request) = RenderRequest::read(options)? else {
        return plain(&doc).map_err(|error| js_error(format!("carve: render refused: {error:?}")));
    };
    let owned = request.extension_boxes();
    let engine_options = request.engine_options(&owned);
    // `Mode::Interactive` rather than the requested mode: static rendering is
    // HTML-only, and the engine forces the same value in
    // `try_to_markdown_with_options`. Passing `mode` through would run the
    // static hooks for a target whose renderer never sees them.
    let prepared =
        carve::prepare_document_for_render(doc, &engine_options, carve::Mode::Interactive, false)
            .map_err(profile_violation_error)?;
    render(&prepared, &engine_options)
        .map_err(|error| js_error(format!("carve: render refused: {error:?}")))
}

/// Render an AST-JSON document (PART 12) to Markdown.
///
/// The direct call for a host that already holds a tree. Without it Markdown was
/// reached through `astJsonToCarve` and then `toMarkdown`: a canonical write, a
/// re-parse and a second render for one answer, and lossier than this, because a
/// tree holding something no Carve source can spell is refused by the write
/// rather than rendered.
///
/// Takes the same options object as [`to_markdown_with_options`], and reads the
/// same narrow part of it. The profile filter and the `before_render` hooks run,
/// as they do in [`ast_json_to_html`].
///
/// ORDERING FOLLOWS THE TREE. `toMarkdown` parses with positions on, because
/// section 7 orders collected definitions by source position. A tree arriving as
/// JSON carries whatever positions its producer put there, and one carrying none
/// prints its footnote and link definitions in label order. That is the only
/// order available to a tree without spans, so it is reported here rather than
/// repaired.
#[cfg(all(feature = "ast-json", feature = "other-renderers"))]
#[wasm_bindgen(js_name = astJsonToMarkdown)]
pub fn ast_json_to_markdown(
    json: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    ast_json_to_text(
        json,
        options,
        carve::render_markdown,
        carve::render_markdown_with_options,
    )
}

/// Render an AST-JSON document (PART 12) to plain text. See
/// [`ast_json_to_markdown`], including what a tree without positions costs.
#[cfg(all(feature = "ast-json", feature = "other-renderers"))]
#[wasm_bindgen(js_name = astJsonToPlainText)]
pub fn ast_json_to_plain_text(
    json: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    ast_json_to_text(
        json,
        options,
        carve::render_plain_text,
        carve::render_plain_text_with_options,
    )
}

/// Render an AST-JSON document (PART 12) to ANSI-styled text. See
/// [`ast_json_to_markdown`], including what a tree without positions costs.
#[cfg(all(feature = "ast-json", feature = "other-renderers"))]
#[wasm_bindgen(js_name = astJsonToAnsi)]
pub fn ast_json_to_ansi(json: &str, options: Option<js_sys::Object>) -> Result<String, JsValue> {
    ast_json_to_text(
        json,
        options,
        carve::render_ansi,
        carve::render_ansi_with_options,
    )
}

/// Apply a security profile to an AST-JSON document (PART 12), keeping the
/// filtered tree.
///
/// The `profile` render option filters on the way to HTML and throws the result
/// of the filtering away. This is the same pass with the tree kept, so a host
/// that stores, diffs or re-renders an untrusted document does not have to
/// render HTML and parse it back to get one.
///
/// `violations` reports what the filter degraded or stripped, which the HTML
/// path cannot: a profile whose action is `error` throws instead, so a
/// resolved call under `to-text` or `strip` is the only place this is visible.
///
/// Only `profileBaseHost` and `smartTypography` are read from `options`. The
/// rest of the render options describe a renderer, and this is not one.
///
/// The profile's `max_length` is NOT enforced here: the engine applies it to the
/// source bytes before a parse, and this entry point is handed a tree.
#[cfg(feature = "ast-json")]
#[wasm_bindgen(js_name = applyProfile, unchecked_return_type = "ProfileFilterResult")]
pub fn apply_profile(
    json: &str,
    profile: &str,
    options: Option<js_sys::Object>,
) -> Result<JsValue, JsValue> {
    let doc = carve::from_json(json)
        .map_err(|error| js_error(format!("carve: invalid AST JSON: {error:?}")))?;
    let profile = named_profile(profile)?;

    let mut base_host = None;
    let mut smart = carve::SmartTypographyMode::default();
    if let Some(object) = options {
        let value: JsValue = object.clone().into();
        if !value.is_null() && !value.is_undefined() {
            base_host = string_field(&object, "profileBaseHost")?;
            smart = smart_typography_field(&object)?;
        }
    }

    let filtered = carve::apply_profile_with_typography(doc, &profile, base_host.as_deref(), smart)
        .map_err(profile_violation_error)?;

    let violations = js_sys::Array::new();
    for violation in &filtered.violations {
        let entry = js_sys::Object::new();
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("nodeType"),
            &JsValue::from_str(&violation.node_type),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("reason"),
            &JsValue::from_str(&violation.reason),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("reasonDescription"),
            &violation
                .reason_description
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
        )?;
        // The engine's own formatting, so this reads identically to an entry in
        // a thrown `ProfileViolationError`.
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("message"),
            &JsValue::from_str(&violation.message()),
        )?;
        violations.push(&entry.into());
    }

    let result = js_sys::Object::new();
    js_sys::Reflect::set(
        &result,
        &JsValue::from_str("json"),
        &JsValue::from_str(&carve::to_json(&filtered.doc)),
    )?;
    js_sys::Reflect::set(
        &result,
        &JsValue::from_str("violations"),
        &violations.into(),
    )?;
    Ok(result.into())
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
    lint_warnings(carve::lint_carve(source))
}

/// The JS array both lint entry points return.
#[cfg(feature = "lint")]
fn lint_warnings(found: Vec<carve::LintWarning>) -> Result<JsValue, JsValue> {
    let warnings = js_sys::Array::new();
    for warning in found {
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

/// Lint a document with the same options object the render entry points take.
///
/// `lintCarve` is the option-less form. Which degradations exist depends on the
/// extensions in play, so a host that renders with a set has to lint with it.
#[cfg(feature = "lint")]
#[wasm_bindgen(js_name = lintCarveWithOptions, unchecked_return_type = "LintWarning[]")]
pub fn lint_carve_with_options(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<JsValue, JsValue> {
    let Some(request) = RenderRequest::read(options)? else {
        return lint_carve(source);
    };
    let owned = request.extension_boxes();
    let engine_options = request.engine_options(&owned);
    lint_warnings(carve::lint_carve_with_options(source, &engine_options))
}

/// The accessibility diagnostics, a second family beside `lintCarve`.
///
/// Its own rule ids (`a11y/image-alt`, `a11y/heading-jump`) and its own
/// severity, which `lintCarve` has no field for. Offsets are `null` when the
/// node that triggered the rule carried no position.
#[cfg(feature = "lint")]
#[wasm_bindgen(js_name = lintAccessibility, unchecked_return_type = "AccessibilityDiagnostic[]")]
pub fn lint_accessibility(source: &str) -> Result<JsValue, JsValue> {
    let diagnostics = js_sys::Array::new();
    for diagnostic in carve::lint_accessibility(source) {
        let entry = js_sys::Object::new();
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("rule"),
            &JsValue::from_str(diagnostic.rule),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("severity"),
            &JsValue::from_str(match diagnostic.severity {
                carve::AccessibilitySeverity::Warning => "warning",
                carve::AccessibilitySeverity::Error => "error",
            }),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("message"),
            &JsValue::from_str(&diagnostic.message),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("startOffset"),
            &offset_or_null(diagnostic.start_offset),
        )?;
        js_sys::Reflect::set(
            &entry,
            &JsValue::from_str("endOffset"),
            &offset_or_null(diagnostic.end_offset),
        )?;
        diagnostics.push(&entry.into());
    }
    Ok(diagnostics.into())
}

/// An absent offset is `null`, not `0`: a node with no position and a node at
/// the start of the document are different answers. Defensive at this pin -
/// `lint_accessibility` parses with positions forced on, so nothing reaches the
/// `None` arm today.
#[cfg(feature = "lint")]
fn offset_or_null(offset: Option<usize>) -> JsValue {
    match offset {
        Some(value) => JsValue::from_f64(value as f64),
        None => JsValue::NULL,
    }
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

/// Write a provenance marker onto already-formatted Carve.
///
/// The other half of `readStamp` and `needsReview`, which could read a marker
/// this package had no way to write. `generatedBy` is the engine identity a
/// host wants recorded, e.g. `"my-app 1.2"`; `form` is `"line"` (default) or
/// `"block"`. Same signature as carve-js `stampCarve`.
///
/// The input is expected to be formatted already: this appends the marker and
/// replaces one the document carries, it does not canonicalize. `toCarve` is
/// the formatter.
#[cfg(feature = "stamp")]
#[wasm_bindgen(js_name = stampCarve)]
pub fn stamp_carve(
    formatted: &str,
    generated_by: &str,
    form: Option<String>,
) -> Result<String, JsValue> {
    let form = match form.as_deref() {
        None | Some("line") => carve::StampForm::Line,
        Some("block") => carve::StampForm::Block,
        Some(other) => {
            return Err(type_error(&format!(
                "carve: `form` must be \"line\" or \"block\", got {other:?}"
            )))
        }
    };
    Ok(carve::stamp_carve(formatted, generated_by, form))
}

/// Sanitize an SVG document, the helper a host embedding SVG would otherwise
/// reimplement.
///
/// `{ svg, ok }`. `ok` false means the input was not a single well-formed
/// `<svg>` root, and a caller must then show the SOURCE rather than the input:
/// `svg` carries nothing to display. Every option defaults to `false`, which is
/// the strict setting.
#[wasm_bindgen(js_name = sanitizeSvg, unchecked_return_type = "SanitizeResult")]
pub fn sanitize_svg(source: &str, options: Option<js_sys::Object>) -> Result<JsValue, JsValue> {
    let mut opts = carve::SanitizeSvgOptions::default();
    if let Some(options) = options {
        let value: JsValue = options.clone().into();
        if !value.is_null() && !value.is_undefined() {
            opts.allow_style = bool_field(&options, "allowStyle")?.unwrap_or(false);
            opts.allow_links = bool_field(&options, "allowLinks")?.unwrap_or(false);
            opts.allow_animation = bool_field(&options, "allowAnimation")?.unwrap_or(false);
            opts.allow_external_images =
                bool_field(&options, "allowExternalImages")?.unwrap_or(false);
        }
    }
    let result = carve::sanitize_svg(source, &opts);
    let out = js_sys::Object::new();
    js_sys::Reflect::set(
        &out,
        &JsValue::from_str("svg"),
        &JsValue::from_str(&result.svg),
    )?;
    js_sys::Reflect::set(
        &out,
        &JsValue::from_str("ok"),
        &JsValue::from_bool(result.ok),
    )?;
    Ok(out.into())
}

/// Parse the locator portion of a citation into `{ label, value, suffixText }`.
///
/// A field the source did not carry is `null`. Same shape as carve-js
/// `parseLocator`; pure, and never throws on bad input.
#[wasm_bindgen(js_name = parseLocator, unchecked_return_type = "ParsedLocator")]
pub fn parse_locator(loc: &str) -> Result<JsValue, JsValue> {
    let parsed = carve::parse_locator(loc);
    let out = js_sys::Object::new();
    for (key, value) in [
        ("label", parsed.label),
        ("value", parsed.value),
        ("suffixText", parsed.suffix_text),
    ] {
        let value = match value {
            Some(text) => JsValue::from_str(&text),
            None => JsValue::NULL,
        };
        js_sys::Reflect::set(&out, &JsValue::from_str(key), &value)?;
    }
    Ok(out.into())
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

/// Convert Djot and retain a conservative v2 fidelity report.
#[cfg(feature = "other-imports")]
#[wasm_bindgen(js_name = migrateDjot, unchecked_return_type = "MigrationResult")]
pub fn migrate_djot(source: &str) -> Result<JsValue, JsValue> {
    migration_result_to_js(carve::migrate_djot(source))
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

/// Convert BBCode and retain a conservative v2 fidelity report.
#[cfg(feature = "other-imports")]
#[wasm_bindgen(js_name = migrateBbcode, unchecked_return_type = "MigrationResult")]
pub fn migrate_bbcode(source: &str) -> Result<JsValue, JsValue> {
    migration_result_to_js(
        carve::migrate_bbcode(source)
            .map_err(|error| js_error(format!("carve: BBCode import failed: {error:?}")))?,
    )
}

/// Lower a `type -> reason` map into a plain JS object.
#[cfg(feature = "prosemirror")]
fn reason_map(entries: &std::collections::BTreeMap<String, String>) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    for (node_type, reason) in entries {
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str(node_type),
            &JsValue::from_str(reason),
        )?;
    }
    Ok(object.into())
}

/// Convert Carve source to the ProseMirror document shape.
///
/// ProseMirror runs in a browser and nowhere else, so this is the one engine
/// capability whose whole audience sits behind a WASM binding. Without it a host
/// wiring a Carve editor either round-trips to a server or reimplements the node
/// mapping in JS, where it drifts from the engine's.
///
/// Returns `{ json, dropped, degraded }`. `json` is the ProseMirror document as
/// a JSON string, the same choice `parseJson` makes and for the same reason.
/// The two maps say what the ProseMirror model could not hold: `dropped` where
/// the content is gone, `degraded` where the text survives without its node
/// type. A silent conversion is the degradation shape `lintCarve` exists to
/// avoid.
#[cfg(feature = "prosemirror")]
#[wasm_bindgen(js_name = toProseMirror, unchecked_return_type = "ProseMirrorResult")]
pub fn to_prose_mirror(source: &str) -> Result<JsValue, JsValue> {
    let converted = carve::to_prosemirror(&carve::parse(source));
    let result = js_sys::Object::new();
    js_sys::Reflect::set(
        &result,
        &JsValue::from_str("json"),
        &JsValue::from_str(&converted.json),
    )?;
    js_sys::Reflect::set(
        &result,
        &JsValue::from_str("dropped"),
        &reason_map(&converted.dropped)?,
    )?;
    js_sys::Reflect::set(
        &result,
        &JsValue::from_str("degraded"),
        &reason_map(&converted.degraded)?,
    )?;
    Ok(result.into())
}

/// Convert a ProseMirror document back to canonical Carve source.
///
/// The save half of the editor loop. A payload the schema map does not describe
/// is refused rather than written approximately, so an editor extended with a
/// node the bridge has never seen fails where it can be reported.
#[cfg(feature = "prosemirror")]
#[wasm_bindgen(js_name = fromProseMirror)]
pub fn from_prose_mirror(doc: &str) -> Result<String, JsValue> {
    let document = carve::from_prosemirror(doc)
        .map_err(|error| js_error(format!("carve: invalid ProseMirror document: {error}")))?;
    carve::render_carve(&document)
        .map_err(|error| js_error(format!("carve: cannot write this tree: {error:?}")))
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
/// * `mentionUrl` and `tagUrl` - URL templates for `@mention` and `#tag`.
///   Without them both render as inert spans, which is why a host that wants
///   them linked has to say where to. The token name is percent-encoded into
///   `{name}` - `{user}` works too for a mention - and the result goes through
///   the same URL sanitizer as an authored link. These are the HOST's
///   configuration, not the document's: the template is trusted, the name
///   substituted into it is not.
///
/// An unrecognized key is ignored: the object is configuration, and a caller
/// who mistypes one deserves the render to still work. A wrong TYPE on a key
/// that is recognized does throw, because that changes behavior silently.
///
/// `renderers` is the one recognized key this entry point refuses, because a
/// render callback can fail and a bare string has nowhere to report that. Use
/// [`to_html_with_renderers`].
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

/// Read one options object and render it to a non-HTML target.
///
/// `plain` is the target's own no-options entry point, which keeps the fast
/// path a caller who passed nothing already had.
#[cfg(feature = "other-renderers")]
fn render_with_options(
    source: &str,
    options: Option<js_sys::Object>,
    plain: fn(&str) -> String,
    render: TargetRender,
) -> Result<String, JsValue> {
    let Some(request) = RenderRequest::read(options)? else {
        return Ok(plain(source));
    };
    render_target(
        source,
        &request.extension_keys(),
        &request.symbols,
        &request.config,
        render,
    )
    .map_err(profile_violation_error)
}

/// Render to Markdown with the same options object as
/// [`to_html_with_options`].
///
/// `profile` is why this exists. Without it a host can hold an untrusted
/// document to a profile on the way to HTML and not on the way to Markdown,
/// out of one package: the max-length bound, the denied constructs and the
/// link policy were all unreachable here. A document the profile rejects
/// throws a `ProfileViolationError` rather than resolving to an empty string.
///
/// What these targets read is narrower than HTML's list: `profile` and
/// `smartTypography` change the output and extensions run, while `symbols`
/// (markup-carve/carve-rs#1668), `labels`, `sections`, `sourceLine`, `mode` and
/// the heading-id switches are HTML-side concerns the engine's other renderers
/// do not consult. `renderers` is refused for the reason
/// [`to_html_with_options`] refuses it.
///
/// [`to_carve_with_options`] is narrower again and reads `profile` alone: the
/// engine's canonical writer is parse-only by contract, so extensions and
/// `smartTypography` are inert there. They are accepted rather than refused
/// because one options object is meant to serve every target.
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toMarkdownWithOptions)]
pub fn to_markdown_with_options(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    render_with_options(
        source,
        options,
        carve::to_markdown,
        carve::try_to_markdown_with_options,
    )
}

/// Render to plain text with an options object. See
/// [`to_markdown_with_options`].
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toPlainTextWithOptions)]
pub fn to_plain_text_with_options(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    render_with_options(
        source,
        options,
        carve::to_plain_text,
        carve::try_to_plain_text_with_options,
    )
}

/// Render to ANSI text with an options object. See
/// [`to_markdown_with_options`].
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toAnsiWithOptions)]
pub fn to_ansi_with_options(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    render_with_options(
        source,
        options,
        carve::to_ansi,
        carve::try_to_ansi_with_options,
    )
}

/// Write canonical Carve with an options object.
///
/// `profile` is the only option this target reads; see
/// [`to_markdown_with_options`] for why.
#[cfg(feature = "other-renderers")]
#[wasm_bindgen(js_name = toCarveWithOptions)]
pub fn to_carve_with_options(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<String, JsValue> {
    render_with_options(
        source,
        options,
        carve::to_carve,
        carve::try_to_carve_with_options,
    )
}

/// Render with a build-time math renderer, the option `mode: "static"` needs.
///
/// Static output carries no client scripts, so a formula it contains has to be
/// rendered while the HTML is being written. `renderers.math` is the callback
/// the engine calls for each ``` ```math ``` fence, with the TeX source and a
/// display flag; KaTeX's `renderToString` has that exact shape.
///
/// ```js
/// toHtmlWithRenderers(src, {
///   mode: 'static',
///   extensions: ['math-block'],
///   renderers: { math: (tex, display) => katex.renderToString(tex, { displayMode: display }) },
/// })
/// // { html: '…', rendererErrors: [] }
/// ```
///
/// SECURITY: what the callback returns is inserted as **TRUSTED RAW HTML**, the
/// same trust class as a `symbols` value. The difference worth stating: a symbol
/// value is host configuration keyed by a NAME, while a renderer is host
/// configuration that is HANDED DOCUMENT CONTENT and typically echoes some of it
/// back. A host rendering documents it did not author is accepting whatever its
/// renderer makes of that input, so the escaping is the renderer's job.
///
/// The engine consults the renderer only under `mode: "static"`; interactive
/// output keeps the `\[…\]` source for the client to typeset, and so does a
/// static render with no renderer supplied.
///
/// The callback must be SYNCHRONOUS. wasm-bindgen cannot await across it, so an
/// `async` renderer returns a Promise the engine has no way to resolve; that is
/// recorded as a failure rather than stringified into the document.
///
/// `renderers.diagrams` is the same callback one level down, keyed by the
/// fence's css class: `{ mermaid: (source) => html }`. A host that renders its
/// diagrams beforehand passes the lookup in that one line -
/// `(source) => prerendered.get(source)`. Mermaid itself cannot be passed,
/// because its `render` returns a Promise from v10 on.
///
/// ```js
/// toHtmlWithRenderers(src, {
///   mode: 'static',
///   extensions: ['fenced-render'],
///   renderers: { diagrams: { mermaid: (source) => prerendered.get(source) } },
/// })
/// ```
///
/// A diagram key the document never uses renders nothing and reports nothing.
/// So does a fence whose class the caller did not configure: the engine
/// degrades it to an escaped source block, and this binding never sees the
/// node.
///
/// A callback that throws, or returns anything other than a string, does not
/// abort the render: the node it was called for emits nothing and the failure
/// is reported in `rendererErrors`, with `renderer` naming the css class.
/// This entry point exists because [`to_html_with_options`] returns a bare
/// string with nowhere to put that, and so it rejects `renderers` rather than
/// dropping the failures.
#[wasm_bindgen(js_name = toHtmlWithRenderers, unchecked_return_type = "StaticRenderResult")]
pub fn to_html_with_renderers(
    source: &str,
    options: Option<js_sys::Object>,
) -> Result<JsValue, JsValue> {
    let failures: Rc<RefCell<Vec<RendererFailure>>> = Rc::default();
    let html = match RenderRequest::read_with(options, true)? {
        None => carve::to_html(source),
        Some(request) => match request.renderers.is_empty() {
            true => request.render(source).map_err(profile_violation_error)?,
            false => request
                .render_static(source, &failures)
                .map_err(profile_violation_error)?,
        },
    };

    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"html".into(), &JsValue::from_str(&html))?;
    let errors = js_sys::Array::new();
    for failure in failures.borrow().iter() {
        let item = js_sys::Object::new();
        js_sys::Reflect::set(&item, &"renderer".into(), &failure.renderer.as_str().into())?;
        // `display` is the math callback's second argument. A diagram renderer
        // has no such flag, and emitting a made-up one would read as data.
        if let Some(display) = failure.display {
            js_sys::Reflect::set(&item, &"display".into(), &display.into())?;
        }
        js_sys::Reflect::set(&item, &"source".into(), &failure.source.as_str().into())?;
        js_sys::Reflect::set(&item, &"message".into(), &failure.message.as_str().into())?;
        errors.push(&item);
    }
    js_sys::Reflect::set(&result, &"rendererErrors".into(), &errors)?;
    Ok(result.into())
}

/// One static-renderer call that produced no markup.
///
/// The engine's closure returns a `String` and has nowhere to put an error, so
/// the failure is recorded here and handed back by the entry point instead.
struct RendererFailure {
    /// `"math"`, or the fence css class a diagram renderer was keyed by.
    renderer: String,
    display: Option<bool>,
    source: String,
    message: String,
}

/// The JS callbacks one options object supplied.
#[derive(Default)]
struct RendererSet {
    math: Option<js_sys::Function>,
    /// In the order the caller wrote them, so a reported failure and the object
    /// the host passed can be read side by side.
    diagrams: Vec<(String, js_sys::Function)>,
}

impl RendererSet {
    fn is_empty(&self) -> bool {
        self.math.is_none() && self.diagrams.is_empty()
    }

    /// The engine's renderer set, with every failure routed to `failures`.
    fn engine_renderers(
        &self,
        failures: &Rc<RefCell<Vec<RendererFailure>>>,
    ) -> carve::StaticRenderers {
        let mut renderers = carve::StaticRenderers::new();
        if let Some(math) = &self.math {
            renderers = renderers.math(math_closure(math.clone(), Rc::clone(failures)));
        }
        for (key, callback) in &self.diagrams {
            renderers = renderers.diagram(
                key.clone(),
                diagram_closure(key.clone(), callback.clone(), Rc::clone(failures)),
            );
        }
        renderers
    }
}

/// The engine's math closure, over a JS callback.
fn math_closure(
    math: js_sys::Function,
    failures: Rc<RefCell<Vec<RendererFailure>>>,
) -> impl Fn(&str, bool) -> String {
    move |tex: &str, display: bool| {
        let call = math.call2(
            &JsValue::NULL,
            &JsValue::from_str(tex),
            &JsValue::from_bool(display),
        );
        returned_html(
            call,
            "math",
            "renderers.math",
            Some(display),
            tex,
            &failures,
        )
    }
}

/// The engine's diagram closure, over a JS callback keyed by `key`.
fn diagram_closure(
    key: String,
    callback: js_sys::Function,
    failures: Rc<RefCell<Vec<RendererFailure>>>,
) -> impl Fn(&str) -> String {
    move |source: &str| {
        let call = callback.call1(&JsValue::NULL, &JsValue::from_str(source));
        let named = format!("renderers.diagrams.{key}");
        returned_html(call, &key, &named, None, source, &failures)
    }
}

/// What one callback produced: its string, or an empty node and a recorded
/// failure.
///
/// `renderer` is what the failure is reported under; `named` is how the options
/// key is spelled in the message.
fn returned_html(
    call: Result<JsValue, JsValue>,
    renderer: &str,
    named: &str,
    display: Option<bool>,
    source: &str,
    failures: &Rc<RefCell<Vec<RendererFailure>>>,
) -> String {
    let record = |message: String| {
        failures.borrow_mut().push(RendererFailure {
            renderer: renderer.to_string(),
            display,
            source: source.to_string(),
            message,
        });
        String::new()
    };
    match call {
        Err(error) => record(format!("`{named}` threw: {}", describe_throw(&error))),
        Ok(value) => match value.as_string() {
            Some(html) => html,
            None => record(format!(
                "`{named}` returned {}, not a string",
                describe_value(&value)
            )),
        },
    }
}

/// The message a thrown JS value carries, whether or not it is an `Error`.
fn describe_throw(error: &JsValue) -> String {
    if let Some(error) = error.dyn_ref::<js_sys::Error>() {
        return String::from(error.message());
    }
    error
        .as_string()
        .unwrap_or_else(|| format!("a thrown {}", describe_value(error)))
}

/// What a non-string return value was, with the Promise case named.
///
/// A Promise is `typeof "object"` like any other, and it is the one wrong type
/// a correct-looking renderer produces - every `async` function returns one.
fn describe_value(value: &JsValue) -> String {
    if value.is_instance_of::<js_sys::Promise>() {
        return "a Promise (this call is synchronous and cannot await one)".to_string();
    }
    value
        .js_typeof()
        .as_string()
        .unwrap_or_else(|| "an unreadable value".to_string())
}

/// Read `renderers` out of a JS options object.
///
/// Every check here is at READ time, matching the rest of the object: a
/// misconfigured host finds out before the render rather than on the first
/// document that happens to contain a formula.
///
/// A diagram KEY is checked for shape only. Validating it against the set of
/// fence classes would mean keeping that list by hand here, because the
/// extension registry does not carry the class an entry claims
/// (markup-carve/carve-rs#1670).
fn renderers_field(options: &js_sys::Object, allowed: bool) -> Result<RendererSet, JsValue> {
    let renderers = js_sys::Reflect::get(options, &JsValue::from_str("renderers"))?;
    if renderers.is_undefined() || renderers.is_null() {
        return Ok(RendererSet::default());
    }
    if !allowed {
        return Err(type_error(
            "carve: `renderers` is only accepted by `toHtmlWithRenderers`, which returns the \
             failures a render callback reports",
        ));
    }
    let renderers = renderers
        .dyn_into::<js_sys::Object>()
        .map_err(|_| type_error("carve: `renderers` must be an object"))?;

    let math = js_sys::Reflect::get(&renderers, &JsValue::from_str("math"))?;
    let math = if math.is_undefined() || math.is_null() {
        None
    } else {
        Some(
            math.dyn_into::<js_sys::Function>()
                .map_err(|_| type_error("carve: `renderers.math` must be a function"))?,
        )
    };

    Ok(RendererSet {
        math,
        diagrams: diagram_renderers_field(&renderers)?,
    })
}

/// Read `renderers.diagrams`: a css class per key, a callback per value.
fn diagram_renderers_field(
    renderers: &js_sys::Object,
) -> Result<Vec<(String, js_sys::Function)>, JsValue> {
    let diagrams = js_sys::Reflect::get(renderers, &JsValue::from_str("diagrams"))?;
    if diagrams.is_undefined() || diagrams.is_null() {
        return Ok(Vec::new());
    }
    // A function here is the shape a host reaches for after reading
    // `renderers.math`, and one callback cannot say which fence it was called
    // for. Name the working spelling rather than reporting "not an object".
    if diagrams.is_function() {
        return Err(type_error(
            "carve: `renderers.diagrams` is keyed by the fence's css class, as \
             `{ mermaid: (source) => html }` - a bare function has no key to be called under",
        ));
    }
    let diagrams = diagrams
        .dyn_into::<js_sys::Object>()
        .map_err(|_| type_error("carve: `renderers.diagrams` must be an object"))?;
    let mut pairs = Vec::new();
    for entry in js_sys::Object::entries(&diagrams).iter() {
        let entry: js_sys::Array = entry.into();
        let key = entry.get(0).as_string().ok_or_else(|| {
            type_error("carve: every key in `renderers.diagrams` must be a string")
        })?;
        let callback = entry.get(1).dyn_into::<js_sys::Function>().map_err(|_| {
            type_error(&format!(
                "carve: `renderers.diagrams.{key}` must be a function"
            ))
        })?;
        pairs.push((key, callback));
    }
    Ok(pairs)
}

fn type_error(message: &str) -> JsValue {
    JsValue::from(js_sys::TypeError::new(message))
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
    /// Only [`to_html_with_renderers`] may set these: it is the one entry point
    /// whose return value can carry what a failing callback reported.
    renderers: RendererSet,
}

impl RenderRequest {
    /// `None` when the caller passed nothing at all, which is the engine's own
    /// default render and takes its fast path.
    fn read(options: Option<js_sys::Object>) -> Result<Option<Self>, JsValue> {
        Self::read_with(options, false)
    }

    fn read_with(
        options: Option<js_sys::Object>,
        renderers_allowed: bool,
    ) -> Result<Option<Self>, JsValue> {
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
            mention_url: string_field(&options, "mentionUrl")?,
            tag_url: string_field(&options, "tagUrl")?,
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
            renderers: renderers_field(&options, renderers_allowed)?,
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

    /// Render with the host's static renderers installed.
    ///
    /// Separate from [`Self::render`] because the renderer set is owned by the
    /// engine `Options` rather than rebuilt per helper, and because none of the
    /// default fast paths may be taken when a renderer is present.
    fn render_static(
        &self,
        source: &str,
        failures: &Rc<RefCell<Vec<RendererFailure>>>,
    ) -> Result<String, carve::ProfileViolationError> {
        let owned = self.extension_boxes();
        let options = self
            .engine_options(&owned)
            .with_renderers(self.renderers.engine_renderers(failures));
        carve::try_to_html_with_options(source, &options)
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

    /// The registry keys this request enables: an explicit list wins over the
    /// preview set, exactly as in [`Self::render`].
    fn extension_keys(&self) -> Vec<String> {
        match (&self.named, self.full) {
            (Some(keys), _) => keys.clone(),
            (None, true) => PREVIEW_EXTENSIONS
                .iter()
                .map(|k| (*k).to_string())
                .collect(),
            (None, false) => Vec::new(),
        }
    }

    /// The extension boxes this request needs, owned by the caller's frame
    /// because `Options` borrows them.
    fn extension_boxes(&self) -> Vec<Box<dyn carve::CarveExtension>> {
        build_extensions(&self.extension_keys())
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

/// The engine's four presets, named rather than constructed. A profile
/// assembled field by field across the wasm boundary would be a second way to
/// spell a security posture, and the presets are what the spec and the other
/// bindings describe.
const PROFILE_NAMES: &str = "\"full\", \"article\", \"comment\", \"minimal\"";

fn profile_by_name(name: &str) -> Option<carve::Profile> {
    match name {
        "full" => Some(carve::Profile::full()),
        "article" => Some(carve::Profile::article()),
        "comment" => Some(carve::Profile::comment()),
        "minimal" => Some(carve::Profile::minimal()),
        _ => None,
    }
}

/// Read the `profile` option out of a render options object.
fn profile_field(options: &js_sys::Object) -> Result<Option<carve::Profile>, JsValue> {
    enum_field(options, "profile", PROFILE_NAMES, profile_by_name)
}

/// Resolve a profile passed as an argument rather than an options key, with the
/// same rejection an options object gets.
#[cfg(feature = "ast-json")]
fn named_profile(name: &str) -> Result<carve::Profile, JsValue> {
    profile_by_name(name).ok_or_else(|| {
        JsValue::from(js_sys::TypeError::new(&format!(
            "carve: unknown `profile` {name:?} (supported: {PROFILE_NAMES})"
        )))
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
        extensions, render_core, render_full, render_with_extensions, RenderConfig, SymbolPairs,
        PREVIEW_EXTENSIONS,
    };
    #[cfg(any(feature = "other-renderers", feature = "ast-json"))]
    use super::{profile_by_name, render_target};

    /// The named profile, everything else at its default.
    #[cfg(any(feature = "other-renderers", feature = "ast-json"))]
    fn under_profile(name: &str) -> RenderConfig {
        RenderConfig {
            profile: profile_by_name(name),
            ..RenderConfig::default()
        }
    }

    /// The document the profile tests share: a heading and an image, both of
    /// which `comment` denies.
    #[cfg(feature = "other-renderers")]
    const DENIED: &str = "# Heading\n\n![alt](x.png)\n";

    #[cfg(feature = "other-renderers")]
    fn filtered(render: super::TargetRender) -> String {
        render_target(
            DENIED,
            &[],
            &SymbolPairs::new(),
            &under_profile("comment"),
            render,
        )
        .unwrap()
    }

    #[cfg(feature = "other-renderers")]
    fn unfiltered(render: super::TargetRender) -> String {
        render_target(
            DENIED,
            &[],
            &SymbolPairs::new(),
            &RenderConfig::default(),
            render,
        )
        .unwrap()
    }

    /// Sections off, everything else at its default.
    fn no_sections() -> RenderConfig {
        RenderConfig {
            sections: false,
            ..RenderConfig::default()
        }
    }

    /// Build the lowered symbol map the JS bridge produces (the `js_sys`
    /// conversion itself only runs inside a JS host).
    fn symbols(pairs: &[(&str, &str)]) -> SymbolPairs {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
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

    // markup-carve/carve-wasm#108: the options object used to reach HTML only,
    // so one package rendered the same document safely to HTML and unsafely to
    // Markdown. Each target is driven on its own, and against its own
    // no-profile render, so a failure names the target that lost the profile
    // rather than reporting that something somewhere changed.
    //
    // Under `comment` a heading is not a heading and an image is not an image:
    // both degrade to text, which is what these expectations pin.
    #[cfg(feature = "other-renderers")]
    #[test]
    fn the_profile_reaches_the_markdown_target() {
        assert_eq!(
            filtered(carve::try_to_markdown_with_options),
            "# Heading\n\n[img: alt\\]\n"
        );
        assert_eq!(
            unfiltered(carve::try_to_markdown_with_options),
            "# Heading\n\n![alt](x.png)\n"
        );
    }

    #[cfg(feature = "other-renderers")]
    #[test]
    fn the_profile_reaches_the_plain_text_target() {
        assert_eq!(
            filtered(carve::try_to_plain_text_with_options),
            "# Heading\n\n[img: alt]\n"
        );
        assert_eq!(
            unfiltered(carve::try_to_plain_text_with_options),
            "Heading\n\nalt\n"
        );
    }

    #[cfg(feature = "other-renderers")]
    #[test]
    fn the_profile_reaches_the_ansi_target() {
        assert_eq!(
            filtered(carve::try_to_ansi_with_options),
            "# Heading\n\n[img: alt]\n"
        );
        // The heading's own styling is what the filtered render no longer has.
        assert!(unfiltered(carve::try_to_ansi_with_options).contains("\u{1b}[1m"));
    }

    #[cfg(feature = "other-renderers")]
    #[test]
    fn the_profile_reaches_the_carve_writer() {
        // The `#` is escaped because it is text now, not a heading marker.
        assert_eq!(
            filtered(carve::try_to_carve_with_options),
            "\\# Heading\n\n[img: alt]\n"
        );
        assert_eq!(
            unfiltered(carve::try_to_carve_with_options),
            "# Heading\n\n![alt](x.png)\n"
        );
    }

    // The bound on the INPUT bytes, the one profile rule that refuses a render
    // outright instead of degrading a node. `minimal` caps at 10,000.
    #[cfg(feature = "other-renderers")]
    #[test]
    fn the_profile_length_bound_refuses_a_non_html_render() {
        let long = "a".repeat(16 * 1024);
        let error = render_target(
            &long,
            &[],
            &SymbolPairs::new(),
            &under_profile("minimal"),
            carve::try_to_markdown_with_options,
        )
        .unwrap_err();
        assert_eq!(error.violations.len(), 1);
        assert_eq!(error.violations[0].reason, "max_length_exceeded");
        assert!(render_target(
            &long,
            &[],
            &SymbolPairs::new(),
            &RenderConfig::default(),
            carve::try_to_markdown_with_options,
        )
        .is_ok());
    }

    // `smartTypography` is the other option these targets genuinely read.
    #[cfg(feature = "other-renderers")]
    #[test]
    fn smart_typography_reaches_the_markdown_target() {
        let source = "a ... b\n";
        let config = RenderConfig {
            smart_typography: carve::SmartTypographyMode::Source,
            ..RenderConfig::default()
        };
        assert_eq!(
            render_target(
                source,
                &[],
                &SymbolPairs::new(),
                &config,
                carve::try_to_markdown_with_options,
            )
            .unwrap(),
            "a ... b\n"
        );
        assert_eq!(
            render_target(
                source,
                &[],
                &SymbolPairs::new(),
                &RenderConfig::default(),
                carve::try_to_markdown_with_options,
            )
            .unwrap(),
            "a \u{2026} b\n"
        );
    }

    // Serializing the tree of an untrusted document had the same hole.
    #[cfg(feature = "ast-json")]
    #[test]
    fn the_profile_reaches_the_json_export() {
        let config = RenderConfig {
            positions: true,
            ..under_profile("comment")
        };
        let json = render_target(
            "# Heading\n",
            &[],
            &SymbolPairs::new(),
            &config,
            carve::try_to_json_with_options,
        )
        .unwrap();
        assert!(!json.contains("\"heading\""), "{json}");
        assert!(json.contains("Heading"), "{json}");
        assert!(crate::parse_json("# Heading\n").contains("\"heading\""));
    }

    // The options form has to agree with `parseJson` when the object carries no
    // switches, or a host gains a profile and loses the positions PART 12 §4
    // requires the serialized form to carry.
    #[cfg(feature = "ast-json")]
    #[test]
    fn the_json_options_form_matches_parse_json_by_default() {
        let source = "# Heading\n\nBody\n";
        let config = RenderConfig {
            positions: true,
            ..RenderConfig::default()
        };
        assert_eq!(
            render_target(
                source,
                &[],
                &SymbolPairs::new(),
                &config,
                carve::try_to_json_with_options,
            )
            .unwrap(),
            crate::parse_json(source)
        );
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

    #[cfg(feature = "source-patches")]
    #[test]
    fn the_pinned_engine_prepares_utf8_source_patches() {
        let source = "see → here";
        let patch = crate::source_patch::create(
            source,
            "see ⇒ here",
            crate::source_patch::SourceEditKind::Refactor,
            "unicode",
        );
        assert_eq!(
            crate::source_patch::apply(source, &patch).unwrap(),
            "see ⇒ here"
        );
        assert!(crate::source_patch::apply("stale", &patch).is_err());
    }
}
