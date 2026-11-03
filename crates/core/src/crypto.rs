use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

use crate::error::{Error, Result};

pub(crate) const NONCE_LEN: usize = 12;
pub(crate) const KEY_LEN: usize = 32;

pub(crate) fn write_key_file(path: &std::path::Path, key: &[u8; KEY_LEN]) -> Result<()> {
    if path.exists() {
        return Err(Error::Config(format!(
            "refusing to overwrite existing key file at {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(Error::Io)?;
        f.write_all(key).map_err(Error::Io)?;
        f.sync_all().map_err(Error::Io)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, key).map_err(Error::Io)?;
    }
    Ok(())
}

pub(crate) fn restrict_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

pub(crate) fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<([u8; NONCE_LEN], Vec<u8>)> {
    let mut nonce = [0u8; NONCE_LEN];
    rand::fill(&mut nonce[..]);
    let ciphertext = cipher(key)
        .encrypt(&Nonce::from(nonce), plaintext)
        .map_err(|_| Error::Config("AES-256-GCM encryption failed".into()))?;
    Ok((nonce, ciphertext))
}

pub(crate) fn decrypt(key: &[u8; KEY_LEN], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let nonce: [u8; NONCE_LEN] = nonce
        .try_into()
        .map_err(|_| Error::Config("nonce must be 12 bytes".into()))?;
    cipher(key)
        .decrypt(&Nonce::from(nonce), ciphertext)
        .map_err(|_| Error::Config("AES-256-GCM decryption failed".into()))
}

fn cipher(key: &[u8; KEY_LEN]) -> Aes256Gcm {
    Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key))
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encodes_lowercase() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa5]), "000fa5");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key: [u8; KEY_LEN] = [7u8; KEY_LEN];
        let (nonce, ciphertext) = encrypt(&key, b"local-first support data").unwrap();
        let plaintext = decrypt(&key, &nonce, &ciphertext).unwrap();
        assert_eq!(plaintext, b"local-first support data");
    }

    #[test]
    fn wrong_password_fails_to_decrypt() {
        let key: [u8; KEY_LEN] = [9u8; KEY_LEN];
        let (nonce, ciphertext) = encrypt(&key, b"secret").unwrap();
        let other_key: [u8; KEY_LEN] = [11u8; KEY_LEN];
        assert!(decrypt(&other_key, &nonce, &ciphertext).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_to_decrypt() {
        let key: [u8; KEY_LEN] = [1u8; KEY_LEN];
        let (nonce, mut ciphertext) = encrypt(&key, b"secret").unwrap();
        ciphertext[0] ^= 0xff;
        assert!(decrypt(&key, &nonce, &ciphertext).is_err());
    }

    #[test]
    fn short_nonce_is_rejected() {
        let key: [u8; KEY_LEN] = [1u8; KEY_LEN];
        assert!(decrypt(&key, &[0u8; 8], b"x").is_err());
    }
}
