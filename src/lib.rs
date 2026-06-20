use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = toHtml)]
pub fn to_html(source: &str) -> String {
    carve::to_html(source)
}

/// Render with the demo-useful built-in Carve extensions enabled
/// (tab-normalize, details, Mermaid, wikilinks, autolink). Lets the WASM engine
/// match an extensions-on host (e.g. the docs Playground) instead of the
/// core-only `toHtml`.
///
/// Deliberately excludes table-of-contents and heading-permalinks (they
/// auto-inject a TOC / mutate headings, which clutters a preview), and
/// external-links / citations (no visible effect without config). This mirrors
/// the JS Playground's extension set, minus the code-group / tabs extensions
/// that carve-rs does not implement.
#[wasm_bindgen(js_name = toHtmlFull)]
pub fn to_html_full(source: &str) -> String {
    use carve::{Autolink, Details, Mermaid, Options, TabNormalize, Wikilinks};
    // `Options` borrows each extension, so they must outlive it; bind locals.
    let tab_normalize = TabNormalize::new();
    let details = Details::new();
    let mermaid = Mermaid::new();
    let wikilinks = Wikilinks::new();
    let autolink = Autolink::new();
    let options = Options::new()
        .with_extension(&tab_normalize)
        .with_extension(&details)
        .with_extension(&mermaid)
        .with_extension(&wikilinks)
        .with_extension(&autolink);
    carve::to_html_with_options(source, &options)
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_html() {
        assert!(crate::to_html("# Hello").contains("<h1>Hello</h1>"));
    }

    #[test]
    fn full_enables_mermaid_extension() {
        let html = crate::to_html_full("``` mermaid\ngraph TD; A-->B\n```\n");
        assert!(html.contains("<pre class=\"mermaid\">"));
    }
}
