use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

const FILE_NAME: &str = "daemon.json";

/// Connection details for clients. Holds the token, so it is written with
/// owner-only permissions and never logged.
#[derive(Serialize)]
pub struct DaemonFile {
    pub port: u16,
    pub token: String,
}

pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("cohors"))
}

/// Writes atomically: a reader never sees a partial file.
pub fn write(dir: &Path, contents: &DaemonFile) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    let temp_path = dir.join(format!("{FILE_NAME}.tmp"));
    let json = serde_json::to_vec(contents).map_err(io::Error::other)?;
    // A leftover temp file could have looser permissions, so never reuse it.
    match fs::remove_file(&temp_path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut file = owner_only_options().open(&temp_path)?;
    file.write_all(&json)?;
    file.sync_all()?;
    fs::rename(&temp_path, &path)?;
    Ok(path)
}

#[cfg(unix)]
fn owner_only_options() -> OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    options
}

#[cfg(not(unix))]
fn owner_only_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    options
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
            },
        )
        .expect("write");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read")).expect("json");
        assert_eq!(value, serde_json::json!({"port": 4321, "token": "abc"}));
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        let path = write(
            &dir,
            &DaemonFile {
                port: 1,
                token: "t".into(),
            },
        )
        .expect("write");
        let mode = fs::metadata(&path).expect("metadata").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
