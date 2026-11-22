use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Notify, mpsc};
use tokio::task::{AbortHandle, JoinHandle};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};

use crate::accounts;
use crate::auth::{origin_allowed, token_matches};
use crate::context::ContextFiles;
use crate::projects::{Projects, ProjectsError};
use crate::protocol::SummaryStatus;
use crate::protocol::{
    Agent, ClientMessage, DAEMON_VERSION, ErrorCode, PROTOCOL_VERSION, ServerMessage,
};
use crate::sessions::{CreateRequest, Sessions};
use crate::summary;

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const OUTGOING_QUEUE: usize = 256;

type Sink = SplitSink<WebSocketStream<TcpStream>, Message>;
type Source = SplitStream<WebSocketStream<TcpStream>>;
type Outgoing = mpsc::Sender<ServerMessage>;

pub struct DaemonState {
    token: String,
    sessions: Arc<Sessions>,
    projects: Projects,
    home: PathBuf,
    shutdown_requested: Notify,
    /// Finish and summary tasks per session, so "finish without summary"
    /// can cancel them.
    summaries: Mutex<HashMap<String, AbortHandle>>,
}

impl DaemonState {
    pub fn new(token: String, sessions: Arc<Sessions>, projects: Projects, home: PathBuf) -> Self {
        Self {
            token,
            sessions,
            projects,
            home,
            shutdown_requested: Notify::new(),
            summaries: Mutex::new(HashMap::new()),
        }
    }

    /// Resolves when a client sends `daemon.shutdown`.
    pub async fn shutdown_requested(&self) {
        self.shutdown_requested.notified().await;
    }
}

pub async fn serve(listener: TcpListener, state: Arc<DaemonState>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle_connection(stream, Arc::clone(&state)));
            }
            Err(error) => eprintln!("failed to accept connection: {error}"),
        }
    }
}

async fn handle_connection(stream: TcpStream, state: Arc<DaemonState>) {
    let socket = match accept_hdr_async(stream, check_origin).await {
        Ok(socket) => socket,
        Err(error) => {
            eprintln!("rejected connection: {error}");
            return;
        }
    };
    let (mut sink, mut source) = socket.split();
    if !handshake(&mut sink, &mut source, &state).await {
        return;
    }

    let (outgoing, mut queue) = mpsc::channel(OUTGOING_QUEUE);
    let mut events = state.sessions.subscribe();
    let mut attachments: HashMap<String, JoinHandle<()>> = HashMap::new();
    loop {
        tokio::select! {
            incoming = source.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    handle_message(text.as_str(), &state, &outgoing, &mut attachments).await;
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            Some(message) = queue.recv() => {
                if send(&mut sink, &message).await.is_err() {
                    break;
                }
            }
            event = events.recv() => {
                let message = match event {
                    Ok(message) => message,
                    // Missed events are replaced by the full current list.
                    Err(RecvError::Lagged(_)) => ServerMessage::SessionListResult {
                        sessions: state.sessions.list(),
                    },
                    Err(RecvError::Closed) => break,
                };
                if send(&mut sink, &message).await.is_err() {
                    break;
                }
            }
        }
    }
    for task in attachments.into_values() {
        task.abort();
    }
}

// The signature is dictated by the tungstenite handshake callback.
#[allow(clippy::result_large_err)]
fn check_origin(request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    let origin = request
        .headers()
        .get("origin")
        .and_then(|value| value.to_str().ok());
    if origin_allowed(origin) {
        return Ok(response);
    }
    let mut rejection = ErrorResponse::new(Some("origin not allowed".into()));
    *rejection.status_mut() = StatusCode::FORBIDDEN;
    Err(rejection)
}

/// The first message must be a valid hello, otherwise the connection is closed.
async fn handshake(sink: &mut Sink, source: &mut Source, state: &DaemonState) -> bool {
    let first = tokio::time::timeout(HELLO_TIMEOUT, source.next()).await;
    let Ok(Some(Ok(Message::Text(text)))) = first else {
        return false;
    };
    let reply = match serde_json::from_str::<ClientMessage>(text.as_str()) {
        Ok(ClientMessage::Hello {
            token: given,
            protocol_version,
        }) => {
            if !token_matches(&state.token, &given.0) {
                ServerMessage::error(ErrorCode::Unauthorized, "invalid token", None)
            } else if protocol_version != PROTOCOL_VERSION {
                ServerMessage::error(
                    ErrorCode::UnsupportedProtocol,
                    format!("daemon speaks protocol {PROTOCOL_VERSION}"),
                    None,
                )
            } else {
                ServerMessage::Ready {
                    protocol_version: PROTOCOL_VERSION,
                    daemon_version: DAEMON_VERSION,
                    claude_available: state.sessions.claude_available(),
                }
            }
        }
        _ => ServerMessage::error(ErrorCode::Unauthorized, "expected hello", None),
    };
    let accepted = matches!(reply, ServerMessage::Ready { .. });
    let sent = send(sink, &reply).await.is_ok();
    if !accepted {
        let _ = sink.close().await;
    }
    accepted && sent
}

async fn handle_message(
    text: &str,
    state: &Arc<DaemonState>,
    outgoing: &Outgoing,
    attachments: &mut HashMap<String, JoinHandle<()>>,
) {
    let message = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => message,
        Err(error) => {
            let reply = ServerMessage::error(ErrorCode::BadRequest, error.to_string(), None);
            let _ = outgoing.send(reply).await;
            return;
        }
    };
    let reply = match message {
        ClientMessage::Hello { .. } => Some(ServerMessage::error(
            ErrorCode::BadRequest,
            "already authenticated",
            None,
        )),
        ClientMessage::SessionList => Some(ServerMessage::SessionListResult {
            sessions: state.sessions.list(),
        }),
        ClientMessage::SessionCreate {
            agent,
            cwd,
            name,
            project_id,
        } => create_session(state, agent, cwd, name, project_id),
        ClientMessage::SessionResume { id } => state
            .sessions
            .resume(&id)
            .err()
            .map(|error| request_error(error.code(), &error, &id)),
        ClientMessage::SessionAttach { id } => {
            if state.sessions.get(&id).is_some() {
                let task = tokio::spawn(forward_output(
                    Arc::clone(state),
                    id.clone(),
                    outgoing.clone(),
                ));
                if let Some(previous) = attachments.insert(id, task) {
                    previous.abort();
                }
                None
            } else {
                Some(session_not_found(id))
            }
        }
        ClientMessage::SessionDetach { id } => {
            if let Some(task) = attachments.remove(&id) {
                task.abort();
            }
            None
        }
        ClientMessage::SessionInput { id, data } => match state.sessions.get(&id) {
            Some(entry) => internal_error(entry.session.write_input(&data), id),
            None => Some(session_not_found(id)),
        },
        ClientMessage::SessionResize { id, cols, rows } => match state.sessions.get(&id) {
            Some(entry) => internal_error(entry.session.resize(cols, rows), id),
            None => Some(session_not_found(id)),
        },
        ClientMessage::SessionFinish { id, with_summary } => {
            match state.sessions.finish(&id, with_summary) {
                Ok(meta) => {
                    let summarize = meta.summary_status == SummaryStatus::Pending;
                    spawn_finish(state, id, true, summarize);
                    None
                }
                Err(error) => Some(request_error(error.code(), &error, &id)),
            }
        }
        ClientMessage::SessionSummarize { id } => match state.sessions.retry_summary(&id) {
            Ok(_) => {
                spawn_finish(state, id, false, true);
                None
            }
            Err(error) => Some(request_error(error.code(), &error, &id)),
        },
        ClientMessage::SessionSkipSummary { id } => {
            if let Ok(mut summaries) = state.summaries.lock()
                && let Some(task) = summaries.remove(&id)
            {
                task.abort();
            }
            state
                .sessions
                .skip_summary(&id)
                .err()
                .map(|error| request_error(error.code(), &error, &id))
        }
        ClientMessage::SessionGetSummary { id } => Some(
            state
                .sessions
                .summary(&id)
                .unwrap_or_else(|error| request_error(error.code(), &error, &id)),
        ),
        ClientMessage::SessionNotes { id, notes } => state
            .sessions
            .set_notes(&id, notes)
            .err()
            .map(|error| request_error(error.code(), &error, &id)),
        ClientMessage::SessionRename { id, name } => state
            .sessions
            .rename(&id, &name)
            .err()
            .map(|error| request_error(error.code(), &error, &id)),
        ClientMessage::SessionTranscript { id } => Some(
            state
                .sessions
                .transcript(&id)
                .unwrap_or_else(|error| request_error(error.code(), &error, &id)),
        ),
        ClientMessage::SessionDelete { id } => state
            .sessions
            .delete(&id)
            .err()
            .map(|error| request_error(error.code(), &error, &id)),
        ClientMessage::DaemonShutdown => {
            // notify_one keeps the request even if main is not waiting yet.
            state.shutdown_requested.notify_one();
            None
        }
        ClientMessage::AccountsList => Some(ServerMessage::AccountsListResult {
            claude_accounts: accounts::claude_accounts(&state.home),
            ssh_keys: accounts::ssh_keys(&state.home),
        }),
        ClientMessage::ProjectList => Some(ServerMessage::ProjectListResult {
            projects: state.projects.list(),
        }),
        ClientMessage::ProjectCreate { project } => {
            project_changed(state, state.projects.create(project), "project.create")
        }
        ClientMessage::ProjectUpdate { id, project } => {
            project_changed(state, state.projects.update(&id, project), &id)
        }
        ClientMessage::ProjectDelete { id } => {
            project_changed(state, state.projects.delete(&id), &id)
        }
        ClientMessage::ProjectContextGet { id } => {
            Some(state.projects.read_context(&id).map_or_else(
                |error| request_error(error.code(), &error, &id),
                |(dir, files)| context_result(id.clone(), &dir, files),
            ))
        }
        ClientMessage::ProjectContextSave { id, rules, plan } => {
            let files = ContextFiles { rules, plan };
            Some(match state.projects.save_context(&id, &files) {
                Ok(dir) => context_result(id, &dir, files),
                Err(error) => request_error(error.code(), &error, &id),
            })
        }
    };
    if let Some(reply) = reply {
        let _ = outgoing.send(reply).await;
    }
}

/// Stops the session's process (when `stop` is set) and then generates its
/// summary (when `summarize` is set), off the connection's task.
fn spawn_finish(state: &Arc<DaemonState>, id: String, stop: bool, summarize: bool) {
    let task_state = Arc::clone(state);
    let task_id = id.clone();
    let task = tokio::spawn(async move {
        if stop {
            let sessions = Arc::clone(&task_state.sessions);
            let stop_id = task_id.clone();
            if let Err(error) =
                tokio::task::spawn_blocking(move || sessions.stop_process(&stop_id)).await
            {
                eprintln!("failed to stop session {task_id}: {error}");
            }
        }
        if summarize {
            let result = match task_state.sessions.summary_launch(&task_id) {
                Ok(launch) => summary::generate(&launch)
                    .await
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            task_state.sessions.summary_finished(&task_id, result);
        }
        if let Ok(mut summaries) = task_state.summaries.lock() {
            summaries.remove(&task_id);
        }
    });
    if let Ok(mut summaries) = state.summaries.lock() {
        summaries.insert(id, task.abort_handle());
    }
}

fn create_session(
    state: &DaemonState,
    agent: Agent,
    cwd: String,
    name: Option<String>,
    project_id: Option<String>,
) -> Option<ServerMessage> {
    const REFERENCE: &str = "session.create";
    let project = match project_id {
        Some(id) => match state.projects.get(&id) {
            Some(project) => Some(project),
            None => {
                return Some(ServerMessage::error(
                    ErrorCode::ProjectNotFound,
                    "project not found",
                    Some(REFERENCE.into()),
                ));
            }
        },
        None => None,
    };
    let context_dir = match &project {
        Some(project) => match state.projects.ensure_context_dir(project) {
            Ok(dir) => Some(dir.display().to_string()),
            Err(error) => return Some(request_error(ErrorCode::Internal, &error, REFERENCE)),
        },
        None => None,
    };
    let request = CreateRequest {
        agent,
        cwd,
        name,
        project,
        context_dir,
    };
    state
        .sessions
        .create(request)
        .err()
        .map(|error| request_error(error.code(), &error, REFERENCE))
}

fn context_result(id: String, dir: &Path, files: ContextFiles) -> ServerMessage {
    ServerMessage::ProjectContextResult {
        id,
        dir: dir.display().to_string(),
        rules: files.rules,
        plan: files.plan,
    }
}

/// After a project change every client gets the full list.
fn project_changed<T>(
    state: &DaemonState,
    result: Result<T, ProjectsError>,
    reference: &str,
) -> Option<ServerMessage> {
    match result {
        Ok(_) => {
            state.sessions.announce_projects(state.projects.list());
            None
        }
        Err(error) => Some(request_error(error.code(), &error, reference)),
    }
}

/// Successes reach every client as events; only failures go back to the
/// requester, with `ref` naming the session or request.
fn request_error(
    code: ErrorCode,
    error: &impl std::fmt::Display,
    reference: &str,
) -> ServerMessage {
    ServerMessage::error(code, error.to_string(), Some(reference.to_owned()))
}

fn internal_error<E: std::fmt::Display>(
    result: Result<(), E>,
    id: String,
) -> Option<ServerMessage> {
    result
        .err()
        .map(|error| ServerMessage::error(ErrorCode::Internal, error.to_string(), Some(id)))
}

fn session_not_found(id: String) -> ServerMessage {
    ServerMessage::error(ErrorCode::SessionNotFound, "session not found", Some(id))
}

/// Sends a snapshot, then live output. A client that falls behind the live
/// channel gets a fresh snapshot instead of a gap.
async fn forward_output(state: Arc<DaemonState>, id: String, outgoing: Outgoing) {
    let Some(entry) = state.sessions.get(&id) else {
        return;
    };
    loop {
        let mut attachment = match entry.session.attach() {
            Ok(attachment) => attachment,
            Err(error) => {
                let reply = ServerMessage::error(ErrorCode::Internal, error.to_string(), Some(id));
                let _ = outgoing.send(reply).await;
                return;
            }
        };
        let snapshot = ServerMessage::SessionSnapshot {
            id: id.clone(),
            seq: attachment.snapshot.seq,
            data: attachment.snapshot.data,
        };
        if outgoing.send(snapshot).await.is_err() {
            return;
        }
        loop {
            match attachment.live.recv().await {
                Ok(chunk) => {
                    let output = ServerMessage::SessionOutput {
                        id: id.clone(),
                        seq: chunk.seq,
                        data: chunk.data,
                    };
                    if outgoing.send(output).await.is_err() {
                        return;
                    }
                }
                Err(RecvError::Lagged(_)) => break,
                Err(RecvError::Closed) => return,
            }
        }
    }
}

async fn send(sink: &mut Sink, message: &ServerMessage) -> Result<(), tungstenite::Error> {
    let json = serde_json::to_string(message).map_err(|error| {
        tungstenite::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })?;
    sink.send(Message::text(json)).await
}
