use super::*;
mod ai;
mod ops;

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
                    self.sync_conversation_ratings_job(rating_id, conv_remote).await?;
                }
                "embed_knowledge_chunks" => {
                    self.embed_chunks_job(
                        kind,
                        crate::embeddings::embed_pending_knowledge,
                    );
                }
                "embed_docs_chunks" => {
                    self.embed_chunks_job(kind, crate::embeddings::embed_pending_docs);
                }
                "embed_conversation_chunks" => {
                    self.embed_chunks_job(
                        kind,
                        crate::embeddings::embed_pending_conversation_chunks,
                    );
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
                        self.automation_approval_job(action, rule_id, conversation_id)?;
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
                let ids: Vec<i64> = stmt
                    .query_map([], |r| r.get::<_, i64>(0))
                    .map(crate::db::rows_ok)
                    .unwrap_or_default();
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
            let ids: Vec<i64> = stmt
                .query_map([], |r| r.get::<_, i64>(0))
                .map(crate::db::rows_ok)
                .unwrap_or_default();
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
            if let Some((trigger, newest_customer_thread)) = trigger {
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
}
#[derive(Clone, Copy)]
enum AiJob {
    Analyze,
    Draft,
    Note,
}
