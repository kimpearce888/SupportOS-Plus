//! Plain-text extraction helpers.
//!
//! `html_to_text` used to live in the (deleted) demo module; it is a
//! production utility used by the AI pipeline, quality metrics and the
//! interaction engine to read the HTML bodies Help Scout stores.

/// Collapse an HTML fragment into readable plain text.
///
/// Strips tags, turns block-level closings (`</p>`, `</div>`, `</li>`,
/// headings, table rows) into newlines, inline tags into spaces, and
/// decodes the handful of entities that appear in real thread bodies.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_name = String::new();
    for c in html.chars() {
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
            }
        } else if c == '<' {
            in_tag = true;
        } else {
            out.push(c);
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
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
}
