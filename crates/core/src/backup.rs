use std::path::Path;

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use rand::RngCore;
use scrypt::scrypt;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::vectorstore::{CollectionSnapshot, VectorStore};

pub const SOSYNC_MAGIC: &[u8; 7] = b"SOSYNC1";

pub const SOSYNC_VERSION: u8 = 1;

const SALT_LEN: usize = 16;

const NONCE_LEN: usize = 12;

const SCRYPT_N: u32 = 1 << 17;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;
const SCRYPT_KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SosyncBackup {
    pub version: u8,
    pub collections: Vec<CollectionSnapshot>,
    pub schema_version: u32,
    pub created_at: String,
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; SCRYPT_KEY_LEN]> {
    let mut key = [0u8; SCRYPT_KEY_LEN];
    scrypt(
        password.as_bytes(),
        salt,
        &scrypt::Params::new(
            SCRYPT_N.trailing_zeros() as u8,
            SCRYPT_R,
            SCRYPT_P,
            SCRYPT_KEY_LEN,
        )
        .map_err(|e| Error::Config(format!("scrypt params invalid: {e}")))?,
        &mut key,
    )
    .map_err(|e| Error::Config(format!("scrypt derivation failed: {e}")))?;
    Ok(key)
}

pub fn backup(
    path: &Path,
    password: &str,
    vectorstore: &dyn VectorStore,
    collection_names: &[String],
    schema_version: u32,
) -> Result<()> {
    let mut collections = Vec::with_capacity(collection_names.len());
    for name in collection_names {
        let bytes = vectorstore.snapshot(name)?;
        let snapshot: CollectionSnapshot = serde_json::from_slice(&bytes).map_err(|e| {
            Error::Config(format!("snapshot deserialization failed for {name}: {e}"))
        })?;
        collections.push(snapshot);
    }

    let backup = SosyncBackup {
        version: SOSYNC_VERSION,
        collections,
        schema_version,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let plaintext = serde_json::to_vec(&backup)
        .map_err(|e| Error::Config(format!("backup serialization failed: {e}")))?;

    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| Error::Config(format!("AES key init failed: {e}")))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| Error::Config(format!("AES encryption failed: {e}")))?;

    let mut file_content =
        Vec::with_capacity(SOSYNC_MAGIC.len() + 1 + SALT_LEN + NONCE_LEN + ciphertext.len());
    file_content.extend_from_slice(SOSYNC_MAGIC);
    file_content.push(SOSYNC_VERSION);
    file_content.extend_from_slice(&salt);
    file_content.extend_from_slice(&nonce_bytes);
    file_content.extend_from_slice(&ciphertext);

    let temp_path = path.with_extension("sosync.tmp");
    std::fs::write(&temp_path, &file_content)?;
    std::fs::rename(&temp_path, path)?;

    Ok(())
}

pub fn restore(path: &Path, password: &str, vectorstore: &dyn VectorStore) -> Result<SosyncBackup> {
    let file_content = std::fs::read(path)?;

    if file_content.len() < SOSYNC_MAGIC.len() + 1 + SALT_LEN + NONCE_LEN {
        return Err(Error::Config(format!(
            "file too short for .sosync header (got {} bytes)",
            file_content.len()
        )));
    }
    if &file_content[..SOSYNC_MAGIC.len()] != SOSYNC_MAGIC {
        return Err(Error::Config(
            "invalid magic header — not a .sosync file".into(),
        ));
    }

    let version = file_content[SOSYNC_MAGIC.len()];
    if version != SOSYNC_VERSION {
        return Err(Error::Config(format!(
            "unsupported .sosync version {version} (expected {SOSYNC_VERSION})"
        )));
    }

    let offset = SOSYNC_MAGIC.len() + 1;
    let salt = &file_content[offset..offset + SALT_LEN];
    let nonce_offset = offset + SALT_LEN;
    let nonce_bytes = &file_content[nonce_offset..nonce_offset + NONCE_LEN];
    let ciphertext = &file_content[nonce_offset + NONCE_LEN..];

    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| Error::Config(format!("AES key init failed: {e}")))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| Error::Config("decryption failed — wrong password or corrupt file".into()))?;

    let backup: SosyncBackup = serde_json::from_slice(&plaintext)
        .map_err(|e| Error::Config(format!("backup deserialization failed: {e}")))?;

    for collection in &backup.collections {
        let bytes = serde_json::to_vec(collection)
            .map_err(|e| Error::Config(format!("collection serialization failed: {e}")))?;
        vectorstore.restore(&bytes)?;
    }

    Ok(backup)
}
