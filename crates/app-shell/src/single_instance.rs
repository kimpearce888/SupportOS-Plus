use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

pub const LOCK_FILE_NAME: &str = "instance.lock";

#[derive(Debug)]
pub enum AcquireError {
    AlreadyRunning,
    Io(std::io::Error),
}

impl std::fmt::Display for AcquireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => {
                write!(f, "another SupportOS+ instance is already running")
            }
            Self::Io(e) => write!(f, "failed to acquire the instance lock: {e}"),
        }
    }
}

impl std::error::Error for AcquireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyRunning => None,
            Self::Io(e) => Some(e),
        }
    }
}

pub struct InstanceLock {
    file: File,
    path: PathBuf,
}

impl InstanceLock {
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

pub fn acquire(data_dir: &Path) -> Result<InstanceLock, AcquireError> {
    std::fs::create_dir_all(data_dir).map_err(AcquireError::Io)?;
    let path = data_dir.join(LOCK_FILE_NAME);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(AcquireError::Io)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        return match err.kind() {
            ErrorKind::WouldBlock => Err(AcquireError::AlreadyRunning),
            _ => Err(AcquireError::Io(err)),
        };
    }
    Ok(InstanceLock { file, path })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("spp-shell-lock-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn second_lock_on_same_file_is_rejected() {
        let dir = temp_dir("second");
        let first = acquire(&dir).expect("first acquire");
        match acquire(&dir) {
            Err(AcquireError::AlreadyRunning) => {}
            Err(e) => panic!("expected AlreadyRunning, got: {e}"),
            Ok(_) => panic!("expected AlreadyRunning, got Ok"),
        }
        drop(first);
        let again = acquire(&dir).expect("acquire after release");
        drop(again);
        cleanup(&dir);
    }

    #[test]
    fn lock_file_is_created_with_expected_name() {
        let dir = temp_dir("created");
        let lock = acquire(&dir).expect("acquire");
        let expected = dir.join(LOCK_FILE_NAME);
        assert!(expected.is_file(), "lock file must exist");
        assert_eq!(lock.path(), expected);
        drop(lock);
        cleanup(&dir);
    }
}
