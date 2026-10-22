use crate::business_hours::{
    BusinessHoursConfig, SlaTargets, SlaVerdict, business_minutes_between, default_business_hours,
    is_valid_timezone, now_iso_millis, sla_status, wall_minutes_between,
};
use crate::error::Result;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const AT_RISK_RATIO: f64 = 0.8;

const ALERTS_CAP: usize = 50;

const ALERTS_NOTE: &str = "Alerts measure BUSINESS minutes (nights/weekends excluded) since each conversation's last customer message, against the mailbox's SLA targets. Mailboxes without business hours or targets are listed as unconfigured - nothing is guessed.";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaDurationStats {
    pub count: i64,
    pub avg_wall_min: Option<i64>,
    pub avg_business_min: Option<i64>,
    pub median_business_min: Option<i64>,
    pub met: i64,
    pub missed: i64,
    pub no_target: i64,
    pub target_min: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaWaitingStats {
    pub count: i64,
    pub oldest_business_min: Option<i64>,
    pub avg_business_min: Option<i64>,
    pub at_risk: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaSchedule {
    pub timezone: String,
    pub days: Vec<i64>,
    pub start_minute: i64,
    pub end_minute: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaMailboxRow {
    pub mailbox_id: i64,
    pub mailbox_name: String,
    pub business_hours_configured: bool,
    pub schedule: Option<SlaSchedule>,
    pub conversations_in_range: i64,
    pub first_response: SlaDurationStats,
    pub resolution: SlaDurationStats,
    pub waiting: SlaWaitingStats,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaReport {
    pub range: SlaRange,
    pub mailboxes: Vec<SlaMailboxRow>,
    pub unconfigured_mailboxes: Vec<String>,
    pub source: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaRange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaAlertRow {
    pub conversation_id: i64,
    pub number: i64,
    pub subject: Option<String>,
    pub status: String,
    pub mailbox_id: i64,
    pub mailbox_name: String,
    pub assignee_local_id: Option<i64>,
    pub state: SlaAlertState,
    pub waited_business_min: i64,
    pub target_min: i64,
    pub target_kind: SlaTargetKind,
    pub overdue_business_min: i64,
    pub since: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlaAlertState {
    Breached,
    AtRisk,
}

impl SlaAlertState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Breached => "breached",
            Self::AtRisk => "at_risk",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlaTargetKind {
    FirstResponse,
    Resolution,
}

impl SlaTargetKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FirstResponse => "first_response",
            Self::Resolution => "resolution",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaPerMailbox {
    pub mailbox_id: i64,
    pub mailbox_name: String,
    pub breached: i64,
    pub at_risk: i64,
    pub monitored: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlaAlerts {
    pub generated_at: String,
    pub total_breached: i64,
    pub total_at_risk: i64,
    pub alerts: Vec<SlaAlertRow>,
    pub per_mailbox: Vec<SlaPerMailbox>,
    pub unconfigured_mailboxes: Vec<String>,
    pub note: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct EffectiveConfig {
    business_hours: BusinessHoursConfig,
    targets: SlaTargets,
}

#[derive(Debug, Clone)]
struct Pair {
    start: Option<String>,
    end: Option<String>,
}

pub fn ensure_sla_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mailbox_business_hours (
            mailbox_local_id INTEGER PRIMARY KEY,
            timezone TEXT NOT NULL DEFAULT 'UTC',
            days TEXT NOT NULL DEFAULT '[1,2,3,4,5]',
            start_minute INTEGER NOT NULL DEFAULT 540,
            end_minute INTEGER NOT NULL DEFAULT 1020,
            first_response_target_min INTEGER,
            resolution_target_min INTEGER,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS conversation_threads (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            type            TEXT NOT NULL,
            body_text       TEXT,
            from_type       TEXT,
            created_by_customer_id INTEGER,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );",
    )?;
    ensure_column(
        conn,
        "conversations",
        "deleted_at",
        "ALTER TABLE conversations ADD COLUMN deleted_at TEXT",
    )?;
    ensure_column(
        conn,
        "conversations",
        "snoozed_until",
        "ALTER TABLE conversations ADD COLUMN snoozed_until TEXT",
    )?;
    ensure_column(
        conn,
        "conversation_threads",
        "deleted_at",
        "ALTER TABLE conversation_threads ADD COLUMN deleted_at TEXT",
    )?;
    ensure_column(
        conn,
        "conversation_threads",
        "state",
        "ALTER TABLE conversation_threads ADD COLUMN state TEXT DEFAULT 'published'",
    )?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let existing: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .flatten()
        .collect();
    if !existing.iter().any(|c| c == column) {
        conn.execute_batch(ddl)?;
    }
    Ok(())
}

fn sanitize_ids(ids: Option<&[i64]>) -> Vec<i64> {
    ids.unwrap_or_default()
        .iter()
        .copied()
        .filter(|id| *id > 0)
        .collect()
}

struct StoredBusinessHours {
    timezone: String,
    days: String,
    start_minute: i64,
    end_minute: i64,
    first_response_target_min: Option<i64>,
    resolution_target_min: Option<i64>,
}

fn effective_config(conn: &Connection, mailbox_local_id: i64) -> Result<Option<EffectiveConfig>> {
    let stored: Option<StoredBusinessHours> = conn
        .query_row(
            "SELECT timezone, days, start_minute, end_minute, first_response_target_min, resolution_target_min
             FROM mailbox_business_hours WHERE mailbox_local_id = ?1",
            [mailbox_local_id],
            |r| {
                Ok(StoredBusinessHours {
                    timezone: r.get(0)?,
                    days: r.get(1)?,
                    start_minute: r.get(2)?,
                    end_minute: r.get(3)?,
                    first_response_target_min: r.get(4)?,
                    resolution_target_min: r.get(5)?,
                })
            },
        )
        .optional()?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let mut days = default_business_hours().days;
    if let Ok(parsed) = serde_json::from_str::<Vec<i64>>(&stored.days)
        && !parsed.is_empty()
    {
        days = parsed;
    }
    Ok(Some(EffectiveConfig {
        business_hours: BusinessHoursConfig {
            timezone: if is_valid_timezone(&stored.timezone) {
                stored.timezone
            } else {
                "UTC".to_string()
            },
            days,
            start_minute: stored.start_minute,
            end_minute: stored.end_minute,
        },
        targets: SlaTargets {
            first_response_target_min: stored.first_response_target_min,
            resolution_target_min: stored.resolution_target_min,
        },
    }))
}

pub fn sla_report(
    conn: &Connection,
    from: &str,
    to: &str,
    mailbox_local_ids: Option<&[i64]>,
) -> Result<SlaReport> {
    ensure_sla_schema(conn)?;
    let id_filter = sanitize_ids(mailbox_local_ids);

    let mut mailboxes: Vec<(i64, String)> = Vec::new();
    {
        let sql = if id_filter.is_empty() {
            "SELECT id, name FROM mailboxes ORDER BY name".to_string()
        } else {
            format!(
                "SELECT id, name FROM mailboxes WHERE id IN ({}) ORDER BY name",
                id_filter
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", i + 1))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let params: Vec<&dyn rusqlite::ToSql> = id_filter
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params.as_slice(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            mailboxes.push(row?);
        }
    }

    let mut rows: Vec<SlaMailboxRow> = Vec::new();
    let mut unconfigured: Vec<String> = Vec::new();
    for (id, name) in mailboxes {
        let cfg = effective_config(conn, id)?;
        if cfg.is_none() {
            unconfigured.push(name.clone());
        }
        let schedule = cfg.as_ref().map(|c| SlaSchedule {
            timezone: c.business_hours.timezone.clone(),
            days: c.business_hours.days.clone(),
            start_minute: c.business_hours.start_minute,
            end_minute: c.business_hours.end_minute,
        });
        rows.push(SlaMailboxRow {
            mailbox_id: id,
            mailbox_name: name,
            business_hours_configured: cfg.is_some(),
            schedule,
            conversations_in_range: count_conversations(conn, id, from, to),
            first_response: duration_stats(
                conn,
                id,
                from,
                to,
                DurationKind::FirstResponse,
                cfg.as_ref(),
            ),
            resolution: duration_stats(conn, id, from, to, DurationKind::Resolution, cfg.as_ref()),
            waiting: waiting_stats(conn, id, cfg.as_ref())?,
        });
    }
    Ok(SlaReport {
        range: SlaRange {
            from: from.to_string(),
            to: to.to_string(),
        },
        mailboxes: rows,
        unconfigured_mailboxes: unconfigured,
        source: vec!["local".to_string()],
    })
}

fn count_conversations(conn: &Connection, mailbox_local_id: i64, from: &str, to: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM conversations
         WHERE mailbox_local_id = ?1 AND deleted_at IS NULL AND created_at >= ?2 AND created_at <= ?3",
        rusqlite::params![mailbox_local_id, from, to],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DurationKind {
    FirstResponse,
    Resolution,
}

fn first_response_pairs(
    conn: &Connection,
    mailbox_local_id: i64,
    from: &str,
    to: &str,
) -> Result<Vec<Pair>> {
    let mut stmt = conn.prepare(
        "SELECT c.created_at AS start, fr.first_reply AS end
         FROM conversations c
         JOIN (SELECT conversation_id, MIN(created_at) AS first_reply
               FROM conversation_threads
               WHERE type = 'reply' AND state = 'published' AND deleted_at IS NULL
               GROUP BY conversation_id) fr
           ON fr.conversation_id = c.id
         WHERE c.mailbox_local_id = ?1 AND c.deleted_at IS NULL AND c.created_at >= ?2 AND c.created_at <= ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![mailbox_local_id, from, to], |r| {
        Ok(Pair {
            start: r.get(0)?,
            end: r.get(1)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

fn resolution_pairs(
    conn: &Connection,
    mailbox_local_id: i64,
    from: &str,
    to: &str,
) -> Result<Vec<Pair>> {
    let mut stmt = conn.prepare(
        "SELECT c.created_at AS start, c.closed_at AS end
         FROM conversations c
         WHERE c.mailbox_local_id = ?1 AND c.deleted_at IS NULL AND c.closed_at IS NOT NULL
           AND c.closed_at >= ?2 AND c.closed_at <= ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![mailbox_local_id, from, to], |r| {
        Ok(Pair {
            start: r.get(0)?,
            end: r.get(1)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

fn js_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

fn duration_stats(
    conn: &Connection,
    mailbox_local_id: i64,
    from: &str,
    to: &str,
    kind: DurationKind,
    cfg: Option<&EffectiveConfig>,
) -> SlaDurationStats {
    let pairs = match kind {
        DurationKind::FirstResponse => {
            first_response_pairs(conn, mailbox_local_id, from, to).unwrap_or_default()
        }
        DurationKind::Resolution => {
            resolution_pairs(conn, mailbox_local_id, from, to).unwrap_or_default()
        }
    };
    let target = match kind {
        DurationKind::FirstResponse => cfg.and_then(|c| c.targets.first_response_target_min),
        DurationKind::Resolution => cfg.and_then(|c| c.targets.resolution_target_min),
    };
    let mut walls: Vec<f64> = Vec::new();
    let mut businesses: Vec<f64> = Vec::new();
    let (mut met, mut missed, mut no_target) = (0i64, 0i64, 0i64);
    for p in &pairs {
        let (Some(start), Some(end)) = (p.start.as_deref(), p.end.as_deref()) else {
            continue;
        };
        let Some(wall) = wall_minutes_between(start, end) else {
            continue;
        };
        walls.push(wall);
        let business = cfg.map(|c| business_minutes_between(start, end, &c.business_hours));
        if let Some(Some(b)) = business {
            businesses.push(b);
        }
        match sla_status(business.flatten(), target) {
            SlaVerdict::Met => met += 1,
            SlaVerdict::Missed => missed += 1,
            _ => no_target += 1,
        }
    }
    SlaDurationStats {
        count: pairs.len() as i64,
        avg_wall_min: avg_rounded(&walls),
        avg_business_min: if businesses.is_empty() {
            None
        } else {
            avg_rounded(&businesses)
        },
        median_business_min: if businesses.is_empty() {
            None
        } else {
            Some(median_rounded(&businesses))
        },
        met,
        missed,
        no_target: if target.is_none() {
            pairs.len() as i64
        } else {
            no_target
        },
        target_min: target,
    }
}

fn avg_rounded(xs: &[f64]) -> Option<i64> {
    if xs.is_empty() {
        return None;
    }
    Some(js_round(xs.iter().sum::<f64>() / xs.len() as f64))
}

fn median_rounded(xs: &[f64]) -> i64 {
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        js_round(sorted[mid])
    } else {
        js_round((sorted[mid - 1] + sorted[mid]) / 2.0)
    }
}

fn waiting_stats(
    conn: &Connection,
    mailbox_local_id: i64,
    cfg: Option<&EffectiveConfig>,
) -> Result<SlaWaitingStats> {
    let mut stmt = conn.prepare(
        // Audit item 13: "waiting" means waiting for a reply to the
        // customer's last message. `updated_at` flips on every status
        // change, tag edit, assignment or sync touch, which silently
        // reset the waiting clock and understated SLA pressure; the
        // honest since-point is the newest published customer thread,
        // falling back to the conversation's own creation time when the
        // customer has not spoken yet.
        "SELECT COALESCE(
             (SELECT MAX(COALESCE(t.remote_created_at, t.created_at))
                FROM conversation_threads t
               WHERE t.conversation_id = c.id
                 AND t.type = 'customer'
                 AND t.state = 'published'
                 AND t.deleted_at IS NULL),
             c.created_at) AS since
         FROM conversations c
         WHERE c.mailbox_local_id = ?1 AND c.status IN ('active','pending') AND c.deleted_at IS NULL",
    )?;
    let rows = stmt.query_map([mailbox_local_id], |r| r.get::<_, Option<String>>(0))?;
    let mut sinces: Vec<Option<String>> = Vec::new();
    for row in rows {
        sinces.push(row?);
    }
    let now = now_iso_millis();
    let mut businesses: Vec<f64> = Vec::new();
    for since in sinces.iter().flatten() {
        let b = match cfg {
            Some(c) => business_minutes_between(since, &now, &c.business_hours),
            None => wall_minutes_between(since, &now),
        };
        if let Some(b) = b {
            businesses.push(b);
        }
    }
    let target = cfg.and_then(|c| c.targets.first_response_target_min);
    let at_risk = match target {
        Some(t) => businesses.iter().filter(|b| **b > t as f64).count() as i64,
        None => 0,
    };
    Ok(SlaWaitingStats {
        count: sinces.len() as i64,
        oldest_business_min: businesses
            .iter()
            .copied()
            .fold(None, |a: Option<f64>, b| {
                Some(match a {
                    Some(x) => x.max(b),
                    None => b,
                })
            })
            .map(js_round),
        avg_business_min: avg_rounded(&businesses),
        at_risk,
    })
}

#[derive(Debug, Clone)]
struct AlertCandidate {
    id: i64,
    number: i64,
    subject: Option<String>,
    status: String,
    assignee_local_id: Option<i64>,
    since: Option<String>,
    replied: bool,
    customer_threads: i64,
}

pub fn sla_alerts(conn: &Connection) -> Result<SlaAlerts> {
    ensure_sla_schema(conn)?;
    let now = now_iso_millis();

    let mut mailboxes: Vec<(i64, String)> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, name FROM mailboxes ORDER BY name")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            mailboxes.push(row?);
        }
    }

    let mut alerts: Vec<SlaAlertRow> = Vec::new();
    let mut per_mailbox: Vec<SlaPerMailbox> = Vec::new();
    let mut unconfigured: Vec<String> = Vec::new();

    for (id, name) in mailboxes {
        let Some(cfg) = effective_config(conn, id)? else {
            let open: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM conversations
                     WHERE mailbox_local_id = ?1 AND status IN ('active','pending') AND deleted_at IS NULL",
                    [id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if open > 0 {
                unconfigured.push(name.clone());
            }
            per_mailbox.push(SlaPerMailbox {
                mailbox_id: id,
                mailbox_name: name,
                breached: 0,
                at_risk: 0,
                monitored: 0,
            });
            continue;
        };

        let candidates: Vec<AlertCandidate> = {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.number, c.subject, c.status, c.assignee_local_id,
                        COALESCE(c.updated_at, c.created_at) AS since,
                        EXISTS (SELECT 1 FROM conversation_threads t
                                WHERE t.conversation_id = c.id AND t.type = 'reply'
                                  AND t.state = 'published' AND t.deleted_at IS NULL) AS replied,
                        (SELECT COUNT(*) FROM conversation_threads t
                         WHERE t.conversation_id = c.id AND t.type = 'customer'
                           AND t.deleted_at IS NULL) AS customer_threads
                 FROM conversations c
                 WHERE c.mailbox_local_id = ?1 AND c.status IN ('active','pending') AND c.deleted_at IS NULL
                   AND NOT (c.snoozed_until IS NOT NULL AND c.snoozed_until > ?2)",
            )?;
            let rows = stmt.query_map(rusqlite::params![id, now], |r| {
                Ok(AlertCandidate {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    subject: r.get(2)?,
                    status: r.get(3)?,
                    assignee_local_id: r.get(4)?,
                    since: r.get(5)?,
                    replied: r.get::<_, i64>(6)? != 0,
                    customer_threads: r.get(7)?,
                })
            })?;
            rows.flatten().collect()
        };

        let mut breached = 0i64;
        let mut at_risk = 0i64;
        for r in &candidates {
            if r.since.is_none() || r.customer_threads == 0 {
                continue;
            }
            let last_customer: Option<String> = conn
                .query_row(
                    "SELECT created_at FROM conversation_threads
                     WHERE conversation_id = ?1 AND type = 'customer' AND deleted_at IS NULL
                     ORDER BY created_at DESC LIMIT 1",
                    [r.id],
                    |row| row.get(0),
                )
                .optional()?;
            let last_customer =
                last_customer.unwrap_or_else(|| r.since.clone().unwrap_or_default());
            let target = if r.replied {
                cfg.targets.resolution_target_min
            } else {
                cfg.targets.first_response_target_min
            };
            let Some(target) = target else {
                continue;
            };
            let Some(waited) = business_minutes_between(&last_customer, &now, &cfg.business_hours)
            else {
                continue;
            };
            let state = if waited > target as f64 {
                SlaAlertState::Breached
            } else if waited >= target as f64 * AT_RISK_RATIO {
                SlaAlertState::AtRisk
            } else {
                continue;
            };
            match state {
                SlaAlertState::Breached => breached += 1,
                SlaAlertState::AtRisk => at_risk += 1,
            }
            alerts.push(SlaAlertRow {
                conversation_id: r.id,
                number: r.number,
                subject: r.subject.clone(),
                status: r.status.clone(),
                mailbox_id: id,
                mailbox_name: name.clone(),
                assignee_local_id: r.assignee_local_id,
                state,
                waited_business_min: js_round(waited),
                target_min: target,
                target_kind: if r.replied {
                    SlaTargetKind::Resolution
                } else {
                    SlaTargetKind::FirstResponse
                },
                overdue_business_min: js_round((waited - target as f64).max(0.0)),
                since: last_customer,
            });
        }
        per_mailbox.push(SlaPerMailbox {
            mailbox_id: id,
            mailbox_name: name,
            breached,
            at_risk,
            monitored: candidates.len() as i64,
        });
    }

    alerts.sort_by(|a, b| {
        b.overdue_business_min
            .cmp(&a.overdue_business_min)
            .then_with(|| a.number.cmp(&b.number))
    });
    let total_breached = alerts
        .iter()
        .filter(|a| a.state == SlaAlertState::Breached)
        .count() as i64;
    let total_at_risk = alerts
        .iter()
        .filter(|a| a.state == SlaAlertState::AtRisk)
        .count() as i64;
    alerts.truncate(ALERTS_CAP);
    Ok(SlaAlerts {
        generated_at: now,
        total_breached,
        total_at_risk,
        alerts,
        per_mailbox,
        unconfigured_mailboxes: unconfigured,
        note: ALERTS_NOTE,
    })
}

/// Clean-audit batch 18 (item 13): waiting statistics measure from the
/// customer's last message, not from any `updated_at` touch.
#[cfg(test)]
mod batch18_tests {
    use super::*;
    use rusqlite::params;

    const OLD: &str = "2026-01-01T00:00:00.000Z";

    /// Seeds one mailbox with three waiting conversations:
    /// - A: customer message at OLD, updated_at OLD
    /// - B: customer message at OLD, but updated_at is RECENT (a status
    ///   or sync touch — what the old query wrongly treated as the
    ///   waiting start)
    /// - C: no customer threads at all, created_at OLD (fallback path)
    fn db_with_waiting_conversations() -> (Connection, i64, String) {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO mailboxes (remote_id, name) VALUES (501, 'Support')",
            [],
        )
        .expect("mailbox");
        let mailbox = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name, email)
             VALUES (701, 'Pat', 'Waiter', 'pat@example.com')",
            [],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();
        let recent = now_iso_millis();
        for (remote, subject, updated_at) in [
            (601_i64, "A plain wait", Some(OLD.to_string())),
            (602_i64, "B touched recently", Some(recent.clone())),
            (603_i64, "C never answered", None),
        ] {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                            mailbox_local_id, customer_local_id,
                                            created_at, updated_at)
                 VALUES (?1, ?1, ?2, 'p', 'active', ?3, ?6, ?4, ?5)",
                params![remote, subject, mailbox, OLD, updated_at, customer],
            )
            .expect("conversation");
            let conv = conn.last_insert_rowid();
            if subject != "C never answered" {
                conn.execute(
                    "INSERT INTO conversation_threads (conversation_id, type, state, body_text,
                                                       remote_created_at)
                     VALUES (?1, 'customer', 'published', 'still waiting for an answer', ?2)",
                    params![conv, OLD],
                )
                .expect("customer thread");
            }
        }
        (conn, mailbox, recent)
    }

    #[test]
    fn waiting_stats_measure_from_the_last_customer_message() {
        let (conn, mailbox, recent) = db_with_waiting_conversations();
        let now = now_iso_millis();
        let expected = wall_minutes_between(OLD, &now).expect("expected wall minutes");

        let stats = waiting_stats(&conn, mailbox, None).expect("waiting stats");
        assert_eq!(stats.count, 3, "all three conversations are waiting");
        let avg = stats.avg_business_min.expect("avg") as f64;
        assert!(
            (avg - expected).abs() <= 2.0,
            "waiting must run from the last customer message ({expected} min), not from a \
             recent updated_at touch (got {avg}; the recent touch was {recent})"
        );
        let oldest = stats.oldest_business_min.expect("oldest") as f64;
        assert!(
            (oldest - expected).abs() <= 2.0,
            "oldest wait runs from the customer's message: {oldest} vs {expected}"
        );
    }

    #[test]
    fn waiting_stats_let_recently_touched_conversations_show_their_real_wait() {
        // Conversation B alone: its updated_at is RECENT, its customer
        // message is OLD. Under the old query this conversation looked
        // fresh-again after every status/tag/sync touch; it must not.
        let (conn, mailbox, recent) = db_with_waiting_conversations();
        let now = now_iso_millis();
        let expected = wall_minutes_between(OLD, &now).expect("expected wall minutes");
        let touched_recently =
            wall_minutes_between(&recent, &now).expect("minutes since the touch");

        let stats = waiting_stats(&conn, mailbox, None).expect("waiting stats");
        let avg = stats.avg_business_min.expect("avg") as f64;
        assert!(
            expected - touched_recently > 60.0 * 24.0,
            "the fixture must be old enough to discriminate: {expected} vs {touched_recently}"
        );
        assert!(
            (avg - expected).abs() <= 2.0,
            "a touched conversation keeps its real wait ({expected} min), not the \
             since-last-touch age ({touched_recently}): got {avg}"
        );
    }
}
