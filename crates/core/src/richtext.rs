//! HTML → styled-span conversion for the native UI (decision D1).
//!
//! The port replaces Leptos/WASM HTML rendering with `egui` styled text.
//! This module is the toolkit-agnostic seam: it sanitizes HTML with
//! `ammonia` and converts it into [`Span`]s — plain text plus a
//! presentation-neutral [`SpanStyle`] — which the UI layer maps onto its
//! text engine. Unknown or hostile markup degrades to plain text
//! (plain-text fallback per the risk register).
//!
//! Newline policy: line breaks live only in *separator* spans
//! (default-styled `"\n"` / `"\n\n"`), never inside styled text runs, so
//! renderers can split spans on newlines without splitting styles.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

/// Presentation-neutral inline styling extracted from HTML.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpanStyle {
    /// `<b>`, `<strong>`, headings render bold at the UI layer's discretion.
    pub bold: bool,
    /// `<i>`, `<em>`.
    pub italic: bool,
    /// `<u>`.
    pub underline: bool,
    /// `<s>`, `<del>`, `<strike>`.
    pub strikethrough: bool,
    /// Monospace: inline `<code>` or block `<pre>`.
    pub code: bool,
    /// Indented quotation block (`<blockquote>`).
    pub quote: bool,
    /// Heading level `1..=6` when the span came from `<h1>`..`<h6>`.
    pub heading: Option<u8>,
    /// Resolved `href` of a surrounding `<a>`.
    pub link: Option<String>,
}

/// A run of text sharing one [`SpanStyle`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Span {
    /// The text content (never contains separator newlines).
    pub text: String,
    /// The style applied to the whole run.
    pub style: SpanStyle,
}

impl Span {
    fn plain(text: &str) -> Self {
        Self {
            text: text.to_string(),
            style: SpanStyle::default(),
        }
    }

    fn styled(text: &str, style: SpanStyle) -> Self {
        Self {
            text: text.to_string(),
            style,
        }
    }
}

/// Convert (possibly untrusted) HTML into styled spans.
///
/// The input is sanitized first: disallowed tags are dropped (scripty
/// tags together with their content), URLs are scheme-checked, and only
/// the formatting subset understood by this module survives. Anything the
/// sanitizer keeps but the parser does not recognize is rendered as
/// plain text.
pub fn html_to_spans(html: &str) -> Vec<Span> {
    let clean = sanitize(html);
    Parser::new(&clean).run()
}

/// Convert already-sanitized HTML into styled spans.
///
/// Only for callers that ran [`crate::security::sanitize_thread_html`] (or
/// an equivalent ammonia pass) themselves; [`html_to_spans`] is the
/// safe default.
pub fn sanitized_html_to_spans(clean: &str) -> Vec<Span> {
    Parser::new(clean).run()
}

fn sanitize(html: &str) -> String {
    let mut tags = HashSet::new();
    for t in [
        "a",
        "b",
        "blockquote",
        "br",
        "code",
        "del",
        "div",
        "em",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "i",
        "li",
        "ol",
        "p",
        "pre",
        "s",
        "span",
        "strike",
        "strong",
        "u",
        "ul",
    ] {
        tags.insert(t);
    }
    let mut a_attrs = HashSet::new();
    a_attrs.insert("href");
    let mut tag_attributes = HashMap::new();
    tag_attributes.insert("a", a_attrs);
    let mut schemes = HashSet::new();
    schemes.insert("https");
    schemes.insert("http");
    schemes.insert("mailto");

    let mut builder = ammonia::Builder::new();
    builder
        .tags(tags)
        .tag_attributes(tag_attributes)
        .rm_tags([
            "script", "style", "title", "textarea", "noscript", "iframe", "object", "embed", "svg",
            "math", "form", "input", "button", "select", "option", "template",
        ])
        .url_schemes(schemes)
        .link_rel(None);
    builder.clean(html).to_string()
}

struct ListState {
    ordered: bool,
    index: u64,
}

struct Parser<'a> {
    reader: Reader<&'a [u8]>,
    style: SpanStyle,
    /// Style snapshots for balanced nesting (`<b><i>…</i></b>`).
    stack: Vec<SpanStyle>,
    buf: String,
    /// Lazy block separator: number of newlines owed before the next text.
    pending_sep: usize,
    list_stack: Vec<ListState>,
    /// Inside `<pre>`: keep source whitespace verbatim.
    preserve_ws: bool,
    out: Vec<Span>,
}

impl<'a> Parser<'a> {
    fn new(clean: &'a str) -> Self {
        let mut reader = Reader::from_str(clean);
        reader.config_mut().trim_text(false);
        Self {
            reader,
            style: SpanStyle::default(),
            stack: Vec::new(),
            buf: String::new(),
            pending_sep: 0,
            list_stack: Vec::new(),
            preserve_ws: false,
            out: Vec::new(),
        }
    }

    fn run(mut self) -> Vec<Span> {
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(e)) => {
                    let href = e.try_get_attribute("href").ok().flatten().and_then(|a| {
                        a.normalized_value(XmlVersion::Implicit1_0)
                            .ok()
                            .map(Cow::into_owned)
                    });
                    self.handle_start(e.name().0, href.as_deref());
                }
                Ok(Event::Empty(e)) => {
                    if e.name().0 == "br" {
                        self.line_break();
                    }
                }
                Ok(Event::End(e)) => {
                    self.handle_end(e.name().0);
                }
                Ok(Event::Text(t)) => {
                    let content = t.xml10_content();
                    self.append_text(&content);
                }
                Ok(Event::GeneralRef(r)) => {
                    if let Some(c) = decode_entity(r.as_ref()) {
                        self.push_char(c);
                    }
                }
                Ok(Event::CData(c)) => {
                    self.append_text(c.as_ref());
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(_) => break, // malformed input: keep what we have (fallback)
            }
        }
        self.flush();
        self.out
    }

    fn handle_start(&mut self, name: &str, href: Option<&str>) {
        match name {
            "b" | "strong" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.bold = true;
            }
            "i" | "em" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.italic = true;
            }
            "u" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.underline = true;
            }
            "s" | "del" | "strike" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.strikethrough = true;
            }
            "code" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.code = true;
            }
            "pre" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.code = true;
                self.preserve_ws = true;
                self.block_sep(2);
            }
            "blockquote" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.quote = true;
                self.block_sep(2);
            }
            "a" => {
                self.flush();
                self.stack.push(self.style.clone());
                if let Some(href) = href {
                    self.style.link = Some(href.to_string());
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                self.stack.push(self.style.clone());
                self.style.heading = name[1..].parse::<u8>().ok().filter(|n| (1..=6).contains(n));
                self.block_sep(2);
            }
            "p" | "div" | "span" => {
                self.stack.push(self.style.clone());
                if name != "span" {
                    self.block_sep(2);
                }
            }
            "ul" | "ol" => {
                self.stack.push(self.style.clone());
                self.list_stack.push(ListState {
                    ordered: name == "ol",
                    index: 0,
                });
                self.block_sep(2);
            }
            "li" => {
                self.stack.push(self.style.clone());
                self.block_sep(1);
                let prefix = self.list_stack.last_mut().map(|l| {
                    l.index += 1;
                    if l.ordered {
                        format!("{}. ", l.index)
                    } else {
                        "\u{2022} ".to_string()
                    }
                });
                if let Some(prefix) = prefix {
                    self.buf.push_str(&prefix);
                }
            }
            "br" => self.line_break(),
            _ => {
                // Unknown (but sanitized-in) tag: transparent container.
                self.stack.push(self.style.clone());
            }
        }
    }

    fn handle_end(&mut self, name: &str) {
        match name {
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" => {
                self.flush();
                self.pop_style();
                self.block_sep(2);
            }
            "pre" => {
                self.flush();
                self.pop_style();
                self.preserve_ws = false;
                self.block_sep(2);
            }
            "code" | "b" | "strong" | "i" | "em" | "u" | "s" | "del" | "strike" | "a" | "span"
            | "ul" | "ol" => {
                self.flush();
                self.pop_style();
                if name == "ul" || name == "ol" {
                    self.list_stack.pop();
                    self.block_sep(2);
                }
            }
            "li" => {
                self.flush();
                self.pop_style();
                self.block_sep(1);
            }
            "br" => self.line_break(),
            _ => {
                self.flush();
                self.pop_style();
            }
        }
    }

    fn pop_style(&mut self) {
        if let Some(prev) = self.stack.pop() {
            self.style = prev;
        }
    }

    fn line_break(&mut self) {
        self.flush();
        self.pending_sep = self.pending_sep.max(1);
    }

    fn block_sep(&mut self, newlines: usize) {
        self.flush();
        if self.out.is_empty() && self.buf.is_empty() {
            return; // document start: never emit a leading separator
        }
        self.pending_sep = self.pending_sep.max(newlines);
    }

    fn append_text(&mut self, text: &str) {
        if self.preserve_ws {
            self.buf.push_str(text);
            return;
        }
        let collapsed = collapse_ws(text);
        if collapsed.is_empty() {
            return;
        }
        // Whitespace-only text between blocks (or at the document start)
        // is redundant: the block separator already provides the break.
        if collapsed.chars().all(char::is_whitespace)
            && self.buf.is_empty()
            && (self.pending_sep > 0 || self.out.is_empty())
        {
            return;
        }
        let start = if self.buf.is_empty() && self.pending_sep > 0 {
            collapsed.trim_start()
        } else {
            collapsed.as_str()
        };
        if !start.is_empty() {
            self.buf.push_str(start);
        }
    }

    fn push_char(&mut self, c: char) {
        self.buf.push(c);
    }

    fn flush(&mut self) {
        if !self.buf.is_empty() {
            if self.pending_sep > 0 {
                self.out.push(Span::plain(&"\n".repeat(self.pending_sep)));
                self.pending_sep = 0;
            }
            self.out.push(Span::styled(&self.buf, self.style.clone()));
            self.buf.clear();
        }
    }
}

/// Collapse HTML whitespace runs (space, tab, CR, LF) to single spaces.
fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_ws = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\r' | '\n') {
            in_ws = true;
        } else {
            if in_ws {
                out.push(' ');
                in_ws = false;
            }
            out.push(c);
        }
    }
    if in_ws {
        out.push(' ');
    }
    out
}

/// Decode the XML/HTML entities ammonia leaves behind in text content.
fn decode_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "nbsp" => Some('\u{a0}'),
        numeric => {
            let n = numeric
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| {
                    numeric
                        .strip_prefix('#')
                        .and_then(|d| d.parse::<u32>().ok())
                });
            n.and_then(char::from_u32)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold() -> SpanStyle {
        SpanStyle {
            bold: true,
            ..SpanStyle::default()
        }
    }

    fn code() -> SpanStyle {
        SpanStyle {
            code: true,
            ..SpanStyle::default()
        }
    }

    fn link(url: &str) -> SpanStyle {
        SpanStyle {
            link: Some(url.to_string()),
            ..SpanStyle::default()
        }
    }

    fn heading(n: u8) -> SpanStyle {
        SpanStyle {
            heading: Some(n),
            ..SpanStyle::default()
        }
    }

    #[test]
    fn plain_text_passes_through() {
        let spans = html_to_spans("just some text");
        assert_eq!(
            spans,
            vec![Span::styled("just some text", SpanStyle::default())]
        );
    }

    #[test]
    fn empty_input_yields_no_spans() {
        assert!(html_to_spans("").is_empty());
        assert!(html_to_spans("   \n\t ").is_empty());
    }

    #[test]
    fn inline_styles_and_nesting() {
        let spans = html_to_spans("<p>Hello <b>bold <i>both</i></b> world</p>");
        assert_eq!(
            spans,
            vec![
                Span::styled("Hello ", SpanStyle::default()),
                Span::styled("bold ", bold()),
                Span::styled(
                    "both",
                    SpanStyle {
                        bold: true,
                        italic: true,
                        ..SpanStyle::default()
                    }
                ),
                Span::styled(" world", SpanStyle::default()),
            ]
        );
    }

    #[test]
    fn paragraphs_separator_is_two_newlines() {
        let spans = html_to_spans("<p>one</p><p>two</p>");
        assert_eq!(
            spans,
            vec![
                Span::styled("one", SpanStyle::default()),
                Span::plain("\n\n"),
                Span::styled("two", SpanStyle::default()),
            ]
        );
    }

    #[test]
    fn br_is_single_newline_inside_style() {
        let spans = html_to_spans("<b>first<br/>second</b>");
        assert_eq!(
            spans,
            vec![
                Span::styled("first", bold()),
                Span::plain("\n"),
                Span::styled("second", bold()),
            ]
        );
    }

    #[test]
    fn entities_are_decoded() {
        let spans = html_to_spans("<p>a &amp; b &lt;c&gt; &#39;q&#39; &quot;d&quot;</p>");
        assert_eq!(
            spans,
            vec![Span::styled("a & b <c> 'q' \"d\"", SpanStyle::default())]
        );
    }

    #[test]
    fn link_href_is_captured() {
        let spans =
            html_to_spans("<p>see <a href=\"https://example.test/x?y=1&amp;z=2\">docs</a> now</p>");
        assert_eq!(
            spans,
            vec![
                Span::styled("see ", SpanStyle::default()),
                Span::styled("docs", link("https://example.test/x?y=1&z=2")),
                Span::styled(" now", SpanStyle::default()),
            ]
        );
    }

    #[test]
    fn javascript_href_is_stripped() {
        let spans = html_to_spans("<a href=\"javascript:alert(1)\">click</a>");
        assert_eq!(spans, vec![Span::styled("click", SpanStyle::default())]);
    }

    #[test]
    fn script_content_is_dropped() {
        let spans = html_to_spans("<p>ok</p><script>alert(1)</script>");
        assert_eq!(spans, vec![Span::styled("ok", SpanStyle::default())]);
    }

    #[test]
    fn headings_carry_level() {
        let spans = html_to_spans("<h2>Title</h2>");
        assert_eq!(spans, vec![Span::styled("Title", heading(2))]);
    }

    #[test]
    fn code_and_pre() {
        let spans = html_to_spans("<pre>line1\n  line2</pre>");
        assert_eq!(spans, vec![Span::styled("line1\n  line2", code())]);
    }

    #[test]
    fn inline_code_keeps_spaces() {
        let spans = html_to_spans("<p>run <code>cargo test</code> now</p>");
        assert_eq!(
            spans,
            vec![
                Span::styled("run ", SpanStyle::default()),
                Span::styled("cargo test", code()),
                Span::styled(" now", SpanStyle::default()),
            ]
        );
    }

    #[test]
    fn unordered_list_items_are_bulleted() {
        let spans = html_to_spans("<ul><li>one</li><li>two</li></ul>");
        assert_eq!(
            spans,
            vec![
                Span::styled("\u{2022} one", SpanStyle::default()),
                Span::plain("\n"),
                Span::styled("\u{2022} two", SpanStyle::default()),
            ]
        );
    }

    #[test]
    fn ordered_list_items_are_numbered() {
        let spans = html_to_spans("<ol><li>first</li><li>second</li></ol>");
        assert_eq!(
            spans,
            vec![
                Span::styled("1. first", SpanStyle::default()),
                Span::plain("\n"),
                Span::styled("2. second", SpanStyle::default()),
            ]
        );
    }

    #[test]
    fn blockquote_sets_quote_style() {
        let spans = html_to_spans("<blockquote>quoted words</blockquote>");
        assert_eq!(
            spans,
            vec![Span::styled(
                "quoted words",
                SpanStyle {
                    quote: true,
                    ..SpanStyle::default()
                }
            )]
        );
    }

    #[test]
    fn whitespace_between_blocks_collapses() {
        let spans = html_to_spans("<p>a</p>\n   <p>b</p>\n");
        assert_eq!(
            spans,
            vec![
                Span::styled("a", SpanStyle::default()),
                Span::plain("\n\n"),
                Span::styled("b", SpanStyle::default()),
            ]
        );
    }

    #[test]
    fn unknown_tag_degrades_to_plain_text() {
        let spans = html_to_spans("<marquee>hi</marquee>");
        assert_eq!(spans, vec![Span::styled("hi", SpanStyle::default())]);
    }

    #[test]
    fn strikethrough_and_underline() {
        let spans = html_to_spans("<u>under</u> <s>gone</s> <del>gone2</del>");
        assert_eq!(
            spans,
            vec![
                Span::styled(
                    "under",
                    SpanStyle {
                        underline: true,
                        ..SpanStyle::default()
                    }
                ),
                Span::styled(" ", SpanStyle::default()),
                Span::styled(
                    "gone",
                    SpanStyle {
                        strikethrough: true,
                        ..SpanStyle::default()
                    }
                ),
                Span::styled(" ", SpanStyle::default()),
                Span::styled(
                    "gone2",
                    SpanStyle {
                        strikethrough: true,
                        ..SpanStyle::default()
                    }
                ),
            ]
        );
    }

    #[test]
    fn pre_separator_after_block_is_lazy_no_trailing_newline() {
        let spans = html_to_spans("<p>only</p>");
        assert_eq!(spans, vec![Span::styled("only", SpanStyle::default())]);
    }
}
