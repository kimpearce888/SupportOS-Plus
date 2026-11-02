use super::*;
fn pick_batch(conn: &Connection, campaign_id: i64) -> Vec<BatchRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, customer_local_id, customer_remote_id, snapshot, attempts
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
                snapshot: r.get(3)?,
                attempts: r.get(4)?,
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

pub(super) fn ensure_recipient_updated_at(conn: &Connection) -> Result<()> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info(?) WHERE name = ?",
            params!["outreach_recipients", "updated_at"],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !exists {
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
        if let Err(e) = ensure_recipient_updated_at(&c) {
            log_event(
                &c,
                campaign_id,
                None,
                "recipient_schema_ensure_failed",
                Some(&e.to_string()),
            );
        }
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
            status: None,
            channel: None,
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
