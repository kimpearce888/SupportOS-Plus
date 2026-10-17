use rusqlite::Connection;

use crate::catalog::ResponseState;
use crate::error::Result;

pub const RESPONSE_STATE_SQL: &str = "response_state";

pub fn filter_by_response_state(state: ResponseState) -> String {
    format!("response_state = '{}'", state.as_str())
}

pub fn count_by_response_state(conn: &Connection) -> Result<Vec<(String, u32)>> {
    let mut stmt = conn.prepare(
        "SELECT response_state, COUNT(*) as cnt
         FROM conversations
         GROUP BY response_state
         ORDER BY response_state",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn count_conversations_with_state(conn: &Connection, state: ResponseState) -> Result<u32> {
    let count: u32 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM conversations WHERE {}",
            filter_by_response_state(state)
        ),
        [],
        |r| r.get(0),
    )?;
    Ok(count)
}

pub fn count_conversations_with_state_in_mailbox(
    conn: &Connection,
    state: ResponseState,
    mailbox_id: Option<i64>,
) -> Result<u32> {
    let sql = match mailbox_id {
        Some(mid) => format!(
            "SELECT COUNT(*) FROM conversations WHERE response_state = '{}' AND mailbox_local_id = {}",
            state.as_str(),
            mid
        ),
        None => format!(
            "SELECT COUNT(*) FROM conversations WHERE response_state = '{}'",
            state.as_str()
        ),
    };
    let count: u32 = conn.query_row(&sql, [], |r| r.get(0))?;
    Ok(count)
}
