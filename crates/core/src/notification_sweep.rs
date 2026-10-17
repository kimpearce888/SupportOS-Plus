use rusqlite::{Connection, params};

use crate::catalog::NotificationType;
use crate::error::Result;
use crate::mentions::{build_mention_directory, parse_mentions};
use crate::notifications::{NotificationInput, record_notification};

pub const CURSOR_KEY: &str = "notif_event_cursor";

pub const SWEEP_AT_KEY: &str = "notif_sweep_at";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SweepOutcome {
    pub created: u32,
}

pub fn first_sync_settled(conn: &Connection) -> Result<bool> {
    let state = crate::sync_engine::get_state(conn);
    Ok(!matches!(
        state.as_str(),
        "NEW" | "INITIALIZING" | "BACKFILLING"
    ))
}

type SweepRow = (i64, String, String, Option<String>, Option<String>);

pub fn sweep(conn: &Connection, bus: Option<&crate::http::EventBus>) -> Result<SweepOutcome> {
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

    let result = run_sub_sweeps(conn, bus, cursor, &sweep_at);

    crate::settings::set_i64(conn, CURSOR_KEY, max_event_id(conn))?;
    crate::settings::set_string(conn, SWEEP_AT_KEY, &now)?;

    result.map(|created| SweepOutcome { created })
}

fn run_sub_sweeps(
    conn: &Connection,
    bus: Option<&crate::http::EventBus>,
    cursor: i64,
    sweep_at: &str,
) -> Result<u32> {
    let mut created = 0u32;
    created += sweep_conversation_events(conn, bus, cursor)?;
    created += sweep_sla(conn, bus)?;
    created += sweep_automation_approvals(conn, bus)?;
    created += sweep_failed_jobs(conn, bus, sweep_at)?;
    created += sweep_sync_state(conn, bus)?;
    created += sweep_campaign_replies(conn, bus, sweep_at)?;
    created += sweep_known_issues(conn, bus, sweep_at)?;
    created += sweep_issue_spikes(conn, bus, sweep_at)?;
    created += sweep_ai_escalations(conn, bus, sweep_at)?;
    created += sweep_ratings(conn, bus, sweep_at)?;
    Ok(created)
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

fn sweep_conversation_events(
    conn: &Connection,
    bus: Option<&crate::http::EventBus>,
    after_event_id: i64,
) -> Result<u32> {
    let events: Vec<EventRow> = {
        let mut stmt = conn.prepare(
            "SELECT e.conversation_id, e.thread_local_id, e.event_type,
                    e.actor_id, e.metadata, e.dedup_key
             FROM activity_events e
             WHERE e.id > ?1
               AND e.event_type IN ('customer_message', 'internal_note', 'assignment_changed')
             ORDER BY e.id LIMIT 500",
        )?;

        stmt.query_map(params![after_event_id], |r| {
            Ok(EventRow {
                conversation_id: r.get(0)?,
                thread_local_id: r.get(1)?,
                event_type: r.get(2)?,
                actor_local_id: r.get(3)?,
                metadata: r.get(4)?,
                dedup_key: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    if events.is_empty() {
        return Ok(0);
    }
    let mut created = 0u32;
    for e in &events {
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
    Ok(created)
}

fn sweep_sla(conn: &Connection, bus: Option<&crate::http::EventBus>) -> Result<u32> {
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
    bus: Option<&crate::http::EventBus>,
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

fn sweep_failed_jobs(
    conn: &Connection,
    bus: Option<&crate::http::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    let rows: Vec<SweepRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, queue, type, error, completed_at FROM jobs
             WHERE status = 'failed' ORDER BY id DESC LIMIT 200",
        )?;

        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (job_id, queue, job_type, error, completed_at) in rows {
        let Some(completed) = completed_at else {
            continue;
        };
        if before_or_equal(&completed, since_stamp) {
            continue;
        }
        let r = record_notification(
            conn,
            bus,
            &NotificationInput {
                notification_type: NotificationType::JobFailure,
                severity: Some("warning"),
                title: format!("Background job failed ({queue}/{job_type})"),
                body: error.as_deref().map(|e| trim(e, 200)),
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

fn sweep_sync_state(conn: &Connection, bus: Option<&crate::http::EventBus>) -> Result<u32> {
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
    bus: Option<&crate::http::EventBus>,
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
             WHERE r.replied_at IS NOT NULL ORDER BY r.id DESC LIMIT 200",
        )?;

        stmt.query_map([], |r| {
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
        let Some(replied_at) = replied_at else {
            continue;
        };
        if before_or_equal(&replied_at, since_stamp) {
            continue;
        }
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
    bus: Option<&crate::http::EventBus>,
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
             ORDER BY id DESC LIMIT 100",
        )?;

        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (id, title, name, status, created_at) in rows {
        let at = created_at.unwrap_or_default();
        if before_or_equal(&at, since_stamp) {
            continue;
        }
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
    bus: Option<&crate::http::EventBus>,
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
             WHERE trend = 'rising' ORDER BY id DESC LIMIT 100",
        )?;

        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    let day = day_stamp();
    for (id, title, name, conversation_count, updated_at) in rows {
        let at = updated_at.unwrap_or_default();
        if before_or_equal(&at, since_stamp) {
            continue;
        }
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
    bus: Option<&crate::http::EventBus>,
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
             ORDER BY a.id DESC LIMIT 200",
        )?;

        stmt.query_map([], |r| {
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
        let at = created_at.unwrap_or_default();
        if before_or_equal(&at, since_stamp) {
            continue;
        }
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
    bus: Option<&crate::http::EventBus>,
    since_stamp: &str,
) -> Result<u32> {
    type RatingRow = (i64, Option<i64>, Option<i64>, Option<String>, Option<i64>);
    let rows: Vec<RatingRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, remote_id, customer_local_id, comments, conversation_id
             FROM ratings WHERE rating = 'not-good' ORDER BY id DESC LIMIT 100",
        )?;

        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut created = 0u32;
    for (id, remote_id, customer_local_id, comments, conversation_id) in rows {
        let at: Option<String> = conn
            .query_row(
                "SELECT remote_created_at FROM ratings WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        let at = at.unwrap_or_default();
        if before_or_equal(&at, since_stamp) {
            continue;
        }
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

fn before_or_equal(stamp: &str, reference: &str) -> bool {
    match (parse_stamp(stamp), parse_stamp(reference)) {
        (Some(a), Some(b)) => a <= b,
        _ => true,
    }
}

fn parse_stamp(s: &str) -> Option<i64> {
    let normalized = if s.contains('T') {
        s.to_string()
    } else {
        format!("{}Z", s.replace(' ', "T"))
    };
    chrono::DateTime::parse_from_rfc3339(&normalized)
        .ok()
        .map(|d| d.timestamp_millis())
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
