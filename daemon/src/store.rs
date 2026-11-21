use std::fs::{self, File};
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::private_fs;
use crate::protocol::SessionMeta;

const META_FILE: &str = "meta.json";
const TRANSCRIPT_FILE: &str = "transcript.log";

/// Session folders on disk: `<root>/<id>/meta.json` and `transcript.log`.
pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn save(&self, meta: &SessionMeta) -> io::Result<()> {
        let dir = self.root.join(&meta.id);
        private_fs::create_dir(&dir)?;
        let json = serde_json::to_vec_pretty(meta).map_err(io::Error::other)?;
        private_fs::write_atomic(&dir.join(META_FILE), &json)
    }

    /// Every readable session. Broken folders are reported and skipped.
    pub fn load_all(&self) -> Vec<SessionMeta> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => {
                eprintln!("failed to read sessions folder: {error}");
                return Vec::new();
            }
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path().join(META_FILE);
                let meta = fs::read(&path)
                    .map_err(io::Error::other)
                    .and_then(|bytes| serde_json::from_slice(&bytes).map_err(io::Error::other));
                meta.map_err(|error| eprintln!("skipping {}: {error}", path.display()))
                    .ok()
            })
            .collect()
    }

    pub fn open_transcript(&self, id: &str) -> io::Result<File> {
        let dir = self.root.join(id);
        private_fs::create_dir(&dir)?;
        private_fs::open_append(&dir.join(TRANSCRIPT_FILE))
    }

    pub fn transcript_modified(&self, id: &str) -> Option<SystemTime> {
        fs::metadata(self.root.join(id).join(TRANSCRIPT_FILE))
            .and_then(|metadata| metadata.modified())
            .ok()
    }

    pub fn remove(&self, id: &str) -> io::Result<()> {
        fs::remove_dir_all(self.root.join(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Agent, SessionStatus};

    fn temp_store() -> (SessionStore, PathBuf) {
        let root = std::env::temp_dir().join(format!("cohors-store-{}", uuid::Uuid::new_v4()));
        (SessionStore::new(root.clone()), root)
    }

    fn meta(id: &str) -> SessionMeta {
        SessionMeta {
            id: id.into(),
            name: "cohors".into(),
            agent: Agent::Claude,
            cwd: "/tmp".into(),
            agent_session_id: Some("agent".into()),
            status: SessionStatus::Running,
            exit_code: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            ended_at: None,
        }
    }

    #[test]
    fn saved_sessions_load_back() {
        let (store, root) = temp_store();
        store.save(&meta("a")).expect("save a");
        store.save(&meta("b")).expect("save b");
        let mut ids: Vec<String> = store.load_all().into_iter().map(|meta| meta.id).collect();
        ids.sort();
        assert_eq!(ids, ["a", "b"]);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn missing_root_loads_nothing() {
        let (store, _) = temp_store();
        assert!(store.load_all().is_empty());
    }

    #[test]
    fn skips_corrupt_meta() {
        let (store, root) = temp_store();
        store.save(&meta("good")).expect("save");
        fs::create_dir_all(root.join("bad")).expect("dir");
        fs::write(root.join("bad").join(META_FILE), "{not json").expect("write");
        let loaded = store.load_all();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "good");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn transcript_appends() {
        use std::io::Write;
        let (store, root) = temp_store();
        store
            .open_transcript("a")
            .expect("open")
            .write_all(b"one")
            .expect("write");
        store
            .open_transcript("a")
            .expect("reopen")
            .write_all(b"two")
            .expect("write");
        let transcript = fs::read_to_string(root.join("a").join(TRANSCRIPT_FILE)).expect("read");
        assert_eq!(transcript, "onetwo");
        assert!(store.transcript_modified("a").is_some());
        fs::remove_dir_all(root).expect("cleanup");
    }
}
