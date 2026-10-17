use rusqlite::{Connection, params};

use crate::error::{Error, Result};

pub mod status {
    pub const QUEUED: &str = "queued";
    pub const RUNNING: &str = "running";
    pub const COMPLETED: &str = "completed";
    pub const FAILED: &str = "failed";
    pub const CANCELLED: &str = "cancelled";
    pub const AWAITING_APPROVAL: &str = "awaiting_approval";
}

pub fn ensure_jobs_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS jobs (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            queue         TEXT    NOT NULL,
            type          TEXT    NOT NULL,
            priority      INTEGER NOT NULL DEFAULT 2,
            status        TEXT    NOT NULL DEFAULT 'queued',
            payload       TEXT,
            attempt       INTEGER DEFAULT 0,
            max_attempts  INTEGER DEFAULT 3,
            error         TEXT,
            run_at        TEXT,
            locked_by     TEXT,
            locked_at     TEXT,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
            started_at    TEXT,
            completed_at  TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_jobs_status
            ON jobs (status, priority, run_at);
        CREATE INDEX IF NOT EXISTS idx_jobs_queue
            ON jobs (queue, status);",
    )?;
    Ok(())
}

pub fn enqueue(conn: &Connection, kind: &str, payload: &str) -> Result<i64> {
    enqueue_on(conn, "sync", kind, payload, 2, 3)
}

pub fn enqueue_on(
    conn: &Connection,
    queue: &str,
    kind: &str,
    payload: &str,
    priority: i64,
    max_attempts: i64,
) -> Result<i64> {
    ensure_jobs_table(conn)?;
    conn.execute(
        "INSERT INTO jobs (queue, type, priority, payload, max_attempts, run_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
        params![queue, kind, priority, payload, max_attempts],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn claim_next(conn: &mut Connection, queue: Option<&str>) -> Result<Option<ClaimedJob>> {
    let tx = conn.transaction()?;
    type ClaimRow = (i64, String, String, i64, i64);
    let select_row = |stmt: &mut rusqlite::Statement<'_>,
                      params: &[&dyn rusqlite::ToSql]|
     -> rusqlite::Result<Option<ClaimRow>> {
        let mut rows = stmt.query(params)?;
        match rows.next()? {
            Some(r) => Ok(Some((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
            ))),
            None => Ok(None),
        }
    };
    let row: Option<ClaimRow> = match queue {
        Some(q) => {
            let mut stmt = tx.prepare(
                "SELECT id, type, payload, attempt, max_attempts
                   FROM jobs
                  WHERE status = 'queued'
                    AND (run_at IS NULL OR run_at <= datetime('now'))
                    AND queue = ?
                  ORDER BY priority ASC, id ASC
                  LIMIT 1;",
            )?;
            select_row(&mut stmt, &[&q])?
        }
        None => {
            let mut stmt = tx.prepare(
                "SELECT id, type, payload, attempt, max_attempts
                   FROM jobs
                  WHERE status = 'queued'
                    AND (run_at IS NULL OR run_at <= datetime('now'))
                  ORDER BY priority ASC, id ASC
                  LIMIT 1;",
            )?;
            select_row(&mut stmt, &[])?
        }
    };

    let Some((id, kind, payload, attempt, max_attempts)) = row else {
        return Ok(None);
    };

    tx.execute(
        "UPDATE jobs
            SET status = 'running',
                started_at = datetime('now'),
                attempt = ?1
          WHERE id = ?2",
        params![attempt + 1, id],
    )?;
    tx.commit()?;

    Ok(Some(ClaimedJob {
        id,
        kind,
        payload,
        attempts: attempt + 1,
        max_attempts,
    }))
}

#[derive(Debug, Clone)]
pub struct ClaimedJob {
    pub id: i64,
    pub kind: String,
    pub payload: String,
    pub attempts: i64,
    pub max_attempts: i64,
}

pub fn complete(conn: &Connection, id: i64) -> Result<()> {
    let rows = conn.execute(
        "UPDATE jobs SET status = 'completed', completed_at = datetime('now'), error = NULL
          WHERE id = ?1 AND status = 'running'",
        params![id],
    )?;
    if rows == 0 {
        return Err(Error::Other(
            format!("job {id} not in 'running' state; cannot complete").into(),
        ));
    }
    Ok(())
}

pub fn fail(conn: &Connection, id: i64, error: &str, retryable: bool) -> Result<()> {
    let row: Option<(i64, i64)> = conn
        .prepare("SELECT attempt, max_attempts FROM jobs WHERE id = ?1 AND status = 'running'")?
        .query_row(params![id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })
        .ok();

    let Some((attempt, max_attempts)) = row else {
        return Err(Error::Other(
            format!("job {id} not in 'running' state; cannot fail").into(),
        ));
    };

    let error: String = error.chars().take(1000).collect();
    let can_retry = retryable && attempt < max_attempts;
    if !can_retry {
        conn.execute(
            "UPDATE jobs SET status = 'failed', completed_at = datetime('now'), error = ?1 WHERE id = ?2",
            params![error, id],
        )?;
    } else {
        let shift = u32::try_from(attempt).unwrap_or(0).min(20);
        let delay_secs = (5i64 << shift).min(300);
        conn.execute(
            "UPDATE jobs
                SET status = 'queued',
                    locked_by = NULL,
                    locked_at = NULL,
                    run_at = datetime('now', '+' || ?1 || ' seconds'),
                    error = ?2
              WHERE id = ?3",
            params![delay_secs, error, id],
        )?;
    }
    Ok(())
}

pub fn park_job(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET status = 'awaiting_approval', completed_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueJob {
    pub id: i64,
    pub queue: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub priority: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    pub attempt: i64,
    pub max_attempts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

pub fn list_jobs(
    conn: &Connection,
    status: Option<&str>,
    queue: Option<&str>,
    limit: i64,
) -> Result<Vec<QueueJob>> {
    let mut sql = String::from("SELECT * FROM jobs");
    let mut conds: Vec<&str> = Vec::new();
    if status.is_some() {
        conds.push("status = ?1");
    }
    if queue.is_some() {
        conds.push("queue = ?");
    }
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
    }
    sql.push_str(" ORDER BY id DESC LIMIT ?");
    let mut stmt = conn.prepare(&sql)?;
    let limit_str = limit.to_string();
    let rows = stmt.query_map(
        [
            status.unwrap_or(""),
            queue.unwrap_or(""),
            limit_str.as_str(),
        ],
        map_queue_job,
    )?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn map_queue_job(r: &rusqlite::Row<'_>) -> rusqlite::Result<QueueJob> {
    let payload: Option<String> = r.get("payload")?;
    Ok(QueueJob {
        id: r.get("id")?,
        queue: r.get("queue")?,
        kind: r.get("type")?,
        priority: r.get("priority")?,
        status: r.get("status")?,
        payload: payload
            .as_deref()
            .and_then(|p| serde_json::from_str(p).ok()),
        attempt: r.get("attempt")?,
        max_attempts: r.get("max_attempts")?,
        error: r.get("error")?,
        run_at: r.get("run_at")?,
        locked_by: r.get("locked_by")?,
        locked_at: r.get("locked_at")?,
        created_at: r.get("created_at")?,
        started_at: r.get("started_at")?,
        completed_at: r.get("completed_at")?,
    })
}

pub fn get_job(conn: &Connection, id: i64) -> Result<Option<QueueJob>> {
    let job = conn
        .prepare("SELECT * FROM jobs WHERE id = ?1")?
        .query_row(params![id], map_queue_job)
        .ok();
    Ok(job)
}

pub fn queue_stats(conn: &Connection) -> Result<(i64, i64, i64, i64)> {
    let (queued, running, failed, completed): (i64, i64, i64, i64) = conn.query_row(
        "SELECT SUM(CASE WHEN status='queued' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status='running' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status='completed' THEN 1 ELSE 0 END)
         FROM jobs",
        [],
        |r| {
            Ok((
                r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            ))
        },
    )?;
    Ok((queued, running, failed, completed))
}

pub fn retry_job(conn: &Connection, id: i64, patch: Option<&str>) -> Result<Option<bool>> {
    let existing: Option<(Option<String>, String)> = conn
        .prepare(
            "SELECT payload, status FROM jobs
              WHERE id = ?1 AND status IN ('failed','cancelled','awaiting_approval')",
        )?
        .query_row(params![id], |r| {
            Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?))
        })
        .ok();
    let Some((payload, _status)) = existing else {
        let any: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ?1)",
            params![id],
            |r| r.get(0),
        )?;
        return Ok(if any { Some(false) } else { None });
    };
    if let Some(patch_json) = patch
        && let Some(existing_payload) = payload.as_deref()
    {
        let merged = merge_payloads(existing_payload, patch_json);
        conn.execute(
            "UPDATE jobs SET payload = ?1 WHERE id = ?2",
            params![merged, id],
        )?;
    }
    let rows = conn.execute(
        "UPDATE jobs SET status = 'queued', attempt = 0, error = NULL, run_at = datetime('now')
          WHERE id = ?1",
        params![id],
    )?;
    Ok(Some(rows > 0))
}

fn merge_payloads(existing: &str, patch: &str) -> String {
    let mut base = serde_json::from_str::<serde_json::Value>(existing)
        .unwrap_or_else(|_| serde_json::json!({}));
    let add =
        serde_json::from_str::<serde_json::Value>(patch).unwrap_or_else(|_| serde_json::json!({}));
    if let (Some(base_map), Some(add_map)) = (base.as_object_mut(), add.as_object()) {
        for (k, v) in add_map {
            base_map.insert(k.clone(), v.clone());
        }
    }
    base.to_string()
}

pub fn cancel_job(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE jobs SET status = 'cancelled', completed_at = datetime('now')
          WHERE id = ?1 AND status IN ('queued','running','awaiting_approval')",
        params![id],
    )?;
    Ok(rows > 0)
}

pub fn clear_completed(conn: &Connection) -> Result<usize> {
    let rows = conn.execute(
        "DELETE FROM jobs
          WHERE status IN ('completed','cancelled')
            AND completed_at < datetime('now', '-1 day')",
        [],
    )?;
    Ok(rows)
}

pub fn recover_stale_jobs(conn: &Connection) -> Result<usize> {
    let rows = conn.execute(
        "UPDATE jobs SET status = 'queued', error = 'Recovered after restart'
          WHERE status = 'running' AND started_at < datetime('now', '-30 minutes')",
        [],
    )?;
    Ok(rows)
}

pub fn requeue_stale_running_jobs(conn: &Connection, threshold_minutes: i64) -> Result<usize> {
    let rows = conn.execute(
        "UPDATE jobs SET status = 'queued',
             error = 'Re-queued by maintenance sweep: running longer than threshold'
          WHERE status = 'running'
            AND started_at < datetime('now', ?1)",
        params![format!("-{threshold_minutes} minutes")],
    )?;
    Ok(rows)
}

pub fn list_outbound_jobs(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    let sql = if status.is_some() {
        "SELECT * FROM outbound_jobs WHERE status = ?1 ORDER BY id DESC LIMIT ?2"
    } else {
        "SELECT * FROM outbound_jobs ORDER BY id DESC LIMIT ?2"
    };
    let mut stmt = conn.prepare(sql)?;
    let columns: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|c| (*c).to_string())
        .collect();
    let limit_str = limit.to_string();
    let rows = stmt.query_map([status.unwrap_or(""), limit_str.as_str()], |r| {
        let mut obj = serde_json::Map::new();
        for (idx, col) in columns.iter().enumerate() {
            let v: rusqlite::types::Value = r.get(idx)?;
            obj.insert(
                col.clone(),
                match v {
                    rusqlite::types::Value::Null => serde_json::Value::Null,
                    rusqlite::types::Value::Integer(i) => serde_json::Value::from(i),
                    rusqlite::types::Value::Real(f) => serde_json::Value::from(f),
                    rusqlite::types::Value::Text(s) => {
                        if col == "payload" {
                            serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
                        } else {
                            serde_json::Value::String(s)
                        }
                    }
                    rusqlite::types::Value::Blob(b) => {
                        serde_json::Value::String(String::from_utf8_lossy(&b).to_string())
                    }
                },
            );
        }
        Ok(serde_json::Value::Object(obj))
    })?;
    for v in rows.flatten() {
        out.push(v);
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub fn audit(
    conn: &Connection,
    actor: &str,
    action: &str,
    conversation_id: Option<i64>,
    before_state: Option<&str>,
    after_state: Option<&str>,
    remote_operation: Option<&str>,
    remote_result: Option<&str>,
    ai_involvement: bool,
) -> Result<()> {
    audit_entry(
        conn,
        actor,
        action,
        conversation_id,
        before_state,
        after_state,
        remote_operation,
        remote_result,
        ai_involvement,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn audit_entry(
    conn: &Connection,
    actor: &str,
    action: &str,
    conversation_id: Option<i64>,
    before_state: Option<&str>,
    after_state: Option<&str>,
    remote_operation: Option<&str>,
    remote_result: Option<&str>,
    ai_involvement: bool,
    job_id: Option<i64>,
    correlation_id: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log (timestamp, actor, action, conversation_id, before_state,
             after_state, remote_operation, remote_result, ai_involvement, job_id, correlation_id)
         VALUES (datetime('now'), ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            actor,
            action,
            conversation_id,
            before_state,
            after_state,
            remote_operation,
            remote_result,
            i64::from(ai_involvement),
            job_id,
            correlation_id
        ],
    )?;
    Ok(())
}

pub fn record_outbound_attempt(
    conn: &Connection,
    job_id: i64,
    attempt: i64,
    summary: &str,
    status_code: Option<i64>,
    response_body: Option<&str>,
    latency_ms: Option<i64>,
) -> Result<()> {
    let summary: String = summary.chars().take(500).collect();
    let body: Option<String> = response_body.map(|b| b.chars().take(2000).collect());
    conn.execute(
        "INSERT INTO outbound_attempts (outbound_job_id, attempt, request_summary,
             status_code, response_body, latency_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![job_id, attempt, summary, status_code, body, latency_ms],
    )?;
    Ok(())
}

pub fn log_error(conn: &Connection, service: &str, message: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO application_errors (timestamp, service, message) VALUES (datetime('now'), ?1, ?2)",
        params![service, message],
    )?;
    Ok(())
}
