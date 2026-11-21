use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::private_fs;

const FILE_NAME: &str = "daemon.json";

/// Connection details for clients, plus the pid so the daemon can be
/// stopped. Holds the token, so it is owner-only and never logged.
#[derive(Deserialize, Serialize)]
pub struct DaemonFile {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("cohors"))
}

pub fn write(dir: &Path, contents: &DaemonFile) -> io::Result<PathBuf> {
    private_fs::create_dir(dir)?;
    let path = dir.join(FILE_NAME);
    let json = serde_json::to_vec(contents).map_err(io::Error::other)?;
    private_fs::write_atomic(&path, &json)?;
    Ok(path)
}

pub fn read(dir: &Path) -> Option<DaemonFile> {
    let bytes = fs::read(dir.join(FILE_NAME)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn remove(dir: &Path) -> io::Result<()> {
    match fs::remove_file(dir.join(FILE_NAME)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("cohors-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn writes_port_and_token() {
        let dir = temp_dir();
        let path = write(
            &dir,
            &DaemonFile {
                port: 4321,
                token: "abc".into(),
                pid: 99,
            },
        )
        .expect("write");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read")).expect("json");
        assert_eq!(
            value,
            serde_json::json!({"port": 4321, "token": "abc", "pid": 99})
        );
        assert_eq!(read(&dir).map(|file| file.pid), Some(99));
        remove(&dir).expect("remove");
        assert!(read(&dir).is_none());
        remove(&dir).expect("removing twice is fine");
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
