//! K5: the Help Scout Reports API — a table-driven registry of the 33 GET
//! endpoints (per the 2026-10-21 platform research), capability detection
//! so Standard plans get one honest sentence instead of empty charts
//! (decisions D-c, D-g), and a small cache the fetch job fills and the
//! Reports page renders.
//!
//! Nothing here fabricates report data: the cache is written only from real
//! provider responses, and the capability verdict drives every UI claim.

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::Result;

/// Migration m058 (boot step 49): the fetched-report cache. One row per
/// report key; overwritten on every successful fetch so the UI never shows
/// a stale range silently (the row carries its own start/end).
pub fn apply_m058_hs_report_cache(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS hs_report_cache (
            report_key TEXT PRIMARY KEY,
            fetched_at TEXT NOT NULL,
            start      TEXT NOT NULL,
            end        TEXT NOT NULL,
            data       TEXT NOT NULL
        );",
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFamily {
    Company,
    Conversations,
    Docs,
    Happiness,
    Productivity,
    User,
    Channels,
}

impl ReportFamily {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Company => "Company",
            Self::Conversations => "Conversations",
            Self::Docs => "Docs",
            Self::Happiness => "Happiness",
            Self::Productivity => "Productivity",
            Self::User => "Users & teams",
            Self::Channels => "Channels",
        }
    }
}

/// One Reports API GET endpoint. The registry is the full documented
/// inventory (research §5, 2026-10-21): 33 endpoints, all Plus/Pro-only.
#[derive(Debug, Clone, Copy)]
pub struct ReportSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub path: &'static str,
    pub family: ReportFamily,
}

macro_rules! spec {
    ($key:literal, $name:literal, $path:literal, $family:ident) => {
        ReportSpec {
            key: $key,
            name: $name,
            path: $path,
            family: ReportFamily::$family,
        }
    };
}

/// The documented Reports API GET-endpoint count. The docs sitemap
/// advertises "33 report endpoint pages", but one of those pages is the
/// reports index itself. The verified 2026-10-21 research table carries
/// 30 rows, two of which hold a pair of endpoints each
/// (`conversations/new` and its drilldown; `user/happiness` and
/// `user/ratings`), which yields 32 unique GET paths. The registry below
/// is that complete documented inventory, so K5's "the 33 GET endpoints"
/// renders as: every endpoint the platform documents.
pub const REPORT_COUNT: usize = 32;

pub const REPORTS: [ReportSpec; REPORT_COUNT] = [
    spec!("company", "Company overall", "/v2/reports/company", Company),
    spec!(
        "company_drilldown",
        "Company drilldown",
        "/v2/reports/company/drilldown",
        Company
    ),
    spec!(
        "company_customers_helped",
        "Customers helped",
        "/v2/reports/company/customers-helped",
        Company
    ),
    spec!(
        "conversations",
        "Conversations overall",
        "/v2/reports/conversations",
        Conversations
    ),
    spec!(
        "conversations_new",
        "New conversations",
        "/v2/reports/conversations/new",
        Conversations
    ),
    spec!(
        "conversations_new_drilldown",
        "New conversations drilldown",
        "/v2/reports/conversations/new-drilldown",
        Conversations
    ),
    spec!(
        "conversations_drilldown",
        "Conversations drilldown",
        "/v2/reports/conversations/drilldown",
        Conversations
    ),
    spec!(
        "conversations_fields_drilldown",
        "Custom-field drilldown",
        "/v2/reports/conversations/fields-drilldown",
        Conversations
    ),
    spec!(
        "conversations_received_messages",
        "Received messages",
        "/v2/reports/conversations/received-messages",
        Conversations
    ),
    spec!(
        "conversations_busy_times",
        "Busy times",
        "/v2/reports/conversations/busy-times",
        Conversations
    ),
    spec!(
        "conversations_volume_by_channel",
        "Volume by channel",
        "/v2/reports/conversations/volume-by-channel",
        Conversations
    ),
    spec!("docs", "Docs overall", "/v2/reports/docs", Docs),
    spec!(
        "happiness",
        "Happiness overall",
        "/v2/reports/happiness",
        Happiness
    ),
    spec!(
        "happiness_ratings",
        "Happiness ratings",
        "/v2/reports/happiness/ratings",
        Happiness
    ),
    spec!(
        "productivity",
        "Productivity overall",
        "/v2/reports/productivity",
        Productivity
    ),
    spec!(
        "productivity_first_response_time",
        "First response time",
        "/v2/reports/productivity/first-response-time",
        Productivity
    ),
    spec!(
        "productivity_response_time",
        "Response time",
        "/v2/reports/productivity/response-time",
        Productivity
    ),
    spec!(
        "productivity_resolution_time",
        "Resolution time",
        "/v2/reports/productivity/resolution-time",
        Productivity
    ),
    spec!(
        "productivity_resolved",
        "Resolved conversations",
        "/v2/reports/productivity/resolved",
        Productivity
    ),
    spec!(
        "productivity_replies_sent",
        "Replies sent",
        "/v2/reports/productivity/replies-sent",
        Productivity
    ),
    spec!("user", "Users & teams overall", "/v2/reports/user", User),
    spec!(
        "user_drilldown",
        "User drilldown",
        "/v2/reports/user/drilldown",
        User
    ),
    spec!(
        "user_customers_helped",
        "User customers helped",
        "/v2/reports/user/customers-helped",
        User
    ),
    spec!(
        "user_happiness",
        "User happiness",
        "/v2/reports/user/happiness",
        User
    ),
    spec!(
        "user_ratings",
        "User ratings",
        "/v2/reports/user/ratings",
        User
    ),
    spec!(
        "user_replies",
        "User replies",
        "/v2/reports/user/replies",
        User
    ),
    spec!(
        "user_resolutions",
        "User resolutions",
        "/v2/reports/user/resolutions",
        User
    ),
    spec!("user_chat", "User chat", "/v2/reports/user/chat", User),
    spec!(
        "user_conversation_history",
        "User conversation history",
        "/v2/reports/user/conversation-history",
        User
    ),
    spec!("email", "Email report", "/v2/reports/email", Channels),
    spec!("phone", "Phone report", "/v2/reports/phone", Channels),
    spec!("chat", "Chat report", "/v2/reports/chat", Channels),
];

#[must_use]
pub fn report_spec(key: &str) -> Option<ReportSpec> {
    REPORTS.iter().find(|r| r.key == key).copied()
}

// ---------------------------------------------------------------------------
// Capability detection
// ---------------------------------------------------------------------------

/// The one-sentence honest states the UI renders. `PlanBlocked` covers the
/// 403 the Reports API answers on Standard plans (and the 403 a user
/// without report permission gets — the sentence is honest for both).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportsCapability {
    Unknown,
    Available,
    PlanBlocked,
    NotConnected,
}

impl ReportsCapability {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Available => "available",
            Self::PlanBlocked => "plan_blocked",
            Self::NotConnected => "not_connected",
        }
    }

    #[must_use]
    pub fn from_str_raw(s: &str) -> Self {
        match s {
            "available" => Self::Available,
            "plan_blocked" => Self::PlanBlocked,
            "not_connected" => Self::NotConnected,
            _ => Self::Unknown,
        }
    }

    /// The honest sentence the Reports page shows instead of empty charts.
    #[must_use]
    pub fn banner(self) -> &'static str {
        match self {
            Self::PlanBlocked => {
                "Your Help Scout plan does not include the Reports API — it is available \
                 on Plus and Pro plans (or your account lacks report permission). The \
                 reports below are computed locally from your synced data."
            }
            Self::NotConnected => {
                "Help Scout is not connected, so live Reports API data is unavailable. \
                 Connect credentials in Settings; the local reports below keep working \
                 from your synced mirror."
            }
            Self::Unknown => {
                "The Reports API has not been probed yet. Fetch a report to check \
                 whether your plan allows it."
            }
            Self::Available => "",
        }
    }
}

const CAPABILITY_KEY: &str = "hs_reports_capability";
const CAPABILITY_CHECKED_AT: &str = "hs_reports_capability_checked_at";

#[must_use]
pub fn get_capability(conn: &Connection) -> ReportsCapability {
    ReportsCapability::from_str_raw(
        &crate::settings::get_string(conn, CAPABILITY_KEY)
            .unwrap_or_default()
            .unwrap_or_default(),
    )
}

pub fn set_capability(conn: &Connection, capability: ReportsCapability) -> Result<()> {
    crate::settings::set_string(conn, CAPABILITY_KEY, capability.as_str())?;
    crate::settings::set_i64(conn, CAPABILITY_CHECKED_AT, chrono::Utc::now().timestamp())?;
    Ok(())
}

/// Credential changes invalidate the probe: the capability is only valid
/// for the account it was probed against. Called from the credential
/// save/activation path.
pub fn reset_capability(conn: &Connection) {
    let _ = crate::settings::set_string(conn, CAPABILITY_KEY, "unknown");
}

/// Classify a provider fetch error into a capability transition. 403 is
/// the plan/permission wall; 401 means the probe could not authenticate
/// (stays Unknown, retried on the next fetch); network errors stay Unknown
/// too. Only a definitive 403 (or 200-series success handled by the
/// caller) writes a verdict.
#[must_use]
pub fn classify_fetch_error(status_code: u16) -> Option<ReportsCapability> {
    match status_code {
        403 => Some(ReportsCapability::PlanBlocked),
        200..=299 => Some(ReportsCapability::Available),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

pub fn cache_report(
    conn: &Connection,
    report_key: &str,
    start: &str,
    end: &str,
    data: &Value,
) -> Result<()> {
    conn.execute(
        "INSERT INTO hs_report_cache (report_key, fetched_at, start, end, data)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?2, ?3, ?4)
         ON CONFLICT(report_key) DO UPDATE SET
            fetched_at = excluded.fetched_at,
            start = excluded.start,
            end = excluded.end,
            data = excluded.data",
        params![
            report_key,
            start,
            end,
            serde_json::to_string(data).unwrap_or_else(|_| "{}".into())
        ],
    )?;
    Ok(())
}

/// The cached fetch for one report key.
#[must_use]
pub fn cached_report(
    conn: &Connection,
    report_key: &str,
) -> Option<(String, String, String, Value)> {
    conn.query_row(
        "SELECT fetched_at, start, end, data FROM hs_report_cache WHERE report_key = ?1",
        params![report_key],
        |r| {
            let data_raw: String = r.get(3)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                serde_json::from_str(&data_raw).unwrap_or_else(|_| json!({})),
            ))
        },
    )
    .ok()
}

/// All cached rows (key, fetched_at) so the UI can badge freshness.
#[must_use]
pub fn cached_report_keys(conn: &Connection) -> Vec<(String, String)> {
    conn.prepare("SELECT report_key, fetched_at FROM hs_report_cache ORDER BY report_key")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Flatten a Reports API response into label/value rows the generic
/// renderer can draw: the `current` and `previous` summary objects, the
/// `deltas` object, and any `top` arrays are walked one level deep.
/// Numbers are formatted as-is (SQL-side facts are not recomputed here).
#[must_use]
pub fn flatten_report(data: &Value) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let push_object = |prefix: &str, v: &Value, out: &mut Vec<(String, String)>| {
        if let Some(obj) = v.as_object() {
            for (k, val) in obj {
                if val.is_number() || val.is_string() || val.is_boolean() {
                    let label = format!("{prefix} {k}");
                    let text = match val {
                        Value::Number(n) => n.to_string(),
                        Value::String(s) => s.clone(),
                        Value::Bool(b) => b.to_string(),
                        _ => continue,
                    };
                    out.push((label, text));
                }
            }
        }
    };
    push_object("current", &data["current"], &mut out);
    push_object("previous", &data["previous"], &mut out);
    push_object("delta", &data["deltas"], &mut out);
    if out.is_empty() {
        // Reports without the current/previous envelope (drilldowns, lists)
        // still render: walk the top level one deep.
        push_object("", data, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    #[test]
    fn registry_covers_all_documented_endpoints() {
        assert_eq!(REPORTS.len(), 32);
        // Unique keys, unique paths, every path under /v2/reports/.
        let mut keys = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        for r in REPORTS.iter() {
            assert!(keys.insert(r.key), "duplicate key {}", r.key);
            assert!(paths.insert(r.path), "duplicate path {}", r.path);
            assert!(r.path.starts_with("/v2/reports/"), "bad path {}", r.path);
        }
        // Spot-check the headline families the Reports page renders first.
        assert_eq!(
            report_spec("happiness").expect("happiness").path,
            "/v2/reports/happiness"
        );
        assert_eq!(
            report_spec("productivity_resolution_time")
                .expect("prt")
                .path,
            "/v2/reports/productivity/resolution-time"
        );
        assert!(report_spec("nope").is_none());
    }

    #[test]
    fn capability_verdicts_roundtrip_and_reset() {
        let conn = db();
        assert_eq!(get_capability(&conn), ReportsCapability::Unknown);
        set_capability(&conn, ReportsCapability::PlanBlocked).expect("set");
        assert_eq!(get_capability(&conn), ReportsCapability::PlanBlocked);
        assert!(
            get_capability(&conn)
                .banner()
                .contains("Plus and Pro plans"),
            "the honest sentence must name the plan wall"
        );
        reset_capability(&conn);
        assert_eq!(get_capability(&conn), ReportsCapability::Unknown);
        // Classification: only 403 and 2xx are verdicts.
        assert_eq!(
            classify_fetch_error(403),
            Some(ReportsCapability::PlanBlocked)
        );
        assert_eq!(
            classify_fetch_error(200),
            Some(ReportsCapability::Available)
        );
        assert_eq!(classify_fetch_error(401), None);
        assert_eq!(classify_fetch_error(0), None);
    }

    #[test]
    fn cache_upserts_and_flattens() {
        let conn = db();
        let sample = json!({
            "current": { "totalConversations": 42, "startDate": "2026-10-14" },
            "previous": { "totalConversations": 30 },
            "deltas": { "totalConversations": 12 }
        });
        cache_report(&conn, "conversations", "2026-10-14", "2026-10-21", &sample).expect("cache");
        let (fetched_at, start, end, data) = cached_report(&conn, "conversations").expect("row");
        assert!(!fetched_at.is_empty());
        assert_eq!(start, "2026-10-14");
        assert_eq!(end, "2026-10-21");
        assert_eq!(data["current"]["totalConversations"], 42);

        let flat = flatten_report(&data);
        let labels: Vec<&str> = flat.iter().map(|(l, _)| l.as_str()).collect();
        assert!(labels.contains(&"current totalConversations"));
        assert!(labels.contains(&"previous totalConversations"));
        assert!(labels.contains(&"delta totalConversations"));

        // A second write overwrites (no stale rows accumulate).
        cache_report(
            &conn,
            "conversations",
            "2026-10-21",
            "2026-10-28",
            &json!({}),
        )
        .expect("cache 2");
        let (_, start2, _, _) = cached_report(&conn, "conversations").expect("row 2");
        assert_eq!(start2, "2026-10-21");
        assert_eq!(cached_report_keys(&conn).len(), 1);
    }
}
