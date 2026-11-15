use super::*;

impl WorkerManager {
    pub(super) async fn sync_conversation_ratings_job(
        &self,
        rating_id: Option<i64>,
        conv_remote: Option<i64>,
    ) -> Result<()> {
        if let Some(rating_id) = rating_id {
            match self.provider.get_rating(rating_id).await {
                Ok(Some(r)) => {
                    let (inserted, conv_local, conv_number, customer_local) = {
                        let conn = self.lock();
                        let conv_local = r.conversation_id.and_then(|id| {
                            if id > 0 {
                                crate::sync_engine::conversation_local_id(&conn, id)
                            } else {
                                None
                            }
                        });
                        let conv_number = conv_local.and_then(|local| {
                            conn.query_row(
                                "SELECT number FROM conversations WHERE id = ?1",
                                rusqlite::params![local],
                                |row| row.get::<_, i64>(0),
                            )
                            .ok()
                        });
                        let customer_local = r.customer_id.and_then(|id| {
                            if id > 0 {
                                crate::sync_engine::local_id(&conn, "customers", id)
                            } else {
                                None
                            }
                        });
                        let user_local = r.user_id.and_then(|id| {
                            if id > 0 {
                                crate::sync_engine::local_id(&conn, "users", id)
                            } else {
                                None
                            }
                        });
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
        Ok(())
    }
    pub(super) fn embed_chunks_job(
        &self,
        kind: &str,
        embed: fn(
            &rusqlite::Connection,
            Option<&crate::vectorstore_sqlitevec::VectorIndex>,
        ) -> crate::error::Result<usize>,
    ) {
        let conn = self.db.long_op_guard();
        let n = embed(&conn, self.qdrant.as_deref()).unwrap_or(0);
        tracing::debug!(operation = kind, embedded = n, "embedding pass");
        if crate::embeddings::pass_hit_cap(n)
            && !crate::jobs::has_pending(&conn, "embeddings", kind).unwrap_or(false)
        {
            let _ = crate::jobs::enqueue_on(&conn, "embeddings", kind, "{}", 4, 2);
        }
    }
    pub(super) fn automation_approval_job(
        &self,
        action: Option<serde_json::Value>,
        rule_id: Option<i64>,
        conversation_id: Option<i64>,
    ) -> Result<()> {
        if let (Some(action), Some(rule_id), Some(conversation_id)) =
            (action, rule_id, conversation_id)
        {
            let executed = {
                let mut conn = self.lock();
                crate::automation::execute_approved_action(&mut conn, conversation_id, &action)?
            };
            if executed {
                let kind = action
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or_default()
                    .to_string();
                let conn = self.lock();
                crate::automation::record_approved_run(&conn, rule_id, conversation_id, &kind)?;
            } else {
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
        Ok(())
    }

    pub(super) async fn run_golden_job(&self) -> Result<()> {
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

    pub(super) async fn run_gap_rebuild_job(&self) -> Result<()> {
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

    pub(super) async fn run_draft_article_job(
        &self,
        job_id: i64,
        payload: &serde_json::Value,
    ) -> Result<()> {
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

    pub(super) async fn run_publish_article_draft_job(
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
                    _ => unreachable!("Ok(Some) is handled by the outer match arm"),
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

    pub(super) async fn run_weekly_digest_job(&self, payload: &serde_json::Value) -> Result<()> {
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

    pub(super) async fn run_fetch_hs_report_job(
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

    pub(super) async fn run_send_chat_message_job(
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

    pub(super) async fn run_saved_reply_write_job(
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

    pub(super) async fn run_workflow_job(
        &self,
        job_id: i64,
        payload: &serde_json::Value,
    ) -> Result<()> {
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

    pub(super) async fn run_webhook_health_job(&self) -> Result<()> {
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
                    crate::webhook_health::NO_RELAY_DETAIL,
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
            crate::db::query_rows(
                &conn,
                "SELECT a.id, a.conversation_id, a.thread_id, a.remote_id
                   FROM attachments a
                   JOIN conversations c ON c.id = a.conversation_id
                  WHERE (a.state IS NULL OR a.state NOT IN ('downloaded'))
                    AND c.deleted_at IS NULL
                  ORDER BY a.id DESC LIMIT 100",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
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
