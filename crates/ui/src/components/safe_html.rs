use leptos::prelude::*;

#[must_use]
pub fn split_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(str::to_string)
        .collect()
}

#[must_use]
pub fn looks_like_html(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'<' && bytes[i + 1].is_ascii_alphabetic() {
            for &b in &bytes[i + 2..(i + 120).min(bytes.len())] {
                if b == b'>' {
                    return true;
                }
                if b == b'<' {
                    break;
                }
            }
        }
        i += 1;
    }
    false
}

#[component]
pub fn SafeHtml(html: Option<String>, fallback_text: String) -> impl IntoView {
    let raw = html.unwrap_or_default();
    let trimmed = raw.trim();
    if !trimmed.is_empty() && looks_like_html(trimmed) {
        view! { <div class="spp-thread-body" inner_html=raw></div> }.into_any()
    } else {
        let text = if trimmed.is_empty() {
            fallback_text
        } else {
            raw.clone()
        };
        view! {
            <div class="spp-thread-body">
                {split_paragraphs(&text)
                    .into_iter()
                    .map(|p| view! { <p>{p}</p> })
                    .collect::<Vec<_>>()}
            </div>
        }
        .into_any()
    }
}
