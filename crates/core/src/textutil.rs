const MAX_TAG_BYTES: usize = 500;

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

fn starts_tag(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '/' || c == '!' || c == '?'
}

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
        assert_eq!(
            html_to_text("price <100 USD and free shipping"),
            "price <100 USD and free shipping"
        );
        assert_eq!(html_to_text("a < b and b > a"), "a < b and b > a");
        assert_eq!(html_to_text("5<6? yes"), "5<6? yes");
        assert_eq!(html_to_text("ends with <"), "ends with <");
    }

    #[test]
    fn a_never_closing_tag_runaway_is_flushed_as_text() {
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
