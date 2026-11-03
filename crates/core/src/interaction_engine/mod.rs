use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::interaction_signals::{CurrentInteraction, InteractionSignal, MessageStats};
use crate::quality::is_closing_acknowledgment;

mod baseline;
pub use baseline::build_profile;
pub(crate) use baseline::{
    comparison_baseline, compute_changes, compute_outcome, detect_repeat_issue,
    heuristic_recommendation, rebuild_baseline,
};

struct ForbiddenPattern {
    regex: regex::Regex,
    reason: &'static str,
}

macro_rules! fp {
    ($re:expr, $reason:expr) => {
        ForbiddenPattern {
            regex: regex::Regex::new(&format!("(?i){}", $re)).expect("static pattern"),
            reason: $reason,
        }
    };
}

fn forbidden_patterns() -> &'static [ForbiddenPattern] {
    static PATTERNS: std::sync::OnceLock<Vec<ForbiddenPattern>> = std::sync::OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            fp!(
                r"\b(narcissis|anxiet|depress|bipolar|adhd|autis|schizo|paranoid|delusional|mentally|insane|unstable|unhinged)\w*",
                "mental-health language"
            ),
            fp!(
                r"\b(personality (disorder|type|trait)|myers.?briggs|big five|temperament|character flaw)\b",
                "personality typing"
            ),
            fp!(
                r"\b(difficult (person|customer|human)|problem (person|customer)|is (a )?(narcissist|aggressive person)|emotionally (unstable|volatile))\b",
                "fixed person label"
            ),
            fp!(
                r"\b(race|ethnicity|religion|politic|sexual orientation|gay|lesbian|trans|muslim|christian|jew|hindu|atheist|white supremacy|nationalist)\b",
                "protected attribute inference"
            ),
            fp!(
                r"\b(diagnos\w+|symptom of|suffers? from|patholog\w+)\b",
                "clinical diagnosis language"
            ),
            fp!(
                r"\b(intelligen(t|ce) (level|of)|iq|cognitive ability|stupid|dumb|incompetent person)\b",
                "cognitive-ability judgment"
            ),
            fp!(
                r"\b(manipulative|toxic person|evil|malicious person|bad person|liar|dishonest person)\b",
                "moral character judgment"
            ),
            fp!(
                r"\b((customer|client|user|person|he|she|they) (is|are|seems|acts|behaves) (a )?(rude|entitled|needy|demanding|lazy|clueless|hostile|abrasive|belligerent|passive.?aggressive|bully)|rude (person|customer|client)|entitled (person|customer|client)|needy (person|customer|client))\b",
                "fixed trait label"
            ),
            fp!(
                r"\b(passive.?aggressive|bullying|arrogant|condescending (person|customer)|vindictive|vengeful)\b",
                "character judgment"
            ),
        ]
    })
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextScan {
    pub ok: bool,
    pub warnings: Vec<String>,
}

pub(crate) fn sanitize_interaction_text(text: &str) -> TextScan {
    let mut warnings = Vec::new();
    for p in forbidden_patterns() {
        if p.regex.is_match(text) {
            warnings.push(format!(
                "Removed {} — SupportOS only reports observable support-communication behavior.",
                p.reason
            ));
        }
    }
    TextScan {
        ok: warnings.is_empty(),
        warnings,
    }
}

pub(crate) fn assert_interaction_text_safe(text: Option<&str>) -> TextScan {
    match text {
        Some(t) if !t.is_empty() => sanitize_interaction_text(t),
        _ => TextScan {
            ok: true,
            warnings: Vec::new(),
        },
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BaselineDimension {
    pub dimension: String,
    pub typical_value: String,
    pub confidence: String,
    pub observation_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_observed: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorBaseline {
    pub customer_local_id: i64,
    pub conversation_count: i64,
    pub observation_count: i64,
    pub dimensions: Vec<BaselineDimension>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    pub profile_version: i64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InteractionChange {
    pub dimension: String,
    pub baseline_value: String,
    pub current_value: String,
    pub direction: String,
    pub magnitude: f64,
    pub significant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SupportApproach {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_with: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    pub avoid: Vec<String>,
    pub response_strategy: Vec<String>,
    pub de_escalation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation_recommendation: Option<String>,
    pub why: Vec<String>,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct OutcomeComputed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_score: Option<f64>,
    pub friction: String,
    pub follow_up_count: i64,
    pub clarification_count: i64,
    pub escalated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_after_first_response: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_style: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EffectiveApproach {
    pub approach: String,
    pub worked_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example_conversation_local_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example_number: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrictionFlag {
    pub conversation_local_id: i64,
    pub number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub friction: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupportOutcomeSummary {
    pub customer_local_id: i64,
    pub total_conversations: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_response_resolution_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_up_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation_rate: Option<f64>,
    pub avg_effort_score: f64,
    pub effective_approaches: Vec<EffectiveApproach>,
    pub friction_flags: Vec<FrictionFlag>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RelatedConversation {
    pub local_id: i64,
    pub number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepeatIssue {
    pub detected: bool,
    pub related_conversations: Vec<RelatedConversation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverrideRow {
    pub id: i64,
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_value: Option<String>,
    pub human_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub created_at: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommunicationPreference {
    pub preference: String,
    pub evidence_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_observed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_observed: Option<String>,
    pub confidence: String,
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_override: Option<HumanOverride>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HumanOverride {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub overridden_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    pub month: String,
    pub conversation_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub conversation_local_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientPlaybook {
    pub best_opening: String,
    pub best_explanation_style: String,
    pub best_troubleshooting_style: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub likely_follow_up: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub historically_successful: Option<String>,
    pub avoid: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientInteractionProfile {
    pub customer_local_id: i64,
    pub client_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<BehaviorBaseline>,
    pub preferences: Vec<CommunicationPreference>,
    pub timeline: Vec<TimelineEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<SupportOutcomeSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playbook: Option<ClientPlaybook>,
    pub overrides: Vec<OverrideRow>,
}

#[derive(Debug, Clone)]
pub struct StoredRecommendation {
    pub recommendation: SupportApproach,
    pub prompt_version: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredCurrent {
    pub signals: Vec<InteractionSignal>,
    pub message_stats: Option<MessageStats>,
    pub customer_goal: Option<String>,
    pub sources: String,
    pub generated_at: String,
    pub analysis_version: Option<String>,
    pub recommendation: Option<StoredRecommendation>,
}

pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS client_behavior_observations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id INTEGER NOT NULL,
            conversation_id INTEGER,
            thread_local_id INTEGER,
            dimension TEXT NOT NULL,
            value TEXT NOT NULL,
            confidence TEXT NOT NULL DEFAULT 'low',
            evidence_excerpt TEXT,
            source TEXT NOT NULL DEFAULT 'heuristic',
            observed_at TEXT NOT NULL DEFAULT (datetime('now')),
            provenance TEXT NOT NULL DEFAULT 'heuristic'
        );
        CREATE INDEX IF NOT EXISTS idx_client_observations_customer
            ON client_behavior_observations(customer_id, dimension);
        CREATE INDEX IF NOT EXISTS idx_client_observations_conversation
            ON client_behavior_observations(conversation_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_client_observations_unique
            ON client_behavior_observations(conversation_id, dimension, source);

        CREATE TABLE IF NOT EXISTS client_behavior_baselines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id INTEGER NOT NULL,
            dimension TEXT NOT NULL,
            typical_value TEXT NOT NULL,
            confidence TEXT NOT NULL DEFAULT 'low',
            observation_count INTEGER NOT NULL DEFAULT 0,
            last_observed TEXT,
            profile_version INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (customer_id, dimension)
        );

        CREATE TABLE IF NOT EXISTS client_communication_preferences (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id INTEGER NOT NULL,
            preference TEXT NOT NULL,
            evidence_count INTEGER NOT NULL DEFAULT 1,
            first_observed TEXT,
            last_observed TEXT,
            confidence TEXT NOT NULL DEFAULT 'low',
            origin TEXT NOT NULL DEFAULT 'ai_inferred',
            human_override_value TEXT,
            human_override_reason TEXT,
            overridden_at TEXT,
            provenance TEXT NOT NULL DEFAULT 'ai_generated',
            UNIQUE (customer_id, preference)
        );
        CREATE INDEX IF NOT EXISTS idx_client_preferences_customer
            ON client_communication_preferences(customer_id);

        CREATE TABLE IF NOT EXISTS client_human_overrides (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id INTEGER NOT NULL,
            field TEXT NOT NULL,
            ai_value TEXT,
            human_value TEXT NOT NULL,
            reason TEXT,
            created_by TEXT NOT NULL DEFAULT 'user',
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_client_overrides_customer
            ON client_human_overrides(customer_id, active);

        CREATE TABLE IF NOT EXISTS client_support_outcomes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id INTEGER NOT NULL,
            conversation_id INTEGER NOT NULL,
            resolved_after_first_response INTEGER,
            follow_up_count INTEGER NOT NULL DEFAULT 0,
            clarification_count INTEGER NOT NULL DEFAULT 0,
            escalated INTEGER NOT NULL DEFAULT 0,
            effort_score REAL,
            response_style TEXT,
            friction TEXT,
            computed_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (conversation_id)
        );
        CREATE INDEX IF NOT EXISTS idx_client_outcomes_customer
            ON client_support_outcomes(customer_id);",
    )?;
    let has_col: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('client_current_signals')
              WHERE name = 'recommendation_json'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if has_col == 0 {
        conn.execute(
            "ALTER TABLE client_current_signals ADD COLUMN recommendation_json TEXT",
            [],
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct ObservationRow {
    pub customer_id: i64,
    pub dimension: String,
    pub value: String,
    pub confidence: String,
    pub evidence_excerpt: Option<String>,
    pub conversation_id: Option<i64>,
    pub thread_local_id: Option<i64>,
    pub observed_at: String,
    pub source: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn save_current_interaction(
    conn: &Connection,
    conversation_id: i64,
    customer_id: Option<i64>,
    signals: &[InteractionSignal],
    message_stats: &MessageStats,
    customer_goal: Option<&str>,
    sources: &str,
    analysis_version: Option<&str>,
) -> Result<()> {
    crate::interaction_signals::ensure_client_current_signals_table(conn)?;
    let provenance = if sources == "heuristic" {
        "heuristic"
    } else {
        "ai_generated"
    };
    conn.execute(
        "INSERT INTO client_current_signals
             (conversation_id, customer_id, signals_json, message_stats_json, customer_goal,
              sources, analysis_version, generated_at, provenance)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'), ?8)
         ON CONFLICT (conversation_id) DO UPDATE SET
           customer_id = excluded.customer_id,
           signals_json = excluded.signals_json,
           message_stats_json = excluded.message_stats_json,
           customer_goal = excluded.customer_goal,
           sources = excluded.sources,
           analysis_version = excluded.analysis_version,
           generated_at = datetime('now'),
           provenance = excluded.provenance",
        params![
            conversation_id,
            customer_id,
            serde_json::to_string(signals)?,
            serde_json::to_string(message_stats)?,
            customer_goal,
            sources,
            analysis_version,
            provenance,
        ],
    )?;
    Ok(())
}

pub(crate) fn get_latest_current_interaction(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<StoredCurrent>> {
    let row = conn
        .query_row(
            "SELECT signals_json, message_stats_json, customer_goal, sources, generated_at,
                    analysis_version, recommendation_json
               FROM client_current_signals WHERE conversation_id = ?1
              ORDER BY id DESC LIMIT 1",
            params![conversation_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .ok();
    let Some((signals_json, stats_json, goal, sources, generated_at, analysis_version, rec)) = row
    else {
        return Ok(None);
    };
    let recommendation = rec.and_then(|json| {
        let v: serde_json::Value = serde_json::from_str(&json).ok()?;
        Some(StoredRecommendation {
            recommendation: serde_json::from_value(v.get("recommendation")?.clone()).ok()?,
            prompt_version: v
                .get("prompt_version")
                .and_then(|p| p.as_str())
                .map(String::from),
            model: v.get("model").and_then(|m| m.as_str()).map(String::from),
        })
    });
    Ok(Some(StoredCurrent {
        signals: serde_json::from_str(&signals_json).unwrap_or_default(),
        message_stats: stats_json.and_then(|s| serde_json::from_str(&s).ok()),
        customer_goal: goal,
        sources,
        generated_at: generated_at.unwrap_or_default(),
        analysis_version,
        recommendation,
    }))
}

pub(crate) fn save_recommendation(
    conn: &Connection,
    conversation_id: i64,
    recommendation: &SupportApproach,
    prompt_version: Option<&str>,
    model: Option<&str>,
) -> Result<()> {
    let wrapped = serde_json::json!({
        "recommendation": recommendation,
        "prompt_version": prompt_version,
        "model": model,
    });
    conn.execute(
        "UPDATE client_current_signals SET recommendation_json = ?1 WHERE conversation_id = ?2",
        params![wrapped.to_string(), conversation_id],
    )?;
    Ok(())
}

pub(crate) fn insert_observations(conn: &Connection, rows: &[ObservationRow]) -> Result<()> {
    for o in rows {
        conn.execute(
            "INSERT INTO client_behavior_observations
                 (customer_id, conversation_id, thread_local_id, dimension, value, confidence,
                  evidence_excerpt, source, observed_at, provenance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (conversation_id, dimension, source) DO UPDATE SET
               value = excluded.value,
               confidence = excluded.confidence,
               evidence_excerpt = excluded.evidence_excerpt,
               thread_local_id = excluded.thread_local_id,
               observed_at = excluded.observed_at,
               provenance = excluded.provenance",
            params![
                o.customer_id,
                o.conversation_id,
                o.thread_local_id,
                o.dimension,
                o.value,
                o.confidence,
                o.evidence_excerpt,
                o.source,
                o.observed_at,
                if o.source == "ai" {
                    "ai_generated"
                } else {
                    "heuristic"
                },
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn get_observations_for_customer(
    conn: &Connection,
    customer_id: i64,
) -> Result<Vec<ObservationRow>> {
    let mut stmt = conn.prepare(
        "SELECT dimension, value, confidence, evidence_excerpt, conversation_id,
                thread_local_id, observed_at, source
           FROM client_behavior_observations
          WHERE customer_id = ?1
          ORDER BY observed_at DESC
          LIMIT 5000",
    )?;
    let rows = stmt
        .query_map(params![customer_id], |r| {
            Ok(ObservationRow {
                dimension: r.get(0)?,
                value: r.get(1)?,
                confidence: r.get(2)?,
                evidence_excerpt: r.get(3)?,
                conversation_id: r.get(4)?,
                thread_local_id: r.get(5)?,
                observed_at: r.get(6)?,
                source: r.get(7)?,
                customer_id,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_baseline(
    conn: &Connection,
    customer_id: i64,
    dimension: &str,
    typical_value: &str,
    confidence: &str,
    observation_count: i64,
    last_observed: Option<&str>,
    profile_version: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO client_behavior_baselines
             (customer_id, dimension, typical_value, confidence, observation_count,
              last_observed, profile_version, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT (customer_id, dimension) DO UPDATE SET
           typical_value = excluded.typical_value,
           confidence = excluded.confidence,
           observation_count = excluded.observation_count,
           last_observed = excluded.last_observed,
           profile_version = excluded.profile_version,
           updated_at = datetime('now')",
        params![
            customer_id,
            dimension,
            typical_value,
            confidence,
            observation_count,
            last_observed,
            profile_version
        ],
    )?;
    Ok(())
}

type BaselineRow = (String, String, String, i64, Option<String>, i64, String);

pub(crate) fn get_baseline(
    conn: &Connection,
    customer_id: i64,
) -> Result<Option<BehaviorBaseline>> {
    let mut stmt = conn.prepare(
        "SELECT dimension, typical_value, confidence, observation_count, last_observed,
                profile_version, updated_at
           FROM client_behavior_baselines WHERE customer_id = ?1",
    )?;
    let rows: Vec<BaselineRow> = stmt
        .query_map(params![customer_id], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    if rows.is_empty() {
        return Ok(None);
    }
    let mut last_updated = rows[0].6.clone();
    let mut observation_count = 0i64;
    let mut profile_version = 0i64;
    let mut dimensions = Vec::with_capacity(rows.len());
    for (dimension, typical_value, confidence, count, last_observed, version, updated) in rows {
        observation_count += count;
        profile_version = profile_version.max(version);
        if updated > last_updated {
            last_updated = updated;
        }
        dimensions.push(BaselineDimension {
            dimension,
            typical_value,
            confidence,
            observation_count: count,
            last_observed,
        });
    }
    let conversation_count = conn.query_row(
        "SELECT COUNT(DISTINCT conversation_id) FROM client_behavior_observations
          WHERE customer_id = ?1 AND conversation_id IS NOT NULL",
        params![customer_id],
        |r| r.get(0),
    )?;
    Ok(Some(BehaviorBaseline {
        customer_local_id: customer_id,
        conversation_count,
        observation_count,
        dimensions,
        last_updated: Some(last_updated),
        profile_version,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_preference(
    conn: &Connection,
    customer_id: i64,
    preference: &str,
    evidence_count: i64,
    first_observed: Option<&str>,
    last_observed: Option<&str>,
    confidence: &str,
    origin: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO client_communication_preferences
             (customer_id, preference, evidence_count, first_observed, last_observed,
              confidence, origin, provenance)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (customer_id, preference) DO UPDATE SET
           evidence_count = MAX(client_communication_preferences.evidence_count, excluded.evidence_count),
           last_observed = excluded.last_observed,
           confidence = excluded.confidence,
           origin = CASE WHEN client_communication_preferences.origin = 'human_entered'
                        THEN 'human_entered' ELSE excluded.origin END",
        params![
            customer_id,
            preference,
            evidence_count,
            first_observed,
            last_observed,
            confidence,
            origin,
            if origin == "human_entered" {
                "human"
            } else {
                "ai_generated"
            }
        ],
    )?;
    Ok(())
}

pub(crate) fn get_preferences(
    conn: &Connection,
    customer_id: i64,
) -> Result<Vec<CommunicationPreference>> {
    let mut stmt = conn.prepare(
        "SELECT preference, evidence_count, first_observed, last_observed, confidence, origin,
                human_override_value, human_override_reason, overridden_at
           FROM client_communication_preferences WHERE customer_id = ?
          ORDER BY evidence_count DESC",
    )?;
    let rows = stmt
        .query_map(params![customer_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();
    Ok(rows
        .into_iter()
        .map(
            |(
                preference,
                evidence_count,
                first_observed,
                last_observed,
                confidence,
                origin,
                hov,
                hor,
                hoa,
            )| {
                CommunicationPreference {
                    preference,
                    evidence_count,
                    first_observed,
                    last_observed,
                    confidence: confidence.unwrap_or_else(|| "unknown".into()),
                    origin: if origin == "human_entered" {
                        "human_entered".into()
                    } else {
                        "ai_inferred".into()
                    },
                    human_override: hov.map(|value| HumanOverride {
                        value,
                        reason: hor,
                        overridden_at: hoa.unwrap_or_default(),
                    }),
                }
            },
        )
        .collect())
}

pub(crate) fn get_active_overrides(
    conn: &Connection,
    customer_id: i64,
) -> Result<Vec<OverrideRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, field, ai_value, human_value, reason, created_at, active
           FROM client_human_overrides
          WHERE customer_id = ?1 AND active = 1
          ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![customer_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();
    Ok(rows
        .into_iter()
        .map(
            |(id, field, ai_value, human_value, reason, created_at, active)| OverrideRow {
                id,
                field,
                ai_value,
                human_value,
                reason,
                created_at,
                active: active == 1,
            },
        )
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_outcome(
    conn: &Connection,
    customer_id: i64,
    conversation_id: i64,
    resolved_after_first_response: Option<bool>,
    follow_up_count: i64,
    clarification_count: i64,
    escalated: bool,
    effort_score: Option<f64>,
    response_style: Option<&str>,
    friction: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO client_support_outcomes
             (customer_id, conversation_id, resolved_after_first_response, follow_up_count,
              clarification_count, escalated, effort_score, response_style, friction, computed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
         ON CONFLICT (conversation_id) DO UPDATE SET
           resolved_after_first_response = excluded.resolved_after_first_response,
           follow_up_count = excluded.follow_up_count,
           clarification_count = excluded.clarification_count,
           escalated = excluded.escalated,
           effort_score = excluded.effort_score,
           response_style = excluded.response_style,
           friction = excluded.friction,
           computed_at = datetime('now')",
        params![
            customer_id,
            conversation_id,
            resolved_after_first_response.map(i64::from),
            follow_up_count,
            clarification_count,
            i64::from(escalated),
            effort_score,
            response_style,
            friction,
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct OutcomeRow {
    pub conversation_id: i64,
    pub resolved_after_first_response: Option<i64>,
    pub follow_up_count: i64,
    pub clarification_count: i64,
    pub escalated: i64,
    pub effort_score: Option<f64>,
    pub response_style: Option<String>,
    pub friction: Option<String>,
}

pub(crate) fn get_outcomes_for_customer(
    conn: &Connection,
    customer_id: i64,
) -> Result<Vec<OutcomeRow>> {
    let mut stmt = conn.prepare(
        "SELECT o.conversation_id, o.resolved_after_first_response, o.follow_up_count,
                o.clarification_count, o.escalated, o.effort_score, o.response_style, o.friction
           FROM client_support_outcomes o
           JOIN conversations c ON c.id = o.conversation_id
          WHERE o.customer_id = ?1 AND c.deleted_at IS NULL
          ORDER BY c.created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![customer_id], |r| {
            Ok(OutcomeRow {
                conversation_id: r.get(0)?,
                resolved_after_first_response: r.get(1)?,
                follow_up_count: r.get(2)?,
                clarification_count: r.get(3)?,
                escalated: r.get(4)?,
                effort_score: r.get(5)?,
                response_style: r.get(6)?,
                friction: r.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[derive(Debug, Clone)]
pub struct CustomerConversation {
    pub id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub status: String,
    pub remote_created_at: Option<String>,
    pub thread_count: i64,
}

pub(crate) fn get_customer_conversations(
    conn: &Connection,
    customer_id: i64,
    exclude_conversation_id: Option<i64>,
) -> Result<Vec<CustomerConversation>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.number, c.subject, c.status, c.created_at,
           (SELECT COUNT(*) FROM conversation_threads t
             WHERE t.conversation_id = c.id AND t.deleted_at IS NULL) AS thread_count
           FROM conversations c
          WHERE c.customer_local_id = ?1 AND c.deleted_at IS NULL AND c.id != ?2
          ORDER BY c.created_at DESC",
    )?;
    let rows = stmt
        .query_map(
            params![customer_id, exclude_conversation_id.unwrap_or(-1)],
            |r| {
                Ok(CustomerConversation {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    subject: r.get(2)?,
                    status: r.get(3)?,
                    remote_created_at: r.get(4)?,
                    thread_count: r.get(5)?,
                })
            },
        )?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

struct ConversationInfo {
    customer_local_id: Option<i64>,
    subject: Option<String>,
    status: String,
}

fn conversation_info(conn: &Connection, conversation_id: i64) -> Option<ConversationInfo> {
    conn.query_row(
        "SELECT customer_local_id, subject, status
           FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
        params![conversation_id],
        |r| {
            Ok(ConversationInfo {
                customer_local_id: r.get(0)?,
                subject: r.get(1)?,
                status: r.get(2)?,
            })
        },
    )
    .ok()
}

pub(crate) fn conversation_customer(conn: &Connection, conversation_id: i64) -> Option<i64> {
    conversation_info(conn, conversation_id)?.customer_local_id
}

fn ratio(n: i64, d: i64) -> Option<f64> {
    (d > 0).then(|| ((n as f64 / d as f64) * 100.0).round() / 100.0)
}

fn humanize_style(style: &str) -> String {
    style
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn dimension_rank(dimension: &str, value: &str) -> Option<usize> {
    crate::interaction_signals::dimension_vocabulary(dimension)?
        .iter()
        .position(|v| *v == value)
}

fn rank_span(dimension: &str) -> usize {
    crate::interaction_signals::dimension_vocabulary(dimension)
        .map(|v| v.len().saturating_sub(1))
        .filter(|s| *s >= 1)
        .unwrap_or(1)
}

fn dominant_value(observations: &[&ObservationRow]) -> Option<String> {
    if observations.is_empty() {
        return None;
    }
    let now = chrono::Utc::now().timestamp_millis() as f64;
    let mut scores: std::collections::HashMap<&str, f64> = std::collections::HashMap::new();
    for o in observations {
        let age_days = parse_age_days(&o.observed_at, now);
        let weight = 0.5_f64.powf(age_days / crate::intelligence::RECENCY_HALF_LIFE_DAYS);
        *scores.entry(o.value.as_str()).or_insert(0.0) += weight;
    }
    scores
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(v, _)| v.to_string())
}

fn parse_age_days(observed_at: &str, now_ms: f64) -> f64 {
    let iso = format!("{}Z", observed_at.replace(' ', "T"));
    chrono::DateTime::parse_from_rfc3339(&iso)
        .map(|t| ((now_ms - t.timestamp_millis() as f64) / 86_400_000.0).max(0.0))
        .unwrap_or(999.0)
}

pub(crate) fn merge_signals(
    base: &[InteractionSignal],
    enrichment: &[InteractionSignal],
) -> Vec<InteractionSignal> {
    let mut by_dim: std::collections::BTreeMap<String, InteractionSignal> = base
        .iter()
        .map(|s| (s.dimension.clone(), s.clone()))
        .collect();
    for e in enrichment {
        by_dim.insert(e.dimension.clone(), e.clone());
    }
    by_dim.into_values().collect()
}

fn next_profile_version(conn: &Connection, customer_id: i64) -> i64 {
    conn.query_row(
        "SELECT MAX(profile_version) FROM client_behavior_baselines WHERE customer_id = ?1",
        params![customer_id],
        |r| r.get::<_, Option<i64>>(0),
    )
    .ok()
    .flatten()
    .unwrap_or(0)
        + 1
}

pub(crate) fn record_observations_for(
    conn: &Connection,
    current: &CurrentInteraction,
) -> Result<()> {
    let Some(customer_id) = current.customer_local_id else {
        return Ok(());
    };
    let conv_date: Option<String> = conn
        .query_row(
            "SELECT created_at FROM conversations WHERE id = ?1",
            params![current.conversation_local_id],
            |r| r.get(0),
        )
        .ok();
    let observed_at = conv_date
        .filter(|d| !d.is_empty())
        .map(|d| d.replace('T', " ").chars().take(19).collect::<String>())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
    let rows: Vec<ObservationRow> = current
        .signals
        .iter()
        .filter(|s| s.dimension != "response_preference" || s.confidence == "high")
        .map(|s| ObservationRow {
            customer_id,
            conversation_id: Some(current.conversation_local_id),
            thread_local_id: s.evidence.as_ref().and_then(|e| e.thread_local_id),
            dimension: s.dimension.clone(),
            value: s.value.clone(),
            confidence: s.confidence.clone(),
            evidence_excerpt: s.evidence.as_ref().map(|e| e.excerpt.clone()),
            source: "heuristic".into(),
            observed_at: observed_at.clone(),
        })
        .collect();
    if !rows.is_empty() {
        insert_observations(conn, &rows)?;
    }
    Ok(())
}

#[cfg(test)]
mod batch30_tests {
    use super::*;
    use rusqlite::{Connection, params};

    #[test]
    fn compute_outcome_survives_synced_threads_with_null_remote_stamps() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (42, 'Casey')",
            [],
        )
        .expect("customer");
        let customer: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, customer_local_id)
             VALUES (77, 77, 'outcome', 'p', 'closed', ?1)",
            params![customer],
        )
        .expect("conversation");
        let conv: i64 = conn.last_insert_rowid();
        for (offset, kind, body) in [
            (
                "-3 days",
                "reply",
                "We found a bug in the exporter; a fix is out.",
            ),
            ("-4 days", "customer", "The export fails every time we try."),
            (
                "-2 days",
                "customer",
                "Now the attachments fail to upload after the export.",
            ),
        ] {
            conn.execute(
                "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
                 VALUES (?1, ?2, 'published', ?3, datetime('now', ?4))",
                params![conv, kind, body, offset],
            )
            .expect("thread");
        }

        let outcome = compute_outcome(&conn, conv)
            .expect("compute")
            .expect("an outcome for a closed, replied conversation");
        assert_eq!(outcome.follow_up_count, 1, "one follow-up customer message");
        assert_eq!(
            outcome.resolved_after_first_response,
            Some(false),
            "a follow-up after the first reply means not resolved on first response: {outcome:?}"
        );
    }
}
