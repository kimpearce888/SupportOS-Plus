use rusqlite::{Connection, params};

use crate::error::{Error, Result};

/// Clean-audit item 93: how long a single-use OAuth state stays valid.
/// The browser flow takes seconds; ten minutes covers a slow login.
/// After that the state is dead whether or not it was consumed — an
/// unconsumed state issued weeks ago must not remain redeemable, and a
/// consumed one is history, not data.
pub const STATE_TTL_SECONDS: i64 = 600;

pub fn ensure_oauth_states_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS oauth_states (
            state        TEXT PRIMARY KEY,
            created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            consumed_at  TEXT,
            redirect_uri TEXT,
            -- The Help Scout OAuth flow needs the redirect URI to be the same
            -- on authorize and callback; we store it here to verify (the
            -- loopback callback checks it against the listener that serves
            -- the request — see `handle_callback`).
            -- States expire: consumed ones after STATE_TTL_SECONDS, and
            -- unconsumed ones too — an abandoned flow's state must not
            -- stay redeemable forever (the sweeper runs on every new
            -- authorization and `consume_state` enforces the TTL itself).
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

/// Consume a single-use state. The state must exist, be unconsumed, AND
/// be fresh: `created_at` within [`STATE_TTL_SECONDS`] (clean-audit item
/// 93 — the pre-fix table kept unconsumed states valid forever, so a
/// state leaked or replayed weeks later still redeemed; the callback's
/// "it may have expired" message was a promise nothing enforced).
pub fn consume_state(conn: &Connection, state: &str) -> Result<()> {
    ensure_oauth_states_table(conn)?;
    let exists_fresh_unconsumed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM oauth_states
           WHERE state = ?1
             AND consumed_at IS NULL
             AND (julianday('now') - julianday(created_at)) * 86400.0 < ?2)",
        params![state, STATE_TTL_SECONDS as f64],
        |r| r.get(0),
    )?;
    if !exists_fresh_unconsumed {
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

/// Sweep dead states: consumed ones older than the TTL, and UNCONSUMED
/// ones older than the TTL too (an abandoned authorization flow's state
/// is dead — it must expire, not linger redeemable forever; clean-audit
/// item 93). Returns the number of rows removed.
pub fn sweep_states(conn: &Connection) -> Result<u32> {
    ensure_oauth_states_table(conn)?;
    let rows = conn.execute(
        "DELETE FROM oauth_states
          WHERE (julianday('now') - julianday(COALESCE(consumed_at, created_at))) >= (?1 / 86400.0)",
        params![STATE_TTL_SECONDS],
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
