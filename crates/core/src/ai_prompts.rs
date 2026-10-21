use serde::{Deserialize, Serialize};

pub const PROMPT_VERSIONS_TICKET_ANALYSIS: &str = "ticket_analysis_v1";
pub const PROMPT_VERSIONS_CUSTOMER_DRAFT: &str = "customer_draft_v1";
pub const PROMPT_VERSIONS_DRAFT_VERIFICATION: &str = "draft_verification_v1";
pub const PROMPT_VERSIONS_ISSUE_CLUSTER: &str = "issue_cluster_v1";
pub const PROMPT_VERSIONS_REPORT_NARRATIVE: &str = "report_narrative_v1";
pub const PROMPT_VERSIONS_MEMORY_EXTRACTION: &str = "memory_extraction_v1";
pub const PROMPT_VERSIONS_INTERACTION_OBSERVATION: &str = "interaction_observation_v1";
pub const PROMPT_VERSIONS_INTERACTION_RECOMMENDATION: &str = "interaction_recommendation_v1";
pub const PROMPT_VERSIONS_ATTRIBUTE_EXTRACTION: &str = "attribute_extraction_v1";
pub const PROMPT_VERSIONS_COPILOT_CHAT: &str = "copilot_chat_v1";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TicketAnalysis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_question: Option<String>,
    #[serde(default)]
    pub secondary_questions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urgency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentiment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_issue_candidate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_cluster_candidate: Option<String>,
    #[serde(default)]
    pub missing_information: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DraftVerification {
    pub verified: bool,
    #[serde(default)]
    pub unsupported_claims: Vec<String>,
    #[serde(default)]
    pub missing_questions: Vec<String>,
    #[serde(default)]
    pub internal_leakage: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub number: i64,
    pub subject: String,
    pub summary: String,
    pub days_ago: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadEntry {
    pub author: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub date: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarCase {
    pub number: i64,
    pub subject: String,
    pub resolution: String,
    pub date: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownIssueEntry {
    pub title: String,
    pub symptoms: String,
    #[serde(rename = "customerSafeExplanation")]
    pub customer_safe_explanation: Option<String>,
    pub workaround: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub title: String,
    pub text: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedReplyEntry {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidenceContext {
    pub conversation_number: i64,
    pub subject: String,
    pub customer_name: String,
    #[serde(default)]
    pub customer_history: Vec<HistoryEntry>,
    #[serde(default)]
    pub threads: Vec<ThreadEntry>,
    #[serde(default)]
    pub similar_cases: Vec<SimilarCase>,
    #[serde(default)]
    pub known_issues: Vec<KnownIssueEntry>,
    #[serde(default)]
    pub knowledge: Vec<KnowledgeEntry>,
    #[serde(default)]
    pub saved_replies: Vec<SavedReplyEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_strategy: Option<String>,
    /// Durable customer facts extracted from past conversations (A2):
    /// the "know before you reply" briefing block. Quarantined keys
    /// never reach the prompt (ai_memory_guard filters them at load).
    #[serde(default)]
    pub customer_memory: Vec<MemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub confidence: String,
}

fn render_evidence(ctx: &EvidenceContext) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "CONVERSATION #{}: {}",
        ctx.conversation_number, ctx.subject
    ));
    parts.push(format!("CUSTOMER: {}", ctx.customer_name));
    if !ctx.customer_history.is_empty() {
        parts.push("\nCUSTOMER HISTORY (previous conversations):".to_string());
        for h in &ctx.customer_history {
            parts.push(format!(
                "  #{} ({}d ago): {} - {}",
                h.number, h.days_ago, h.subject, h.summary
            ));
        }
    }
    parts.push("\nTHREADS (oldest first):".to_string());
    for t in &ctx.threads {
        parts.push(format!(
            "  [{}] {} ({}): {}",
            t.date, t.author, t.kind, t.text
        ));
    }
    if !ctx.similar_cases.is_empty() {
        parts.push("\nSIMILAR PAST CONVERSATIONS:".to_string());
        for s in &ctx.similar_cases {
            parts.push(format!(
                "  #{} {} ({}) - resolution: {} [visibility: {}]",
                s.number, s.subject, s.date, s.resolution, s.visibility
            ));
        }
    }
    if !ctx.known_issues.is_empty() {
        parts.push("\nKNOWN ISSUES:".to_string());
        for k in &ctx.known_issues {
            let mut line = format!("  - {}: {}", k.title, k.symptoms);
            if let Some(ref cse) = k.customer_safe_explanation {
                line.push_str(&format!(" | customer-safe explanation: {cse}"));
            }
            if let Some(ref wa) = k.workaround {
                line.push_str(&format!(" | workaround: {wa}"));
            }
            parts.push(line);
        }
    }
    if !ctx.knowledge.is_empty() {
        parts.push("\nKNOWLEDGE DOCUMENTS:".to_string());
        for k in &ctx.knowledge {
            parts.push(format!("  [{}] {}: {}", k.visibility, k.title, k.text));
        }
    }
    if !ctx.saved_replies.is_empty() {
        parts.push("\nSAVED REPLIES:".to_string());
        for s in &ctx.saved_replies {
            parts.push(format!("  \"{}\": {}", s.name, s.text));
        }
    }
    if !ctx.customer_memory.is_empty() {
        parts.push("\nDURABLE CUSTOMER FACTS (know before you reply):".to_string());
        for m in &ctx.customer_memory {
            parts.push(format!(
                "  - {}: {} (confidence: {})",
                m.key, m.value, m.confidence
            ));
        }
    }
    if let Some(ref strategy) = ctx.interaction_strategy {
        parts.push(strategy.clone());
    }
    parts.join("\n")
}

pub const TICKET_ANALYSIS_SYSTEM: &str = "You are the analysis engine of a local customer-support assistant. You analyze support conversations and produce STRICT JSON.

Rules:
- Base every field ONLY on the evidence provided. If information is missing, use null or an empty array - never invent.
- \"evidence_quality\": \"strong\" when multiple evidence sources support the analysis, \"some\" when one source supports it, \"limited\" when the evidence is thin, \"insufficient\" when you are mostly guessing.
- Do not speculate about timeframes, releases, or engineering status.
- Respond with a single JSON object, no prose.

JSON shape:
{
  \"intent\": \"one of: question | bug_report | feature_request | billing | complaint | how_to | other, or null\",
  \"primary_question\": \"the customer's main question in one sentence, or null\",
  \"secondary_questions\": [\"other explicit questions\"],
  \"customer_goal\": \"what the customer ultimately wants to achieve\",
  \"product\": \"product/area mentioned or null\",
  \"feature\": \"specific feature or null\",
  \"problem_type\": \"one of: configuration | defect | documentation_gap | account_access | billing | data | integration | other, or null\",
  \"requested_action\": \"what the customer asks us to do\",
  \"urgency\": \"low | normal | high | critical\",
  \"sentiment\": \"positive | neutral | negative | frustrated\",
  \"known_issue_candidate\": \"title of a known issue from the evidence that matches, or null\",
  \"issue_cluster_candidate\": \"a short 2-4 word topic label grouping this ticket with similar ones (e.g. 'timezone schedules', 'invite delivery'), or null\",
  \"missing_information\": [\"information we still need from the customer\"],
  \"summary\": \"2-3 sentence factual summary\",
  \"evidence_quality\": \"strong | some | limited | insufficient\"
}";

pub fn build_ticket_analysis_user(ctx: &EvidenceContext) -> String {
    format!(
        "Analyze the following support conversation evidence and respond with the JSON object described.\n\n{}",
        render_evidence(ctx)
    )
}

pub const CUSTOMER_DRAFT_SYSTEM: &str = "You draft customer-facing support replies. You operate in VERIFIED ANSWER MODE.

Hard rules:
- Use ONLY customer-safe evidence: the current conversation, verified account information, approved customer-safe knowledge, verified known-issue customer-safe explanations, and saved replies.
- NEVER use internal-only notes, engineering details, or another customer's data.
- NEVER invent: timeframes, feature availability, bug-fix status, release dates, engineering decisions, policies, or account-specific facts.
- If the evidence does not support an answer, say that verification with the team is needed.
- Keep the tone professional, warm and concise. No markdown headers. Plain paragraphs and optional short lists.
- Respond with a single JSON object: {\"draft\": \"<reply text>\", \"used_evidence\": [\"short label of each piece of evidence you relied on\"]}";

pub fn build_customer_draft_user(
    ctx: &EvidenceContext,
    mode: &str,
    analysis: Option<&TicketAnalysis>,
) -> String {
    let analysis_line = if let Some(a) = analysis {
        format!(
            "\nAI ANALYSIS (internal - do not leak verbatim): intent={}; primary question={}; known issue candidate={}",
            a.intent.as_deref().unwrap_or("null"),
            a.primary_question.as_deref().unwrap_or("unknown"),
            a.known_issue_candidate.as_deref().unwrap_or("none")
        )
    } else {
        String::new()
    };
    let mode_line = if mode == "verified_answer" {
        "VERIFIED ANSWER MODE is active - only customer-safe evidence may support statements."
    } else {
        "Standard mode - still never expose internal-only information."
    };
    let mut safe = ctx.clone();
    safe.similar_cases
        .retain(|s| s.visibility == "customer_safe");
    safe.knowledge.retain(|k| k.visibility == "customer_safe");
    format!(
        "{mode_line}{analysis_line}\n\nDraft a reply to the customer's LATEST message using the evidence below.\n\n{}",
        render_evidence(&safe)
    )
}

pub const DRAFT_VERIFICATION_SYSTEM: &str = "You verify AI-drafted customer replies against evidence. You are strict and skeptical.

Check:
1. Does the draft answer every explicit customer question? (missing_questions)
2. Are all factual claims supported by the evidence? (unsupported_claims)
3. Does it invent a timeframe, feature, fix status, release date, policy or account fact? (unsupported_claims)
4. Does it expose internal-only information (engineering notes, internal IDs, other customers)? (internal_leakage)
5. Does it contradict the evidence? (conflicts)
6. Any other risk? (warnings)

Respond ONLY with JSON: {\"verified\": true/false, \"unsupported_claims\": [\"...\"], \"missing_questions\": [\"...\"], \"internal_leakage\": [\"...\"], \"conflicts\": [\"...\"], \"warnings\": [\"...\"]}";

pub fn build_draft_verification_user(
    ctx: &EvidenceContext,
    draft: &str,
    customer_questions: &[String],
) -> String {
    let q = if customer_questions.is_empty() {
        "- (none extracted)".to_string()
    } else {
        customer_questions
            .iter()
            .map(|q| format!("- {q}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "Customer's explicit questions:\n{q}\n\nDRAFT TO VERIFY:\n\"\"\"\n{draft}\n\"\"\"\n\nEVIDENCE:\n{}",
        render_evidence(ctx)
    )
}

pub const ISSUE_CLUSTER_SYSTEM: &str = "You group support conversations into issue clusters based on their content.

Rules:
- Derive categories from the ACTUAL data - do not force predefined categories.
- Only create clusters with 2+ conversations.
- Titles: short, specific, lowercase topic style (e.g. \"timezone after DST change\", \"invite emails bouncing\").
- Respond ONLY with JSON: {\"clusters\": [{\"title\": \"...\", \"summary\": \"one sentence\", \"category\": \"...\", \"product\": \"...\"|null, \"feature\": \"...\"|null, \"conversation_numbers\": [..]}]}";

pub fn build_issue_cluster_user(conversations: &[ClusterConversation]) -> String {
    let lines: Vec<String> = conversations
        .iter()
        .map(|c| {
            let tags = if c.tags.is_empty() {
                "no tags".to_string()
            } else {
                c.tags.join(",")
            };
            format!("#{} [{}] {}: {}", c.number, tags, c.subject, c.preview)
        })
        .collect();
    format!(
        "Group these conversations into issue clusters. Only output clusters with 2+ members.\n\n{}",
        lines.join("\n")
    )
}

#[derive(Debug, Clone)]
pub struct ClusterConversation {
    pub number: i64,
    pub subject: String,
    pub preview: String,
    pub tags: Vec<String>,
}

pub const REPORT_NARRATIVE_SYSTEM: &str = "You write a short factual narrative for a support report. SQL-computed numbers are given to you as facts - never recalculate or invent numbers. Clearly flag any statement you are unsure about with \"requires verification\". Respond ONLY with JSON: {\"narrative\": \"...\"}";

pub fn build_report_narrative_user(report_name: &str, facts: &serde_json::Value) -> String {
    format!(
        "Report: {report_name}\n\nComputed facts (authoritative, do not change):\n{}\n\nWrite a 3-5 sentence narrative for a support lead summarizing what matters. The narrative will be clearly labeled as AI-generated.",
        serde_json::to_string_pretty(facts).unwrap_or_else(|_| "{}".to_string())
    )
}

pub const MEMORY_EXTRACTION_SYSTEM: &str = "You extract durable customer facts worth remembering for future support interactions. Only include facts clearly supported by the conversation. Do NOT include sensitive payment data or credentials. Respond ONLY with JSON: {\"memories\": [{\"key\": \"short_label\", \"value\": \"one sentence fact\", \"confidence\": \"high|medium|low\"}]}";

pub fn build_memory_extraction_user(customer_name: &str, threads: &[(String, String)]) -> String {
    let lines: Vec<String> = threads
        .iter()
        .map(|(author, text)| format!("{author}: {text}"))
        .collect();
    format!(
        "Customer: {customer_name}\n\nConversation:\n{}",
        lines.join("\n")
    )
}

pub const INTERACTION_OBSERVATION_SYSTEM: &str = "You analyze the client's observable communication patterns for support purposes. Identify current communication signals, recurring support interaction patterns, relevant communication preferences, and meaningful changes from historical behavior. Do not diagnose mental health or infer sensitive personal traits. Base each important observation on evidence from the supplied conversation history.

HARD RULES:
- Report ONLY observable communication behavior: tone, directness, detail level, technical language, question structure, urgency cues, frustration cues, expectation, response preference.
- NEVER produce personality labels, psychological claims, diagnoses, or judgments about the person.
- Every signal must quote an evidence_excerpt copied from the messages. Signals without evidence are invalid.
- Allowed values per dimension (use EXACTLY these):
  tone: neutral|friendly|frustrated|appreciative|disappointed|confrontational|urgent|uncertain
  directness: indirect|conversational|direct|highly_direct
  detail: very_low|low|moderate|high|very_high
  technical_language: non_technical|mixed|technical|highly_technical
  question_structure: single_question|multiple_questions|troubleshooting_oriented|confirmation_oriented|explanation_oriented
  urgency: none|low|moderate|high
  frustration: none|possible|moderate|strong
  expectation: information|explanation|troubleshooting|action|immediate_resolution|escalation|confirmation
  response_preference: concise|detailed|step_by_step|technical|conversational|outcome_focused
- confidence is one of high|medium|low|unknown and is an operational judgment, not a probability.

Respond ONLY with JSON:
{\"signals\": [{\"dimension\": \"...\", \"value\": \"...\", \"confidence\": \"...\", \"evidence_excerpt\": \"...\", \"evidence_thread_local_id\": 123}], \"customer_goal\": \"...\", \"notes\": [\"...\"]}";

pub const INTERACTION_RECOMMENDATION_SYSTEM: &str = "You are a support-approach advisor. Given observations about a client's observable communication (never psychological claims), you recommend how a support rep should approach this specific conversation.

RULES:
- Be actionable: tone, length, how to start, what to avoid, response strategy steps.
- Recommend ONLY what the observations support. Do not invent history.
- Never promise unsupported timeframes; frame expectations conservatively.
- If frustration is strong, include de-escalation guidance (acknowledge impact, avoid defensiveness, answer the central issue).
- \"why\" must reference the concrete observations that justify each recommendation.

Respond ONLY with JSON:
{\"tone\": \"...\", \"length\": \"concise|moderate|detailed\", \"start_with\": \"...\", \"then\": \"...\", \"avoid\": [\"...\"], \"response_strategy\": [\"step 1\", \"step 2\"], \"de_escalation\": false, \"escalation_recommendation\": null, \"why\": [\"...\"]}";

pub fn interaction_strategy_block(
    tone: Option<&str>,
    length: Option<&str>,
    response_strategy: &[String],
    avoid: &[String],
    preferences: &[String],
    already_provided: &[String],
) -> String {
    let mut out = format!(
        "\nCOMMUNICATION APPROACH (from client interaction analysis):\n- Tone: {}\n- Length: {}\n- Response strategy: {}\n- Avoid: {}",
        tone.unwrap_or("unspecified"),
        length.unwrap_or("moderate"),
        if response_strategy.is_empty() {
            "answer the primary question directly".to_string()
        } else {
            response_strategy.join(" -> ")
        },
        if avoid.is_empty() {
            "nothing specific".to_string()
        } else {
            avoid.join("; ")
        },
    );
    if !preferences.is_empty() {
        out.push_str(&format!(
            "\n- Client communication preferences: {}",
            preferences.join("; ")
        ));
        out.push('\n');
    }
    if !already_provided.is_empty() {
        out.push_str("\nALREADY PROVIDED BY THE CUSTOMER (do NOT ask again, do not repeat):\n");
        for a in already_provided {
            out.push_str(&format!("- {a}\n"));
        }
    }
    out.push_str("Use this to shape HOW you answer, not WHAT you answer.");
    out
}

pub const ATTRIBUTE_EXTRACTION_SYSTEM: &str = "You extract structured attributes from a customer support conversation. You are part of a LOCAL-first support tool: everything you output is stored as versioned, evidence-backed local metadata - never sent to the customer.

Rules:
- Extract ONLY what the messages support. A missing attribute is simply omitted - never invent values.
- \"intent\" MUST be exactly one of: question | bug_report | feature_request | billing | how_to | account_management | feedback | other.
- \"response_style\" MUST be exactly one of: concise | detailed | step_by_step | technical | conversational | outcome_focused - pick the style the CUSTOMER's own messages ask for, not what you would write.
- \"product\", \"feature\", \"issue\", \"customer_goal\" are short free text (max ~12 words each).
- For every attribute with confidence \"medium\" or \"high\" you MUST include an evidence_excerpt copied from the messages provided, plus the thread id it came from.
- Use confidence \"low\" when the signal is weak or ambiguous.
- Respond with a single JSON object, no prose.

JSON shape:
{\"attributes\": [{\"attribute\": \"intent|product|feature|issue|customer_goal|response_style\", \"value\": \"...\", \"confidence\": \"high|medium|low|unknown\", \"evidence_excerpt\": \"...\", \"evidence_thread_local_id\": 123}]}";

pub fn build_attribute_extraction_user(subject: &str, messages: &[(String, i64)]) -> String {
    let mut parts = vec![
        format!("SUBJECT: {}", truncate(subject, 300)),
        "CUSTOMER MESSAGES (oldest first):".to_string(),
    ];
    for (text, thread_id) in messages {
        parts.push(format!("  [thread {thread_id}] {text}"));
    }
    parts.push(String::new());
    parts.push("Extract the attributes that are actually supported by these messages.".to_string());
    parts.join("\n")
}

pub const COPILOT_SYSTEM: &str = "You are the SupportOS Local Copilot - an assistant for one support rep, running fully locally. You help the rep understand a ticket, its customer and the local knowledge base BEFORE they reply.

Core rules:
- You have READ-ONLY tools that search the local archive: conversations, customer history, similar tickets, knowledge, known issues, issue clusters, saved replies, AI analyses and ticket metadata. Use them instead of guessing.
- ANSWER WITH EVIDENCE: support important claims with inline markers like [1], [2] referencing the numbered tool results you actually used. If you did not retrieve evidence for a claim, say so plainly.
- If the tools return nothing, SAY SO - \"I found no matching tickets\" is a correct answer. Never fabricate ticket numbers, dates, resolutions or quotes.
- Distinguish clearly: what is in the current ticket vs what is in past tickets vs what is in knowledge/docs.
- You are internal-only: everything you say is for the rep, never sent to the customer directly.
- You do not send anything, change any ticket, or write to Help Scout. You only read and explain.
- Be concise and structured: short paragraphs or tight bullet lists. Lead with the direct answer, then the evidence.
- When the rep asks \"what should I check before replying\", produce a practical checklist grounded in the retrieved evidence (unanswered questions, missing info, known issue status, similar resolutions).";

pub fn build_copilot_context_block(
    conversation_number: Option<i64>,
    subject: Option<&str>,
    customer_name: Option<&str>,
    today: &str,
) -> String {
    let mut lines = vec![format!("Today: {today}")];
    if let Some(number) = conversation_number {
        let mut line = format!("The rep is currently viewing conversation #{number}");
        if let Some(s) = subject {
            line.push_str(&format!(" (\"{}\"", truncate(s, 160)));
            if let Some(c) = customer_name {
                line.push_str(&format!(" from {c}"));
            }
            line.push(')');
        } else if let Some(c) = customer_name {
            line.push_str(&format!(" from {c}"));
        }
        line.push_str(". Questions like \"this customer\" refer to it.");
        lines.push(line);
    } else {
        lines.push(
            "The rep is not viewing a specific conversation; use search tools to find what they mean."
                .to_string(),
        );
    }
    lines.join("\n")
}

pub fn truncate(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

pub struct InteractionChangeInput {
    pub dimension: String,
    pub baseline_value: Option<String>,
    pub current_value: Option<String>,
    pub significant: bool,
}

pub fn build_interaction_observation_user(
    customer_name: &str,
    client_kind: &str,
    current_messages: &[(String, Option<i64>)],
    baseline_summary: Option<&str>,
    recent_history: &[(i64, Option<&str>, &str)],
) -> String {
    let mut out = format!(
        "Client: {customer_name} ({})\n\nCURRENT TICKET customer messages:\n",
        if client_kind == "returning" {
            "returning client"
        } else {
            "first-time client"
        }
    );
    let msgs = current_messages
        .iter()
        .map(|(text, tid)| {
            format!(
                "[thread {}] {}",
                tid.map(|i| i.to_string()).unwrap_or_else(|| "?".into()),
                text.chars().take(900).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");
    out.push_str(&msgs);
    out.push('\n');
    out.push('\n');
    match baseline_summary {
        Some(b) => out.push_str(&format!(
            "HISTORICAL BASELINE (observed, recency-weighted):\n{b}\n"
        )),
        None => out.push_str(
            "No historical baseline exists (first-time client). Do NOT claim any historical pattern.\n",
        ),
    }
    if !recent_history.is_empty() {
        let hist = recent_history
            .iter()
            .map(|(number, subject, excerpt)| {
                format!(
                    "#{number} {}: {}",
                    subject.unwrap_or(""),
                    excerpt.chars().take(200).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        out.push_str(&format!(
            "\nRECENT PREVIOUS TICKETS (for context):\n{hist}\n"
        ));
    }
    out.push_str(
        "\nAnalyze observable communication behavior for support purposes. Signal what is happening in THIS interaction; only mention a historical pattern if the baseline above supports it.",
    );
    out
}

pub fn build_interaction_recommendation_user(
    client_kind: &str,
    current_signals: &[(String, String, String)],
    changes: &[InteractionChangeInput],
    _baseline_summary: Option<&str>,
    preferences: &[(String, String)],
    repeat_issue: bool,
    effort_score: Option<f64>,
) -> String {
    let mut out = format!("Client kind: {client_kind}\n\nCURRENT INTERACTION SIGNALS:\n");
    let sigs = current_signals
        .iter()
        .map(|(d, v, c)| format!("- {d}: {v} (confidence {c})"))
        .collect::<Vec<_>>()
        .join("\n");
    out.push_str(&sigs);
    out.push_str("\n\n");
    if changes.is_empty() {
        out.push_str("No baseline comparison available.\n");
    } else {
        let ch = changes
            .iter()
            .map(|c| {
                format!(
                    "- {}: {} -> {}{}",
                    c.dimension,
                    c.baseline_value.as_deref().unwrap_or("\u{2014}"),
                    c.current_value.as_deref().unwrap_or("\u{2014}"),
                    if c.significant { " [SIGNIFICANT]" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        out.push_str(&format!("CHANGES VS HISTORICAL BASELINE:\n{ch}\n"));
    }
    if !preferences.is_empty() {
        let prefs = preferences
            .iter()
            .map(|(p, o)| format!("- {p} ({o})"))
            .collect::<Vec<_>>()
            .join("\n");
        out.push_str(&format!(
            "\nOBSERVED PREFERENCES (human-entered overrides take precedence):\n{prefs}\n"
        ));
    }
    if repeat_issue {
        out.push_str(
            "\nNOTE: this appears to be a RECURRING unresolved issue for this client \u{2014} review previous cases before replying.\n",
        );
    }
    if let Some(score) = effort_score {
        out.push_str(&format!("Customer effort score so far: {score}/10.\n"));
    }
    out.push_str("\nRecommend a support approach for this conversation.");
    out
}
