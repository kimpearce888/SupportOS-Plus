//! AI memory quarantine guard (promoted from the retired axum
//! `http/routes/memory.rs` at the Phase 4 cutover).
//!
//! Personality, temperament, and mental-health adjacent memory entries are
//! quarantined: they never feed AI prompts or surface in tool results.
//! Pure string logic — the AI tools and the memory routes share it.

const QUARANTINE_WORDS: [&str; 16] = [
    "personality",
    "temperament",
    "mental health",
    "emotional state",
    "emotional instability",
    "cognitive trait",
    "mood disorder",
    "anxiety disorder",
    "bipolar",
    "diagnosis",
    "diagnosed",
    "diagnoses",
    "depressed",
    "depression",
    "intelligence level",
    "intelligence score",
];

const QUARANTINE_PREFIXES: [&str; 8] = [
    "psycholog",
    "introvert",
    "extrovert",
    "extravert",
    "neurotic",
    "narcissis",
    "schizo",
    "autis",
];

fn is_js_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn has_lower_word(hay: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(idx) = hay[start..].find(needle) {
        let at = start + idx;
        let before_ok = !hay[..at].chars().next_back().is_some_and(is_js_word_char);
        let after = at + needle.len();
        let after_ok = !hay[after..].chars().next().is_some_and(is_js_word_char);
        if before_ok && after_ok {
            return true;
        }
        start = at + needle.len();
    }
    false
}

fn has_lower_prefix(hay: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(idx) = hay[start..].find(needle) {
        let at = start + idx;
        if !hay[..at].chars().next_back().is_some_and(is_js_word_char) {
            return true;
        }
        start = at + needle.len();
    }
    false
}

fn has_case_word(text: &str, word: &str) -> bool {
    let mut start = 0;
    while let Some(idx) = text[start..].find(word) {
        let at = start + idx;
        let before_ok = !text[..at].chars().next_back().is_some_and(is_js_word_char);
        let after = at + word.len();
        let after_ok = !text[after..].chars().next().is_some_and(is_js_word_char);
        if before_ok && after_ok {
            return true;
        }
        start = at + word.len();
    }
    false
}

pub fn is_quarantined(key: &str, value: Option<&str>) -> bool {
    let text = value.map_or(key.to_string(), |v| format!("{key} {v}"));
    let key_lower = key.to_lowercase();
    let text_lower = text.to_lowercase();
    for w in QUARANTINE_WORDS {
        if has_lower_word(&key_lower, w) || has_lower_word(&text_lower, w) {
            return true;
        }
    }
    for p in QUARANTINE_PREFIXES {
        if has_lower_prefix(&key_lower, p) || has_lower_prefix(&text_lower, p) {
            return true;
        }
    }
    has_case_word(key, "IQ") || has_case_word(&text, "IQ") || has_case_word(&text, "EQ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personality_and_health_terms_are_quarantined() {
        assert!(is_quarantined("personality", None));
        assert!(is_quarantined(
            "note",
            Some("patient has a diagnosis of ...")
        ));
        assert!(is_quarantined("psychological profile", None));
        // Word-boundary matching: "custom" must not trip "autis".
        assert!(!is_quarantined("custom", None));
    }

    #[test]
    fn ordinary_entries_pass() {
        assert!(!is_quarantined("plan", Some("annual plan")));
        assert!(!is_quarantined("email", Some("kim@example.com")));
        // IQ is a case-sensitive whole word.
        assert!(is_quarantined("notes", Some("IQ test result")));
        assert!(!is_quarantined("notes", Some("iq test result")));
    }
}
