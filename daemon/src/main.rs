use std::error::Error;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cohors_daemon::auth::generate_token;
use cohors_daemon::daemon_file::{self, DaemonFile};
use cohors_daemon::projects::Projects;
use cohors_daemon::server::{self, DaemonState};
use cohors_daemon::sessions::Sessions;
use cohors_daemon::store::SessionStore;
use cohors_daemon::{context, control, instance, login_path};
use tokio::net::TcpListener;

const USAGE: &str = "usage: cohors-daemon [--detach | --stop | --print-context <dir>]";
/// How often sessions are checked for idleness.
const IDLE_CHECK: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, dir] = args.as_slice()
        && flag == context::PRINT_FLAG
    {
        context::print(Path::new(dir));
        return ExitCode::SUCCESS;
    }
    let result = data_dir().and_then(|dir| match args.as_slice() {
        [] => run(&dir),
        [flag] if flag == "--detach" => control::detach(&dir),
        [flag] if flag == "--stop" => control::stop(&dir),
        _ => Err(USAGE.into()),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cohors-daemon: {error}");
            ExitCode::FAILURE
        }
    }
}

fn data_dir() -> Result<PathBuf, Box<dyn Error>> {
    Ok(daemon_file::data_dir().ok_or("no user data directory")?)
}

fn run(data_dir: &Path) -> Result<(), Box<dyn Error>> {
    // Held until the process exits.
    let Some(_lock) = instance::acquire(data_dir)? else {
        eprintln!("cohors-daemon: already running");
        return Ok(());
    };
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve(data_dir))
}

async fn serve(data_dir: &Path) -> Result<(), Box<dyn Error>> {
    let path = login_path::resolve(std::env::var_os("PATH")).await;
    let sessions = Sessions::load(SessionStore::new(data_dir.join("sessions")), path);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let token = generate_token()?;
    daemon_file::write(
        data_dir,
        &DaemonFile {
            port,
            token: token.clone(),
            pid: std::process::id(),
        },
    )?;
    eprintln!("listening on 127.0.0.1:{port}");

    let idle_sessions = Arc::clone(&sessions);
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(IDLE_CHECK);
        loop {
            ticks.tick().await;
            idle_sessions.refresh_idle(Instant::now());
        }
    });

    let home = dirs::home_dir().ok_or("no home directory")?;
    let projects = Projects::load(data_dir.join("projects.json"), home.clone());
    let state = Arc::new(DaemonState::new(
        token,
        Arc::clone(&sessions),
        projects,
        home,
    ));
    tokio::select! {
        () = server::serve(listener, Arc::clone(&state)) => {}
        () = state.shutdown_requested() => eprintln!("shutdown requested"),
        result = termination_signal() => result?,
    }
    // Clients stop finding the daemon before its sessions go away.
    daemon_file::remove(data_dir)?;
    tokio::task::spawn_blocking(move || sessions.shutdown()).await?;
    Ok(())
}

#[cfg(unix)]
async fn termination_signal() -> std::io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result,
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(windows)]
async fn termination_signal() -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}
