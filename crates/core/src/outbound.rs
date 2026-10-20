//! Outbound job dispatch — the missing half of the composer pipeline.
//!
//! Since the Phase 4 cutover the reply/note/status/assign path was
//! enqueue-only: the composer inserted a row into `outbound_jobs` and
//! nothing ever picked it up, so user actions never reached Help Scout
//! while the UI reported green success ("Reply queued as job #N"). This
//! module is the dispatcher the cutover lost: `WorkerManager` claims
//! queued rows every tick, performs the live provider call, records the
//! attempt, and either confirms (then pulls the remote truth back into
//! the mirror) or retries with exponential backoff.
//!
//! Status lifecycle:
//! `queued` → `sending` → `confirmed` (terminal success)
//!                     ↘ `queued` (retryable failure, `updated_at` holds
//!                        the next-run gate: claim only picks rows whose
//!                        `updated_at <= now`)
//!                     ↘ `failed` (terminal failure: non-retryable error,
//!                        or retries exhausted)

use rusqlite::{Connection, params};

use crate::error::Result;

/// Claim/execute budget per worker tick. User actions are low-volume and
/// deserve priority over background job kinds.
pub const OUTBOUND_BATCH_PER_TICK: usize = 4;

/// Default retry budget for outbound writes (mirrors the jobs table).
pub const OUTBOUND_MAX_ATTEMPTS: i64 = 3;

#[derive(Debug, Clone)]
pub struct OutboundJob {
    pub id: i64,
    pub kind: String,
    pub conversation_local_id: Option<i64>,
    pub payload: String,
    pub attempts: i64,
    pub max_attempts: i64,
}

/// Transactionally claim the oldest runnable outbound job (queued, past
/// its backoff gate), marking it `sending`. Returns `None` when nothing
/// is runnable — the common case.
pub fn claim_next(conn: &mut Connection) -> Result<Option<OutboundJob>> {
    let tx = conn.transaction()?;
    let row: Option<(i64, String, Option<i64>, String, i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT id, kind, conversation_id, payload, attempts, max_attempts
               FROM outbound_jobs
              WHERE status = 'queued' AND updated_at <= datetime('now')
              ORDER BY id ASC
              LIMIT 1;",
        )?;
        match stmt.query_row([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        }) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(other) => return Err(other.into()),
        }
    };
    let Some((id, kind, conversation_local_id, payload, attempts, max_attempts)) = row else {
        return Ok(None);
    };
    tx.execute(
        "UPDATE outbound_jobs SET status = 'sending', updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    tx.commit()?;
    Ok(Some(OutboundJob {
        id,
        kind,
        conversation_local_id,
        payload,
        attempts,
        max_attempts,
    }))
}

/// Mark a job as delivered: status `confirmed`, attempts bumped, the
/// provider result retained for the Diagnostics view.
pub fn mark_confirmed(conn: &Connection, id: i64, remote_result: &str) -> Result<()> {
    conn.execute(
        "UPDATE outbound_jobs
            SET status = 'confirmed', attempts = attempts + 1, error = NULL,
                remote_result = ?2, confirmed_at = datetime('now'),
                updated_at = datetime('now')
          WHERE id = ?1",
        params![id, remote_result],
    )?;
    Ok(())
}

/// Mark an attempt as failed. Retryable failures re-queue with an
/// exponential backoff gate on `updated_at` (5s << attempt, capped at 5
/// minutes); exhausted or permanent failures land in `failed` with the
/// error text the UI surfaces verbatim.
///
/// Returns whether the job will be retried.
pub fn mark_failed(conn: &Connection, id: i64, error: &str, retryable: bool) -> Result<bool> {
    let error: String = error.chars().take(1000).collect();
    let row: Option<(i64, i64)> = conn
        .prepare("SELECT attempts, max_attempts FROM outbound_jobs WHERE id = ?1")?
        .query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?)))
        .ok();
    let Some((attempts, max_attempts)) = row else {
        return Ok(false);
    };
    let max_attempts = if max_attempts <= 0 {
        OUTBOUND_MAX_ATTEMPTS
    } else {
        max_attempts
    };
    let will_retry = retryable && attempts < max_attempts;
    if will_retry {
        let shift = u32::try_from(attempts).unwrap_or(0).min(16);
        let delay_secs = (5i64 << shift).min(300);
        conn.execute(
            "UPDATE outbound_jobs
                SET status = 'queued', attempts = attempts + 1, error = ?2,
                    updated_at = datetime('now', '+' || ?3 || ' seconds')
              WHERE id = ?1",
            params![id, error, delay_secs],
        )?;
    } else {
        conn.execute(
            "UPDATE outbound_jobs
                SET status = 'failed', attempts = attempts + 1, error = ?2,
                    updated_at = datetime('now')
              WHERE id = ?1",
            params![id, error],
        )?;
    }
    Ok(will_retry)
}

/// Boot recovery: a crash mid-send leaves rows stuck in `sending`
/// forever. Re-queue them so the dispatcher can finish the user's work.
pub fn recover_stale_sending(conn: &Connection) -> Result<usize> {
    let n = conn.execute(
        "UPDATE outbound_jobs SET status = 'queued', updated_at = datetime('now')
          WHERE status = 'sending' AND updated_at < datetime('now', '-10 minutes')",
        [],
    )?;
    if n > 0 {
        tracing::info!(
            count = n,
            operation = "outbound_recover",
            "Re-queued stuck outbound job(s)"
        );
    }
    Ok(n)
}

/// Queue counters for the Diagnostics page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutboundStats {
    pub queued: i64,
    pub sending: i64,
    pub confirmed: i64,
    pub failed: i64,
}

pub fn stats(conn: &Connection) -> OutboundStats {
    conn.query_row(
        "SELECT SUM(CASE WHEN status='queued' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status='sending' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status='confirmed' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END)
         FROM outbound_jobs",
        [],
        |r| {
            Ok(OutboundStats {
                queued: r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                sending: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                confirmed: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                failed: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        },
    )
    .unwrap_or_default()
}

/// The latest outbound job for a conversation — the inbox derives its
/// honest per-conversation status line ("delivered ✓" / "sending…" /
/// "failed: …") from this row.
#[derive(Debug, Clone, PartialEq)]
pub struct LatestOutbound {
    pub kind: String,
    pub status: String,
    pub error: Option<String>,
    pub updated_at: String,
}

pub fn latest_for_conversation(conn: &Connection, conversation_id: i64) -> Option<LatestOutbound> {
    conn.query_row(
        "SELECT kind, status, error, updated_at FROM outbound_jobs
          WHERE conversation_id = ?1 AND kind IN ('create_reply','create_note','update_status','assign')
          ORDER BY id DESC LIMIT 1",
        params![conversation_id],
        |r| {
            Ok(LatestOutbound {
                kind: r.get(0)?,
                status: r.get(1)?,
                error: r.get(2)?,
                updated_at: r.get(3)?,
            })
        },
    )
    .ok()
}

/// Migration m051 (schema v42): the outbound queue becomes a real
/// dispatch lane — retries get a budget, rows stuck mid-send by an older
/// crash are re-queued, and installs on the old 5-minute default move to
/// the 1-minute poll so "real-time" means what it says.
pub fn apply_m051(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "UPDATE outbound_jobs SET max_attempts = 3
          WHERE max_attempts IS NULL OR max_attempts < 3;
         UPDATE outbound_jobs SET status = 'queued', updated_at = datetime('now')
          WHERE status = 'sending';
         UPDATE application_settings SET value = '1', updated_at = datetime('now')
          WHERE key = 'sync_interval_minutes' AND value = '5';
         UPDATE app_state SET schema_version = 42 WHERE id = 1;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> crate::db::DbHandle {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn enqueue_job(conn: &Connection, kind: &str) -> i64 {
        conn.execute(
            "INSERT INTO outbound_jobs (kind, conversation_id, payload, status, max_attempts)
             VALUES (?1, NULL, '{}', 'queued', 3)",
            params![kind],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn claim_marks_sending_and_returns_payload() {
        let handle = db();
        let mut guard = handle.write_guard();
        let id = enqueue_job(&guard, "create_reply");
        let claimed = claim_next(&mut guard).expect("claim").expect("one job");
        assert_eq!(claimed.id, id);
        assert_eq!(claimed.kind, "create_reply");
        assert_eq!(claimed.attempts, 0);
        let status: String = guard
            .query_row(
                "SELECT status FROM outbound_jobs WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "sending");
        // A row mid-send is not claimable again.
        assert!(
            claim_next(&mut guard).expect("claim").is_none(),
            "a sending row must not be claimable"
        );
        mark_confirmed(&guard, id, "ok").unwrap();
    }

    #[test]
    fn failed_retry_uses_backoff_gate() {
        let handle = db();
        let conn = handle.write_guard();
        let id = enqueue_job(&conn, "create_reply");
        let retry = mark_failed(&conn, id, "boom", true).expect("mark");
        assert!(retry, "first retryable failure must re-queue");
        let status: String = conn
            .query_row(
                "SELECT status FROM outbound_jobs WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "queued");
        let gated: bool = conn
            .query_row(
                "SELECT updated_at > datetime('now') FROM outbound_jobs WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(gated, "retry must be gated on a future updated_at");
    }

    #[test]
    fn failed_exhausted_marks_failed() {
        let handle = db();
        let conn = handle.write_guard();
        let id = enqueue_job(&conn, "create_reply");
        conn.execute(
            "UPDATE outbound_jobs SET attempts = 3 WHERE id = ?1",
            params![id],
        )
        .unwrap();
        let retry = mark_failed(&conn, id, "permanent boom", true).expect("mark");
        assert!(!retry, "exhausted attempts must not re-queue");
        let status: String = conn
            .query_row(
                "SELECT status FROM outbound_jobs WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "failed");
    }

    #[test]
    fn non_retryable_failure_is_terminal() {
        let handle = db();
        let conn = handle.write_guard();
        let id = enqueue_job(&conn, "create_reply");
        let retry = mark_failed(&conn, id, "400 bad request", false).expect("mark");
        assert!(!retry, "non-retryable failures must not re-queue");
        let status: String = conn
            .query_row(
                "SELECT status FROM outbound_jobs WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "failed");
    }

    #[test]
    fn confirmed_records_remote_result() {
        let handle = db();
        let conn = handle.write_guard();
        let id = enqueue_job(&conn, "create_note");
        mark_confirmed(&conn, id, "thread 98765").expect("confirm");
        let (status, result): (String, Option<String>) = conn
            .query_row(
                "SELECT status, remote_result FROM outbound_jobs WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "confirmed");
        assert_eq!(result.as_deref(), Some("thread 98765"));
    }

    #[test]
    fn recover_requeues_stale_sending_rows() {
        let handle = db();
        let conn = handle.write_guard();
        let id = enqueue_job(&conn, "create_reply");
        conn.execute(
            "UPDATE outbound_jobs SET status = 'sending',
                 updated_at = datetime('now', '-30 minutes') WHERE id = ?1",
            params![id],
        )
        .unwrap();
        let n = recover_stale_sending(&conn).unwrap();
        assert_eq!(n, 1);
        let status: String = conn
            .query_row(
                "SELECT status FROM outbound_jobs WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "queued");
    }

    #[test]
    fn stats_and_latest_for_conversation_read_the_queue() {
        let handle = db();
        let conn = handle.write_guard();
        // outbound_jobs.conversation_id has an FK to conversations(id).
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, status)
             VALUES (700, 7, 'queue test', 'active')",
            [],
        )
        .unwrap();
        let conv = conn.last_insert_rowid();
        let a = enqueue_job(&conn, "create_reply");
        conn.execute(
            "UPDATE outbound_jobs SET conversation_id = ?1 WHERE id = ?2",
            params![conv, a],
        )
        .unwrap();
        mark_confirmed(&conn, a, "delivered").unwrap();
        enqueue_job(&conn, "create_note");
        let s = stats(&conn);
        assert_eq!(s.confirmed, 1);
        assert_eq!(s.queued, 1);
        let latest = latest_for_conversation(&conn, conv).expect("latest");
        assert_eq!(latest.kind, "create_reply");
        assert_eq!(latest.status, "confirmed");
    }
}
