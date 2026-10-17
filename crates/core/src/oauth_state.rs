use rusqlite::{params, Connection};

use crate::error::{Error, Result};

pub fn ensure_oauth_states_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS oauth_states (
            state        TEXT PRIMARY KEY,
            created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            consumed_at  TEXT,
            redirect_uri TEXT,
            -- The Help Scout OAuth flow needs the redirect URI to be the same
            -- on authorize and callback; we store it here to verify.
            -- States are deleted after consumption AND after a TTL (M2 will
            -- add the TTL sweeper; for M1 we only need single-use).
            scope        TEXT
        );",
    )?;
    Ok(())
}

pub fn issue_state(conn: &Connection, redirect_uri: &str, scope: Option<&str>) -> Result<String> {
    ensure_oauth_states_table(conn)?;
    let state = random_state();
    conn.execute(
        "INSERT INTO oauth_states (state, redirect_uri, scope) VALUES (?1, ?2, ?3)",
        params![state, redirect_uri, scope],
    )?;
    Ok(state)
}

pub fn consume_state(conn: &Connection, state: &str) -> Result<()> {
    ensure_oauth_states_table(conn)?;
    let exists_unconsumed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM oauth_states WHERE state = ?1 AND consumed_at IS NULL)",
        params![state],
        |r| r.get(0),
    )?;
    if !exists_unconsumed {
        return Err(Error::OauthStateInvalid);
    }
    conn.execute(
        "UPDATE oauth_states SET consumed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state = ?1",
        params![state],
    )?;
    Ok(())
}

pub fn redirect_uri_for(conn: &Connection, state: &str) -> Result<Option<String>> {
    ensure_oauth_states_table(conn)?;
    let v: Option<String> = conn
        .prepare("SELECT redirect_uri FROM oauth_states WHERE state = ?1")?
        .query_row(params![state], |r| r.get::<_, String>(0))
        .ok();
    Ok(v)
}

pub fn sweep_consumed(conn: &Connection, older_than_seconds: i64) -> Result<u32> {
    let rows = conn.execute(
        "DELETE FROM oauth_states
          WHERE consumed_at IS NOT NULL
            AND julianday('now') - julianday(consumed_at) >= (?1 / 86400.0)",
        params![older_than_seconds],
    )?;
    Ok(u32::try_from(rows).unwrap_or(0))
}

fn random_state() -> String {
    let bytes: [u8; 16] = rand::random();
    hex_lower(&bytes)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(hex_digit(b >> 4));
        out.push(hex_digit(b & 0x0f));
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
