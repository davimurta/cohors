use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::executable;
use crate::protocol::{Agent, ErrorCode, Project, ServerMessage, SessionMeta, SessionStatus};
use crate::session::{Launch, Session, SessionError};
use crate::store::SessionStore;

const CLAUDE_COMMAND: &str = "claude";
const EVENT_QUEUE: usize = 256;
const RESUME_SEPARATOR: &str = "\r\n\x1b[2m--- sessão retomada ---\x1b[0m\r\n";
/// How long processes get to exit on their own before a hard kill.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);
/// Without output for this long, a session counts as idle. Claude Code
/// streams output every few hundred milliseconds while it works and none
/// at all while it waits at its prompt.
pub const IDLE_AFTER: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum SessionsError {
    #[error("folder does not exist: {0}")]
    InvalidCwd(String),
    #[error("claude was not found in PATH")]
    AgentNotFound,
    #[error("session not found")]
    NotFound,
    #[error("session is {0:?}")]
    InvalidState(SessionStatus),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("failed to save session: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to format timestamp: {0}")]
    Timestamp(#[from] time::error::Format),
    #[error("session registry lock poisoned")]
    Poisoned,
}

impl SessionsError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidCwd(_) => ErrorCode::InvalidCwd,
            Self::AgentNotFound => ErrorCode::AgentNotFound,
            Self::NotFound => ErrorCode::SessionNotFound,
            Self::InvalidState(_) => ErrorCode::InvalidState,
            _ => ErrorCode::Internal,
        }
    }
}

pub struct CreateRequest {
    pub agent: Agent,
    pub cwd: String,
    pub name: Option<String>,
    /// The session takes the project's Claude account and SSH key.
    pub project: Option<Project>,
}

pub struct SessionEntry {
    meta: Mutex<SessionMeta>,
    pub session: Session,
}

/// Every session the daemon knows, persisted in the store. Changes are
/// announced on the events channel for all connected clients.
pub struct Sessions {
    store: SessionStore,
    /// PATH used to find agents and given to every session process.
    path: OsString,
    events: broadcast::Sender<ServerMessage>,
    entries: Mutex<HashMap<String, Arc<SessionEntry>>>,
    /// Lets process exit callbacks reach the registry without keeping it alive.
    this: Weak<Sessions>,
}

impl Sessions {
    /// Loads saved sessions. Any that were running belonged to a daemon that
    /// is gone, so their processes are lost: they become interrupted.
    pub fn load(store: SessionStore, path: OsString) -> Arc<Self> {
        let (events, _) = broadcast::channel(EVENT_QUEUE);
        let mut entries = HashMap::new();
        for mut meta in store.load_all() {
            if meta.status.is_live() {
                let last_activity = store.transcript_modified(&meta.id);
                mark_interrupted(&mut meta, last_activity);
                if let Err(error) = store.save(&meta) {
                    eprintln!("failed to save session {}: {error}", meta.id);
                }
            }
            let transcript = store
                .open_transcript(&meta.id)
                .map_err(|error| eprintln!("failed to open transcript {}: {error}", meta.id))
                .ok();
            let entry = SessionEntry {
                meta: Mutex::new(meta.clone()),
                session: Session::new(transcript),
            };
            entries.insert(meta.id, Arc::new(entry));
        }
        Arc::new_cyclic(|this| Self {
            store,
            path,
            events,
            entries: Mutex::new(entries),
            this: this.clone(),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.events.subscribe()
    }

    /// Sessions in creation order.
    pub fn list(&self) -> Vec<SessionMeta> {
        let Ok(entries) = self.entries.lock() else {
            return Vec::new();
        };
        let mut sessions: Vec<SessionMeta> = entries
            .values()
            .filter_map(|entry| entry.meta.lock().ok().map(|meta| meta.clone()))
            .collect();
        sessions.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        sessions
    }

    pub fn get(&self, id: &str) -> Option<Arc<SessionEntry>> {
        self.entries.lock().ok()?.get(id).cloned()
    }

    pub fn create(&self, request: CreateRequest) -> Result<SessionMeta, SessionsError> {
        let cwd = existing_dir(&request.cwd)?;
        let agent_session_id = match request.agent {
            Agent::Claude => Some(Uuid::new_v4().to_string()),
            Agent::Shell => None,
        };
        let project = request.project.as_ref();
        let meta = SessionMeta {
            id: Uuid::new_v4().to_string(),
            name: session_name(request.name.as_deref(), &cwd),
            agent: request.agent,
            cwd: request.cwd,
            agent_session_id,
            status: SessionStatus::Running,
            exit_code: None,
            created_at: now()?,
            ended_at: None,
            idle_since: None,
            project_id: project.map(|project| project.id.clone()),
            claude_config_dir: project.and_then(|project| project.claude_config_dir.clone()),
            ssh_key: project.and_then(|project| project.ssh_key.clone()),
        };
        let launch = launch_for(&self.path, &meta, &cwd, false)?;
        let session = Session::new(Some(self.store.open_transcript(&meta.id)?));
        let started = session
            .start(&launch, self.exit_handler(&meta.id))
            .map_err(SessionsError::from)
            .and_then(|()| self.store.save(&meta).map_err(SessionsError::from));
        if let Err(error) = started {
            // Nothing should remain of a session that never started.
            let _ = self.store.remove(&meta.id);
            return Err(error);
        }
        let entry = Arc::new(SessionEntry {
            meta: Mutex::new(meta.clone()),
            session,
        });
        lock(&self.entries)?.insert(meta.id.clone(), entry);
        self.announce(ServerMessage::SessionCreated {
            session: meta.clone(),
        });
        Ok(meta)
    }

    /// Starts a new process for an exited or interrupted session: Claude
    /// continues its conversation, a shell starts fresh in the same folder.
    pub fn resume(&self, id: &str) -> Result<SessionMeta, SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let mut meta = lock(&entry.meta)?;
        if !matches!(
            meta.status,
            SessionStatus::Exited | SessionStatus::Interrupted
        ) {
            return Err(SessionsError::InvalidState(meta.status));
        }
        let cwd = existing_dir(&meta.cwd)?;
        let launch = launch_for(&self.path, &meta, &cwd, true)?;
        entry.session.write_marker(RESUME_SEPARATOR)?;
        entry.session.start(&launch, self.exit_handler(id))?;
        meta.status = SessionStatus::Running;
        meta.exit_code = None;
        meta.ended_at = None;
        meta.idle_since = None;
        self.store.save(&meta)?;
        let updated = meta.clone();
        drop(meta);
        self.announce(ServerMessage::SessionUpdated {
            session: updated.clone(),
        });
        Ok(updated)
    }

    pub fn claude_available(&self) -> bool {
        executable::find_in(&self.path, CLAUDE_COMMAND).is_some()
    }

    /// Ends every process for "Encerrar tudo" or a daemon stop. Sessions are
    /// saved as interrupted first, so they can be resumed later. Blocks for
    /// up to the grace period.
    pub fn shutdown(&self) {
        let entries: Vec<Arc<SessionEntry>> = match self.entries.lock() {
            Ok(entries) => entries.values().cloned().collect(),
            Err(_) => return,
        };
        for entry in &entries {
            if let Ok(mut meta) = entry.meta.lock()
                && meta.status.is_live()
            {
                mark_interrupted(&mut meta, None);
                if let Err(error) = self.store.save(&meta) {
                    eprintln!("failed to save session {}: {error}", meta.id);
                }
            }
            entry.session.terminate();
        }
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while entries.iter().any(|entry| entry.session.is_alive()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        for entry in &entries {
            entry.session.force_kill();
        }
    }

    pub fn announce_projects(&self, projects: Vec<Project>) {
        self.announce(ServerMessage::ProjectListResult { projects });
    }

    /// Moves live sessions between running and idle based on their last
    /// output. Called periodically; announces only actual changes.
    pub fn refresh_idle(&self, now: Instant) {
        let entries: Vec<Arc<SessionEntry>> = match self.entries.lock() {
            Ok(entries) => entries.values().cloned().collect(),
            Err(_) => return,
        };
        for entry in entries {
            let Some(last_output) = entry.session.last_output() else {
                continue;
            };
            let Ok(mut meta) = entry.meta.lock() else {
                continue;
            };
            let quiet_for = now.saturating_duration_since(last_output);
            let next = match meta.status {
                SessionStatus::Running if quiet_for >= IDLE_AFTER => SessionStatus::Idle,
                SessionStatus::Idle if quiet_for < IDLE_AFTER => SessionStatus::Running,
                _ => continue,
            };
            meta.status = next;
            meta.idle_since = (next == SessionStatus::Idle)
                .then(|| SystemTime::now().checked_sub(quiet_for))
                .flatten()
                .and_then(|since| OffsetDateTime::from(since).format(&Rfc3339).ok());
            if let Err(error) = self.store.save(&meta) {
                eprintln!("failed to save session {}: {error}", meta.id);
            }
            let updated = meta.clone();
            drop(meta);
            self.announce(ServerMessage::SessionUpdated { session: updated });
        }
    }

    fn exit_handler(&self, id: &str) -> impl FnOnce(Option<u32>) + Send + 'static {
        let sessions = self.this.clone();
        let id = id.to_owned();
        move |exit_code| {
            if let Some(sessions) = sessions.upgrade() {
                sessions.process_exited(&id, exit_code);
            }
        }
    }

    fn process_exited(&self, id: &str, exit_code: Option<u32>) {
        let Some(entry) = self.get(id) else {
            return;
        };
        let Ok(mut meta) = entry.meta.lock() else {
            return;
        };
        if !meta.status.is_live() {
            return;
        }
        meta.status = SessionStatus::Exited;
        meta.idle_since = None;
        meta.exit_code = exit_code;
        meta.ended_at = now().ok();
        if let Err(error) = self.store.save(&meta) {
            eprintln!("failed to save session {id}: {error}");
        }
        let updated = meta.clone();
        drop(meta);
        self.announce(ServerMessage::SessionUpdated { session: updated });
    }

    fn announce(&self, message: ServerMessage) {
        // No receivers simply means no client is connected.
        let _ = self.events.send(message);
    }
}

fn mark_interrupted(meta: &mut SessionMeta, last_activity: Option<SystemTime>) {
    meta.status = SessionStatus::Interrupted;
    let ended = last_activity.map_or_else(OffsetDateTime::now_utc, OffsetDateTime::from);
    meta.ended_at = ended.format(&Rfc3339).ok();
}

fn existing_dir(path: &str) -> Result<PathBuf, SessionsError> {
    let dir = PathBuf::from(path);
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(SessionsError::InvalidCwd(path.to_owned()))
    }
}

fn launch_for(
    path: &OsStr,
    meta: &SessionMeta,
    cwd: &Path,
    resume: bool,
) -> Result<Launch, SessionsError> {
    let (program, args) = match (meta.agent, meta.agent_session_id.as_deref()) {
        (Agent::Claude, Some(id)) => {
            let program =
                executable::find_in(path, CLAUDE_COMMAND).ok_or(SessionsError::AgentNotFound)?;
            let flag = if resume { "--resume" } else { "--session-id" };
            (
                Some(program.to_string_lossy().into_owned()),
                vec![flag.to_owned(), id.to_owned()],
            )
        }
        _ => (None, Vec::new()),
    };
    Ok(Launch {
        program,
        args,
        cwd: cwd.to_path_buf(),
        path: Some(path.to_os_string()),
        env: environment(meta),
    })
}

/// Account and SSH key of the session's project. Without a project the
/// default Claude account is used, even if the daemon inherited another.
/// Shells get them too, so `claude` and `git` run inside them match.
fn environment(meta: &SessionMeta) -> Vec<(String, Option<String>)> {
    let mut env = vec![(
        "CLAUDE_CONFIG_DIR".to_owned(),
        meta.claude_config_dir.clone(),
    )];
    if let Some(key) = &meta.ssh_key {
        env.push(("GIT_SSH_COMMAND".to_owned(), Some(ssh_command(key))));
    }
    env
}

fn ssh_command(key: &str) -> String {
    // Single quotes keep spaces in the path; embedded quotes are escaped.
    let quoted = key.replace('\'', "'\\''");
    format!("ssh -i '{quoted}' -o IdentitiesOnly=yes")
}

/// Uses the given name, or the folder name when it is blank.
fn session_name(name: Option<&str>, cwd: &Path) -> String {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_owned(),
        None => cwd
            .file_name()
            .map(|folder| folder.to_string_lossy().into_owned())
            .unwrap_or_else(|| cwd.display().to_string()),
    }
}

fn now() -> Result<String, time::error::Format> {
    OffsetDateTime::now_utc().format(&Rfc3339)
}

fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, SessionsError> {
    mutex.lock().map_err(|_| SessionsError::Poisoned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("cohors-sessions-{}", uuid::Uuid::new_v4()))
    }

    fn load(root: &Path) -> Arc<Sessions> {
        Sessions::load(
            SessionStore::new(root.to_path_buf()),
            std::env::var_os("PATH").unwrap_or_default(),
        )
    }

    fn shell_request(name: &str) -> CreateRequest {
        CreateRequest {
            agent: Agent::Shell,
            cwd: std::env::temp_dir().display().to_string(),
            name: Some(name.into()),
            project: None,
        }
    }

    fn status_of(sessions: &Sessions, id: &str) -> SessionStatus {
        sessions
            .get(id)
            .expect("entry")
            .meta
            .lock()
            .expect("meta")
            .status
    }

    fn wait_for_status(sessions: &Sessions, id: &str, status: SessionStatus) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if status_of(sessions, id) == status {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn blank_name_falls_back_to_folder_name() {
        let cwd = Path::new("/home/me/dev/cohors");
        assert_eq!(session_name(None, cwd), "cohors");
        assert_eq!(session_name(Some("  "), cwd), "cohors");
        assert_eq!(session_name(Some(" api "), cwd), "api");
    }

    #[test]
    fn root_folder_name_uses_full_path() {
        assert_eq!(session_name(None, Path::new("/")), "/");
    }

    #[test]
    fn claude_resume_uses_resume_flag() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let meta = SessionMeta {
            agent: Agent::Claude,
            agent_session_id: Some("abc".into()),
            ..meta("s")
        };
        let Ok(launch) = launch_for(&path, &meta, Path::new("/tmp"), true) else {
            return; // claude is not installed on this machine
        };
        assert_eq!(launch.args, ["--resume", "abc"]);
    }

    #[test]
    fn project_environment_sets_account_and_ssh_key() {
        let meta = SessionMeta {
            claude_config_dir: Some("/home/me/.claude-work".into()),
            ssh_key: Some("/home/me/.ssh/it's key".into()),
            ..meta("s")
        };
        assert_eq!(
            environment(&meta),
            [
                (
                    "CLAUDE_CONFIG_DIR".to_owned(),
                    Some("/home/me/.claude-work".to_owned())
                ),
                (
                    "GIT_SSH_COMMAND".to_owned(),
                    Some("ssh -i '/home/me/.ssh/it'\\''s key' -o IdentitiesOnly=yes".to_owned())
                ),
            ]
        );
    }

    #[test]
    fn default_account_clears_inherited_config_dir() {
        assert_eq!(
            environment(&meta("s")),
            [("CLAUDE_CONFIG_DIR".to_owned(), None)]
        );
    }

    fn meta(id: &str) -> SessionMeta {
        SessionMeta {
            id: id.into(),
            name: id.into(),
            agent: Agent::Shell,
            cwd: "/tmp".into(),
            agent_session_id: None,
            status: SessionStatus::Running,
            exit_code: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            ended_at: None,
            idle_since: None,
            project_id: None,
            claude_config_dir: None,
            ssh_key: None,
        }
    }

    #[test]
    fn running_sessions_load_as_interrupted() {
        let root = temp_root();
        let store = SessionStore::new(root.clone());
        let mut meta = meta("a");
        store.save(&meta).expect("save");
        meta.id = "b".into();
        meta.status = SessionStatus::Exited;
        store.save(&meta).expect("save");

        let sessions = load(&root);
        assert_eq!(status_of(&sessions, "a"), SessionStatus::Interrupted);
        assert_eq!(status_of(&sessions, "b"), SessionStatus::Exited);
        let reloaded = SessionStore::new(root.clone()).load_all();
        let a = reloaded.iter().find(|meta| meta.id == "a").expect("a");
        assert_eq!(a.status, SessionStatus::Interrupted);
        assert!(a.ended_at.is_some());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_missing_folder() {
        let sessions = load(&temp_root());
        let result = sessions.create(CreateRequest {
            agent: Agent::Shell,
            cwd: "/definitely/not/here".into(),
            name: None,
            project: None,
        });
        assert!(matches!(result, Err(SessionsError::InvalidCwd(_))));
    }

    #[cfg(unix)]
    #[test]
    fn created_session_is_saved_and_announced() {
        let root = temp_root();
        let sessions = load(&root);
        let mut events = sessions.subscribe();
        let meta = sessions.create(shell_request("build")).expect("create");
        assert_eq!(meta.name, "build");
        assert!(matches!(
            events.try_recv(),
            Ok(ServerMessage::SessionCreated { .. })
        ));
        let saved = SessionStore::new(root.clone()).load_all();
        assert_eq!(saved.len(), 1);
        assert!(root.join(&meta.id).join("transcript.log").exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn quiet_session_turns_idle_and_output_makes_it_run_again() {
        let root = temp_root();
        let sessions = load(&root);
        let mut events = sessions.subscribe();
        let meta = sessions.create(shell_request("s")).expect("create");
        let _ = events.try_recv();
        let entry = sessions.get(&meta.id).expect("entry");

        sessions.refresh_idle(Instant::now());
        assert_eq!(status_of(&sessions, &meta.id), SessionStatus::Running);

        sessions.refresh_idle(Instant::now() + IDLE_AFTER + Duration::from_secs(5));
        assert_eq!(status_of(&sessions, &meta.id), SessionStatus::Idle);
        assert!(entry.meta.lock().expect("meta").idle_since.is_some());
        assert!(matches!(
            events.try_recv(),
            Ok(ServerMessage::SessionUpdated { .. })
        ));
        sessions.refresh_idle(Instant::now() + IDLE_AFTER + Duration::from_secs(5));
        assert!(events.try_recv().is_err(), "no event without a change");

        entry.session.write_marker("activity").expect("output");
        sessions.refresh_idle(Instant::now());
        assert_eq!(status_of(&sessions, &meta.id), SessionStatus::Running);
        assert_eq!(entry.meta.lock().expect("meta").idle_since, None);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn shutdown_interrupts_running_sessions() {
        let root = temp_root();
        let sessions = load(&root);
        let meta = sessions.create(shell_request("s")).expect("create");
        sessions.shutdown();
        assert_eq!(status_of(&sessions, &meta.id), SessionStatus::Interrupted);
        assert!(!sessions.get(&meta.id).expect("entry").session.is_alive());
        let saved = SessionStore::new(root.clone()).load_all();
        assert_eq!(saved[0].status, SessionStatus::Interrupted);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn exit_marks_session_exited_and_resume_restarts_it() {
        let root = temp_root();
        let sessions = load(&root);
        let meta = sessions.create(shell_request("s")).expect("create");
        let entry = sessions.get(&meta.id).expect("entry");
        entry.session.write_input("exit 4\r").expect("write");
        assert!(wait_for_status(&sessions, &meta.id, SessionStatus::Exited));
        let exited = entry.meta.lock().expect("meta").clone();
        assert_eq!(exited.exit_code, Some(4));
        assert!(exited.ended_at.is_some());

        let resumed = sessions.resume(&meta.id).expect("resume");
        assert_eq!(resumed.status, SessionStatus::Running);
        assert_eq!(resumed.exit_code, None);
        assert!(matches!(
            sessions.resume(&meta.id),
            Err(SessionsError::InvalidState(SessionStatus::Running))
        ));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
