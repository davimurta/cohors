//! Claude Code hooks that Cohors adds to its Claude sessions through
//! `--settings`. Each hook runs this binary again with a flag:
//!
//! - `SessionStart` prints the project context (see [`crate::context`]).
//! - `Stop` and `Notification` mark the session as waiting for the user,
//!   and `UserPromptSubmit` clears the mark. The hook writes the state to a
//!   file in the session folder, which the daemon picks up and removes.
//!
//! Hooks inherit the session's environment, so they find their session
//! through [`SESSION_ENV`].

use std::io;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::context;
use crate::private_fs;
use crate::protocol::Attention;

/// `cohors-daemon --attention <done|input|clear>`, run by the hooks.
pub const ATTENTION_FLAG: &str = "--attention";
/// The Cohors session id, set in every session process.
pub const SESSION_ENV: &str = "COHORS_SESSION_ID";
/// Written by a hook in the session folder, read and removed by the daemon.
pub const ATTENTION_FILE: &str = "attention";

/// What a hook reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Waiting(Attention),
    /// The user sent a prompt (`UserPromptSubmit`).
    Clear,
}

impl Signal {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "done" => Some(Self::Waiting(Attention::Done)),
            "input" => Some(Self::Waiting(Attention::Input)),
            "clear" => Some(Self::Clear),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Waiting(Attention::Done) => "done",
            Self::Waiting(Attention::Input) => "input",
            Self::Clear => "clear",
        }
    }
}

/// Arguments for a Claude session: the hooks, plus the context folder when
/// the session has one. `program` is the daemon binary the hooks run.
pub fn claude_args(program: &Path, context_dir: Option<&Path>) -> Vec<String> {
    let program = shell_quote(&program.display().to_string());
    let attention =
        |signal: Signal| hook(format!("{program} {ATTENTION_FLAG} {}", signal.as_str()));
    let mut hooks = serde_json::json!({
        "Stop": [attention(Signal::Waiting(Attention::Done))],
        "Notification": [attention(Signal::Waiting(Attention::Input))],
        "UserPromptSubmit": [attention(Signal::Clear)],
    });
    let mut args = Vec::new();
    if let Some(dir) = context_dir {
        let dir_text = dir.display().to_string();
        hooks["SessionStart"] = serde_json::json!([hook(format!(
            "{program} {} {}",
            context::PRINT_FLAG,
            shell_quote(&dir_text)
        ))]);
        args.extend(["--add-dir".to_owned(), dir_text]);
    }
    let settings = serde_json::json!({ "hooks": hooks });
    args.extend(["--settings".to_owned(), settings.to_string()]);
    args
}

/// Hook entry point. Never fails the hook: Claude would show the error in
/// the session. Prints nothing, since `UserPromptSubmit` output would be
/// added to the conversation.
pub fn report(sessions_dir: Option<PathBuf>, value: &str) {
    let result = match (
        sessions_dir,
        std::env::var(SESSION_ENV),
        Signal::parse(value),
    ) {
        (Some(dir), Ok(id), Some(signal)) => write_signal(&dir, &id, signal),
        _ => return,
    };
    if let Err(error) = result {
        eprintln!("failed to report {value}: {error}");
    }
}

/// Only an existing session folder is written to, named by a valid id.
pub fn write_signal(sessions_dir: &Path, id: &str, signal: Signal) -> io::Result<()> {
    if Uuid::parse_str(id).is_err() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid session id",
        ));
    }
    let dir = sessions_dir.join(id);
    if !dir.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "unknown session"));
    }
    private_fs::write_atomic(&dir.join(ATTENTION_FILE), signal.as_str().as_bytes())
}

fn hook(command: String) -> serde_json::Value {
    serde_json::json!({ "hooks": [{ "type": "command", "command": command }] })
}

/// Quotes a value for a POSIX shell. Claude Code runs hook commands with a
/// POSIX shell on every platform (Git Bash on Windows).
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command<'a>(settings: &'a serde_json::Value, event: &str) -> &'a serde_json::Value {
        &settings["hooks"][event][0]["hooks"][0]["command"]
    }

    #[test]
    fn hooks_run_this_binary() {
        let args = claude_args(
            Path::new("/Apps/Co hors/cohors-daemon"),
            Some(Path::new("/data/it's")),
        );
        assert_eq!(args[..3], ["--add-dir", "/data/it's", "--settings"]);
        let settings: serde_json::Value = serde_json::from_str(&args[3]).expect("json");
        assert_eq!(
            command(&settings, "SessionStart"),
            "'/Apps/Co hors/cohors-daemon' --print-context '/data/it'\\''s'"
        );
        assert_eq!(
            command(&settings, "Stop"),
            "'/Apps/Co hors/cohors-daemon' --attention done"
        );
        assert_eq!(
            command(&settings, "Notification"),
            "'/Apps/Co hors/cohors-daemon' --attention input"
        );
        assert_eq!(
            command(&settings, "UserPromptSubmit"),
            "'/Apps/Co hors/cohors-daemon' --attention clear"
        );
    }

    #[test]
    fn without_context_only_attention_hooks() {
        let args = claude_args(Path::new("/bin/cohors-daemon"), None);
        assert_eq!(args[0], "--settings");
        let settings: serde_json::Value = serde_json::from_str(&args[1]).expect("json");
        assert!(settings["hooks"]["SessionStart"].is_null());
        assert!(command(&settings, "Stop").is_string());
    }

    #[test]
    fn signals_round_trip() {
        for value in ["done", "input", "clear"] {
            let signal = Signal::parse(value).expect("known");
            assert_eq!(signal.as_str(), value);
        }
        assert_eq!(Signal::parse("other"), None);
    }

    #[test]
    fn writes_only_to_known_sessions() {
        let root = std::env::temp_dir().join(format!("cohors-hooks-{}", Uuid::new_v4()));
        let id = Uuid::new_v4().to_string();
        std::fs::create_dir_all(root.join(&id)).expect("dir");
        write_signal(&root, &id, Signal::Waiting(Attention::Done)).expect("write");
        let written = std::fs::read_to_string(root.join(&id).join(ATTENTION_FILE)).expect("read");
        assert_eq!(written, "done");
        assert!(write_signal(&root, "../escape", Signal::Clear).is_err());
        assert!(write_signal(&root, &Uuid::new_v4().to_string(), Signal::Clear).is_err());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
