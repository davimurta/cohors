//! Signals for processes the daemon only knows by pid. Uses the system
//! tools instead of raw syscalls, which would need unsafe code.

use std::process::{Command, Stdio};

pub fn terminate(pid: u32) {
    #[cfg(unix)]
    run("kill", &["-TERM", &pid.to_string()]);
    #[cfg(windows)]
    run("taskkill", &["/PID", &pid.to_string(), "/T", "/F"]);
}

pub fn force_kill(pid: u32) {
    #[cfg(unix)]
    run("kill", &["-KILL", &pid.to_string()]);
    #[cfg(windows)]
    run("taskkill", &["/PID", &pid.to_string(), "/T", "/F"]);
}

fn run(program: &str, args: &[&str]) {
    let result = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if let Err(error) = result {
        eprintln!("failed to run {program}: {error}");
    }
}
