use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::{Error, Result};

use crate::helpscout::{
    ConversationQuery, CustomerQuery, HelpScoutProvider, HsConversation, HsThread,
};
use crate::mirror;

mod loops;

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
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

struct RunningGuard<'a>(&'a AtomicBool);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

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

pub fn cancel_orphaned_runs(conn: &Connection) -> Result<usize> {
    let n = conn.execute(
        "UPDATE sync_runs SET state = 'CANCELLED', finished_at = datetime('now') \
         WHERE state IN ('INITIALIZING', 'BACKFILLING') AND finished_at IS NULL",
        [],
    )?;
    if n > 0 {
        let _ = crate::audit::log_event(
            conn,
            "info",
            "sync",
            &format!("{n} interrupted sync run(s) marked cancelled after an unclean shutdown"),
            None,
            None,
        );
    }
    Ok(n)
}

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
    Some(shifted.format("%Y-%m-%dT%H:%M:%SZ").to_string())
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

pub fn local_id_str(conn: &Connection, table: &str, remote_id: &str) -> Option<i64> {
    conn.query_row(
        &format!("SELECT id FROM {table} WHERE remote_id = ?1"),
        params![remote_id],
        |r| r.get(0),
    )
    .ok()
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
    local_id_str(conn, "docs_collections", &c.remote_id)
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
        .as_deref()
        .and_then(|cid| local_id_str(conn, "docs_categories", cid));
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
            None => local_id_str(conn, "docs_articles", &a.remote_id)
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

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn upsert_thread(conn: &Connection, conversation_local: i64, t: &HsThread) -> Result<()> {
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
        let _ = crate::interaction_signals::record_current_interaction(conn, conversation_local);
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
    pub bus: Option<crate::event_bus::EventBus>,
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
    pub fn with_bus(mut self, bus: crate::event_bus::EventBus) -> Self {
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
        tracing::info!(
            candidates = candidates.len(),
            "thread backfill: re-fetching conversations that came back thread-less"
        );
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
        tracing::info!(
            resources = INITIAL_SYNC_ORDER.len(),
            "initial pull started — long stages emit a heartbeat every few hundred records and can take an hour or more on large accounts; leave the app running until 'Initial sync finished' appears"
        );
        let mut results: Vec<ResourceSyncResult> = Vec::new();
        let outcome: Result<()> = async {
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
                let stage_started = std::time::Instant::now();
                tracing::info!(
                    resource,
                    stage = results.len() + 1,
                    total = INITIAL_SYNC_ORDER.len(),
                    "sync resource: starting"
                );
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
                tracing::info!(
                    resource,
                    records = r.processed,
                    failed = r.failed,
                    elapsed_s = stage_started.elapsed().as_secs(),
                    "sync resource: finished"
                );
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
                let n = crate::interaction_signals::backfill_all(&conn).unwrap_or(0);
                if n > 0 {
                    tracing::debug!(conversations = n, "interaction snapshots built");
                }
                if !cancelled {
                    let already: bool = conn
                        .query_row(
                            "SELECT 1 FROM jobs WHERE type = 'rebuild_search_index'
                              AND status = 'queued' LIMIT 1",
                            [],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);
                    if !already {
                        let _ = crate::jobs::enqueue_on(
                            &conn,
                            "sync",
                            "rebuild_search_index",
                            "{}",
                            3,
                            2,
                        );
                    }
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
                let already: bool = conn
                    .query_row(
                        "SELECT 1 FROM jobs WHERE type = 'rebuild_search_index'
                          AND status = 'queued' LIMIT 1",
                        [],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);
                if !already {
                    let _ = crate::jobs::enqueue_on(
                        &conn,
                        "sync",
                        "rebuild_search_index",
                        "{}",
                        3,
                        2,
                    );
                }
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
    fn orphaned_run_rows_are_cancelled_after_an_unclean_shutdown() {
        let db = temp_db();
        db.with_write(|conn| {
            let a = start_run(conn, "initial").expect("run a");
            let _b = start_run(conn, "incremental").expect("run b");
            let _c = start_run(conn, "incremental").expect("run c");
            update_run(
                conn,
                a,
                RunUpdate {
                    state: Some("ERROR"),
                    resources_done: None,
                    resources_total: None,
                    records_processed: Some(10),
                    errors: Some(1),
                    detail: None,
                    finished: true,
                },
            )
            .expect("finish run a");

            let n = cancel_orphaned_runs(conn).expect("sweep");
            assert_eq!(n, 2, "both unfinished rows are orphans");

            let states: Vec<(i64, String, Option<String>)> = conn
                .prepare("SELECT id, state, finished_at FROM sync_runs ORDER BY id")
                .expect("prepare")
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .expect("query")
                .collect::<rusqlite::Result<_>>()
                .expect("rows");
            assert_eq!(states.len(), 3);
            assert_eq!(states[0].1, "ERROR", "the finished run is untouched");
            assert!(states[0].2.is_some());
            assert_eq!(states[1].1, "CANCELLED");
            assert!(states[1].2.is_some(), "zombies get a finished_at");
            assert_eq!(states[2].1, "CANCELLED");

            assert_eq!(cancel_orphaned_runs(conn).expect("sweep again"), 0);
            let events = crate::audit::list_recent_events(conn, 10, Some("info")).expect("events");
            assert!(
                events
                    .iter()
                    .any(|e| e.message.contains("interrupted sync run")),
                "the sweep is announced: {:?}",
                events.iter().map(|e| &e.message).collect::<Vec<_>>()
            );
        });
    }

    #[test]
    fn r2_engine_refuses_every_sync_against_the_empty_provider() {
        let db = temp_db();
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
