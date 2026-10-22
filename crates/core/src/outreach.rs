use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::Result;

pub const M023_TO_M025_SQL: &str = r#"
    -- M023: saved_segments
    CREATE TABLE IF NOT EXISTS saved_segments (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        name            TEXT NOT NULL,
        criteria_json   TEXT NOT NULL,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    -- M024: campaigns + campaign_recipients
    CREATE TABLE IF NOT EXISTS campaigns (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        name            TEXT NOT NULL,
        segment_id      INTEGER REFERENCES saved_segments (id),
        status          TEXT NOT NULL DEFAULT 'draft',
        message_template TEXT,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        started_at      TEXT,
        completed_at    TEXT
    );
    CREATE TABLE IF NOT EXISTS campaign_recipients (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        campaign_id     INTEGER NOT NULL REFERENCES campaigns (id) ON DELETE CASCADE,
        customer_id     INTEGER NOT NULL,
        status          TEXT NOT NULL DEFAULT 'pending',
        attempts        INTEGER NOT NULL DEFAULT 0,
        max_attempts    INTEGER NOT NULL DEFAULT 3,
        last_error      TEXT,
        sent_at         TEXT,
        replied_at      TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_campaign_recipients_campaign
        ON campaign_recipients (campaign_id, status);

    -- M025: do_not_contact
    CREATE TABLE IF NOT EXISTS do_not_contact (
        customer_id     INTEGER PRIMARY KEY,
        reason          TEXT NOT NULL DEFAULT 'manual',
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    UPDATE app_state SET schema_version = 25 WHERE id = 1;
"#;

pub fn apply_m023_to_m025(conn: &Connection) -> Result<()> {
    conn.execute_batch(M023_TO_M025_SQL)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedSegment {
    pub id: Option<i64>,
    pub name: String,
    pub criteria: String,
    pub created_at: String,
}

pub fn create_segment(conn: &Connection, name: &str, criteria: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO saved_segments (name, criteria_json) VALUES (?1, ?2)",
        params![name, criteria],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_segments(conn: &Connection) -> Result<Vec<SavedSegment>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, criteria_json, created_at FROM saved_segments ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SavedSegment {
                id: r.get(0)?,
                name: r.get(1)?,
                criteria: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_segment(conn: &Connection, id: i64) -> Result<Option<SavedSegment>> {
    let row = conn
        .query_row(
            "SELECT id, name, criteria_json, created_at FROM saved_segments WHERE id = ?1",
            params![id],
            |r| {
                Ok(SavedSegment {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    criteria: r.get(2)?,
                    created_at: r.get(3)?,
                })
            },
        )
        .ok();
    Ok(row)
}

pub fn delete_segment(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM saved_segments WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

pub const DEFAULT_MAX_CAMPAIGN_ATTEMPTS: i64 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub id: Option<i64>,
    pub name: String,
    pub segment_id: Option<i64>,
    pub status: String,
    pub message_template: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignRecipient {
    pub id: Option<i64>,
    pub campaign_id: i64,
    pub customer_id: i64,
    pub status: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub last_error: Option<String>,
    pub sent_at: Option<String>,
    pub replied_at: Option<String>,
}

pub fn create_campaign(
    conn: &Connection,
    name: &str,
    segment_id: Option<i64>,
    message_template: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO campaigns (name, segment_id, message_template) VALUES (?1, ?2, ?3)",
        params![name, segment_id, message_template],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn add_campaign_recipient(
    conn: &Connection,
    campaign_id: i64,
    customer_id: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO campaign_recipients (campaign_id, customer_id) VALUES (?1, ?2)",
        params![campaign_id, customer_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn start_campaign(conn: &Connection, campaign_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE campaigns SET status = 'sending', started_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1 AND status = 'draft'",
        params![campaign_id],
    )?;
    Ok(rows > 0)
}

pub fn mark_recipient_sent(conn: &Connection, recipient_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE campaign_recipients SET status = 'sent', sent_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1 AND status = 'pending'",
        params![recipient_id],
    )?;
    Ok(rows > 0)
}

pub fn mark_recipient_failed(conn: &Connection, recipient_id: i64, error: &str) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE campaign_recipients SET status = 'failed', last_error = ?1
         WHERE id = ?2",
        params![error, recipient_id],
    )?;
    Ok(rows > 0)
}

pub fn increment_attempt(conn: &Connection, recipient_id: i64) -> Result<i64> {
    conn.execute(
        "UPDATE campaign_recipients SET attempts = attempts + 1 WHERE id = ?1",
        params![recipient_id],
    )?;
    let attempts: i64 = conn.query_row(
        "SELECT attempts FROM campaign_recipients WHERE id = ?1",
        params![recipient_id],
        |r| r.get(0),
    )?;
    Ok(attempts)
}

pub fn recipient_exhausted(conn: &Connection, recipient_id: i64) -> Result<bool> {
    let (attempts, max): (i64, i64) = conn.query_row(
        "SELECT attempts, max_attempts FROM campaign_recipients WHERE id = ?1",
        params![recipient_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(attempts >= max)
}

pub fn retry_failed_recipients(conn: &Connection, campaign_id: i64) -> Result<u32> {
    let rows = conn.execute(
        "UPDATE campaign_recipients
         SET status = 'pending', attempts = 0, last_error = NULL
         WHERE campaign_id = ?1 AND status = 'failed'",
        params![campaign_id],
    )?;
    Ok(u32::try_from(rows).unwrap_or(0))
}

pub fn count_active_campaigns(conn: &Connection) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM campaigns WHERE status = 'sending'",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn list_campaigns(conn: &Connection) -> Result<Vec<Campaign>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, segment_id, status, message_template, created_at, started_at, completed_at
         FROM campaigns ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Campaign {
            id: r.get(0)?,
            name: r.get(1)?,
            segment_id: r.get(2)?,
            status: r.get(3)?,
            message_template: r.get(4)?,
            created_at: r.get(5)?,
            started_at: r.get(6)?,
            completed_at: r.get(7)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

pub fn add_to_dnc(conn: &Connection, customer_id: i64, reason: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO do_not_contact (customer_id, reason) VALUES (?1, ?2)",
        params![customer_id, reason],
    )?;
    Ok(())
}

pub fn remove_from_dnc(conn: &Connection, customer_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM do_not_contact WHERE customer_id = ?1",
        params![customer_id],
    )?;
    Ok(rows > 0)
}

pub fn is_on_dnc(conn: &Connection, customer_id: i64) -> Result<bool> {
    let exists: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM do_not_contact WHERE customer_id = ?1)",
        params![customer_id],
        |r| r.get(0),
    )?;
    Ok(exists != 0)
}

pub fn list_dnc(conn: &Connection) -> Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT customer_id, reason, created_at FROM do_not_contact ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignStats {
    pub total_recipients: u32,
    pub sent: u32,
    pub failed: u32,
    pub pending: u32,
    pub replied: u32,
    pub reply_rate: f64,
}

pub fn get_campaign_stats(conn: &Connection, campaign_id: i64) -> Result<CampaignStats> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM campaign_recipients WHERE campaign_id = ?1",
        params![campaign_id],
        |r| r.get(0),
    )?;
    let sent: i64 = conn.query_row(
        "SELECT COUNT(*) FROM campaign_recipients WHERE campaign_id = ?1 AND status = 'sent'",
        params![campaign_id],
        |r| r.get(0),
    )?;
    let failed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM campaign_recipients WHERE campaign_id = ?1 AND status = 'failed'",
        params![campaign_id],
        |r| r.get(0),
    )?;
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM campaign_recipients WHERE campaign_id = ?1 AND status = 'pending'",
        params![campaign_id],
        |r| r.get(0),
    )?;
    let replied: i64 = conn.query_row(
        "SELECT COUNT(*) FROM campaign_recipients WHERE campaign_id = ?1 AND replied_at IS NOT NULL",
        params![campaign_id],
        |r| r.get(0),
    )?;
    let total_u = u32::try_from(total).unwrap_or(0);
    let sent_u = u32::try_from(sent).unwrap_or(0);
    let replied_u = u32::try_from(replied).unwrap_or(0);
    let reply_rate = if sent_u > 0 {
        replied_u as f64 / sent_u as f64
    } else {
        0.0
    };
    Ok(CampaignStats {
        total_recipients: total_u,
        sent: sent_u,
        failed: u32::try_from(failed).unwrap_or(0),
        pending: u32::try_from(pending).unwrap_or(0),
        replied: replied_u,
        reply_rate,
    })
}

pub fn mark_recipient_replied(conn: &Connection, recipient_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE campaign_recipients SET replied_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1 AND replied_at IS NULL",
        params![recipient_id],
    )?;
    Ok(rows > 0)
}

pub const PERSONALIZATION_VARIABLES: [&str; 6] = [
    "first_name",
    "last_name",
    "company",
    "organization",
    "last_ticket_number",
    "last_ticket_subject",
];

const MAX_ATTEMPTS: i64 = 3;

pub fn apply_m031(conn: &Connection) -> Result<()> {
    let needs_dnc_rebuild: bool = {
        let old_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name='do_not_contact'
                   AND sql LIKE '%NOT NULL DEFAULT ''manual''%'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        old_exists
    };

    let dnc_rebuild_sql = if needs_dnc_rebuild {
        "CREATE TABLE IF NOT EXISTS do_not_contact_m031 (
            customer_id     INTEGER PRIMARY KEY,
            reason          TEXT,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        INSERT OR IGNORE INTO do_not_contact_m031 (customer_id, reason, created_at)
            SELECT customer_id, CASE WHEN reason = 'manual' THEN NULL ELSE reason END, created_at
              FROM do_not_contact;
        DROP TABLE IF EXISTS do_not_contact;
        ALTER TABLE do_not_contact_m031 RENAME TO do_not_contact;"
    } else {
        "CREATE TABLE IF NOT EXISTS do_not_contact (
            customer_id     INTEGER PRIMARY KEY,
            reason          TEXT,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );"
    };

    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS outreach_campaigns (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            subject         TEXT NOT NULL,
            body            TEXT NOT NULL,
            mailbox_local_id INTEGER,
            tags            TEXT NOT NULL DEFAULT '[]',
            status          TEXT NOT NULL DEFAULT 'draft',
            segment_id      INTEGER,
            segment_snapshot TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            queued_at       TEXT,
            completed_at    TEXT,
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS outreach_recipients (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            campaign_id     INTEGER NOT NULL,
            customer_local_id INTEGER NOT NULL,
            customer_remote_id INTEGER,
            email           TEXT,
            snapshot        TEXT NOT NULL DEFAULT '{{}}',
            state           TEXT NOT NULL DEFAULT 'selected',
            attempts        INTEGER NOT NULL DEFAULT 0,
            last_error      TEXT,
            hs_conversation_remote_id INTEGER,
            hs_conversation_number INTEGER,
            sent_at         TEXT,
            replied_at      TEXT,
            updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            UNIQUE (campaign_id, customer_local_id)
        );
        CREATE INDEX IF NOT EXISTS idx_outreach_recipients_campaign
            ON outreach_recipients(campaign_id, state);
        CREATE TABLE IF NOT EXISTS outreach_attempts (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            recipient_id    INTEGER NOT NULL,
            attempt_no      INTEGER NOT NULL,
            started_at      TEXT NOT NULL,
            finished_at     TEXT,
            result          TEXT,
            error           TEXT
        );
        CREATE TABLE IF NOT EXISTS outreach_events (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            campaign_id     INTEGER NOT NULL,
            recipient_id    INTEGER,
            event           TEXT NOT NULL,
            detail          TEXT,
            at              TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_outreach_events_campaign
            ON outreach_events(campaign_id);
        {dnc_rebuild_sql}",
    ))?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 31 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct CampaignDetail {
    pub id: i64,
    pub name: String,
    pub subject: String,
    pub body: String,
    pub mailbox_local_id: Option<i64>,
    pub mailbox_name: Option<String>,
    pub tags: Vec<String>,
    pub status: String,
    pub segment_id: Option<i64>,
    pub segment_name: Option<String>,
    pub recipients: i64,
    pub sent: i64,
    pub failed: i64,
    pub skipped: i64,
    pub unknown: i64,
    pub replied: i64,
    pub created_at: String,
    pub queued_at: Option<String>,
    pub completed_at: Option<String>,
}

fn parse_tags(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

fn map_campaign_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CampaignDetail> {
    Ok(CampaignDetail {
        id: r.get(0)?,
        name: r.get(1)?,
        subject: r.get(2)?,
        body: r.get(3)?,
        mailbox_local_id: r.get(4)?,
        tags: parse_tags(&r.get::<_, String>(5).unwrap_or_default()),
        status: r.get(6)?,
        segment_id: r.get(7)?,
        created_at: r.get(8)?,
        queued_at: r.get(9)?,
        completed_at: r.get(10)?,
        mailbox_name: r.get(11)?,
        segment_name: r.get(12)?,
        recipients: r.get(13)?,
        sent: r.get(14)?,
        failed: r.get(15)?,
        skipped: r.get(16)?,
        unknown: r.get(17)?,
        replied: r.get(18)?,
    })
}

const CAMPAIGN_SELECT: &str = "SELECT oc.id, oc.name, oc.subject, oc.body, oc.mailbox_local_id, oc.tags,
        oc.status, oc.segment_id, oc.created_at, oc.queued_at, oc.completed_at,
        m.name, s.name,
        (SELECT COUNT(*) FROM outreach_recipients r WHERE r.campaign_id = oc.id),
        (SELECT COUNT(*) FROM outreach_recipients r WHERE r.campaign_id = oc.id AND r.state = 'sent'),
        (SELECT COUNT(*) FROM outreach_recipients r WHERE r.campaign_id = oc.id AND r.state = 'failed'),
        (SELECT COUNT(*) FROM outreach_recipients r WHERE r.campaign_id = oc.id AND r.state = 'skipped'),
        (SELECT COUNT(*) FROM outreach_recipients r WHERE r.campaign_id = oc.id AND r.state = 'unknown'),
        (SELECT COUNT(*) FROM outreach_recipients r WHERE r.campaign_id = oc.id AND r.replied_at IS NOT NULL)
   FROM outreach_campaigns oc
   LEFT JOIN mailboxes m ON m.id = oc.mailbox_local_id
   LEFT JOIN saved_segments s ON s.id = oc.segment_id";

pub fn get_outreach_campaign(conn: &Connection, id: i64) -> Result<Option<CampaignDetail>> {
    let mut stmt = conn.prepare(&format!("{CAMPAIGN_SELECT} WHERE oc.id = ?1"))?;
    let mut rows = stmt.query_map([id], map_campaign_row)?;
    Ok(rows.next().transpose()?)
}

pub fn list_campaign_events(conn: &Connection, campaign_id: i64) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, campaign_id, recipient_id, event, detail, at
           FROM outreach_events WHERE campaign_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([campaign_id], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "campaign_id": r.get::<_, i64>(1)?,
            "recipient_id": r.get::<_, Option<i64>>(2)?,
            "event": r.get::<_, String>(3)?,
            "detail": r.get::<_, Option<String>>(4)?,
            "at": r.get::<_, String>(5)?,
        }))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

fn log_event(
    conn: &Connection,
    campaign_id: i64,
    recipient_id: Option<i64>,
    event: &str,
    detail: Option<&str>,
) {
    let _ = conn.execute(
        "INSERT INTO outreach_events (campaign_id, recipient_id, event, detail)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![campaign_id, recipient_id, event, detail],
    );
}

fn update_status(conn: &Connection, id: i64, status: &str) {
    let _ = conn.execute(
        "UPDATE outreach_campaigns SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![status, id],
    );
}

fn count_remaining(conn: &Connection, campaign_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM outreach_recipients
          WHERE campaign_id = ?1 AND state IN ('selected','queued','sending')",
        [campaign_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn personalization_context(
    conn: &Connection,
    customer_local_id: i64,
    matching_tickets: &[serde_json::Value],
) -> serde_json::Value {
    let (first, last, org): (Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT c.first_name, c.last_name, o.name
               FROM customers c LEFT JOIN organizations o ON o.id = c.organization_id
              WHERE c.id = ?1",
            [customer_local_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((None, None, None));
    let last_ticket = matching_tickets.first();
    serde_json::json!({
        "first_name": first,
        "last_name": last,
        "company": org,
        "organization": org,
        "last_ticket_number": last_ticket.and_then(|t| t.get("number")).cloned().unwrap_or(serde_json::Value::Null),
        "last_ticket_subject": last_ticket.and_then(|t| t.get("subject")).cloned().unwrap_or(serde_json::Value::Null),
    })
}

pub fn render_template(text: &str, ctx: &serde_json::Value) -> (String, Vec<String>) {
    let mut unresolved: Vec<String> = Vec::new();
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'{'
            && let Some(close) = text[i + 2..].find("}}")
        {
            let inner = &text[i + 2..i + 2 + close];
            let raw_name = inner.trim();
            let name = raw_name.to_lowercase();
            if !raw_name.is_empty()
                && raw_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                if PERSONALIZATION_VARIABLES.contains(&name.as_str()) {
                    let v = &ctx[&name];
                    match v {
                        serde_json::Value::Null => {
                            unresolved.push(name.clone());
                        }
                        serde_json::Value::String(s) if s.is_empty() => {
                            unresolved.push(name.clone());
                        }
                        serde_json::Value::String(s) => out.push_str(s),
                        other => out.push_str(&other.to_string()),
                    }
                } else {
                    unresolved.push(name);
                    out.push_str(&format!("{{{{{raw_name}}}}}"));
                }
                i = i + 2 + close + 2;
                continue;
            }
        }
        let ch_len = text[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&text[i..i + ch_len]);
        i += ch_len;
    }
    (out, unresolved)
}

pub fn render_for(
    conn: &Connection,
    customer_local_id: i64,
    matching_tickets: &[serde_json::Value],
    subject: &str,
    body: &str,
) -> serde_json::Value {
    let ctx = personalization_context(conn, customer_local_id, matching_tickets);
    let (s, mut unresolved_s) = render_template(subject, &ctx);
    let (b, unresolved_b) = render_template(body, &ctx);
    unresolved_s.extend(unresolved_b);
    serde_json::json!({ "subject": s, "body": b, "unresolved": unresolved_s })
}

pub fn campaign_validate(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let Some(c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return serde_json::json!({
            "ok": false,
            "errors": ["Campaign not found."],
            "warnings": [],
            "counts": { "recipients": 0, "ready": 0, "invalid_email": 0,
                        "already_sent": 0, "on_dnc": 0, "no_email": 0 }
        });
    };
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    if c.subject.trim().is_empty() {
        errors.push("Subject is empty.".into());
    }
    if c.body.trim().is_empty() {
        errors.push("Message body is empty.".into());
    }
    match c.mailbox_local_id {
        None => errors.push("No sending mailbox selected.".into()),
        Some(mid) => {
            let mailbox: Option<i64> = conn
                .query_row("SELECT id FROM mailboxes WHERE id = ?1", [mid], |r| {
                    r.get(0)
                })
                .ok();
            if mailbox.is_none() {
                errors.push("The selected mailbox no longer exists.".into());
            }
        }
    }
    let dnc: std::collections::HashSet<i64> = conn
        .prepare("SELECT customer_id FROM do_not_contact")
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .map(|rows| rows.filter_map(|x| x.ok()).collect())
                .ok()
        })
        .unwrap_or_default();
    let email_ok = |e: &str| {
        let parts: Vec<&str> = e.split('@').collect();
        parts.len() == 2
            && !parts[0].trim().is_empty()
            && parts[0] == parts[0].trim()
            && parts[1].split('.').count() >= 2
            && parts[1].split('.').all(|p| !p.trim().is_empty())
            && parts[1]
                .rsplit('.')
                .next()
                .is_some_and(|tld| tld.len() >= 2)
    };
    let mut ready = 0i64;
    let mut invalid_email = 0i64;
    let mut already_sent = 0i64;
    let mut on_dnc = 0i64;
    let mut no_email = 0i64;
    let rows: Vec<(i64, Option<String>, String)> = conn
        .prepare("SELECT customer_local_id, email, state FROM outreach_recipients WHERE campaign_id = ?1")
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([campaign_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map(|rows| rows.filter_map(|x| x.ok()).collect())
                .ok()
        })
        .unwrap_or_default();
    for (cid, email, state) in &rows {
        if state == "sent" {
            already_sent += 1;
        }
        if state == "sent" || state == "cancelled" {
            continue;
        }
        if dnc.contains(cid) {
            on_dnc += 1;
            continue;
        }
        match email.as_deref() {
            None => no_email += 1,
            Some(e) if !email_ok(e) => invalid_email += 1,
            Some(_) => ready += 1,
        }
    }
    if already_sent > 0 {
        warnings.push(format!("{already_sent} recipient(s) were already sent this campaign and will not be sent again."));
    }
    if on_dnc > 0 {
        warnings.push(format!(
            "{on_dnc} recipient(s) are on the Do-Not-Contact list and will be skipped."
        ));
    }
    if no_email > 0 {
        warnings.push(format!(
            "{no_email} recipient(s) have no usable email address and will be skipped."
        ));
    }
    if invalid_email > 0 {
        errors.push(format!(
            "{invalid_email} recipient(s) have an invalid email address."
        ));
    }
    if ready == 0 {
        errors.push("No recipients are ready to send.".into());
    }
    let sample: Vec<(i64, String)> = conn
        .prepare(
            "SELECT customer_local_id, snapshot FROM outreach_recipients
              WHERE campaign_id = ?1 ORDER BY id LIMIT 25",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([campaign_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|x| x.ok()).collect())
                .ok()
        })
        .unwrap_or_default();
    let mut unresolved_vars: Vec<String> = Vec::new();
    for (cid, snapshot) in &sample {
        let tickets: Vec<serde_json::Value> = serde_json::from_str::<serde_json::Value>(snapshot)
            .ok()
            .and_then(|v| v.get("matching_tickets").cloned())
            .and_then(|t| serde_json::from_value(t).ok())
            .unwrap_or_default();
        let rendered = render_for(conn, *cid, &tickets, &c.subject, &c.body);
        for v in rendered["unresolved"].as_array().unwrap_or(&Vec::new()) {
            if let Some(name) = v.as_str()
                && !unresolved_vars.contains(&name.to_string())
            {
                unresolved_vars.push(name.to_string());
            }
        }
    }
    if !unresolved_vars.is_empty() {
        warnings.push(format!(
            "Personalization variable(s) with no value for some recipients: {} (they render as empty text).",
            unresolved_vars.join(", ")
        ));
    }
    serde_json::json!({
        "ok": errors.is_empty(),
        "errors": errors,
        "warnings": warnings,
        "counts": {
            "recipients": c.recipients, "ready": ready, "invalid_email": invalid_email,
            "already_sent": already_sent, "on_dnc": on_dnc, "no_email": no_email
        }
    })
}

pub fn campaign_queue(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let Some(c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return serde_json::json!({ "ok": false, "message": "Campaign not found." });
    };
    if c.status != "draft" && c.status != "paused" {
        return serde_json::json!({ "ok": false, "message": format!("Campaign is {}; only draft or paused campaigns can be queued.", c.status) });
    }
    let validation = campaign_validate(conn, campaign_id);
    if !validation["ok"].as_bool().unwrap_or(false) {
        let errors: Vec<String> = validation["errors"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        return serde_json::json!({ "ok": false, "message": format!("Validation failed: {}", errors.join(" ")) });
    }
    let ready = validation["counts"]["ready"].as_i64().unwrap_or(0);
    let recipients = validation["counts"]["recipients"].as_i64().unwrap_or(0);
    update_status(conn, campaign_id, "queued");
    let _ = conn.execute(
        "UPDATE outreach_campaigns SET queued_at = datetime('now') WHERE id = ?1",
        [campaign_id],
    );
    log_event(
        conn,
        campaign_id,
        None,
        "campaign_queued",
        Some(&format!("{ready} ready of {recipients}")),
    );
    let _ = crate::jobs::enqueue_on(
        conn,
        "outreach",
        "outreach_send_batch",
        &serde_json::json!({ "campaignId": campaign_id }).to_string(),
        1,
        3,
    );
    serde_json::json!({ "ok": true, "message": format!("Campaign queued: {ready} conversations will be created (one per customer).") })
}

pub fn campaign_pause(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let Some(c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return serde_json::json!({ "ok": false, "message": "Campaign not found." });
    };
    if c.status != "queued" && c.status != "sending" {
        return serde_json::json!({ "ok": false, "message": format!("Only queued or sending campaigns can be paused (current: {}).", c.status) });
    }
    update_status(conn, campaign_id, "paused");
    log_event(conn, campaign_id, None, "campaign_paused", None);
    serde_json::json!({ "ok": true, "message": "Campaign paused. Already-sent recipients are untouched." })
}

pub fn campaign_resume(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let Some(c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return serde_json::json!({ "ok": false, "message": "Campaign not found." });
    };
    if c.status != "paused" {
        return serde_json::json!({ "ok": false, "message": format!("Only paused campaigns can be resumed (current: {}).", c.status) });
    }
    update_status(conn, campaign_id, "queued");
    log_event(conn, campaign_id, None, "campaign_resumed", None);
    let _ = crate::jobs::enqueue_on(
        conn,
        "outreach",
        "outreach_send_batch",
        &serde_json::json!({ "campaignId": campaign_id }).to_string(),
        1,
        3,
    );
    serde_json::json!({ "ok": true, "message": "Campaign resumed." })
}

pub fn campaign_cancel_remaining(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let Some(_c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return serde_json::json!({ "ok": false, "message": "Campaign not found." });
    };
    let cancelled = conn
        .execute(
            "UPDATE outreach_recipients SET state = 'cancelled'
              WHERE campaign_id = ?1 AND state IN ('selected','queued','sending')",
            [campaign_id],
        )
        .unwrap_or(0) as i64;
    update_status(conn, campaign_id, "cancelled");
    log_event(
        conn,
        campaign_id,
        None,
        "campaign_cancelled",
        Some(&format!("{cancelled} remaining recipients cancelled")),
    );
    serde_json::json!({ "ok": true, "message": format!("{cancelled} remaining recipient(s) cancelled. Sent recipients are untouched.") })
}

pub fn campaign_retry_failed(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let Some(_c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return serde_json::json!({ "ok": false, "message": "Campaign not found." });
    };
    let reset = conn
        .execute(
            "UPDATE outreach_recipients SET state = 'queued', attempts = 0, last_error = NULL
              WHERE campaign_id = ?1 AND state = 'failed'",
            [campaign_id],
        )
        .unwrap_or(0) as i64;
    if reset > 0 {
        update_status(conn, campaign_id, "queued");
        log_event(
            conn,
            campaign_id,
            None,
            "campaign_retry_failed",
            Some(&format!(
                "{reset} recipients re-queued (attempt budget reset)"
            )),
        );
        let _ = crate::jobs::enqueue_on(
            conn,
            "outreach",
            "outreach_send_batch",
            &serde_json::json!({ "campaignId": campaign_id }).to_string(),
            1,
            3,
        );
    }
    serde_json::json!({ "ok": true, "message": if reset > 0 {
        format!("{reset} failed recipient(s) re-queued.")
    } else {
        "No failed recipients to retry.".to_string()
    }})
}

/// Audit items 16 + 17: recover recipients whose delivery state is
/// stranded or ambiguous — resolve 'unknown' rows against the platform
/// mirror (sent / safe-retry / exhausted) and recover 'sending' rows left
/// behind by a crash (mirror match → 'sent', otherwise bounded requeue).
/// Returns (resolved_sent, returned_to_queue, still_unknown). Pure state
/// repair: no continuation job is enqueued here.
fn reconcile_recipient_states(conn: &Connection, campaign_id: i64) -> (i64, i64, i64) {
    type UnknownRow = (i64, Option<i64>, Option<String>, Option<String>);
    let unknowns: Vec<UnknownRow> = conn
        .prepare(
            "SELECT r.id, r.customer_remote_id, r.email, oc.subject
               FROM outreach_recipients r
               JOIN outreach_campaigns oc ON oc.id = r.campaign_id
              WHERE r.campaign_id = ?1 AND r.state = 'unknown'",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([campaign_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    let mut resolved_sent = 0i64;
    let mut returned = 0i64;
    let mut still_unknown = 0i64;
    for (rid, customer_remote, email, subject) in unknowns {
        let Some(customer_remote) = customer_remote else {
            let _ = conn.execute(
                "UPDATE outreach_recipients SET state = 'failed',
                     last_error = 'Unknown outcome and no customer id to reconcile against'
                   WHERE id = ?1",
                [rid],
            );
            continue;
        };
        let match_conv: Option<(i64, Option<i64>)> = conn
            .query_row(
                "SELECT remote_id, number FROM conversations
                  WHERE customer_local_id = (SELECT id FROM customers WHERE remote_id = ?1)
                    AND LOWER(TRIM(COALESCE(subject, ''))) = LOWER(TRIM(COALESCE(?2, '')))
                  LIMIT 1",
                rusqlite::params![customer_remote, subject],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let attempts: i64 = conn
            .query_row(
                "SELECT attempts FROM outreach_recipients WHERE id = ?1",
                [rid],
                |r| r.get(0),
            )
            .unwrap_or(0);
        match match_conv {
            Some((remote, number)) => {
                let _ = conn.execute(
                    "UPDATE outreach_recipients SET state = 'sent', hs_conversation_remote_id = ?1,
                         hs_conversation_number = ?2, sent_at = datetime('now')
                       WHERE id = ?3",
                    rusqlite::params![remote, number, rid],
                );
                log_event(
                    conn,
                    campaign_id,
                    Some(rid),
                    "recipient_reconciled_sent",
                    Some(&format!("found conversation {}", number.unwrap_or(remote))),
                );
                resolved_sent += 1;
            }
            // Audit item 17: reconciliation proved NOT delivered and
            // attempts remain, so the row returns to the retryable pool —
            // 'failed' here contradicted the branch guard, the message and
            // the `returned` counter (and, with attempts always 0 before
            // item 16, it fired every time, so no reconciled row ever
            // actually retried). Item 16's attempts cap bounds the retry.
            None if attempts < MAX_ATTEMPTS => {
                let _ = conn.execute(
                    "UPDATE outreach_recipients SET state = 'queued',
                         last_error = 'Reconciled: not delivered, safe to retry',
                         updated_at = datetime('now')
                       WHERE id = ?1",
                    [rid],
                );
                log_event(
                    conn,
                    campaign_id,
                    Some(rid),
                    "recipient_reconciled_retry",
                    None,
                );
                returned += 1;
            }
            // Reconciliation proved not delivered AND the attempts are
            // exhausted: the honest terminal state is 'failed' with the
            // reason recorded (previously the row lingered in 'unknown'
            // forever, re-logged by every reconcile pass).
            None => {
                let _ = conn.execute(
                    "UPDATE outreach_recipients SET state = 'failed',
                         last_error = 'Reconciled: not delivered and attempts exhausted',
                         updated_at = datetime('now')
                       WHERE id = ?1",
                    [rid],
                );
                log_event(
                    conn,
                    campaign_id,
                    Some(rid),
                    "recipient_reconciled_unknown",
                    Some("not found but attempts exhausted"),
                );
                still_unknown += 1;
            }
        }
        let _ = email;
    }

    // Audit item 17: rows stranded in 'sending' by a crash. The original
    // bulk requeue was doubly dead code: it compared `updated_at` against
    // the literal TEXT of a datetime() expression bound as a parameter
    // (never a past timestamp), and the column itself only exists from
    // this batch on. Probe the platform mirror first — a crashed send
    // that actually landed is marked 'sent' (no duplicate email), and
    // only a genuinely undelivered row returns to 'queued' for a
    // bounded retry.
    let campaign_subject: Option<String> = conn
        .query_row(
            "SELECT subject FROM outreach_campaigns WHERE id = ?1",
            [campaign_id],
            |r| r.get(0),
        )
        .ok();
    let stale_sending: Vec<(i64, Option<i64>)> = conn
        .prepare(
            "SELECT id, customer_remote_id FROM outreach_recipients
              WHERE campaign_id = ?1 AND state = 'sending'
                AND updated_at < datetime('now', '-10 minutes')",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([campaign_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|x| x.ok()).collect())
                .ok()
        })
        .unwrap_or_default();
    let mut stale_requeued: i64 = 0;
    let mut stale_resolved_sent: i64 = 0;
    for (rid, customer_remote) in stale_sending {
        let probeable_subject = campaign_subject
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let match_conv: Option<(i64, Option<i64>)> = match (customer_remote, probeable_subject) {
            (Some(cr), Some(subject)) => conn
                .query_row(
                    "SELECT remote_id, number FROM conversations
                      WHERE customer_local_id = (SELECT id FROM customers WHERE remote_id = ?1)
                        AND LOWER(TRIM(COALESCE(subject, ''))) = LOWER(TRIM(COALESCE(?2, '')))
                      LIMIT 1",
                    rusqlite::params![cr, subject],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok(),
            _ => None,
        };
        match match_conv {
            Some((remote, number)) => {
                let _ = conn.execute(
                    "UPDATE outreach_recipients SET state = 'sent',
                         hs_conversation_remote_id = ?1, hs_conversation_number = ?2,
                         sent_at = datetime('now'),
                         last_error = 'Recovered: crashed send had landed (mirror match)',
                         updated_at = datetime('now')
                       WHERE id = ?3",
                    rusqlite::params![remote, number, rid],
                );
                log_event(
                    conn,
                    campaign_id,
                    Some(rid),
                    "recipient_reconciled_sent",
                    Some("stale sending recovered as sent from the platform mirror"),
                );
                stale_resolved_sent += 1;
            }
            None => {
                let _ = conn.execute(
                    "UPDATE outreach_recipients SET state = 'queued',
                         last_error = 'Requeued: stale sending state (reconcile; no platform match)',
                         updated_at = datetime('now')
                       WHERE id = ?1",
                    [rid],
                );
                stale_requeued += 1;
            }
        }
    }
    if stale_requeued > 0 || stale_resolved_sent > 0 {
        log_event(
            conn,
            campaign_id,
            None,
            "recipient_stale_sending_requeued",
            Some(&format!(
                "{stale_resolved_sent} recovered as sent, {stale_requeued} requeued from stale sending state"
            )),
        );
    }

    (resolved_sent, returned, still_unknown)
}

/// The full public reconcile pass: recover stranded/ambiguous recipients
/// (`reconcile_recipient_states`), then either keep the batch chain going
/// or complete the campaign honestly. The send loop calls only the recovery
/// portion so it never double-enqueues the continuation job.
pub fn campaign_reconcile(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let (resolved_sent, returned, still_unknown) = reconcile_recipient_states(conn, campaign_id);
    let remaining = count_remaining(conn, campaign_id);
    if remaining > 0 {
        let _ = crate::jobs::enqueue_on(
            conn,
            "outreach",
            "outreach_send_batch",
            &serde_json::json!({ "campaignId": campaign_id }).to_string(),
            1,
            3,
        );
    } else {
        if let Some(c) = get_outreach_campaign(conn, campaign_id).ok().flatten()
            && c.status != "completed"
            && c.status != "cancelled"
            && c.status != "paused"
        {
            update_status(conn, campaign_id, "completed");
            let _ = conn.execute(
                "UPDATE outreach_campaigns SET completed_at = datetime('now') WHERE id = ?1",
                [campaign_id],
            );
            log_event(
                conn,
                campaign_id,
                None,
                "campaign_completed",
                Some(&format!(
                    "{} sent, {} failed, {} skipped{}",
                    c.sent,
                    c.failed,
                    c.skipped,
                    if still_unknown > 0 {
                        format!(
                            ", {still_unknown} recipients left with unknown delivery outcomes (attempts exhausted)"
                        )
                    } else {
                        String::new()
                    }
                )),
            );
        }
    }
    serde_json::json!({
        "resolvedSent": resolved_sent,
        "returnedToQueue": returned,
        "stillUnknown": still_unknown
    })
}

pub fn refresh_replies(conn: &Connection, campaign_id: i64) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let sent: Vec<(i64, Option<i64>, Option<String>)> = {
        let mut stmt = tx.prepare(
            "SELECT r.id, r.hs_conversation_remote_id, r.sent_at
               FROM outreach_recipients r
              WHERE r.campaign_id = ?1 AND r.state = 'sent'
                AND r.replied_at IS NULL
                AND r.hs_conversation_remote_id IS NOT NULL",
        )?;

        stmt.query_map([campaign_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };
    let mut replied = 0i64;
    for (id, remote_id, sent_at) in sent {
        let Some(remote) = remote_id else { continue };
        let Some(conv_local): Option<i64> = tx
            .query_row(
                "SELECT id FROM conversations WHERE remote_id = ?1",
                [remote],
                |r| r.get(0),
            )
            .ok()
        else {
            continue;
        };
        let customer_thread: Option<String> = tx
            .query_row(
                "SELECT created_at FROM conversation_threads
                  WHERE conversation_id = ?1 AND type = 'customer'
                    AND deleted_at IS NULL
                    AND julianday(created_at) > julianday(COALESCE(?2, '1970-01-01'))
                  ORDER BY created_at ASC LIMIT 1",
                rusqlite::params![conv_local, sent_at],
                |r| r.get(0),
            )
            .ok();
        if let Some(thread_at) = customer_thread {
            tx.execute(
                "UPDATE outreach_recipients SET replied_at = ?1 WHERE id = ?2",
                rusqlite::params![thread_at, id],
            )?;
            log_event(&tx, campaign_id, Some(id), "recipient_replied", None);
            replied += 1;
        }
    }
    tx.commit()?;
    Ok(replied)
}

pub fn campaign_report_not_found() -> serde_json::Value {
    serde_json::json!({
        "campaign": serde_json::Value::Null,
        "totals": { "recipients": 0, "sent": 0, "failed": 0, "skipped": 0,
                    "cancelled": 0, "unknown": 0, "replied": 0, "reply_rate": serde_json::Value::Null },
        "replies": [],
        "note": "Campaign not found."
    })
}

pub fn campaign_report(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    if get_outreach_campaign(conn, campaign_id)
        .ok()
        .flatten()
        .is_none()
    {
        return serde_json::json!({
            "campaign": serde_json::Value::Null,
            "totals": { "recipients": 0, "sent": 0, "failed": 0, "skipped": 0,
                        "cancelled": 0, "unknown": 0, "replied": 0, "reply_rate": serde_json::Value::Null },
            "replies": [],
            "note": "Campaign not found."
        });
    }
    let _ = refresh_replies(conn, campaign_id);
    let Some(c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return campaign_report_not_found();
    };
    let cancelled: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outreach_recipients WHERE campaign_id = ?1 AND state = 'cancelled'",
            [campaign_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let mut replies: Vec<serde_json::Value> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT r.customer_local_id, r.email, r.hs_conversation_number, r.replied_at,
                c.first_name, c.last_name, cv.id
           FROM outreach_recipients r
           LEFT JOIN customers c ON c.id = r.customer_local_id
           LEFT JOIN conversations cv ON cv.remote_id = r.hs_conversation_remote_id
          WHERE r.campaign_id = ?1 AND r.replied_at IS NOT NULL
          ORDER BY r.replied_at DESC LIMIT 500",
    ) && let Ok(rows) = stmt.query_map([campaign_id], |r| {
        Ok(serde_json::json!({
            "customer_local_id": r.get::<_, i64>(0)?,
            "email": r.get::<_, Option<String>>(1)?,
            "hs_conversation_number": r.get::<_, Option<i64>>(2)?,
            "replied_at": r.get::<_, Option<String>>(3)?,
            "first_name": r.get::<_, Option<String>>(4)?,
            "last_name": r.get::<_, Option<String>>(5)?,
            "conversation_local_id": r.get::<_, Option<i64>>(6)?,
        }))
    }) {
        for row in rows.flatten() {
            let name = {
                let first = row["first_name"].as_str().unwrap_or("");
                let last = row["last_name"].as_str().unwrap_or("");
                let joined = format!("{first} {last}").trim().to_string();
                if !joined.is_empty() {
                    joined
                } else if let Some(e) = row["email"].as_str() {
                    e.to_string()
                } else {
                    format!("customer {}", row["customer_local_id"])
                }
            };
            replies.push(serde_json::json!({
                "customer": name,
                "conversation_number": row["hs_conversation_number"],
                "conversation_local_id": row["conversation_local_id"],
                "replied_at": row["replied_at"],
            }));
        }
    }
    let reply_rate = if c.sent > 0 {
        Some((c.replied as f64 / c.sent as f64 * 100.0).round() / 100.0)
    } else {
        None
    };
    serde_json::json!({
        "campaign": { "id": c.id, "name": c.name, "status": c.status },
        "totals": {
            "recipients": c.recipients, "sent": c.sent, "failed": c.failed,
            "skipped": c.skipped, "cancelled": cancelled, "unknown": c.unknown,
            "replied": c.replied, "reply_rate": reply_rate
        },
        "replies": replies,
        "note": "These are SupportOS campaign conversation outcomes based on Help Scout conversation state in the local mirror - not email-delivery analytics."
    })
}

pub fn campaign_delete(conn: &Connection, campaign_id: i64) -> serde_json::Value {
    let Some(c) = get_outreach_campaign(conn, campaign_id).ok().flatten() else {
        return serde_json::json!({ "ok": false, "message": "Campaign not found." });
    };
    if c.status == "queued" || c.status == "sending" {
        return serde_json::json!({ "ok": false, "message": "Pause or cancel the campaign before deleting it." });
    }
    let ok = conn
        .execute(
            "DELETE FROM outreach_campaigns WHERE id = ?1",
            [campaign_id],
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    serde_json::json!({ "ok": ok, "message": if ok { "Campaign deleted (audit events and Help Scout conversations are untouched)." } else { "Delete failed." } })
}

pub fn ensure_segments_v2(conn: &Connection) -> Result<()> {
    fn add_column_if_missing(
        conn: &Connection,
        table: &str,
        column: &str,
        decl: &str,
    ) -> Result<()> {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info(?) WHERE name = ?",
                params![table, column],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !exists {
            conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl};"))?;
        }
        Ok(())
    }
    add_column_if_missing(conn, "saved_segments", "description", "TEXT")?;
    add_column_if_missing(conn, "saved_segments", "condition_tree", "TEXT")?;
    add_column_if_missing(
        conn,
        "saved_segments",
        "version",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    add_column_if_missing(
        conn,
        "saved_segments",
        "updated_at",
        "TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
    )?;
    Ok(())
}

fn segment_row_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    let id: i64 = r.get(0)?;
    let name: String = r.get(1)?;
    let description: Option<String> = r.get(2)?;
    let condition_tree: Option<String> = r.get(3)?;
    let version: i64 = r.get::<_, Option<i64>>(4)?.unwrap_or(1);
    let created_at: String = r.get(5)?;
    let updated_at: Option<String> = r.get(6)?;
    let definition = condition_tree
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .filter(|v| {
            v.get("conditions").and_then(|c| c.as_array()).is_some()
                && v.get("exclude").and_then(|e| e.as_array()).is_some()
        })
        .unwrap_or_else(
            || serde_json::json!({"combinator": "all", "conditions": [], "exclude": []}),
        );
    Ok(serde_json::json!({
        "id": id,
        "name": name,
        "description": description,
        "definition": definition,
        "version": version,
        "created_at": created_at,
        "updated_at": updated_at.unwrap_or(created_at),
    }))
}

pub fn list_segments_v2(conn: &Connection) -> Result<Vec<serde_json::Value>> {
    ensure_segments_v2(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, condition_tree, version, created_at, updated_at
           FROM saved_segments ORDER BY updated_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], segment_row_json)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

pub fn get_segment_v2(conn: &Connection, id: i64) -> Result<Option<serde_json::Value>> {
    ensure_segments_v2(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, condition_tree, version, created_at, updated_at
           FROM saved_segments WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], segment_row_json)?;
    Ok(rows.next().transpose()?)
}

pub fn save_segment_v2(
    conn: &Connection,
    id: Option<i64>,
    name: &str,
    description: Option<&str>,
    definition_json: &str,
) -> Result<i64> {
    ensure_segments_v2(conn)?;
    if let Some(id) = id.filter(|id| *id > 0) {
        let exists: bool = conn
            .query_row("SELECT 1 FROM saved_segments WHERE id = ?1", [id], |_| {
                Ok(true)
            })
            .unwrap_or(false);
        if exists {
            conn.execute(
                "UPDATE saved_segments SET name = ?1, description = ?2, condition_tree = ?3,
                        version = version + 1, updated_at = datetime('now')
                  WHERE id = ?4",
                params![name, description, definition_json, id],
            )?;
            return Ok(id);
        }
    }
    conn.execute(
        "INSERT INTO saved_segments (name, criteria_json, description, condition_tree, version)
         VALUES (?1, '[]', ?2, ?3, 1)",
        params![name, description, definition_json],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(clippy::too_many_arguments)]
pub fn create_outreach_campaign(
    conn: &Connection,
    name: &str,
    subject: &str,
    body: &str,
    mailbox_local_id: i64,
    tags: &[String],
    segment_id: Option<i64>,
    segment_snapshot: Option<&str>,
    recipients: &[serde_json::Value],
) -> Result<i64> {
    conn.execute(
        "INSERT INTO outreach_campaigns (name, subject, body, mailbox_local_id, tags, status, segment_id, segment_snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6, ?7)",
        params![name, subject, body, mailbox_local_id, serde_json::to_string(tags).unwrap_or_default(), segment_id, segment_snapshot],
    )?;
    let campaign_id = conn.last_insert_rowid();
    let mut ins = conn.prepare(
        "INSERT OR IGNORE INTO outreach_recipients
             (campaign_id, customer_local_id, customer_remote_id, email, snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for rec in recipients {
        let snapshot = serde_json::json!({
            "why": rec.get("why").cloned().unwrap_or(serde_json::json!([])),
            "matching_tickets": rec.get("matching_tickets").cloned().unwrap_or(serde_json::json!([])),
            "property_values": rec.get("properties").cloned().unwrap_or(serde_json::json!([])),
            "selected_at": now_iso(),
        });
        ins.execute(params![
            campaign_id,
            rec.get("customer_local_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            rec.get("customer_remote_id").and_then(|v| v.as_i64()),
            rec.get("chosen_email").and_then(|v| v.as_str()),
            serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string()),
        ])?;
    }
    log_event(
        conn,
        campaign_id,
        None,
        "campaign_created",
        Some(&format!("{} recipients snapshotted", recipients.len())),
    );
    Ok(campaign_id)
}

fn now_iso() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            let days = secs / 86400;
            let rem = secs % 86400;
            let (y, mo, da) = civil_from_days(days as i64);
            format!(
                "{y:04}-{mo:02}-{da:02}T{:02}:{:02}:{:02}.000Z",
                rem / 3600,
                (rem % 3600) / 60,
                rem % 60
            )
        })
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000Z".to_string())
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn list_outreach_campaigns(conn: &Connection) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(&format!(
        "{CAMPAIGN_SELECT} ORDER BY oc.created_at DESC, oc.id DESC"
    ))?;
    let rows = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "subject": r.get::<_, String>(2)?,
            "status": r.get::<_, String>(6)?,
            "mailbox_local_id": r.get::<_, Option<i64>>(4)?,
            "mailbox_name": r.get::<_, Option<String>>(11)?,
            "segment_id": r.get::<_, Option<i64>>(7)?,
            "segment_name": r.get::<_, Option<String>>(12)?,
            "recipients": r.get::<_, i64>(13)?,
            "sent": r.get::<_, i64>(14)?,
            "failed": r.get::<_, i64>(15)?,
            "skipped": r.get::<_, i64>(16)?,
            "unknown": r.get::<_, i64>(17)?,
            "replied": r.get::<_, i64>(18)?,
            "created_at": r.get::<_, String>(8)?,
            "queued_at": r.get::<_, Option<String>>(9)?,
            "completed_at": r.get::<_, Option<String>>(10)?,
        }))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

pub fn list_campaign_recipients(
    conn: &Connection,
    campaign_id: i64,
    limit: i64,
) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.customer_local_id, r.customer_remote_id, r.email, r.snapshot,
                r.state, r.attempts, r.last_error, r.hs_conversation_remote_id,
                r.hs_conversation_number, r.sent_at, r.replied_at,
                c.first_name, c.last_name
           FROM outreach_recipients r
           LEFT JOIN customers c ON c.id = r.customer_local_id
          WHERE r.campaign_id = ?1 ORDER BY r.id LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![campaign_id, limit], |r| {
        let snapshot_raw: String = r.get(4)?;
        let snapshot: serde_json::Value =
            serde_json::from_str(&snapshot_raw).unwrap_or(serde_json::json!({}));
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "customer_local_id": r.get::<_, i64>(1)?,
            "customer_remote_id": r.get::<_, Option<i64>>(2)?,
            "first_name": r.get::<_, Option<String>>(12)?,
            "last_name": r.get::<_, Option<String>>(13)?,
            "email": r.get::<_, Option<String>>(3)?,
            "state": r.get::<_, String>(5)?,
            "attempts": r.get::<_, i64>(6)?,
            "last_error": r.get::<_, Option<String>>(7)?,
            "hs_conversation_remote_id": r.get::<_, Option<i64>>(8)?,
            "hs_conversation_number": r.get::<_, Option<i64>>(9)?,
            "conversation_local_id": serde_json::Value::Null,
            "sent_at": r.get::<_, Option<String>>(10)?,
            "replied_at": r.get::<_, Option<String>>(11)?,
            "why": snapshot.get("why").cloned().unwrap_or(serde_json::json!([])),
            "matching_tickets": snapshot.get("matching_tickets").cloned().unwrap_or(serde_json::json!([])),
        }))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

pub fn get_campaign_full(conn: &Connection, id: i64) -> Result<Option<serde_json::Value>> {
    let Some(c) = get_outreach_campaign(conn, id)? else {
        return Ok(None);
    };
    let segment_snapshot: serde_json::Value = conn
        .query_row(
            "SELECT segment_snapshot FROM outreach_campaigns WHERE id = ?1",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .filter(|v: &serde_json::Value| {
            v.get("conditions").and_then(|x| x.as_array()).is_some()
                && v.get("exclude").and_then(|x| x.as_array()).is_some()
        })
        .unwrap_or_else(
            || serde_json::json!({"combinator": "all", "conditions": [], "exclude": []}),
        );
    let recipients_list = list_campaign_recipients(conn, id, 1000)?;
    Ok(Some(serde_json::json!({
        "id": c.id,
        "name": c.name,
        "subject": c.subject,
        "body": c.body,
        "status": c.status,
        "mailbox_local_id": c.mailbox_local_id,
        "mailbox_name": c.mailbox_name,
        "segment_id": c.segment_id,
        "segment_name": c.segment_name,
        "tags": c.tags,
        "segment_snapshot": segment_snapshot,
        "recipients": c.recipients,
        "sent": c.sent,
        "failed": c.failed,
        "skipped": c.skipped,
        "unknown": c.unknown,
        "replied": c.replied,
        "created_at": c.created_at,
        "queued_at": c.queued_at,
        "completed_at": c.completed_at,
        "recipients_list": recipients_list,
    })))
}

pub const SEND_BATCH_SIZE: i64 = 5;

pub const SEND_MAX_ATTEMPTS: i64 = 3;

#[allow(dead_code)]
struct BatchRow {
    recipient_id: i64,
    customer_local_id: i64,
    customer_remote_id: Option<i64>,
    email: Option<String>,
    snapshot: String,
    attempts: i64,
}

enum SendOutcome {
    Sent { remote_id: i64, number: i64 },
    FailedPermanent(String),
    FailedRetryable(String),
    Unknown(String),
}

impl SendOutcome {
    fn label(&self) -> &'static str {
        match self {
            Self::Sent { .. } => "sent",
            Self::FailedPermanent(_) => "failed",
            Self::FailedRetryable(_) => "queued",
            Self::Unknown(_) => "unknown",
        }
    }
}

fn skip_ineligible_recipients(conn: &Connection, campaign_id: i64) -> i64 {
    let no_email = conn
        .execute(
            "UPDATE outreach_recipients
                SET state = 'skipped', last_error = 'No usable email address on file.'
              WHERE campaign_id = ?1
                AND state IN ('selected','queued')
                AND (email IS NULL OR TRIM(email) = '')",
            [campaign_id],
        )
        .unwrap_or(0) as i64;
    let on_dnc = conn
        .execute(
            "UPDATE outreach_recipients
                SET state = 'skipped', last_error = 'On Do-Not-Contact list.'
              WHERE campaign_id = ?1
                AND state IN ('selected','queued')
                AND customer_local_id IN (SELECT customer_id FROM do_not_contact)",
            [campaign_id],
        )
        .unwrap_or(0) as i64;
    no_email + on_dnc
}

/// Audit item 17: the send loop does network I/O between the pick and the
/// outcome write, so a crash (or a second worker) would re-pick the same
/// 'queued' rows and send duplicate emails. Claim each picked row by
/// moving it to the 'sending' state before it leaves the database guard;
/// a row whose claim UPDATE changed nothing was raced away and is simply
/// not sent. Rows stranded in 'sending' by a crash are recovered by the
/// reconcile sweep's staleness arm (mirror probe, then bounded requeue).
fn pick_batch(conn: &Connection, campaign_id: i64) -> Vec<BatchRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, customer_local_id, customer_remote_id, email, snapshot, attempts
           FROM outreach_recipients
          WHERE campaign_id = ?1 AND state IN ('selected','queued')
          ORDER BY id ASC LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt
        .query_map(params![campaign_id, SEND_BATCH_SIZE], |r| {
            Ok(BatchRow {
                recipient_id: r.get(0)?,
                customer_local_id: r.get(1)?,
                customer_remote_id: r.get(2)?,
                email: r.get(3)?,
                snapshot: r.get(4)?,
                attempts: r.get(5)?,
            })
        })
        .ok();
    let picked: Vec<BatchRow> = rows
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    let mut claimed = Vec::with_capacity(picked.len());
    for row in picked {
        let claimed_now = conn
            .execute(
                "UPDATE outreach_recipients
                    SET state = 'sending', updated_at = datetime('now')
                  WHERE id = ?1 AND state IN ('selected','queued')",
                params![row.recipient_id],
            )
            .unwrap_or(0);
        if claimed_now > 0 {
            claimed.push(row);
        }
    }
    claimed
}

fn classify_error(e: &crate::error::Error, attempts: i64) -> SendOutcome {
    let status = crate::helpscout_real::hs_status(e).unwrap_or(0);
    let label = e.to_string();
    match status {
        0 => SendOutcome::FailedRetryable(format!("Network error: {label}")),
        200 => SendOutcome::Unknown(format!(
            "Provider accepted the send but returned no conversation id. {label}"
        )),
        400 | 403 | 404 | 409 | 412 | 413 | 415 | 423 => {
            SendOutcome::FailedPermanent(format!("Provider error {status}: {label}"))
        }
        429 | 500 | 502 | 503 | 504 => {
            if attempts + 1 >= SEND_MAX_ATTEMPTS {
                SendOutcome::FailedPermanent(format!(
                    "Provider error {status} after {attempts} attempts: {label}"
                ))
            } else {
                SendOutcome::FailedRetryable(format!("Provider error {status}: {label}"))
            }
        }
        _ => SendOutcome::FailedRetryable(format!("Provider error {status}: {label}")),
    }
}

fn record_outcome(
    conn: &Connection,
    campaign_id: i64,
    row: &BatchRow,
    attempt_no: i64,
    outcome: &SendOutcome,
    error_or_remote: Option<&str>,
) {
    let now = datetime_now();
    let _ = conn.execute(
        "INSERT INTO outreach_attempts
             (recipient_id, attempt_no, started_at, finished_at, result, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.recipient_id,
            attempt_no,
            now,
            datetime_now(),
            outcome.label(),
            match outcome {
                SendOutcome::Sent { .. } => None,
                SendOutcome::FailedPermanent(m)
                | SendOutcome::FailedRetryable(m)
                | SendOutcome::Unknown(m) => Some(m.as_str()),
            }
        ],
    );

    // Audit item 16: every recorded outcome is a spent attempt. The
    // recipient row's `attempts` is the counter `classify_error` and the
    // exhaustion backstop read; without this increment it stayed 0
    // forever, so 429/5xx answers were retryable without limit and the
    // same stuck recipients filled every batch (infinite retry storm
    // + head-of-line block).
    //
    // Audit item 17: `updated_at` is the claim/staleness clock the
    // reconcile sweep reads (a claimed row that never finished is
    // re-queued only once it is provably stale).
    let _ = match outcome {
        SendOutcome::Sent { remote_id, number } => conn.execute(
            "UPDATE outreach_recipients
                SET state = 'sent',
                    attempts = attempts + 1,
                    hs_conversation_remote_id = ?1,
                    hs_conversation_number = ?2,
                    sent_at = datetime('now'),
                    last_error = NULL,
                    updated_at = datetime('now')
              WHERE id = ?3",
            params![remote_id, number, row.recipient_id],
        ),
        SendOutcome::FailedPermanent(msg) => conn.execute(
            "UPDATE outreach_recipients
                SET state = 'failed', attempts = attempts + 1, last_error = ?1,
                    updated_at = datetime('now')
              WHERE id = ?2",
            params![msg, row.recipient_id],
        ),
        SendOutcome::FailedRetryable(msg) => conn.execute(
            "UPDATE outreach_recipients
                SET state = 'queued', attempts = attempts + 1, last_error = ?1,
                    updated_at = datetime('now')
              WHERE id = ?2",
            params![msg, row.recipient_id],
        ),
        SendOutcome::Unknown(msg) => conn.execute(
            "UPDATE outreach_recipients
                SET state = 'unknown', attempts = attempts + 1, last_error = ?1,
                    updated_at = datetime('now')
              WHERE id = ?2",
            params![msg, row.recipient_id],
        ),
    };

    let event = match outcome {
        SendOutcome::Sent { .. } => "recipient_sent",
        SendOutcome::FailedPermanent(_) => "recipient_failed",
        SendOutcome::FailedRetryable(_) => "recipient_retryable_error",
        SendOutcome::Unknown(_) => "recipient_unknown",
    };
    log_event(
        conn,
        campaign_id,
        Some(row.recipient_id),
        event,
        error_or_remote,
    );
}

fn datetime_now() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Audit item 17: `updated_at` on outreach_recipients is the claim and
/// staleness clock. Databases created before this batch lack the column
/// (the reconcile sweep's stale-state query referenced it and silently
/// no-opped), so it is ensured lazily at the top of every campaign batch
/// run — the same self-healing pattern `ensure_main_rule_columns` uses.
fn ensure_recipient_updated_at(conn: &Connection) -> Result<()> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info(?) WHERE name = ?",
            params!["outreach_recipients", "updated_at"],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !exists {
        // ALTER TABLE ADD COLUMN only accepts CONSTANT defaults — SQLite
        // rejects the expression default (`DEFAULT (strftime(...))`) the
        // schema used at CREATE time. Add the column nullable and backfill
        // the clock from each row's created_at, which is the correct
        // zero-point for staleness accounting.
        conn.execute_batch(
            "ALTER TABLE outreach_recipients ADD COLUMN updated_at TEXT;
             UPDATE outreach_recipients
                SET updated_at = COALESCE(created_at, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
              WHERE updated_at IS NULL;",
        )?;
    }
    Ok(())
}

pub async fn send_batch(
    conn: &crate::db::DbHandle,
    provider: &Arc<dyn crate::helpscout::HelpScoutProvider>,
    campaign_id: i64,
) -> serde_json::Value {
    let (campaign, mailbox_local_id, tags, subject, body, batch) = {
        let c = conn.write_guard();
        // Audit items 16 + 17: self-heal before every batch. Ensure the
        // claim/staleness clock column, then run the reconcile sweep —
        // crash-stranded 'sending' rows are recovered (mirror probe →
        // 'sent', or bounded requeue) and ambiguous 'unknown' rows are
        // resolved against the platform mirror BEFORE anything new is
        // picked, so no send is retried blindly.
        if let Err(e) = ensure_recipient_updated_at(&c) {
            log_event(
                &c,
                campaign_id,
                None,
                "recipient_schema_ensure_failed",
                Some(&e.to_string()),
            );
        }
        // Recovery only — the continuation/completion logic stays with
        // finalize_if_drained at the end of the batch, so this can never
        // double-enqueue the next batch job.
        let _ = reconcile_recipient_states(&c, campaign_id);
        let _ = skip_ineligible_recipients(&c, campaign_id);
        let Some(camp) = get_outreach_campaign(&c, campaign_id).unwrap_or_default() else {
            return serde_json::json!({ "ok": false, "message": "Campaign not found." });
        };
        if camp.status == "paused" || camp.status == "cancelled" || camp.status == "completed" {
            return serde_json::json!({
                "ok": false,
                "message": format!(
                    "Campaign is {}; nothing to send.",
                    camp.status
                ),
            });
        }
        if camp.status != "sending" {
            update_status(&c, campaign_id, "sending");
        }
        let mailbox_local_id = camp.mailbox_local_id;
        let tags = camp.tags.clone();
        let subject = camp.subject.clone();
        let body = camp.body.clone();
        let batch = pick_batch(&c, campaign_id);
        (camp, mailbox_local_id, tags, subject, body, batch)
    };

    let _ = campaign;

    if batch.is_empty() {
        return finalize_if_drained(conn, campaign_id);
    }

    let Some(mailbox_local_id) = mailbox_local_id else {
        // Scoped: the writer guard must be dropped before
        // finalize_if_drained re-acquires the lane (let-else keeps
        // bindings alive until the block ends, and holding the guard
        // across the call self-deadlocks the same-thread mutex).
        {
            let c = conn.write_guard();
            for row in &batch {
                record_outcome(
                    &c,
                    campaign_id,
                    row,
                    row.attempts + 1,
                    &SendOutcome::FailedPermanent(
                        "Campaign has no sending mailbox configured.".into(),
                    ),
                    Some("no mailbox"),
                );
            }
        }
        return finalize_if_drained(conn, campaign_id);
    };

    // Audit item 15: the campaign stores the LOCAL mirror row id of the
    // mailbox, but the platform's `mailboxId` must be the REMOTE Help
    // Scout mailbox id — the local row id (1, 2, ...) is not a valid
    // mailbox on the account, so every send failed. Resolve the remote
    // id once per batch; a mailbox that never synced is a permanent,
    // honestly-reported failure for the whole batch.
    let mailbox_remote_id: Option<i64> = {
        let c = conn.write_guard();
        c.query_row(
            "SELECT remote_id FROM mailboxes WHERE id = ?1",
            [mailbox_local_id],
            |r| r.get(0),
        )
        .ok()
    };
    let Some(mailbox_remote_id) = mailbox_remote_id else {
        // Scoped the same way: drop the writer lane before finalizing.
        {
            let c = conn.write_guard();
            for row in &batch {
                record_outcome(
                    &c,
                    campaign_id,
                    row,
                    row.attempts + 1,
                    &SendOutcome::FailedPermanent(
                        "Sending mailbox has no remote id (not synced) - cannot create \
                         conversations on the platform."
                            .into(),
                    ),
                    Some("mailbox not synced"),
                );
            }
        }
        return finalize_if_drained(conn, campaign_id);
    };

    let mut sent_remote_ids: Vec<i64> = Vec::new();
    let mut counts = BatchCounts::default();

    for row in batch {
        let attempt_no = row.attempts + 1;

        let Some(customer_remote_id) = row.customer_remote_id else {
            let c = conn.write_guard();
            record_outcome(
                &c,
                campaign_id,
                &row,
                attempt_no,
                &SendOutcome::FailedPermanent(
                    "Recipient has no remote customer id; cannot create conversation.".into(),
                ),
                Some("no customer_remote_id"),
            );
            counts.failed_permanent += 1;
            continue;
        };

        let rendered = {
            let c = conn.write_guard();
            let matching_tickets: Vec<serde_json::Value> =
                serde_json::from_str::<serde_json::Value>(&row.snapshot)
                    .ok()
                    .and_then(|v| v.get("matching_tickets").cloned())
                    .and_then(|t| serde_json::from_value(t).ok())
                    .unwrap_or_default();
            render_for(
                &c,
                row.customer_local_id,
                &matching_tickets,
                &subject,
                &body,
            )
        };
        let r_subject = rendered["subject"].as_str().unwrap_or(&subject).to_string();
        let r_body = rendered["body"].as_str().unwrap_or(&body).to_string();

        let input = crate::helpscout::CreateConversationInput {
            mailbox_id: mailbox_remote_id,
            customer_id: customer_remote_id,
            subject: r_subject,
            body: r_body,
            tags: tags.clone(),
            status: Some("active".to_string()),
        };
        let result = provider.create_conversation(input).await;

        let outcome = match result {
            Ok(crate::helpscout::ConversationCreated {
                conversation_id,
                number,
                ..
            }) => {
                counts.sent += 1;
                sent_remote_ids.push(conversation_id);
                SendOutcome::Sent {
                    remote_id: conversation_id,
                    number,
                }
            }
            Err(e) => {
                let mut o = classify_error(&e, row.attempts);
                match &o {
                    SendOutcome::Sent { .. } => {}
                    SendOutcome::FailedPermanent(_) => counts.failed_permanent += 1,
                    SendOutcome::FailedRetryable(_) => counts.retryable += 1,
                    SendOutcome::Unknown(_) => counts.unknown += 1,
                }
                if let SendOutcome::FailedRetryable(msg) = &o
                    && attempt_no >= SEND_MAX_ATTEMPTS
                {
                    counts.retryable -= 1;
                    counts.failed_permanent += 1;
                    o = SendOutcome::FailedPermanent(format!(
                        "{msg} (attempts exhausted: {attempt_no})"
                    ));
                }
                o
            }
        };

        let error_or_remote: Option<&str> = match &outcome {
            SendOutcome::Sent { remote_id, .. } => {
                let c = conn.write_guard();
                record_outcome(
                    &c,
                    campaign_id,
                    &row,
                    attempt_no,
                    &outcome,
                    Some(&remote_id.to_string()),
                );
                None
            }
            SendOutcome::FailedPermanent(m)
            | SendOutcome::FailedRetryable(m)
            | SendOutcome::Unknown(m) => {
                let c = conn.write_guard();
                record_outcome(&c, campaign_id, &row, attempt_no, &outcome, Some(m));
                None
            }
        };
        let _ = error_or_remote;
    }

    if !sent_remote_ids.is_empty() {
        let c = conn.write_guard();
        for rid in &sent_remote_ids {
            let _ = crate::jobs::enqueue_on(
                &c,
                "sync",
                "sync_conversation",
                &serde_json::json!({ "remoteId": rid }).to_string(),
                2,
                2,
            );
        }
        log_event(
            &c,
            campaign_id,
            None,
            "batch_sync_back_enqueued",
            Some(&format!("{} sync_conversation jobs", sent_remote_ids.len())),
        );
    }

    tracing::info!(
        campaign_id = campaign_id,
        sent = counts.sent,
        failed = counts.failed_permanent,
        retryable = counts.retryable,
        unknown = counts.unknown,
        "Outreach batch outcomes"
    );

    let mut summary = finalize_if_drained(conn, campaign_id);
    if let Some(obj) = summary.as_object_mut() {
        obj.insert(
            "batch".to_string(),
            serde_json::json!({
                "sent": counts.sent,
                "failed": counts.failed_permanent,
                "retryable": counts.retryable,
                "unknown": counts.unknown,
            }),
        );
    }
    summary
}

#[derive(Default)]
struct BatchCounts {
    sent: i64,
    failed_permanent: i64,
    retryable: i64,
    unknown: i64,
}

fn count_remaining_for_send(conn: &Connection, campaign_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM outreach_recipients
          WHERE campaign_id = ?1 AND state IN ('selected','queued','sending')",
        [campaign_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn finalize_if_drained(conn: &crate::db::DbHandle, campaign_id: i64) -> serde_json::Value {
    let (remaining, sent, failed, skipped, unknown, status) = {
        let c = conn.write_guard();
        let remaining = count_remaining_for_send(&c, campaign_id);
        let camp = get_outreach_campaign(&c, campaign_id)
            .ok()
            .flatten()
            .map(|c| (c.sent, c.failed, c.skipped, c.unknown, c.status));
        let (sent, failed, skipped, unknown, status) =
            camp.unwrap_or((0, 0, 0, 0, "unknown".into()));
        (remaining, sent, failed, skipped, unknown, status)
    };

    if remaining > 0 {
        let c = conn.write_guard();
        let _ = crate::jobs::enqueue_on(
            &c,
            "outreach",
            "outreach_send_batch",
            &serde_json::json!({ "campaignId": campaign_id }).to_string(),
            1,
            3,
        );
        return serde_json::json!({
            "ok": true,
            "campaign_id": campaign_id,
            "remaining": remaining,
            "message": "Batch processed; next batch enqueued.",
        });
    }

    {
        let c = conn.write_guard();
        if status != "completed" && status != "cancelled" && status != "paused" {
            update_status(&c, campaign_id, "completed");
            let _ = c.execute(
                "UPDATE outreach_campaigns SET completed_at = datetime('now') WHERE id = ?1",
                [campaign_id],
            );
            log_event(
                &c,
                campaign_id,
                None,
                "campaign_completed",
                Some(&format!(
                    "{sent} sent, {failed} failed, {skipped} skipped, {unknown} unknown"
                )),
            );
        }
    }

    serde_json::json!({
        "ok": true,
        "campaign_id": campaign_id,
        "remaining": 0,
        "message": "Campaign completed: all recipients processed.",
        "totals": {
            "sent": sent, "failed": failed, "skipped": skipped, "unknown": unknown
        }
    })
}

/// Clean-audit batch 18 (item 15): campaign sends must pass the REMOTE
/// Help Scout mailbox id (never the local mirror row id), and a mailbox
/// that never synced must fail the batch honestly.
#[cfg(test)]
mod batch18_tests {
    use super::*;
    use crate::db::DbHandle;
    use crate::error::Error;
    use crate::helpscout::{
        ConversationPatch, ConversationQuery, CreateThreadInput, CustomerQuery, HelpScoutProvider,
        HsBeaconChat, HsDocArticle, HsMailbox, HsRating, HsTag, HsTeam, HsUser, Page,
        ThreadCreated,
    };
    use std::sync::Mutex;

    /// Records every `CreateConversationInput` it is asked to send.
    struct RecordingSender {
        created: Mutex<Vec<crate::helpscout::CreateConversationInput>>,
    }

    #[async_trait::async_trait]
    impl HelpScoutProvider for RecordingSender {
        fn kind(&self) -> &'static str {
            "recording"
        }
        async fn get_me(&self) -> Result<HsUser> {
            Err(Error::Other("not needed".into()))
        }
        async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
            Ok(Vec::new())
        }
        async fn list_users(&self) -> Result<Vec<HsUser>> {
            Ok(Vec::new())
        }
        async fn list_teams(&self) -> Result<Vec<HsTeam>> {
            Ok(Vec::new())
        }
        async fn list_tags(&self) -> Result<Vec<HsTag>> {
            Ok(Vec::new())
        }
        async fn list_conversations(
            &self,
            _query: &ConversationQuery,
        ) -> Result<Page<crate::helpscout::HsConversation>> {
            Ok(Page {
                items: Vec::new(),
                next_cursor: None,
            })
        }
        async fn list_customers(
            &self,
            _query: &CustomerQuery,
        ) -> Result<Page<crate::helpscout::HsCustomer>> {
            Ok(Page {
                items: Vec::new(),
                next_cursor: None,
            })
        }
        async fn list_beacon_chats(&self) -> Result<Vec<HsBeaconChat>> {
            Ok(Vec::new())
        }
        async fn list_docs(&self) -> Result<Vec<HsDocArticle>> {
            Ok(Vec::new())
        }
        async fn list_ratings(&self) -> Result<Vec<HsRating>> {
            Ok(Vec::new())
        }
        async fn create_reply_thread(
            &self,
            _input: CreateThreadInput,
        ) -> Result<crate::helpscout::ThreadCreated> {
            Err(Error::Other("not needed".into()))
        }
        async fn create_note_thread(&self, _input: CreateThreadInput) -> Result<ThreadCreated> {
            Err(Error::Other("not needed".into()))
        }
        async fn update_conversation(
            &self,
            _conversation_id: i64,
            _patch: ConversationPatch,
        ) -> Result<bool> {
            Ok(false)
        }
        async fn create_conversation(
            &self,
            input: crate::helpscout::CreateConversationInput,
        ) -> Result<crate::helpscout::ConversationCreated> {
            self.created
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(input);
            Ok(crate::helpscout::ConversationCreated {
                conversation_id: 777_001,
                number: 4242,
                thread_id: 9001,
            })
        }
    }

    fn db() -> DbHandle {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        DbHandle::new(conn)
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    /// Seeds a mailbox with REMOTE id 2024 and a queued campaign with one
    /// ready recipient. Returns (mailbox_local_id, campaign_id).
    fn seed_campaign(conn: &Connection) -> (i64, i64) {
        conn.execute(
            "INSERT INTO mailboxes (remote_id, name) VALUES (2024, 'Support')",
            [],
        )
        .expect("mailbox");
        let mailbox_local = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO outreach_campaigns (name, subject, body, mailbox_local_id, tags, status)
             VALUES ('winter check', 'Quick question', 'Hello!', ?1, '[]', 'queued')",
            params![mailbox_local],
        )
        .expect("campaign");
        let campaign = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO outreach_recipients
                 (campaign_id, customer_local_id, customer_remote_id, email, snapshot, state)
             VALUES (?1, 1, 5501, 'person@example.com', '{}', 'selected')",
            params![campaign],
        )
        .expect("recipient");
        (mailbox_local, campaign)
    }

    #[test]
    fn campaign_sends_pass_the_remote_mailbox_id() {
        let db = db();
        let (mailbox_local, campaign) = db.with_write(|c| seed_campaign(c));
        let recorder = Arc::new(RecordingSender {
            created: Mutex::new(Vec::new()),
        });
        let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = recorder.clone();
        let summary = rt().block_on(async { send_batch(&db, &sender, campaign).await });

        let created = recorder.created.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(
            created.len(),
            1,
            "one platform conversation per recipient: {summary}"
        );
        assert_ne!(
            created[0].mailbox_id, mailbox_local,
            "the LOCAL mirror row id must never reach the platform"
        );
        assert_eq!(
            created[0].mailbox_id, 2024,
            "the platform must receive the REMOTE mailbox id"
        );
        assert_eq!(created[0].customer_id, 5501);

        let state: String = db
            .with_read(|c| {
                c.query_row(
                    "SELECT state FROM outreach_recipients WHERE campaign_id = ?1",
                    params![campaign],
                    |r| r.get(0),
                )
            })
            .expect("recipient row");
        assert_eq!(state, "sent", "the send is recorded as delivered");
    }

    #[test]
    fn an_unsynced_mailbox_fails_the_batch_honestly() {
        let db = db();
        // A campaign whose mailbox references a local row that does not
        // exist (pruned / never synced): nothing may be sent, and the
        // recipient must carry a permanent, honest failure.
        let campaign = db.with_write(|c| {
            // The live schema foreign-keys campaigns to mailboxes, but a
            // dangling mailbox reference is still constructable in the
            // wild (best-effort `let _ =` deletes); simulate that state.
            c.execute("PRAGMA foreign_keys = OFF", []).expect("fk off");
            c.execute(
                "INSERT INTO outreach_campaigns
                     (name, subject, body, mailbox_local_id, tags, status)
                 VALUES ('broken', 'Hi', 'Body', 999, '[]', 'queued')",
                [],
            )
            .expect("campaign");
            c.execute("PRAGMA foreign_keys = ON", []).expect("fk on");
            let campaign = c.last_insert_rowid();
            c.execute(
                "INSERT INTO outreach_recipients
                     (campaign_id, customer_local_id, customer_remote_id, email, snapshot, state)
                 VALUES (?1, 1, 5501, 'person@example.com', '{}', 'selected')",
                params![campaign],
            )
            .expect("recipient");
            campaign
        });
        let recorder = Arc::new(RecordingSender {
            created: Mutex::new(Vec::new()),
        });
        let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = recorder.clone();
        let summary = rt().block_on(async { send_batch(&db, &sender, campaign).await });

        let created = recorder.created.lock().unwrap_or_else(|p| p.into_inner());
        assert!(
            created.is_empty(),
            "nothing may be sent with an unsynced mailbox: {created:?}"
        );
        let (state, error): (String, Option<String>) = db
            .with_read(|c| {
                c.query_row(
                    "SELECT state, last_error FROM outreach_recipients WHERE campaign_id = ?1",
                    params![campaign],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            })
            .expect("recipient row");
        assert_eq!(state, "failed", "permanent failure, no silent retry loop");
        assert!(
            error.as_deref().unwrap_or("").contains("not synced"),
            "the error says what happened: {error:?} / {summary}"
        );
    }
}

/// Clean-audit batch 19 (items 16 + 17): recipient attempts are spent and
/// counted, picked rows are claimed against duplicate sends, and
/// crash-stranded / ambiguous rows recover through the reconcile sweep.
#[cfg(test)]
mod batch19_tests {
    use super::*;
    use crate::db::DbHandle;
    use crate::error::Error;
    use crate::helpscout::{
        ConversationPatch, ConversationQuery, CreateThreadInput, CustomerQuery, HelpScoutProvider,
        HsBeaconChat, HsDocArticle, HsMailbox, HsRating, HsTag, HsTeam, HsUser, Page,
        ThreadCreated,
    };
    use crate::helpscout_real::HsApiError;
    use std::sync::Mutex;

    /// Fails the first N `create_conversation` calls with a retryable 500,
    /// then records and succeeds — lets one mock drive both the retry-storm
    /// and the happy-path tests.
    struct ScriptedProvider {
        failures_remaining: Mutex<i64>,
        created: Mutex<Vec<crate::helpscout::CreateConversationInput>>,
    }

    impl ScriptedProvider {
        fn always_failing() -> Arc<Self> {
            Arc::new(Self {
                failures_remaining: Mutex::new(i64::MAX),
                created: Mutex::new(Vec::new()),
            })
        }

        fn succeeding() -> Arc<Self> {
            Arc::new(Self {
                failures_remaining: Mutex::new(0),
                created: Mutex::new(Vec::new()),
            })
        }

        fn sends(&self) -> usize {
            self.created.lock().unwrap_or_else(|p| p.into_inner()).len()
        }
    }

    #[async_trait::async_trait]
    impl HelpScoutProvider for ScriptedProvider {
        fn kind(&self) -> &'static str {
            "scripted"
        }
        async fn get_me(&self) -> Result<HsUser> {
            Err(Error::Other("not needed".into()))
        }
        async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
            Ok(Vec::new())
        }
        async fn list_users(&self) -> Result<Vec<HsUser>> {
            Ok(Vec::new())
        }
        async fn list_teams(&self) -> Result<Vec<HsTeam>> {
            Ok(Vec::new())
        }
        async fn list_tags(&self) -> Result<Vec<HsTag>> {
            Ok(Vec::new())
        }
        async fn list_conversations(
            &self,
            _query: &ConversationQuery,
        ) -> Result<Page<crate::helpscout::HsConversation>> {
            Ok(Page {
                items: Vec::new(),
                next_cursor: None,
            })
        }
        async fn list_customers(
            &self,
            _query: &CustomerQuery,
        ) -> Result<Page<crate::helpscout::HsCustomer>> {
            Ok(Page {
                items: Vec::new(),
                next_cursor: None,
            })
        }
        async fn list_beacon_chats(&self) -> Result<Vec<HsBeaconChat>> {
            Ok(Vec::new())
        }
        async fn list_docs(&self) -> Result<Vec<HsDocArticle>> {
            Ok(Vec::new())
        }
        async fn list_ratings(&self) -> Result<Vec<HsRating>> {
            Ok(Vec::new())
        }
        async fn create_reply_thread(
            &self,
            _input: CreateThreadInput,
        ) -> Result<crate::helpscout::ThreadCreated> {
            Err(Error::Other("not needed".into()))
        }
        async fn create_note_thread(&self, _input: CreateThreadInput) -> Result<ThreadCreated> {
            Err(Error::Other("not needed".into()))
        }
        async fn update_conversation(
            &self,
            _conversation_id: i64,
            _patch: ConversationPatch,
        ) -> Result<bool> {
            Ok(false)
        }
        async fn create_conversation(
            &self,
            input: crate::helpscout::CreateConversationInput,
        ) -> Result<crate::helpscout::ConversationCreated> {
            let mut left = self
                .failures_remaining
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if *left > 0 {
                *left -= 1;
                return Err(Error::Other(Box::new(HsApiError {
                    status_code: 500,
                    message: "backend exploded".into(),
                    friendly: "Help Scout returned a server error".into(),
                    retryable: true,
                })));
            }
            drop(left);
            self.created
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(input);
            Ok(crate::helpscout::ConversationCreated {
                conversation_id: 778_001,
                number: 5252,
                thread_id: 9101,
            })
        }
    }

    fn db() -> DbHandle {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        DbHandle::new(conn)
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    /// Seeds a synced mailbox and a queued campaign with one ready
    /// recipient. Returns (campaign_id, recipient_id).
    fn seed_campaign(conn: &Connection) -> (i64, i64) {
        conn.execute(
            "INSERT INTO mailboxes (remote_id, name) VALUES (2024, 'Support')",
            [],
        )
        .expect("mailbox");
        let mailbox_local = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO outreach_campaigns (name, subject, body, mailbox_local_id, tags, status)
             VALUES ('spring check', 'Quick question', 'Hello!', ?1, '[]', 'queued')",
            params![mailbox_local],
        )
        .expect("campaign");
        let campaign = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO outreach_recipients
                 (campaign_id, customer_local_id, customer_remote_id, email, snapshot, state)
             VALUES (?1, 1, 5501, 'person@example.com', '{}', 'selected')",
            params![campaign],
        )
        .expect("recipient");
        let recipient = conn.last_insert_rowid();
        (campaign, recipient)
    }

    fn recipient_row(conn: &Connection, recipient: i64) -> (String, i64, Option<String>) {
        conn.query_row(
            "SELECT state, attempts, last_error FROM outreach_recipients WHERE id = ?1",
            params![recipient],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("recipient row")
    }

    #[test]
    fn retryable_failures_spend_attempts_and_terminate() {
        let db = db();
        let (campaign, recipient) = db.with_write(|c| seed_campaign(c));
        let provider: Arc<dyn crate::helpscout::HelpScoutProvider> =
            ScriptedProvider::always_failing();

        let mut last = serde_json::Value::Null;
        for _ in 0..5 {
            last = rt().block_on(async { send_batch(&db, &provider, campaign).await });
        }

        let (state, attempts, error) = db.with_read(|c| recipient_row(c, recipient));
        assert_eq!(
            attempts, 3,
            "exactly SEND_MAX_ATTEMPTS sends happen, then the recipient is parked: {last}"
        );
        assert_eq!(
            state, "failed",
            "the 429/5xx retry storm terminates at the cap instead of looping forever"
        );
        assert!(
            error.as_deref().unwrap_or("").contains("exhausted")
                || error.as_deref().unwrap_or("").contains("500"),
            "the failure is recorded honestly: {error:?} / {last}"
        );
    }

    #[test]
    fn a_fresh_claimed_row_is_invisible_to_a_second_pick() {
        let db = db();
        let (campaign, recipient) = db.with_write(|c| {
            let (campaign, recipient) = seed_campaign(c);
            // Simulate a crash AFTER the claim: the row is 'sending' with a
            // fresh claim timestamp, so neither pick_batch nor the stale
            // sweep may touch it, and no duplicate send may go out.
            c.execute(
                "UPDATE outreach_recipients SET state = 'sending',
                     updated_at = datetime('now')
                   WHERE id = ?1",
                params![recipient],
            )
            .expect("claim");
            (campaign, recipient)
        });
        let provider = ScriptedProvider::succeeding();
        let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
        rt().block_on(async { send_batch(&db, &sender, campaign).await });

        assert_eq!(
            provider.sends(),
            0,
            "a fresh 'sending' row is never re-picked"
        );
        let (state, _attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
        assert_eq!(state, "sending", "the in-flight claim is left alone");
    }

    #[test]
    fn a_stale_claimed_row_that_landed_is_recovered_as_sent() {
        let db = db();
        let (campaign, recipient) = db.with_write(|c| {
            let (campaign, recipient) = seed_campaign(c);
            c.execute(
                "INSERT INTO customers (remote_id, first_name, last_name, email)
                 VALUES (5501, 'Pat', 'Person', 'person@example.com')",
                [],
            )
            .expect("customer");
            let customer_local = c.last_insert_rowid();
            c.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status,
                     customer_local_id)
                 VALUES (99001, 99, 'Quick question', 'preview', 'active', ?1)",
                params![customer_local],
            )
            .expect("landed conversation");
            c.execute(
                "UPDATE outreach_recipients SET state = 'sending',
                     updated_at = datetime('now', '-11 minutes')
                   WHERE id = ?1",
                params![recipient],
            )
            .expect("stale claim");
            (campaign, recipient)
        });
        let provider = ScriptedProvider::succeeding();
        let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
        rt().block_on(async { send_batch(&db, &sender, campaign).await });

        assert_eq!(
            provider.sends(),
            0,
            "a crashed send that provably landed is marked sent, never re-sent"
        );
        let (state, _attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
        assert_eq!(state, "sent", "recovered from the platform mirror");
        let remote: i64 = db
            .with_read(|c| {
                c.query_row(
                    "SELECT hs_conversation_remote_id FROM outreach_recipients WHERE id = ?1",
                    params![recipient],
                    |r| r.get(0),
                )
            })
            .expect("recovered remote id");
        assert_eq!(remote, 99001, "the mirrored conversation id is recorded");
    }

    #[test]
    fn a_stale_claimed_row_that_never_landed_is_requeued_and_sent_once() {
        let db = db();
        let (campaign, recipient) = db.with_write(|c| {
            let (campaign, recipient) = seed_campaign(c);
            c.execute(
                "UPDATE outreach_recipients SET state = 'sending',
                     updated_at = datetime('now', '-11 minutes')
                   WHERE id = ?1",
                params![recipient],
            )
            .expect("stale claim");
            (campaign, recipient)
        });
        let provider = ScriptedProvider::succeeding();
        let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
        let summary = rt().block_on(async { send_batch(&db, &sender, campaign).await });

        assert_eq!(
            provider.sends(),
            1,
            "the genuinely undelivered row is requeued and sent exactly once: {summary}"
        );
        let (state, attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
        assert_eq!(state, "sent");
        assert_eq!(attempts, 1, "the recovery send spends an attempt");
    }

    #[test]
    fn a_reconciled_unknown_returns_to_the_queue_under_the_cap() {
        let db = db();
        let (campaign, recipient) = db.with_write(|c| {
            let (campaign, recipient) = seed_campaign(c);
            c.execute(
                "UPDATE outreach_recipients SET state = 'unknown', attempts = 0
                   WHERE id = ?1",
                params![recipient],
            )
            .expect("unknown");
            (campaign, recipient)
        });
        let provider = ScriptedProvider::succeeding();
        let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
        rt().block_on(async { send_batch(&db, &sender, campaign).await });

        let (state, _attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
        assert_eq!(
            state, "sent",
            "a provably-undelivered unknown returns to the queue and is sent (was 'failed')"
        );
    }

    #[test]
    fn a_reconciled_unknown_with_exhausted_attempts_fails_honestly() {
        let db = db();
        let (campaign, recipient) = db.with_write(|c| {
            let (campaign, recipient) = seed_campaign(c);
            c.execute(
                "UPDATE outreach_recipients SET state = 'unknown', attempts = 3
                   WHERE id = ?1",
                params![recipient],
            )
            .expect("unknown exhausted");
            (campaign, recipient)
        });
        let provider = ScriptedProvider::succeeding();
        let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
        rt().block_on(async { send_batch(&db, &sender, campaign).await });

        assert_eq!(provider.sends(), 0, "attempts exhausted: no blind re-send");
        let (state, attempts, error) = db.with_read(|c| recipient_row(c, recipient));
        assert_eq!(
            state, "failed",
            "the terminal state is honest, not a lingering 'unknown'"
        );
        assert_eq!(attempts, 3);
        assert!(
            error.as_deref().unwrap_or("").contains("exhausted"),
            "the reason is recorded: {error:?}"
        );
    }

    #[test]
    fn the_staleness_clock_column_is_ensured_by_the_batch_run() {
        let db = db();
        let (campaign, _recipient) = db.with_write(|c| seed_campaign(c));
        let provider: Arc<dyn crate::helpscout::HelpScoutProvider> = ScriptedProvider::succeeding();
        rt().block_on(async { send_batch(&db, &provider, campaign).await });

        let has_column: bool = db
            .with_read(|c| {
                c.query_row(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('outreach_recipients')
                      WHERE name = 'updated_at'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("pragma");
        assert!(has_column, "the claim/staleness clock column exists");
    }
}
