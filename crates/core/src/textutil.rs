//! Plain-text extraction helpers.
//!
//! `html_to_text` used to live in the (deleted) demo module; it is a
//! production utility used by the AI pipeline, quality metrics and the
//! interaction engine to read the HTML bodies Help Scout stores.

/// Runaway-tag guard (clean-audit item 66): a `<` whose "tag" never closes
/// within this many bytes was never a tag — everything after it is emitted
/// as text instead of silently swallowed to end-of-input.
const MAX_TAG_BYTES: usize = 500;

/// The entity candidates `html_to_text` decodes (exactly the set the old
/// sequential `replace` chain knew — the fix is the single-pass decoder, not
/// a bigger vocabulary).
fn decode_entity(candidate: &str) -> Option<char> {
    match candidate {
        "nbsp" => Some(' '),
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "#39" => Some('\''),
        _ => None,
    }
}

/// Does the char right after a `<` begin an actual tag? (The HTML5
/// tokenizer's rule: a letter, `/`, `!` or `?`.) Anything else — `5 < 6`,
/// `a < b`, a lone trailing `<` — is literal text.
fn starts_tag(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '/' || c == '!' || c == '?'
}

/// Collapse an HTML fragment into readable plain text.
///
/// Strips tags, turns block-level closings (`</p>`, `</div>`, `</li>`,
/// headings, table rows) into newlines, inline tags into spaces, and
/// decodes the handful of entities that appear in real thread bodies.
///
/// Clean-audit item 66, two defects fixed:
///
/// * a bare `<` used to switch the scanner into tag mode and swallow the
///   rest of the body ("price <100, …", "a < b") — a `<` now starts a tag
///   only when the HTML5 tokenizer would say so (followed by a letter,
///   `/`, `!` or `?`), and even then a tag that never closes within
///   [`MAX_TAG_BYTES`] is flushed back out as text instead of eating the
///   document;
/// * entities used to decode through sequential `str::replace` calls, so
///   `&amp;lt;` became `&lt;` by the `&amp;` pass and then `<` by the
///   `&lt;` pass — a double decode of text the author had carefully
///   escaped. Decoding is now a single pass that never re-scans what it
///   emitted.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_name = String::new();
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        if in_tag {
            if c == '>' {
                in_tag = false;
                let tag = tag_name.trim().to_lowercase();
                if tag == "br"
                    || tag == "/p"
                    || tag == "/div"
                    || tag == "/li"
                    || tag.starts_with("/h")
                    || tag == "/tr"
                {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                tag_name.clear();
            } else {
                tag_name.push(c);
                if tag_name.len() > MAX_TAG_BYTES {
                    // This '<' never met a '>': it was literal text after
                    // all. Flush the swallowed bytes instead of dropping
                    // the rest of the body.
                    out.push('<');
                    out.push_str(&tag_name);
                    tag_name.clear();
                    in_tag = false;
                }
            }
        } else if c == '<' {
            match chars.peek() {
                Some(next) if starts_tag(*next) => in_tag = true,
                _ => out.push('<'),
            }
        } else if c == '&' {
            // Single-pass entity decode: consume the candidate run (at
            // most 8 chars — the longest known entity is 4), and only
            // commit the decode when a `;` terminates a KNOWN entity.
            // Anything else is emitted verbatim, '&' first, and never
            // re-scanned — `&amp;lt;` stays `&lt;`.
            let mut candidate = String::new();
            let mut decoded: Option<char> = None;
            while candidate.len() < 8 {
                match chars.peek() {
                    Some(';') => {
                        decoded = decode_entity(&candidate);
                        if decoded.is_some() {
                            chars.next();
                        }
                        break;
                    }
                    Some(next) if next.is_ascii_alphanumeric() || *next == '#' => {
                        candidate.push(*next);
                        chars.next();
                    }
                    _ => break,
                }
            }
            match decoded {
                Some(ch) => out.push(ch),
                None => {
                    out.push('&');
                    out.push_str(&candidate);
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_breaks_paragraphs() {
        let text = html_to_text("<p>Hello</p><p>World</p>");
        assert_eq!(text, " Hello\n World\n");
    }

    #[test]
    fn inline_tags_become_spaces_and_entities_decode() {
        let text = html_to_text("<b>A</b>&amp;<i>B</i>");
        assert_eq!(text, " A & B ");
    }

    #[test]
    fn br_and_list_items_break_lines() {
        let text = html_to_text("a<br>b</li>c");
        assert_eq!(text, "a\nb\nc");
    }

    #[test]
    fn a_bare_less_than_no_longer_swallows_the_rest_of_the_body() {
        // Pre-fix: the '<' switched the scanner into tag mode and
        // everything after it vanished.
        assert_eq!(
            html_to_text("price <100 USD and free shipping"),
            "price <100 USD and free shipping"
        );
        assert_eq!(html_to_text("a < b and b > a"), "a < b and b > a");
        // A '<' followed by a non-tag-start char is text even mid-word.
        assert_eq!(html_to_text("5<6? yes"), "5<6? yes");
        // A trailing lone '<' is text, not a hungry tag.
        assert_eq!(html_to_text("ends with <"), "ends with <");
    }

    #[test]
    fn a_never_closing_tag_runaway_is_flushed_as_text() {
        // '<' + letter IS a plausible tag opening, but no '>' ever arrives;
        // the runaway guard flushes it instead of swallowing the document.
        let body = format!("start <div class=\"{}\" end of body", "x".repeat(600));
        let text = html_to_text(&body);
        assert!(
            text.contains("start"),
            "text before the '<' survives: {text:?}"
        );
        assert!(
            text.contains("end of body"),
            "text after the runaway survives: {text:?}"
        );
        assert!(text.contains('<'), "the literal '<' survives: {text:?}");
        assert!(
            text.contains("x"),
            "the swallowed bytes come back out: {text:?}"
        );
    }

    #[test]
    fn entities_decode_exactly_once() {
        // Pre-fix: the sequential replace chain decoded &amp;lt; to < (twice).
        assert_eq!(html_to_text("&amp;lt;"), "&lt;");
        assert_eq!(html_to_text("&amp;amp;"), "&amp;");
        assert_eq!(html_to_text("&amp;#39;"), "&#39;");
    }

    #[test]
    fn unrecognized_entity_runs_stay_verbatim() {
        assert_eq!(html_to_text("&foo;"), "&foo;");
        assert_eq!(html_to_text("a & b"), "a & b");
        assert_eq!(html_to_text("&#x27;"), "&#x27;");
        assert_eq!(
            html_to_text("&thisiswaytoolongtobeanentity;"),
            "&thisiswaytoolongtobeanentity;"
        );
    }

    #[test]
    fn known_entities_decode_as_before() {
        assert_eq!(html_to_text("&nbsp;"), " ");
        assert_eq!(html_to_text("&lt;&gt;"), "<>");
        assert_eq!(html_to_text("&quot;&#39;"), "\"'");
        assert_eq!(html_to_text("&amp;"), "&");
    }
}
