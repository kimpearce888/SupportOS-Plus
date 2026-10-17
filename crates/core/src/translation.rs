use std::collections::HashMap;

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::Result;

pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("ru", "Russian"),
    ("zh", "Chinese"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("ar", "Arabic"),
    ("hi", "Hindi"),
    ("th", "Thai"),
    ("vi", "Vietnamese"),
    ("pl", "Polish"),
    ("tr", "Turkish"),
    ("sv", "Swedish"),
];

pub fn language_name(code: &str) -> String {
    SUPPORTED_LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| code.to_string())
}

fn is_supported(code: &str) -> bool {
    SUPPORTED_LANGUAGES.iter().any(|(c, _)| *c == code)
}

pub fn ensure_translation_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS translation_cache (
            cache_key        TEXT PRIMARY KEY,
            source_lang      TEXT NOT NULL,
            target_lang      TEXT NOT NULL,
            purpose          TEXT NOT NULL DEFAULT 'general'
                CHECK (purpose IN ('customer_inbound','agent_draft','general')),
            source_text      TEXT NOT NULL,
            translated_text  TEXT NOT NULL,
            model            TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_translation_cache_created
            ON translation_cache(created_at DESC);",
    )?;
    Ok(())
}

struct ScriptRange {
    code: &'static str,
    ranges: &'static [(u32, u32)],
    confidence: &'static str,
}

static SCRIPT_RANGES: &[ScriptRange] = &[
    ScriptRange {
        code: "ko",
        ranges: &[(0xac00, 0xd7a3)],
        confidence: "medium",
    },
    ScriptRange {
        code: "ja",
        ranges: &[(0x3040, 0x30ff)],
        confidence: "medium",
    },
    ScriptRange {
        code: "zh",
        ranges: &[(0x4e00, 0x9fff), (0x3400, 0x4dbf)],
        confidence: "low",
    },
    ScriptRange {
        code: "ru",
        ranges: &[(0x0400, 0x04ff)],
        confidence: "high",
    },
    ScriptRange {
        code: "el",
        ranges: &[(0x0370, 0x03ff)],
        confidence: "high",
    },
    ScriptRange {
        code: "he",
        ranges: &[(0x0590, 0x05ff)],
        confidence: "high",
    },
    ScriptRange {
        code: "ar",
        ranges: &[(0x0600, 0x06ff)],
        confidence: "high",
    },
    ScriptRange {
        code: "hi",
        ranges: &[(0x0900, 0x097f)],
        confidence: "high",
    },
    ScriptRange {
        code: "th",
        ranges: &[(0x0e00, 0x0e7f)],
        confidence: "high",
    },
];

static STOPWORDS: &[(&str, &[&str])] = &[
    (
        "en",
        &[
            "the", "and", "is", "are", "was", "you", "for", "with", "that", "this", "have", "not",
            "but", "can", "how", "what", "when", "why", "please", "thank", "we", "our", "your",
            "it", "my", "do", "does", "did", "has", "had", "will", "would", "could", "should",
            "from", "about",
        ],
    ),
    (
        "es",
        &[
            "el", "la", "los", "las", "de", "que", "y", "en", "un", "una", "por", "con", "para",
            "no", "se", "lo", "su", "más", "está", "estoy", "hola", "gracias", "cómo", "qué",
            "cuando", "dónde", "puedo", "necesito", "favor",
        ],
    ),
    (
        "fr",
        &[
            "le", "la", "les", "de", "des", "et", "en", "un", "une", "du", "que", "qui", "pour",
            "avec", "dans", "pas", "est", "je", "vous", "nous", "merci", "bonjour", "comment",
            "pourquoi", "peux", "avez", "être",
        ],
    ),
    (
        "de",
        &[
            "der", "die", "das", "und", "ist", "nicht", "mit", "für", "ein", "eine", "auf", "von",
            "ich", "sie", "wir", "und", "auch", "als", "wie", "danke", "bitte", "hallo", "können",
            "haben", "sehr", "warum",
        ],
    ),
    (
        "it",
        &[
            "il", "lo", "la", "le", "di", "che", "e", "in", "un", "una", "per", "con", "non",
            "sono", "ho", "mi", "si", "come", "grazie", "ciao", "perché", "quando", "posso",
            "molto", "anche", "dove",
        ],
    ),
    (
        "pt",
        &[
            "o", "a", "os", "as", "de", "que", "e", "em", "um", "uma", "para", "com", "não", "por",
            "do", "da", "estou", "você", "obrigado", "olá", "como", "por", "quando", "posso",
            "muito", "também",
        ],
    ),
    (
        "nl",
        &[
            "de", "het", "een", "en", "van", "is", "dat", "niet", "met", "voor", "ik", "wij",
            "jullie", "heb", "hebben", "kan", "niet", "dank", "hallo", "hoe", "waarom", "want",
            "ook", "maar", "nog", "wel",
        ],
    ),
    (
        "pl",
        &[
            "nie",
            "jest",
            "się",
            "na",
            "że",
            "do",
            "mam",
            "jak",
            "ale",
            "czy",
            "dziękuję",
            "cześć",
            "dlaczego",
            "kiedy",
            "można",
            "bardzo",
            "proszę",
            "jeśli",
            "tego",
            "dla",
            "od",
            "przy",
            "bez",
        ],
    ),
    (
        "tr",
        &[
            "bir",
            "ve",
            "bu",
            "için",
            "ile",
            "değil",
            "mi",
            "my",
            "nasıl",
            "teşekkür",
            "merhaba",
            "neden",
            "ne",
            "zaman",
            "olabilir",
            "çok",
            "ama",
            "gerekli",
            "lütfen",
            "var",
            "yok",
            "olarak",
        ],
    ),
    (
        "sv",
        &[
            "och", "att", "det", "en", "som", "är", "för", "med", "inte", "har", "den", "jag",
            "vi", "ni", "tack", "hej", "hur", "varför", "när", "kan", "mycket", "också", "men",
            "om",
        ],
    ),
    (
        "vi",
        &[
            "của",
            "và",
            "là",
            "có",
            "không",
            "được",
            "cho",
            "với",
            "này",
            "tôi",
            "bạn",
            "chúng",
            "cảm ơn",
            "xin",
            "làm",
            "thế nào",
            "tại sao",
            "khi",
            "có thể",
            "rất",
            "nhưng",
        ],
    ),
];

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round() / factor
}

fn is_letterish(c: char) -> bool {
    c.is_ascii_alphanumeric() || (u32::from(c) >= 0xc0 && u32::from(c) <= 0xffff)
}

fn clean_for_detection(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let lower = text.to_lowercase();
    let chars: Vec<char> = text.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if lower_chars[i] == 'h' && lower_chars[i..].starts_with(&['h', 't', 't', 'p']) {
            let rest: String = lower_chars[i..].iter().collect();
            if rest.starts_with("http://") || rest.starts_with("https://") {
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                out.push(' ');
                continue;
            }
        }
        if chars[i] == '`'
            && chars[i..].starts_with(&['`', '`', '`'])
            && let Some(close) = find_fence_close(&chars, i + 3)
        {
            i = close + 3;
            out.push(' ');
            continue;
        }
        if chars[i] == '@'
            && let Some((start, end)) = email_span(&chars, i)
        {
            let _ = start;
            i = end;
            out.push(' ');
            continue;
        }
        if chars[i].is_ascii_hexdigit() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_hexdigit() {
                j += 1;
            }
            if j - i >= 8 {
                let before_ok = i == 0 || !is_word_char(chars[i - 1]);
                let after_ok = j >= chars.len() || !is_word_char(chars[j]);
                if before_ok && after_ok {
                    i = j;
                    out.push(' ');
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn find_fence_close(chars: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 2 < chars.len() {
        if chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn email_span(chars: &[char], at: usize) -> Option<(usize, usize)> {
    let mut start = at;
    while start > 0 {
        let c = chars[start - 1];
        if is_word_char(c) || c == '.' || c == '+' || c == '-' {
            start -= 1;
        } else {
            break;
        }
    }
    if start == at {
        return None;
    }
    let mut i = at + 1;
    while i < chars.len() && (is_word_char(chars[i]) || chars[i] == '-') {
        i += 1;
    }
    if i == at + 1 || i >= chars.len() || chars[i] != '.' {
        return None;
    }
    let dot = i;
    let _ = dot;
    i += 1;
    let mut tld_chars = 0;
    while i < chars.len() && (is_word_char(chars[i]) || chars[i] == '.') {
        i += 1;
        tld_chars += 1;
    }
    if tld_chars == 0 {
        return None;
    }
    Some((start, i))
}

pub fn detect_language(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return json!({
            "text": trimmed,
            "code": Value::Null,
            "name": Value::Null,
            "confidence": "unknown",
            "method": "empty",
            "alternatives": [],
            "note": "No text to analyze."
        });
    }
    let cleaned = clean_for_detection(trimmed);
    let chars: Vec<char> = cleaned.chars().collect();
    let letter_count = chars.iter().filter(|c| is_letterish(**c)).count();
    if letter_count < 3 {
        return json!({
            "text": trimmed,
            "code": Value::Null,
            "name": Value::Null,
            "confidence": "unknown",
            "method": "empty",
            "alternatives": [],
            "note": "Not enough recognizable characters to detect a language."
        });
    }

    let mut script_scores: Vec<(&str, f64, &str)> = Vec::new();
    for s in SCRIPT_RANGES {
        let mut hits = 0usize;
        for ch in &chars {
            let cp = u32::from(*ch);
            if s.ranges.iter().any(|(lo, hi)| cp >= *lo && cp <= *hi) {
                hits += 1;
            }
        }
        if hits > 0 {
            script_scores.push((s.code, hits as f64 / letter_count as f64, s.confidence));
        }
    }
    script_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(&(code, share, confidence)) = script_scores.first()
        && share >= 0.15
    {
        let is_ambiguous_han = code == "zh";
        let confidence = if is_ambiguous_han {
            "low"
        } else if share >= 0.4 {
            confidence
        } else {
            "medium"
        };
        let alternatives: Vec<Value> = script_scores[1..3.min(script_scores.len())]
                .iter()
                .map(|&(c, s, _)| {
                    json!({ "code": c, "name": language_name(c), "score": round_to(s, 2) })
                })
                .collect();
        let note = if is_ambiguous_han {
            "Han script detected (Chinese/Japanese share it); reported as Chinese with low confidence - confirm before relying on it.".to_string()
        } else {
            format!(
                "Detected by Unicode script range analysis ({}% of letters).",
                (share * 100.0).round() as i64
            )
        };
        return json!({
            "text": trimmed,
            "code": code,
            "name": language_name(code),
            "confidence": confidence,
            "method": "script",
            "alternatives": alternatives,
            "note": note
        });
    }

    let words = stopwords_from(&cleaned);
    let mut scored: Vec<(&str, String, f64, usize)> = STOPWORDS
        .iter()
        .map(|&(code, list)| {
            let hits = words.iter().filter(|w| list.iter().any(|s| s == w)).count();
            let score = if words.is_empty() {
                0.0
            } else {
                hits as f64 / (words.len().max(4)) as f64
            };
            (code, language_name(code), score, hits)
        })
        .collect();
    scored.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.3.cmp(&a.3))
    });
    let best = scored.first();
    let second = scored.get(1);
    let Some(&(best_code, ref best_name, best_score, best_hits)) = best else {
        return unknown_stopwords(trimmed);
    };
    if best_hits == 0 {
        return unknown_stopwords(trimmed);
    }
    let separation = best_score - second.map(|s| s.2).unwrap_or(0.0);
    let confidence = if best_score >= 0.08 && separation >= 0.03 {
        "high"
    } else if best_score >= 0.05 {
        "medium"
    } else {
        "low"
    };
    let alternatives: Vec<Value> = scored[1..3.min(scored.len())]
        .iter()
        .map(|&(c, ref name, s, _)| json!({ "code": c, "name": name, "score": round_to(s, 3) }))
        .collect();
    let note = if confidence == "high" {
        format!("Detected by function-word frequency ({best_hits} matching words).")
    } else if confidence == "medium" {
        format!(
            "Detected by function-word frequency with limited separation from {}.",
            second
                .map(|s| s.1.clone())
                .unwrap_or_else(|| "other candidates".into())
        )
    } else {
        "Weak function-word signal; treat the language as a low-confidence guess.".to_string()
    };
    json!({
        "text": trimmed,
        "code": best_code,
        "name": best_name,
        "confidence": confidence,
        "method": "stopwords",
        "alternatives": alternatives,
        "note": note
    })
}

fn unknown_stopwords(trimmed: &str) -> Value {
    json!({
        "text": trimmed,
        "code": Value::Null,
        "name": Value::Null,
        "confidence": "unknown",
        "method": "stopwords",
        "alternatives": [],
        "note": "No language-specific function words matched; the language is honestly unknown."
    })
}

fn stopwords_from(cleaned: &str) -> Vec<String> {
    let lowered = cleaned.to_lowercase();
    let mut words = Vec::new();
    let mut current = String::new();
    for c in lowered.chars() {
        let keep = c.is_ascii_lowercase()
            || c.is_ascii_digit()
            || c.is_whitespace()
            || matches!(c, 'à'..='ö' | 'ø'..='ÿ' | '\u{0101}'..='\u{017e}');
        if keep {
            if c.is_whitespace() {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            } else {
                current.push(c);
            }
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

pub fn detect(texts: &[String]) -> Vec<Value> {
    texts.iter().take(50).map(|t| detect_language(t)).collect()
}

pub fn conversation_languages(conn: &Connection, conversation_id: i64) -> Result<Option<Value>> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok();
    if exists.is_none() {
        return Ok(None);
    }
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, html_stripped FROM (
             SELECT id, COALESCE(body_html, body_text, '') AS html_stripped,
                    COALESCE(remote_created_at, created_at) AS at
             FROM conversation_threads
             WHERE conversation_id = ?1 AND type = 'customer'
               AND deleted_at IS NULL AND state = 'published'
         ) ORDER BY at ASC LIMIT 100",
    ) else {
        return Ok(Some(conversation_summary(conversation_id, Vec::new())));
    };
    let per_message: Vec<(i64, Value)> = stmt
        .query_map(params![conversation_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .map(|(id, html)| {
                    let text = crate::demo::html_to_text(&html);
                    (id, detect_language(&text))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(conversation_summary(conversation_id, per_message)))
}

fn conversation_summary(conversation_id: i64, per_message: Vec<(i64, Value)>) -> Value {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (_, detection) in &per_message {
        if let (Some(code), Some(conf)) = (
            detection.get("code").and_then(|v| v.as_str()),
            detection.get("confidence").and_then(|v| v.as_str()),
        ) && conf != "unknown"
        {
            *counts.entry(code.to_string()).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let primary = ranked.first();
    let primary_detection: Option<&Value> = per_message
        .iter()
        .find(|(_, d)| d.get("code").and_then(|v| v.as_str()) == primary.map(|(c, _)| c.as_str()))
        .map(|(_, d)| d);
    let mut notes = vec![
        "Detection is deterministic (script ranges + function words) and runs entirely locally."
            .to_string(),
    ];
    if primary.is_none() {
        notes.push("No customer message had a detectable language.".to_string());
    }
    if !per_message.is_empty() && ranked.len() > 1 {
        let listing = ranked
            .iter()
            .map(|(c, n)| format!("{} x{}", language_name(c), n))
            .collect::<Vec<_>>()
            .join(", ");
        notes.push(format!(
            "Multiple languages detected ({listing}); the primary is the most frequent."
        ));
    }
    let (p_code, p_name, p_conf, p_method) = if let Some((code, _)) = primary {
        let detection = primary_detection;
        (
            json!(code),
            json!(language_name(code)),
            detection
                .and_then(|d| d.get("confidence"))
                .cloned()
                .unwrap_or(json!("unknown")),
            detection
                .and_then(|d| d.get("method"))
                .cloned()
                .unwrap_or(json!("empty")),
        )
    } else {
        (Value::Null, Value::Null, json!("unknown"), json!("empty"))
    };
    json!({
        "conversation_id": conversation_id,
        "customer_messages": per_message.len(),
        "analyzed_messages": per_message.len(),
        "primary_language": {
            "code": p_code,
            "name": p_name,
            "confidence": p_conf,
            "method": p_method
        },
        "per_message": per_message
            .iter()
            .map(|(id, d)| json!({ "thread_id": id, "detection": d }))
            .collect::<Vec<_>>(),
        "notes": notes
    })
}

#[derive(Debug)]
pub enum TranslateError {
    Client(String),
    Service(String),
    BadGateway(String),
}

fn purpose_prompt(purpose: &str) -> &'static str {
    match purpose {
        "customer_inbound" => {
            "Translate the CUSTOMER support message into the target language for an agent to review."
        }
        "agent_draft" => {
            "Translate the AGENT draft reply into the target language the customer asked to be served in."
        }
        _ => "Translate the text into the target language.",
    }
}

pub type ChatFn<'a> = &'a dyn Fn(
    Vec<crate::ai_provider::ChatMessage>,
    f64,
    u32,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = std::result::Result<(Option<String>, String), String>>
            + 'a,
    >,
>;

pub async fn translate(
    conn: &Connection,
    chat: ChatFn<'_>,
    text: &str,
    from: Option<&str>,
    to: &str,
    purpose: Option<&str>,
) -> std::result::Result<Value, TranslateError> {
    let text = text.trim();
    let purpose = purpose.unwrap_or("general");
    if !is_supported(to) {
        return Err(TranslateError::Client(format!(
            "Unsupported target language '{to}'. Supported: {}.",
            SUPPORTED_LANGUAGES
                .iter()
                .map(|(c, _)| *c)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if text.is_empty() {
        return Err(TranslateError::BadGateway(
            "Nothing to translate: empty text.".into(),
        ));
    }
    if text.chars().count() > 8000 {
        return Err(TranslateError::BadGateway(
            "Text too long for a single local translation (8000 char cap).".into(),
        ));
    }

    let mut detected: Option<Value> = None;
    let mut source_lang = from.unwrap_or("").trim().to_lowercase();
    if source_lang.is_empty() || source_lang == "auto" {
        let detection = detect_language(text);
        let code = detection
            .get("code")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        source_lang = code.clone().unwrap_or_else(|| "en".into());
        if code.is_none() {
            return Err(TranslateError::Client(
                "Source language could not be detected; please specify it explicitly.".into(),
            ));
        }
        detected = Some(detection);
    }
    if source_lang == to {
        return Err(TranslateError::Client(format!(
            "Source and target language are both '{to}' - nothing to translate."
        )));
    }

    let mut hasher = Sha256::new();
    hasher.update(format!("{source_lang}|{to}|{purpose}|{text}"));
    let cache_key = crate::crypto::hex(&hasher.finalize());
    let cached: Option<(String, String, String, Option<String>, String)> = conn
        .query_row(
            "SELECT source_lang, target_lang, translated_text, model, purpose
             FROM translation_cache WHERE cache_key = ?1",
            params![cache_key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .ok();
    if let Some((src, tgt, translated, model, purpose)) = cached {
        return Ok(json!({
            "source_lang": src,
            "source_lang_name": language_name(&src),
            "target_lang": tgt,
            "target_lang_name": language_name(&tgt),
            "purpose": purpose,
            "source_text": text,
            "translated_text": translated,
            "model": model,
            "cached": true,
            "detected": detected,
            "note": "Served from the local translation cache - the model was not re-run. Review original and translation side by side; nothing is sent automatically."
        }));
    }

    let system = [
        format!("You are a precise translation engine. {}", purpose_prompt(purpose)),
        format!("Translate from {} to {}.", language_name(&source_lang), language_name(to)),
        "Rules:".to_string(),
        "- Preserve technical terms, product names, feature names, error messages, code, URLs and email addresses VERBATIM (do not translate them).".to_string(),
        "- Keep the original meaning and tone; do not add, remove or answer any content.".to_string(),
        "- Keep line breaks and list structure.".to_string(),
        "- Output ONLY the translated text, no explanations, no quotes.".to_string(),
    ]
    .join("\n");
    let messages = vec![
        crate::ai_provider::ChatMessage {
            role: "system".into(),
            content: system,
        },
        crate::ai_provider::ChatMessage {
            role: "user".into(),
            content: text.to_string(),
        },
    ];
    let (content, model) = chat(messages, 0.1, 2048)
        .await
        .map_err(TranslateError::Service)?;
    let translated = content.unwrap_or_default().trim().to_string();
    if translated.is_empty() {
        return Err(TranslateError::BadGateway(
            "The local model returned an empty translation.".into(),
        ));
    }

    conn.execute(
        "INSERT INTO translation_cache (cache_key, source_lang, target_lang, purpose, source_text, translated_text, model, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT (cache_key) DO UPDATE SET translated_text = excluded.translated_text, model = excluded.model",
        params![cache_key, source_lang, to, purpose, text, translated, model],
    )
    .map_err(|e| TranslateError::BadGateway(e.to_string()))?;

    Ok(json!({
        "source_lang": source_lang,
        "source_lang_name": language_name(&source_lang),
        "target_lang": to,
        "target_lang_name": language_name(to),
        "purpose": purpose,
        "source_text": text,
        "translated_text": translated,
        "model": model,
        "cached": false,
        "detected": detected,
        "note": "Translated by the locally configured model (LM Studio) - no cloud service. Review original and translation side by side; nothing is sent automatically."
    }))
}

pub fn agent_language(conn: &Connection) -> Value {
    let raw = crate::settings::get_string(conn, "agent_language")
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut code = "en".to_string();
    if !raw.is_empty() {
        let parsed = serde_json::from_str::<Value>(&raw)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| raw.clone());
        if is_supported(&parsed) {
            code = parsed;
        }
    }
    json!({ "code": code, "name": language_name(&code) })
}
