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
    #[serde(rename = "session.attach")]
    SessionAttach { id: String },
    #[serde(rename = "session.detach")]
    SessionDetach { id: String },
    #[serde(rename = "session.input")]
    SessionInput { id: String, data: String },
    #[serde(rename = "session.resize")]
    SessionResize { id: String, cols: u16, rows: u16 },
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Shell,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub agent: Agent,
    pub cwd: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    UnsupportedProtocol,
    BadRequest,
    SessionNotFound,
    Internal,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    #[serde(rename = "ready")]
    Ready {
        protocol_version: u32,
        daemon_version: &'static str,
    },
    #[serde(rename = "session.list.result")]
    SessionListResult { sessions: Vec<SessionInfo> },
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
        })
        .expect("serialize");
        assert_eq!(
            value,
            json!({"type": "ready", "protocolVersion": 1, "daemonVersion": "0.1.0"})
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
