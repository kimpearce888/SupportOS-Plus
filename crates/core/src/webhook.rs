use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use rusqlite::{Connection, OptionalExtension, params};
use sha1::Sha1;

use crate::error::{Error, Result};

type HmacSha1 = Hmac<Sha1>;

/// UX-26B: Help Scout imposes no secret-length cap (the UX audit live-probed
/// the platform: it stores 48+ characters), so verification uses the FULL
/// stored secret. `LEGACY_SECRET_CAP` records the cap that pre-UX-26B builds
/// applied both at verification and when (re-)registering the webhook; hooks
/// registered by those builds have the first 40 characters as their
/// platform-side secret, and `verify_delivery` still accepts signatures over
/// that prefix. The prefix is derived from the full secret and was the ONLY
/// accepted key before the fix, so the fallback is never weaker than the
/// pre-fix state.
pub const LEGACY_SECRET_CAP: usize = 40;

fn legacy_capped(secret: &str) -> String {
    secret.chars().take(LEGACY_SECRET_CAP).collect()
}

/// Verify a Help Scout webhook delivery against the stored secret: the full
/// secret first, then — when the stored secret is longer than the legacy
/// cap — the capped prefix (what cap-era builds registered on the platform).
pub(crate) fn verify_delivery(secret: &[u8], body: &[u8], provided_signature: &str) -> Result<()> {
    if verify_signature(secret, body, provided_signature).is_ok() {
        return Ok(());
    }
    let secret_str = String::from_utf8_lossy(secret);
    if secret_str.chars().count() > LEGACY_SECRET_CAP {
        let capped = legacy_capped(&secret_str);
        return verify_signature(capped.as_bytes(), body, provided_signature);
    }
    Err(Error::WebhookSignature)
}

pub(crate) fn verify_signature(secret: &[u8], body: &[u8], provided_signature: &str) -> Result<()> {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    let provided = base64::engine::general_purpose::STANDARD
        .decode(provided_signature)
        .map_err(|_| Error::WebhookSignature)?;
    mac.verify_slice(&provided)
        .map_err(|_| Error::WebhookSignature)
}

#[derive(Debug, Clone)]
pub(crate) struct WebhookEventRecord {
    pub id: i64,
    pub event_type: String,
    pub payload: String,
}

pub(crate) fn apply_m048_webhook_events(conn: &Connection) -> Result<()> {
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

pub(crate) fn insert_webhook_event(
    conn: &Connection,
    event_type: &str,
    payload: &str,
    event_id: Option<&str>,
) -> Result<(i64, bool)> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("{event_type}:{payload}"));
    let hash = hex::encode(hasher.finalize());

    let existing: Option<(i64, String, i64)> = conn
        .query_row(
            "SELECT id, processing_state, attempts FROM webhook_events WHERE event_hash = ?1",
            params![hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((id, state, attempts)) = existing {
        // AU2-14: a Help Scout redelivery of an event whose retry budget is
        // exhausted must not be swallowed as a duplicate — it is the
        // platform's explicit "try again". Reset the budget and requeue it.
        // Genuinely processed / in-flight / still-retryable events stay
        // deduplicated.
        if state == "failed" && attempts >= 5 {
            conn.execute(
                "UPDATE webhook_events SET processing_state = 'pending',
                     processing_error = NULL, attempts = 0, received_at = ?2
                   WHERE id = ?1",
                params![
                    id,
                    chrono::Utc::now()
                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                        .to_string()
                ],
            )?;
            return Ok((id, false));
        }
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

pub(crate) fn set_webhook_event_state(
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

pub(crate) fn recover_stale_webhook_events(conn: &Connection) -> Result<usize> {
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

pub(crate) fn prune_webhook_events(conn: &Connection, keep: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM webhook_events
          WHERE NOT (processing_state IN ('pending','failed') AND attempts < 5)
            AND id NOT IN (SELECT id FROM webhook_events ORDER BY id DESC LIMIT ?1)",
        params![keep.max(1)],
    )?;
    Ok(())
}

pub(crate) fn get_pending_webhook_events(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<WebhookEventRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_type, payload FROM webhook_events
          WHERE processing_state IN ('pending','failed') AND attempts < 5
          ORDER BY id LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(WebhookEventRecord {
                id: r.get(0)?,
                event_type: r.get(1)?,
                payload: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
pub(crate) fn compute_signature(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

#[cfg(test)]
pub(crate) fn pending_event_count(conn: &Connection) -> crate::error::Result<u32> {
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
    fn full_length_secret_verifies_end_to_end() {
        // UX-26B: the audit's live probe showed Help Scout storing 48+
        // characters while the app truncated to 40 — those deliveries
        // 401'd. The full stored secret must now verify as-is.
        for len in [41usize, 48, 60, 120] {
            let secret = "s".repeat(len);
            let body = br#"{"objectID":1}"#;
            let sig = compute_signature(secret.as_bytes(), body);
            assert!(
                verify_delivery(secret.as_bytes(), body, &sig).is_ok(),
                "a {len}-char secret must verify with its full value"
            );
        }
    }

    #[test]
    fn legacy_capped_hook_signature_still_accepted() {
        // Hooks registered by cap-era builds sign with the first 40
        // characters; those deployments must not regress on the fix.
        let long: String = "y".repeat(60);
        let body = br#"{"objectID":1}"#;
        let legacy_sig = compute_signature(legacy_capped(&long).as_bytes(), body);
        assert!(
            verify_delivery(long.as_bytes(), body, &legacy_sig).is_ok(),
            "a signature over the capped prefix is the pre-fix contract — accepted"
        );
        // Exactly-at-the-cap secrets take the full-verification branch.
        let exactly_40: String = "x".repeat(40);
        let sig40 = compute_signature(exactly_40.as_bytes(), body);
        assert!(verify_delivery(exactly_40.as_bytes(), body, &sig40).is_ok());
    }

    #[test]
    fn unrelated_signature_is_still_rejected() {
        let secret = "y".repeat(60);
        let body = br#"{"objectID":1}"#;
        let wrong_sig = compute_signature(b"a-different-secret", body);
        assert!(
            verify_delivery(secret.as_bytes(), body, &wrong_sig).is_err(),
            "no fallback accepts a signature from an unrelated secret"
        );
        assert!(
            verify_delivery(secret.as_bytes(), body, "not-base64").is_err(),
            "undecodable signatures stay rejected"
        );
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
