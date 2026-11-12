use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::ai_lm_studio::OpenAiCompatibleClient;
use crate::ai_pipeline::AiBackend;
use crate::error::Result;
use crate::settings;

pub(crate) const SETTINGS_ENDPOINTS: &str = "ai_endpoints";
pub(crate) const SETTINGS_ROLES: &str = "ai_model_roles";
pub(crate) const ENDPOINT_KEY_PREFIX: &str = "ai_endpoint_key_";

pub(crate) const DEFAULT_LMSTUDIO_URL: &str = "http://127.0.0.1:1234/v1";
pub(crate) const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiRole {
    Embed,
    Classify,
    Summarize,
    Draft,
    Agent,
}

impl AiRole {
    pub const ALL: [Self; 5] = [
        Self::Embed,
        Self::Classify,
        Self::Summarize,
        Self::Draft,
        Self::Agent,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Embed => "embed",
            Self::Classify => "classify",
            Self::Summarize => "summarize",
            Self::Draft => "draft",
            Self::Agent => "agent",
        }
    }

    #[must_use]
    pub fn note(self) -> &'static str {
        match self {
            Self::Embed => "embeddings — local by default; cloud is an explicit opt-in",
            Self::Classify => "triage, sentiment, topic, urgency — high volume, low stakes",
            Self::Summarize => "thread summaries, memory extraction, notes, digest",
            Self::Draft => "customer-facing drafts — the only role where cloud is recommended",
            Self::Agent => "the tool-calling copilot chat",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub enabled: bool,
    #[serde(default)]
    pub price_prompt_per_mtok: f64,
    #[serde(default)]
    pub price_completion_per_mtok: f64,
    #[serde(default)]
    pub builtin: bool,
}

impl EndpointConfig {
    #[must_use]
    pub fn secret_key(id: &str) -> String {
        format!("{ENDPOINT_KEY_PREFIX}{id}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub endpoint: String,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleMap {
    #[serde(default)]
    pub embed: Option<RoleAssignment>,
    #[serde(default)]
    pub classify: Option<RoleAssignment>,
    #[serde(default)]
    pub summarize: Option<RoleAssignment>,
    #[serde(default)]
    pub draft: Option<RoleAssignment>,
    #[serde(default)]
    pub agent: Option<RoleAssignment>,
}

impl RoleMap {
    #[must_use]
    pub fn get(&self, role: AiRole) -> Option<&RoleAssignment> {
        match role {
            AiRole::Embed => self.embed.as_ref(),
            AiRole::Classify => self.classify.as_ref(),
            AiRole::Summarize => self.summarize.as_ref(),
            AiRole::Draft => self.draft.as_ref(),
            AiRole::Agent => self.agent.as_ref(),
        }
    }

    pub fn set(&mut self, role: AiRole, assignment: Option<RoleAssignment>) {
        let slot = match role {
            AiRole::Embed => &mut self.embed,
            AiRole::Classify => &mut self.classify,
            AiRole::Summarize => &mut self.summarize,
            AiRole::Draft => &mut self.draft,
            AiRole::Agent => &mut self.agent,
        };
        *slot = assignment;
    }
}

pub fn load_endpoints(conn: &Connection) -> Result<Vec<EndpointConfig>> {
    if let Some(list) = settings::get_json::<Vec<EndpointConfig>>(conn, SETTINGS_ENDPOINTS)? {
        return Ok(list);
    }
    let legacy_url = settings::get_string(conn, "lmstudio_base_url")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_LMSTUDIO_URL.to_string());
    let seeded = vec![
        EndpointConfig {
            id: "lmstudio".into(),
            name: "LM Studio (local)".into(),
            base_url: legacy_url,
            enabled: true,
            price_prompt_per_mtok: 0.0,
            price_completion_per_mtok: 0.0,
            builtin: true,
        },
        EndpointConfig {
            id: "ollama".into(),
            name: "Ollama (local, optional)".into(),
            base_url: DEFAULT_OLLAMA_URL.into(),
            enabled: true,
            price_prompt_per_mtok: 0.0,
            price_completion_per_mtok: 0.0,
            builtin: true,
        },
    ];
    settings::set_json(conn, SETTINGS_ENDPOINTS, &seeded)?;
    Ok(seeded)
}

pub fn save_endpoints(conn: &Connection, list: &[EndpointConfig]) -> Result<()> {
    settings::set_json(conn, SETTINGS_ENDPOINTS, &list.to_vec())
}

pub fn load_roles(conn: &Connection) -> Result<RoleMap> {
    Ok(settings::get_json::<RoleMap>(conn, SETTINGS_ROLES)?.unwrap_or_default())
}

pub fn save_roles(conn: &Connection, map: &RoleMap) -> Result<()> {
    settings::set_json(conn, SETTINGS_ROLES, map)
}

pub fn set_endpoint_api_key(conn: &Connection, id: &str, key: &str) -> Result<()> {
    settings::set_secret(conn, &EndpointConfig::secret_key(id), key.as_bytes())
}

pub fn get_endpoint_api_key(conn: &Connection, id: &str) -> Option<String> {
    match settings::get_secret(conn, &EndpointConfig::secret_key(id)) {
        Ok(bytes) => bytes
            .and_then(|b| String::from_utf8(b).ok())
            .filter(|k| !k.is_empty()),
        Err(error) => {
            tracing::warn!(endpoint = id, error = %error, "endpoint api-key envelope unreadable; treating as unset");
            None
        }
    }
}

pub fn endpoint_has_key(conn: &Connection, id: &str) -> bool {
    settings::has_secret(conn, &EndpointConfig::secret_key(id)).unwrap_or(false)
}

pub fn remove_endpoint(conn: &Connection, id: &str) -> Result<()> {
    let _ = settings::delete_secret(conn, &EndpointConfig::secret_key(id));
    let mut list: Vec<EndpointConfig> = load_endpoints(conn)?;
    list.retain(|e| e.id != id);
    save_endpoints(conn, &list)?;
    let mut roles = load_roles(conn)?;
    for role in AiRole::ALL {
        if roles.get(role).is_some_and(|a| a.endpoint == id) {
            roles.set(role, None);
        }
    }
    save_roles(conn, &roles)
}

pub fn update_endpoint_prices(
    conn: &Connection,
    id: &str,
    price_prompt_per_mtok: f64,
    price_completion_per_mtok: f64,
) -> Result<bool> {
    let mut list: Vec<EndpointConfig> = load_endpoints(conn)?;
    let Some(endpoint) = list.iter_mut().find(|e| e.id == id) else {
        return Ok(false);
    };
    endpoint.price_prompt_per_mtok = price_prompt_per_mtok;
    endpoint.price_completion_per_mtok = price_completion_per_mtok;
    save_endpoints(conn, &list)?;
    Ok(true)
}

fn endpoint_by_id<'a>(list: &'a [EndpointConfig], id: &str) -> Option<&'a EndpointConfig> {
    list.iter().find(|e| e.id == id && e.enabled)
}

pub fn backend_configured(conn: &Connection) -> bool {
    if AiRole::ALL
        .iter()
        .any(|role| resolve_role(conn, *role).is_some())
    {
        return true;
    }
    settings::get_string(conn, "lmstudio_base_url")
        .ok()
        .flatten()
        .is_some_and(|s| !s.is_empty())
}

#[must_use]
pub(crate) fn resolve_role(
    conn: &Connection,
    role: AiRole,
) -> Option<(EndpointConfig, Option<String>, Option<String>)> {
    let list = load_endpoints(conn).ok()?;
    let roles = load_roles(conn).ok()?;
    let assignment = roles.get(role)?.clone();
    let endpoint = endpoint_by_id(&list, &assignment.endpoint)?.clone();
    let api_key = get_endpoint_api_key(conn, &endpoint.id);
    Some((endpoint, assignment.model, api_key))
}

pub fn backend_for_role(conn: &Connection, role: AiRole) -> AiBackend {
    if !settings::get_bool(conn, "ai_enabled", true).unwrap_or(true) {
        return AiBackend::Disabled;
    }
    let timeout_ms =
        u64::try_from(settings::get_i64(conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000))
            .unwrap_or(crate::ai_lm_studio::LM_STUDIO_DEFAULT_TIMEOUT_MS);
    if let Some((endpoint, model, api_key)) = resolve_role(conn, role) {
        let mut client = OpenAiCompatibleClient::new_with_timeout(&endpoint.base_url, timeout_ms);
        if let Some(key) = api_key {
            client = client.with_api_key(&key);
        }
        return AiBackend::LmStudio {
            client,
            model,
            endpoint_id: Some(endpoint.id),
        };
    }
    if matches!(role, AiRole::Embed) {
        return AiBackend::Disabled;
    }
    crate::ai_pipeline::backend_from_settings(conn)
}

pub const AI_FEATURES: [(&str, &str, bool); 16] = [
    ("ticket_analysis", "automatic_analysis_enabled", true),
    ("ai_note", "automatic_note_enabled", false),
    ("draft", "automatic_draft_enabled", false),
    ("attributes", "ai_feature_attributes_enabled", true),
    (
        "memory_extraction",
        "ai_feature_memory_extraction_enabled",
        true,
    ),
    (
        "issue_clustering",
        "ai_feature_issue_clustering_enabled",
        true,
    ),
    ("copilot", "ai_feature_copilot_enabled", true),
    ("draft_coaching", "ai_feature_draft_coaching_enabled", true),
    ("translation", "ai_feature_translation_enabled", true),
    (
        "post_resolution_qa",
        "ai_feature_post_resolution_qa_enabled",
        true,
    ),
    (
        "interaction_intelligence",
        "ai_feature_interaction_intelligence_enabled",
        true,
    ),
    ("retrieval", "ai_feature_retrieval_enabled", true),
    ("golden_eval", "ai_feature_golden_eval_enabled", true),
    ("gap_detection", "ai_feature_gap_detection_enabled", true),
    (
        "article_drafting",
        "ai_feature_article_drafting_enabled",
        true,
    ),
    ("weekly_digest", "ai_feature_weekly_digest_enabled", true),
];

pub fn ai_feature_enabled(conn: &Connection, feature: &str) -> bool {
    if !settings::get_bool(conn, "ai_enabled", true).unwrap_or(true) {
        return false;
    }
    for (name, key, default) in AI_FEATURES {
        if name == feature {
            return settings::get_bool(conn, key, default).unwrap_or(default);
        }
    }
    false
}

pub fn set_ai_feature_enabled(conn: &Connection, feature: &str, value: bool) -> Result<()> {
    for (name, key, _) in AI_FEATURES {
        if name == feature {
            return settings::set_bool(conn, key, value);
        }
    }
    Err(crate::error::Error::Config(format!(
        "unknown AI feature {feature:?}"
    )))
}

#[derive(Debug, Clone)]
pub struct EndpointProbe {
    pub ok: bool,
    pub model_count: usize,
    pub latency_ms: u64,
    pub message: String,
    pub models: Vec<String>,
}

pub async fn probe_endpoint(
    base_url: &str,
    timeout_ms: u64,
    api_key: Option<&str>,
) -> EndpointProbe {
    let mut client = OpenAiCompatibleClient::new_with_timeout(base_url, timeout_ms);
    if let Some(key) = api_key {
        client = client.with_api_key(key);
    }
    let started = std::time::Instant::now();
    match client.list_models().await {
        Ok(models) => {
            let latency_ms = started.elapsed().as_millis() as u64;
            EndpointProbe {
                ok: true,
                model_count: models.len(),
                latency_ms,
                message: format!("reachable — {} model(s)", models.len()),
                models: models.into_iter().map(|m| m.id).collect(),
            }
        }
        Err(e) => EndpointProbe {
            ok: false,
            model_count: 0,
            latency_ms: started.elapsed().as_millis() as u64,
            message: e.to_string(),
            models: Vec::new(),
        },
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageRow {
    pub day: String,
    pub feature: String,
    pub model: String,
    pub endpoint_id: Option<String>,
    pub runs: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub latency_ms_total: i64,
    pub est_cost_usd: f64,
}

pub fn usage_ledger(conn: &Connection, days: i64) -> Result<Vec<UsageRow>> {
    let endpoints = load_endpoints(conn).unwrap_or_default();
    let price_for = |endpoint_id: Option<&str>| -> (f64, f64) {
        let Some(id) = endpoint_id else {
            return (0.0, 0.0);
        };
        let Some(e) = endpoints.iter().find(|e| e.id == id) else {
            return (0.0, 0.0);
        };
        (e.price_prompt_per_mtok, e.price_completion_per_mtok)
    };
    let mut stmt = conn.prepare(
        "SELECT date(COALESCE(started_at, created_at)), type, model, endpoint_id, COUNT(*),
                COALESCE(SUM(CAST(json_extract(token_usage, '$.prompt_tokens') AS INTEGER)), 0),
                COALESCE(SUM(CAST(json_extract(token_usage, '$.completion_tokens') AS INTEGER)), 0),
                COALESCE(SUM(latency_ms), 0)
           FROM ai_runs
          WHERE status = 'completed'
            AND COALESCE(started_at, created_at) >= datetime('now', ?1)
          GROUP BY date(COALESCE(started_at, created_at)), type, model, endpoint_id
          ORDER BY date(COALESCE(started_at, created_at)) DESC, type",
    )?;
    let rows = stmt
        .query_map([format!("-{days} days")], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })?
        .filter_map(|row| row.ok())
        .collect::<Vec<_>>();
    drop(stmt);
    Ok(rows
        .into_iter()
        .map(
            |(day, feature, model, endpoint_id, runs, prompt, completion, latency)| {
                let (pp, pc) = price_for(endpoint_id.as_deref());
                let est_cost_usd =
                    prompt as f64 / 1_000_000.0 * pp + completion as f64 / 1_000_000.0 * pc;
                UsageRow {
                    day,
                    feature,
                    model,
                    endpoint_id,
                    runs,
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    latency_ms_total: latency,
                    est_cost_usd,
                }
            },
        )
        .collect())
}

pub(crate) fn apply_m054_ai_runs_endpoint(conn: &Connection) -> Result<()> {
    if !crate::sync_schema::table_exists(conn, "ai_runs")? {
        return Ok(());
    }
    let has: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ai_runs') WHERE name = 'endpoint_id'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !has {
        conn.execute_batch("ALTER TABLE ai_runs ADD COLUMN endpoint_id TEXT;")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn key_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::settings::SECRETS_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn hub_seeds_from_legacy_settings_and_resolves_roles() {
        let _g = key_guard();
        crate::settings::set_master_key_for_tests([7u8; 32]);
        let conn = conn();
        crate::settings::set_string(&conn, "lmstudio_base_url", "http://legacy:1234/v1")
            .expect("set");

        let endpoints = load_endpoints(&conn).expect("endpoints");
        assert!(
            endpoints
                .iter()
                .any(|e| e.id == "lmstudio" && e.base_url == "http://legacy:1234/v1" && e.builtin)
        );
        assert!(endpoints.iter().any(|e| e.id == "ollama" && e.builtin));
        assert_eq!(load_endpoints(&conn).expect("again").len(), endpoints.len());

        let cloud = EndpointConfig {
            id: "cloudy".into(),
            name: "Cloud (OpenAI-compatible)".into(),
            base_url: "https://api.example.com/v1".into(),
            enabled: true,
            price_prompt_per_mtok: 1.5,
            price_completion_per_mtok: 6.0,
            builtin: false,
        };
        let mut list = load_endpoints(&conn).expect("list");
        list.push(cloud.clone());
        save_endpoints(&conn, &list).expect("save");
        set_endpoint_api_key(&conn, "cloudy", "sk-test-1234").expect("key");
        let _ = &cloud;
        let mut roles = load_roles(&conn).expect("roles");
        roles.set(
            AiRole::Draft,
            Some(RoleAssignment {
                endpoint: "cloudy".into(),
                model: Some("gpt-test".into()),
            }),
        );
        save_roles(&conn, &roles).expect("save roles");

        let (endpoint, model, api_key) = resolve_role(&conn, AiRole::Draft).expect("resolve");
        assert_eq!(endpoint.id, "cloudy");
        assert_eq!(model.as_deref(), Some("gpt-test"));
        assert_eq!(api_key.as_deref(), Some("sk-test-1234"));
        assert!(resolve_role(&conn, AiRole::Classify).is_none());
    }

    #[test]
    fn removing_an_endpoint_clears_its_key_and_assignments() {
        let _g = key_guard();
        crate::settings::set_master_key_for_tests([7u8; 32]);
        let conn = conn();
        let mut list = load_endpoints(&conn).expect("seed");
        list.push(EndpointConfig {
            id: "temp".into(),
            name: "Temporary".into(),
            base_url: "http://temp:1/v1".into(),
            enabled: true,
            price_prompt_per_mtok: 0.0,
            price_completion_per_mtok: 0.0,
            builtin: false,
        });
        save_endpoints(&conn, &list).expect("save");
        set_endpoint_api_key(&conn, "temp", "k").expect("key");
        let mut roles = load_roles(&conn).expect("roles");
        roles.set(
            AiRole::Summarize,
            Some(RoleAssignment {
                endpoint: "temp".into(),
                model: Some("m".into()),
            }),
        );
        save_roles(&conn, &roles).expect("roles");

        remove_endpoint(&conn, "temp").expect("remove");
        assert!(
            !load_endpoints(&conn)
                .expect("list")
                .iter()
                .any(|e| e.id == "temp")
        );
        assert!(!endpoint_has_key(&conn, "temp"));
        assert!(resolve_role(&conn, AiRole::Summarize).is_none());
    }

    #[test]
    fn feature_gate_honors_the_kill_switch_and_defaults() {
        let conn = conn();
        crate::settings::set_bool(&conn, "ai_enabled", false).expect("off");
        assert!(!ai_feature_enabled(&conn, "attributes"));
        crate::settings::set_bool(&conn, "ai_enabled", true).expect("on");
        assert!(ai_feature_enabled(&conn, "ticket_analysis"));
        assert!(!ai_feature_enabled(&conn, "ai_note"));
        assert!(!ai_feature_enabled(&conn, "draft"));
        assert!(ai_feature_enabled(&conn, "attributes"));
        assert!(ai_feature_enabled(&conn, "memory_extraction"));
        assert!(ai_feature_enabled(&conn, "issue_clustering"));
        assert!(ai_feature_enabled(&conn, "copilot"));
        assert!(!ai_feature_enabled(&conn, "nope"));
        set_ai_feature_enabled(&conn, "issue_clustering", false).expect("set");
        assert!(!ai_feature_enabled(&conn, "issue_clustering"));
    }

    #[test]
    fn ledger_prices_runs_by_endpoint() {
        let _g = key_guard();
        crate::settings::set_master_key_for_tests([7u8; 32]);
        let conn = conn();
        let mut list = load_endpoints(&conn).expect("seed");
        list.push(EndpointConfig {
            id: "priced".into(),
            name: "Priced".into(),
            base_url: "http://priced:1/v1".into(),
            enabled: true,
            price_prompt_per_mtok: 2.0,
            price_completion_per_mtok: 8.0,
            builtin: false,
        });
        save_endpoints(&conn, &list).expect("save");

        conn.execute(
            "INSERT INTO ai_runs (type, model, endpoint_id, status, latency_ms, started_at, token_usage, input_hash, prompt_version, response_json)
             VALUES ('ticket_analysis', 'gpt-test', 'priced', 'completed', 1200,
                     datetime('now'), '{\"prompt_tokens\":500000,\"completion_tokens\":250000,\"total_tokens\":750000}', 'seed', 'seedv', '{}')",
            [],
        )
        .expect("run");
        conn.execute(
            "INSERT INTO ai_runs (type, model, status, latency_ms, started_at, input_hash, prompt_version, response_json)
             VALUES ('ticket_analysis', 'old-model', 'completed', 800, datetime('now'), 'seed2', 'seedv', '{}')",
            [],
        )
        .expect("legacy run");

        let ledger = usage_ledger(&conn, 7).expect("ledger");
        let priced = ledger
            .iter()
            .find(|r| r.endpoint_id.as_deref() == Some("priced"))
            .expect("priced row");
        assert_eq!(priced.prompt_tokens, 500_000);
        assert_eq!(priced.completion_tokens, 250_000);
        let expected = 500_000.0 / 1_000_000.0 * 2.0 + 250_000.0 / 1_000_000.0 * 8.0;
        assert!(
            (priced.est_cost_usd - expected).abs() < 1e-9,
            "{} vs {expected}",
            priced.est_cost_usd
        );
        let legacy = ledger
            .iter()
            .find(|r| r.endpoint_id.is_none())
            .expect("legacy row");
        assert_eq!(legacy.prompt_tokens, 0);
        assert_eq!(legacy.est_cost_usd, 0.0);
        assert_eq!(legacy.runs, 1);
    }
    #[test]
    fn audp16_update_endpoint_prices_edits_and_reports_unknown_ids() {
        let _g = key_guard();
        crate::settings::set_master_key_for_tests([7u8; 32]);
        let conn = conn();
        let mut list = load_endpoints(&conn).expect("seed");
        list.push(EndpointConfig {
            id: "priced".into(),
            name: "Priced".into(),
            base_url: "http://priced:1/v1".into(),
            enabled: true,
            price_prompt_per_mtok: 0.0,
            price_completion_per_mtok: 0.0,
            builtin: false,
        });
        save_endpoints(&conn, &list).expect("save");

        assert!(
            update_endpoint_prices(&conn, "priced", 1.25, 6.0).expect("update"),
            "the edit lands"
        );
        let after = load_endpoints(&conn).expect("reload");
        let endpoint = after.iter().find(|e| e.id == "priced").expect("priced");
        assert_eq!(endpoint.price_prompt_per_mtok, 1.25);
        assert_eq!(endpoint.price_completion_per_mtok, 6.0);

        assert!(
            !update_endpoint_prices(&conn, "nope", 1.0, 1.0).expect("no-op"),
            "unknown ids report false, not an error"
        );
    }
}
