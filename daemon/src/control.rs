//! Command-line modes used by the app: start a background daemon, or stop
//! the running one.

use std::error::Error;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::{daemon_file, instance, private_fs, process};

const LOG_FILE: &str = "daemon.log";
const START_TIMEOUT: Duration = Duration::from_secs(10);
/// Longer than the daemon's own grace period for session processes.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(50);

/// Starts the daemon in its own process group, detached from the caller, so
/// closing the app does not end it. Returns once it accepts connections.
pub fn detach(data_dir: &Path) -> Result<(), Box<dyn Error>> {
    if instance::is_running(data_dir)? {
        return Ok(());
    }
    private_fs::create_dir(data_dir)?;
    let log = private_fs::open_append(&data_dir.join(LOG_FILE))?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    detach_from_caller(&mut command);
    let child = command.spawn()?;
    wait_until(START_TIMEOUT, || {
        daemon_file::read(data_dir).is_some_and(|file| file.pid == child.id())
    })
    .then_some(())
    .ok_or_else(|| "daemon did not start in time".into())
}

/// Asks the running daemon to shut down cleanly, killing it if it does not.
pub fn stop(data_dir: &Path) -> Result<(), Box<dyn Error>> {
    // The lock, not daemon.json, says whether a daemon is alive: a stale pid
    // may belong to an unrelated process by now.
    if !instance::is_running(data_dir)? {
        daemon_file::remove(data_dir)?;
        return Ok(());
    }
    let pid = daemon_file::read(data_dir)
        .map(|file| file.pid)
        .ok_or("daemon is running but daemon.json is missing")?;
    process::terminate(pid);
    let stopped = || !instance::is_running(data_dir).unwrap_or(true);
    if !wait_until(STOP_TIMEOUT, stopped) {
        process::force_kill(pid);
        if !wait_until(Duration::from_secs(2), stopped) {
            return Err("daemon did not stop".into());
        }
    }
    daemon_file::remove(data_dir)?;
    Ok(())
}

#[cfg(unix)]
fn detach_from_caller(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    // A new process group keeps the caller's signals (Ctrl+C, app exit) away.
    command.process_group(0);
}

#[cfg(windows)]
fn detach_from_caller(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        thread::sleep(POLL);
    }
    done()
}
