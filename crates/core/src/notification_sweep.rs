use rusqlite::{Connection, params};

use crate::catalog::NotificationType;
use crate::error::Result;
use crate::mentions::{build_mention_directory, parse_mentions};
use crate::notifications::{NotificationInput, record_notification};

pub(crate) const CURSOR_KEY: &str = "notif_event_cursor";

pub(crate) const SWEEP_AT_KEY: &str = "notif_sweep_at";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SweepOutcome {
    pub created: u32,
}

pub(crate) fn first_sync_settled(conn: &Connection) -> Result<bool> {
    let state = crate::sync_engine::get_state(conn);
    Ok(!matches!(
        state.as_str(),
        "NEW" | "INITIALIZING" | "BACKFILLING"
    ))
}

type SweepRow = (i64, String, String, Option<String>, Option<String>);

pub fn sweep(conn: &Connection, bus: Option<&crate::event_bus::EventBus>) -> Result<SweepOutcome> {
    let now = now_stamp();
    let cursor_raw = crate::settings::get_i64(conn, CURSOR_KEY, -1)?;
    let sweep_at = crate::settings::get_string(conn, SWEEP_AT_KEY)?;

    let (cursor, sweep_at) = match (cursor_raw >= 0, sweep_at) {
        (true, Some(at)) => (cursor_raw, at),
        _ => {
            if !first_sync_settled(conn)? {
                return Ok(SweepOutcome::default());
            }
            crate::settings::set_i64(conn, CURSOR_KEY, max_event_id(conn))?;
            crate::settings::set_string(conn, SWEEP_AT_KEY, &now)?;
            return Ok(SweepOutcome::default());
        }
    };

    let (created, processed_to) = run_sub_sweeps(conn, bus, cursor, &sweep_at)?;

    let ceiling = max_event_id(conn);
    let next_cursor = match processed_to {
        Some(cap) => cap.min(ceiling).max(cursor),
        None => ceiling.max(cursor),
    };
    crate::settings::set_i64(conn, CURSOR_KEY, next_cursor)?;
    crate::settings::set_string(conn, SWEEP_AT_KEY, &now)?;

    Ok(SweepOutcome { created })
}

type SubSweepOutcome = (u32, Option<i64>);

fn run_sub_sweeps(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    cursor: i64,
    sweep_at: &str,
) -> Result<SubSweepOutcome> {
    let mut created = 0u32;
    let (event_created, processed_to) = sweep_conversation_events(conn, bus, cursor)?;
    created += event_created;
    created += sweep_sla(conn, bus)?;
    created += sweep_automation_approvals(conn, bus)?;
    created += sweep_failed_jobs(conn, bus, sweep_at)?;
    created += sweep_sync_state(conn, bus)?;
    created += sweep_campaign_replies(conn, bus, sweep_at)?;
    created += sweep_known_issues(conn, bus, sweep_at)?;
    created += sweep_issue_spikes(conn, bus, sweep_at)?;
    created += sweep_ai_escalations(conn, bus, sweep_at)?;
    created += sweep_ratings(conn, bus, sweep_at)?;
    Ok((created, processed_to))
}

struct EventRow {
    conversation_id: i64,
    thread_local_id: Option<i64>,
    event_type: String,
    actor_local_id: Option<i64>,
    metadata: Option<String>,
    dedup_key: String,
}

struct ConversationFacts {
    id: i64,
    number: Option<i64>,
    subject: Option<String>,
    assignee_local_id: Option<i64>,
    customer_local_id: Option<i64>,
}

fn load_conversation(conn: &Connection, id_or_remote: i64) -> Result<Option<ConversationFacts>> {
    let row = conn
        .query_row(
            "SELECT id, number, subject, assignee_local_id, customer_local_id
             FROM conversations
             WHERE id = ?1 OR remote_id = ?1
             ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END
             LIMIT 1",
            params![id_or_remote],
            |r| {
                Ok(ConversationFacts {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    subject: r.get(2)?,
                    assignee_local_id: r.get(3)?,
                    customer_local_id: r.get(4)?,
                })
            },
        )
        .ok();
    Ok(row)
}

const EVENT_SWEEP_CAP: usize = 500;

fn sweep_conversation_events(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    after_event_id: i64,
) -> Result<(u32, Option<i64>)> {
    let mut events: Vec<(i64, EventRow)> = {
        let mut stmt = conn.prepare(
            "SELECT e.id, e.conversation_id, e.thread_local_id, e.event_type,
                    e.actor_id, e.metadata, e.dedup_key
             FROM activity_events e
             WHERE e.id > ?1
               AND e.event_type IN ('customer_message', 'internal_note', 'assignment_changed')
             ORDER BY e.id LIMIT ?2",
        )?;

        stmt.query_map(params![after_event_id, (EVENT_SWEEP_CAP + 1) as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                EventRow {
                    conversation_id: r.get(1)?,
                    thread_local_id: r.get(2)?,
                    event_type: r.get(3)?,
                    actor_local_id: r.get(4)?,
                    metadata: r.get(5)?,
                    dedup_key: r.get(6)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    if events.is_empty() {
        return Ok((0, None));
    }
    let hit_cap = events.len() > EVENT_SWEEP_CAP;
    if hit_cap {
        events.truncate(EVENT_SWEEP_CAP);
    }
    let processed_to = if hit_cap {
        Some(
            events
                .last()
                .map(|(id, _)| *id)
                .expect("hit_cap implies at least one row"),
        )
    } else {
        None
    };
    let mut created = 0u32;
    for (_, e) in &events {
        let Some(conv) = load_conversation(conn, e.conversation_id)? else {
            continue;
        };
        let conv_id = conv.id;
        let number = conv.number;
        match e.event_type.as_str() {
            "customer_message" => {
                let excerpt = thread_excerpt(conn, e.thread_local_id)?;
                let subject_suffix = conv
                    .subject
                    .as_deref()
                    .map(|s| format!(" — {}", trim(s, 60)))
                    .unwrap_or_default();
                let r = record_notification(
                    conn,
                    bus,
                    &NotificationInput {
                        notification_type: NotificationType::CustomerReplied,
                        title: format!(
                            "Customer replied on #{}{}",
                            number.unwrap_or(conv_id),
                            subject_suffix
                        ),
                        body: excerpt,
                        target_user_local_id: conv.assignee_local_id,
                        conversation_id: Some(conv_id),
                        conversation_number: number,
                        customer_local_id: conv.customer_local_id,
                        dedup_key: format!("n:evt:customer_replied:{}", e.dedup_key),
                        ..Default::default()
                    },
                )?;
                if r.is_some() {
                    created += 1;
                }
            }
            "assignment_changed" => {
                let meta = safe_json_object(e.metadata.as_deref());
                let next = meta.get("next").and_then(|v| v.as_i64());
                if let Some(next) = next
                    && Some(next) != e.actor_local_id
                {
                    let name = user_display(conn, Some(next));
                    let r = record_notification(
                        conn,
                        bus,
                        &NotificationInput {
                            notification_type: NotificationType::TicketAssigned,
                            title: format!("#{} assigned to {name}", number.unwrap_or(conv_id)),
                            body: conv.subject.as_deref().map(|s| trim(s, 120)),
                            target_user_local_id: Some(next),
                            actor_user_local_id: e.actor_local_id,
                            conversation_id: Some(conv_id),
                            conversation_number: number,
                            dedup_key: format!("n:evt:ticket_assigned:{}", e.dedup_key),
                            ..Default::default()
                        },
                    )?;
                    if r.is_some() {
                        created += 1;
                    }
                }
            }
            "internal_note" => {
                let Some(body) = thread_body(conn, e.thread_local_id)? else {
                    continue;
                };
                let directory = build_mention_directory(conn)?;
                let mentions = parse_mentions(&body, &directory)?;
                let author = e.actor_local_id;
                for m in mentions {
                    let Some(user) = m.user_local_id else {
                        continue;
                    };
                    if Some(user) == author {
                        continue;
                    }
                    let r = record_notification(
                        conn,
                        bus,
                        &NotificationInput {
                            notification_type: NotificationType::Mentioned,
                            title: format!(
                                "{} mentioned you on #{}",
                                user_display(conn, author),
                                number.unwrap_or(conv_id)
                            ),
                            body: Some(trim(&body, 200)),
                            target_user_local_id: Some(user),
                            actor_user_local_id: author,
                            conversation_id: Some(conv_id),
                            conversation_number: number,
                            dedup_key: format!("n:mention:{}:{user}", e.dedup_key),
                            ..Default::default()
                        },
                    )?;
                    if r.is_some() {
                        created += 1;
                    }
                }
            }
            _ => {}
        }
    }
    Ok((created, processed_to))
}

fn sweep_sla(conn: &Connection, bus: Option<&crate::event_bus::EventBus>) -> Result<u32> {
    let mut created = 0u32;
    let alerts = crate::sla::sla_alerts(conn)?;
    let day = day_stamp();
    for a in &alerts.alerts {
        let breached = a.state == crate::sla::SlaAlertState::Breached;
        let notif_type = if breached {
            NotificationType::SlaBreach
        } else {
            NotificationType::SlaRisk
        };
        let title = if breached {
            format!(
                "SLA breached on #{} ({}) — {} min over target",
                a.number, a.mailbox_name, a.overdue_business_min
            )
        } else {
            format!(
                "SLA at risk on #{} ({}) — {}/{} business min",
                a.number, a.mailbox_name, a.waited_business_min, a.target_min
            )
        };
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: notif_type,
                severity: Some(if breached { "critical" } else { "warning" }),
                title,
                body: a.subject.as_deref().map(|s| trim(s, 120)),
                target_user_local_id: a.assignee_local_id,
                conversation_id: Some(a.conversation_id),
                conversation_number: Some(a.number),
                dedup_key: format!("n:sla:{}:{}:{day}", a.state.as_str(), a.conversation_id),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

fn sweep_automation_approvals(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
) -> Result<u32> {
    let rows: Vec<(i64, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, payload FROM jobs
             WHERE type = 'automation_action_awaiting_approval'
               AND status IN ('queued', 'awaiting_approval')
             ORDER BY id LIMIT 100",
        )?;

        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (job_id, payload) in rows {
        let payload = safe_json_object(payload.as_deref());
        let conversation_id = payload.get("conversationId").and_then(|v| v.as_i64());
        let conv = conversation_id.and_then(|id| load_conversation(conn, id).ok().flatten());
        let action = payload
            .get("action")
            .and_then(|a| a.get("kind"))
            .and_then(|k| k.as_str())
            .unwrap_or("action")
            .to_string();
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::AutomationApproval,
                severity: Some("warning"),
                title: format!(
                    "Automation approval required: {action}{}",
                    conv.as_ref()
                        .and_then(|c| c.number)
                        .map(|n| format!(" on #{n}"))
                        .unwrap_or_default()
                ),
                body: conv
                    .as_ref()
                    .and_then(|c| c.subject.as_deref())
                    .map(|s| trim(s, 120)),
                conversation_id: conv.as_ref().map(|c| c.id),
                conversation_number: conv.as_ref().and_then(|c| c.number),
                job_id: Some(job_id),
                dedup_key: format!("n:approval:{job_id}"),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

/// UX-35: friendly, user-facing label for an internal job type — no more
/// snake_case identifiers (`ai`, `embed_conversation_chunks`) in
/// notification titles. Unknown types degrade to a cleaned-up phrase.
fn friendly_job_type(job_type: &str) -> String {
    match job_type {
        "ai" | "analyze_ticket" | "generate_draft" | "create_ai_note" | "review_draft"
        | "qa_ai_layer" | "rewrite_composer" => "AI task".to_string(),
        "sync" => "Help Scout sync".to_string(),
        "embeddings" | "embed_conversation_chunks" | "embed_knowledge_chunks" => {
            "knowledge embeddings".to_string()
        }
        "cluster_issues" => "issue clustering".to_string(),
        "draft_article" => "article drafting".to_string(),
        "golden_eval" => "AI quality evaluation".to_string(),
        "rebuild_activity" | "rebuild_gaps" => "report rebuild".to_string(),
        "saved_reply_write" => "saved reply write".to_string(),
        "run_workflow" => "automation workflow".to_string(),
        "webhook_health_check" => "webhook health check".to_string(),
        "automation_action_awaiting_approval" => "automation approval".to_string(),
        "send_chat_message" => "live chat message".to_string(),
        "outreach_send_batch" => "campaign send".to_string(),
        "download_recent_attachments" => "attachment downloads".to_string(),
        other => {
            let cleaned = other.replace('_', " ");
            match cleaned.char_indices().next() {
                Some((0, first)) => format!("{}{}", first.to_uppercase(), &cleaned[1..]),
                _ => cleaned,
            }
        }
    }
}

/// UX-35: pull the `host:port` out of a reqwest-style error like
/// `… error sending request for url (http://localhost:1234/v1/chat/completions)`.
fn extract_host(text: &str) -> Option<String> {
    let start = text.find("url (").map(|i| i + "url (".len())?;
    let rest = &text[start..];
    let end = rest.find([')', ' ']).unwrap_or(rest.len());
    let url = &rest[..end];
    let after_scheme = url.split_once("://").map_or(url, |x| x.1);
    let host = after_scheme.split('/').next().unwrap_or(after_scheme);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// UX-35: classify a raw job error into a stable machine-readable `code`
/// plus a friendly sentence for the notification body. The raw string is
/// preserved verbatim in the structured payload (hover / Diagnostics).
fn classify_job_error(raw: &str) -> (&'static str, String) {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("connection refused")
        || lower.contains("error sending request")
        || lower.contains("failed to lookup")
        || lower.contains("dns")
        || lower.contains("timed out")
        || lower.contains("timeout")
    {
        let host = extract_host(raw);
        let at = host
            .as_deref()
            .map_or_else(String::new, |h| format!(" at {h}"));
        let is_local = host
            .as_deref()
            .is_some_and(|h| h.starts_with("localhost") || h.starts_with("127.0.0.1"));
        let hint = if is_local {
            " If this is the AI assistant, check that LM Studio (or your OpenAI-compatible server) is running and that its address in Settings is correct."
        } else {
            ""
        };
        return (
            "unreachable",
            format!(
                "A background task could not reach the service it needs{at} — \
                 the connection was refused, timed out, or the address does not \
                 resolve.{hint}"
            ),
        );
    }
    if lower.contains("unauthorized")
        || lower.contains("401")
        || lower.contains("403")
        || lower.contains("api key")
        || lower.contains("credentials")
    {
        return (
            "auth",
            "A background task was rejected because its credentials were not \
             accepted — check the connected accounts and API keys in Settings."
                .to_string(),
        );
    }
    if lower.contains("429") || lower.contains("rate limit") {
        return (
            "rate_limited",
            "A background task was rate-limited by the remote service until it \
             gave up — retry later from Diagnostics."
                .to_string(),
        );
    }
    if lower.contains("parse failed")
        || lower.contains("deserialize")
        || lower.contains("json")
        || lower.contains("unexpected response")
    {
        return (
            "bad_response",
            "A background task got a response from the remote service that it \
             could not read — the service may be misbehaving."
                .to_string(),
        );
    }
    if lower.contains("missing")
        || lower.contains("unknown job type")
        || lower.contains("not configured")
    {
        return (
            "misconfigured",
            "A background task could not run because it was set up incorrectly \
             (an internal misconfiguration, not something you did)."
                .to_string(),
        );
    }
    (
        "unknown",
        "A background task failed after exhausting its retries.".to_string(),
    )
}

fn sweep_failed_jobs(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    let rows: Vec<SweepRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, queue, type, error, completed_at FROM jobs
             WHERE status = 'failed'
               AND datetime(completed_at) > datetime(?1)
             ORDER BY id DESC LIMIT 200",
        )?;

        stmt.query_map([since_stamp], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (job_id, queue, job_type, error, completed_at) in rows {
        let Some(completed) = completed_at else {
            continue;
        };
        let _ = completed;
        // UX-35: the notification is a user-facing surface — the title names
        // the job in plain language, the body is a friendly classification,
        // and the verbatim error moves into the structured payload (shown on
        // hover, kept for Diagnostics) instead of the body.
        let raw = error.unwrap_or_default();
        let (code, friendly) = classify_job_error(&raw);
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::JobFailure,
                severity: Some("warning"),
                title: format!("Background job failed: {}", friendly_job_type(&job_type)),
                body: Some(format!("{friendly} (Hover for the technical detail.)")),
                payload: Some(
                    serde_json::json!({
                        "code": code,
                        "raw": raw,
                        "queue": queue,
                        "job_type": job_type,
                    })
                    .to_string(),
                ),
                job_id: Some(job_id),
                dedup_key: format!("n:jobfail:{job_id}"),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

fn sweep_sync_state(conn: &Connection, bus: Option<&crate::event_bus::EventBus>) -> Result<u32> {
    let state = crate::sync_engine::get_state(conn);
    if state != "ERROR" {
        return Ok(0);
    }
    let r = record_notification(
        conn,
        bus,
        &NotificationInput {
            notification_type: NotificationType::SyncFailure,
            severity: Some("critical"),
            title: "Help Scout sync is in ERROR state".into(),
            body: Some("Open Sync Health to see the last errors and restart the sync.".into()),
            dedup_key: format!("n:syncfail:{}", day_stamp()),
            ..Default::default()
        },
    )?;
    Ok(u32::from(r.is_some()))
}

fn sweep_campaign_replies(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    type SweepRows0 = (
        i64,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<SweepRows0> = {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.campaign_id, r.customer_local_id, r.replied_at,
                    c.name, c.subject
             FROM outreach_recipients r JOIN outreach_campaigns c ON c.id = r.campaign_id
             WHERE r.replied_at IS NOT NULL
               AND datetime(r.replied_at) > datetime(?1)
             ORDER BY r.id DESC LIMIT 200",
        )?;

        stmt.query_map([since_stamp], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (id, campaign_id, customer_local_id, replied_at, name, subject) in rows {
        let Some(_stamp) = replied_at else {
            continue;
        };
        let customer = customer_display(conn, customer_local_id);
        let campaign_name = name.as_deref().unwrap_or("campaign");
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::CampaignReply,
                title: format!(
                    "Campaign reply: {} answered \"{}\"",
                    customer.as_deref().unwrap_or("a customer"),
                    trim(campaign_name, 50)
                ),
                body: subject.as_deref().map(|s| trim(s, 120)),
                customer_local_id,
                campaign_id,
                dedup_key: format!("n:campreply:{id}"),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

fn sweep_known_issues(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    type SweepRows1 = (
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<SweepRows1> = {
        let mut stmt = conn.prepare(
            "SELECT id, title, name, status, created_at FROM known_issues
             WHERE datetime(created_at) > datetime(?1)
             ORDER BY id DESC LIMIT 100",
        )?;

        stmt.query_map([since_stamp], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (id, title, name, status, created_at) in rows {
        let _ = created_at;
        let display = title.or(name).unwrap_or_default();
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::KnownIssueDetected,
                title: format!("Known issue tracked: {}", trim(&display, 80)),
                body: Some(format!(
                    "Status: {}",
                    status.as_deref().unwrap_or("investigating")
                )),
                issue_id: Some(id),
                dedup_key: format!("n:knownissue:{id}"),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

fn sweep_issue_spikes(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    type SweepRows2 = (
        i64,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
    );
    let rows: Vec<SweepRows2> = {
        let mut stmt = conn.prepare(
            "SELECT id, title, name, conversation_count, updated_at FROM issue_clusters
             WHERE trend = 'rising'
               AND datetime(updated_at) > datetime(?1)
             ORDER BY id DESC LIMIT 100",
        )?;

        stmt.query_map([since_stamp], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    let day = day_stamp();
    for (id, title, name, conversation_count, updated_at) in rows {
        let _ = updated_at;
        let display = title.or(name).unwrap_or_default();
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::IssueSpike,
                severity: Some("warning"),
                title: format!(
                    "Issue spike: {} ({} conversations)",
                    trim(&display, 80),
                    conversation_count.unwrap_or(0)
                ),
                issue_id: Some(id),
                dedup_key: format!("n:spike:{id}:{day}"),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

fn sweep_ai_escalations(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    type SweepRows3 = (
        i64,
        i64,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<SweepRows3> = {
        let mut stmt = conn.prepare(
            "SELECT a.id, a.conversation_id, a.response_json, a.created_at,
                    c.number, c.subject, c.status
             FROM ai_runs a JOIN conversations c ON c.id = a.conversation_id
             WHERE a.type = 'ticket_analysis' AND a.status = 'completed'
               AND a.conversation_id IS NOT NULL
               AND datetime(a.created_at) > datetime(?1)
             ORDER BY a.id DESC LIMIT 200",
        )?;

        stmt.query_map([since_stamp], |r| {
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
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (run_id, conversation_id, output, created_at, number, subject, status) in rows {
        let _ = created_at;
        if status.as_deref() == Some("closed") {
            continue;
        }
        let analysis = safe_json_object(output.as_deref());
        let urgency = analysis.get("urgency").and_then(|v| v.as_str());
        let sentiment = analysis.get("sentiment").and_then(|v| v.as_str());
        let confidence = analysis
            .get("confidence")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let escalate = (matches!(urgency, Some("high") | Some("critical"))
            || sentiment == Some("frustrated"))
            && matches!(confidence, "medium" | "high");
        if !escalate {
            continue;
        }
        let urgency_suffix = match urgency {
            Some("critical") => " — urgency: critical",
            Some("high") => " — urgency: high",
            _ => " — customer frustrated",
        };
        let body = [
            subject.as_deref().map(|s| trim(s, 100)),
            Some(format!("AI confidence: {confidence}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::AiEscalation,
                severity: Some("warning"),
                title: format!(
                    "AI escalation on #{}{urgency_suffix}",
                    number.unwrap_or(conversation_id)
                ),
                body: if body.is_empty() { None } else { Some(body) },
                conversation_id: Some(conversation_id),
                conversation_number: number,
                dedup_key: format!("n:aiescal:{run_id}"),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

fn sweep_ratings(
    conn: &Connection,
    bus: Option<&crate::event_bus::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    type RatingRow = (
        i64,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<String>,
    );
    let rows: Vec<RatingRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, remote_id, customer_local_id, comments, conversation_id, remote_created_at
             FROM ratings
             WHERE rating = 'not-good'
               AND datetime(remote_created_at) > datetime(?1)
             ORDER BY id DESC LIMIT 100",
        )?;

        stmt.query_map([since_stamp], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (id, remote_id, customer_local_id, comments, conversation_id, _at) in rows {
        let customer = customer_display(conn, customer_local_id);
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::CustomerEvent,
                title: format!(
                    "Not-good rating received{}",
                    customer
                        .as_deref()
                        .map(|c| format!(" from {c}"))
                        .unwrap_or_default()
                ),
                body: comments.as_deref().map(|c| trim(c, 200)),
                customer_local_id,
                conversation_id,
                dedup_key: format!(
                    "n:rating:{}",
                    remote_id.map_or_else(|| format!("id{id}"), |r| r.to_string())
                ),
                ..Default::default()
            },
        )?;
        if r.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

fn max_event_id(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COALESCE(MAX(id), 0) AS m FROM activity_events",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn now_stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn day_stamp() -> String {
    chrono::Utc::now().format("%Y%m%d").to_string()
}

fn thread_body(conn: &Connection, thread_local_id: Option<i64>) -> Result<Option<String>> {
    let Some(id) = thread_local_id else {
        return Ok(None);
    };
    let body: Option<String> = conn
        .query_row(
            "SELECT body_text FROM conversation_threads WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    Ok(body)
}

fn thread_excerpt(conn: &Connection, thread_local_id: Option<i64>) -> Result<Option<String>> {
    Ok(thread_body(conn, thread_local_id)?.map(|b| trim(&b, 160)))
}

fn user_display(conn: &Connection, user_local_id: Option<i64>) -> String {
    let Some(id) = user_local_id else {
        return "Someone".into();
    };
    let row: Option<(Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT first_name, last_name, email FROM users WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    match row {
        None => format!("user #{id}"),
        Some((first, last, email)) => {
            let name = [first, last]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if !name.is_empty() {
                name
            } else {
                email.unwrap_or_else(|| format!("user #{id}"))
            }
        }
    }
}

fn customer_display(conn: &Connection, customer_local_id: Option<i64>) -> Option<String> {
    let id = customer_local_id?;
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT first_name, last_name FROM customers WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    row.and_then(|(first, last)| {
        let name = [first, last]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        (!name.is_empty()).then_some(name)
    })
}

fn safe_json_object(v: Option<&str>) -> serde_json::Value {
    v.and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null)
        .as_object()
        .cloned()
        .map(serde_json::Value::Object)
        .unwrap_or(serde_json::json!({}))
}

fn trim(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() > max {
        let cut: String = t.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod batch25_item65_tests {
    use super::*;

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::sync_engine::set_state(&conn, "LIVE");
        conn
    }

    fn seed_backlog(conn: &Connection, count: usize) -> i64 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (501, 51, 'Backlog', 'p', 'active')",
            [],
        )
        .expect("conversation");
        for i in 0..count {
            conn.execute(
                "INSERT INTO activity_events
                    (conversation_id, event_type, actor_type, occurred_at, dedup_key,
                     thread_local_id, source, metadata)
                 VALUES (1, 'customer_message', 'customer', '2026-10-23T00:00:00Z', ?1,
                         NULL, 'test', NULL)",
                params![format!("evt-{i:04}")],
            )
            .expect("event");
        }
        conn.query_row("SELECT MAX(id) FROM activity_events", [], |r| r.get(0))
            .expect("max id")
    }

    fn notification_exists(conn: &Connection, dedup: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE dedup_key = ?1",
            params![dedup],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
    }

    #[test]
    fn a_backlog_beyond_the_cap_is_not_skipped_forever() {
        let conn = conn();
        let max_id = seed_backlog(&conn, 502);
        crate::settings::set_i64(&conn, CURSOR_KEY, 0).expect("cursor seeded");
        crate::settings::set_string(&conn, SWEEP_AT_KEY, "2026-10-23 00:00:00")
            .expect("sweep_at seeded");
        let id_of_500th: i64 = conn
            .query_row(
                "SELECT id FROM activity_events WHERE dedup_key = 'evt-0499'",
                [],
                |r| r.get(0),
            )
            .expect("the 500th event");

        let outcome = sweep(&conn, None).expect("first sweep");
        assert_eq!(
            outcome.created, 500,
            "exactly the cap's worth of events processed"
        );
        let cursor = crate::settings::get_i64(&conn, CURSOR_KEY, -1).expect("cursor");
        assert_eq!(
            cursor, id_of_500th,
            "the cursor stops where the read stopped, not at the table ceiling ({max_id})"
        );
        assert!(cursor < max_id, "the two surplus events stay queued");
        assert!(
            notification_exists(&conn, "n:evt:customer_replied:evt-0499"),
            "the 500th event was processed"
        );
        assert!(
            !notification_exists(&conn, "n:evt:customer_replied:evt-0500"),
            "the 501st event was NOT processed by the first sweep (pre-fix it was skipped forever)"
        );

        let outcome = sweep(&conn, None).expect("second sweep");
        assert_eq!(outcome.created, 2, "the two surplus events run now");
        let cursor = crate::settings::get_i64(&conn, CURSOR_KEY, -1).expect("cursor");
        assert_eq!(
            cursor, max_id,
            "the cursor reaches the ceiling once drained"
        );
        assert!(notification_exists(
            &conn,
            "n:evt:customer_replied:evt-0500"
        ));
        assert!(notification_exists(
            &conn,
            "n:evt:customer_replied:evt-0501"
        ));

        let outcome = sweep(&conn, None).expect("third sweep");
        assert_eq!(outcome.created, 0, "nothing left to process");
    }

    #[test]
    fn a_small_backlog_advances_the_cursor_to_the_ceiling() {
        let conn = conn();
        let max_id = seed_backlog(&conn, 3);
        crate::settings::set_i64(&conn, CURSOR_KEY, 0).expect("cursor seeded");
        crate::settings::set_string(&conn, SWEEP_AT_KEY, "2026-10-23 00:00:00")
            .expect("sweep_at seeded");
        let outcome = sweep(&conn, None).expect("sweep");
        assert_eq!(outcome.created, 3);
        let cursor = crate::settings::get_i64(&conn, CURSOR_KEY, -1).expect("cursor");
        assert_eq!(cursor, max_id, "no cap hit — the cursor advances fully");
        assert!(notification_exists(
            &conn,
            "n:evt:customer_replied:evt-0002"
        ));
    }

    #[test]
    fn non_matching_events_alone_advance_the_cursor() {
        let conn = conn();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (502, 52, 'Quiet', 'p', 'active')",
            [],
        )
        .expect("conversation");
        for i in 0..5 {
            conn.execute(
                "INSERT INTO activity_events
                    (conversation_id, event_type, actor_type, occurred_at, dedup_key,
                     thread_local_id, source, metadata)
                 VALUES (1, 'status_changed', 'user', '2026-10-23T00:00:00Z', ?1,
                         NULL, 'test', NULL)",
                params![format!("quiet-{i}")],
            )
            .expect("event");
        }
        let max_id: i64 = conn
            .query_row("SELECT MAX(id) FROM activity_events", [], |r| r.get(0))
            .expect("max");
        crate::settings::set_i64(&conn, CURSOR_KEY, 0).expect("cursor seeded");
        crate::settings::set_string(&conn, SWEEP_AT_KEY, "2026-10-23 00:00:00")
            .expect("sweep_at seeded");
        let outcome = sweep(&conn, None).expect("sweep");
        assert_eq!(outcome.created, 0, "no notification-bearing events");
        let cursor = crate::settings::get_i64(&conn, CURSOR_KEY, -1).expect("cursor");
        assert_eq!(
            cursor, max_id,
            "the filter can never select those rows — the cursor advances"
        );
    }
}

#[cfg(test)]
mod batch31_tests {
    use super::*;
    use rusqlite::{Connection, params};

    #[test]
    fn sweep_burst_oldest_are_not_dropped() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::sync_engine::set_state(&conn, "LIVE");
        crate::settings::set_i64(&conn, CURSOR_KEY, 0).expect("cursor");
        let sweep_at = "2026-10-24 10:00:00";
        crate::settings::set_string(&conn, SWEEP_AT_KEY, sweep_at).expect("sweep_at");

        for i in 1..=5_i64 {
            conn.execute(
                "INSERT INTO jobs (queue, type, status, error, completed_at)
                 VALUES ('outreach', 'demo_job', 'failed', ?1, '2026-10-24 11:00:00')",
                params![format!("job {i} exploded")],
            )
            .expect("new failed job");
        }
        for _ in 0..205 {
            conn.execute(
                "INSERT INTO jobs (queue, type, status, error, completed_at)
                 VALUES ('outreach', 'demo_job', 'failed', 'old', '2026-10-24 09:00:00')",
                [],
            )
            .expect("swept failed job");
        }

        let outcome = sweep(&conn, None).expect("sweep");
        assert_eq!(
            outcome.created, 5,
            "every new failure notifies, none starved by the old rows"
        );
    }
}

#[cfg(test)]
mod ux35_tests {
    use super::*;
    use rusqlite::{Connection, params};

    fn sweep_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::sync_engine::set_state(&conn, "LIVE");
        crate::settings::set_i64(&conn, CURSOR_KEY, 0).expect("cursor");
        crate::settings::set_string(&conn, SWEEP_AT_KEY, "2026-11-06 10:00:00").expect("sweep_at");
        conn
    }

    fn seeded_failure(conn: &Connection, job_type: &str, queue: &str, error: &str) {
        conn.execute(
            "INSERT INTO jobs (queue, type, status, error, completed_at)
             VALUES (?1, ?2, 'failed', ?3, '2026-11-06 11:00:00')",
            params![queue, job_type, error],
        )
        .expect("failed job");
    }

    fn job_failure_row(conn: &Connection) -> (String, Option<String>, Option<String>) {
        conn.query_row(
            "SELECT title, body, payload FROM notifications
             WHERE type = 'job_failure' ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("the job-failure notification")
    }

    #[test]
    fn classifier_maps_the_audit_seen_error_shapes() {
        // The exact shape the audit flagged: LM Studio's localhost URL
        // arriving verbatim in a notification body.
        let (code, friendly) = classify_job_error(
            "LM Studio chat failed: error sending request for url \
             (http://localhost:1234/v1/chat/completions)",
        );
        assert_eq!(code, "unreachable");
        assert!(friendly.contains("localhost:1234"), "{friendly}");
        assert!(friendly.contains("LM Studio"), "{friendly}");
        assert!(!friendly.contains("http://"), "{friendly}");

        let (code, _) = classify_job_error("request timed out");
        assert_eq!(code, "unreachable");

        let (code, _) = classify_job_error("Help Scout API unauthorized (401)");
        assert_eq!(code, "auth");

        let (code, _) = classify_job_error("429 too many requests — rate limit hit");
        assert_eq!(code, "rate_limited");

        let (code, _) = classify_job_error("LM Studio chat parse failed: expected value");
        assert_eq!(code, "bad_response");

        let (code, _) = classify_job_error("analyze_ticket missing conversationId");
        assert_eq!(code, "misconfigured");

        let (code, _) = classify_job_error("something entirely novel");
        assert_eq!(code, "unknown");

        assert_eq!(
            extract_host("error sending request for url (https://api.helpscoutapp.com/v1/x)"),
            Some("api.helpscoutapp.com".to_string())
        );
        assert_eq!(extract_host("no url here"), None);
    }

    #[test]
    fn failed_job_notifications_carry_friendly_text_and_structured_payload() {
        let conn = sweep_db();
        seeded_failure(
            &conn,
            "ai",
            "ai",
            "LM Studio chat failed: error sending request for url \
             (http://localhost:1234/v1/chat/completions)",
        );

        let outcome = sweep(&conn, None).expect("sweep");
        assert_eq!(outcome.created, 1);

        let (title, body, payload) = job_failure_row(&conn);
        assert_eq!(
            title, "Background job failed: AI task",
            "no internal queue/job_type names in the title"
        );
        let body = body.expect("body");
        assert!(
            body.contains("could not reach") && body.contains("localhost:1234"),
            "the body is the friendly classification: {body}"
        );
        assert!(
            !body.contains("error sending request") && !body.contains("http://"),
            "no raw error in the body: {body}"
        );
        assert!(
            body.contains("Hover"),
            "the body points at the hover for the technical detail: {body}"
        );

        let payload = payload.expect("payload");
        let value: serde_json::Value = serde_json::from_str(&payload).expect("payload JSON");
        assert_eq!(
            value.get("code").and_then(|c| c.as_str()),
            Some("unreachable")
        );
        assert_eq!(value.get("queue").and_then(|q| q.as_str()), Some("ai"));
        assert_eq!(value.get("job_type").and_then(|j| j.as_str()), Some("ai"));
        let raw = value.get("raw").and_then(|r| r.as_str()).expect("raw kept");
        assert!(
            raw.contains("http://localhost:1234"),
            "the verbatim error is preserved in the payload: {raw}"
        );
    }

    #[test]
    fn unknown_job_types_still_get_a_clean_title() {
        let conn = sweep_db();
        seeded_failure(&conn, "some_future_job", "sync", "mystery failure");

        let outcome = sweep(&conn, None).expect("sweep");
        assert_eq!(outcome.created, 1);
        let (title, body, _) = job_failure_row(&conn);
        assert_eq!(title, "Background job failed: Some future job");
        assert!(
            body.expect("body").contains("exhausting its retries"),
            "the unknown class falls back to a plain sentence"
        );
    }
}
