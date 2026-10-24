use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::{Error, Result};

use crate::helpscout::{
    ConversationQuery, CustomerQuery, HelpScoutProvider, HsConversation, HsThread,
};
use crate::sync as mirror;

// Removed from the sweep by design (R6, honest stubs):
//   chats, attachments, ratings — Help Scout exposes no list endpoints for
//   Beacon chats (id-only, `GET /v2/chats/{id}`) or ratings (id-only,
// `GET /v2/ratings/{id}`); they arrive through webhooks and are fetched by
// id. Attachment metadata rows are hydrated during thread ingestion and
// downloads go through the `download_recent_attachments` job, so a sweep
// stage for them is a no-op by definition.
pub const INITIAL_SYNC_ORDER: [&str; 19] = [
    "account",
    "users",
    "system_users",
    "teams",
    "mailboxes",
    "folders",
    "tags",
    "inbox_fields",
    "customer_property_definitions",
    "organization_property_definitions",
    "organizations",
    "customers",
    "saved_replies",
    "workflows",
    "conversations",
    "threads",
    "docs_collections",
    "docs_articles",
    "user_statuses",
];

pub const INCREMENTAL_REFERENCE_ORDER: [&str; 13] = [
    "mailboxes",
    "folders",
    "tags",
    "users",
    "teams",
    "workflows",
    "saved_replies",
    "inbox_fields",
    "customer_property_definitions",
    "organization_property_definitions",
    "organizations",
    "docs_collections",
    "docs_articles",
];

pub const SYNC_OVERLAP_MINUTES: i64 = 10;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceSyncResult {
    pub resource: String,
    pub processed: i64,
    pub failed: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ReconciliationResult {
    pub checked: i64,
    pub added: i64,
    pub updated: i64,
    pub deleted: i64,
    pub merged: i64,
    pub failed: i64,
    pub skipped: i64,
    pub details: Vec<String>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Clears the engine's `running` flag when dropped — panic-safe, unlike
/// the manual trailing store it replaces. A panic between the
/// `compare_exchange` and the end of the run used to leave
/// `running == true` forever, after which every trigger answered
/// "A sync is already running" and the shell showed a phantom sync.
struct RunningGuard<'a>(&'a AtomicBool);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Boot recovery for a state machine stranded mid-flight: if the app
/// died during an initial/backfill sync, `sync_state` stays
/// `INITIALIZING`/`BACKFILLING` forever and the auto-sync timer skips
/// itself for eternity (workers.rs reads the state before every tick).
/// Recovery rule: any finished run on record means the account has data
/// → `CATCHING_UP` (incremental); otherwise → `NEW` (initial again).
pub fn recover_interrupted_state(conn: &Connection) -> bool {
    let state = get_state(conn);
    if state != "INITIALIZING" && state != "BACKFILLING" {
        return false;
    }
    let has_runs: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sync_runs WHERE state IN ('LIVE','ERROR','CATCHING_UP','CANCELLED'))",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);
    let recovered = if has_runs { "CATCHING_UP" } else { "NEW" };
    set_state(conn, recovered);
    let _ = crate::audit::log_event(
        conn,
        "warn",
        "sync",
        &format!(
            "Recovered interrupted sync state ({state} -> {recovered}) after an unclean shutdown"
        ),
        None,
        None,
    );
    true
}

/// R2 (D1): the watermark-integrity policy for a provider transition.
///
/// Called exactly once per boot (and once per in-session activation)
/// with the kind that was active *before* the change and the kind that
/// is active now:
///
/// * `empty → real`: the previous session ran against the inert
///   provider. Any `LIVE`/`CATCHING_UP` state and every checkpoint it
///   stamped describe a zero-record account, so the engine forgets
///   them: `sync_state` resets to `NEW` and the full initial sync runs
///   on the next tick. Without this, a pre-1.4.0 install that was
///   poisoned while disconnected would stay "up to date" forever.
/// * `real → empty`: informational only; the mirror is preserved and
///   the R2 engine guard stops the empty provider from advancing
///   anything.
/// * same-kind transitions: no-op.
///
/// Returns the state the machine ended in (for tests and diagnostics).
pub fn on_provider_kind_transition(
    conn: &Connection,
    previous: Option<&str>,
    current: &str,
) -> String {
    let previous = previous.unwrap_or("");
    if previous == current {
        return get_state(conn);
    }
    match (previous, current) {
        ("empty", "real") | ("", "real") => {
            let before = get_state(conn);
            set_state(conn, "NEW");
            let _ = crate::audit::log_event(
                conn,
                "warn",
                "sync",
                &format!(
                    "Help Scout credentials activated: sync state reset ({before} → NEW) so the \
                     full account history is pulled from scratch — an earlier session with no \
                     credentials may have stamped empty watermarks"
                ),
                None,
                None,
            );
            tracing::info!(
                before = %before,
                "provider transition empty→real: sync state reset to NEW"
            );
            "NEW".to_string()
        }
        (previous, current) => {
            let _ = crate::audit::log_event(
                conn,
                "info",
                "sync",
                &format!("provider transition {previous}→{current}"),
                None,
                None,
            );
            get_state(conn)
        }
    }
}

pub fn get_state(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value FROM application_settings WHERE key = 'sync_state'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| serde_json::from_str::<String>(&v).ok().or(Some(v)))
    .unwrap_or_else(|| "NEW".to_string())
}

pub fn set_state(conn: &Connection, state: &str) {
    let _ = conn.execute(
        "INSERT INTO application_settings (key, value, updated_at)
         VALUES ('sync_state', ?1, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![format!("\"{state}\"")],
    );
}

pub fn start_run(conn: &Connection, kind: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO sync_runs (kind, state, started_at) VALUES (?1, 'INITIALIZING', ?2)",
        params![kind, now_iso()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct RunUpdate<'a> {
    pub state: Option<&'a str>,
    pub resources_done: Option<i64>,
    pub resources_total: Option<i64>,
    pub records_processed: Option<i64>,
    pub errors: Option<i64>,
    pub detail: Option<Value>,
    pub finished: bool,
}

pub fn update_run(conn: &Connection, id: i64, fields: RunUpdate<'_>) -> Result<()> {
    let mut sets: Vec<&str> = Vec::new();
    let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(s) = fields.state {
        sets.push("state = ?");
        args.push(Box::new(s.to_string()));
    }
    if let Some(v) = fields.resources_done {
        sets.push("resources_done = ?");
        args.push(Box::new(v));
    }
    if let Some(v) = fields.resources_total {
        sets.push("resources_total = ?");
        args.push(Box::new(v));
    }
    if let Some(v) = fields.records_processed {
        sets.push("records_processed = ?");
        args.push(Box::new(v));
    }
    if let Some(v) = fields.errors {
        sets.push("errors = ?");
        args.push(Box::new(v));
    }
    if let Some(v) = fields.detail {
        sets.push("detail = ?");
        args.push(Box::new(v.to_string()));
    }
    if fields.finished {
        sets.push("finished_at = datetime('now')");
    }
    if sets.is_empty() {
        return Ok(());
    }
    let sql = format!("UPDATE sync_runs SET {} WHERE id = ?", sets.join(", "));
    args.push(Box::new(id));
    let refs: Vec<&dyn rusqlite::types::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())?;
    Ok(())
}

pub fn record_success(conn: &Connection, resource: &str, records_processed: i64) {
    let _ = conn.execute(
        "INSERT INTO sync_checkpoints (resource, last_success_at, records_processed, records_failed, last_error, retry_count, status)
         VALUES (?1, ?2, ?3, 0, NULL, 0, 'ok')
         ON CONFLICT(resource) DO UPDATE SET last_success_at = excluded.last_success_at,
           records_processed = records_processed + excluded.records_processed,
           last_error = NULL, retry_count = 0, status = 'ok'",
        params![resource, now_iso(), records_processed],
    );
}

pub fn record_failure(conn: &Connection, resource: &str, error: &str, records_failed: i64) {
    let clipped: String = error.chars().take(500).collect();
    let _ = conn.execute(
        "INSERT INTO sync_checkpoints (resource, records_processed, records_failed, last_error, retry_count, status)
         VALUES (?1, 0, ?2, ?3, 1, 'error')
         ON CONFLICT(resource) DO UPDATE SET records_failed = records_failed + excluded.records_failed,
           last_error = excluded.last_error, retry_count = retry_count + 1, status = 'error'",
        params![resource, records_failed, clipped],
    );
    // 1.2.0: every resource failure also lands in the Diagnostics log
    // (application_errors) so the user can see *why* nothing is syncing
    // from Settings → Diagnostics instead of a stdout log they never see.
    let _ = crate::audit::log_event(
        conn,
        "error",
        "sync",
        &format!("Sync resource {resource} failed"),
        None,
        Some(&serde_json::json!({ "resource": resource, "error": clipped })),
    );
}

pub fn set_checkpoint_running(conn: &Connection, resource: &str, running: bool) {
    let _ = conn.execute(
        "INSERT INTO sync_checkpoints (resource, status) VALUES (?1, ?2)
         ON CONFLICT(resource) DO UPDATE SET status = excluded.status",
        params![resource, if running { "running" } else { "idle" }],
    );
}

pub fn get_incremental_since(
    conn: &Connection,
    resource: &str,
    overlap_minutes: i64,
) -> Option<String> {
    let last: Option<String> = conn
        .query_row(
            "SELECT last_success_at FROM sync_checkpoints WHERE resource = ?1",
            params![resource],
            |r| r.get(0),
        )
        .ok()
        .flatten()?;
    let t = chrono::DateTime::parse_from_rfc3339(last.as_deref().unwrap_or_default()).ok()?;
    let shifted = t - chrono::Duration::minutes(overlap_minutes);
    Some(shifted.to_rfc3339())
}

pub fn last_successful_sync(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT MAX(last_success_at) FROM sync_checkpoints WHERE status = 'ok'",
        [],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
}

/// How long ago the last successful sync completed, in whole seconds
/// (R8 freshness ring). `None` means "never synced".
#[must_use]
pub fn last_sync_age_seconds(conn: &Connection) -> Option<i64> {
    let last = last_successful_sync(conn)?;
    let t = chrono::DateTime::parse_from_rfc3339(last.as_str().trim()).ok()?;
    let age = chrono::Utc::now() - t.with_timezone(&chrono::Utc);
    Some(age.num_seconds().max(0))
}

pub fn get_cursor(conn: &Connection, resource: &str) -> Option<String> {
    conn.query_row(
        "SELECT cursor FROM sync_cursors WHERE resource = ?1",
        params![resource],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
    .filter(|c| !c.is_empty())
}

pub fn set_cursor(conn: &Connection, resource: &str, cursor: Option<&str>) {
    let _ = conn.execute(
        "INSERT INTO sync_cursors (resource, cursor, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(resource) DO UPDATE SET cursor = excluded.cursor, updated_at = excluded.updated_at",
        params![resource, cursor, now_iso()],
    );
}

pub fn local_id(conn: &Connection, table: &str, remote_id: i64) -> Option<i64> {
    conn.query_row(
        &format!("SELECT id FROM {table} WHERE remote_id = ?1"),
        params![remote_id],
        |r| r.get(0),
    )
    .ok()
}

pub fn conversation_local_id(conn: &Connection, remote_id: i64) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM conversations WHERE remote_id = ?1",
        params![remote_id],
        |r| r.get(0),
    )
    .ok()
}

fn upsert_folder(
    conn: &Connection,
    mailbox_local: i64,
    f: &crate::helpscout::HsFolder,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folders (remote_id, mailbox_id, name, type, user_id, total_count, active_count, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT(remote_id) DO UPDATE SET
            mailbox_id = excluded.mailbox_id, name = excluded.name, type = excluded.type,
            user_id = excluded.user_id, total_count = excluded.total_count,
            active_count = excluded.active_count, last_synced_at = datetime('now')",
        params![f.remote_id, mailbox_local, f.name, f.kind, f.user_id, f.total_count, f.active_count],
    )?;
    Ok(())
}

fn upsert_inbox_field(
    conn: &Connection,
    mailbox_local: i64,
    f: &crate::helpscout::HsField,
) -> Result<()> {
    conn.execute(
        "INSERT INTO inbox_fields (remote_id, mailbox_id, name, type, system_type, required, sort_order, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT(remote_id) DO UPDATE SET
            mailbox_id = excluded.mailbox_id, name = excluded.name, type = excluded.type,
            system_type = excluded.system_type, required = excluded.required,
            sort_order = excluded.sort_order, last_synced_at = datetime('now')",
        params![f.remote_id, mailbox_local, f.name, f.kind, f.system_type, i64::from(f.required), f.sort_order],
    )?;
    for opt in &f.options {
        conn.execute(
            "INSERT INTO inbox_field_options (remote_id, field_id, label, sort_order)
             VALUES (?1, (SELECT id FROM inbox_fields WHERE remote_id = ?2), ?3, ?4)
             ON CONFLICT(remote_id) DO UPDATE SET label = excluded.label, sort_order = excluded.sort_order",
            params![opt.id, f.remote_id, opt.label, opt.order],
        )?;
    }
    Ok(())
}

fn upsert_property_definitions(
    conn: &Connection,
    table: &str,
    defs: &[crate::helpscout::HsPropertyDef],
) -> Result<()> {
    for d in defs {
        conn.execute(
            &format!(
                "INSERT INTO {table} (remote_id, name, slug, type, sort_order, last_synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
                 ON CONFLICT(remote_id) DO UPDATE SET name = excluded.name, slug = excluded.slug,
                   type = excluded.type, sort_order = excluded.sort_order, last_synced_at = datetime('now')"
            ),
            params![d.remote_id, d.name, d.slug, d.kind, d.sort_order],
        )?;
    }
    Ok(())
}

pub fn upsert_organization(conn: &Connection, o: &crate::helpscout::HsOrganization) -> Result<()> {
    let domains = serde_json::to_string(&o.domains).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO organizations (remote_id, name, domains, remote_created_at, remote_updated_at, last_seen_at, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))
         ON CONFLICT(remote_id) DO UPDATE SET name = excluded.name, domains = excluded.domains,
           remote_created_at = excluded.remote_created_at, remote_updated_at = excluded.remote_updated_at,
           last_seen_at = datetime('now'), last_synced_at = datetime('now'),
           deleted_at = NULL",
        params![o.remote_id, o.name, domains, o.created_at, o.updated_at],
    )?;
    Ok(())
}

pub fn upsert_saved_reply(
    conn: &Connection,
    mailbox_local: Option<i64>,
    r: &crate::helpscout::HsSavedReply,
) -> Result<()> {
    conn.execute(
        "INSERT INTO saved_replies (remote_id, mailbox_local_id, name, preview, text, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
         ON CONFLICT(remote_id) DO UPDATE SET mailbox_local_id = excluded.mailbox_local_id,
           name = excluded.name, preview = excluded.preview, text = excluded.text,
           last_synced_at = datetime('now'), deleted_at = NULL",
        params![r.remote_id, mailbox_local, r.name, r.preview, r.text],
    )?;
    Ok(())
}

pub fn upsert_workflow(conn: &Connection, w: &crate::helpscout::HsWorkflow) -> Result<()> {
    let mailbox_local = w.mailbox_id.and_then(|m| local_id(conn, "mailboxes", m));
    conn.execute(
        "INSERT INTO workflows (remote_id, mailbox_local_id, name, type, status, sort_order, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
         ON CONFLICT(remote_id) DO UPDATE SET mailbox_local_id = excluded.mailbox_local_id,
           name = excluded.name, type = excluded.type, status = excluded.status,
           sort_order = excluded.sort_order, last_synced_at = datetime('now'),
           deleted_at = NULL",
        params![w.remote_id, mailbox_local, w.name, w.kind, w.status, w.sort_order],
    )?;
    Ok(())
}

/// Clean-audit item 91: platform-side deletions propagate. These mirrors
/// are full-list resources — a row whose remote id is absent from the
/// platform's (complete, successfully fetched) list was deleted up there,
/// so it is stamped deleted locally instead of serving it forever. The
/// empty list is deliberately NOT a wipe signal (a degraded or
/// permission-downgraded answer must not blank the mirror); a genuinely
/// emptied platform leaves stale rows until the resource returns data.
/// Soft-deleted rows come back to life on the next listing (the upserts
/// clear `deleted_at`).
fn prune_missing_rows(conn: &Connection, table: &str, seen: &[i64]) -> Result<u64> {
    if seen.is_empty() {
        return Ok(0);
    }
    let placeholders = vec!["?".to_string(); seen.len()].join(", ");
    let sql = format!(
        "UPDATE {table} SET deleted_at = datetime('now')
          WHERE deleted_at IS NULL AND remote_id NOT IN ({placeholders})"
    );
    let params: Vec<&dyn rusqlite::types::ToSql> = seen
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let n = conn.execute(&sql, params.as_slice())?;
    Ok(u64::try_from(n).unwrap_or(0))
}

fn upsert_user_status(
    conn: &Connection,
    user_local: i64,
    s: &crate::helpscout::HsUserStatus,
) -> Result<()> {
    conn.execute(
        "INSERT INTO user_statuses (user_local_id, email_status, email_updated_at, chat_status, mailbox_statuses, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
         ON CONFLICT(user_local_id) DO UPDATE SET email_status = excluded.email_status,
           email_updated_at = excluded.email_updated_at, chat_status = excluded.chat_status,
           mailbox_statuses = excluded.mailbox_statuses, last_synced_at = datetime('now')",
        params![user_local, s.email_status, s.email_updated_at, s.chat_status, s.mailbox_statuses.to_string()],
    )?;
    Ok(())
}

fn upsert_doc_collection(conn: &Connection, c: &crate::helpscout::HsDocCollection) -> Result<i64> {
    conn.execute(
        "INSERT INTO docs_collections (remote_id, slug, name, description, visibility, article_count, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
         ON CONFLICT(remote_id) DO UPDATE SET slug = excluded.slug, name = excluded.name,
           description = excluded.description, visibility = excluded.visibility,
           article_count = excluded.article_count, last_synced_at = datetime('now')",
        params![
            c.remote_id,
            c.slug,
            c.name,
            c.description,
            c.visibility,
            c.article_count
        ],
    )?;
    local_id(conn, "docs_collections", c.remote_id)
        .ok_or_else(|| Error::Other("docs_collections upsert lost its row".into()))
}

fn upsert_doc_categories(
    conn: &Connection,
    collection_local: i64,
    cats: &[crate::helpscout::HsDocCategory],
) -> Result<()> {
    for c in cats {
        conn.execute(
            "INSERT INTO docs_categories (remote_id, collection_local_id, slug, name, last_synced_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(remote_id) DO UPDATE SET collection_local_id = excluded.collection_local_id,
               slug = excluded.slug, name = excluded.name, last_synced_at = datetime('now')",
            params![c.remote_id, collection_local, c.slug, c.name],
        )?;
    }
    Ok(())
}

pub(crate) fn upsert_doc_article(
    conn: &mut Connection,
    a: &crate::helpscout::HsDocArticle,
    collection_local: i64,
) -> Result<()> {
    let category_local = a
        .category_id
        .and_then(|cid| local_id(conn, "docs_categories", cid));
    let preview = a.text.as_deref().filter(|t| !t.is_empty()).map(|t| {
        let collapsed = t.split_whitespace().collect::<Vec<_>>().join(" ");
        crate::embeddings::utf16_slice(&collapsed, 0, 220)
    });
    let words = a
        .text
        .as_deref()
        .filter(|t| !t.is_empty())
        .map(|t| i64::try_from(t.split_whitespace().count()).unwrap_or(0));
    let chunk_source = match a.text.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => format!("{}\n\n{}", a.name, t),
        None => a.name.clone(),
    };
    let content_hash = crate::embeddings::content_hash(&chunk_source);
    let existing: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT id, content_hash FROM docs_articles WHERE remote_id = ?1",
            params![a.remote_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let content_changed = match &existing {
        Some((_, Some(prev_hash))) => *prev_hash != content_hash,
        _ => true,
    };
    conn.execute(
        "INSERT INTO docs_articles (remote_id, collection_local_id, category_local_id, number, slug, name, status, preview, text, views, words, remote_created_at, remote_updated_at, last_synced_at, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime('now'), ?14)
         ON CONFLICT(remote_id) DO UPDATE SET collection_local_id = excluded.collection_local_id,
           category_local_id = excluded.category_local_id, number = excluded.number,
           slug = excluded.slug, name = excluded.name, status = excluded.status,
           preview = excluded.preview, text = excluded.text, views = excluded.views,
           words = excluded.words, remote_created_at = excluded.remote_created_at,
           remote_updated_at = excluded.remote_updated_at, last_synced_at = datetime('now'),
           content_hash = excluded.content_hash",
        params![
            a.remote_id,
            collection_local,
            category_local,
            a.number,
            a.slug,
            a.name,
            a.status,
            preview,
            a.text,
            a.views,
            words,
            a.created_at,
            a.updated_at,
            content_hash
        ],
    )?;
    if content_changed {
        let id = match existing {
            Some((id, _)) => id,
            None => local_id(conn, "docs_articles", a.remote_id)
                .ok_or_else(|| Error::Other("docs article upsert lost its row".into()))?,
        };
        reindex_doc_fts(conn, id, &a.name, a.text.as_deref())?;
        rechunk_doc_article(conn, id, &a.name, a.text.as_deref())?;
    }
    Ok(())
}

fn reindex_doc_fts(
    conn: &mut Connection,
    article_id: i64,
    name: &str,
    text: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM docs_fts WHERE article_id = ?1",
        params![article_id],
    )?;
    tx.execute(
        "INSERT INTO docs_fts (name, text, article_id) VALUES (?1, ?2, ?3)",
        params![name, text.unwrap_or(""), article_id],
    )?;
    tx.commit()?;
    Ok(())
}

fn rechunk_doc_article(
    conn: &mut Connection,
    article_id: i64,
    name: &str,
    text: Option<&str>,
) -> Result<()> {
    let chunks = match text.filter(|t| !t.is_empty()) {
        Some(t) => crate::embeddings::chunk_text(&format!("{name}\n\n{t}"), 1200, 150),
        None => Vec::new(),
    };
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM docs_chunks WHERE article_id = ?1",
        params![article_id],
    )?;
    for (i, content) in chunks.iter().enumerate() {
        tx.execute(
            "INSERT INTO docs_chunks (article_id, chunk_index, content)
             VALUES (?1, ?2, ?3)",
            params![article_id, i64::try_from(i).unwrap_or(0), content],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// C1: hex SHA-256 of the thread's serialized content — see
/// `upsert_thread`.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn upsert_thread(conn: &Connection, conversation_local: i64, t: &HsThread) -> Result<()> {
    // C1: content hash of the ingested thread, written on insert AND on
    // every in-place re-sync update (edited body, state flip, moved
    // conversation). The analysis-cache signature folds these hashes
    // in, so an in-place edit invalidates cached analyses instead of
    // serving stale ones forever.
    let raw_json_hash = sha256_hex(serde_json::to_string(t).unwrap_or_default().as_bytes());
    let from_type: &str = if t.created_by_customer_id.is_some() {
        "customer"
    } else if t.created_by_user_id.is_some() {
        "user"
    } else {
        "system_user"
    };
    let created_by_user_id = (from_type == "user")
        .then(|| {
            t.created_by_user_id
                .and_then(|rid| local_id(conn, "users", rid))
        })
        .flatten();
    let created_by_customer_id = (from_type == "customer")
        .then(|| {
            t.created_by_customer_id
                .and_then(|rid| local_id(conn, "customers", rid))
        })
        .flatten();
    let created_by_system_user_id: Option<i64> = None;
    conn.execute(
        "INSERT INTO conversation_threads
             (conversation_id, type, state, body_text, from_type,
              created_by_user_id, created_by_customer_id, created_by_system_user_id,
              created_at, remote_id, raw_json_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(remote_id) DO UPDATE SET
            conversation_id = excluded.conversation_id, type = excluded.type,
            state = excluded.state,
            body_text = excluded.body_text, from_type = excluded.from_type,
            created_by_user_id = excluded.created_by_user_id,
            created_by_customer_id = excluded.created_by_customer_id,
            created_by_system_user_id = excluded.created_by_system_user_id,
            created_at = excluded.created_at,
            -- DB-05 (M18): threads resurrect on upsert too
            -- (conversationRepo.ts:667 `deleted_at=NULL`).
            deleted_at = NULL,
            raw_json_hash = excluded.raw_json_hash",
        params![
            conversation_local,
            t.kind,
            t.state,
            t.body,
            from_type,
            created_by_user_id,
            created_by_customer_id,
            created_by_system_user_id,
            t.created_at,
            t.remote_id,
            raw_json_hash
        ],
    )?;
    record_thread_event(
        conn,
        conversation_local,
        t,
        from_type,
        created_by_user_id,
        created_by_customer_id,
    )?;
    persist_thread_details(conn, t)?;
    if t.kind == "customer" {
        let _ = crate::interaction_current::record_current_interaction(conn, conversation_local);
    }
    Ok(())
}

fn persist_thread_details(conn: &Connection, t: &HsThread) -> Result<()> {
    let Some(thread_local) = local_id(conn, "conversation_threads", t.remote_id) else {
        return Ok(());
    };
    conn.execute(
        "DELETE FROM thread_recipients WHERE thread_id = ?1",
        params![thread_local],
    )?;
    for (kind, list) in [("to", &t.to), ("cc", &t.cc)] {
        for r in list {
            if let Some(email) = r.email.as_deref().filter(|e| !e.trim().is_empty()) {
                conn.execute(
                    "INSERT INTO thread_recipients (thread_id, email, type)
                     VALUES (?1, ?2, ?3)",
                    params![thread_local, email.trim(), kind],
                )?;
            }
        }
    }
    for a in &t.attachments {
        conn.execute(
            "INSERT INTO attachments (remote_id, thread_id, conversation_id, filename, mime_type, size, state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'metadata')
             ON CONFLICT(remote_id) DO UPDATE SET
                thread_id = excluded.thread_id,
                conversation_id = excluded.conversation_id,
                filename = excluded.filename,
                mime_type = excluded.mime_type,
                size = excluded.size",
            params![
                a.remote_id,
                thread_local,
                local_id(conn, "conversations", t.conversation_id),
                a.filename,
                a.mime_type,
                a.size,
            ],
        )?;
    }
    Ok(())
}

fn record_thread_event(
    conn: &Connection,
    conversation_local: i64,
    t: &HsThread,
    from_type: &str,
    created_by_user_id: Option<i64>,
    created_by_customer_id: Option<i64>,
) -> Result<Option<&'static str>> {
    if t.state.as_deref() != Some("published") {
        return Ok(None);
    }
    let kind = t.kind.as_str();
    let event_actor_local_id = created_by_user_id.or(created_by_customer_id);
    let (event_type, actor_type, event_actor_id): (&'static str, &str, Option<i64>) = match kind {
        "note" => ("internal_note", "user", created_by_user_id),
        "lineitem" => {
            let text = t.body.as_deref().unwrap_or("").to_lowercase();
            let event_type = if text.contains("status") {
                "status_changed"
            } else if text.contains("assign") {
                "assignment_changed"
            } else if text.contains("moved") {
                "moved"
            } else if text.contains("tag") {
                "tag_added"
            } else {
                "lineitem_action"
            };
            (event_type, "system_user", None)
        }
        _ => {
            if t.created_by_customer_id.is_some() {
                ("customer_message", from_type, event_actor_local_id)
            } else if t.created_by_user_id.is_some() {
                ("human_agent_message", from_type, event_actor_local_id)
            } else {
                ("customer_message", "unknown", None)
            }
        }
    };
    let thread_local: Option<i64> = conn
        .query_row(
            "SELECT id FROM conversation_threads WHERE remote_id = ?1",
            params![t.remote_id],
            |r| r.get(0),
        )
        .ok();
    crate::activity::record_full_event(
        conn,
        &crate::activity::FullActivityEvent {
            base: crate::activity::ActivityEvent {
                id: None,
                conversation_id: conversation_local,
                event_type: event_type.into(),
                actor_type: actor_type.into(),
                actor_id: event_actor_id,
                occurred_at: t.created_at.clone().unwrap_or_default(),
                dedup_key: format!("thread:{}", t.remote_id),
            },
            thread_local_id: thread_local,
            source: "sync".into(),
            metadata: Some(
                serde_json::json!({
                    "thread_remote_id": t.remote_id,
                    "thread_type": t.kind,
                })
                .to_string(),
            ),
        },
    )?;
    Ok(Some(event_type))
}

fn stored_user_updated_at_matches(
    conn: &Connection,
    remote_id: i64,
    remote_updated_at: Option<&str>,
) -> bool {
    let Some(remote) = remote_updated_at else {
        return false;
    };
    conn.query_row(
        "SELECT updated_at FROM conversations WHERE remote_id = ?1",
        params![remote_id],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
    .is_some_and(|stored| stored == remote)
}

fn soft_delete_by_remote_id(conn: &Connection, remote_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE conversations SET status = 'deleted',
             deleted_at = COALESCE(deleted_at, datetime('now'))
           WHERE remote_id = ?1",
        params![remote_id],
    )?;
    Ok(())
}

pub struct SyncEngine {
    db: crate::db::DbHandle,
    provider: Arc<dyn HelpScoutProvider>,
    running: AtomicBool,
    cancel_requested: AtomicBool,
    pub bus: Option<crate::eventing::EventBus>,
}

impl SyncEngine {
    #[must_use]
    pub fn new(db: crate::db::DbHandle, provider: Arc<dyn HelpScoutProvider>) -> Self {
        Self {
            db,
            provider,
            running: AtomicBool::new(false),
            cancel_requested: AtomicBool::new(false),
            bus: None,
        }
    }

    #[must_use]
    pub fn with_bus(mut self, bus: crate::eventing::EventBus) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn provider(&self) -> &Arc<dyn HelpScoutProvider> {
        &self.provider
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn request_cancellation(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
    }

    fn cancelled(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }

    fn lock(&self) -> crate::db::DbWriteGuard<'_> {
        self.db.write_guard()
    }

    fn emit_sync_completed(
        &self,
        kind: &str,
        processed: i64,
        errors: i64,
        trigger: crate::events::SyncTrigger,
    ) {
        if let Some(bus) = &self.bus {
            use crate::events::ServerEvent;
            bus.emit(&ServerEvent::sync_completed(
                kind,
                processed.max(0) as u32,
                errors.max(0) as u32,
                trigger,
            ));
        }
    }

    /// Emit an [`crate::events::ServerEvent::ErrorOccurred`] to the bus
    /// (v1.4.0 R5, D4): background failures used to land only in the
    /// Diagnostics log, so a failing sync or a dead resource looked
    /// exactly like a healthy quiet one everywhere else.
    fn emit_error(
        &self,
        severity: &str,
        service: &str,
        message: &str,
        conversation_id: Option<i64>,
    ) {
        if let Some(bus) = &self.bus {
            use crate::events::ServerEvent;
            bus.emit(&ServerEvent::error_occurred(
                severity,
                message,
                service,
                conversation_id,
            ));
        }
    }

    /// R2 (D1): every sync entry point refuses to run against the empty
    /// provider. Before this guard, a fresh install without credentials
    /// ran "successful" zero-record syncs every minute: checkpoints
    /// stamped forward, `sync_state` flipped to LIVE, and when real
    /// credentials finally arrived the engine believed the account was
    /// already synchronized and only pulled a ten-minute incremental
    /// window — the entire history never arrived. The guard keeps that
    /// poisoning from ever starting, and the transition policy in
    /// [`on_provider_kind_transition`] repairs a state that already
    /// carries poisoned watermarks (a pre-1.4.0 install).
    fn guard_provider_connected(&self) -> Result<()> {
        if self.provider.kind() == "empty" {
            return Err(Error::Other(
                "Help Scout is not connected — nothing to sync. Add your \n\
                 credentials in Settings; the connection activates \n\
                 immediately and the first sync starts by itself."
                    .into(),
            ));
        }
        Ok(())
    }

    /// Mid-run progress heartbeat. Before 1.3.0 the engine was silent
    /// until the very end of a run: pages cached their snapshots, so a
    /// multi-hour initial sync looked like a dead "in progress" list
    /// even while rows landed. Every event triggers a repaint through
    /// the bridge, so Sync Health now shows live X-of-Y progress and
    /// the inbox streams conversations in as they ingest.
    fn emit_progress(
        &self,
        stage: &str,
        done: u32,
        total: u32,
        processed: i64,
        stage_records: u32,
    ) {
        if let Some(bus) = &self.bus {
            use crate::events::ServerEvent;
            bus.emit(&ServerEvent::sync_progress(
                stage,
                done,
                total,
                processed.max(0) as u64,
                stage_records,
            ));
        }
    }

    fn emit_conversation_updated_local(&self, local_id: i64, subject: Option<&str>) {
        if let Some(bus) = &self.bus {
            use crate::events::ServerEvent;
            let conn = self.lock();
            let row: Option<(i64, Option<i64>)> = conn
                .query_row(
                    "SELECT number, mailbox_local_id FROM conversations WHERE id = ?1",
                    params![local_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            if let Some((number, mailbox_id)) = row {
                bus.emit(&ServerEvent::conversation_updated(
                    Some(local_id),
                    Some(number),
                    mailbox_id,
                    subject.map(str::to_string),
                    "sync",
                ));
            }
        }
    }

    /// Refresh ONLY the Help Center mirror (collections + articles) —
    /// the Knowledge page's "Sync Help Center now" button runs this
    /// instead of a full engine pass. Emits `SyncCompleted("docs", ...)`
    /// so the page reloads itself when the articles land.
    ///
    /// `trigger` (audit item 23) says who started the run; the shell only
    /// toasts completions the user asked for.
    pub async fn sync_docs_only(
        &self,
        trigger: crate::events::SyncTrigger,
    ) -> Result<Vec<ResourceSyncResult>> {
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
            start_run(&conn, "docs")?
        };
        // R5 (D15): a missing Docs API key used to degrade to a silent
        // empty Help Center (one buried warn in the log). Say it live on
        // the bus instead so the Knowledge page banner and a toast can
        // explain the zeros.
        if !self.provider.docs_configured() {
            self.emit_error(
                "warn",
                "docs",
                "Help Center sync is idle: no Docs API key configured. Add one in \n\
                 Settings → Help Scout credentials (Help Scout Profile → \n\
                 Security & Access → Docs API).",
                None,
            );
        }
        let mut results = Vec::new();
        for resource in ["docs_collections", "docs_articles"] {
            let r = self.run_resource(resource, false).await;
            let r = match r {
                Ok(r) => r,
                Err(e) => {
                    let conn = self.lock();
                    record_failure(&conn, resource, &e.to_string(), 1);
                    ResourceSyncResult {
                        resource: resource.to_string(),
                        processed: 0,
                        failed: 1,
                        error: Some(e.to_string()),
                    }
                }
            };
            match &r {
                ok if ok.error.is_none() => {
                    let conn = self.lock();
                    record_success(&conn, resource, r.processed);
                }
                other => {
                    let conn = self.lock();
                    record_failure(
                        &conn,
                        resource,
                        other.error.as_deref().unwrap_or(""),
                        other.failed,
                    );
                }
            }
            let processed = r.processed;
            results.push(r);
            self.emit_progress(
                resource,
                results.len() as u32,
                2,
                processed,
                processed as u32,
            );
        }
        let processed: i64 = results.iter().map(|r| r.processed).sum();
        let failed = results.iter().filter(|r| r.error.is_some()).count() as i64;
        {
            let conn = self.lock();
            let _ = update_run(
                &conn,
                run_id,
                RunUpdate {
                    state: Some(if failed == 0 { "LIVE" } else { "ERROR" }),
                    resources_done: None,
                    resources_total: None,
                    records_processed: Some(processed),
                    errors: Some(failed),
                    detail: None,
                    finished: true,
                },
            );
            let _ = crate::audit::log_event(
                &conn,
                if failed == 0 { "info" } else { "warn" },
                "docs",
                &format!(
                    "Help Center sync finished: {processed} articles/collections, {failed} failed"
                ),
                None,
                None,
            );
        }
        self.emit_sync_completed("docs", processed, failed, trigger);
        Ok(results)
    }

    /// Backfill threads for conversations whose thread list is empty
    /// locally (crashed run, hard rate limit mid-walk). Bounded per pass;
    /// the next pass continues where this one stopped.
    async fn sync_missing_threads(&self) -> Result<ResourceSyncResult> {
        let resource = "threads".to_string();
        let candidates: Vec<(i64, i64)> = {
            let conn = self.lock();
            let mut stmt = conn.prepare(
                "SELECT id, remote_id FROM conversations
                  WHERE deleted_at IS NULL AND merged_into_conversation_id IS NULL
                    AND NOT EXISTS (SELECT 1 FROM conversation_threads t
                                     WHERE t.conversation_id = conversations.id)
                  ORDER BY id ASC LIMIT 200",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.filter_map(|r| r.ok()).collect()
        };
        let mut processed = 0i64;
        let mut failed = 0i64;
        for (local, remote) in candidates {
            if self.cancelled() {
                break;
            }
            match self.provider.get_conversation(remote).await {
                Ok(Some(conv)) => {
                    self.ingest_conversation(&conv, &[], true).await?;
                    processed += 1;
                    self.emit_conversation_updated_local(local, conv.subject.as_deref());
                }
                Ok(None) => {
                    let conn = self.lock();
                    soft_delete_by_remote_id(&conn, remote)?;
                }
                Err(e) => {
                    // A single unreachable conversation must not fail the
                    // pass — but it must be COUNTED (clean-audit item 89:
                    // the pre-fix pass hardcoded failed: 0 with error: None,
                    // so the caller's record_success stamped 'ok' over the
                    // per-row record_failure and the run's error count
                    // never saw these). The resource-level bookkeeping in
                    // the caller records the summarized failure once.
                    failed += 1;
                    tracing::warn!(
                        conversation_remote = remote,
                        error = %e,
                        "thread backfill: conversation re-fetch failed"
                    );
                }
            }
        }
        self.emit_progress(&resource, 1, 1, processed, processed as u32);
        Ok(ResourceSyncResult {
            resource,
            processed,
            failed,
            error: (failed > 0).then(|| {
                format!("{failed} conversation(s) could not be re-fetched for thread backfill")
            }),
        })
    }

    /// `trigger` (audit item 23) says who started the run — the shell's
    /// completion toast only fires for user-initiated runs, so the
    /// adaptive auto-sync timer no longer toasts ~once a minute.
    pub async fn initial_sync(
        &self,
        trigger: crate::events::SyncTrigger,
    ) -> Result<Vec<ResourceSyncResult>> {
        self.guard_provider_connected()?;
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(Error::Other("A sync is already running".into()));
        }
        let _running = RunningGuard(&self.running);
        self.cancel_requested.store(false, Ordering::SeqCst);
        let run_id = {
            let conn = self.lock();
            set_state(&conn, "INITIALIZING");
            start_run(&conn, "initial")?
        };
        let mut results: Vec<ResourceSyncResult> = Vec::new();
        let outcome: Result<()> = async {
            // R5 (D15): same honest docs-key warning as the docs-only
            // pass, so the first full sync explains an empty Help Center
            // instead of silently mirroring zero articles.
            if !self.provider.docs_configured() {
                self.emit_error(
                    "warn",
                    "docs",
                    "Help Center sync is idle: no Docs API key configured. Add one in Settings → Help Scout credentials.",
                    None,
                );
            }
            for resource in INITIAL_SYNC_ORDER {
                if self.cancelled() {
                    break;
                }
                {
                    let conn = self.lock();
                    set_checkpoint_running(&conn, resource, true);
                    let _ = update_run(
                        &conn,
                        run_id,
                        RunUpdate {
                            state: None,
                            resources_done: Some(results.len() as i64),
                            resources_total: Some(INITIAL_SYNC_ORDER.len() as i64),
                            records_processed: None,
                            errors: None,
                            detail: None,
                            finished: false,
                        },
                    );
                }
                let r = self.sync_resource(resource, true).await;
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
                                "Sync resource {} failed: {}",
                                resource,
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
                let r = match r {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(resource, error = %e, "sync resource failed");
                        ResourceSyncResult {
                            resource: resource.to_string(),
                            processed: 0,
                            failed: 1,
                            error: Some(e.to_string()),
                        }
                    }
                };
                let processed: i64 = results.iter().map(|x| x.processed).sum::<i64>() + r.processed;
                let errors = results.iter().filter(|x| x.error.is_some()).count() as i64
                    + i64::from(r.error.is_some());
                let stage_records = r.processed.max(0) as u32;
                let stage = resource.to_string();
                results.push(r);
                {
                    let conn = self.lock();
                    let _ = update_run(
                        &conn,
                        run_id,
                        RunUpdate {
                            state: None,
                            resources_done: Some(results.len() as i64),
                            resources_total: None,
                            records_processed: Some(processed),
                            errors: Some(errors),
                            detail: None,
                            finished: false,
                        },
                    );
                }
                // Live progress: one event per resource so Sync Health and
                // the inbox track the run instead of freezing on a stale
                // snapshot until the terminal event.
                self.emit_progress(
                    &stage,
                    results.len() as u32,
                    INITIAL_SYNC_ORDER.len() as u32,
                    processed,
                    stage_records,
                );
            }
            let cancelled = self.cancelled();
            let failed = results.iter().filter(|r| r.error.is_some()).count() as i64;
            {
                let conn = self.lock();
                // Clean-audit item 88: a cancelled initial sync used to fall
                // through to the same verdict as a completed one — a
                // mid-run cancel with zero resource errors was stamped
                // LIVE (and the run row "finished, LIVE"), so the
                // auto-sync timer never re-ran the initial pass and the
                // thread-less conversations the cancel stranded were
                // never backfilled. A cancelled run now records the
                // partial truth: the mirror is CATCHING_UP (incrementals
                // — which run the threads backfill too — drain it), and
                // the run row says CANCELLED.
                let new_state = if cancelled {
                    "CATCHING_UP"
                } else if failed == 0 {
                    "LIVE"
                } else if failed < results.len() as i64 / 2 {
                    "CATCHING_UP"
                } else {
                    "ERROR"
                };
                set_state(&conn, new_state);
                let _ = update_run(
                    &conn,
                    run_id,
                    RunUpdate {
                        state: Some(if cancelled {
                            "CANCELLED"
                        } else if failed == 0 {
                            "LIVE"
                        } else {
                            "ERROR"
                        }),
                        resources_done: None,
                        resources_total: None,
                        records_processed: None,
                        errors: None,
                        detail: None,
                        finished: true,
                    },
                );
            }
            let processed: i64 = results.iter().map(|r| r.processed).sum();
            {
                let conn = self.lock();
                let n = crate::interaction_current::backfill_all(&conn).unwrap_or(0);
                if n > 0 {
                    tracing::debug!(conversations = n, "interaction snapshots built");
                }
                let _ = crate::audit::log_event(
                    &conn,
                    if cancelled || failed > 0 { "warn" } else { "info" },
                    "sync",
                    &format!(
                        "Initial sync {}: {processed} records processed, {failed} resource(s) failed",
                        if cancelled { "cancelled partway" } else { "finished" }
                    ),
                    None,
                    None,
                );
            }
            self.emit_sync_completed("initial", processed, failed, trigger);
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            self.emit_error("error", "sync", &format!("Initial sync failed: {e}"), None);
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
            let _ = crate::jobs::log_error(&conn, "sync", &format!("Initial sync failed: {e}"));
        }
        Ok(results)
    }

    /// `trigger` (audit item 23) says who started the run — the shell's
    /// completion toast only fires for user-initiated runs, so the
    /// adaptive auto-sync timer no longer toasts ~once a minute.
    pub async fn incremental_sync(
        &self,
        trigger: crate::events::SyncTrigger,
    ) -> Result<Vec<ResourceSyncResult>> {
        self.guard_provider_connected()?;
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(Error::Other("A sync is already running".into()));
        }
        let _running = RunningGuard(&self.running);
        self.cancel_requested.store(false, Ordering::SeqCst);
        let run_id = {
            let conn = self.lock();
            if get_state(&conn) != "LIVE" {
                set_state(&conn, "CATCHING_UP");
            }
            start_run(&conn, "incremental")?
        };
        let mut results: Vec<ResourceSyncResult> = Vec::new();
        let outcome: Result<()> = async {
            // +3 beyond the reference set: customers, conversations, and
            // the threads backfill (clean-audit item 88 — see below).
            let total = INCREMENTAL_REFERENCE_ORDER.len() as u32 + 3;
            for resource in INCREMENTAL_REFERENCE_ORDER {
                if self.cancelled() {
                    break;
                }
                let r = self.sync_resource(resource, false).await;
                let stage_records = r.as_ref().map_or(0, |x| x.processed).max(0) as u32;
                let r = self.finish_resource(resource, r, &mut results);
                let processed: i64 = results.iter().map(|x| x.processed).sum();
                self.emit_progress(
                    resource,
                    results.len() as u32,
                    total,
                    processed,
                    stage_records,
                );
                let _ = r;
            }
            {
                let since = {
                    let conn = self.lock();
                    get_incremental_since(&conn, "customers", SYNC_OVERLAP_MINUTES)
                };
                let r = self.sync_customers(since.as_deref(), false).await;
                let processed = r.as_ref().map_or(0, |x| x.processed).max(0) as u32;
                let _ = self.finish_resource("customers", r, &mut results);
                self.emit_progress(
                    "customers",
                    results.len() as u32,
                    total,
                    results.iter().map(|x| x.processed).sum::<i64>(),
                    processed as u32,
                );
            }
            {
                let since = {
                    let conn = self.lock();
                    get_incremental_since(&conn, "conversations", SYNC_OVERLAP_MINUTES)
                };
                let r = self.sync_conversations(since.as_deref(), false).await;
                let processed = r.as_ref().map_or(0, |x| x.processed).max(0) as u32;
                let _ = self.finish_resource("conversations", r, &mut results);
                self.emit_progress(
                    "conversations",
                    results.len() as u32,
                    total,
                    results.iter().map(|x| x.processed).sum::<i64>(),
                    processed as u32,
                );
            }
            {
                // Clean-audit item 88: the threads backfill used to run
                // only inside the initial sync — a cancelled or crashed
                // first run left its thread-less conversations stranded
                // forever, because nothing ever re-ran the bounded pass.
                // It now rides every incremental too, so the backlog
                // drains at the sync cadence (200 conversations/pass,
                // same bound as the initial path).
                let r = self.sync_missing_threads().await;
                let stage_records = r.as_ref().map_or(0, |x| x.processed).max(0) as u32;
                let _ = self.finish_resource("threads", r, &mut results);
                self.emit_progress(
                    "threads",
                    results.len() as u32,
                    total,
                    results.iter().map(|x| x.processed).sum::<i64>(),
                    stage_records,
                );
            }
            let failed = results.iter().filter(|r| r.error.is_some()).count() as i64;
            {
                let conn = self.lock();
                set_state(&conn, if failed == 0 { "LIVE" } else { "CATCHING_UP" });
                let processed: i64 = results.iter().map(|r| r.processed).sum();
                let _ = update_run(
                    &conn,
                    run_id,
                    RunUpdate {
                        state: Some(if failed == 0 { "LIVE" } else { "CATCHING_UP" }),
                        resources_done: None,
                        resources_total: None,
                        records_processed: Some(processed),
                        errors: Some(failed),
                        detail: None,
                        finished: true,
                    },
                );
            }
            let processed: i64 = results.iter().map(|r| r.processed).sum();
            {
                let conn = self.lock();
                let _ = crate::audit::log_event(
                    &conn,
                    if failed == 0 { "info" } else { "warn" },
                    "sync",
                    &format!("Incremental sync finished: {processed} records processed, {failed} resource(s) failed"),
                    None,
                    None,
                );
            }
            self.emit_sync_completed("incremental", processed, failed, trigger);
            Ok(())
        }
        .await;
        if let Err(e) = outcome {
            self.emit_error(
                "error",
                "sync",
                &format!("Incremental sync failed: {e}"),
                None,
            );
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
            let _ = crate::jobs::log_error(&conn, "sync", &format!("Incremental sync failed: {e}"));
        }
        Ok(results)
    }

    fn finish_resource(
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

    pub async fn sync_single_conversation(&self, remote_id: i64) -> Result<bool> {
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

    pub async fn run_resource(&self, resource: &str, initial: bool) -> Result<ResourceSyncResult> {
        self.sync_resource(resource, initial).await
    }

    async fn sync_resource(&self, resource: &str, initial: bool) -> Result<ResourceSyncResult> {
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
            // The conversations pass above already hydrates threads; this
            // resource now only backfills rows that came back thread-less
            // (a crashed run, a rate-limited page) instead of walking the
            // whole account a second time.
            "threads" => self.sync_missing_threads().await,
            "docs_collections" => {
                let collections = self.provider.list_doc_collections().await?;
                let mut processed = 0;
                for col in &collections {
                    let categories = self.provider.list_doc_categories(col.remote_id).await?;
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
                let collections: Vec<i64> = {
                    let conn = self.lock();
                    let mut stmt = conn.prepare("SELECT remote_id FROM docs_collections")?;
                    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
                    rows.filter_map(|r| r.ok()).collect()
                };
                for col_remote in collections {
                    let articles = self.provider.list_doc_articles(col_remote).await?;
                    let mut conn = self.lock();
                    if let Some(col_local) = local_id(&conn, "docs_collections", col_remote) {
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

    async fn sync_customers(
        &self,
        since: Option<&str>,
        initial: bool,
    ) -> Result<ResourceSyncResult> {
        let resource = "customers".to_string();
        let mut processed = 0;
        let mut cursor = if initial {
            let conn = self.lock();
            get_cursor(&conn, "customers")
        } else {
            None
        };
        loop {
            let page = self
                .provider
                .list_customers(&CustomerQuery {
                    modified_since: since.map(|s| s.to_string()),
                    cursor: cursor.clone(),
                    ..Default::default()
                })
                .await?;
            {
                let conn = self.lock();
                for c in &page.items {
                    mirror::upsert_customer(&conn, c)?;
                    processed += 1;
                }
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
            failed: 0,
            error: None,
        })
    }

    async fn sync_conversations(
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
        loop {
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
                self.ingest_conversation(c, &threads, !initial).await?;
                processed += 1;
                // Real-time feel: each ingested conversation notifies the
                // UI immediately, so the inbox fills while the sync runs
                // instead of after it.
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
            // A page-level progress heartbeat even when the page is the
            // current stage (total unknown mid-walk: done = records).
            self.emit_progress(
                "conversations",
                processed.max(0) as u32,
                0,
                processed,
                page.items.len() as u32,
            );
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
            failed: 0,
            error: None,
        })
    }

    async fn ingest_conversation(
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
        }
        // A4 retrieval spine: every ingested conversation is re-chunked
        // (idempotent rebuild) so semantic search and the evidence engine see
        // fresh content. The embedding pass is enqueued only when the
        // retrieval feature is on, new chunks exist, and one isn't already
        // queued — chunking itself stays on regardless (it is local, cheap,
        // and future-proof: enabling embeddings later picks chunks up).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbHandle;

    fn temp_db() -> DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-sync-engine-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("engine.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        DbHandle::new(conn)
    }

    fn empty_engine(db: &DbHandle) -> SyncEngine {
        SyncEngine::new(
            db.clone(),
            Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        )
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    #[test]
    fn r2_engine_refuses_every_sync_against_the_empty_provider() {
        let db = temp_db();
        // Poison the state the way a pre-1.4.0 install would have been.
        db.with_write(|conn| {
            set_state(conn, "LIVE");
        });
        let engine = empty_engine(&db);
        let rt = runtime();

        let err = rt
            .block_on(engine.initial_sync(crate::events::SyncTrigger::Manual))
            .expect_err("the engine must refuse to run against the empty provider");
        assert!(
            err.to_string().contains("not connected"),
            "the refusal must be legible: {err}"
        );
        let err = rt
            .block_on(engine.incremental_sync(crate::events::SyncTrigger::Manual))
            .expect_err("incremental must refuse too");
        assert!(err.to_string().contains("not connected"));
        let err = rt
            .block_on(engine.sync_docs_only(crate::events::SyncTrigger::Manual))
            .expect_err("docs-only must refuse too");
        assert!(err.to_string().contains("not connected"));
        assert!(
            rt.block_on(engine.sync_single_conversation(1)).is_err(),
            "single-conversation sync must refuse on the empty provider"
        );
        assert!(rt.block_on(engine.reconcile()).is_err());

        // The poisoned state and the checkpoints are untouched by the
        // refusals: no run rows, no watermark movement.
        let (state, runs, checkpoints) = db.with_read(|conn| {
            (
                get_state(conn),
                conn.query_row("SELECT COUNT(*) FROM sync_runs", [], |r| r.get::<_, i64>(0))
                    .unwrap_or(0),
                conn.query_row(
                    "SELECT COUNT(*) FROM sync_checkpoints WHERE status = 'ok'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0),
            )
        });
        assert_eq!(state, "LIVE", "a refused run must not touch the state");
        assert_eq!(runs, 0, "a refused run must not start a run row");
        assert_eq!(checkpoints, 0, "no watermark is stamped");
    }

    #[test]
    fn r2_transition_policy_resets_a_poisoned_empty_session() {
        let db = temp_db();
        db.with_write(|conn| {
            set_state(conn, "LIVE");
        });
        let after = on_provider_kind_transition(&db.write_guard(), Some("empty"), "real");
        assert_eq!(after, "NEW", "empty→real forgets poisoned watermarks");
        assert_eq!(db.with_read(get_state), "NEW");
    }

    #[test]
    fn r2_transition_policy_keeps_a_healthy_real_session() {
        let db = temp_db();
        db.with_write(|conn| {
            set_state(conn, "LIVE");
        });
        let after = on_provider_kind_transition(&db.write_guard(), Some("real"), "real");
        assert_eq!(after, "LIVE", "same-kind transitions are no-ops");
        assert_eq!(db.with_read(get_state), "LIVE");
        let after = on_provider_kind_transition(&db.write_guard(), Some("real"), "empty");
        assert_eq!(after, "LIVE", "deactivating preserves the mirror state");
        assert_eq!(db.with_read(get_state), "LIVE");
    }
}

#[cfg(test)]
mod r8_freshness_tests {
    use super::*;

    fn temp_conn() -> Connection {
        let dir = std::env::temp_dir().join(format!(
            "spp-freshness-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("freshness.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    /// R8 (D10): the header ring's data source — an ok checkpoint stamps
    /// a fresh age, a database with none reports "never".
    #[test]
    fn last_sync_age_reflects_the_newest_ok_checkpoint() {
        let conn = temp_conn();
        assert_eq!(
            last_sync_age_seconds(&conn),
            None,
            "a database with no successful checkpoints has never synced"
        );
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sync_checkpoints (resource, status, last_success_at)
             VALUES ('conversations', 'ok', ?1)",
            params![now],
        )
        .expect("stamp");
        let age = last_sync_age_seconds(&conn).expect("age after a stamp");
        assert!(
            age <= 2,
            "a just-stamped checkpoint is at most 2 s old: {age}"
        );

        // A second, older checkpoint must not win the MAX().
        let older = (chrono::Utc::now() - chrono::Duration::hours(3)).to_rfc3339();
        conn.execute(
            "INSERT INTO sync_checkpoints (resource, status, last_success_at)
             VALUES ('customers', 'ok', ?1)",
            params![older],
        )
        .expect("stamp");
        let age = last_sync_age_seconds(&conn).expect("age");
        assert!(age <= 2, "the newest ok checkpoint wins: {age}");
    }
}
