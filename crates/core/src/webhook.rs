use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use rusqlite::{Connection, OptionalExtension, params};
use sha1::Sha1;

use crate::error::{Error, Result};

pub const HELPSCOUT_SIGNATURE_HEADER: &str = "X-Helpscout-Signature";

type HmacSha1 = Hmac<Sha1>;

pub const SECRET_CAP: usize = 40;

#[must_use]
pub fn effective_secret(secret: &str) -> String {
    secret.chars().take(SECRET_CAP).collect()
}

#[must_use]
pub fn compute_signature(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

pub fn verify_signature(secret: &[u8], body: &[u8], provided_signature: &str) -> Result<()> {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    let provided = base64::engine::general_purpose::STANDARD
        .decode(provided_signature)
        .map_err(|_| Error::WebhookSignature)?;
    mac.verify_slice(&provided)
        .map_err(|_| Error::WebhookSignature)
}

#[derive(Debug, Clone)]
pub struct WebhookEventRecord {
    pub id: i64,
    pub event_id: Option<String>,
    pub event_type: String,
    pub payload: String,
    pub processing_state: String,
}

pub fn apply_m048_webhook_events(conn: &Connection) -> Result<()> {
    let old_schema = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='webhook_events'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    if let Some(schema) = old_schema
        && schema.contains("body")
        && !schema.contains("event_hash")
    {
        conn.execute_batch(
            "ALTER TABLE webhook_events RENAME TO webhook_events_legacy;
                 DROP TABLE IF EXISTS webhook_events_legacy;",
        )?;
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS webhook_events (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id          TEXT,
            event_hash        TEXT NOT NULL UNIQUE,
            event_type        TEXT NOT NULL,
            received_at       TEXT NOT NULL,
            payload           TEXT NOT NULL,
            processing_state  TEXT NOT NULL DEFAULT 'pending',
            processing_error  TEXT,
            attempts          INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_webhook_events_state
            ON webhook_events (processing_state, id);",
    )?;
    Ok(())
}

pub fn insert_webhook_event(
    conn: &Connection,
    event_type: &str,
    payload: &str,
    event_id: Option<&str>,
) -> Result<(i64, bool)> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("{event_type}:{payload}"));
    let hash = hex::encode(hasher.finalize());

    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM webhook_events WHERE event_hash = ?1",
            params![hash],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok((id, true));
    }

    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    conn.execute(
        "INSERT INTO webhook_events (event_id, event_hash, event_type, received_at, payload, processing_state)
         VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
        params![event_id, hash, event_type, now, payload],
    )?;
    Ok((conn.last_insert_rowid(), false))
}

pub fn set_webhook_event_state(
    conn: &Connection,
    id: i64,
    state: &str,
    error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE webhook_events SET processing_state = ?1, processing_error = ?2, attempts = attempts + 1 WHERE id = ?3",
        params![state, error, id],
    )?;
    Ok(())
}

pub fn recover_stale_webhook_events(conn: &Connection) -> Result<usize> {
    const LEASE_MINUTES: i64 = 15;
    const MAX_STALE_ATTEMPTS: i64 = 5;
    let cutoff = (chrono::Utc::now() - chrono::Duration::minutes(LEASE_MINUTES))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let stranded: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM webhook_events
              WHERE processing_state = 'processing' AND received_at < ?1",
        )?;
        let rows = stmt.query_map(params![cutoff], |r| r.get::<_, i64>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    let mut recovered = 0usize;
    for id in stranded {
        let attempts: i64 = conn
            .query_row(
                "SELECT attempts FROM webhook_events WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if attempts + 1 >= MAX_STALE_ATTEMPTS {
            set_webhook_event_state(
                conn,
                id,
                "failed",
                Some("processing lease expired repeatedly — giving up"),
            )?;
        } else {
            set_webhook_event_state(
                conn,
                id,
                "pending",
                Some("processing lease expired — requeued"),
            )?;
        }
        recovered += 1;
    }
    Ok(recovered)
}

pub fn prune_webhook_events(conn: &Connection, keep: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM webhook_events
          WHERE NOT (processing_state IN ('pending','failed') AND attempts < 5)
            AND id NOT IN (SELECT id FROM webhook_events ORDER BY id DESC LIMIT ?1)",
        params![keep.max(1)],
    )?;
    Ok(())
}

pub fn get_pending_webhook_events(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<WebhookEventRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_id, event_type, payload FROM webhook_events
          WHERE processing_state IN ('pending','failed') AND attempts < 5
          ORDER BY id LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(WebhookEventRecord {
                id: r.get(0)?,
                event_id: r.get(1)?,
                event_type: r.get(2)?,
                payload: r.get(3)?,
                processing_state: "pending".into(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn pending_event_count(conn: &Connection) -> Result<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM webhook_events WHERE processing_state = 'pending'",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64_of_hex(hex_str: &str) -> String {
        use base64::Engine;
        let bytes = hex::decode(hex_str).expect("valid hex");
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn signature_matches_hmac_sha1_test_vector() {
        let sig = compute_signature(b"key", b"The quick brown fox jumps over the lazy dog");
        assert_eq!(sig, "3nybhbi3iqa8ino29wqQcBydtNk=");
    }

    #[test]
    fn effective_secret_applies_the_platform_cap() {
        assert_eq!(
            effective_secret("short-secret"),
            "short-secret",
            "under the cap: unchanged"
        );
        let exactly_40: String = "x".repeat(40);
        assert_eq!(
            effective_secret(&exactly_40),
            exactly_40,
            "exactly at the cap: unchanged"
        );
        let longer: String = "y".repeat(60);
        assert_eq!(
            effective_secret(&longer),
            "y".repeat(40),
            "over the cap: the first 40 characters are what the platform holds"
        );
        let body = br#"{"objectID":1}"#;
        let sig = compute_signature(effective_secret(&longer).as_bytes(), body);
        assert!(verify_signature(effective_secret(&longer).as_bytes(), body, &sig).is_ok());
        assert!(verify_signature(longer.as_bytes(), body, &sig).is_err());
    }

    #[test]
    fn signature_matches_rfc2202_vectors() {
        let sig = compute_signature(&[0x0b; 20], b"Hi There");
        assert_eq!(sig, b64_of_hex("b617318655057264e28bc0b6fb378c8ef146be00"));

        let sig = compute_signature(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(sig, b64_of_hex("effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"));

        let long_key = [0xaa; 131];
        let data = b"Test Using Larger Than Block-Size Key - Hash Key First";
        let sig = compute_signature(&long_key, data);
        assert_eq!(sig, b64_of_hex("90d0dace1c1bdc957339307803160335bde6df2b"));
    }

    #[test]
    fn signature_is_deterministic_base64_hmac() {
        let sig = compute_signature(b"top-secret", b"{\"id\":1}");
        assert_eq!(sig.len(), 28);
        assert_eq!(sig, compute_signature(b"top-secret", b"{\"id\":1}"));
        assert_ne!(sig, compute_signature(b"other-secret", b"{\"id\":1}"));
    }

    #[test]
    fn signature_verification_accepts_valid_and_rejects_invalid() {
        let sig = compute_signature(b"top-secret", b"body");
        assert!(verify_signature(b"top-secret", b"body", &sig).is_ok());
        assert!(verify_signature(b"top-secret", b"tampered", &sig).is_err());
        assert!(verify_signature(b"wrong", b"body", &sig).is_err());
        assert!(verify_signature(b"top-secret", b"body", "not-hex").is_err());
    }

    fn event_db() -> Connection {
        let conn = Connection::open_in_memory().expect("db");
        apply_m048_webhook_events(&conn).expect("table");
        conn
    }

    fn insert_event(conn: &Connection, id: i64, state: &str, attempts: i64) {
        conn.execute(
            "INSERT INTO webhook_events (id, event_id, event_hash, event_type, received_at,
                                         payload, processing_state, attempts)
             VALUES (?1, ?2, ?3, 'convo.created', '2026-10-23T00:00:00.000Z', '{}', ?4, ?5)",
            rusqlite::params![
                id,
                format!("evt-{id}"),
                format!("hash-{id}"),
                state,
                attempts
            ],
        )
        .expect("insert event");
    }

    fn ids(conn: &Connection, state: &str) -> Vec<i64> {
        let mut stmt = conn
            .prepare("SELECT id FROM webhook_events WHERE processing_state = ?1 ORDER BY id")
            .expect("select");
        stmt.query_map(rusqlite::params![state], |r| r.get::<_, i64>(0))
            .expect("rows")
            .filter_map(|r| r.ok())
            .collect()
    }

    #[test]
    fn audit94_prune_keeps_unprocessed_events_under_backlog() {
        let conn = event_db();
        for id in 1..=3 {
            insert_event(&conn, id, "pending", 0);
        }
        for id in 4..=6 {
            insert_event(&conn, id, "processed", 1);
        }

        prune_webhook_events(&conn, 2).expect("prune");

        assert_eq!(
            ids(&conn, "pending"),
            vec![1, 2, 3],
            "pending events survive the prune at any age"
        );
        assert_eq!(
            ids(&conn, "processed"),
            vec![5, 6],
            "processed history is trimmed to the newest `keep`"
        );
    }

    #[test]
    fn audit94_prune_removes_failed_events_past_the_retry_budget() {
        let conn = event_db();
        insert_event(&conn, 1, "failed", 5);
        insert_event(&conn, 2, "failed", 2);
        insert_event(&conn, 3, "processed", 1);

        prune_webhook_events(&conn, 2).expect("prune");
        let remaining: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT id FROM webhook_events ORDER BY id")
                .expect("select");
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .expect("rows")
                .filter_map(|r| r.ok())
                .collect()
        };
        assert_eq!(
            remaining,
            vec![2, 3],
            "the terminal attempts=5 row is pruned; the retries-left row \
             survives outside the window, and the in-window history stays"
        );
        assert_eq!(
            ids(&conn, "failed"),
            vec![2],
            "only the retry-budget-spent failure went"
        );
    }
}
