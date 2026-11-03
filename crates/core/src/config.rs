use std::path::{Path, PathBuf};

pub(crate) const DATA_DIR_NAME: &str = "supportos-plus";

pub const DB_FILE_NAME: &str = "supportos-plus.db";

const LEGACY_DATA_DIR_NAME: &str = "supportos-plusplus";

const LEGACY_DB_FILE_NAME: &str = "supportos-plusplus.db";

fn data_dir_root() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty());
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    base.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home).join(".local").join("share"))
}

pub fn default_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SPP_DATA_DIR") {
        return PathBuf::from(dir);
    }
    data_dir_root().join(DATA_DIR_NAME)
}

pub fn migrate_legacy_data_dir() {
    if std::env::var("SPP_DATA_DIR").is_ok() {
        return;
    }
    let root = data_dir_root();
    let new = root.join(DATA_DIR_NAME);
    let legacy = root.join(LEGACY_DATA_DIR_NAME);
    match migrate_legacy_dir(&root, &new) {
        Ok(0) => {}
        Ok(moved) => tracing::info!(
            from = %legacy.display(),
            to = %new.display(),
            moved,
            "migrated the pre-1.0.0 data directory"
        ),
        Err(e) => tracing::warn!(
            error = %e,
            "could not migrate the pre-1.0.0 data directory; continuing with a fresh one"
        ),
    }
}

fn migrate_legacy_dir(root: &Path, new: &Path) -> std::io::Result<usize> {
    let legacy = root.join(LEGACY_DATA_DIR_NAME);
    let legacy_db = legacy.join(LEGACY_DB_FILE_NAME);
    if !legacy_db.is_file() || new.join(DB_FILE_NAME).is_file() {
        return Ok(0);
    }

    if !new.exists() {
        std::fs::rename(&legacy, new)?;
        let moved_db = new.join(LEGACY_DB_FILE_NAME);
        if moved_db.is_file() {
            std::fs::rename(moved_db, new.join(DB_FILE_NAME))?;
        }
        return Ok(1);
    }

    let mut moved = 0usize;
    for entry in std::fs::read_dir(&legacy)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "instance.lock" {
            continue;
        }
        let to = if name == LEGACY_DB_FILE_NAME {
            new.join(DB_FILE_NAME)
        } else {
            new.join(&name)
        };
        if to.exists() {
            continue;
        }
        if std::fs::rename(entry.path(), &to).is_ok() {
            moved += 1;
        }
    }
    Ok(moved)
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        ),
        _ => default,
    }
}

fn env_int(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvConfig {
    pub log_level: String,
    pub helpscout: EnvHelpScout,
    pub lmstudio: EnvLmStudio,
    pub qdrant: EnvQdrant,
    pub sync: EnvSync,
}

#[derive(Clone, PartialEq)]
pub struct EnvHelpScout {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub api_base: String,
    pub webhook_secret: String,
    pub docs_api_key: String,
    pub docs_api_base: String,
}

impl core::fmt::Debug for EnvHelpScout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EnvHelpScout")
            .field("client_id", &self.client_id)
            .field("client_secret", &debug_redact(&self.client_secret))
            .field("redirect_uri", &self.redirect_uri)
            .field("api_base", &self.api_base)
            .field("webhook_secret", &debug_redact(&self.webhook_secret))
            .field("docs_api_key", &debug_redact(&self.docs_api_key))
            .field("docs_api_base", &self.docs_api_base)
            .finish()
    }
}

pub(crate) fn debug_redact(v: &str) -> String {
    if v.is_empty() {
        "<unset>".to_string()
    } else {
        format!("<redacted {} chars>", v.chars().count())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvLmStudio {
    pub base_url: String,
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub timeout_ms: i64,
    pub concurrency: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvQdrant {
    pub url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvSync {
    pub interval_minutes: i64,
    pub api_concurrency: i64,
}

pub fn load_env_config() -> EnvConfig {
    EnvConfig {
        log_level: env_str("LOG_LEVEL", "info"),
        helpscout: EnvHelpScout {
            client_id: env_str("HELPSCOUT_CLIENT_ID", ""),
            client_secret: env_str("HELPSCOUT_CLIENT_SECRET", ""),
            redirect_uri: env_str(
                "HELPSCOUT_REDIRECT_URI",
                "http://localhost:3000/oauth/callback",
            ),
            api_base: env_str("HELPSCOUT_API_BASE", "https://api.helpscout.net"),
            webhook_secret: env_str("HELPSCOUT_WEBHOOK_SECRET", ""),
            docs_api_key: env_str("HELPSCOUT_DOCS_API_KEY", ""),
            docs_api_base: env_str("HELPSCOUT_DOCS_API_BASE", "https://docsapi.helpscout.net"),
        },
        lmstudio: EnvLmStudio {
            base_url: env_str("LMSTUDIO_BASE_URL", "http://127.0.0.1:1234"),
            chat_model: std::env::var("LMSTUDIO_CHAT_MODEL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            embedding_model: std::env::var("LMSTUDIO_EMBEDDING_MODEL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            timeout_ms: env_int("LMSTUDIO_TIMEOUT_MS", 120_000),
            concurrency: env_int("LMSTUDIO_CONCURRENCY", 2),
        },
        qdrant: EnvQdrant {
            url: env_str("QDRANT_URL", "http://127.0.0.1:6333"),
            enabled: env_bool("QDRANT_ENABLED", true),
        },
        sync: EnvSync {
            interval_minutes: env_int("SYNC_INTERVAL_MINUTES", 1),
            api_concurrency: env_int("SYNC_API_CONCURRENCY", 2),
        },
    }
}

#[cfg(test)]
mod migrate_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(tag: &str) -> PathBuf {
        let unique = format!(
            "spp-migrate-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("temp root");
        dir
    }

    fn seed_legacy(root: &Path) -> PathBuf {
        let legacy = root.join(LEGACY_DATA_DIR_NAME);
        let vectors = legacy.join("vectors");
        std::fs::create_dir_all(&vectors).expect("legacy dirs");
        std::fs::write(legacy.join(LEGACY_DB_FILE_NAME), b"").expect("db");
        std::fs::write(legacy.join("secrets.key"), [0u8; 32]).expect("key");
        std::fs::write(vectors.join("vectors.db"), b"v").expect("vectors db");
        std::fs::write(legacy.join("instance.lock"), b"").expect("lock");
        legacy
    }

    #[test]
    fn whole_dir_rename_moves_everything_and_renames_the_db() {
        let root = temp_root("whole");
        seed_legacy(&root);
        let new = root.join(DATA_DIR_NAME);

        let moved = migrate_legacy_dir(&root, &new).expect("migration");
        assert_eq!(moved, 1);

        assert!(new.is_dir(), "whole directory moved");
        assert!(new.join(DB_FILE_NAME).is_file(), "db renamed");
        assert!(!new.join(LEGACY_DB_FILE_NAME).exists(), "old db name gone");
        assert!(new.join("secrets.key").is_file(), "key moved");
        assert!(
            new.join("vectors").join("vectors.db").is_file(),
            "vectors moved"
        );
        assert!(!root.join(LEGACY_DATA_DIR_NAME).exists(), "legacy gone");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn no_op_when_the_new_install_already_has_its_db() {
        let root = temp_root("noop-new");
        let legacy = seed_legacy(&root);
        let new = root.join(DATA_DIR_NAME);
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join(DB_FILE_NAME), b"fresh").unwrap();

        assert_eq!(migrate_legacy_dir(&root, &new).expect("migration"), 0);
        assert!(
            legacy.join(LEGACY_DB_FILE_NAME).is_file(),
            "legacy untouched"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn no_op_without_a_real_legacy_install() {
        let root = temp_root("noop-none");
        let new = root.join(DATA_DIR_NAME);
        assert_eq!(migrate_legacy_dir(&root, &new).expect("migration"), 0);
        let legacy = root.join(LEGACY_DATA_DIR_NAME);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("secrets.key"), [0u8; 32]).unwrap();
        assert_eq!(migrate_legacy_dir(&root, &new).expect("migration"), 0);
        assert!(legacy.join("secrets.key").is_file(), "left alone");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn itemized_fallback_when_the_new_dir_exists() {
        let root = temp_root("fallback");
        seed_legacy(&root);
        let new = root.join(DATA_DIR_NAME);
        std::fs::create_dir_all(&new).unwrap();

        let moved = migrate_legacy_dir(&root, &new).expect("migration");
        assert_eq!(moved, 3, "db + key + vectors moved");
        assert!(new.join(DB_FILE_NAME).is_file());
        assert!(new.join("secrets.key").is_file());
        assert!(new.join("vectors").join("vectors.db").is_file());
        assert!(!new.join("instance.lock").exists(), "stale lock not moved");
        assert!(!legacy_has_db(&root));
        std::fs::remove_dir_all(&root).ok();
    }

    fn legacy_has_db(root: &Path) -> bool {
        root.join(LEGACY_DATA_DIR_NAME)
            .join(LEGACY_DB_FILE_NAME)
            .is_file()
    }
}

#[cfg(test)]
mod debug_redaction_tests {
    use super::*;

    #[test]
    fn env_helpscout_debug_never_contains_secret_values() {
        let env = EnvHelpScout {
            client_id: "client-id-abc".into(),
            client_secret: "SUPERSECRETVALUE1".into(),
            redirect_uri: "http://localhost:3000/oauth/callback".into(),
            api_base: "https://api.helpscout.net".into(),
            webhook_secret: "WEBHOOKSECRET1".into(),
            docs_api_key: "DOCSAPIKEY1".into(),
            docs_api_base: "https://docsapi.helpscout.net".into(),
        };
        let rendered = format!("{env:?}");
        assert!(!rendered.contains("SUPERSECRETVALUE1"), "{rendered}");
        assert!(!rendered.contains("WEBHOOKSECRET1"), "{rendered}");
        assert!(!rendered.contains("DOCSAPIKEY1"), "{rendered}");
        assert!(rendered.contains("<redacted 17 chars>"), "{rendered}");
        assert!(rendered.contains("client-id-abc"), "{rendered}");
    }

    #[test]
    fn debug_redact_masks_set_values_and_admits_unset_ones() {
        assert_eq!(debug_redact(""), "<unset>");
        assert_eq!(debug_redact("hunter2"), "<redacted 7 chars>");
    }
}

#[cfg(test)]
mod batch30_tests {
    fn redact_string(s: &str) -> String {
        if s.is_empty() {
            return String::new();
        }
        let chars: Vec<char> = s.chars().collect();
        if chars.len() <= 4 {
            return "••••".into();
        }
        let head: String = chars[..2].iter().collect();
        let tail: String = chars[chars.len() - 2..].iter().collect();
        format!("{head}••••{tail}")
    }

    #[test]
    fn redact_string_survives_multibyte_secrets() {
        assert_eq!(redact_string(""), "");
        assert_eq!(redact_string("ab"), "••••");
        assert_eq!(redact_string("abcd"), "••••");
        assert_eq!(redact_string("🔒🔒🔒🔒🔒🔒"), "🔒🔒••••🔒🔒");
        let cjk = redact_string("秘密のキーです");
        assert!(cjk.starts_with("秘"), "{cjk}");
        assert!(cjk.ends_with("す"), "{cjk}");
        assert!(cjk.contains("••••"));
        assert_eq!(redact_string("sk-live-abcdef123456"), "sk••••56");
    }
}
