use rusqlite::{Connection, params};

use crate::error::{Error, Result};

pub fn get_string(conn: &Connection, key: &str) -> Result<Option<String>> {
    let v: Option<String> = conn
        .prepare("SELECT value FROM application_settings WHERE key = ?1")?
        .query_row(params![key], |r| r.get::<_, String>(0))
        .ok();
    Ok(v)
}

pub fn set_string(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO application_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_bool(conn: &Connection, key: &str, default: bool) -> Result<bool> {
    Ok(match get_string(conn, key)? {
        None => default,
        Some(s) => match s.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(Error::Config(format!(
                    "settings key {key:?} = {other:?}; expected \"true\" or \"false\""
                )));
            }
        },
    })
}

pub fn set_bool(conn: &Connection, key: &str, value: bool) -> Result<()> {
    set_string(conn, key, if value { "true" } else { "false" })
}

pub fn get_int(conn: &Connection, key: &str, default: i64) -> Result<i64> {
    Ok(match get_string(conn, key)? {
        None => default,
        Some(s) => s.parse().unwrap_or(default),
    })
}

pub fn get_i64(conn: &Connection, key: &str, default: i64) -> Result<i64> {
    Ok(match get_string(conn, key)? {
        None => default,
        Some(s) => s.parse().map_err(|_| {
            Error::Config(format!("settings key {key:?} = {s:?}; expected an integer"))
        })?,
    })
}

pub fn set_i64(conn: &Connection, key: &str, value: i64) -> Result<()> {
    set_string(conn, key, &value.to_string())
}

pub fn get_json<T: serde::de::DeserializeOwned>(conn: &Connection, key: &str) -> Result<Option<T>> {
    match get_string(conn, key)? {
        None => Ok(None),
        Some(s) => {
            let v = serde_json::from_str(&s).map_err(|e| {
                Error::Config(format!("settings key {key:?} is not valid JSON: {e}"))
            })?;
            Ok(Some(v))
        }
    }
}

pub fn set_json<T: serde::Serialize>(conn: &Connection, key: &str, value: &T) -> Result<()> {
    let s = serde_json::to_string(value)
        .map_err(|e| Error::Config(format!("settings key {key:?} failed to serialize: {e}")))?;
    set_string(conn, key, &s)
}

pub fn get_secret(conn: &Connection, key: &str) -> Result<Option<Vec<u8>>> {
    let v: Option<Vec<u8>> = conn
        .prepare("SELECT value FROM secrets WHERE key = ?1")?
        .query_row(params![key], |r| r.get::<_, Vec<u8>>(0))
        .ok();
    Ok(v)
}

pub fn has_secret(conn: &Connection, key: &str) -> Result<bool> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM secrets WHERE key = ?1)",
        params![key],
        |r| r.get(0),
    )?;
    Ok(exists)
}

pub fn redacted_secret(conn: &Connection, key: &str) -> Result<String> {
    if has_secret(conn, key)? {
        Ok("••••".into())
    } else {
        Ok(String::new())
    }
}

pub fn delete_secret(conn: &Connection, key: &str) -> Result<bool> {
    let rows = conn.execute("DELETE FROM secrets WHERE key = ?1", params![key])?;
    Ok(rows > 0)
}

pub fn secret_or_error(conn: &Connection, key: &str) -> Result<Vec<u8>> {
    get_secret(conn, key)?.ok_or_else(|| Error::SecretNotFound { key: key.into() })
}

pub fn first_run_done(conn: &Connection) -> Result<bool> {
    let v: i64 = conn.query_row(
        "SELECT first_run_done FROM app_state WHERE id = 1",
        [],
        |r| r.get(0),
    )?;
    Ok(v != 0)
}

pub fn mark_first_run_done(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE app_state SET first_run_done = 1 WHERE id = 1", [])?;
    Ok(())
}
