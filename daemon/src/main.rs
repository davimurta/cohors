use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;
use std::process::ExitCode;
use std::sync::Arc;

use cohors_daemon::auth::generate_token;
use cohors_daemon::daemon_file::{self, DaemonFile};
use cohors_daemon::server::{self, DaemonState};
use tokio::net::TcpListener;

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

    let state = Arc::new(DaemonState::new(token));
    tokio::select! {
        () = server::serve(listener, state) => {}
        signal = tokio::signal::ctrl_c() => signal?,
    }
    fs::remove_file(daemon_file)?;
    Ok(())
}
