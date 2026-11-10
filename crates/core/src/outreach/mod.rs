use rusqlite::{Connection, params};
use serde::Serialize;
use std::sync::Arc;

use crate::error::Result;
use crate::migrations::add_column_if_missing;

mod send;
use send::ensure_recipient_updated_at;
pub(crate) use send::send_batch;

pub(crate) const M023_TO_M025_SQL: &str = r#"
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

pub(crate) fn apply_m023_to_m025(conn: &Connection) -> Result<()> {
    conn.execute_batch(M023_TO_M025_SQL)?;
    Ok(())
}

pub fn delete_segment(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM saved_segments WHERE id = ?1", params![id])?;
    Ok(rows > 0)
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

pub(crate) const PERSONALIZATION_VARIABLES: [&str; 6] = [
    "first_name",
    "last_name",
    "company",
    "organization",
    "last_ticket_number",
    "last_ticket_subject",
];

const MAX_ATTEMPTS: i64 = 3;

pub(crate) fn apply_m031(conn: &Connection) -> Result<()> {
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
pub(crate) struct CampaignDetail {
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

pub(crate) fn get_outreach_campaign(conn: &Connection, id: i64) -> Result<Option<CampaignDetail>> {
    let mut stmt = conn.prepare(&format!("{CAMPAIGN_SELECT} WHERE oc.id = ?1"))?;
    let mut rows = stmt.query_map([id], map_campaign_row)?;
    Ok(rows.next().transpose()?)
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

pub(crate) fn render_template(text: &str, ctx: &serde_json::Value) -> (String, Vec<String>) {
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

pub(crate) fn render_for(
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

pub(crate) fn campaign_validate(conn: &Connection, campaign_id: i64) -> serde_json::Value {
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

fn reconcile_recipient_states(conn: &Connection, campaign_id: i64) -> (i64, i64, i64) {
    if let Err(e) = ensure_recipient_updated_at(conn) {
        log_event(
            conn,
            campaign_id,
            None,
            "recipient_schema_ensure_failed",
            Some(&e.to_string()),
        );
    }
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

pub(crate) fn refresh_replies(conn: &Connection, campaign_id: i64) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let sent: Vec<(i64, Option<i64>)> = {
        let mut stmt = tx.prepare(
            "SELECT r.id, r.hs_conversation_remote_id
               FROM outreach_recipients r
              WHERE r.campaign_id = ?1 AND r.state = 'sent'
                AND r.replied_at IS NULL
                AND r.hs_conversation_remote_id IS NOT NULL",
        )?;

        stmt.query_map([campaign_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };
    let mut replied = 0i64;
    for (id, remote_id) in sent {
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
        let campaign_thread: Option<(i64, String)> = tx
            .query_row(
                "SELECT id, created_at FROM conversation_threads
                  WHERE conversation_id = ?1 AND deleted_at IS NULL
                  ORDER BY created_at ASC, id ASC LIMIT 1",
                [conv_local],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    ))
                },
            )
            .ok();
        let Some((campaign_thread_id, campaign_at)) = campaign_thread else {
            continue;
        };
        let customer_thread: Option<String> = tx
            .query_row(
                "SELECT created_at FROM conversation_threads
                  WHERE conversation_id = ?1 AND type = 'customer'
                    AND deleted_at IS NULL
                    AND (julianday(created_at) > julianday(?2)
                         OR (julianday(created_at) = julianday(?2) AND id > ?3))
                  ORDER BY created_at ASC, id ASC LIMIT 1",
                rusqlite::params![conv_local, campaign_at, campaign_thread_id],
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

pub(crate) fn campaign_report_not_found() -> serde_json::Value {
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

pub(crate) fn ensure_segments_v2(conn: &Connection) -> Result<()> {
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

// AUD-P2-13 (C9): the campaign edit lane — content edits (name/subject/
// body/mailbox/tags) are safe only while the campaign is still a draft
// (recipients are snapshotted at queue time; a queued campaign's sends
// must not change under it). Non-draft edits are refused honestly.
pub fn update_outreach_campaign(
    conn: &Connection,
    campaign_id: i64,
    name: &str,
    subject: &str,
    body: &str,
    mailbox_local_id: i64,
    tags: &[String],
) -> Result<()> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM outreach_campaigns WHERE id = ?1",
            params![campaign_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let Some(status) = status else {
        return Err(crate::error::Error::Config(
            "campaign not found".to_string(),
        ));
    };
    if status != "draft" {
        return Err(crate::error::Error::Config(format!(
            "campaign is {status} — only draft campaigns can be edited (pause/cancel/retry apply after queueing)"
        )));
    }
    if name.trim().is_empty() || subject.trim().is_empty() || body.trim().is_empty() {
        return Err(crate::error::Error::Config(
            "name, subject and body are all required".to_string(),
        ));
    }
    let mailbox_ok: bool = conn
        .query_row(
            "SELECT 1 FROM mailboxes WHERE id = ?1 AND deleted_at IS NULL",
            params![mailbox_local_id],
            |_| Ok(()),
        )
        .is_ok();
    if !mailbox_ok {
        return Err(crate::error::Error::Config(format!(
            "mailbox {mailbox_local_id} does not exist"
        )));
    }
    conn.execute(
        "UPDATE outreach_campaigns
            SET name = ?1, subject = ?2, body = ?3, mailbox_local_id = ?4, tags = ?5,
                updated_at = datetime('now')
          WHERE id = ?6 AND status = 'draft'",
        params![
            name.trim(),
            subject.trim(),
            body,
            mailbox_local_id,
            serde_json::to_string(tags).unwrap_or_default(),
            campaign_id
        ],
    )?;
    log_event(
        conn,
        campaign_id,
        None,
        "campaign_edited",
        Some("draft content updated"),
    );
    Ok(())
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

pub(crate) const SEND_BATCH_SIZE: i64 = 5;

pub(crate) const SEND_MAX_ATTEMPTS: i64 = 3;

struct BatchRow {
    recipient_id: i64,
    customer_local_id: i64,
    customer_remote_id: Option<i64>,
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
        let campaign = db.with_write(|c| {
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

#[cfg(test)]
mod batch31_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    #[test]
    fn refresh_replies_ignores_the_campaigns_own_message_under_clock_skew() {
        let conn = db();
        conn.execute(
            "INSERT INTO outreach_campaigns (name, subject, body, status)
             VALUES ('skew', 'Hi', 'Body', 'sending')",
            [],
        )
        .expect("campaign");
        let campaign: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (1, 'Casey')",
            [],
        )
        .expect("customer");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (500, 500, 'campaign conv', 'p', 'active')",
            [],
        )
        .expect("conversation");
        let conv: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO outreach_recipients
                 (campaign_id, customer_local_id, customer_remote_id, email, state,
                  hs_conversation_remote_id, sent_at)
             VALUES (?1, 1, 1, 'c@example.com', 'sent', 500, '2026-10-24 10:00:00')",
            params![campaign],
        )
        .expect("recipient");
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
             VALUES (?1, 'customer', 'published', 'the campaign email itself',
                     '2026-10-24T10:00:01.000Z')",
            [conv],
        )
        .expect("campaign thread");
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
             VALUES (?1, 'customer', 'published', 'thanks — where do I find it?',
                     '2026-10-24T11:00:00.000Z')",
            [conv],
        )
        .expect("reply thread");

        let replied = refresh_replies(&conn, campaign).expect("refresh");
        assert_eq!(replied, 1, "exactly one real reply");
        let replied_at: String = conn
            .query_row(
                "SELECT replied_at FROM outreach_recipients WHERE campaign_id = ?1",
                [campaign],
                |r| r.get(0),
            )
            .expect("replied_at");
        assert_eq!(
            replied_at, "2026-10-24T11:00:00.000Z",
            "the reply must be the LATER thread, not the campaign's own message"
        );
    }

    #[test]
    fn refresh_replies_counts_nothing_when_only_the_campaign_message_exists() {
        let conn = db();
        conn.execute(
            "INSERT INTO outreach_campaigns (name, subject, body, status)
             VALUES ('skew2', 'Hi', 'Body', 'sending')",
            [],
        )
        .expect("campaign");
        let campaign: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (1, 'Casey')",
            [],
        )
        .expect("customer");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (501, 501, 'campaign conv', 'p', 'active')",
            [],
        )
        .expect("conversation");
        let conv: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO outreach_recipients
                 (campaign_id, customer_local_id, customer_remote_id, email, state,
                  hs_conversation_remote_id, sent_at)
             VALUES (?1, 1, 1, 'c@example.com', 'sent', 501, '2026-10-24 10:00:00')",
            params![campaign],
        )
        .expect("recipient");
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text, created_at)
             VALUES (?1, 'customer', 'published', 'the campaign email itself',
                     '2026-10-24T10:00:01.000Z')",
            [conv],
        )
        .expect("campaign thread");

        let replied = refresh_replies(&conn, campaign).expect("refresh");
        assert_eq!(replied, 0, "the campaign's own message is not a reply");
        let replied_at: Option<String> = conn
            .query_row(
                "SELECT replied_at FROM outreach_recipients WHERE campaign_id = ?1",
                [campaign],
                |r| r.get(0),
            )
            .expect("replied_at");
        assert!(replied_at.is_none());
    }
}

// AUD-P2-13 (C9): the draft-guarded campaign edit lane.
#[cfg(test)]
mod audp2_tests {
    use super::*;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut c).expect("migrations");
        c
    }

    fn seed(c: &Connection) -> i64 {
        c.execute(
            "INSERT INTO mailboxes (remote_id, name) VALUES (1, 'Support')",
            [],
        )
        .expect("mailbox");
        let mailbox = c.last_insert_rowid();
        create_outreach_campaign(c, "Check-in", "Hi", "Body", mailbox, &[], None, None, &[])
            .expect("campaign")
    }

    #[test]
    fn audp2_campaign_edit_round_trips_on_drafts_and_refuses_after_queue() {
        let c = conn();
        let campaign = seed(&c);

        update_outreach_campaign(
            &c,
            campaign,
            "Check-in v2",
            "Hello",
            "New body",
            1,
            &["tag".to_string()],
        )
        .expect("draft edit");
        let (name, subject, tags): (String, String, String) = c
            .query_row(
                "SELECT name, subject, tags FROM outreach_campaigns WHERE id = ?1",
                params![campaign],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("row");
        assert_eq!(name, "Check-in v2");
        assert_eq!(subject, "Hello");
        assert!(tags.contains("tag"), "{tags}");

        // Status guard: a queued campaign refuses with the honest message.
        c.execute(
            "UPDATE outreach_campaigns SET status = 'queued' WHERE id = ?1",
            params![campaign],
        )
        .expect("queue");
        let err = update_outreach_campaign(&c, campaign, "X", "Y", "Z", 1, &[]).expect_err("guard");
        assert!(
            err.to_string().contains("only draft campaigns"),
            "the refusal explains the rule: {err}"
        );

        // Blank content is refused.
        c.execute(
            "UPDATE outreach_campaigns SET status = 'draft' WHERE id = ?1",
            params![campaign],
        )
        .expect("redraft");
        let err = update_outreach_campaign(&c, campaign, "", "Y", "Z", 1, &[]).expect_err("blank");
        assert!(err.to_string().contains("required"), "{err}");

        // Unknown mailbox is refused.
        let err =
            update_outreach_campaign(&c, campaign, "N", "S", "B", 999, &[]).expect_err("mailbox");
        assert!(err.to_string().contains("does not exist"), "{err}");

        // Unknown campaign id refuses.
        let err = update_outreach_campaign(&c, 999, "N", "S", "B", 1, &[]).expect_err("missing");
        assert!(err.to_string().contains("not found"), "{err}");
    }
}
