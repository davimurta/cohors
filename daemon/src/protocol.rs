use std::fmt;

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Client token. Debug output is redacted so it can never reach logs.
#[derive(Deserialize, PartialEq)]
#[serde(transparent)]
pub struct Token(pub String);

impl fmt::Debug for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Token([redacted])")
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ClientMessage {
    #[serde(rename = "hello")]
    Hello { token: Token, protocol_version: u32 },
    #[serde(rename = "session.list")]
    SessionList,
    #[serde(rename = "session.create")]
    SessionCreate {
        agent: Agent,
        cwd: String,
        name: Option<String>,
    },
    #[serde(rename = "session.attach")]
    SessionAttach { id: String },
    #[serde(rename = "session.resume")]
    SessionResume { id: String },
    #[serde(rename = "session.detach")]
    SessionDetach { id: String },
    #[serde(rename = "session.input")]
    SessionInput { id: String, data: String },
    #[serde(rename = "session.resize")]
    SessionResize { id: String, cols: u16, rows: u16 },
    #[serde(rename = "daemon.shutdown")]
    DaemonShutdown,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Shell,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    /// The process ended on its own.
    Exited,
    /// The daemon lost the process, e.g. after a reboot or crash.
    Interrupted,
}

/// Also the on-disk format of `meta.json`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub agent: Agent,
    pub cwd: String,
    pub agent_session_id: Option<String>,
    pub status: SessionStatus,
    pub exit_code: Option<u32>,
    pub created_at: String,
    pub ended_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    UnsupportedProtocol,
    BadRequest,
    SessionNotFound,
    InvalidCwd,
    AgentNotFound,
    InvalidState,
    Internal,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    #[serde(rename = "ready")]
    Ready {
        protocol_version: u32,
        daemon_version: &'static str,
        claude_available: bool,
    },
    #[serde(rename = "session.list.result")]
    SessionListResult { sessions: Vec<SessionMeta> },
    #[serde(rename = "session.created")]
    SessionCreated { session: SessionMeta },
    #[serde(rename = "session.updated")]
    SessionUpdated { session: SessionMeta },
    #[serde(rename = "session.snapshot")]
    SessionSnapshot { id: String, seq: u64, data: String },
    #[serde(rename = "session.output")]
    SessionOutput { id: String, seq: u64, data: String },
    #[serde(rename = "error")]
    Error {
        code: ErrorCode,
        message: String,
        #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
}

impl ServerMessage {
    pub fn error(code: ErrorCode, message: impl Into<String>, reference: Option<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
            reference,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(value: serde_json::Value) -> ClientMessage {
        serde_json::from_value(value).expect("valid client message")
    }

    #[test]
    fn parses_hello() {
        let message = parse(json!({"type": "hello", "token": "abc", "protocolVersion": 1}));
        assert_eq!(
            message,
            ClientMessage::Hello {
                token: Token("abc".into()),
                protocol_version: 1
            }
        );
    }

    #[test]
    fn parses_resize() {
        let message = parse(json!({"type": "session.resize", "id": "s", "cols": 120, "rows": 40}));
        assert_eq!(
            message,
            ClientMessage::SessionResize {
                id: "s".into(),
                cols: 120,
                rows: 40
            }
        );
    }

    #[test]
    fn parses_create_with_optional_name() {
        let message = parse(json!({"type": "session.create", "agent": "shell", "cwd": "/tmp"}));
        assert_eq!(
            message,
            ClientMessage::SessionCreate {
                agent: Agent::Shell,
                cwd: "/tmp".into(),
                name: None
            }
        );
    }

    #[test]
    fn serializes_session_meta_in_camel_case() {
        let meta = SessionMeta {
            id: "s".into(),
            name: "cohors".into(),
            agent: Agent::Claude,
            cwd: "/dev/cohors".into(),
            agent_session_id: Some("a".into()),
            status: SessionStatus::Running,
            exit_code: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            ended_at: None,
        };
        let value = serde_json::to_value(ServerMessage::SessionCreated { session: meta })
            .expect("serialize");
        assert_eq!(
            value,
            json!({
                "type": "session.created",
                "session": {
                    "id": "s",
                    "name": "cohors",
                    "agent": "claude",
                    "cwd": "/dev/cohors",
                    "agentSessionId": "a",
                    "status": "running",
                    "exitCode": null,
                    "createdAt": "2026-01-01T00:00:00Z",
                    "endedAt": null
                }
            })
        );
    }

    #[test]
    fn rejects_unknown_type() {
        let result = serde_json::from_value::<ClientMessage>(json!({"type": "nope"}));
        assert!(result.is_err());
    }

    #[test]
    fn token_debug_is_redacted() {
        let message = parse(json!({"type": "hello", "token": "secret", "protocolVersion": 1}));
        assert!(!format!("{message:?}").contains("secret"));
    }

    #[test]
    fn serializes_ready() {
        let value = serde_json::to_value(ServerMessage::Ready {
            protocol_version: 1,
            daemon_version: "0.1.0",
            claude_available: true,
        })
        .expect("serialize");
        assert_eq!(
            value,
            json!({
                "type": "ready",
                "protocolVersion": 1,
                "daemonVersion": "0.1.0",
                "claudeAvailable": true
            })
        );
    }

    #[test]
    fn serializes_error_with_ref() {
        let message = ServerMessage::error(ErrorCode::SessionNotFound, "missing", Some("s".into()));
        let value = serde_json::to_value(message).expect("serialize");
        assert_eq!(
            value,
            json!({"type": "error", "code": "session_not_found", "message": "missing", "ref": "s"})
        );
    }

    #[test]
    fn serializes_output() {
        let message = ServerMessage::SessionOutput {
            id: "s".into(),
            seq: 7,
            data: "hi".into(),
        };
        let value = serde_json::to_value(message).expect("serialize");
        assert_eq!(
            value,
            json!({"type": "session.output", "id": "s", "seq": 7, "data": "hi"})
        );
    }
}
