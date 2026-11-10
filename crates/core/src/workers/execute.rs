use super::*;

impl WorkerManager {
    pub(super) async fn execute_job(&self, job_id: i64, kind: &str, payload: &str) {
        let started = std::time::Instant::now();
        let payload: serde_json::Value =
            serde_json::from_str(payload).unwrap_or(serde_json::Value::Null);
        let num = |key: &str| -> Option<i64> {
            payload.get(key).and_then(|v| v.as_i64()).or_else(|| {
                payload
                    .get(key)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
            })
        };
        let outcome: Result<()> = async {
            let mut composer_result: Option<String> = None;
            match kind {
                "sync_conversation" | "sync_conversation_merge" => {
                    // AUD-P0-4 (A5): a malformed payload (missing remoteId /
                    // conversationId) must fail the job loudly — the pre-fix
                    // `if let` with no else let a producer bug complete green
                    // while doing nothing.
                    if let Some(remote) = num("remoteId").or_else(|| num("conversationId")) {
                        self.run_engine(|e| async move { e.sync_single_conversation(remote).await.map(|_| ()) })
                            .await?;
                        if kind == "sync_conversation" {
                            self.emit_conversation_updated_for_remote(remote);
                            if let Some(local) = self.conversation_local_id_for_remote(remote) {
                                self.on_conversation_changed(local);
                            }
                        }
                    } else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "sync_conversation missing remoteId", false)?;
                        return Ok(());
                    }
                }
                "delete_conversation" => {
                    if let Some(remote) = num("remoteId") {
                        let conn = self.lock();
                        conn.execute(
                            "UPDATE conversations SET status = 'deleted',
                                 deleted_at = COALESCE(deleted_at, datetime('now'))
                               WHERE remote_id = ?1",
                            rusqlite::params![remote],
                        )?;
                    } else {
                        // AUD-P0-4 (A5): fail loudly on a malformed payload.
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "delete_conversation missing remoteId", false)?;
                        return Ok(());
                    }
                }
                "sync_customer" => {
                    if let Some(remote) = num("remoteId")
                        && let Some(customer) = self.provider.get_customer(remote).await? {
                            let conn = self.lock();
                            crate::mirror::upsert_customer(&conn, &customer)?;
                        } else if num("remoteId").is_none() {
                        // AUD-P0-4 (A5): fail loudly on a malformed payload —
                        // an absent remote customer is a legitimate no-op.
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "sync_customer missing remoteId", false)?;
                        return Ok(());
                    }
                }
                "delete_customer" => {
                    if let Some(remote) = num("remoteId") {
                        let conn = self.lock();
                        conn.execute(
                            "UPDATE customers SET deleted_at = datetime('now') WHERE remote_id = ?1",
                            rusqlite::params![remote],
                        )?;
                    } else {
                        // AUD-P0-4 (A5): fail loudly on a malformed payload.
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "delete_customer missing remoteId", false)?;
                        return Ok(());
                    }
                }
                "sync_organizations" => {
                    let orgs = self.provider.list_organizations().await?;
                    let conn = self.lock();
                    for o in &orgs {
                        crate::sync_engine::upsert_organization(&conn, o)?;
                    }
                }
                "sync_tags" => {
                    let tags = self.provider.list_tags().await?;
                    let conn = self.lock();
                    for t in &tags {
                        crate::mirror::upsert_tag(&conn, t)?;
                    }
                }
                "sync_user_statuses" => {
                    self.run_engine(|e| async move { e.run_resource("user_statuses", false).await.map(|_| ()) })
                        .await?;
                }
                "sync_conversation_ratings" => {
                    let rating_id = num("ratingId");
                    let conv_remote = num("conversationId").or_else(|| num("remoteId"));
                    if let Some(rating_id) = rating_id {
                        match self.provider.get_rating(rating_id).await {
                            Ok(Some(r)) => {
                                let (inserted, conv_local, conv_number, customer_local) = {
                                    let conn = self.lock();
                                    let conv_local = r
                                        .conversation_id
                                        .and_then(|id| if id > 0 { crate::sync_engine::conversation_local_id(&conn, id) } else { None });
                                    let conv_number = conv_local.and_then(|local| {
                                        conn.query_row(
                                            "SELECT number FROM conversations WHERE id = ?1",
                                            rusqlite::params![local],
                                            |row| row.get::<_, i64>(0),
                                        )
                                        .ok()
                                    });
                                    let customer_local = r
                                        .customer_id
                                        .and_then(|id| if id > 0 { crate::sync_engine::local_id(&conn, "customers", id) } else { None });
                                    let user_local = r
                                        .user_id
                                        .and_then(|id| if id > 0 { crate::sync_engine::local_id(&conn, "users", id) } else { None });
                                    (
                                        upsert_rating_row(&conn, &r, conv_local, customer_local, user_local)?,
                                        conv_local,
                                        conv_number,
                                        customer_local,
                                    )
                                };
                                if inserted {
                                    crate::event_bus::notify_rating_received(
                                        &self.bus,
                                        r.rating.as_deref(),
                                        conv_local,
                                        conv_number,
                                        customer_local,
                                        r.customer_name.as_deref(),
                                        r.comment.as_deref(),
                                    );
                                }
                            }
                            Ok(None) => {
                                tracing::info!(
                                    operation = "sync_conversation_ratings",
                                    rating_id,
                                    "rating absent platform-side - nothing to store"
                                );
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    if let Some(conv_remote) = conv_remote {
                        self.run_engine(|e| async move {
                            e.sync_single_conversation(conv_remote).await.map(|_| ())
                        })
                        .await?;
                    }
                }
                "embed_knowledge_chunks" => {
                    let conn = self.db.long_op_guard();
                    let n = crate::embeddings::embed_pending_knowledge(
                        &conn,
                        self.qdrant.as_deref(),
                    )
                    .unwrap_or(0);
                    tracing::debug!(operation = kind, embedded = n, "embedding pass");
                    // AUD-P1-8 (D2): a full pass means more chunks may be
                    // waiting — requeue so a large corpus keeps flowing
                    // instead of stalling at the 60-chunk cap.
                    if crate::embeddings::pass_hit_cap(n)
                        && !crate::jobs::has_pending(
                            &conn,
                            "embeddings",
                            "embed_knowledge_chunks",
                        )
                        .unwrap_or(false)
                    {
                        let _ = crate::jobs::enqueue_on(
                            &conn,
                            "embeddings",
                            "embed_knowledge_chunks",
                            "{}",
                            4,
                            2,
                        );
                    }
                }
                "embed_docs_chunks" => {
                    let conn = self.db.long_op_guard();
                    let n = crate::embeddings::embed_pending_docs(
                        &conn,
                        self.qdrant.as_deref(),
                    )
                    .unwrap_or(0);
                    tracing::debug!(operation = kind, embedded = n, "embedding pass");
                    // AUD-P1-8 (D2): see embed_knowledge_chunks.
                    if crate::embeddings::pass_hit_cap(n)
                        && !crate::jobs::has_pending(&conn, "embeddings", "embed_docs_chunks")
                            .unwrap_or(false)
                    {
                        let _ =
                            crate::jobs::enqueue_on(&conn, "embeddings", "embed_docs_chunks", "{}", 4, 2);
                    }
                }
                "embed_conversation_chunks" => {
                    let conn = self.db.long_op_guard();
                    let n = crate::embeddings::embed_pending_conversation_chunks(
                        &conn,
                        self.qdrant.as_deref(),
                    )
                    .unwrap_or(0);
                    tracing::debug!(operation = kind, embedded = n, "embedding pass");
                    // AUD-P1-8 (D2): see embed_knowledge_chunks.
                    if crate::embeddings::pass_hit_cap(n)
                        && !crate::jobs::has_pending(
                            &conn,
                            "embeddings",
                            "embed_conversation_chunks",
                        )
                        .unwrap_or(false)
                    {
                        let _ = crate::jobs::enqueue_on(
                            &conn,
                            "embeddings",
                            "embed_conversation_chunks",
                            "{}",
                            4,
                            2,
                        );
                    }
                }
                "rebuild_search_index" => {
                    let conn = self.lock();
                    let n = crate::search::rebuild_indexes(&conn)?;
                    tracing::info!(operation = kind, conversations = n, "Search index rebuilt");
                }
                "rebuild_activity" => {
                    let conn = self.lock();
                    let summary = crate::activity::rebuild_all(&conn)?;
                    tracing::info!(
                        operation = kind,
                        conversations = summary.conversations,
                        events_written = summary.events_written,
                        events_total = summary.events_total,
                        "Activity rebuilt from the thread mirror (rebuildAll)"
                    );
                }
                "outreach_send_batch" => {
                    if let Some(campaign_id) = num("campaignId") {
                        let summary = crate::outreach::send_batch(
                            &self.db,
                            &self.provider,
                            campaign_id,
                        )
                        .await;
                        tracing::info!(
                            operation = kind,
                            campaign_id = campaign_id,
                            remaining = summary["remaining"].as_i64().unwrap_or(-1),
                            "Outreach send batch processed"
                        );
                    } else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "outreach_send_batch missing campaignId", true)?;
                        return Ok(());
                    }
                }
                "analyze_ticket" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "analyze_ticket missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_ai_job(kind, conv_id, AiJob::Analyze).await?;
                }
                "generate_draft" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "generate_draft missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_ai_job(kind, conv_id, AiJob::Draft).await?;
                }
                "create_ai_note" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "create_ai_note missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_ai_job(kind, conv_id, AiJob::Note).await?;
                }
                "extract_attributes" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(
                            &conn,
                            job_id,
                            "extract_attributes missing conversationId",
                            true,
                        )?;
                        return Ok(());
                    };
                    self.run_attributes_job(conv_id).await?;
                }
                "extract_memories" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(
                            &conn,
                            job_id,
                            "extract_memories missing conversationId",
                            true,
                        )?;
                        return Ok(());
                    };
                    self.run_memory_job(conv_id).await?;
                }
                "cluster_issues" => {
                    self.run_clustering_job().await?;
                }
                "rewrite_composer" => {
                    composer_result = self.run_rewrite_job(&payload).await?;
                }
                "translate_composer" => {
                    composer_result = self.run_translate_job(&payload).await?;
                }
                "review_draft" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "review_draft missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_coaching_job(conv_id, &payload).await?;
                }
                "qa_ai_layer" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(&conn, job_id, "qa_ai_layer missing conversationId", true)?;
                        return Ok(());
                    };
                    self.run_qa_ai_job(conv_id).await?;
                }
                "analyze_interaction" => {
                    let Some(conv_id) = num("conversationId") else {
                        let conn = self.lock();
                        jobs::fail(
                            &conn,
                            job_id,
                            "analyze_interaction missing conversationId",
                            true,
                        )?;
                        return Ok(());
                    };
                    self.run_interaction_job(conv_id).await?;
                }
                "copilot_chat" => {
                    self.run_copilot_chat_job(job_id, &payload).await?;
                }
                "golden_eval" => {
                    self.run_golden_job().await?;
                }
                "rebuild_gaps" => {
                    self.run_gap_rebuild_job().await?;
                }
                "draft_article" => {
                    self.run_draft_article_job(job_id, &payload).await?;
                }
                "publish_article_draft" => {
                    self.run_publish_article_draft_job(job_id, &payload).await?;
                }
                "weekly_digest" => {
                    self.run_weekly_digest_job(&payload).await?;
                }
                "fetch_hs_report" => {
                    self.run_fetch_hs_report_job(job_id, &payload).await?;
                }
                "send_chat_message" => {
                    let chat_id = payload
                        .get("chatId")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let message_id = payload
                        .get("messageId")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let text = payload
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if chat_id.is_empty() || message_id.is_empty() {
                        let conn = self.lock();
                        jobs::fail(
                            &conn,
                            job_id,
                            "send_chat_message missing chatId/messageId",
                            false,
                        )?;
                        return Ok(());
                    }
                    self.run_send_chat_message_job(job_id, &chat_id, &message_id, &text)
                        .await?;
                }
                "saved_reply_write" => {
                    self.run_saved_reply_write_job(job_id, &payload).await?;
                }
                "run_workflow" => {
                    self.run_workflow_job(job_id, &payload).await?;
                }
                "webhook_health_check" => {
                    self.run_webhook_health_job().await?;
                }
                "automation_action_awaiting_approval" => {
                    let approved = payload
                        .get("approved")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if approved {
                        let action = payload.get("action").cloned();
                        let rule_id = num("ruleId");
                        let conversation_id = num("conversationId");
                        if let (Some(action), Some(rule_id), Some(conversation_id)) =
                            (action, rule_id, conversation_id)
                        {
                            let executed = {
                                let mut conn = self.lock();
                                crate::automation::execute_approved_action(
                                    &mut conn,
                                    conversation_id,
                                    &action,
                                )?
                            };
                            if executed {
                                let kind = action
                                    .get("kind")
                                    .and_then(|k| k.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let conn = self.lock();
                                crate::automation::record_approved_run(
                                    &conn,
                                    rule_id,
                                    conversation_id,
                                    &kind,
                                )?;
                            } else {
                                // AUD-P0-6 (A6): an approved action that did
                                // not execute used to record nothing at all
                                // (and pre-batch, unknown kinds fake-completed
                                // via the catch-all). Record the honest skip.
                                let kind = action
                                    .get("kind")
                                    .and_then(|k| k.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let conn = self.lock();
                                crate::automation::record_run(
                                    &conn,
                                    rule_id,
                                    Some(conversation_id),
                                    "skipped",
                                    &format!(
                                        "Approved action {kind} was not executed \
                                         (no engine handler or missing parameters)"
                                    ),
                                )?;
                            }
                        }
                    } else {
                        let conn = self.lock();
                        jobs::park_job(&conn, job_id)?;
                    }
                }
                "download_recent_attachments" => {
                    self.run_attachment_downloads().await;
                }
                #[cfg(test)]
                "__test_panic" => {
                    panic!("intentional test panic (batch 27 item 86)");
                }
                _ => {
                    let conn = self.lock();
                    jobs::fail(&conn, job_id, &format!("Unknown job type: {kind}"), false)?;
                    return Ok(());
                }
            }
            let conn = self.lock();
            match composer_result {
                Some(text) => jobs::complete_with_result(&conn, job_id, &text)?,
                None => jobs::complete(&conn, job_id)?,
            }
            tracing::debug!(
                operation = kind,
                latency_ms = started.elapsed().as_millis() as u64,
                "Job completed"
            );
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            let msg = e.to_string();
            let conn = self.lock();
            // AUD-P2-1 (A7): the catch-all used to hard-code
            // `let retryable = true;`, so a deterministic AI failure
            // (unparseable model output, "conversation not found",
            // AI disabled — all `LmStudioError { retryable: false }`)
            // burned every attempt and re-notified each time. Honor
            // the flag when the error is a typed LmStudioError;
            // anything else (sqlite, io, panic-adjacent) keeps the
            // old retry-everything behavior. `jobs::fail` marks the
            // job failed either way — only the requeue differs.
            let retryable = crate::ai_pipeline::lm_error(&e)
                .map(|lm| lm.retryable)
                .unwrap_or(true);
            let _ = jobs::fail(&conn, job_id, &msg, retryable);
            let _ = jobs::log_error(&conn, "workers", &format!("Job {kind} failed: {msg}"));
            tracing::warn!(operation = kind, error = %msg, retryable, "Job failed");
        }
    }

    pub(super) fn on_after_initial_sync(&self) {
        let conn = self.lock();
        let _ = crate::maintenance::refresh_products(&conn);
        let auto_download =
            crate::settings::get_i64(&conn, "attachment_auto_download", 1).unwrap_or(1) != 0;
        let _ = jobs::enqueue_on(&conn, "embeddings", "embed_knowledge_chunks", "{}", 4, 2);
        let _ = jobs::enqueue_on(&conn, "embeddings", "embed_conversation_chunks", "{}", 4, 2);
        let _ = jobs::enqueue_on(&conn, "sync", "rebuild_activity", "{}", 3, 2);
        if auto_download {
            let _ = jobs::enqueue_on(
                &conn,
                "attachments",
                "download_recent_attachments",
                "{}",
                4,
                2,
            );
        }
        if crate::ai_hub::ai_feature_enabled(&conn, "ticket_analysis")
            // AUD-P1-7 (D1): no backend configured → no bulk AI jobs (the
            // fresh-install noise: every conversation used to spawn a
            // doomed analyze job pair).
            && crate::ai_hub::backend_configured(&conn)
        {
            let fresh: Vec<i64> = {
                let mut stmt = match conn.prepare(
                    "SELECT c.id FROM conversations c
                      WHERE NOT EXISTS (
                            SELECT 1 FROM ai_runs a
                             WHERE a.conversation_id = c.id
                               AND a.type = 'ticket_analysis'
                               AND a.status = 'completed')
                        AND c.deleted_at IS NULL
                      ORDER BY c.id",
                ) {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let ids: Vec<i64> = match stmt.query_map([], |r| r.get::<_, i64>(0)) {
                    Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                    Err(_) => Vec::new(),
                };
                ids
            };
            for id in fresh {
                let _ = jobs::enqueue_on(
                    &conn,
                    "ai",
                    "analyze_ticket",
                    &format!("{{\"conversationId\":{id}}}"),
                    3,
                    2,
                );
            }
        }
        if crate::ai_hub::ai_feature_enabled(&conn, "attributes")
            && crate::ai_hub::backend_configured(&conn)
        {
            let mut stmt = match conn.prepare(
                "SELECT c.id FROM conversations c
                      WHERE NOT EXISTS (
                            SELECT 1 FROM ai_attributes a
                             WHERE a.conversation_id = c.id
                               AND a.superseded_at IS NULL)
                        AND c.deleted_at IS NULL
                      ORDER BY c.id",
            ) {
                Ok(s) => s,
                Err(_) => return,
            };
            let ids: Vec<i64> = match stmt.query_map([], |r| r.get::<_, i64>(0)) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(_) => Vec::new(),
            };
            drop(stmt);
            for id in ids {
                let _ = jobs::enqueue_on(
                    &conn,
                    "ai",
                    "extract_attributes",
                    &format!("{{\"conversationId\":{id}}}"),
                    3,
                    2,
                );
            }
        }
    }

    fn remote_id_for_local(&self, local_id: i64) -> Option<i64> {
        let conn = self.lock();
        conn.query_row(
            "SELECT remote_id FROM conversations WHERE id = ?1",
            rusqlite::params![local_id],
            |r| r.get(0),
        )
        .ok()
    }

    pub(super) fn emit_conversation_updated_for_remote(&self, remote_id: i64) {
        use crate::events::ServerEvent;
        let conn = self.lock();
        let row: Option<(i64, i64, Option<i64>, Option<String>)> = conn
            .query_row(
                "SELECT id, number, mailbox_local_id, subject FROM conversations WHERE remote_id = ?1",
                rusqlite::params![remote_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok();
        if let Some((id, number, mailbox_id, subject)) = row {
            self.bus.emit(&ServerEvent::conversation_updated(
                Some(id),
                Some(number),
                mailbox_id,
                subject,
                "sync",
            ));
        }
    }

    pub(super) fn conversation_local_id_for_remote(&self, remote_id: i64) -> Option<i64> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id FROM conversations WHERE remote_id = ?1",
            rusqlite::params![remote_id],
            |r| r.get(0),
        )
        .ok()
    }

    pub(super) fn on_conversation_changed(&self, conversation_local_id: i64) {
        let result: Result<()> = (|| {
            let (last_thread_type, latest_customer_thread, last_fired): (
                Option<String>,
                Option<i64>,
                Option<i64>,
            ) = {
                let conn = self.lock();
                let last_thread_type: Option<String> = conn
                    .query_row(
                        "SELECT type FROM conversation_threads
                          WHERE conversation_id = ?1 AND deleted_at IS NULL
                          ORDER BY created_at DESC, id DESC LIMIT 1",
                        rusqlite::params![conversation_local_id],
                        |r| r.get(0),
                    )
                    .ok();
                let latest_customer_thread: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM conversation_threads
                          WHERE conversation_id = ?1 AND deleted_at IS NULL
                            AND type = 'customer'
                          ORDER BY created_at DESC, id DESC LIMIT 1",
                        rusqlite::params![conversation_local_id],
                        |r| r.get(0),
                    )
                    .ok()
                    .flatten();
                let last_fired: Option<i64> = conn
                    .query_row(
                        "SELECT automation_last_customer_thread_id
                           FROM conversations WHERE id = ?1",
                        rusqlite::params![conversation_local_id],
                        |r| r.get(0),
                    )
                    .ok()
                    .flatten();
                (last_thread_type, latest_customer_thread, last_fired)
            };
            if last_thread_type.as_deref() != Some("customer") {
                return Ok(());
            }
            let trigger = crate::automation::customer_thread_trigger_decision(
                last_fired,
                latest_customer_thread,
            );
            if let Some(trigger) = trigger {
                let newest_customer_thread =
                    latest_customer_thread.expect("decision guarantees a thread id");
                let mut conn = self.lock();
                let fired = crate::automation::fire_trigger_for_conversation(
                    &mut conn,
                    trigger,
                    conversation_local_id,
                );
                if fired.is_ok() {
                    conn.execute(
                        "UPDATE conversations
                            SET automation_last_customer_thread_id = ?1
                          WHERE id = ?2",
                        rusqlite::params![newest_customer_thread, conversation_local_id],
                    )?;
                }
            }
            {
                let conn = self.lock();
                // AUD-P1-7 (D1): the per-conversation AI fan-out is gated on a
                // configured backend — a fresh install (no role routed, no
                // legacy base URL) no longer spawns jobs that can only fail
                // with the "check that LM Studio… is running" notification.
                if !crate::ai_hub::backend_configured(&conn) {
                    tracing::debug!(
                        conversation_id = conversation_local_id,
                        operation = "on_conversation_changed",
                        "no AI backend configured — skipping the AI job fan-out"
                    );
                } else {
                    if crate::ai_hub::ai_feature_enabled(&conn, "ticket_analysis") {
                        let _ = jobs::enqueue_on(
                            &conn,
                            "ai",
                            "analyze_ticket",
                            &format!("{{\"conversationId\":{conversation_local_id}}}"),
                            3,
                            2,
                        );
                    }
                    if !crate::ai_hub::ai_feature_enabled(&conn, "ticket_analysis")
                        && crate::ai_hub::ai_feature_enabled(&conn, "attributes")
                    {
                        let _ = jobs::enqueue_on(
                            &conn,
                            "ai",
                            "extract_attributes",
                            &format!("{{\"conversationId\":{conversation_local_id}}}"),
                            3,
                            2,
                        );
                    }
                    if crate::ai_hub::ai_feature_enabled(&conn, "memory_extraction") {
                        let _ = jobs::enqueue_on(
                            &conn,
                            "ai",
                            "extract_memories",
                            &format!("{{\"conversationId\":{conversation_local_id}}}"),
                            3,
                            2,
                        );
                    }
                    if crate::ai_hub::ai_feature_enabled(&conn, "interaction_intelligence") {
                        let _ = jobs::enqueue_on(
                            &conn,
                            "ai",
                            "analyze_interaction",
                            &format!("{{\"conversationId\":{conversation_local_id}}}"),
                            3,
                            2,
                        );
                    }
                }
                let _ = crate::interaction_signals::record_current_interaction(
                    &conn,
                    conversation_local_id,
                );
            }
            Ok(())
        })();
        if let Err(e) = result {
            tracing::warn!(
                error = %e,
                conversation_id = conversation_local_id,
                operation = "on_conversation_changed",
                "Automation fire on conversation change failed (non-fatal)"
            );
        }
    }

    pub(super) async fn run_engine<F, Fut>(&self, f: F) -> Result<()>
    where
        F: FnOnce(Arc<SyncEngine>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let Some(engine) = self.engine.clone() else {
            return Ok(());
        };
        f(engine).await
    }

    async fn run_ai_job(&self, kind: &str, conv_id: i64, job: AiJob) -> Result<()> {
        let (role, feature) = match job {
            AiJob::Analyze => (crate::ai_hub::AiRole::Classify, "ticket_analysis"),
            AiJob::Draft => (crate::ai_hub::AiRole::Draft, "draft"),
            AiJob::Note => (crate::ai_hub::AiRole::Summarize, "ai_note"),
        };
        let backend = {
            let conn = self.lock();
            if !crate::ai_hub::ai_feature_enabled(&conn, feature) {
                tracing::info!(
                    operation = kind,
                    conversation_id = conv_id,
                    feature,
                    "AI feature disabled - job completed without AI output"
                );
                return Ok(());
            }
            crate::ai_hub::backend_for_role(&conn, role)
        };
        if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!(
                operation = kind,
                conversation_id = conv_id,
                "AI backend disabled - job completed without AI output"
            );
            return Ok(());
        }
        let db = self.db.clone();
        let index = self.qdrant.clone();
        let (result, note_text, remote_id) = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                let remote: Option<i64> = conn
                    .query_row(
                        "SELECT remote_id FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                        rusqlite::params![conv_id],
                        |r| r.get(0),
                    )
                    .ok();
                let remote_id = match remote {
                    Some(r) => r,
                    None => {
                        return (
                            // AUD-P2-1 (A7): a missing conversation is
                            // deterministic — the pipeline's own
                            // "Conversation not found" carries
                            // retryable: false, and this pre-flight
                            // guard now agrees instead of retrying a
                            // job that can never succeed.
                            Err(Error::Other(Box::new(
                                crate::ai_pipeline::LmStudioError::new(
                                    "Conversation not found locally.",
                                    false,
                                ),
                            ))),
                            None,
                            None,
                        );
                    }
                };
                match job {
                    AiJob::Analyze => {
                        let r = crate::ai_pipeline::analyze_ticket(
                            &conn,
                            &backend,
                            index.as_deref(),
                            conv_id,
                            false,
                        )
                        .await
                        .map(|_| ())
                        // AUD-P2-1 (A7): box the typed error, don't
                        // stringify — the failure handler downcasts it
                        // back to read `retryable`.
                        .map_err(|e| Error::Other(Box::new(e)));
                        (r, None, Some(remote_id))
                    }
                    AiJob::Draft => {
                        let mode = "standard".to_string();
                        let r = crate::ai_pipeline::generate_draft(
                            &conn,
                            &backend,
                            index.as_deref(),
                            conv_id,
                            &mode,
                            false,
                            None,
                        )
                        .await
                        .map(|_| ())
                        // AUD-P2-1 (A7): typed box, see the Analyze arm.
                        .map_err(|e| Error::Other(Box::new(e)));
                        (r, None, Some(remote_id))
                    }
                    AiJob::Note => {
                        let analysis = match crate::ai_pipeline::analyze_ticket(
                            &conn,
                            &backend,
                            index.as_deref(),
                            conv_id,
                            false,
                        )
                        .await
                        {
                            Ok(outcome) => outcome.analysis,
                            Err(e) => {
                                // AUD-P2-1 (A7): typed box, see the
                                // Analyze arm.
                                return (Err(Error::Other(Box::new(e))), None, Some(remote_id));
                            }
                        };
                        let mut body = String::new();
                        if let Some(summary) = &analysis.summary {
                            body.push_str(&format!("Summary: {summary}\n"));
                        }
                        if let Some(intent) = &analysis.intent {
                            body.push_str(&format!("Intent: {intent}\n"));
                        }
                        if let Some(goal) = &analysis.customer_goal {
                            body.push_str(&format!("Customer goal: {goal}\n"));
                        }
                        if let Some(sentiment) = &analysis.sentiment {
                            body.push_str(&format!("Sentiment: {sentiment}"));
                            if let Some(urgency) = &analysis.urgency {
                                body.push_str(&format!(" (urgency: {urgency})"));
                            }
                            body.push('\n');
                        }
                        if !analysis.missing_information.is_empty() {
                            body.push_str(&format!(
                                "Missing information: {}\n",
                                analysis.missing_information.join(", ")
                            ));
                        }
                        if body.trim().is_empty() {
                            return (
                                Err(Error::Other(
                                    "Ticket analysis carried no note content.".into(),
                                )),
                                None,
                                Some(remote_id),
                            );
                        }
                        body.push_str("\n(This note was drafted by SupportOS AI.)");
                        (Ok(()), Some(body), Some(remote_id))
                    }
                }
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;

        result?;
        if matches!(job, AiJob::Analyze) {
            let conn = self.lock();
            if crate::ai_hub::ai_feature_enabled(&conn, "attributes") {
                let _ = jobs::enqueue_on(
                    &conn,
                    "ai",
                    "extract_attributes",
                    &format!("{{\"conversationId\":{conv_id}}}"),
                    3,
                    2,
                );
            }
            if crate::ai_hub::ai_feature_enabled(&conn, "memory_extraction") {
                let _ = jobs::enqueue_on(
                    &conn,
                    "ai",
                    "extract_memories",
                    &format!("{{\"conversationId\":{conv_id}}}"),
                    3,
                    2,
                );
            }
        }
        if let (Some(text), Some(remote_id)) = (note_text, remote_id) {
            self.provider
                .create_note_thread(crate::helpscout::CreateThreadInput {
                    conversation_id: remote_id,
                    text,
                    draft: false,
                    cc: vec![],
                    bcc: vec![],
                    status_after: None,
                    assign_to: None,
                    customer_id: None,
                })
                .await?;
            let engine = self.engine.clone();
            if let Some(engine) = engine {
                engine.sync_single_conversation(remote_id).await?;
            }
            let conn = self.lock();
            let _ = crate::jobs::audit(
                &conn,
                "ai",
                "note_added",
                crate::sync_engine::conversation_local_id(&conn, remote_id),
                None,
                None,
                None,
                None,
                true,
            );
            self.emit_conversation_updated_for_remote(remote_id);
        }
        tracing::info!(
            operation = kind,
            conversation_id = conv_id,
            "AI job completed"
        );
        Ok(())
    }

    async fn run_attributes_job(&self, conv_id: i64) -> Result<()> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "attributes"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Classify),
            )
        };
        if !enabled {
            tracing::info!(
                operation = "extract_attributes",
                conversation_id = conv_id,
                "attributes feature disabled - job skipped"
            );
            return Ok(());
        }
        let db = self.db.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = db.long_op_guard();
            block_ai_pipeline(async {
                let remote: Option<i64> = conn
                    .query_row(
                        "SELECT remote_id FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                        rusqlite::params![conv_id],
                        |r| r.get(0),
                    )
                    .ok();
                crate::ai_attributes::compute_with_backend(&mut conn, &backend, conv_id, false)
                    .await
                    .map(|snap| (snap.is_some(), remote))
                    .map_err(|e| Error::Other(e.to_string().into()))
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;
        let (snapshotted, remote) = result?;
        if let Some(remote) = remote.filter(|_| snapshotted) {
            self.emit_conversation_updated_for_remote(remote);
        }
        tracing::info!(
            operation = "extract_attributes",
            conversation_id = conv_id,
            snapshotted,
            "attribute extraction finished"
        );
        Ok(())
    }

    async fn run_memory_job(&self, conv_id: i64) -> Result<()> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "memory_extraction"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Summarize),
            )
        };
        if !enabled {
            tracing::info!(
                operation = "extract_memories",
                conversation_id = conv_id,
                "memory extraction disabled - job skipped"
            );
            return Ok(());
        }
        if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!(
                operation = "extract_memories",
                conversation_id = conv_id,
                "no Summarize role endpoint routed - job skipped"
            );
            return Ok(());
        }
        let db = self.db.clone();
        let extracted = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::ai_pipeline::extract_memories(&conn, &backend, conv_id).await
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;
        tracing::info!(
            operation = "extract_memories",
            conversation_id = conv_id,
            extracted,
            "customer memory extraction finished"
        );
        Ok(())
    }

    async fn run_clustering_job(&self) -> Result<()> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "issue_clustering"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Classify),
            )
        };
        if !enabled {
            tracing::info!("issue clustering disabled - job skipped");
            return Ok(());
        }
        if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!("no Classify role endpoint routed - clustering skipped");
            return Ok(());
        }
        let db = self.db.clone();
        let clustered = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::ai_pipeline::cluster_issues(&conn, &backend, 14)
                    .await
                    .map(|clusters| clusters.len())
                    .map_err(|e| Error::Other(e.to_string().into()))
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;
        let clustered = clustered?;
        tracing::info!(clusters = clustered, "issue clustering finished");
        // AUD-P2-12 (C8): clustering completion now emits the same
        // conversation_updated event the gap rebuild does — the issue
        // radar is invalidated by it (queries.rs), so a queued "Cluster
        // now" refreshes the Clusters tab instead of waiting for a
        // manual reload (the audit's dead-end: no completion signal).
        {
            use crate::events::ServerEvent;
            self.bus.emit(&ServerEvent::conversation_updated(
                None,
                None,
                None,
                None,
                "cluster_issues",
            ));
        }
        Ok(())
    }

    async fn run_rewrite_job(&self, payload: &serde_json::Value) -> Result<Option<String>> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "draft"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Draft),
            )
        };
        let text: String = payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if !enabled || matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!(
                operation = "rewrite_composer",
                "rewrite skipped - draft feature off or no Draft role endpoint routed"
            );
            return Ok(None);
        }
        if text.trim().is_empty() {
            let conn = self.lock();
            jobs::log_error(&conn, "workers", "rewrite_composer called with empty text")?;
            return Ok(None);
        }
        let instruction: String = payload
            .get("instruction")
            .and_then(|v| v.as_str())
            .unwrap_or("more_direct")
            .to_string();
        let conversation_id = payload.get("conversationId").and_then(|v| v.as_i64());
        let db = self.db.clone();
        let rewritten = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::ai_pipeline::rewrite_composer_text(
                    &conn,
                    &backend,
                    conversation_id,
                    &text,
                    &instruction,
                )
                .await
                .map_err(|e| Error::Other(e.to_string().into()))
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))??;
        tracing::info!(
            operation = "rewrite_composer",
            chars = rewritten.chars().count(),
            "composer rewrite finished"
        );
        Ok(Some(rewritten))
    }

    async fn run_translate_job(&self, payload: &serde_json::Value) -> Result<Option<String>> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "translation"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Draft),
            )
        };
        if !enabled || matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!(
                operation = "translate_composer",
                "translation skipped - feature off or no Draft role endpoint routed"
            );
            return Ok(None);
        }
        let text: String = payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let target: String = payload
            .get("targetLang")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let from: Option<String> = payload
            .get("fromLang")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let conversation_id = payload.get("conversationId").and_then(|v| v.as_i64());
        if text.trim().is_empty() || target.is_empty() {
            let conn = self.lock();
            jobs::log_error(
                &conn,
                "workers",
                "translate_composer called with empty text or target language",
            )?;
            return Ok(None);
        }
        let db = self.db.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            let started = std::time::Instant::now();
            let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let usage_slot: std::sync::Arc<
                std::sync::Mutex<Option<crate::ai_provider::TokenUsage>>,
            > = std::sync::Arc::new(std::sync::Mutex::new(None));
            let result = block_ai_pipeline(async {
                let backend_ref = &backend;
                let called_ref = called.clone();
                let usage_ref = usage_slot.clone();
                let chat = |msgs: Vec<crate::ai_provider::ChatMessage>,
                            temp: f64,
                            max_tokens: u32|
                 -> TranslateChatFuture<'_> {
                    let called_ref = called_ref.clone();
                    let usage_ref = usage_ref.clone();
                    Box::pin(async move {
                        let res = backend_ref
                            .chat_qa(msgs, temp, max_tokens, false)
                            .await
                            .map_err(|e| e.to_string())?;
                        called_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                        *usage_ref.lock().unwrap_or_else(|p| p.into_inner()) = res.usage;
                        Ok((res.content, res.model))
                    })
                };
                crate::translation::translate(
                    &conn,
                    &chat,
                    &text,
                    from.as_deref(),
                    &target,
                    Some("agent_draft"),
                )
                .await
            });
            match result {
                Ok(value) => {
                    let model_called =
                        called.load(std::sync::atomic::Ordering::SeqCst);
                    if model_called {
                        let run_id = crate::ai_pipeline::start_run(
                            &conn,
                            "translation",
                            conversation_id,
                            Some(value.get("model").and_then(|m| m.as_str()).unwrap_or_default()),
                            crate::ai_prompts::PROMPT_VERSIONS_TRANSLATION,
                            None,
                            &serde_json::json!([target, conversation_id]),
                        )
                        .ok();
                        if let Some(run_id) = run_id {
                            let usage = *usage_slot.lock().unwrap_or_else(|p| p.into_inner());
                            let _ = crate::ai_pipeline::complete_run_with_usage(
                                &conn,
                                run_id,
                                &serde_json::json!({
                                    "output": value.get("translated_text").cloned().unwrap_or_default(),
                                    "cached": false,
                                    "target_lang": target,
                                }),
                                started.elapsed().as_millis() as u64,
                                usage.as_ref(),
                                backend.endpoint_id(),
                            );
                        }
                    }
                    Ok(value.get("translated_text")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string())
                }
                Err(e) => Err(match e {
                    crate::translation::TranslateError::Client(m)
                    | crate::translation::TranslateError::Service(m)
                    | crate::translation::TranslateError::BadGateway(m) => m,
                }),
            }
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;
        match outcome {
            Ok(translated) => {
                tracing::info!(
                    operation = "translate_composer",
                    chars = translated.chars().count(),
                    "composer translation finished"
                );
                Ok(Some(translated))
            }
            Err(message) => Err(Error::Other(message.into())),
        }
    }

    async fn run_coaching_job(&self, conv_id: i64, payload: &serde_json::Value) -> Result<()> {
        let draft: String = payload
            .get("draft")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if draft.trim().is_empty() {
            let conn = self.lock();
            jobs::log_error(&conn, "workers", "review_draft called with empty draft")?;
            return Ok(());
        }
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "draft_coaching"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Summarize),
            )
        };
        let include_ai = enabled && !matches!(backend, crate::ai_pipeline::AiBackend::Disabled);
        let db = self.db.clone();
        let review = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                let backend_ref = &backend;
                let chat = |msgs: Vec<crate::ai_provider::ChatMessage>,
                            temp: f64,
                            max_tokens: u32,
                            json_mode: bool|
                 -> CoachingChatFuture<'_> {
                    Box::pin(async move {
                        let res = backend_ref
                            .chat_qa(msgs, temp, max_tokens, json_mode)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok((res.content, res.model, res.latency_ms))
                    })
                };
                let chat_for_review: Option<crate::coaching::CoachingChatFn<'_>> =
                    if include_ai { Some(&chat) } else { None };
                crate::coaching::review_draft(&conn, chat_for_review, conv_id, &draft, include_ai)
                    .await
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;
        if let Err(reason) = review {
            let conn = self.lock();
            let _ = jobs::log_error(&conn, "workers", &format!("review_draft: {reason}"));
            return Ok(());
        }
        if let Some(remote) = self.remote_id_for_local(conv_id) {
            self.emit_conversation_updated_for_remote(remote);
        }
        tracing::info!(
            operation = "review_draft",
            conversation_id = conv_id,
            include_ai,
            "draft coaching review finished"
        );
        Ok(())
    }

    async fn run_qa_ai_job(&self, conv_id: i64) -> Result<()> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "post_resolution_qa"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Classify),
            )
        };
        if !enabled {
            tracing::info!(
                operation = "qa_ai_layer",
                "post-resolution QA feature off - skipped"
            );
            return Ok(());
        }
        if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            let conn = self.lock();
            jobs::log_error(
                &conn,
                "workers",
                "qa_ai_layer: no Classify role endpoint routed (AI Center > Model-role routing)",
            )?;
            return Ok(());
        }
        let db = self.db.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::quality::compute_qa_ai_layer(&conn, &backend, conv_id).await
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;
        tracing::info!(
            operation = "qa_ai_layer",
            conversation_id = conv_id,
            error = outcome.error.as_deref().unwrap_or("none"),
            "post-resolution QA AI layer finished"
        );
        if let Some(remote) = self.remote_id_for_local(conv_id) {
            self.emit_conversation_updated_for_remote(remote);
        }
        Ok(())
    }

    async fn run_interaction_job(&self, conv_id: i64) -> Result<()> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "interaction_intelligence"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Classify),
            )
        };
        if !enabled || matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!(
                operation = "analyze_interaction",
                conversation_id = conv_id,
                "interaction intelligence skipped - feature off or no Classify endpoint"
            );
            return Ok(());
        }
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::ai_pipeline::analyze_interaction(&conn, &backend, conv_id)
                    .await
                    .map(|o| o.ai_enriched)
                    .map_err(|e| Error::Other(e.to_string().into()))
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))??;
        tracing::info!(
            operation = "analyze_interaction",
            conversation_id = conv_id,
            "interaction analysis finished"
        );
        Ok(())
    }

    async fn run_copilot_chat_job(&self, job_id: i64, payload: &serde_json::Value) -> Result<()> {
        let question: String = payload
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let conversation_id = payload.get("conversationId").and_then(|v| v.as_i64());
        let session_id = payload.get("sessionId").and_then(|v| v.as_i64());
        if question.trim().is_empty() {
            let conn = self.lock();
            jobs::fail(&conn, job_id, "copilot_chat: empty question", false)?;
            return Ok(());
        }
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "copilot"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Agent),
            )
        };
        if !enabled {
            let conn = self.lock();
            jobs::fail(
                &conn,
                job_id,
                "Copilot is switched off (AI Center > Features & kill switches).",
                false,
            )?;
            return Ok(());
        }
        if matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            let conn = self.lock();
            jobs::fail(
                &conn,
                job_id,
                "No Agent role endpoint routed - assign one in AI Center > Model-role routing.",
                false,
            )?;
            return Ok(());
        }
        let db = self.db.clone();
        let index = self.qdrant.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::copilot::chat(
                    &conn,
                    &backend,
                    index.as_deref(),
                    &question,
                    conversation_id,
                    session_id,
                )
                .await
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()));
        match outcome {
            Ok(Ok(_value)) => {
                if let Some(conv) = conversation_id
                    && let Some(remote) = self.remote_id_for_local(conv)
                {
                    self.emit_conversation_updated_for_remote(remote);
                } else {
                    use crate::events::ServerEvent;
                    self.bus.emit(&ServerEvent::conversation_updated(
                        None,
                        None,
                        None,
                        None,
                        "copilot_chat",
                    ));
                }
                tracing::info!(operation = "copilot_chat", "copilot chat turn finished");
                Ok(())
            }
            Ok(Err(e)) => {
                let conn = self.lock();
                // AUD-P2-1 (A7): copilot failures were hard-marked
                // non-retryable — a transient LM Studio outage killed
                // the turn on the first try while every other AI lane
                // retried. The flag already carries the right
                // semantics (transport -> true, deterministic ->
                // false); pass it through.
                jobs::fail(&conn, job_id, &e.message, e.retryable)?;
                let _ = jobs::log_error(
                    &conn,
                    "workers",
                    &format!("copilot_chat failed: {}", e.message),
                );
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn run_golden_job(&self) -> Result<()> {
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "golden_eval"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Classify),
            )
        };
        if !enabled {
            tracing::info!("golden eval feature off - skipped");
            return Ok(());
        }
        let db = self.db.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::ai_eval::run_golden_set(&conn, &backend)
                    .await
                    .map_err(|e| Error::Other(e.to_string().into()))
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?;
        let summary = outcome?;
        {
            let conn = self.lock();
            let _ = crate::ai_eval::store_golden_fingerprint(&conn);
        }
        tracing::info!(
            operation = "golden_eval",
            passed = summary.get("passed").and_then(|v| v.as_i64()).unwrap_or(0),
            failed = summary.get("failed").and_then(|v| v.as_i64()).unwrap_or(0),
            "golden set run finished"
        );
        use crate::events::ServerEvent;
        self.bus.emit(&ServerEvent::conversation_updated(
            None,
            None,
            None,
            None,
            "golden_eval",
        ));
        Ok(())
    }

    async fn run_gap_rebuild_job(&self) -> Result<()> {
        let enabled = {
            let conn = self.lock();
            crate::ai_hub::ai_feature_enabled(&conn, "gap_detection")
        };
        if !enabled {
            tracing::info!("gap detection feature off - skipped");
            return Ok(());
        }
        let db = self.db.clone();
        let (friction_scanned, friction_created, gaps_scanned, gaps_created) =
            tokio::task::spawn_blocking(move || {
                let conn = db.long_op_guard();
                let (scanned, created) = crate::quality::rebuild_friction(&conn);
                let (g_scanned, g_created) = crate::quality::rebuild_gaps(&conn, 90);
                (scanned, created, g_scanned, g_created)
            })
            .await
            .map_err(|e| Error::Other(e.to_string().into()))?;
        tracing::info!(
            operation = "rebuild_gaps",
            friction_scanned,
            friction_created,
            gaps_scanned,
            gaps_created,
            "gap detection pipeline finished"
        );
        use crate::events::ServerEvent;
        self.bus.emit(&ServerEvent::conversation_updated(
            None,
            None,
            None,
            None,
            "rebuild_gaps",
        ));
        Ok(())
    }

    async fn run_draft_article_job(&self, job_id: i64, payload: &serde_json::Value) -> Result<()> {
        let candidate_id = payload
            .get("candidateId")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if candidate_id <= 0 {
            let conn = self.lock();
            jobs::fail(
                &conn,
                job_id,
                "draft_article: no candidateId in payload",
                false,
            )?;
            return Ok(());
        }
        let (enabled, backend) = {
            let conn = self.lock();
            (
                crate::ai_hub::ai_feature_enabled(&conn, "article_drafting"),
                crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Draft),
            )
        };
        if !enabled || matches!(backend, crate::ai_pipeline::AiBackend::Disabled) {
            tracing::info!(
                operation = "draft_article",
                "article draft skipped - feature off or no Draft role endpoint routed"
            );
            return Ok(());
        }
        let db = self.db.clone();
        let draft_id = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::ai_pipeline::draft_article(&conn, &backend, candidate_id).await
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?
        .map_err(|e| Error::Other(e.message.into()))?;
        tracing::info!(
            operation = "draft_article",
            draft_id,
            candidate_id,
            "article draft queued for review"
        );
        use crate::events::ServerEvent;
        self.bus.emit(&ServerEvent::conversation_updated(
            None,
            None,
            None,
            None,
            "draft_article",
        ));
        Ok(())
    }

    async fn run_publish_article_draft_job(
        &self,
        job_id: i64,
        payload: &serde_json::Value,
    ) -> Result<()> {
        let draft_id = payload.get("draftId").and_then(|v| v.as_i64()).unwrap_or(0);
        if draft_id <= 0 {
            let conn = self.lock();
            jobs::fail(
                &conn,
                job_id,
                "publish_article_draft: no draftId in payload",
                false,
            )?;
            return Ok(());
        }
        let (draft, collection_remote) = {
            let conn = self.lock();
            let Some(draft) = crate::knowledge_store::article_draft_by_id(&conn, draft_id) else {
                jobs::fail(
                    &conn,
                    job_id,
                    &format!("Article draft {draft_id} does not exist."),
                    false,
                )?;
                return Ok(());
            };
            if draft.status != "approved" {
                jobs::fail(
                    &conn,
                    job_id,
                    &format!(
                        "Article draft {draft_id} is not approved - the review queue is the \
                         only path to publishing."
                    ),
                    false,
                )?;
                return Ok(());
            }
            let collection_remote: Option<String> = draft
                .collection_local_id
                .or_else(|| payload.get("collectionLocalId").and_then(|v| v.as_i64()))
                .and_then(|local| {
                    conn.query_row(
                        "SELECT remote_id FROM docs_collections WHERE id = ?1",
                        rusqlite::params![local],
                        |r| r.get::<_, String>(0),
                    )
                    .ok()
                });
            (draft, collection_remote)
        };
        let Some(collection_remote) = collection_remote else {
            let conn = self.lock();
            let _ = crate::knowledge_store::set_article_draft_publish_state(
                &conn,
                draft_id,
                "publish_failed",
                None,
                None,
                Some("No Help Center collection selected for the article."),
            );
            jobs::fail(
                &conn,
                job_id,
                "No Help Center collection selected for the article.",
                false,
            )?;
            return Ok(());
        };

        let input = crate::helpscout::DocArticleDraftInput {
            collection_id: collection_remote.clone(),
            name: draft.title.clone(),
            text: draft.body.clone(),
            remote_id: draft.remote_article_id,
            slug: None,
        };
        let remote_id = match self.provider.upsert_doc_article_draft(&input).await {
            Ok(id) => id,
            Err(e) => {
                let msg = e.to_string();
                let conn = self.lock();
                let _ = crate::knowledge_store::set_article_draft_publish_state(
                    &conn,
                    draft_id,
                    "publish_failed",
                    None,
                    None,
                    Some(&msg),
                );
                return Err(e);
            }
        };
        {
            let conn = self.lock();
            crate::knowledge_store::set_article_draft_publish_state(
                &conn,
                draft_id,
                "draft_upserted",
                Some(remote_id.clone()),
                None,
                None,
            )?;
        }

        if let Err(e) = self
            .provider
            .publish_doc_article(remote_id.clone(), &draft.title, &draft.body)
            .await
        {
            let msg = e.to_string();
            let conn = self.lock();
            let _ = crate::knowledge_store::set_article_draft_publish_state(
                &conn,
                draft_id,
                "publish_failed",
                Some(remote_id),
                None,
                Some(&msg),
            );
            return Err(e);
        }
        {
            let conn = self.lock();
            crate::knowledge_store::set_article_draft_publish_state(
                &conn,
                draft_id,
                "published",
                Some(remote_id.clone()),
                None,
                None,
            )?;
        }

        match self.provider.get_doc_article(remote_id.clone()).await {
            Ok(Some(article)) => {
                let collection_local = {
                    let conn = self.lock();
                    crate::sync_engine::local_id_str(&conn, "docs_collections", &collection_remote)
                };
                if let Some(col_local) = collection_local {
                    let mut conn = self.lock();
                    crate::sync_engine::upsert_doc_article(&mut conn, &article, col_local)?;
                }
                let conn = self.lock();
                crate::knowledge_store::set_article_draft_publish_state(
                    &conn,
                    draft_id,
                    "verified",
                    Some(remote_id.clone()),
                    None,
                    None,
                )?;
            }
            verify => {
                let msg = match verify {
                    Ok(None) => {
                        "The article published but the verify fetch found no article.".to_string()
                    }
                    Err(ref e) => format!("Published, but the verify re-sync failed: {e}"),
                    _ => unreachable!(),
                };
                tracing::warn!(operation = "publish_article_draft", draft_id, %msg);
                let conn = self.lock();
                crate::knowledge_store::set_article_draft_publish_state(
                    &conn,
                    draft_id,
                    "published",
                    Some(remote_id.clone()),
                    None,
                    Some(&msg),
                )?;
            }
        }
        tracing::info!(
            operation = "publish_article_draft",
            draft_id,
            remote_id = %remote_id,
            "article published and verified"
        );
        use crate::events::ServerEvent;
        self.bus.emit(&ServerEvent::conversation_updated(
            None,
            None,
            None,
            None,
            "publish_article_draft",
        ));
        Ok(())
    }

    async fn run_weekly_digest_job(&self, payload: &serde_json::Value) -> Result<()> {
        let enabled = {
            let conn = self.lock();
            crate::ai_hub::ai_feature_enabled(&conn, "weekly_digest")
        };
        if !enabled {
            tracing::info!("weekly digest feature off - skipped");
            return Ok(());
        }
        let trigger_kind: String = payload
            .get("triggerKind")
            .and_then(|v| v.as_str())
            .unwrap_or("manual")
            .to_string();
        let days = payload.get("days").and_then(|v| v.as_i64()).unwrap_or(7);
        let backend = {
            let conn = self.lock();
            crate::ai_hub::backend_for_role(&conn, crate::ai_hub::AiRole::Summarize)
        };
        let db = self.db.clone();
        let trigger_kind_label = trigger_kind.clone();
        let digest_id = tokio::task::spawn_blocking(move || {
            let conn = db.long_op_guard();
            block_ai_pipeline(async {
                crate::ai_pipeline::weekly_digest(&conn, &backend, &trigger_kind, days).await
            })
        })
        .await
        .map_err(|e| Error::Other(e.to_string().into()))?
        .map_err(|e| Error::Other(e.message.into()))?;
        tracing::info!(
            operation = "weekly_digest",
            digest_id,
            trigger_kind = %trigger_kind_label,
            "digest archived"
        );
        use crate::events::ServerEvent;
        self.bus.emit(&ServerEvent::conversation_updated(
            None,
            None,
            None,
            None,
            "weekly_digest",
        ));
        Ok(())
    }

    async fn run_fetch_hs_report_job(
        &self,
        job_id: i64,
        payload: &serde_json::Value,
    ) -> Result<()> {
        let key: String = payload
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let start: String = payload
            .get("start")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let end: String = payload
            .get("end")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if crate::reports_api::report_spec(&key).is_none() {
            let conn = self.lock();
            jobs::fail(&conn, job_id, "fetch_hs_report: unknown report key", false)?;
            return Ok(());
        }
        if self.provider.kind() == "empty" {
            let conn = self.lock();
            crate::reports_api::set_capability(
                &conn,
                crate::reports_api::ReportsCapability::NotConnected,
            )?;
            tracing::info!(
                operation = "fetch_hs_report",
                "no Help Scout connection - verdict recorded"
            );
            return Ok(());
        }
        match self.provider.get_report(&key, &start, &end).await {
            Ok(Some(row)) => {
                let conn = self.lock();
                crate::reports_api::cache_report(&conn, &key, &start, &end, &row.data)?;
                crate::reports_api::set_capability(
                    &conn,
                    crate::reports_api::ReportsCapability::Available,
                )?;
                tracing::info!(
                    operation = "fetch_hs_report",
                    key,
                    "report fetched and cached"
                );
            }
            Ok(None) => {
                let conn = self.lock();
                crate::reports_api::set_capability(
                    &conn,
                    crate::reports_api::ReportsCapability::NotConnected,
                )?;
            }
            Err(e) => {
                let status = if let crate::error::Error::Other(boxed) = &e {
                    boxed
                        .downcast_ref::<crate::helpscout_real::HsApiError>()
                        .map(|hs| hs.status_code)
                } else {
                    None
                };
                let conn = self.lock();
                if let Some(cap) = crate::reports_api::classify_fetch_error(status.unwrap_or(0)) {
                    crate::reports_api::set_capability(&conn, cap)?;
                }
                return Err(e);
            }
        }
        use crate::events::ServerEvent;
        self.bus.emit(&ServerEvent::conversation_updated(
            None,
            None,
            None,
            None,
            "fetch_hs_report",
        ));
        Ok(())
    }

    fn mailbox_local_id(conn: &rusqlite::Connection, remote_id: i64) -> i64 {
        conn.query_row(
            "SELECT id FROM mailboxes WHERE remote_id = ?1",
            rusqlite::params![remote_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    async fn run_send_chat_message_job(
        &self,
        job_id: i64,
        chat_id: &str,
        message_id: &str,
        text: &str,
    ) -> Result<()> {
        match self
            .provider
            .send_chat_message(chat_id, message_id, text)
            .await
        {
            Ok(()) => {
                {
                    let conn = self.lock();
                    crate::chats::record_outgoing_message(&conn, chat_id, message_id, text)?;
                }
                tracing::info!(operation = "send_chat_message", chat_id, "message sent");
                use crate::events::ServerEvent;
                self.bus.emit(&ServerEvent::conversation_updated(
                    None,
                    None,
                    None,
                    None,
                    "send_chat_message",
                ));
                Ok(())
            }
            Err(e) => {
                let terminal = matches!(
                    &e,
                    Error::Other(b)
                        if b.downcast_ref::<crate::helpscout_real::HsApiError>()
                            .is_some_and(|hs| hs.status_code == 400 || hs.status_code == 404)
                );
                if terminal {
                    let msg = e.to_string();
                    let conn = self.lock();
                    jobs::fail(&conn, job_id, &msg, false)?;
                    return Ok(());
                }
                Err(e)
            }
        }
    }

    async fn run_saved_reply_write_job(
        &self,
        job_id: i64,
        payload: &serde_json::Value,
    ) -> Result<()> {
        let action = payload
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let mailbox_remote_id = payload
            .get("mailboxRemoteId")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let name = payload
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let text = payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let reply_remote_id = payload.get("replyRemoteId").and_then(|v| v.as_i64());

        match action.as_str() {
            "create" => {
                if name.trim().is_empty() {
                    let conn = self.lock();
                    jobs::fail(
                        &conn,
                        job_id,
                        "saved reply requires a name (the platform rejects a blank one)",
                        false,
                    )?;
                    return Ok(());
                }
                let existing = self.provider.list_saved_replies(mailbox_remote_id).await?;
                let already_landed = existing
                    .iter()
                    .any(|r| r.name == name && r.text.clone().unwrap_or_default() == text);
                if already_landed {
                    tracing::info!(
                        operation = "saved_reply_write",
                        mailbox = mailbox_remote_id,
                        "create already landed on the platform; skipping the re-POST"
                    );
                } else {
                    self.provider
                        .create_saved_reply(mailbox_remote_id, &name, &text)
                        .await?;
                }
            }
            "update" => {
                if name.trim().is_empty() {
                    let conn = self.lock();
                    jobs::fail(
                        &conn,
                        job_id,
                        "saved reply requires a name (the platform rejects a blank one)",
                        false,
                    )?;
                    return Ok(());
                }
                let Some(reply_id) = reply_remote_id else {
                    let conn = self.lock();
                    jobs::fail(
                        &conn,
                        job_id,
                        "saved reply update missing replyRemoteId",
                        false,
                    )?;
                    return Ok(());
                };
                self.provider
                    .update_saved_reply(mailbox_remote_id, reply_id, &name, &text)
                    .await?;
            }
            "delete" => {
                let Some(reply_id) = reply_remote_id else {
                    let conn = self.lock();
                    jobs::fail(
                        &conn,
                        job_id,
                        "saved reply delete missing replyRemoteId",
                        false,
                    )?;
                    return Ok(());
                };
                self.provider
                    .delete_saved_reply(mailbox_remote_id, reply_id)
                    .await?;
            }
            other => {
                let conn = self.lock();
                jobs::fail(
                    &conn,
                    job_id,
                    &format!("saved_reply_write: unknown action {other}"),
                    false,
                )?;
                return Ok(());
            }
        }

        let replies = self.provider.list_saved_replies(mailbox_remote_id).await?;
        {
            let conn = self.lock();
            let mailbox_local = Self::mailbox_local_id(&conn, mailbox_remote_id);
            let mailbox_local = (mailbox_local > 0).then_some(mailbox_local);
            for reply in &replies {
                crate::sync_engine::upsert_saved_reply(&conn, mailbox_local, reply)?;
            }
            tracing::info!(
                operation = "saved_reply_write",
                action = %action,
                reread = replies.len(),
                "saved reply written; platform list re-read"
            );
        }
        use crate::events::ServerEvent;
        self.bus.emit(&ServerEvent::sync_completed(
            "incremental",
            0,
            0,
            crate::events::SyncTrigger::Refresh,
        ));
        Ok(())
    }

    async fn run_workflow_job(&self, job_id: i64, payload: &serde_json::Value) -> Result<()> {
        let workflow_remote_id = payload
            .get("workflowRemoteId")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let number = payload
            .get("conversationNumber")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        if workflow_remote_id <= 0 || number <= 0 {
            let conn = self.lock();
            jobs::fail(
                &conn,
                job_id,
                "run_workflow missing workflowRemoteId/conversationNumber",
                false,
            )?;
            return Ok(());
        }
        let conversation_remote_id: Option<i64> = {
            let conn = self.lock();
            conn.query_row(
                "SELECT remote_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
                rusqlite::params![number],
                |r| r.get(0),
            )
            .ok()
        };
        let Some(conversation_remote_id) = conversation_remote_id else {
            let conn = self.lock();
            jobs::fail(
                &conn,
                job_id,
                &format!("conversation #{number} is not in the local mirror (run a sync first)"),
                false,
            )?;
            return Ok(());
        };
        match self
            .provider
            .run_workflow(workflow_remote_id, conversation_remote_id)
            .await
        {
            Ok(ran) => {
                tracing::info!(
                    operation = "run_workflow",
                    workflow_remote_id,
                    conversation_remote_id,
                    ran,
                    "workflow run requested"
                );
                use crate::events::ServerEvent;
                self.bus.emit(&ServerEvent::conversation_updated(
                    None,
                    None,
                    None,
                    None,
                    "run_workflow",
                ));
                Ok(())
            }
            Err(e) => {
                let terminal = matches!(
                    &e,
                    Error::Other(b)
                        if b.downcast_ref::<crate::helpscout_real::HsApiError>()
                            .is_some_and(|hs| hs.status_code == 400 || hs.status_code == 404)
                );
                if terminal {
                    let msg = e.to_string();
                    let conn = self.lock();
                    jobs::fail(&conn, job_id, &msg, false)?;
                    return Ok(());
                }
                Err(e)
            }
        }
    }

    async fn run_webhook_health_job(&self) -> Result<()> {
        if self.provider.kind() == "empty" {
            let conn = self.lock();
            crate::webhook_health::store_outcome(
                &conn,
                &crate::webhook_health::not_configured_outcome(
                    "no Help Scout connection — the webhook guard has nothing to check",
                ),
            )?;
            return Ok(());
        }
        let (url, secret) = {
            let conn = self.lock();
            let url = crate::settings::get_string(&conn, "webhook_public_url")
                .ok()
                .flatten()
                .unwrap_or_default();
            let secret = crate::settings::get_secret(&conn, "webhook_secret")
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            (url, secret)
        };
        if url.trim().is_empty() {
            let conn = self.lock();
            crate::webhook_health::store_outcome(
                &conn,
                &crate::webhook_health::not_configured_outcome(
                    "no relay URL configured — the listener stays loopback-only until you run a tunnel",
                ),
            )?;
            return Ok(());
        }
        if secret.trim().is_empty() {
            let conn = self.lock();
            crate::webhook_health::store_outcome(
                &conn,
                &crate::webhook_health::failed_outcome(
                    "relay URL is configured but the webhook secret is empty — \
                     refusing to register a hook that would fail signature \
                     verification on every delivery. Set the webhook secret in \
                     Settings (same secret you register on the Help Scout \
                     webhook) and run the check again.",
                ),
            )?;
            tracing::warn!(
                operation = "webhook_health_check",
                "webhook relay misconfigured: URL set but secret empty — registration skipped"
            );
            return Ok(());
        }
        match crate::webhook_health::run_check(self.provider.as_ref(), &url, &secret).await {
            Ok(outcome) => {
                let healed = outcome.is_healed();
                {
                    let conn = self.lock();
                    crate::webhook_health::store_outcome(&conn, &outcome)?;
                }
                tracing::info!(
                    operation = "webhook_health_check",
                    verdict = %outcome.verdict,
                    "webhook health checked"
                );
                if outcome.verdict == "failed" {
                    let conn = self.lock();
                    let _ = crate::audit::log_event(
                        &conn,
                        "error",
                        "webhook",
                        &format!("Webhook health check failed — {}", outcome.detail),
                        None,
                        None,
                    );
                } else if healed {
                    let conn = self.lock();
                    let _ = crate::audit::log_event(
                        &conn,
                        "info",
                        "webhook",
                        &format!("Webhook healed — {}", outcome.detail),
                        None,
                        None,
                    );
                }
                use crate::events::ServerEvent;
                if healed {
                    self.bus.emit(&ServerEvent::error_occurred(
                        "warn",
                        "the webhook was re-registered (it had been dropped or disabled)",
                        "connect",
                        None,
                    ));
                } else {
                    self.bus.emit(&ServerEvent::sync_completed(
                        "incremental",
                        0,
                        0,
                        crate::events::SyncTrigger::Refresh,
                    ));
                }
                Ok(())
            }
            Err(e) => {
                let outcome = crate::webhook_health::failed_outcome(&format!(
                    "the health check could not run: {e}"
                ));
                let conn = self.lock();
                crate::webhook_health::store_outcome(&conn, &outcome)?;
                let _ = crate::audit::log_event(
                    &conn,
                    "error",
                    "webhook",
                    &format!("Webhook health check failed — {}", outcome.detail),
                    None,
                    None,
                );
                drop(conn);
                // AU2-11: the failed run also stored a fresh outcome — pages
                // (Settings wh_outcome, SyncHealth) must be told to reload it.
                // Same refresh signal the non-healed success path emits.
                use crate::events::ServerEvent;
                self.bus.emit(&ServerEvent::sync_completed(
                    "incremental",
                    0,
                    0,
                    crate::events::SyncTrigger::Refresh,
                ));
                Err(e)
            }
        }
    }

    pub(super) async fn run_attachment_downloads(&self) {
        let rows: Vec<(i64, i64, Option<i64>, Option<i64>)> = {
            let conn = self.lock();
            let Ok(mut stmt) = conn.prepare(
                "SELECT a.id, a.conversation_id, a.thread_id, a.remote_id
                   FROM attachments a
                   JOIN conversations c ON c.id = a.conversation_id
                  WHERE (a.state IS NULL OR a.state NOT IN ('downloaded'))
                    AND c.deleted_at IS NULL
                  ORDER BY a.id DESC LIMIT 100",
            ) else {
                return;
            };
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        };
        if rows.is_empty() {
            return;
        }
        let attachments_dir = self.data_dir.join("attachments");
        let mut downloaded = 0usize;
        for (id, conversation_id, thread_id, att_remote) in rows {
            let (conv_remote, thread_remote): (i64, Option<i64>) = {
                let conn = self.lock();
                let conv: i64 = conn
                    .query_row(
                        "SELECT remote_id FROM conversations WHERE id = ?1",
                        rusqlite::params![conversation_id],
                        |r| r.get(0),
                    )
                    .unwrap_or_default();
                let t: Option<i64> = thread_id.and_then(|tid| {
                    conn.query_row(
                        "SELECT remote_id FROM conversation_threads WHERE id = ?1",
                        rusqlite::params![tid],
                        |r| r.get(0),
                    )
                    .ok()
                });
                (conv, t)
            };
            match self
                .provider
                .get_attachment_data(
                    conv_remote,
                    thread_remote.unwrap_or(0),
                    att_remote.unwrap_or(0),
                )
                .await
            {
                Ok(Some(bytes)) => {
                    let conn = self.lock();
                    match crate::conversation_ops::persist_attachment_bytes(
                        &conn,
                        &attachments_dir,
                        id,
                        &bytes.data,
                    ) {
                        Ok(_) => downloaded += 1,
                        Err(msg) => {
                            tracing::warn!(attachment_id = id, error = %msg, "Attachment download failed");
                        }
                    }
                }
                Ok(None) => {
                    let conn = self.lock();
                    crate::conversation_ops::set_attachment_state(&conn, id, "failed", None, None);
                    tracing::warn!(attachment_id = id, "Attachment no longer available");
                }
                Err(e) => {
                    let conn = self.lock();
                    crate::conversation_ops::set_attachment_state(&conn, id, "failed", None, None);
                    tracing::warn!(attachment_id = id, error = %e, "Attachment download failed");
                }
            }
        }
        tracing::info!(count = downloaded, "Recent attachments downloaded");
    }
}
#[derive(Clone, Copy)]
enum AiJob {
    Analyze,
    Draft,
    Note,
}
