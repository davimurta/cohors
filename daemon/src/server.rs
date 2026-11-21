use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};

use crate::auth::{origin_allowed, token_matches};
use crate::protocol::{ClientMessage, DAEMON_VERSION, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use crate::sessions::{CreateRequest, Sessions, claude_available};

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const OUTGOING_QUEUE: usize = 256;
const EVENT_QUEUE: usize = 256;

type Sink = SplitSink<WebSocketStream<TcpStream>, Message>;
type Source = SplitStream<WebSocketStream<TcpStream>>;
type Outgoing = mpsc::Sender<ServerMessage>;

pub struct DaemonState {
    token: String,
    sessions: Sessions,
    /// Changes every connected client must see, such as a new session.
    events: broadcast::Sender<ServerMessage>,
}

impl DaemonState {
    pub fn new(token: String) -> Self {
        let (events, _) = broadcast::channel(EVENT_QUEUE);
        Self {
            token,
            sessions: Sessions::default(),
            events,
        }
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
    if !handshake(&mut sink, &mut source, &state.token).await {
        return;
    }

    let (outgoing, mut queue) = mpsc::channel(OUTGOING_QUEUE);
    let mut events = state.events.subscribe();
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
async fn handshake(sink: &mut Sink, source: &mut Source, token: &str) -> bool {
    let first = tokio::time::timeout(HELLO_TIMEOUT, source.next()).await;
    let Ok(Some(Ok(Message::Text(text)))) = first else {
        return false;
    };
    let reply = match serde_json::from_str::<ClientMessage>(text.as_str()) {
        Ok(ClientMessage::Hello {
            token: given,
            protocol_version,
        }) => {
            if !token_matches(token, &given.0) {
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
                    claude_available: claude_available(),
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
        ClientMessage::SessionCreate { agent, cwd, name } => {
            create_session(state, CreateRequest { agent, cwd, name })
        }
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
    };
    if let Some(reply) = reply {
        let _ = outgoing.send(reply).await;
    }
}

/// Announces the new session to every client; only failures go back to the
/// requester, with `ref` naming the request.
fn create_session(state: &DaemonState, request: CreateRequest) -> Option<ServerMessage> {
    match state.sessions.create(request) {
        Ok(session) => {
            let _ = state.events.send(ServerMessage::SessionCreated { session });
            None
        }
        Err(error) => Some(ServerMessage::error(
            error.code(),
            error.to_string(),
            Some("session.create".into()),
        )),
    }
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
