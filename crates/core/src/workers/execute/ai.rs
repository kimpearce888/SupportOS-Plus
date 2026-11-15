use super::*;

impl WorkerManager {
    pub(super) async fn run_ai_job(&self, kind: &str, conv_id: i64, job: AiJob) -> Result<()> {
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

    pub(super) async fn run_attributes_job(&self, conv_id: i64) -> Result<()> {
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

    pub(super) async fn run_memory_job(&self, conv_id: i64) -> Result<()> {
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

    pub(super) async fn run_clustering_job(&self) -> Result<()> {
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

    pub(super) async fn run_rewrite_job(
        &self,
        payload: &serde_json::Value,
    ) -> Result<Option<String>> {
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

    pub(super) async fn run_translate_job(
        &self,
        payload: &serde_json::Value,
    ) -> Result<Option<String>> {
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
                            temperature: f64,
                            max_tokens: u32|
                 -> TranslateChatFuture<'_> {
                    let called_ref = called_ref.clone();
                    let usage_ref = usage_ref.clone();
                    Box::pin(async move {
                        let res = backend_ref
                            .chat_qa(msgs, temperature, max_tokens, false)
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
                Err(e) => Err(e.0),
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

    pub(super) async fn run_coaching_job(
        &self,
        conv_id: i64,
        payload: &serde_json::Value,
    ) -> Result<()> {
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
                            temperature: f64,
                            max_tokens: u32,
                            json_mode: bool|
                 -> CoachingChatFuture<'_> {
                    Box::pin(async move {
                        let res = backend_ref
                            .chat_qa(msgs, temperature, max_tokens, json_mode)
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

    pub(super) async fn run_qa_ai_job(&self, conv_id: i64) -> Result<()> {
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

    pub(super) async fn run_interaction_job(&self, conv_id: i64) -> Result<()> {
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

    pub(super) async fn run_copilot_chat_job(
        &self,
        job_id: i64,
        payload: &serde_json::Value,
    ) -> Result<()> {
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
}
