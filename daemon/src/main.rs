use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use cohors_daemon::auth::generate_token;
use cohors_daemon::daemon_file::{self, DaemonFile};
use cohors_daemon::protocol::{Agent, SessionInfo};
use cohors_daemon::server::{self, DaemonState, SessionEntry};
use cohors_daemon::session::{Launch, Session, SessionError};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cohors-daemon: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let data_dir = daemon_file::data_dir().ok_or("no user data directory")?;
    let cwd = std::env::current_dir()?;
    let entry = spawn_initial_session(&cwd)?;
    eprintln!(
        "started {:?} session in {}",
        entry.info.agent, entry.info.cwd
    );
    let sessions = HashMap::from([(entry.info.id.clone(), entry)]);

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let token = generate_token()?;
    let daemon_file = daemon_file::write(
        &data_dir,
        &DaemonFile {
            port,
            token: token.clone(),
        },
    )?;
    eprintln!("listening on 127.0.0.1:{port}");

    let state = Arc::new(DaemonState { token, sessions });
    tokio::select! {
        () = server::serve(listener, state) => {}
        signal = tokio::signal::ctrl_c() => signal?,
    }
    fs::remove_file(daemon_file)?;
    Ok(())
}

/// Runs Claude Code when it is installed, otherwise the user's shell.
fn spawn_initial_session(cwd: &Path) -> Result<SessionEntry, SessionError> {
    let launch = |program: Option<&str>| Launch {
        program: program.map(str::to_owned),
        args: Vec::new(),
        cwd: cwd.to_path_buf(),
    };
    let (agent, session) = match Session::spawn(&launch(Some("claude"))) {
        Ok(session) => (Agent::Claude, session),
        Err(_) => (Agent::Shell, Session::spawn(&launch(None))?),
    };
    Ok(SessionEntry {
        info: SessionInfo {
            id: Uuid::new_v4().to_string(),
            agent,
            cwd: cwd.display().to_string(),
        },
        session,
    })
}
