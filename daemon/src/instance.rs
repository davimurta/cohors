//! One daemon per user: the daemon holds an exclusive lock on a file for its
//! whole life. The OS releases it when the process ends, even on a crash.

use std::fs::{File, TryLockError};
use std::io;
use std::path::Path;

use crate::private_fs;

const LOCK_FILE: &str = "daemon.lock";

/// Takes the lock, or returns None when another daemon holds it.
pub fn acquire(data_dir: &Path) -> io::Result<Option<File>> {
    private_fs::create_dir(data_dir)?;
    let file = private_fs::open_append(&data_dir.join(LOCK_FILE))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

pub fn is_running(data_dir: &Path) -> io::Result<bool> {
    // The probe lock is released when the file is dropped.
    Ok(acquire(data_dir)?.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_lock_is_refused_until_the_first_is_released() {
        let dir = std::env::temp_dir().join(format!("cohors-lock-{}", uuid::Uuid::new_v4()));
        let first = acquire(&dir).expect("acquire").expect("lock");
        assert!(acquire(&dir).expect("acquire").is_none());
        assert!(is_running(&dir).expect("probe"));
        drop(first);
        assert!(!is_running(&dir).expect("probe"));
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
