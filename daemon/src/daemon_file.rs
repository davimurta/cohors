use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::private_fs;

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

pub fn write(dir: &Path, contents: &DaemonFile) -> io::Result<PathBuf> {
    private_fs::create_dir(dir)?;
    let path = dir.join(FILE_NAME);
    let json = serde_json::to_vec(contents).map_err(io::Error::other)?;
    private_fs::write_atomic(&path, &json)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
}
