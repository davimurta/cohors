//! Session summaries come from Claude Code itself, run once in
//! non-interactive mode (`claude -p --resume <id>`), which prints clean text
//! instead of a redrawn terminal screen.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::session::{INHERITED_AGENT_MARKERS, Launch};

/// The history screen renders these three sections.
pub const PROMPT: &str = "Resuma esta sessão de trabalho em português do Brasil, em Markdown, \
com exatamente três seções de nível 2, nesta ordem: \"## O que foi feito\", \"## Decisões\" e \
\"## Pendente\". Use listas com \"-\" e frases curtas. Não use ferramentas e não escreva \
introdução nem conclusão. Se uma seção não tiver itens, escreva \"- Nada.\"";

const TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Enough of stderr to explain a failure without flooding the UI.
const ERROR_CHARS: usize = 500;

#[derive(Debug, thiserror::Error)]
pub enum SummaryError {
    #[error("claude did not finish within {} minutes", TIMEOUT.as_secs() / 60)]
    Timeout,
    #[error("could not run claude: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("claude failed: {0}")]
    Failed(String),
    #[error("claude returned an empty summary")]
    Empty,
}

pub async fn generate(launch: &Launch) -> Result<String, SummaryError> {
    let output = tokio::time::timeout(TIMEOUT, command(launch).output())
        .await
        .map_err(|_| SummaryError::Timeout)??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let reason = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };
        return Err(SummaryError::Failed(
            reason.trim().chars().take(ERROR_CHARS).collect(),
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if text.is_empty() {
        return Err(SummaryError::Empty);
    }
    Ok(text)
}

fn command(launch: &Launch) -> Command {
    let program = launch.program.as_deref().unwrap_or("claude");
    let mut command = Command::new(program);
    command
        .args(&launch.args)
        .current_dir(&launch.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Dropping the future (timeout or "finish without summary") kills claude.
        .kill_on_drop(true);
    if let Some(path) = &launch.path {
        command.env("PATH", path);
    }
    for marker in INHERITED_AGENT_MARKERS {
        command.env_remove(marker);
    }
    for (key, value) in &launch.env {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    command
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Launch {
        Launch {
            program: Some("/bin/sh".into()),
            args: vec!["-c".into(), script.into()],
            cwd: std::env::temp_dir(),
            path: None,
            env: vec![("COHORS_TEST".into(), Some("ok".into()))],
        }
    }

    #[tokio::test]
    async fn returns_trimmed_stdout() {
        let text = generate(&sh("printf \"  ## Feito $COHORS_TEST\\n\"")).await;
        assert_eq!(text.expect("summary"), "## Feito ok");
    }

    #[tokio::test]
    async fn reports_failure_with_stderr() {
        let error = generate(&sh("echo 'No conversation found' >&2; exit 1")).await;
        let Err(SummaryError::Failed(reason)) = error else {
            panic!("expected a failure");
        };
        assert_eq!(reason, "No conversation found");
    }

    #[tokio::test]
    async fn empty_output_is_an_error() {
        assert!(matches!(
            generate(&sh("true")).await,
            Err(SummaryError::Empty)
        ));
    }
}
