use rusqlite::{Connection, params};

use crate::error::{Error, Result};

pub(crate) fn ensure_jobs_table(conn: &Connection) -> Result<()> {
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
            result        TEXT,
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
    let has_result: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('jobs') WHERE name = 'result'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !has_result {
        conn.execute("ALTER TABLE jobs ADD COLUMN result TEXT", [])?;
    }
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

/// True when at least one queued (not yet claimed) job of `kind` exists on `queue`.
/// Used by UI callers to coalesce repeated triggers (e.g. per-keystroke commits).
pub fn has_pending(conn: &Connection, queue: &str, kind: &str) -> Result<bool> {
    ensure_jobs_table(conn)?;
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM jobs WHERE queue = ?1 AND type = ?2 AND status = 'queued'",
        params![queue, kind],
        |r| r.get(0),
    )?;
    Ok(pending > 0)
}

pub(crate) fn claim_next(conn: &mut Connection, queue: Option<&str>) -> Result<Option<ClaimedJob>> {
    let tx = conn.transaction()?;
    type ClaimRow = (i64, String, String, i64);
    let select_row = |stmt: &mut rusqlite::Statement<'_>,
                      params: &[&dyn rusqlite::ToSql]|
     -> rusqlite::Result<Option<ClaimRow>> {
        let mut rows = stmt.query(params)?;
        match rows.next()? {
            Some(r) => Ok(Some((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))),
            None => Ok(None),
        }
    };
    let row: Option<ClaimRow> = match queue {
        Some(q) => {
            let mut stmt = tx.prepare(
                "SELECT id, type, payload, attempt
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
                "SELECT id, type, payload, attempt
                   FROM jobs
                  WHERE status = 'queued'
                    AND (run_at IS NULL OR run_at <= datetime('now'))
                  ORDER BY priority ASC, id ASC
                  LIMIT 1;",
            )?;
            select_row(&mut stmt, &[])?
        }
    };

    let Some((id, kind, payload, attempt)) = row else {
        return Ok(None);
    };

    // AU2-14: guard the claim on the status the row was selected with, so a
    // concurrent claimer (or a state flip between select and update) can never
    // re-claim a job that is no longer queued. Unreachable under the
    // single-writer lane today; the guard makes the invariant local.
    let claimed = tx.execute(
        "UPDATE jobs
            SET status = 'running',
                started_at = datetime('now'),
                attempt = ?1
          WHERE id = ?2 AND status = 'queued'",
        params![attempt + 1, id],
    )?;
    tx.commit()?;

    if claimed == 0 {
        return Ok(None);
    }

    Ok(Some(ClaimedJob { id, kind, payload }))
}

#[derive(Debug, Clone)]
pub(crate) struct ClaimedJob {
    pub id: i64,
    pub kind: String,
    pub payload: String,
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

pub(crate) fn complete_with_result(conn: &Connection, id: i64, result: &str) -> Result<()> {
    let rows = conn.execute(
        "UPDATE jobs SET status = 'completed', completed_at = datetime('now'), error = NULL,
                         result = ?2
          WHERE id = ?1 AND status = 'running'",
        params![id, result],
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

pub(crate) fn park_job(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET status = 'awaiting_approval', completed_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub(crate) fn recover_stale_jobs(conn: &Connection) -> Result<usize> {
    let rows = conn.execute(
        "UPDATE jobs SET status = 'queued', error = 'Recovered after restart'
          WHERE status = 'running' AND started_at < datetime('now', '-30 minutes')",
        [],
    )?;
    Ok(rows)
}

pub(crate) fn requeue_stale_running_jobs(
    conn: &Connection,
    threshold_minutes: i64,
    executing: &std::collections::HashSet<i64>,
    boot_unix: i64,
) -> Result<usize> {
    let stale: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM jobs
              WHERE status = 'running'
                AND started_at < datetime('now', ?1)",
        )?;
        let rows = stmt.query_map(params![format!("-{threshold_minutes} minutes")], |r| {
            r.get::<_, i64>(0)
        })?;
        rows.filter_map(|r| r.ok())
            .filter(|id| !executing.contains(id))
            .collect()
    };
    let mut requeued = 0usize;
    for id in stale {
        let started_unix: i64 = conn
            .query_row(
                "SELECT CAST(strftime('%s', started_at) AS INTEGER) FROM jobs WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or(i64::MAX);
        if started_unix >= boot_unix {
            continue;
        }
        let n = conn.execute(
            "UPDATE jobs SET status = 'queued',
                 error = 'Re-queued by maintenance sweep: orphaned by a previous process'
              WHERE id = ?1 AND status = 'running'",
            params![id],
        )?;
        requeued += n;
    }
    Ok(requeued)
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

pub(crate) fn record_outbound_attempt(
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

#[cfg(test)]
mod batch20_audit24_tests {
    use super::*;

    #[test]
    fn audit24_complete_with_result_pins_the_output_to_the_job_row() {
        let mut conn = Connection::open_in_memory().expect("db");
        ensure_jobs_table(&conn).expect("ensure");
        let job_id = enqueue_on(&conn, "ai", "rewrite_composer", "{}", 3, 2).expect("enqueue");
        let claimed = claim_next(&mut conn, Some("ai"))
            .expect("claim")
            .expect("one job");
        assert_eq!(claimed.id, job_id);

        complete_with_result(&conn, job_id, "rewritten text").expect("complete");
        let (status, result): (String, Option<String>) = conn
            .query_row(
                "SELECT status, result FROM jobs WHERE id = ?1",
                params![job_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("job row");
        assert_eq!(status, "completed");
        assert_eq!(result.as_deref(), Some("rewritten text"));

        assert!(complete_with_result(&conn, job_id, "again").is_err());
    }

    #[test]
    fn audit24_old_databases_gain_the_result_column_lazily() {
        let mut conn = Connection::open_in_memory().expect("db");
        conn.execute_batch(
            "CREATE TABLE jobs (
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
            );",
        )
        .expect("legacy schema");
        conn.execute(
            "INSERT INTO jobs (queue, type, status, payload, run_at)
             VALUES ('ai', 'translate_composer', 'queued', '{}', datetime('now'))",
            [],
        )
        .expect("legacy row");
        let legacy_id = conn.last_insert_rowid();
        let before = conn.query_row(
            "SELECT type, result FROM jobs WHERE id = ?1",
            params![legacy_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        );
        assert!(
            before.is_err(),
            "the legacy row must be unreadable before the heal (no result column)"
        );
        ensure_jobs_table(&conn).expect("heal");
        let (kind, result): (String, Option<String>) = conn
            .query_row(
                "SELECT type, result FROM jobs WHERE id = ?1",
                params![legacy_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("read after heal");
        assert_eq!(kind, "translate_composer");
        assert!(result.is_none());
        claim_next(&mut conn, Some("ai"))
            .expect("claim")
            .expect("job");
        complete_with_result(&conn, legacy_id, "translated text").expect("complete");
        let result: Option<String> = conn
            .query_row(
                "SELECT result FROM jobs WHERE id = ?1",
                params![legacy_id],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(result, Some("translated text".to_string()));
    }
}
