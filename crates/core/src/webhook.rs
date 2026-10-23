use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use rusqlite::{Connection, OptionalExtension, params};
use sha1::Sha1;

use crate::error::{Error, Result};

pub const HELPSCOUT_SIGNATURE_HEADER: &str = "X-Helpscout-Signature";

type HmacSha1 = Hmac<Sha1>;

/// The platform's cap on webhook secrets (registration answers 400 for
/// anything longer — Help Scout's own rule).
pub const SECRET_CAP: usize = 40;

/// The secret Help Scout actually holds: the first 40 characters of the
/// configured one (the platform rejects longer values at registration).
///
/// Audit item 32: registration capped the secret at 40 characters while
/// signature verification used the full stored value — any configured
/// secret longer than the cap meant the platform signed with the
/// truncation while the app verified with the whole string, so every
/// delivery failed the check, ten discards deactivated the hook, and the
/// health check re-registered it into the same trap. Both sides now
/// resolve through this one function: registration and verification
/// always agree.
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
    // Mac::verify_slice compares the tag in constant time.
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

/// Numbered migration (boot step m048): creates the `webhook_events`
/// table and index, upgrading legacy tables that stored the raw body.
///
/// The schema is created here — at boot, inside its own migration — and is
/// deliberately NOT ensured per-insert: data-path functions assume a database
/// opened through `bootstrap::apply_all` and surface real errors instead of
/// silently rebuilding schema mid-write.
pub fn apply_m048_webhook_events(conn: &Connection) -> Result<()> {
    // Tables from the pre-migration era stored the raw body and no event hash.
    // They are dropped rather than migrated: their events were re-delivered by
    // Help Scout on retry anyway, and the hash column is now NOT NULL.
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

pub fn prune_webhook_events(conn: &Connection, keep: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM webhook_events WHERE id NOT IN (SELECT id FROM webhook_events ORDER BY id DESC LIMIT ?1)",
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

pub fn persist_event(conn: &Connection, event_id: &str, body: &str) -> Result<bool> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM webhook_events WHERE event_id = ?1",
            params![event_id],
            |r| r.get(0),
        )
        .optional()?;
    if existing.is_some() {
        return Ok(false);
    }
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("legacy:{event_id}:{body}"));
    let hash = hex::encode(hasher.finalize());
    conn.execute(
        "INSERT INTO webhook_events (event_id, event_hash, event_type, received_at, payload, processing_state)
         VALUES (?1, ?2, 'unknown', ?3, ?4, 'pending')",
        params![event_id, hash, now, body],
    )?;
    Ok(true)
}

pub fn mark_event_processed(conn: &Connection, event_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE webhook_events
            SET processing_state = 'processed',
                processing_error = NULL
          WHERE event_id = ?1",
        params![event_id],
    )?;
    Ok(())
}

pub fn pending_event_count(conn: &Connection) -> Result<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM webhook_events WHERE processing_state = 'pending'",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}

pub fn pending_event_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT event_id FROM webhook_events WHERE processing_state = 'pending' AND event_id IS NOT NULL ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
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
        // RFC 2202-style vector: HMAC-SHA1("key", "The quick brown fox jumps
        // over the lazy dog") = de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9
        let sig = compute_signature(b"key", b"The quick brown fox jumps over the lazy dog");
        assert_eq!(sig, "3nybhbi3iqa8ino29wqQcBydtNk=");
    }

    /// Clean-audit item 32: registration and verification must agree on
    /// the effective secret — the platform's 40-character cap applied to
    /// whatever the user stored.
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
        // The truncation and the registration path sign identically: the
        // signature computed over the capped secret verifies with the
        // capped secret, and NOT with the full one — which is exactly the
        // trap the pre-fix loopback fell into (register truncated,
        // verify full).
        let body = br#"{"objectID":1}"#;
        let sig = compute_signature(effective_secret(&longer).as_bytes(), body);
        assert!(verify_signature(effective_secret(&longer).as_bytes(), body, &sig).is_ok());
        assert!(verify_signature(longer.as_bytes(), body, &sig).is_err());
    }

    #[test]
    fn signature_matches_rfc2202_vectors() {
        // RFC 2202 test 1: key = 0x0b x 20, data = "Hi There"
        let sig = compute_signature(&[0x0b; 20], b"Hi There");
        assert_eq!(sig, b64_of_hex("b617318655057264e28bc0b6fb378c8ef146be00"));

        // RFC 2202 test 2: key = "Jefe"
        let sig = compute_signature(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(sig, b64_of_hex("effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"));

        // RFC 2202 test 6: key longer than the 64-byte block size (key is hashed first).
        // Digest cross-verified against Python stdlib hmac (hashlib.sha1).
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
}
