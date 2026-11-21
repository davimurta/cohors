use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::executable;
use crate::protocol::{Agent, ErrorCode, SessionMeta, SessionStatus};
use crate::session::{Launch, Session, SessionError};

const CLAUDE_COMMAND: &str = "claude";

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("folder does not exist: {0}")]
    InvalidCwd(String),
    #[error("claude was not found in PATH")]
    AgentNotFound,
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("failed to format timestamp: {0}")]
    Timestamp(#[from] time::error::Format),
    #[error("session registry lock poisoned")]
    Poisoned,
}

impl CreateError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidCwd(_) => ErrorCode::InvalidCwd,
            Self::AgentNotFound => ErrorCode::AgentNotFound,
            _ => ErrorCode::Internal,
        }
    }
}

pub struct CreateRequest {
    pub agent: Agent,
    pub cwd: String,
    pub name: Option<String>,
}

pub struct SessionEntry {
    pub meta: SessionMeta,
    pub session: Session,
}

#[derive(Default)]
pub struct Sessions {
    entries: Mutex<HashMap<String, Arc<SessionEntry>>>,
}

impl Sessions {
    /// Sessions in creation order.
    pub fn list(&self) -> Vec<SessionMeta> {
        let Ok(entries) = self.entries.lock() else {
            return Vec::new();
        };
        let mut sessions: Vec<SessionMeta> =
            entries.values().map(|entry| entry.meta.clone()).collect();
        sessions.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        sessions
    }

    pub fn get(&self, id: &str) -> Option<Arc<SessionEntry>> {
        self.entries.lock().ok()?.get(id).cloned()
    }

    pub fn create(&self, request: CreateRequest) -> Result<SessionMeta, CreateError> {
        let cwd = PathBuf::from(&request.cwd);
        if !cwd.is_dir() {
            return Err(CreateError::InvalidCwd(request.cwd));
        }
        let agent_session_id = match request.agent {
            Agent::Claude => Some(Uuid::new_v4().to_string()),
            Agent::Shell => None,
        };
        let launch = launch_for(request.agent, &cwd, agent_session_id.as_deref())?;
        let session = Session::new(None);
        session.start(&launch, |_| {})?;
        let meta = SessionMeta {
            id: Uuid::new_v4().to_string(),
            name: session_name(request.name.as_deref(), &cwd),
            agent: request.agent,
            cwd: request.cwd,
            agent_session_id,
            status: SessionStatus::Running,
            created_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        };
        let entry = Arc::new(SessionEntry {
            meta: meta.clone(),
            session,
        });
        self.entries
            .lock()
            .map_err(|_| CreateError::Poisoned)?
            .insert(meta.id.clone(), entry);
        Ok(meta)
    }
}

pub fn claude_available() -> bool {
    executable::find(CLAUDE_COMMAND).is_some()
}

fn launch_for(
    agent: Agent,
    cwd: &Path,
    agent_session_id: Option<&str>,
) -> Result<Launch, CreateError> {
    let (program, args) = match (agent, agent_session_id) {
        (Agent::Claude, Some(id)) => {
            let program = executable::find(CLAUDE_COMMAND).ok_or(CreateError::AgentNotFound)?;
            let program = program.to_string_lossy().into_owned();
            (
                Some(program),
                vec!["--session-id".to_owned(), id.to_owned()],
            )
        }
        _ => (None, Vec::new()),
    };
    Ok(Launch {
        program,
        args,
        cwd: cwd.to_path_buf(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rejects_missing_folder() {
        let sessions = Sessions::default();
        let result = sessions.create(CreateRequest {
            agent: Agent::Shell,
            cwd: "/definitely/not/here".into(),
            name: None,
        });
        assert!(matches!(result, Err(CreateError::InvalidCwd(_))));
    }

    #[cfg(unix)]
    #[test]
    fn creates_shell_session() {
        let sessions = Sessions::default();
        let meta = sessions
            .create(CreateRequest {
                agent: Agent::Shell,
                cwd: std::env::temp_dir().display().to_string(),
                name: Some("build".into()),
            })
            .expect("create");
        assert_eq!(meta.name, "build");
        assert_eq!(meta.agent_session_id, None);
        assert!(sessions.get(&meta.id).is_some());
        assert_eq!(sessions.list().len(), 1);
    }
}
