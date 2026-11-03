use super::*;

impl SyncEngine {
    pub(super) fn finish_resource(
        &self,
        resource: &str,
        r: Result<ResourceSyncResult>,
        results: &mut Vec<ResourceSyncResult>,
    ) -> ResourceSyncResult {
        match &r {
            Ok(res) if res.error.is_some() => {
                let conn = self.lock();
                record_failure(
                    &conn,
                    resource,
                    res.error.as_deref().unwrap_or(""),
                    res.failed,
                );
                self.emit_error(
                    "warn",
                    "sync",
                    &format!(
                        "Sync resource {resource} failed: {}",
                        res.error.as_deref().unwrap_or("unknown error")
                    ),
                    None,
                );
            }
            Ok(res) => {
                let conn = self.lock();
                record_success(&conn, resource, res.processed);
            }
            Err(e) => {
                let conn = self.lock();
                record_failure(&conn, resource, &e.to_string(), 1);
                self.emit_error(
                    "warn",
                    "sync",
                    &format!("Sync resource {resource} failed: {e}"),
                    None,
                );
            }
        }
        let r = r.unwrap_or(ResourceSyncResult {
            resource: resource.to_string(),
            processed: 0,
            failed: 1,
            error: Some("resource sync failed".into()),
        });
        results.push(r.clone());
        r
    }

    pub async fn reconcile(&self) -> Result<ReconciliationResult> {
        self.guard_provider_connected()?;
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(Error::Other("A sync is already running".into()));
        }
        let _running = RunningGuard(&self.running);
        let run_id = {
            let conn = self.lock();
            set_state(&conn, "RECONCILING");
            start_run(&conn, "reconciliation")?
        };
        let mut result = ReconciliationResult::default();
        let outcome: Result<()> = async {
            let mut remote_ids: Vec<i64> = Vec::new();
            let mut cursor: Option<String> = None;
            loop {
                let page = self
                    .provider
                    .list_conversations(&ConversationQuery {
                        status: Some("all".into()),
                        cursor: cursor.clone(),
                        ..Default::default()
                    })
                    .await?;
                for c in &page.items {
                    remote_ids.push(c.remote_id);
                    let existing = {
                        let conn = self.lock();
                        conversation_local_id(&conn, c.remote_id)
                    };
                    match existing {
                        None => {
                            self.ingest_conversation(c, &[], true).await?;
                            result.added += 1;
                            result
                                .details
                                .push(format!("Added missing conversation #{}", c.number));
                        }
                        Some(_) => {
                            let unchanged = {
                                let conn = self.lock();
                                stored_user_updated_at_matches(&conn, c.remote_id, c.updated_at.as_deref())
                            };
                            if unchanged {
                                result.skipped += 1;
                            } else {
                                self.ingest_conversation(c, &[], true).await?;
                                result.updated += 1;
                            }
                        }
                    }
                    result.checked += 1;
                }
                match page.next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }

            let locals: Vec<(i64, i64, i64)> = {
                let conn = self.lock();
                let mut stmt = conn.prepare(
                    "SELECT id, remote_id, number FROM conversations WHERE deleted_at IS NULL",
                )?;
                let rows = stmt.query_map([], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
                })?;
                rows.filter_map(|r| r.ok()).collect()
            };
            for (local_id, remote_id, number) in locals {
                result.checked += 1;
                if remote_ids.contains(&remote_id) {
                    result.skipped += 1;
                    continue;
                }
                match self.provider.get_conversation(remote_id).await {
                    Ok(Some(remote)) => {
                        self.ingest_conversation(&remote, &[], true).await?;
                        result.updated += 1;
                        result
                            .details
                            .push(format!("Refreshed stale conversation #{number}"));
                    }
                    Ok(None) => {
                        let conn = self.lock();
                        soft_delete_by_remote_id(&conn, remote_id)?;
                        result.deleted += 1;
                        result.details.push(format!(
                            "Conversation #{number} no longer exists remotely - marked deleted locally"
                        ));
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("merged into") {
                            let target = msg
                                .split_whitespace()
                                .position(|w| w == "into")
                                .and_then(|at| {
                                    msg.split_whitespace()
                                        .nth(at + 1)
                                        .and_then(|w| w.parse::<i64>().ok())
                                });
                            let marked = {
                                let conn = self.lock();
                                match target.and_then(|t| conversation_local_id(&conn, t)) {
                                    Some(target_local) => conn
                                        .execute(
                                            "UPDATE conversations SET merged_into_conversation_id = ?1,
                                                updated_at = datetime('now')
                                              WHERE id = ?2",
                                            params![target_local, local_id],
                                        )
                                        .is_ok(),
                                    None => false,
                                }
                            };
                            let _ = marked;
                            result.merged += 1;
                            result
                                .details
                                .push(format!("Conversation #{number} merged into another conversation"));
                        } else {
                            result.failed += 1;
                        }
                    }
                }
            }

            let fts_exists: bool = {
                let conn = self.lock();
                conn.query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'fts_threads'",
                    [],
                    |_| Ok(()),
                )
                .is_ok()
            };
            let orphans: i64 = if fts_exists {
                let conn = self.lock();
                conn.query_row(
                    "SELECT COUNT(*) FROM conversation_threads
                      WHERE (fts_indexed IS NULL OR fts_indexed = 0)
                        AND body_text IS NOT NULL AND LENGTH(body_text) > 0",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0)
            } else {
                0
            };
            if orphans > 0 {
                let conn = self.lock();
                conn.execute(
                    "INSERT INTO fts_threads (body, thread_id, conversation_id)
                     SELECT t.body_text, t.id, t.conversation_id FROM conversation_threads t
                      WHERE (t.fts_indexed IS NULL OR t.fts_indexed = 0)
                        AND t.body_text IS NOT NULL AND LENGTH(t.body_text) > 0
                        AND NOT EXISTS (SELECT 1 FROM fts_threads f WHERE f.thread_id = t.id)",
                    [],
                )?;
                conn.execute(
                    "UPDATE conversation_threads SET fts_indexed = 1
                      WHERE (fts_indexed IS NULL OR fts_indexed = 0)
                        AND body_text IS NOT NULL AND LENGTH(body_text) > 0",
                    [],
                )?;
                result
                    .details
                    .push(format!("Rebuilt FTS index for {orphans} threads"));
            }

            let conn = self.lock();
            set_state(&conn, "LIVE");
            let _ = update_run(
                &conn,
                run_id,
                RunUpdate {
                    state: Some("LIVE"),
                    resources_done: None,
                    resources_total: None,
                    records_processed: None,
                    errors: None,
                    detail: Some(serde_json::to_value(&result).unwrap_or(Value::Null)),
                    finished: true,
                },
            );
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            let conn = self.lock();
            set_state(&conn, "ERROR");
            let _ = update_run(
                &conn,
                run_id,
                RunUpdate {
                    state: Some("ERROR"),
                    resources_done: None,
                    resources_total: None,
                    records_processed: None,
                    errors: None,
                    detail: Some(json!({ "error": e.to_string() })),
                    finished: true,
                },
            );
            let _ = crate::jobs::log_error(&conn, "sync", &format!("Reconciliation failed: {e}"));
            result.failed += 1;
        }
        Ok(result)
    }

    pub(crate) async fn sync_single_conversation(&self, remote_id: i64) -> Result<bool> {
        self.guard_provider_connected()?;
        let remote = self.provider.get_conversation(remote_id).await;
        match remote {
            Ok(Some(conv)) => {
                let threads = self
                    .provider
                    .list_threads(remote_id)
                    .await
                    .unwrap_or_default();
                self.ingest_conversation(&conv, &threads, false).await?;
                Ok(true)
            }
            Ok(None) => {
                let conn = self.lock();
                soft_delete_by_remote_id(&conn, remote_id)?;
                Ok(true)
            }
            Err(e) => {
                let merged_or_gone = crate::helpscout_real::hs_status(&e) == Some(301)
                    || crate::helpscout_real::hs_status(&e) == Some(404)
                    || e.to_string().contains("merged into");
                if merged_or_gone {
                    let conn = self.lock();
                    soft_delete_by_remote_id(&conn, remote_id)?;
                    Ok(true)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub(crate) async fn run_resource(
        &self,
        resource: &str,
        initial: bool,
    ) -> Result<ResourceSyncResult> {
        self.sync_resource(resource, initial).await
    }

    pub(super) async fn sync_resource(
        &self,
        resource: &str,
        initial: bool,
    ) -> Result<ResourceSyncResult> {
        let resource = resource.to_string();
        match resource.as_str() {
            "account" => {
                let me = self.provider.get_me().await?;
                let conn = self.lock();
                conn.execute(
                    "INSERT INTO accounts (remote_id, plan, raw_json, last_seen_at)
                     VALUES (?1, ?2, ?3, datetime('now'))
                     ON CONFLICT(remote_id) DO UPDATE SET plan = excluded.plan,
                       raw_json = excluded.raw_json, last_seen_at = datetime('now')",
                    params![
                        me.remote_id,
                        me.role,
                        serde_json::to_string(&me).unwrap_or_default()
                    ],
                )?;
                mirror::upsert_user(&conn, &me)?;
                Ok(ResourceSyncResult {
                    resource,
                    processed: 1,
                    failed: 0,
                    error: None,
                })
            }
            "users" => {
                let users = self.provider.list_users().await?;
                let conn = self.lock();
                for u in &users {
                    mirror::upsert_user(&conn, u)?;
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed: users.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "system_users" => {
                let users = self.provider.list_system_users().await?;
                let conn = self.lock();
                for u in &users {
                    mirror::upsert_user(&conn, u)?;
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed: users.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "teams" => {
                let teams = self.provider.list_teams().await?;
                let conn = self.lock();
                for t in &teams {
                    mirror::upsert_team(&conn, t)?;
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed: teams.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "mailboxes" => {
                let mbs = self.provider.list_mailboxes().await?;
                let conn = self.lock();
                for m in &mbs {
                    mirror::upsert_mailbox(&conn, m)?;
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed: mbs.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "folders" => {
                let mbs = self.provider.list_mailboxes().await?;
                let mut processed = 0;
                for m in &mbs {
                    let folders = self.provider.list_folders(m.remote_id).await?;
                    let conn = self.lock();
                    if let Some(mailbox_local) = local_id(&conn, "mailboxes", m.remote_id) {
                        for f in &folders {
                            upsert_folder(&conn, mailbox_local, f)?;
                            processed += 1;
                        }
                    }
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed,
                    failed: 0,
                    error: None,
                })
            }
            "tags" => {
                let tags = self.provider.list_tags().await?;
                let conn = self.lock();
                for t in &tags {
                    mirror::upsert_tag(&conn, t)?;
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed: tags.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "inbox_fields" => {
                let mbs = self.provider.list_mailboxes().await?;
                let mut processed = 0;
                for m in &mbs {
                    let fields = self.provider.list_inbox_fields(m.remote_id).await?;
                    let conn = self.lock();
                    if let Some(mailbox_local) = local_id(&conn, "mailboxes", m.remote_id) {
                        for f in &fields {
                            upsert_inbox_field(&conn, mailbox_local, f)?;
                            processed += 1;
                        }
                    }
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed,
                    failed: 0,
                    error: None,
                })
            }
            "customer_property_definitions" => {
                let defs = self.provider.list_customer_property_definitions().await?;
                let conn = self.lock();
                upsert_property_definitions(&conn, "customer_property_definitions", &defs)?;
                Ok(ResourceSyncResult {
                    resource,
                    processed: defs.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "organization_property_definitions" => {
                let defs = self
                    .provider
                    .list_organization_property_definitions()
                    .await?;
                let conn = self.lock();
                upsert_property_definitions(&conn, "organization_property_definitions", &defs)?;
                Ok(ResourceSyncResult {
                    resource,
                    processed: defs.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "organizations" => {
                let orgs = self.provider.list_organizations().await?;
                let conn = self.lock();
                let mut seen: Vec<i64> = Vec::with_capacity(orgs.len());
                for o in &orgs {
                    upsert_organization(&conn, o)?;
                    seen.push(o.remote_id);
                }
                let pruned = prune_missing_rows(&conn, "organizations", &seen)?;
                if pruned > 0 {
                    tracing::debug!(deleted = pruned, "organizations removed platform-side");
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed: orgs.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "customers" => {
                let since = if initial {
                    None
                } else {
                    let conn = self.lock();
                    get_incremental_since(&conn, "customers", SYNC_OVERLAP_MINUTES)
                };
                self.sync_customers(since.as_deref(), initial).await
            }
            "saved_replies" => {
                let mbs = self.provider.list_mailboxes().await?;
                let mut processed = 0;
                let mut seen: Vec<i64> = Vec::new();
                for m in &mbs {
                    let replies = self.provider.list_saved_replies(m.remote_id).await?;
                    let conn = self.lock();
                    let mailbox_local = local_id(&conn, "mailboxes", m.remote_id);
                    for r in &replies {
                        upsert_saved_reply(&conn, mailbox_local, r)?;
                        seen.push(r.remote_id);
                        processed += 1;
                    }
                }
                {
                    let conn = self.lock();
                    let pruned = prune_missing_rows(&conn, "saved_replies", &seen)?;
                    if pruned > 0 {
                        tracing::debug!(deleted = pruned, "saved replies removed platform-side");
                    }
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed,
                    failed: 0,
                    error: None,
                })
            }
            "workflows" => {
                let wfs = self.provider.list_workflows().await?;
                let conn = self.lock();
                let mut seen: Vec<i64> = Vec::with_capacity(wfs.len());
                for w in &wfs {
                    upsert_workflow(&conn, w)?;
                    seen.push(w.remote_id);
                }
                let pruned = prune_missing_rows(&conn, "workflows", &seen)?;
                if pruned > 0 {
                    tracing::debug!(deleted = pruned, "workflows removed platform-side");
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed: wfs.len() as i64,
                    failed: 0,
                    error: None,
                })
            }
            "conversations" => self.sync_conversations(None, initial).await,
            "threads" => self.sync_missing_threads().await,
            "docs_collections" => {
                let collections = self.provider.list_doc_collections().await?;
                let mut processed = 0;
                for col in &collections {
                    let categories = self
                        .provider
                        .list_doc_categories(col.remote_id.clone())
                        .await?;
                    let conn = self.lock();
                    let local = upsert_doc_collection(&conn, col)?;
                    upsert_doc_categories(&conn, local, &categories)?;
                    processed += 1;
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed,
                    failed: 0,
                    error: None,
                })
            }
            "docs_articles" => {
                let mut processed = 0;
                let collections: Vec<String> = {
                    let conn = self.lock();
                    let mut stmt = conn.prepare("SELECT remote_id FROM docs_collections")?;
                    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
                    rows.filter_map(|r| r.ok()).collect()
                };
                for col_remote in collections {
                    let articles = self.provider.list_doc_articles(col_remote.clone()).await?;
                    let mut conn = self.lock();
                    if let Some(col_local) = local_id_str(&conn, "docs_collections", &col_remote) {
                        for a in &articles {
                            upsert_doc_article(&mut conn, a, col_local)?;
                            processed += 1;
                        }
                    }
                }
                if processed > 0 {
                    let conn = self.lock();
                    let _ = crate::jobs::enqueue_on(
                        &conn,
                        "embeddings",
                        "embed_docs_chunks",
                        "{}",
                        4,
                        2,
                    );
                    let _ = crate::jobs::enqueue_on(
                        &conn,
                        "embeddings",
                        "embed_conversation_chunks",
                        "{}",
                        4,
                        2,
                    );
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed,
                    failed: 0,
                    error: None,
                })
            }
            "user_statuses" => {
                let users: Vec<(i64, i64)> = {
                    let conn = self.lock();
                    let mut stmt = conn.prepare("SELECT id, remote_id FROM users")?;
                    let rows =
                        stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
                    rows.filter_map(|r| r.ok()).collect()
                };
                let mut processed = 0;
                for (local, remote) in users {
                    if let Some(status) = self.provider.get_user_status(remote).await? {
                        let conn = self.lock();
                        upsert_user_status(&conn, local, &status)?;
                        processed += 1;
                    }
                }
                Ok(ResourceSyncResult {
                    resource,
                    processed,
                    failed: 0,
                    error: None,
                })
            }
            other => Ok(ResourceSyncResult {
                resource: other.to_string(),
                processed: 0,
                failed: 0,
                error: Some(format!("Unknown resource {other}")),
            }),
        }
    }

    pub(super) async fn sync_customers(
        &self,
        since: Option<&str>,
        initial: bool,
    ) -> Result<ResourceSyncResult> {
        let resource = "customers".to_string();
        let mut processed = 0;
        let mut failed = 0i64;
        let mut cursor = if initial {
            let conn = self.lock();
            get_cursor(&conn, "customers")
        } else {
            None
        };
        let mut pages = 0i64;
        let mut heartbeat = -1i64;
        let mut channel_fetches = 0usize;
        const CHANNEL_FETCH_BUDGET: usize = 400;
        loop {
            if self.cancelled() {
                tracing::info!(resource = "customers", processed, "customer sync cancelled");
                break;
            }
            let page = self
                .provider
                .list_customers(&CustomerQuery {
                    modified_since: since.map(|s| s.to_string()),
                    cursor: cursor.clone(),
                    ..Default::default()
                })
                .await?;

            let mut to_fetch: Vec<i64> = Vec::new();
            let mut bare_payload_ids: std::collections::HashSet<i64> =
                std::collections::HashSet::new();
            {
                let conn = self.lock();
                for c in &page.items {
                    if !c.channels_all_empty() {
                        continue;
                    }
                    bare_payload_ids.insert(c.remote_id);
                    match local_id(&conn, "customers", c.remote_id) {
                        None => to_fetch.push(c.remote_id),
                        Some(local) => {
                            let has_rows: i64 = conn
                                .query_row(
                                    "SELECT
                                        (SELECT COUNT(*) FROM customer_emails WHERE customer_id = ?1)
                                      + (SELECT COUNT(*) FROM customer_phones WHERE customer_id = ?1)
                                      + (SELECT COUNT(*) FROM customer_chats WHERE customer_id = ?1)
                                      + (SELECT COUNT(*) FROM customer_websites WHERE customer_id = ?1)
                                      + (SELECT COUNT(*) FROM customer_social_profiles WHERE customer_id = ?1)",
                                    params![local],
                                    |r| r.get(0),
                                )
                                .unwrap_or(0);
                            if has_rows == 0 && channel_fetches < CHANNEL_FETCH_BUDGET {
                                to_fetch.push(c.remote_id);
                            }
                        }
                    }
                }
            }
            let mut fetched: std::collections::HashMap<i64, crate::helpscout::HsCustomer> =
                std::collections::HashMap::new();
            for rid in &to_fetch {
                match self.provider.get_customer_channels(*rid).await {
                    Ok(Some(ch)) => {
                        channel_fetches += 1;
                        fetched.insert(*rid, ch);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(
                            customer = rid,
                            error = %e,
                            "customer channel fetch failed — syncing the bare profile"
                        );
                        failed += 1;
                    }
                }
            }
            {
                let conn = self.lock();
                for c in &page.items {
                    let outcome = if let Some(ch) = fetched.get(&c.remote_id) {
                        let mut full = c.clone();
                        full.emails = ch.emails.clone();
                        full.phones = ch.phones.clone();
                        full.chats = ch.chats.clone();
                        full.websites = ch.websites.clone();
                        full.social_profiles = ch.social_profiles.clone();
                        mirror::upsert_customer(&conn, &full)
                    } else if bare_payload_ids.contains(&c.remote_id) {
                        mirror::upsert_customer_without_channels(&conn, c)
                    } else {
                        mirror::upsert_customer(&conn, c)
                    };
                    match outcome {
                        Ok(()) => processed += 1,
                        Err(e) => {
                            failed += 1;
                            let _ = crate::jobs::log_error(
                                &conn,
                                "sync",
                                &format!("customer {} failed to upsert: {e}", c.remote_id),
                            );
                        }
                    }
                }
            }
            pages += 1;
            self.emit_progress(
                "customers",
                processed.max(0) as u32,
                0,
                processed,
                page.items.len() as u32,
            );
            if initial && processed / 1_000 > heartbeat {
                heartbeat = processed / 1_000;
                tracing::info!(processed, pages, "customers crawl in progress");
            }
            if initial && pages >= 5_000 {
                tracing::warn!(
                    pages,
                    processed,
                    "customers crawl: safety page cap reached — stopping; the next run resumes from the stored cursor"
                );
                break;
            }
            let empty = page.items.is_empty();
            match page.next_cursor {
                Some(next) => {
                    if initial {
                        let conn = self.lock();
                        set_cursor(&conn, "customers", Some(&next));
                    }
                    cursor = Some(next);
                }
                None => {
                    if initial {
                        let conn = self.lock();
                        set_cursor(&conn, "customers", None);
                    }
                    break;
                }
            }
            if since.is_some() && empty {
                break;
            }
        }
        Ok(ResourceSyncResult {
            resource,
            processed,
            failed,
            error: None,
        })
    }

    pub(super) async fn sync_conversations(
        &self,
        since: Option<&str>,
        initial: bool,
    ) -> Result<ResourceSyncResult> {
        let resource = "conversations".to_string();
        let mut processed = 0;
        let mut cursor = if initial {
            let conn = self.lock();
            get_cursor(&conn, "conversations")
        } else {
            None
        };
        if initial && cursor.is_some() {
            tracing::info!(
                "conversations crawl: resuming from the stored cursor — the previous interrupted run's progress is preserved"
            );
        }
        let mut pages = 0i64;
        let mut heartbeat = -1i64;
        let mut failed = 0i64;
        loop {
            if self.cancelled() {
                tracing::info!(
                    resource = "conversations",
                    processed,
                    "conversation sync cancelled"
                );
                break;
            }
            let page = self
                .provider
                .list_conversations(&ConversationQuery {
                    status: Some("all".into()),
                    modified_since: since.map(|s| s.to_string()),
                    cursor: cursor.clone(),
                    ..Default::default()
                })
                .await?;
            for c in &page.items {
                let threads = if initial {
                    self.provider
                        .list_threads(c.remote_id)
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                if let Err(e) = self.ingest_conversation(c, &threads, !initial).await {
                    failed += 1;
                    tracing::warn!(
                        conversation = c.remote_id,
                        error = %e,
                        "conversation ingest failed — isolated"
                    );
                    let conn = self.lock();
                    let _ = crate::jobs::log_error(
                        &conn,
                        "sync",
                        &format!("conversation {} failed to ingest: {e}", c.remote_id),
                    );
                    continue;
                }
                processed += 1;
                if let Some(bus) = &self.bus
                    && let Some(local) = {
                        let conn = self.lock();
                        conversation_local_id(&conn, c.remote_id)
                    }
                {
                    use crate::events::ServerEvent;
                    let conn = self.lock();
                    let row: Option<(i64, Option<i64>)> = conn
                        .query_row(
                            "SELECT number, mailbox_local_id FROM conversations WHERE id = ?1",
                            params![local],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )
                        .ok();
                    if let Some((number, mailbox_id)) = row {
                        bus.emit(&ServerEvent::conversation_updated(
                            Some(local),
                            Some(number),
                            mailbox_id,
                            c.subject.clone(),
                            "sync",
                        ));
                    }
                }
            }
            self.emit_progress(
                "conversations",
                processed.max(0) as u32,
                0,
                processed,
                page.items.len() as u32,
            );
            pages += 1;
            if initial && processed / 500 > heartbeat {
                heartbeat = processed / 500;
                tracing::info!(
                    processed,
                    pages,
                    "conversations crawl in progress — one threads request per conversation makes this the longest stage of the initial pull"
                );
            }
            if initial && pages >= 5_000 {
                tracing::warn!(
                    pages,
                    processed,
                    "conversations crawl: safety page cap reached — stopping; the next run resumes from the stored cursor"
                );
                break;
            }
            let empty = page.items.is_empty();
            match page.next_cursor {
                Some(next) => {
                    if initial {
                        let conn = self.lock();
                        set_cursor(&conn, "conversations", Some(&next));
                    }
                    cursor = Some(next);
                }
                None => {
                    if initial {
                        let conn = self.lock();
                        set_cursor(&conn, "conversations", None);
                    }
                    break;
                }
            }
            if !initial && empty {
                break;
            }
        }
        Ok(ResourceSyncResult {
            resource,
            processed,
            failed,
            error: None,
        })
    }

    pub(super) async fn ingest_conversation(
        &self,
        c: &HsConversation,
        threads: &[HsThread],
        fetch_threads_if_needed: bool,
    ) -> Result<()> {
        let need_mailboxes = {
            let conn = self.lock();
            local_id(&conn, "mailboxes", c.mailbox_id).is_none()
        };
        if need_mailboxes {
            let mbs = self.provider.list_mailboxes().await?;
            let conn = self.lock();
            for m in &mbs {
                if local_id(&conn, "mailboxes", m.remote_id).is_none() {
                    mirror::upsert_mailbox(&conn, m)?;
                }
            }
        }
        let need_customer = {
            let conn = self.lock();
            c.customer_id > 0 && local_id(&conn, "customers", c.customer_id).is_none()
        };
        if need_customer && let Some(cust) = self.provider.get_customer(c.customer_id).await? {
            let conn = self.lock();
            mirror::upsert_customer(&conn, &cust)?;
        }
        {
            let conn = self.lock();
            mirror::upsert_conversation(&conn, c)?;
        }
        let local = {
            let conn = self.lock();
            conversation_local_id(&conn, c.remote_id)
        };
        let Some(local_id) = local else {
            return Ok(());
        };

        let mut list: Vec<HsThread> = threads.to_vec();
        if fetch_threads_if_needed || list.is_empty() {
            match self.provider.list_threads(c.remote_id).await {
                Ok(fetched) => list = fetched,
                Err(e) => {
                    let conn = self.lock();
                    let _ = crate::jobs::log_error(
                        &conn,
                        "sync",
                        &format!("Thread fetch failed for conversation {}: {e}", c.remote_id),
                    );
                    list = Vec::new();
                }
            }
        }
        let seen: Vec<i64> = list.iter().map(|t| t.remote_id).collect();
        {
            let mut conn = self.lock();
            for t in &list {
                upsert_thread(&conn, local_id, t)?;
            }
            if !list.is_empty() || !threads.is_empty() {
                let stale: Vec<i64> = {
                    let mut stmt = conn.prepare(
                        "SELECT id, remote_id FROM conversation_threads
                          WHERE conversation_id = ?1 AND remote_id IS NOT NULL",
                    )?;
                    let rows: Vec<(i64, i64)> = stmt
                        .query_map(params![local_id], |r| {
                            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                        })?
                        .filter_map(|r| r.ok())
                        .collect();
                    rows.into_iter()
                        .filter(|(_, remote)| !seen.contains(remote))
                        .map(|(id, _)| id)
                        .collect()
                };
                if !stale.is_empty() {
                    let fts_exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM sqlite_master
                              WHERE type = 'table' AND name = 'fts_threads'",
                            [],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);
                    let tx = conn.transaction()?;
                    for id in &stale {
                        if fts_exists {
                            tx.execute(
                                "DELETE FROM fts_threads WHERE thread_id = ?1",
                                params![id],
                            )?;
                        }
                        tx.execute(
                            "DELETE FROM conversation_threads WHERE id = ?1",
                            params![id],
                        )?;
                    }
                    tx.commit()?;
                }
            }
            if let Err(e) = crate::search::index_conversation_fts(&conn, local_id) {
                let _ = crate::jobs::log_error(
                    &conn,
                    "sync",
                    &format!("FTS index failed for conversation {}: {e}", c.remote_id),
                );
            } else {
                let _ = conn.execute(
                    "UPDATE conversation_threads SET fts_indexed = 1
                      WHERE conversation_id = ?1 AND deleted_at IS NULL
                        AND body_text IS NOT NULL AND LENGTH(body_text) > 0",
                    params![local_id],
                );
            }
        }
        {
            let conn = self.lock();
            match crate::embeddings::chunk_conversation(&conn, local_id) {
                Ok(0) => {}
                Ok(_written) => {
                    let feature_on = crate::ai_hub::ai_feature_enabled(&conn, "retrieval");
                    let already_queued: bool = conn
                        .query_row(
                            "SELECT 1 FROM jobs
                              WHERE type = 'embed_conversation_chunks'
                                AND status = 'queued' LIMIT 1",
                            [],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);
                    if feature_on && !already_queued {
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
                Err(e) => {
                    let _ = crate::jobs::log_error(
                        &conn,
                        "sync",
                        &format!("Chunking failed for conversation {}: {e}", c.remote_id),
                    );
                }
            }
        }
        Ok(())
    }
}
