use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = toHtml)]
pub fn to_html(source: &str) -> String {
    carve::to_html(source)
}

/// Render with every built-in Carve extension enabled (Mermaid, wikilinks,
/// autolink, external links, heading permalinks, table of contents, citations,
/// tab normalization). Lets the WASM engine match an extensions-on host (e.g.
/// the docs Playground) as closely as carve-rs supports, instead of the
/// core-only `toHtml`.
#[wasm_bindgen(js_name = toHtmlFull)]
pub fn to_html_full(source: &str) -> String {
    use carve::{
        Autolink, Citations, Details, ExternalLinks, HeadingPermalinks, Mermaid, Options,
        TabNormalize, TableOfContents, Wikilinks,
    };
    // `Options` borrows each extension, so they must outlive it; bind locals.
    let tab_normalize = TabNormalize::new();
    let details = Details::new();
    let mermaid = Mermaid::new();
    let wikilinks = Wikilinks::new();
    let autolink = Autolink::new();
    let external_links = ExternalLinks::new();
    let table_of_contents = TableOfContents::new();
    let heading_permalinks = HeadingPermalinks::new();
    let citations = Citations::new();
    let options = Options::new()
        .with_extension(&tab_normalize)
        .with_extension(&details)
        .with_extension(&mermaid)
        .with_extension(&wikilinks)
        .with_extension(&autolink)
        .with_extension(&external_links)
        .with_extension(&table_of_contents)
        .with_extension(&heading_permalinks)
        .with_extension(&citations);
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
