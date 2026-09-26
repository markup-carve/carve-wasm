//! DUPLICATES the Rust importer's denied-scheme rule, for markup-carve/carve-wasm#145.
//! DELETE this module and its two call sites when the engine pin moves to a crate whose
//! importer applies the rule itself.
//!
//! The pinned `carve-lang =0.1.6` imports a `javascript:` destination as a live link and
//! reports nothing, so `htmlToCarve(input, "safe")` hands a consumer the destination the
//! argument named `safe` promised to remove. `docs/html-import-contract.md` ("A denied
//! destination is not a destination either") asks for the content, any surviving
//! attributes, and one `attribute-dropped` row per removal; that is what this produces.
//!
//! Two things it does not match. The rows land after the engine's own rather than in
//! document order among them, because this pass runs on the finished tree. And a
//! `<figure>` whose image carries a denied source keeps the figure and its caption, where
//! the engine's empty-destination path unwraps both and drops the figure's own attributes;
//! the guard loses nothing there, so it leaves the structure standing.

use std::borrow::Cow;

use carve::{
    AttrSlot, Attrs, BlockNode, Document, FigureTarget, HtmlImportDiagnosticCode,
    HtmlImportSeverity, InlineNode, LinkPolicy, MigrationDiagnostic, Paragraph, Span,
};

/// PART 9 §25's URL sink denylist, copied from the engine's
/// `escape::DANGEROUS_VALUE_SCHEMES`, which is `pub(crate)` there.
const DENIED_SCHEMES: [&str; 23] = [
    "javascript",
    "vbscript",
    "data",
    "file",
    "ms-msdt",
    "ms-office",
    "ms-word",
    "ms-excel",
    "ms-powerpoint",
    "ms-access",
    "ms-visio",
    "ms-project",
    "ms-publisher",
    "ms-infopath",
    "ms-spd",
    "ms-search",
    "search-ms",
    "ms-cxh",
    "ms-cxh-full",
    "shell",
    "vscode",
    "vscode-insiders",
    "jar",
];

/// One removed destination, in the vocabulary the contract pins.
pub struct Removal {
    tag: &'static str,
    attr: &'static str,
    destination: String,
}

impl Removal {
    pub fn diagnostic(&self) -> MigrationDiagnostic {
        let code = HtmlImportDiagnosticCode::AttributeDropped;
        MigrationDiagnostic {
            code: code.as_str().to_owned(),
            message: format!(
                "Dropped {} with a denied URL scheme on <{}>",
                self.attr, self.tag
            ),
            severity: HtmlImportSeverity::Warning,
            fidelity: code.fidelity(),
            confidence: code.confidence(),
            path: None,
        }
    }
}

/// The probe is the engine's own: a policy carrying only the scheme denylist answers the
/// same question `sanitize_url` does, control-stripping and all, so the two cannot drift
/// on how a scheme is read - only on which schemes are named.
fn policy() -> LinkPolicy {
    LinkPolicy::unrestricted()
        .set_denied_schemes(DENIED_SCHEMES.iter().map(|s| (*s).to_owned()).collect())
}

/// The destinations a raw import found denied, so the source pass can recognize them after
/// the canonical writer has escaped them.
///
/// The writer escapes the very characters the contract says to strip: a tab inside the
/// scheme reaches the source as `%09`, and neither the sink denylist nor this probe still
/// sees a control there. Resolving the escape blindly would deny `java%09script:x`, which
/// is a relative name a browser never reads as a scheme, so only a value the raw tree
/// already condemned is resolved.
#[derive(Default)]
pub struct Escaped(std::collections::HashSet<String>);

pub fn escaped(removals: &[Removal]) -> Escaped {
    Escaped(
        removals
            .iter()
            .map(|removal| removal.destination.clone())
            .collect(),
    )
}

struct Guard {
    policy: LinkPolicy,
    escaped: Escaped,
    removals: Vec<Removal>,
}

impl Guard {
    fn denied(&self, url: &str) -> bool {
        if url.is_empty() {
            return false;
        }
        !self.policy.is_url_allowed(url, None)
            || (!self.escaped.0.is_empty() && self.escaped.0.contains(decoded(url).as_ref()))
    }

    fn took(&mut self, tag: &'static str, attr: &'static str, destination: String) {
        self.removals.push(Removal {
            tag,
            attr,
            destination,
        });
    }
}

/// Every percent-escape resolved. Safe to be this broad because the result is only ever
/// compared against a destination the raw import already denied.
fn decoded(url: &str) -> Cow<'_, str> {
    if !url.contains('%') {
        return Cow::Borrowed(url);
    }
    let bytes = url.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match escape_at(bytes, index) {
            Some(byte) => {
                out.push(byte);
                index += 3;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    Cow::Owned(String::from_utf8_lossy(&out).into_owned())
}

fn escape_at(bytes: &[u8], at: usize) -> Option<u8> {
    let [b'%', high, low] = bytes.get(at..at + 3)? else {
        return None;
    };
    Some(hex(*high)? * 16 + hex(*low)?)
}

fn hex(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|digit| digit as u8)
}

fn paragraph(content: InlineNode) -> Paragraph {
    Paragraph {
        attrs: None,
        children: vec![content],
        at_content_column: true,
        block_image: false,
        pos: None,
    }
}

/// What a denied image leaves behind: its alternative text, carrying any attribute the
/// element kept. `None` when it carried neither and there is nothing to stand in its place.
///
/// One function for all three image slots, because the engine's rule does not vary by
/// slot: a lone `<img src="" alt="logo" title="tip">` comes back as `[logo]{title=tip}`
/// inside a paragraph, not as a paragraph carrying the attributes.
fn image_content(image: &mut carve::Image) -> Option<InlineNode> {
    let alt = std::mem::take(&mut image.alt);
    match fold_title(image.attrs.take(), image.title.take()) {
        Some(attrs) => Some(InlineNode::Span(Span {
            attrs: Some(attrs),
            children: vec![InlineNode::text(alt)],
            injected: false,
            pos: None,
        })),
        None => (!alt.is_empty()).then(|| InlineNode::text(alt)),
    }
}

/// A link's and an image's `title` is a field of its own, not an attribute, and the
/// engine's own destination-less rule keeps it as `{title=...}`. Without this the guard
/// would drop it silently, which is the loss it exists to report rather than take.
fn fold_title(attrs: Option<Attrs>, title: Option<String>) -> Option<Attrs> {
    let Some(title) = title else {
        return attrs;
    };
    let mut attrs = attrs.unwrap_or_default();
    if !attrs.key_values.contains_key("title") {
        attrs.key_values.insert("title".to_owned(), title);
        attrs.order.push(AttrSlot::Key("title".to_owned()));
    }
    Some(attrs)
}

/// Replace every link and image whose destination carries a denied scheme by its content,
/// reporting one removal each. Returns the removals in document order.
pub fn strip(doc: &mut Document, escaped: Escaped) -> Vec<Removal> {
    let mut guard = Guard {
        policy: policy(),
        escaped,
        removals: Vec::new(),
    };
    blocks(&mut doc.children, &mut guard);
    for definition in doc.footnote_defs.values_mut() {
        blocks(definition, &mut guard);
    }
    guard.removals
}

fn blocks(nodes: &mut Vec<BlockNode>, guard: &mut Guard) {
    let mut kept = Vec::with_capacity(nodes.len());
    for node in nodes.drain(..) {
        match node {
            BlockNode::BlockImage(mut image) if guard.denied(&image.src) => {
                guard.took("img", "src", std::mem::take(&mut image.src));
                if let Some(content) = image_content(&mut image) {
                    kept.push(BlockNode::Paragraph(paragraph(content)));
                }
            }
            mut other => {
                descend(&mut other, guard);
                kept.push(other);
            }
        }
    }
    *nodes = kept;
}

/// Every block variant that can hold a link or an image, plus the leaves that cannot.
/// The match is exhaustive so a variant added to the engine's AST does not compile until
/// it is classified here.
fn descend(node: &mut BlockNode, guard: &mut Guard) {
    match node {
        BlockNode::Heading(n) => inlines(&mut n.children, guard),
        BlockNode::Paragraph(n) => {
            inlines(&mut n.children, guard);
            // The flag says the paragraph IS a lone image, which it no longer is once the
            // image has become text.
            if n.block_image && !n.children.iter().any(|c| matches!(c, InlineNode::Image(_))) {
                n.block_image = false;
            }
        }
        BlockNode::List(n) => {
            for item in &mut n.items {
                blocks(&mut item.children, guard);
            }
        }
        BlockNode::BlockQuote(n) => blocks(&mut n.children, guard),
        BlockNode::Table(n) => table(n, guard),
        BlockNode::Admonition(n) => {
            if let Some(title) = &mut n.title {
                inlines(title, guard);
            }
            blocks(&mut n.children, guard);
        }
        BlockNode::Div(n) => blocks(&mut n.children, guard),
        BlockNode::LineBlock(n) => blocks(&mut n.children, guard),
        BlockNode::DefinitionList(n) => {
            for item in &mut n.items {
                for term in &mut item.terms {
                    inlines(&mut term.children, guard);
                }
                for definition in &mut item.definitions {
                    blocks(&mut definition.children, guard);
                }
            }
        }
        BlockNode::Figure(n) => {
            figure_target(&mut n.target, guard);
            inlines(&mut n.caption, guard);
            if let Some(caption) = &mut n.short_caption {
                inlines(caption, guard);
            }
        }
        BlockNode::FigureGroup(n) => {
            blocks(&mut n.children, guard);
            if let Some(caption) = &mut n.caption {
                inlines(caption, guard);
            }
        }
        BlockNode::CitationDefinition(n) => inlines(&mut n.children, guard),
        BlockNode::Extension(n) => {
            blocks(&mut n.children, guard);
            if let Some(summary) = &mut n.summary {
                inlines(summary, guard);
            }
        }
        // A destination this pass would have to read reaches none of these: the importer
        // writes no `[label]: url` definition, and the rest carry no URL at all.
        BlockNode::BlockImage(_)
        | BlockNode::CodeBlock(_)
        | BlockNode::AbbreviationDef(_)
        | BlockNode::LinkReferenceDefinition(_)
        | BlockNode::RawBlock(_)
        | BlockNode::Comment(_)
        | BlockNode::ThematicBreak(_) => {}
    }
}

fn table(node: &mut carve::Table, guard: &mut Guard) {
    if let Some(caption) = &mut node.caption {
        inlines(caption, guard);
    }
    if let Some(caption) = &mut node.short_caption {
        inlines(caption, guard);
    }
    for row in &mut node.rows {
        for cell in &mut row.cells {
            inlines(&mut cell.children, guard);
        }
    }
}

fn figure_target(target: &mut FigureTarget, guard: &mut Guard) {
    match target {
        FigureTarget::Image(image) => {
            if guard.denied(&image.src) {
                guard.took("img", "src", std::mem::take(&mut image.src));
                let content = image_content(image).unwrap_or_else(|| InlineNode::text(""));
                *target = FigureTarget::Paragraph(paragraph(content));
            }
        }
        FigureTarget::BlockQuote(n) => blocks(&mut n.children, guard),
        FigureTarget::Table(n) => table(n, guard),
        FigureTarget::Paragraph(n) => inlines(&mut n.children, guard),
        FigureTarget::CodeBlock(_) => {}
    }
}

/// The inline half. A denied link keeps its children; one that carried an attribute block
/// keeps it as a span, which is the surviving-attributes half of the contract.
fn inlines(nodes: &mut Vec<InlineNode>, guard: &mut Guard) {
    let mut kept = Vec::with_capacity(nodes.len());
    let mut unwrapped = false;
    for node in nodes.drain(..) {
        match node {
            InlineNode::Link(mut link) if guard.denied(&link.href) => {
                guard.took("a", "href", std::mem::take(&mut link.href));
                unwrapped = true;
                inlines(&mut link.children, guard);
                match fold_title(link.attrs, link.title) {
                    Some(attrs) => kept.push(InlineNode::Span(Span {
                        attrs: Some(attrs),
                        children: link.children,
                        injected: false,
                        pos: None,
                    })),
                    None => kept.extend(link.children),
                }
            }
            InlineNode::Image(mut image) if guard.denied(&image.src) => {
                guard.took("img", "src", std::mem::take(&mut image.src));
                unwrapped = true;
                kept.extend(image_content(&mut image));
            }
            mut other => {
                descend_inline(&mut other, guard);
                kept.push(other);
            }
        }
    }
    if !unwrapped {
        *nodes = kept;
        return;
    }
    // Splicing a link's children out leaves text beside text, which the other importers
    // never produce: the oracle tree for `denied-scheme-destination` carries one text node
    // across the removed anchor.
    let mut merged: Vec<InlineNode> = Vec::with_capacity(kept.len());
    for node in kept {
        match (merged.last_mut(), node) {
            (Some(InlineNode::Text(last)), InlineNode::Text(next)) => {
                last.value.push_str(&next.value);
                last.pos = None;
            }
            (_, node) => merged.push(node),
        }
    }
    *nodes = merged;
}

/// Exhaustive for the same reason [`descend`] is.
fn descend_inline(node: &mut InlineNode, guard: &mut Guard) {
    match node {
        InlineNode::Emphasis(n) => inlines(&mut n.children, guard),
        InlineNode::Link(n) => inlines(&mut n.children, guard),
        InlineNode::Span(n) => inlines(&mut n.children, guard),
        InlineNode::Extension(n) => inlines(&mut n.children, guard),
        InlineNode::CriticInsert(n) => inlines(&mut n.children, guard),
        InlineNode::CriticDelete(n) => inlines(&mut n.children, guard),
        InlineNode::CriticSubstitute(n) => {
            inlines(&mut n.old, guard);
            inlines(&mut n.new, guard);
        }
        InlineNode::Footnote(n) => {
            if let Some(inline) = &mut n.inline {
                inlines(inline, guard);
            }
        }
        InlineNode::CitationGroup(n) => {
            for citation in &mut n.items {
                for part in [
                    &mut citation.prefix,
                    &mut citation.locator,
                    &mut citation.suffix,
                ]
                .into_iter()
                .flatten()
                {
                    inlines(part, guard);
                }
            }
        }
        // `AutoLink` and `CrossRef` hold a destination, and the importer builds neither:
        // reading them here would be a check that cannot fire.
        InlineNode::Image(_)
        | InlineNode::Text(_)
        | InlineNode::EscapedText(_)
        | InlineNode::SmartPunctuation(_)
        | InlineNode::Code(_)
        | InlineNode::Math(_)
        | InlineNode::RawInline(_)
        | InlineNode::LiteralInline(_)
        | InlineNode::Symbol(_)
        | InlineNode::AutoLink(_)
        | InlineNode::CrossRef(_)
        | InlineNode::CaptionNumber(_)
        | InlineNode::Mention(_)
        | InlineNode::Tag(_)
        | InlineNode::Abbreviation(_)
        | InlineNode::SoftBreak(_)
        | InlineNode::HardBreak(_)
        | InlineNode::CriticComment(_)
        | InlineNode::Comment(_) => {}
    }
}
