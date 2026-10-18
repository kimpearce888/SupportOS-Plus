use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::crypto::{self, KEY_LEN, NONCE_LEN};
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Encrypted secrets envelope (migration-map row 13)
//
// `secrets.value` stores `0x01 || nonce(12) || AES-256-GCM ciphertext` sealed
// under a per-installation master key (`<data_dir>/secrets.key`, created with
// 0600 permissions on first boot). Plaintext never enters the table. This
// replaces the previous fiction of a "secrets" table that nothing ever wrote
// and that the schema comment claimed was encrypted.
// ---------------------------------------------------------------------------

/// Master-key holder. Set once at boot; the RwLock (rather than a OnceLock)
/// keeps the envelope testable in-process.
static MASTER_KEY: std::sync::RwLock<Option<[u8; KEY_LEN]>> = std::sync::RwLock::new(None);

/// Loads the master key from `path`, creating a fresh random key (mode 0600)
/// if the file does not exist yet. Idempotent: if a key is already loaded,
/// this is a no-op.
pub fn init_master_key(path: &Path) -> Result<()> {
    if MASTER_KEY.read().expect("master key lock").is_some() {
        return Ok(());
    }
    let key = match std::fs::read(path) {
        Ok(bytes) if bytes.len() == KEY_LEN => bytes.try_into().expect("length checked"),
        Ok(_) => {
            return Err(Error::Config(format!(
                "secrets master key at {} has the wrong length; refusing to overwrite it",
                path.display()
            )));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut key = [0u8; KEY_LEN];
            rand::fill(&mut key);
            crypto::write_key_file(path, &key)?;
            key
        }
        Err(e) => return Err(Error::Io(e)),
    };
    let mut slot = MASTER_KEY.write().expect("master key lock");
    slot.get_or_insert(key);
    Ok(())
}

/// Installs an in-memory master key (tests and embedded tooling only).
#[cfg(test)]
pub(crate) fn set_master_key_for_tests(key: [u8; KEY_LEN]) {
    *MASTER_KEY.write().expect("master key lock") = Some(key);
}

/// Drops the loaded master key (tests only).
#[cfg(test)]
pub(crate) fn clear_master_key_for_tests() {
    *MASTER_KEY.write().expect("master key lock") = None;
}

fn master_key() -> Result<[u8; KEY_LEN]> {
    MASTER_KEY.read().expect("master key lock").ok_or_else(|| {
        Error::Config(
            "secrets master key is not initialized; call settings::init_master_key at boot".into(),
        )
    })
}

const ENVELOPE_VERSION: u8 = 0x01;

/// Stores `plaintext` in the `secrets` table, encrypted under the master key.
/// The plaintext never enters the database.
pub fn set_secret(conn: &Connection, key: &str, plaintext: &[u8]) -> Result<()> {
    let mk = master_key()?;
    let (nonce, ciphertext) = crypto::encrypt(&mk, plaintext)?;
    let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    blob.push(ENVELOPE_VERSION);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);
    conn.execute(
        "INSERT INTO secrets (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, blob],
    )?;
    Ok(())
}

/// Reads and decrypts the secret stored under `key`.
///
/// Returns `Ok(None)` when no row exists. A corrupt envelope, an unsupported
/// version, or a ciphertext that fails GCM authentication (e.g. written under
/// a different installation's master key, such as an imported sync bundle)
/// surfaces as an error rather than being silently treated as "missing".
pub fn get_secret(conn: &Connection, key: &str) -> Result<Option<Vec<u8>>> {
    let blob = conn
        .prepare("SELECT value FROM secrets WHERE key = ?1")?
        .query_row(params![key], |r| r.get::<_, Vec<u8>>(0))
        .optional()?;
    let Some(blob) = blob else { return Ok(None) };
    if blob.len() < 1 + NONCE_LEN || blob[0] != ENVELOPE_VERSION {
        return Err(Error::Config(format!(
            "secret {key:?} has a corrupt or unsupported envelope"
        )));
    }
    let nonce = &blob[1..1 + NONCE_LEN];
    let plaintext = crypto::decrypt(&master_key()?, nonce, &blob[1 + NONCE_LEN..])
        .map_err(|e| Error::Config(format!("secret {key:?} could not be decrypted: {e}")))?;
    Ok(Some(plaintext))
}

/// Numbered migration (boot step m049): moves the legacy plaintext
/// `webhook_secret` from `application_settings` into the encrypted envelope.
/// Idempotent; skipped with a warning when the master key is unavailable (the
/// next boot with a key completes it).
pub fn apply_m049_secret_envelope(conn: &Connection) -> Result<()> {
    if master_key().is_err() {
        tracing::warn!(
            "secrets master key not initialized; leaving plaintext webhook_secret in place for now"
        );
        return Ok(());
    }
    if let Some(legacy) = get_string(conn, "webhook_secret")? {
        if !legacy.is_empty() {
            set_secret(conn, "webhook_secret", legacy.as_bytes())?;
            tracing::info!("migrated plaintext webhook_secret into the encrypted envelope");
        }
        conn.execute(
            "DELETE FROM application_settings WHERE key = 'webhook_secret'",
            [],
        )?;
    }
    Ok(())
}

pub fn get_string(conn: &Connection, key: &str) -> Result<Option<String>> {
    let v: Option<String> = conn
        .prepare("SELECT value FROM application_settings WHERE key = ?1")?
        .query_row(params![key], |r| r.get::<_, String>(0))
        .optional()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory connection");
        crate::db::ensure_migrations_table(&conn).expect("migrations table");
        crate::migrations::run_all(&mut conn).expect("migrations");
        conn
    }

    fn temp_key_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "spp-settings-test-{tag}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ))
    }

    // The master-key holder is process-global and cargo runs tests in
    // parallel threads, so every behavior that touches it lives in this one
    // function — the sequence also mirrors a real installation's lifecycle.
    #[test]
    fn secrets_envelope_lifecycle() {
        let conn = migrated_conn();

        // 1. No master key: writes fail loudly, never silently store plaintext.
        clear_master_key_for_tests();
        assert!(set_secret(&conn, "webhook_secret", b"x").is_err());
        set_string(&conn, "webhook_secret", "legacy-plaintext").expect("seed plaintext");
        apply_m049_secret_envelope(&conn).expect("m049 skip");
        // migration was skipped; the plaintext row is untouched
        assert_eq!(
            get_string(&conn, "webhook_secret").expect("read"),
            Some("legacy-plaintext".into())
        );

        // 2. First boot: key file is created with 0600 permissions.
        let key_path = temp_key_path("lifecycle");
        let _ = std::fs::remove_file(&key_path);
        init_master_key(&key_path).expect("init creates key");
        assert_eq!(std::fs::read(&key_path).expect("key file").len(), KEY_LEN);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&key_path)
                .expect("key metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_file(&key_path);

        // 3. Envelope round-trip; plaintext never enters the table.
        set_secret(&conn, "webhook_secret", b"hs-secret-1").expect("set");
        assert_eq!(
            get_secret(&conn, "webhook_secret").expect("get").as_deref(),
            Some(b"hs-secret-1".as_slice())
        );
        let blob: Vec<u8> = conn
            .query_row(
                "SELECT value FROM secrets WHERE key = 'webhook_secret'",
                [],
                |r| r.get(0),
            )
            .expect("blob");
        assert!(blob.len() > 1 + NONCE_LEN);
        assert!(!blob.windows(10).any(|w| w == b"hs-secret-1"));

        // 4. Missing keys read as None; presence/redaction/delete still work.
        assert!(get_secret(&conn, "nope").expect("get").is_none());
        assert!(has_secret(&conn, "webhook_secret").expect("has"));
        assert_eq!(
            redacted_secret(&conn, "webhook_secret").expect("redacted"),
            "••••"
        );
        assert!(delete_secret(&conn, "webhook_secret").expect("delete"));
        assert!(!has_secret(&conn, "webhook_secret").expect("has"));
        assert!(get_secret(&conn, "webhook_secret").expect("get").is_none());

        // 5. Wrong installation key (e.g. an imported sync bundle): the
        //    envelope fails to authenticate instead of pretending to be empty.
        set_master_key_for_tests([9u8; KEY_LEN]);
        set_secret(&conn, "imported", b"other-installation").expect("set under other key");
        set_master_key_for_tests([7u8; KEY_LEN]);
        assert!(get_secret(&conn, "imported").is_err());
        assert!(delete_secret(&conn, "imported").expect("cleanup"));

        // 6. A corrupt envelope surfaces as an error, not as "missing".
        conn.execute(
            "INSERT INTO secrets (key, value) VALUES ('corrupt', x'00ff')",
            [],
        )
        .expect("seed corrupt blob");
        assert!(get_secret(&conn, "corrupt").is_err());

        // 7. m049: legacy plaintext moves into the envelope and disappears
        //    from application_settings; the migration is idempotent.
        apply_m049_secret_envelope(&conn).expect("m049");
        assert_eq!(get_string(&conn, "webhook_secret").expect("read"), None);
        assert_eq!(
            get_secret(&conn, "webhook_secret").expect("get").as_deref(),
            Some(b"legacy-plaintext".as_slice())
        );
        apply_m049_secret_envelope(&conn).expect("m049 again");
        assert_eq!(
            get_secret(&conn, "webhook_secret").expect("get").as_deref(),
            Some(b"legacy-plaintext".as_slice())
        );

        // 8. Fresh boot after losing the key file: a new key is created (old
        //    ciphertexts become undecryptable by design) and new secrets work.
        clear_master_key_for_tests();
        init_master_key(&key_path).expect("init creates a new key");
        let _ = std::fs::remove_file(&key_path);
        set_secret(&conn, "reload-check", b"still-readable").expect("set");
        assert_eq!(
            get_secret(&conn, "reload-check").expect("get").as_deref(),
            Some(b"still-readable".as_slice())
        );

        // 9. A malformed key file is refused, not overwritten.
        let bad_path = temp_key_path("bad");
        std::fs::write(&bad_path, [0u8; 5]).expect("bad key file");
        clear_master_key_for_tests();
        assert!(init_master_key(&bad_path).is_err());
        assert_eq!(std::fs::read(&bad_path).expect("untouched").len(), 5);
        let _ = std::fs::remove_file(&bad_path);
    }

    #[test]
    fn secret_or_error_reports_missing_key_name() {
        // A row that does not exist never reaches decryption, so this test
        // must not touch the process-global master key (the lifecycle test
        // above owns it).
        let conn = migrated_conn();
        let err = secret_or_error(&conn, "oauth_token").expect_err("missing secret");
        assert!(err.to_string().contains("oauth_token"));
    }
}
