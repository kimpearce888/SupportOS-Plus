use rusqlite::{params, Connection};
use subtle::ConstantTimeEq;

use crate::error::{Error, Result};

pub const HELPSCOUT_SIGNATURE_HEADER: &str = "X-Helpscout-Signature";

#[must_use]
pub fn compute_signature(secret: &[u8], body: &[u8]) -> String {
    use std::io::Write;
    let block_size = 64;
    let key: Vec<u8> = if secret.len() > block_size {
        sha1(secret).to_vec()
    } else {
        secret.to_vec()
    };
    let mut padded = key.clone();
    padded.resize(block_size, 0);
    let i_pad: Vec<u8> = padded.iter().map(|b| b ^ 0x36).collect();
    let o_pad: Vec<u8> = padded.iter().map(|b| b ^ 0x5c).collect();

    let mut inner = Vec::with_capacity(i_pad.len() + body.len());
    inner.write_all(&i_pad).ok();
    inner.write_all(body).ok();
    let inner_hash = sha1(&inner);

    let mut outer = Vec::with_capacity(o_pad.len() + inner_hash.len());
    outer.write_all(&o_pad).ok();
    outer.write_all(&inner_hash).ok();
    let outer_hash = sha1(&outer);

    base64_std(&outer_hash)
}

pub fn verify_signature(secret: &[u8], body: &[u8], provided_signature: &str) -> Result<()> {
    let expected = compute_signature(secret, body);
    if expected.len() != provided_signature.len() {
        return Err(Error::WebhookSignature);
    }
    let expected_bytes = expected.as_bytes();
    let provided_bytes = provided_signature.as_bytes();
    if expected_bytes.ct_eq(provided_bytes).into() {
        Ok(())
    } else {
        Err(Error::WebhookSignature)
    }
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];

    let bit_len: u64 = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks(64) {
        let mut w = [0u32; 80];
        #[allow(clippy::needless_range_loop)]
        for i in 0..16 {
            let j = i * 4;
            w[i] = u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        #[allow(clippy::needless_range_loop)]
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];

        #[allow(clippy::needless_range_loop)]
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(hex_digit(b >> 4));
        out.push(hex_digit(b & 0x0f));
    }
    out
}

fn base64_std(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(triple >> 6) as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[triple as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn hex_digit(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => unreachable!("hex_digit got {n} (must be 0..15)"),
    }
}

#[derive(Debug, Clone)]
pub struct WebhookEventRecord {
    pub id: i64,
    pub event_id: Option<String>,
    pub event_type: String,
    pub payload: String,
    pub processing_state: String,
}

pub fn ensure_webhook_events_table(conn: &Connection) -> Result<()> {
    let old_schema: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='webhook_events'",
            [],
            |r| r.get(0),
        )
        .ok();
    if let Some(schema) = old_schema {
        if schema.contains("body") && !schema.contains("event_hash") {
            conn.execute_batch(
                "ALTER TABLE webhook_events RENAME TO webhook_events_legacy;
                 DROP TABLE IF EXISTS webhook_events_legacy;",
            )?;
        }
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
    ensure_webhook_events_table(conn)?;
    let mut hasher = Sha256::new();
    hasher.update(format!("{event_type}:{payload}"));
    let hash = hex_lower(&hasher.finalize());

    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM webhook_events WHERE event_hash = ?1",
            params![hash],
            |r| r.get(0),
        )
        .ok();
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
    ensure_webhook_events_table(conn)?;
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
    ensure_webhook_events_table(conn)?;
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM webhook_events WHERE event_id = ?1",
            params![event_id],
            |r| r.get(0),
        )
        .ok();
    if existing.is_some() {
        return Ok(false);
    }
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("legacy:{event_id}:{body}"));
    let hash = hex_lower(&hasher.finalize());
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
