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

use crate::conversation::{self, Conversation};
use crate::executable;
use crate::hooks::{self, Signal};
use crate::protocol::{
    Agent, ErrorCode, Project, ServerMessage, SessionMeta, SessionStatus, SummaryStatus,
};
use crate::session::{Launch, Session, SessionError};
use crate::store::SessionStore;
use crate::summary;
use crate::worktree::{Git, RemovedBranch, WorktreeError, WorktreeStatus};

const CLAUDE_COMMAND: &str = "claude";
const EVENT_QUEUE: usize = 256;
const RESUME_SEPARATOR: &str = "\r\n\x1b[2m--- sessão retomada ---\x1b[0m\r\n";
/// How long processes get to exit on their own before a hard kill.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);
/// Without output for this long, a session counts as idle. Claude Code
/// streams output every few hundred milliseconds while it works and none
/// at all while it waits at its prompt.
pub const IDLE_AFTER: Duration = Duration::from_secs(10);
/// The history view shows at most this much of a transcript.
const TRANSCRIPT_VIEW_BYTES: u64 = 2 * 1024 * 1024;

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
    #[error("session name is empty")]
    EmptyName,
    #[error("session has no worktree")]
    NoWorktree,
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
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
            Self::EmptyName | Self::NoWorktree => ErrorCode::BadRequest,
            Self::Worktree(error) => match error {
                WorktreeError::NotARepository(_) => ErrorCode::NotARepository,
                WorktreeError::NoCommits => ErrorCode::NoCommits,
                WorktreeError::Dirty => ErrorCode::WorktreeDirty,
                _ => ErrorCode::GitFailed,
            },
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
    /// The project's context folder, already created.
    pub context_dir: Option<String>,
    /// Start in a new git worktree of the folder's repository.
    pub worktree: bool,
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
    /// Where session worktrees are created.
    worktrees_dir: PathBuf,
    events: broadcast::Sender<ServerMessage>,
    entries: Mutex<HashMap<String, Arc<SessionEntry>>>,
    /// Lets process exit callbacks reach the registry without keeping it alive.
    this: Weak<Sessions>,
}

impl Sessions {
    /// Loads saved sessions. Any that were running belonged to a daemon that
    /// is gone, so their processes are lost: they become interrupted.
    pub fn load(store: SessionStore, path: OsString, worktrees_dir: PathBuf) -> Arc<Self> {
        let (events, _) = broadcast::channel(EVENT_QUEUE);
        let mut entries = HashMap::new();
        for mut meta in store.load_all() {
            let mut changed = false;
            if meta.attention.take().is_some() {
                changed = true;
            }
            if meta.status.is_live() {
                let last_activity = store.transcript_modified(&meta.id);
                mark_interrupted(&mut meta, last_activity);
                changed = true;
            }
            // The daemon that was generating this summary is gone.
            if meta.summary_status == SummaryStatus::Pending {
                meta.summary_status = SummaryStatus::Failed;
                changed = true;
            }
            if changed && let Err(error) = store.save(&meta) {
                eprintln!("failed to save session {}: {error}", meta.id);
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
            worktrees_dir,
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
        let chosen = existing_dir(&request.cwd)?;
        let agent_session_id = match request.agent {
            Agent::Claude => Some(Uuid::new_v4().to_string()),
            Agent::Shell => None,
        };
        let id = Uuid::new_v4().to_string();
        let name = session_name(request.name.as_deref(), &chosen);
        let (worktree, cwd) = if request.worktree {
            let git = Git::find(&self.path)?;
            let (worktree, cwd) = git.create(&chosen, &self.worktrees_dir, &name, &id)?;
            (Some(worktree), cwd)
        } else {
            (None, chosen)
        };
        let project = request.project.as_ref();
        let meta = SessionMeta {
            id,
            name,
            agent: request.agent,
            cwd: cwd.display().to_string(),
            agent_session_id,
            status: SessionStatus::Running,
            exit_code: None,
            created_at: now()?,
            ended_at: None,
            idle_since: None,
            project_id: project.map(|project| project.id.clone()),
            claude_config_dir: project.and_then(|project| project.claude_config_dir.clone()),
            ssh_key: project.and_then(|project| project.ssh_key.clone()),
            context_dir: request.context_dir,
            attention: None,
            worktree,
            notes: String::new(),
            summary_status: SummaryStatus::None,
        };
        let started = launch_for(&self.path, &meta, &cwd, false).and_then(|launch| {
            let session = Session::new(Some(self.store.open_transcript(&meta.id)?));
            session.start(&launch, self.exit_handler(&meta.id))?;
            self.store.save(&meta)?;
            Ok(session)
        });
        let session = match started {
            Ok(session) => session,
            Err(error) => {
                // Nothing should remain of a session that never started.
                let _ = self.store.remove(&meta.id);
                if let Some(worktree) = &meta.worktree
                    && let Ok(git) = Git::find(&self.path)
                {
                    let _ = git.remove(worktree);
                }
                return Err(error);
            }
        };
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
        meta.attention = None;
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
        }
        stop_processes(&entries);
    }

    /// Moves a session to the history. Its process is stopped separately,
    /// with `stop_process`, because that blocks. Returns the session; a
    /// `Pending` summary status means the caller should run the summary.
    pub fn finish(&self, id: &str, with_summary: bool) -> Result<SessionMeta, SessionsError> {
        self.update(id, |meta| {
            if meta.status == SessionStatus::Finished {
                return Err(SessionsError::InvalidState(meta.status));
            }
            let summarize = with_summary && can_summarize(meta);
            meta.status = SessionStatus::Finished;
            meta.idle_since = None;
            meta.attention = None;
            // Exited and interrupted sessions keep the time they actually stopped.
            if meta.ended_at.is_none() {
                meta.ended_at = Some(now()?);
            }
            meta.summary_status = if summarize {
                SummaryStatus::Pending
            } else {
                SummaryStatus::None
            };
            Ok(())
        })
    }

    /// Blocks until the session's process is gone.
    pub fn stop_process(&self, id: &str) {
        if let Some(entry) = self.get(id) {
            stop_processes(&[entry]);
        }
    }

    /// Asks for a new summary of a finished Claude session.
    pub fn retry_summary(&self, id: &str) -> Result<SessionMeta, SessionsError> {
        self.update(id, |meta| {
            let allowed = meta.status == SessionStatus::Finished
                && can_summarize(meta)
                && meta.summary_status != SummaryStatus::Pending;
            if !allowed {
                return Err(SessionsError::InvalidState(meta.status));
            }
            meta.summary_status = SummaryStatus::Pending;
            Ok(())
        })
    }

    /// "Finalizar sem resumo": drops a pending or failed summary.
    pub fn skip_summary(&self, id: &str) -> Result<SessionMeta, SessionsError> {
        self.update(id, |meta| {
            if meta.status != SessionStatus::Finished {
                return Err(SessionsError::InvalidState(meta.status));
            }
            meta.summary_status = SummaryStatus::None;
            Ok(())
        })
    }

    /// How `claude -p --resume` must run to summarize this session: same
    /// folder, account and SSH key as the session itself.
    pub fn summary_launch(&self, id: &str) -> Result<Launch, SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let meta = lock(&entry.meta)?.clone();
        let cwd = existing_dir(&meta.cwd)?;
        let mut launch = launch_for(&self.path, &meta, &cwd, true)?;
        launch.args = vec![
            "-p".to_owned(),
            "--resume".to_owned(),
            meta.agent_session_id.unwrap_or_default(),
            summary::PROMPT.to_owned(),
        ];
        Ok(launch)
    }

    /// Records a summary result, unless the user skipped it meanwhile.
    pub fn summary_finished(&self, id: &str, result: Result<String, String>) {
        let Some(entry) = self.get(id) else {
            return;
        };
        let Ok(mut meta) = entry.meta.lock() else {
            return;
        };
        if meta.summary_status != SummaryStatus::Pending {
            return;
        }
        let (status, text) = match result {
            Ok(text) => match self.store.save_summary(id, &text) {
                Ok(()) => (SummaryStatus::Done, Some(text)),
                Err(error) => {
                    eprintln!("failed to save summary {id}: {error}");
                    (SummaryStatus::Failed, None)
                }
            },
            Err(reason) => {
                eprintln!("summary of {id} failed: {reason}");
                (SummaryStatus::Failed, None)
            }
        };
        meta.summary_status = status;
        if let Err(error) = self.store.save(&meta) {
            eprintln!("failed to save session {id}: {error}");
        }
        let updated = meta.clone();
        drop(meta);
        self.announce(ServerMessage::SessionUpdated { session: updated });
        self.announce(ServerMessage::SessionSummary {
            id: id.to_owned(),
            status,
            text,
        });
    }

    pub fn summary(&self, id: &str) -> Result<ServerMessage, SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let status = lock(&entry.meta)?.summary_status;
        Ok(ServerMessage::SessionSummary {
            id: id.to_owned(),
            status,
            text: self.store.read_summary(id)?,
        })
    }

    pub fn set_notes(&self, id: &str, notes: String) -> Result<SessionMeta, SessionsError> {
        self.update(id, |meta| {
            meta.notes = notes;
            Ok(())
        })
    }

    /// Any session can be renamed, finished ones included.
    pub fn rename(&self, id: &str, name: &str) -> Result<SessionMeta, SessionsError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SessionsError::EmptyName);
        }
        self.update(id, |meta| {
            meta.name = name.to_owned();
            Ok(())
        })
    }

    /// The Claude conversation of a session, when Claude wrote a log for it.
    /// `home` locates the default Claude config folder.
    pub fn conversation(
        &self,
        id: &str,
        home: &Path,
    ) -> Result<Option<Conversation>, SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let meta = lock(&entry.meta)?.clone();
        Ok(conversation_of(&meta, home))
    }

    /// Finished sessions whose name, notes, summary or conversation text
    /// contain `query`, ignoring case, in creation order.
    pub fn search(&self, query: &str, home: &Path) -> Vec<String> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let contains = |text: &str| text.to_lowercase().contains(&query);
        self.list()
            .into_iter()
            .filter(|meta| meta.status == SessionStatus::Finished)
            .filter(|meta| {
                contains(&meta.name)
                    || contains(&meta.notes)
                    || self
                        .store
                        .read_summary(&meta.id)
                        .ok()
                        .flatten()
                        .is_some_and(|summary| contains(&summary))
                    || conversation_of(meta, home)
                        .is_some_and(|conversation| conversation.mentions(&query))
            })
            .map(|meta| meta.id)
            .collect()
    }

    pub fn worktree_status(&self, id: &str) -> Result<WorktreeStatus, SessionsError> {
        let worktree = self.worktree_of(id)?;
        Ok(Git::find(&self.path)?.status(&worktree)?)
    }

    /// Removes the worktree of a session that no longer runs, if it has no
    /// uncommitted changes. The session keeps the record of it.
    pub fn remove_worktree(&self, id: &str) -> Result<RemovedBranch, SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let meta = lock(&entry.meta)?.clone();
        if meta.status.is_live() {
            return Err(SessionsError::InvalidState(meta.status));
        }
        let worktree = meta.worktree.ok_or(SessionsError::NoWorktree)?;
        Ok(Git::find(&self.path)?.remove(&worktree)?)
    }

    fn worktree_of(&self, id: &str) -> Result<crate::worktree::Worktree, SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let worktree = lock(&entry.meta)?.worktree.clone();
        worktree.ok_or(SessionsError::NoWorktree)
    }

    pub fn transcript(&self, id: &str) -> Result<ServerMessage, SessionsError> {
        self.get(id).ok_or(SessionsError::NotFound)?;
        let bytes = self.store.read_transcript(id, TRANSCRIPT_VIEW_BYTES + 1)?;
        let truncated = bytes.len() as u64 > TRANSCRIPT_VIEW_BYTES;
        let start = bytes.len().saturating_sub(TRANSCRIPT_VIEW_BYTES as usize);
        Ok(ServerMessage::SessionTranscriptResult {
            id: id.to_owned(),
            data: String::from_utf8_lossy(&bytes[start..]).into_owned(),
            truncated,
        })
    }

    /// Removes a finished session and its folder for good.
    pub fn delete(&self, id: &str) -> Result<(), SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let meta = lock(&entry.meta)?;
        if meta.status != SessionStatus::Finished || meta.summary_status == SummaryStatus::Pending {
            return Err(SessionsError::InvalidState(meta.status));
        }
        drop(meta);
        lock(&self.entries)?.remove(id);
        // Windows cannot delete a folder while a file in it is open.
        entry.session.close_transcript();
        self.store.remove(id)?;
        self.announce(ServerMessage::SessionDeleted { id: id.to_owned() });
        Ok(())
    }

    /// Applies `change` to a session's metadata, saves it and tells clients.
    fn update(
        &self,
        id: &str,
        change: impl FnOnce(&mut SessionMeta) -> Result<(), SessionsError>,
    ) -> Result<SessionMeta, SessionsError> {
        let entry = self.get(id).ok_or(SessionsError::NotFound)?;
        let mut meta = lock(&entry.meta)?;
        change(&mut meta)?;
        self.store.save(&meta)?;
        let updated = meta.clone();
        drop(meta);
        self.announce(ServerMessage::SessionUpdated {
            session: updated.clone(),
        });
        Ok(updated)
    }

    pub fn announce_projects(&self, projects: Vec<Project>) {
        self.announce(ServerMessage::ProjectListResult { projects });
    }

    /// Applies the signals the Claude hooks left. A session already waiting
    /// keeps its first reason: a `Notification` after `Stop` is Claude Code
    /// reminding that the user has not answered yet.
    pub fn refresh_attention(&self) {
        let entries: Vec<Arc<SessionEntry>> = match self.entries.lock() {
            Ok(entries) => entries.values().cloned().collect(),
            Err(_) => return,
        };
        for entry in entries {
            let Ok(mut meta) = entry.meta.lock() else {
                continue;
            };
            if meta.agent != Agent::Claude {
                continue;
            }
            let signal = match self.store.take_signal(&meta.id) {
                Ok(Some(signal)) => signal,
                Ok(None) => continue,
                Err(error) => {
                    eprintln!("failed to read hook signal {}: {error}", meta.id);
                    continue;
                }
            };
            let next = match signal {
                Signal::Waiting(_) if !meta.status.is_live() => continue,
                Signal::Waiting(_) if meta.attention.is_some() => continue,
                Signal::Waiting(reason) => Some(reason),
                Signal::Clear if meta.attention.is_none() => continue,
                Signal::Clear => None,
            };
            meta.attention = next;
            self.save_and_announce(meta);
        }
    }

    /// The user typed in the session: it no longer waits.
    pub fn clear_attention(&self, id: &str) {
        let Some(entry) = self.get(id) else {
            return;
        };
        let Ok(mut meta) = entry.meta.lock() else {
            return;
        };
        if meta.attention.take().is_some() {
            self.save_and_announce(meta);
        }
    }

    fn save_and_announce(&self, meta: MutexGuard<'_, SessionMeta>) {
        if let Err(error) = self.store.save(&meta) {
            eprintln!("failed to save session {}: {error}", meta.id);
        }
        let updated = meta.clone();
        drop(meta);
        self.announce(ServerMessage::SessionUpdated { session: updated });
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
        meta.attention = None;
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

fn can_summarize(meta: &SessionMeta) -> bool {
    meta.agent == Agent::Claude && meta.agent_session_id.is_some()
}

/// Asks the processes to end, waits a grace period, then kills the rest.
fn stop_processes(entries: &[Arc<SessionEntry>]) {
    for entry in entries {
        entry.session.terminate();
    }
    let deadline = Instant::now() + SHUTDOWN_GRACE;
    while entries.iter().any(|entry| entry.session.is_alive()) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    for entry in entries {
        entry.session.force_kill();
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
            let mut args = vec![flag.to_owned(), id.to_owned()];
            args.extend(hook_args(meta));
            (Some(program.to_string_lossy().into_owned()), args)
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

/// Claude keeps its logs in the session's config folder: the project's
/// account, or `~/.claude` for the default one.
fn conversation_of(meta: &SessionMeta, home: &Path) -> Option<Conversation> {
    if meta.agent != Agent::Claude {
        return None;
    }
    let session_id = meta.agent_session_id.as_deref()?;
    let config_dir = meta
        .claude_config_dir
        .as_ref()
        .map_or_else(|| home.join(".claude"), PathBuf::from);
    let path = conversation::find_log(&config_dir, session_id)?;
    conversation::read(&path)
        .map_err(|error| eprintln!("failed to read conversation {}: {error}", meta.id))
        .ok()
}

/// Hooks for a Claude session, with its project context while the folder
/// exists (a deleted project loses it). None when the daemon cannot name its
/// own binary for the hooks.
fn hook_args(meta: &SessionMeta) -> Vec<String> {
    let Ok(program) = std::env::current_exe() else {
        return Vec::new();
    };
    let context_dir = meta
        .context_dir
        .as_deref()
        .map(Path::new)
        .filter(|dir| dir.is_dir());
    hooks::claude_args(&program, context_dir)
}

/// Account and SSH key of the session's project. Without a project the
/// default Claude account is used, even if the daemon inherited another.
/// Shells get them too, so `claude` and `git` run inside them match.
fn environment(meta: &SessionMeta) -> Vec<(String, Option<String>)> {
    let mut env = vec![
        (
            "CLAUDE_CONFIG_DIR".to_owned(),
            meta.claude_config_dir.clone(),
        ),
        // Lets the hooks find their session.
        (hooks::SESSION_ENV.to_owned(), Some(meta.id.clone())),
    ];
    if let Some(key) = &meta.ssh_key {
        env.push(("GIT_SSH_COMMAND".to_owned(), Some(ssh_command(key))));
    }
    env
}

fn ssh_command(key: &str) -> String {
    format!("ssh -i {} -o IdentitiesOnly=yes", hooks::shell_quote(key))
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
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("cohors-sessions-{}", uuid::Uuid::new_v4()))
    }

    fn load(root: &Path) -> Arc<Sessions> {
        Sessions::load(
            SessionStore::new(root.to_path_buf()),
            std::env::var_os("PATH").unwrap_or_default(),
            root.join("worktrees"),
        )
    }

    #[cfg(unix)]
    fn shell_request(name: &str) -> CreateRequest {
        CreateRequest {
            agent: Agent::Shell,
            cwd: std::env::temp_dir().display().to_string(),
            name: Some(name.into()),
            project: None,
            context_dir: None,
            worktree: false,
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

    #[cfg(unix)]
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
        assert_eq!(launch.args[..3], ["--resume", "abc", "--settings"]);
    }

    #[test]
    fn claude_gets_the_context_folder_while_it_exists() {
        let dir = temp_root();
        std::fs::create_dir_all(&dir).expect("dir");
        let with_context = SessionMeta {
            context_dir: Some(dir.display().to_string()),
            ..meta("s")
        };
        let args = hook_args(&with_context);
        assert_eq!(
            args[..2],
            ["--add-dir".to_owned(), dir.display().to_string()]
        );
        assert_eq!(args[2], "--settings");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(hook_args(&with_context)[0], "--settings");
        assert_eq!(hook_args(&meta("s"))[0], "--settings");
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
                (hooks::SESSION_ENV.to_owned(), Some("s".to_owned())),
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
            [
                ("CLAUDE_CONFIG_DIR".to_owned(), None),
                (hooks::SESSION_ENV.to_owned(), Some("s".to_owned())),
            ]
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
            context_dir: None,
            attention: None,
            worktree: None,
            notes: String::new(),
            summary_status: SummaryStatus::None,
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
    fn rename_trims_and_rejects_blank_names() {
        let root = temp_root();
        let store = SessionStore::new(root.clone());
        store.save(&meta("a")).expect("save");
        let sessions = load(&root);
        let renamed = sessions.rename("a", "  api login ").expect("rename");
        assert_eq!(renamed.name, "api login");
        assert!(matches!(
            sessions.rename("a", "   "),
            Err(SessionsError::EmptyName)
        ));
        assert!(matches!(
            sessions.rename("missing", "x"),
            Err(SessionsError::NotFound)
        ));
        let reloaded = SessionStore::new(root.clone()).load_all();
        assert_eq!(reloaded[0].name, "api login");
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    /// A live Claude session whose `claude` is a script that waits, so the
    /// test needs no real Claude Code.
    #[cfg(unix)]
    fn fake_claude_session(root: &Path) -> (Arc<Sessions>, SessionMeta) {
        use std::os::unix::fs::PermissionsExt;
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let script = bin.join(CLAUDE_COMMAND);
        std::fs::write(&script, "#!/bin/sh\nexec /bin/sleep 30\n").expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let sessions = Sessions::load(
            SessionStore::new(root.join("sessions")),
            bin.into_os_string(),
            root.join("worktrees"),
        );
        let meta = sessions
            .create(CreateRequest {
                agent: Agent::Claude,
                cwd: std::env::temp_dir().display().to_string(),
                name: None,
                project: None,
                context_dir: None,
                worktree: false,
            })
            .expect("create");
        (sessions, meta)
    }

    #[cfg(unix)]
    #[test]
    fn hook_signals_mark_and_clear_waiting() {
        use crate::protocol::Attention;
        let root = temp_root();
        let (sessions, meta) = fake_claude_session(&root);
        let sessions_dir = root.join("sessions");
        let attention = |sessions: &Sessions| {
            sessions
                .list()
                .into_iter()
                .find(|session| session.id == meta.id)
                .and_then(|session| session.attention)
        };

        hooks::write_signal(&sessions_dir, &meta.id, Signal::Waiting(Attention::Done))
            .expect("signal");
        sessions.refresh_attention();
        assert_eq!(attention(&sessions), Some(Attention::Done));

        // A reminder keeps the first reason.
        hooks::write_signal(&sessions_dir, &meta.id, Signal::Waiting(Attention::Input))
            .expect("signal");
        sessions.refresh_attention();
        assert_eq!(attention(&sessions), Some(Attention::Done));

        hooks::write_signal(&sessions_dir, &meta.id, Signal::Clear).expect("signal");
        sessions.refresh_attention();
        assert_eq!(attention(&sessions), None);

        hooks::write_signal(&sessions_dir, &meta.id, Signal::Waiting(Attention::Input))
            .expect("signal");
        sessions.refresh_attention();
        assert_eq!(attention(&sessions), Some(Attention::Input));
        sessions.clear_attention(&meta.id);
        assert_eq!(attention(&sessions), None);

        sessions.shutdown();
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn search_looks_at_name_notes_summary_and_conversation() {
        let root = temp_root();
        let store = SessionStore::new(root.join("sessions"));
        let config = root.join("claude");
        let log_dir = config.join("projects").join("-dev-api");
        std::fs::create_dir_all(&log_dir).expect("log dir");
        std::fs::write(
            log_dir.join("conv.jsonl"),
            r#"{"type":"user","message":{"content":"Fix the OAuth callback"}}"#,
        )
        .expect("log");
        let finished = |id: &str| SessionMeta {
            status: SessionStatus::Finished,
            ..meta(id)
        };
        store
            .save(&SessionMeta {
                name: "Billing".into(),
                ..finished("by-name")
            })
            .expect("save");
        store
            .save(&SessionMeta {
                notes: "check billing later".into(),
                ..finished("by-notes")
            })
            .expect("save");
        store.save(&finished("by-summary")).expect("save");
        store
            .save_summary("by-summary", "Billing page done")
            .expect("summary");
        store
            .save(&SessionMeta {
                agent: Agent::Claude,
                agent_session_id: Some("conv".into()),
                claude_config_dir: Some(config.display().to_string()),
                ..finished("by-conversation")
            })
            .expect("save");
        store
            .save(&SessionMeta {
                name: "billing live".into(),
                status: SessionStatus::Exited,
                ..meta("not-finished")
            })
            .expect("save");

        let sessions = Sessions::load(store, OsString::new(), root.join("worktrees"));
        let mut billing = sessions.search(" BILLING ", &root);
        billing.sort();
        assert_eq!(billing, ["by-name", "by-notes", "by-summary"]);
        assert_eq!(sessions.search("oauth", &root), ["by-conversation"]);
        assert!(sessions.search("  ", &root).is_empty());
        let conversation = sessions
            .conversation("by-conversation", &root)
            .expect("known")
            .expect("log");
        assert_eq!(conversation.entries.len(), 1);
        assert_eq!(
            sessions.conversation("by-name", &root).expect("known"),
            None
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn worktree_session_runs_in_its_own_checkout() {
        use std::process::Command;
        let root = temp_root();
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args([
                    "-c",
                    "user.name=Cohors",
                    "-c",
                    "user.email=cohors@example.com",
                ])
                .args(args)
                .output()
                .expect("git")
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        std::fs::write(repo.join("README.md"), "hi").expect("file");
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "init"]);

        let sessions = load(&root);
        let meta = sessions
            .create(CreateRequest {
                worktree: true,
                cwd: repo.display().to_string(),
                ..shell_request("isolated")
            })
            .expect("create");
        let worktree = meta.worktree.clone().expect("worktree");
        assert_eq!(meta.cwd, worktree.path);
        assert!(Path::new(&meta.cwd).join("README.md").is_file());
        assert!(matches!(
            sessions.remove_worktree(&meta.id),
            Err(SessionsError::InvalidState(_))
        ));

        sessions.finish(&meta.id, false).expect("finish");
        sessions.stop_process(&meta.id);
        assert!(sessions.worktree_status(&meta.id).expect("status").exists);
        assert_eq!(
            sessions.remove_worktree(&meta.id).expect("remove"),
            RemovedBranch::Deleted
        );
        assert!(!sessions.worktree_status(&meta.id).expect("status").exists);
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
            context_dir: None,
            worktree: false,
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
    fn finish_notes_and_delete() {
        let root = temp_root();
        let sessions = load(&root);
        let meta = sessions.create(shell_request("s")).expect("create");

        let finished = sessions.finish(&meta.id, true).expect("finish");
        assert_eq!(finished.status, SessionStatus::Finished);
        // Shells have no summary even when asked for one.
        assert_eq!(finished.summary_status, SummaryStatus::None);
        assert!(finished.ended_at.is_some());
        sessions.stop_process(&meta.id);
        assert!(!sessions.get(&meta.id).expect("entry").session.is_alive());
        // The exit of the stopped process must not turn it into "exited".
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(status_of(&sessions, &meta.id), SessionStatus::Finished);
        assert!(matches!(
            sessions.finish(&meta.id, false),
            Err(SessionsError::InvalidState(_))
        ));

        let noted = sessions
            .set_notes(&meta.id, "revisar".into())
            .expect("notes");
        assert_eq!(noted.notes, "revisar");

        sessions.delete(&meta.id).expect("delete");
        assert!(sessions.get(&meta.id).is_none());
        assert!(!root.join(&meta.id).exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn live_sessions_cannot_be_deleted() {
        let root = temp_root();
        let sessions = load(&root);
        let meta = sessions.create(shell_request("s")).expect("create");
        assert!(matches!(
            sessions.delete(&meta.id),
            Err(SessionsError::InvalidState(_))
        ));
        sessions.shutdown();
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn summary_result_is_kept_unless_skipped() {
        let root = temp_root();
        let store = SessionStore::new(root.clone());
        let claude = SessionMeta {
            agent: Agent::Claude,
            agent_session_id: Some("agent".into()),
            status: SessionStatus::Exited,
            ..meta("c")
        };
        store.save(&claude).expect("save");
        let sessions = load(&root);

        let finished = sessions.finish("c", true).expect("finish");
        assert_eq!(finished.summary_status, SummaryStatus::Pending);
        sessions.summary_finished("c", Err("no conversation".into()));
        let entry = sessions.get("c").expect("entry");
        assert_eq!(
            entry.meta.lock().expect("meta").summary_status,
            SummaryStatus::Failed
        );

        sessions.retry_summary("c").expect("retry");
        sessions.summary_finished("c", Ok("## O que foi feito".into()));
        let ServerMessage::SessionSummary { status, text, .. } =
            sessions.summary("c").expect("summary")
        else {
            panic!("expected a summary");
        };
        assert_eq!(status, SummaryStatus::Done);
        assert_eq!(text.as_deref(), Some("## O que foi feito"));

        sessions.retry_summary("c").expect("retry again");
        sessions.skip_summary("c").expect("skip");
        sessions.summary_finished("c", Ok("late".into()));
        assert_eq!(
            entry.meta.lock().expect("meta").summary_status,
            SummaryStatus::None
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn pending_summary_fails_after_restart() {
        let root = temp_root();
        let store = SessionStore::new(root.clone());
        store
            .save(&SessionMeta {
                status: SessionStatus::Finished,
                summary_status: SummaryStatus::Pending,
                ..meta("p")
            })
            .expect("save");
        let sessions = load(&root);
        let entry = sessions.get("p").expect("entry");
        assert_eq!(
            entry.meta.lock().expect("meta").summary_status,
            SummaryStatus::Failed
        );
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
