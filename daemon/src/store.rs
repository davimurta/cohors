use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::hooks::{self, Signal};
use crate::private_fs;
use crate::protocol::SessionMeta;
use crate::transcript::{self, Transcript};

const META_FILE: &str = "meta.json";
const SUMMARY_FILE: &str = "summary.md";

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

    pub fn open_transcript(&self, id: &str) -> io::Result<Transcript> {
        Transcript::open(&self.root.join(id))
    }

    pub fn read_transcript(&self, id: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        transcript::read_tail(&self.root.join(id), max_bytes)
    }

    pub fn save_summary(&self, id: &str, text: &str) -> io::Result<()> {
        private_fs::write_atomic(&self.root.join(id).join(SUMMARY_FILE), text.as_bytes())
    }

    pub fn read_summary(&self, id: &str) -> io::Result<Option<String>> {
        match fs::read_to_string(self.root.join(id).join(SUMMARY_FILE)) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn transcript_modified(&self, id: &str) -> Option<SystemTime> {
        fs::metadata(self.root.join(id).join(transcript::FILE_NAME))
            .and_then(|metadata| metadata.modified())
            .ok()
    }

    /// Takes the latest hook signal of a session, if any arrived.
    pub fn take_signal(&self, id: &str) -> io::Result<Option<Signal>> {
        let path = self.root.join(id).join(hooks::ATTENTION_FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        fs::remove_file(&path)?;
        Ok(Signal::parse(&text))
    }

    pub fn remove(&self, id: &str) -> io::Result<()> {
        fs::remove_dir_all(self.root.join(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Agent, SessionStatus, SummaryStatus};

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
            idle_since: None,
            project_id: None,
            claude_config_dir: None,
            ssh_key: None,
            context_dir: None,
            attention: None,
            notes: String::new(),
            summary_status: SummaryStatus::None,
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
    fn loads_meta_saved_before_idle_tracking() {
        let (store, root) = temp_store();
        let dir = root.join("old");
        fs::create_dir_all(&dir).expect("dir");
        let json = r#"{"id":"old","name":"n","agent":"shell","cwd":"/tmp","agentSessionId":null,
            "status":"exited","exitCode":0,"createdAt":"2026-01-01T00:00:00Z","endedAt":null}"#;
        fs::write(dir.join(META_FILE), json).expect("write");
        assert_eq!(store.load_all()[0].idle_since, None);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn summary_round_trip() {
        let (store, root) = temp_store();
        store.save(&meta("a")).expect("save");
        assert_eq!(store.read_summary("a").expect("read"), None);
        store.save_summary("a", "## Feito").expect("save summary");
        assert_eq!(
            store.read_summary("a").expect("read").as_deref(),
            Some("## Feito")
        );
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
        let (store, root) = temp_store();
        store
            .open_transcript("a")
            .expect("open")
            .append(b"one")
            .expect("write");
        store
            .open_transcript("a")
            .expect("reopen")
            .append(b"two")
            .expect("write");
        assert_eq!(store.read_transcript("a", 100).expect("read"), b"onetwo");
        assert!(store.transcript_modified("a").is_some());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn hook_signals_are_taken_once() {
        let (store, root) = temp_store();
        let id = uuid::Uuid::new_v4().to_string();
        store.save(&meta(&id)).expect("save");
        assert_eq!(store.take_signal(&id).expect("none"), None);
        hooks::write_signal(&root, &id, Signal::Clear).expect("write");
        assert_eq!(store.take_signal(&id).expect("take"), Some(Signal::Clear));
        assert_eq!(store.take_signal(&id).expect("taken"), None);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
