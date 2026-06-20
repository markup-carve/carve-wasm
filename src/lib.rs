use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = toHtml)]
pub fn to_html(source: &str) -> String {
    carve::to_html(source)
}

/// Render with the demo-useful built-in Carve extensions enabled
/// (tab-normalize, details, Mermaid, wikilinks, autolink, list-table,
/// math-block, heading-permalinks, citations, external-links). Lets the WASM
/// engine match an extensions-on host (e.g. the docs Playground) instead of the
/// core-only `toHtml`.
///
/// Deliberately excludes table-of-contents (it auto-injects a TOC, which
/// clutters a preview). The code-group / tabs extensions are also absent
/// because carve-rs does not implement them.
#[wasm_bindgen(js_name = toHtmlFull)]
pub fn to_html_full(source: &str) -> String {
    use carve::{
        Autolink, Citations, Details, ExternalLinks, HeadingPermalinks, ListTable, MathBlock,
        Mermaid, Options, TabNormalize, Wikilinks,
    };
    // `Options` borrows each extension, so they must outlive it; bind locals.
    let tab_normalize = TabNormalize::new();
    let details = Details::new();
    let mermaid = Mermaid::new();
    let wikilinks = Wikilinks::new();
    let autolink = Autolink::new();
    let list_table = ListTable::new();
    let math_block = MathBlock::new();
    let heading_permalinks = HeadingPermalinks::new();
    let citations = Citations::new();
    let external_links = ExternalLinks::new();
    let options = Options::new()
        .with_extension(&tab_normalize)
        .with_extension(&details)
        .with_extension(&mermaid)
        .with_extension(&wikilinks)
        .with_extension(&autolink)
        .with_extension(&list_table)
        .with_extension(&math_block)
        .with_extension(&heading_permalinks)
        .with_extension(&citations)
        .with_extension(&external_links);
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

    #[test]
    fn full_enables_list_table_extension() {
        let src = "{header-rows=1}\n::: list-table \"Quarterly results\"\n- - Region\n  - Notes\n- - EMEA\n  - Strong quarter.\n:::\n";
        let html = crate::to_html_full(src);
        assert!(html.contains("<table"), "expected a <table>, got: {html}");
        assert!(!html.contains("class=\"list-table\""));
    }
}
