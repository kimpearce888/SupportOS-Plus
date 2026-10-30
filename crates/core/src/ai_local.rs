//! Built-in local intelligence — the zero-configuration Copilot.
//!
//! Every generative path in this app (ticket analysis, draft replies,
//! copilot chat) silently required a running LM Studio server with
//! hand-typed model names; with nothing configured the AI features
//! looked completely dead. This module gives the conversation view a
//! local, offline, deterministic analysis engine so "read the entire
//! conversation, check previous conversations for the same issue,
//! suggest an answer" works on a fresh install with nothing but synced
//! Help Scout data:
//!
//! * extractive summary + key questions from the customer's messages,
//! * intent / urgency / sentiment / escalation signals (keyword markers),
//! * "same issue before" via [`crate::ai_evidence::find_similar`] with
//!   resolution excerpts,
//! * relevant Help Center articles and knowledge-base snippets,
//! * an evidence-grounded draft reply assembled from the best past
//!   resolution (never a hallucination — every line cites its source),
//! * the customer's history snapshot.
//!
//! When LM Studio IS configured, the UI offers the deeper generative
//! passes on top (analyze_ticket / generate_draft jobs); this engine is
//! the always-on floor, not the ceiling.

use rusqlite::{Connection, params};

use crate::ai_evidence::SimilarConversation;
use crate::error::Result;

// ---------------------------------------------------------------------------
// Marker lists (kept local and compact; mirrors of the deterministic
// attribute vocabulary, scoped to what the copilot panel displays).
// ---------------------------------------------------------------------------

const URGENCY_MARKERS: &[&str] = &[
    "asap",
    "urgent",
    "urgently",
    "immediately",
    "emergency",
    "critical",
    "production is down",
    "outage",
    "cannot work",
    "blocked",
    "deadline",
    "today",
    "right now",
    "losing money",
    "losing customers",
];

const FRUSTRATION_MARKERS: &[&str] = &[
    "unacceptable",
    "frustrated",
    "frustrating",
    "ridiculous",
    "angry",
    "furious",
    "terrible",
    "worst",
    "again",
    "still not working",
    "third time",
    "multiple times",
    "nobody responded",
    "no response",
    "last chance",
    "cancel my account",
    "cancel our account",
    "requesting a refund",
    "disappointed",
    "fed up",
];

const ESCALATION_MARKERS: &[&str] = &[
    "speak to a manager",
    "speak to a human",
    "supervisor",
    "legal",
    "lawyer",
    "chargeback",
    "bbb",
    "better business",
    "twitter",
    "complaint",
    "escalate",
    "escalation",
];

const INTENT_RULES: &[(&str, &[&str])] = &[
    (
        "Billing question",
        &[
            "invoice",
            "billing",
            "charge",
            "charged",
            "payment",
            "refund",
            "subscription",
            "price",
            "pricing",
            "plan",
        ],
    ),
    (
        "Bug report",
        &[
            "bug",
            "broken",
            "error",
            "crash",
            "crashed",
            "fails",
            "failed",
            "failing",
            "doesn't work",
            "not working",
            "issue",
        ],
    ),
    (
        "How-to question",
        &[
            "how do i",
            "how to",
            "can i",
            "is it possible",
            "where can i",
            "what's the difference",
            "setup",
            "configure",
        ],
    ),
    (
        "Access problem",
        &[
            "login",
            "log in",
            "password",
            "sign in",
            "locked out",
            "access",
            "permission",
            "unauthorized",
            "403",
            "sso",
        ],
    ),
    (
        "Feature request",
        &[
            "feature request",
            "would be great if",
            "can you add",
            "roadmap",
            "upcoming",
            "please add",
            "wish you had",
        ],
    ),
    (
        "Cancellation",
        &[
            "cancel",
            "close my account",
            "downgrade",
            "unsubscribe",
            "terminate",
        ],
    ),
];

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "your", "with", "this", "that", "have",
    "from", "our", "was", "has", "had", "will", "would", "could", "should", "they", "them",
    "their", "there", "here", "what", "when", "where", "which", "while", "about", "into", "onto",
    "just", "very", "much", "many", "some", "any", "all", "can", "get", "got", "please", "thanks",
    "thank", "hello", "hi", "hey", "regards", "best", "also", "been", "being", "were", "does",
    "did", "doing", "how", "why", "who", "whom", "its", "it's", "his", "her", "she", "him", "she",
];

// ---------------------------------------------------------------------------
// Report model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct LocalAnalysis {
    /// 2-4 extractive sentences summarizing what the customer wants.
    pub summary: Vec<String>,
    /// Explicit questions the customer asked.
    pub questions: Vec<String>,
    /// Best-guess intent label (see INTENT_RULES).
    pub intent: String,
    /// low | medium | high.
    pub urgency: String,
    /// negative | neutral | positive.
    pub sentiment: String,
    pub escalation: bool,
    /// Top discriminating keywords (for search and the panel chips).
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RelatedArticle {
    pub article_id: i64,
    pub name: String,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct CustomerHistory {
    pub previous_conversations: i64,
    pub first_contact: Option<String>,
    pub last_contact: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CopilotReport {
    pub analysis: LocalAnalysis,
    pub similar: Vec<SimilarConversation>,
    pub articles: Vec<RelatedArticle>,
    /// Evidence-grounded draft reply; every line traces to a real past
    /// resolution or a real article.
    pub suggested_reply: String,
    pub history: CustomerHistory,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 2 && !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn count_markers(text_lower: &str, markers: &[&str]) -> usize {
    markers.iter().filter(|m| text_lower.contains(*m)).count()
}

fn top_keywords(text: &str, limit: usize) -> Vec<String> {
    let mut freq: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for token in tokenize(text) {
        if STOPWORDS.contains(&token.as_str()) {
            continue;
        }
        *freq.entry(token).or_default() += 1;
    }
    let mut ranked: Vec<(String, usize)> = freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().take(limit).map(|(w, _)| w).collect()
}

fn sentences(text: &str) -> Vec<String> {
    text.split(['\n', '.', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| s.chars().count() > 12)
        .map(|s| {
            // Keep the terminal punctuation so questions stay questions.
            if s.chars().count() < 400 {
                s.to_string()
            } else {
                s.chars().take(400).collect()
            }
        })
        .collect()
}

/// The local, always-on analysis of one conversation.
pub fn analyze(conn: &Connection, conversation_local_id: i64) -> Result<LocalAnalysis> {
    // Customer messages drive the analysis; agent messages only add
    // context keywords.
    let mut stmt = conn.prepare(
        "SELECT type, COALESCE(body_text, '') FROM conversation_threads
          WHERE conversation_id = ?1 AND deleted_at IS NULL
          ORDER BY COALESCE(remote_created_at, created_at) ASC, id ASC",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map(params![conversation_local_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        // RS-4-2 (G4-rejection fix): the mirror stores the source HTML
        // (Help Scout import) — the reader renders it through
        // `richtext::html_to_spans`, but the analyzer previously sliced
        // the raw body, so `<br>`/`<p>` leaked into the copilot's summary
        // and questions. Same pipeline, plain-text projection.
        .map(|(t, b): (String, String)| (t, crate::richtext::strip_html(&b)))
        .collect();

    let customer_text: String = rows
        .iter()
        .filter(|(t, _)| t == "customer")
        .map(|(_, b)| b.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let all_text: String = rows
        .iter()
        .map(|(_, b)| b.clone())
        .collect::<Vec<_>>()
        .join("\n");

    let customer_lower = customer_text.to_lowercase();
    let keywords = top_keywords(&format!("{all_text} {customer_text}"), 8);

    // Questions: customer sentences that ask something.
    let mut questions: Vec<String> = Vec::new();
    for s in sentences(&customer_text) {
        let lower = s.to_lowercase();
        let asks = s.ends_with('?')
            || lower.starts_with("how do i")
            || lower.starts_with("how to")
            || lower.starts_with("can you")
            || lower.starts_with("is there")
            || lower.starts_with("when will")
            || lower.starts_with("will you");
        if asks && questions.len() < 5 {
            questions.push(s.trim_end_matches('?').to_string());
        }
    }

    // Signals.
    let urgency_hits = count_markers(&customer_lower, URGENCY_MARKERS);
    let frustration_hits = count_markers(&customer_lower, FRUSTRATION_MARKERS);
    let escalation = count_markers(&customer_lower, ESCALATION_MARKERS) > 0;
    let positive = [
        "thanks",
        "thank you",
        "appreciate",
        "great",
        "awesome",
        "excellent",
    ]
    .iter()
    .filter(|m| customer_lower.contains(*m))
    .count();
    let urgency = if urgency_hits >= 2 {
        "high"
    } else if urgency_hits == 1 || frustration_hits >= 3 {
        "medium"
    } else {
        "low"
    };
    let sentiment = if frustration_hits + usize::from(escalation) > positive + 1 {
        "negative"
    } else if positive > frustration_hits {
        "positive"
    } else {
        "neutral"
    };

    // Intent: the rule with the most distinct keyword hits wins (ties
    // keep declaration order) — a bug report that happens to mention an
    // invoice must not classify as billing.
    let mut intent = "General question".to_string();
    let mut best_hits = 0usize;
    for (label, words) in INTENT_RULES {
        let hits = words.iter().filter(|w| customer_lower.contains(*w)).count();
        if hits > best_hits {
            best_hits = hits;
            intent = (*label).to_string();
        }
    }

    // Extractive summary: score customer sentences by keyword density,
    // keep the top 3 in chronological order.
    let mut summary: Vec<(usize, String, f64)> = Vec::new();
    for (idx, s) in sentences(&customer_text).into_iter().enumerate() {
        let tokens = tokenize(&s).len().max(1);
        let hits = tokenize(&s)
            .into_iter()
            .filter(|t| keywords.contains(t))
            .count();
        summary.push((idx, s, hits as f64 / tokens as f64));
    }
    summary.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let mut picked: Vec<(usize, String)> = summary
        .into_iter()
        .take(3)
        .map(|(idx, s, _)| (idx, s))
        .collect();
    picked.sort_by_key(|(idx, _)| *idx);
    let summary: Vec<String> = picked.into_iter().map(|(_, s)| s).collect();

    Ok(LocalAnalysis {
        summary,
        questions,
        intent,
        urgency: urgency.to_string(),
        sentiment: sentiment.to_string(),
        escalation,
        keywords,
    })
}

/// Help Center + knowledge base hits for the conversation's keywords.
pub fn related_articles(
    conn: &Connection,
    keywords: &[String],
    subject: Option<&str>,
    limit: usize,
) -> Vec<RelatedArticle> {
    let query = format!("{} {}", subject.unwrap_or_default(), keywords.join(" "));
    let fts = crate::search::fts_query_or(&query, 10);
    let mut out: Vec<RelatedArticle> = Vec::new();
    if fts == "\"\"" {
        return out;
    }
    // Help Center mirror (docs_fts: name, text, article_id).
    if let Ok(mut stmt) = conn.prepare(
        "SELECT article_id, name, snippet(docs_fts, 1, '[', ']', '…', 14)
                        FROM docs_fts WHERE docs_fts MATCH ?1 ORDER BY rank LIMIT ?2",
    ) && let Ok(rows) = stmt.query_map(params![fts, limit as i64], |r| {
        Ok(RelatedArticle {
            article_id: r.get(0)?,
            name: r.get(1)?,
            snippet: r.get(2)?,
        })
    }) {
        out.extend(rows.filter_map(|r| r.ok()));
    }
    if out.len() < limit
        && let Ok(hits) =
            crate::search::search_knowledge(conn, &query, None, (limit - out.len()) as i64)
    {
        for hit in hits {
            out.push(RelatedArticle {
                article_id: hit.id,
                name: hit.title,
                snippet: hit.snippet,
            });
        }
    }
    out
}

pub fn customer_history(conn: &Connection, conversation_local_id: i64) -> CustomerHistory {
    conn.query_row(
        "SELECT COUNT(*), MIN(remote_created_at), MAX(remote_created_at)
           FROM conversations c
          WHERE c.customer_local_id = (SELECT customer_local_id
                                         FROM conversations WHERE id = ?1)
            AND c.deleted_at IS NULL",
        params![conversation_local_id],
        |r| {
            Ok(CustomerHistory {
                previous_conversations: r.get(0)?,
                first_contact: r.get(1)?,
                last_contact: r.get(2)?,
            })
        },
    )
    .unwrap_or(CustomerHistory {
        previous_conversations: 0,
        first_contact: None,
        last_contact: None,
    })
}

/// Assemble the evidence-grounded draft. Sources, in priority order:
/// 1. the best similar past resolution (adapted, with a visible pointer
///    to the past conversation),
/// 2. the strongest matching Help Center article,
/// 3. a neutral skeleton that asks the clarifying questions the local
///    analysis extracted — never an invented answer.
fn build_draft(
    analysis: &LocalAnalysis,
    similar: &[SimilarConversation],
    articles: &[RelatedArticle],
    customer_first: Option<&str>,
    subject: Option<&str>,
    signature: Option<&str>,
) -> String {
    let mut draft = String::new();
    match customer_first {
        Some(name) => draft.push_str(&format!("Hi {name},\n\n")),
        None => draft.push_str("Hi,\n\n"),
    }
    draft.push_str(&format!(
        "Thanks for reaching out about \"{}\".\n\n",
        subject.unwrap_or("your request")
    ));

    if let Some(best) = similar.first().filter(|s| !s.resolution.is_empty()) {
        let resolution: String = best
            .resolution
            .chars()
            .take(500)
            .collect::<String>()
            .trim()
            .to_string();
        draft.push_str(&format!(
            "This looks like an issue we resolved recently (conversation #{}). \
             What worked then:\n\n{}\n\n",
            best.number, resolution
        ));
    } else if let Some(article) = articles.first() {
        draft.push_str(&format!(
            "Our Help Center article \"{}\" covers this — here is the \
             relevant part:\n\n{}\n\n",
            article.name,
            article.snippet.chars().take(400).collect::<String>()
        ));
    }

    if !analysis.questions.is_empty() {
        draft.push_str("To make sure we solve this quickly, could you confirm:\n");
        for q in analysis.questions.iter().take(3) {
            draft.push_str(&format!("- {q}\n"));
        }
        draft.push('\n');
    }

    draft.push_str("We're on it and will get back to you as soon as possible.\n\nBest,\n");
    draft.push_str(signature.unwrap_or("Support"));
    draft.push_str(
        "\n\n[Drafted by SupportOS Copilot from past resolutions and help articles — review before sending.]",
    );
    draft
}

/// The full local copilot report for one conversation. A4: when a vector
/// index is available the similar-issues lane fuses FTS + sqlite-vec hits,
/// so the panel's "semantic match" reasons are backed by real vectors.
pub fn copilot_report(
    conn: &Connection,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    conversation_local_id: i64,
) -> Result<CopilotReport> {
    let analysis = analyze(conn, conversation_local_id)?;
    // Clean-audit item 102: the signature must be the conversation's
    // actual assignee — the pre-fix subquery grabbed `users ORDER BY id
    // LIMIT 1`, i.e. whichever teammate happens to have the lowest row
    // id, and signed the draft with that arbitrary agent's name. An
    // unassigned conversation falls back to the honest "Support".
    let (subject, customer_first, me_name): (Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT c.subject,
                    (SELECT cu.first_name FROM customers cu WHERE cu.id = c.customer_local_id),
                    (SELECT u.first_name FROM users u WHERE u.id = c.assignee_local_id)
               FROM conversations c WHERE c.id = ?1",
            params![conversation_local_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((None, None, None));

    let semantic: Vec<(i64, f64)> = match index {
        Some(ix) => crate::ai_evidence::semantic_hits(conn, ix, conversation_local_id, 8),
        None => Vec::new(),
    };
    let similar = crate::ai_evidence::find_similar(conn, conversation_local_id, 5, &semantic)
        .unwrap_or_default();
    let articles = related_articles(conn, &analysis.keywords, subject.as_deref(), 4);
    let history = customer_history(conn, conversation_local_id);
    let suggested_reply = build_draft(
        &analysis,
        &similar,
        &articles,
        customer_first.as_deref(),
        subject.as_deref(),
        me_name.as_deref(),
    );

    Ok(CopilotReport {
        analysis,
        similar,
        articles,
        suggested_reply,
        history,
    })
}

/// The newest LM Studio draft for a conversation (the deep generative
/// pass the panel offers when a provider is configured).
pub fn latest_ai_draft(conn: &Connection, conversation_local_id: i64) -> Option<(String, String)> {
    conn.query_row(
        "SELECT content, model FROM ai_drafts
          WHERE conversation_id = ?1 ORDER BY id DESC LIMIT 1",
        params![conversation_local_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}

/// A6 feedback loop variant of [`latest_ai_draft`]: the newest draft's
/// **row id** rides along so the composer can track provenance and record
/// accept / edit-distance / reject against the real `ai_drafts` row.
pub fn latest_ai_draft_row(
    conn: &Connection,
    conversation_local_id: i64,
) -> Option<(i64, String, String)> {
    conn.query_row(
        "SELECT id, content, model FROM ai_drafts
          WHERE conversation_id = ?1 ORDER BY id DESC LIMIT 1",
        params![conversation_local_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .ok()
}

/// Whether the generative draft pass is usable (A1: routed through the
/// Draft role — the hub's endpoint + model, or the legacy single-model
/// fallback with a configured chat model).
pub fn generative_backend_available(conn: &Connection) -> bool {
    match crate::ai_hub::backend_for_role(conn, crate::ai_hub::AiRole::Draft) {
        crate::ai_pipeline::AiBackend::Disabled => false,
        crate::ai_pipeline::AiBackend::LmStudio { model, .. } => {
            // A role-assigned model qualifies; the legacy fallback needs
            // its explicitly configured chat model.
            model.is_some()
                || crate::settings::get_string(conn, "lmstudio_chat_model")
                    .ok()
                    .flatten()
                    .is_some_and(|m| !m.is_empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> crate::db::DbHandle {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn seed_thread(conn: &Connection, conv: i64, kind: &str, body: &str) {
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, body_text, created_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            params![conv, kind, body],
        )
        .unwrap();
    }

    #[test]
    fn analyze_extracts_questions_urgency_and_intent() {
        let handle = db();
        let conn = handle.write_guard();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (10, 1, 'Export broken again', 'preview', 'active')",
            [],
        )
        .unwrap();
        let conv = conn.last_insert_rowid();
        seed_thread(
            &conn,
            conv,
            "customer",
            "The CSV export is broken, throws an error and fails every time. This is urgent, \
             the export is not working and we cannot work! How do I fix the export timeout?",
        );
        let a = analyze(&conn, conv).unwrap();
        assert_eq!(a.intent, "Bug report");
        assert_eq!(a.urgency, "high");
        assert!(a.questions.iter().any(|q| q.contains("export timeout")));
        assert!(!a.summary.is_empty(), "summary must extract something");
        assert!(a.keywords.iter().any(|k| k == "export"));
    }

    /// RS-4-2 (G4-rejection fix): Help Scout imports store source HTML in
    /// `body_text`; the analyzer must summarize what the reader shows —
    /// the plain-text projection — never the raw markup. The owner's live
    /// data produced summary lines like `• <br><br>When I opened the link
    /// …` before the fix.
    #[test]
    fn analyze_strips_html_from_thread_bodies() {
        let handle = db();
        let conn = handle.write_guard();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (12, 3, 'Webinar watch page', 'preview', 'closed')",
            [],
        )
        .unwrap();
        let conv = conn.last_insert_rowid();
        seed_thread(
            &conn,
            conv,
            "customer",
            "Hey there,<br><br>Thanks so much for all the help you've given me.<br><br>\
             The export is broken, throws an error and fails every time. This is urgent \
             and the export is not working! How do I give them full access to the watch page?",
        );
        let a = analyze(&conn, conv).unwrap();
        let rendered: String = a
            .summary
            .iter()
            .chain(a.questions.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!rendered.contains('<'), "markup leaked: {rendered:?}");
        assert!(!rendered.contains("br>"), "markup leaked: {rendered:?}");
        assert!(
            a.summary.iter().any(|l| l.contains("export is broken")),
            "the stripped sentence must be summarizable: {:?}",
            a.summary
        );
    }

    #[test]
    fn build_draft_is_evidence_grounded_and_never_empty() {
        let handle = db();
        let conn = handle.write_guard();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (11, 2, 'Login broken', 'preview', 'active')",
            [],
        )
        .unwrap();
        let conv = conn.last_insert_rowid();
        seed_thread(
            &conn,
            conv,
            "customer",
            "I am locked out of my account and cannot log in. How do I reset my password?",
        );
        let report = copilot_report(&conn, None, conv).unwrap();
        assert!(
            report.suggested_reply.contains("Thanks for reaching out"),
            "draft must greet"
        );
        assert!(
            !report.suggested_reply.contains("conversation #"),
            "no similar history yet - draft must not cite one"
        );
        assert!(
            report.suggested_reply.contains("Login broken"),
            "draft must reference the actual subject"
        );
        assert!(
            report.suggested_reply.contains("reset my password"),
            "draft must carry the customer's clarifying question"
        );
    }
}

#[cfg(test)]
mod batch29_audit102_tests {
    use super::*;

    /// Clean-audit item 102: the draft signature must be the
    /// conversation's actual assignee. The pre-fix subquery grabbed
    /// `users ORDER BY id LIMIT 1` — whichever teammate happened to hold
    /// the lowest row id signed every copilot draft; an unassigned
    /// conversation falls back to the honest "Support".
    #[test]
    fn audit102_signature_is_the_assignee_not_an_arbitrary_user() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        // The arbitrary user the pre-fix query would have picked: lowest id.
        conn.execute(
            "INSERT INTO users (remote_id, first_name, last_name, email)
             VALUES (11, 'Adam', 'Arbitrary', 'adam@example.com')",
            [],
        )
        .expect("lowest-id user");
        conn.execute(
            "INSERT INTO users (remote_id, first_name, last_name, email)
             VALUES (12, 'Grace', 'Assignee', 'grace@example.com')",
            [],
        )
        .expect("assignee user");
        let assignee = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name)
             VALUES (80, 'Sam', 'Customer')",
            [],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();

        // The assigned conversation: the draft signs as Grace.
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                        customer_local_id, assignee_local_id)
             VALUES (9501, 95, 'Login broken', 'preview', 'active', ?1, ?2)",
            params![customer, assignee],
        )
        .expect("conversation");
        let assigned = conn.last_insert_rowid();
        let report = copilot_report(&conn, None, assigned).expect("report");
        assert!(
            report.suggested_reply.contains("Best,\nGrace"),
            "the draft must be signed by the assignee, not an arbitrary user: {}",
            report.suggested_reply
        );
        assert!(
            !report.suggested_reply.contains("Adam"),
            "the lowest-id user must not sign another agent's draft"
        );

        // The unassigned conversation: the honest fallback.
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                        customer_local_id)
             VALUES (9502, 96, 'Export failing', 'preview', 'active', ?1)",
            params![customer],
        )
        .expect("unassigned conversation");
        let unassigned = conn.last_insert_rowid();
        let report = copilot_report(&conn, None, unassigned).expect("report");
        assert!(
            report.suggested_reply.contains("Best,\nSupport"),
            "an unassigned conversation falls back to the Support signature: {}",
            report.suggested_reply
        );
    }
}
