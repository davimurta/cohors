use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::broadcast;

use crate::ring_buffer::{RingBuffer, Snapshot};
use crate::utf8::Utf8Decoder;

const RING_BUFFER_BYTES: usize = 1024 * 1024;
const LIVE_CHANNEL_CHUNKS: usize = 1024;
const READ_CHUNK_BYTES: usize = 16 * 1024;
/// Output right after a resize is the program repainting, not activity.
const REPAINT_WINDOW: Duration = Duration::from_secs(1);
const INITIAL_SIZE: PtySize = PtySize {
    rows: 24,
    cols: 80,
    pixel_width: 0,
    pixel_height: 0,
};

/// Variables that mark a process as running inside another Claude Code
/// session. Inheriting them (e.g. when the daemon is started from a Claude
/// Code terminal) makes the child disable transcript saving and resume.
const INHERITED_AGENT_MARKERS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "AI_AGENT",
];

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("failed to open pty: {0}")]
    OpenPty(String),
    #[error("failed to spawn process: {0}")]
    Spawn(String),
    #[error("session has no running process")]
    NotRunning,
    #[error("failed to resize pty: {0}")]
    Resize(String),
    #[error("session state lock poisoned")]
    Poisoned,
}

#[derive(Clone, Debug)]
pub struct OutputChunk {
    pub seq: u64,
    pub data: String,
}

/// What to run inside the PTY. `program: None` runs the user's default shell.
pub struct Launch {
    pub program: Option<String>,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// PATH for the process, so agents find their tools even when the daemon
    /// was started with a minimal environment.
    pub path: Option<std::ffi::OsString>,
}

/// A live snapshot plus a receiver for every chunk after it.
pub struct Attachment {
    pub snapshot: Snapshot,
    pub live: broadcast::Receiver<OutputChunk>,
}

struct Output {
    buffer: RingBuffer,
    live: broadcast::Sender<OutputChunk>,
    transcript: Option<File>,
    last_output: Instant,
    repaint_until: Option<Instant>,
}

impl Output {
    fn push(&mut self, data: String) {
        let now = Instant::now();
        if self.repaint_until.is_none_or(|until| now >= until) {
            self.last_output = now;
        }
        if let Some(transcript) = &mut self.transcript {
            // A failed transcript write must not stop the live terminal.
            if let Err(error) = transcript.write_all(data.as_bytes()) {
                eprintln!("failed to write transcript: {error}");
                self.transcript = None;
            }
        }
        let seq = self.buffer.push(data.clone());
        // No receivers simply means nobody is attached right now.
        let _ = self.live.send(OutputChunk { seq, data });
    }
}

struct Process {
    input: mpsc::Sender<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    pid: Option<u32>,
    alive: Arc<AtomicBool>,
}

/// Terminal output of a session plus the process currently feeding it.
/// Resuming starts a new process on the same output, so attached clients
/// keep streaming across restarts.
pub struct Session {
    output: Arc<Mutex<Output>>,
    process: Mutex<Option<Process>>,
}

impl Session {
    pub fn new(transcript: Option<File>) -> Self {
        let (live, _) = broadcast::channel(LIVE_CHANNEL_CHUNKS);
        Self {
            output: Arc::new(Mutex::new(Output {
                buffer: RingBuffer::new(RING_BUFFER_BYTES),
                live,
                transcript,
                last_output: Instant::now(),
                repaint_until: None,
            })),
            process: Mutex::new(None),
        }
    }

    /// Runs `launch` in a new PTY. `on_exit` gets the exit code once the
    /// process ends and its output is fully read.
    pub fn start(
        &self,
        launch: &Launch,
        on_exit: impl FnOnce(Option<u32>) + Send + 'static,
    ) -> Result<(), SessionError> {
        let pair = native_pty_system()
            .openpty(INITIAL_SIZE)
            .map_err(|error| SessionError::OpenPty(error.to_string()))?;
        let mut child = pair
            .slave
            .spawn_command(command(launch))
            .map_err(|error| SessionError::Spawn(error.to_string()))?;
        // The slave must be closed here so reads hit EOF once the child exits.
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| SessionError::OpenPty(error.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| SessionError::OpenPty(error.to_string()))?;

        // A new process counts as fresh activity.
        lock(&self.output)?.last_output = Instant::now();
        let killer = child.clone_killer();
        let pid = child.process_id();
        let alive = Arc::new(AtomicBool::new(true));
        let output = Arc::clone(&self.output);
        let reader_alive = Arc::clone(&alive);
        thread::spawn(move || {
            pump_output(reader, &output);
            let exit_code = child.wait().ok().map(|status| status.exit_code());
            reader_alive.store(false, Ordering::SeqCst);
            on_exit(exit_code);
        });

        // Writes block when the child stops reading, so they get their own
        // thread instead of stalling the async runtime.
        let (input, pending_input) = mpsc::channel();
        thread::spawn(move || pump_input(writer, &pending_input));

        *lock(&self.process)? = Some(Process {
            input,
            master: pair.master,
            killer,
            pid,
            alive,
        });
        Ok(())
    }

    pub fn last_output(&self) -> Option<Instant> {
        lock(&self.output).ok().map(|output| output.last_output)
    }

    pub fn is_alive(&self) -> bool {
        lock(&self.process).is_ok_and(|process| {
            process
                .as_ref()
                .is_some_and(|process| process.alive.load(Ordering::SeqCst))
        })
    }

    /// Asks the process to end: SIGHUP on Unix, like a closed terminal.
    /// On Windows this already terminates it.
    pub fn terminate(&self) {
        if let Ok(mut process) = lock(&self.process)
            && let Some(process) = process.as_mut()
            && process.alive.load(Ordering::SeqCst)
        {
            let _ = process.killer.kill();
        }
    }

    /// For a process that ignored `terminate`.
    pub fn force_kill(&self) {
        let Ok(process) = lock(&self.process) else {
            return;
        };
        if let Some(Process {
            pid: Some(pid),
            alive,
            ..
        }) = process.as_ref()
            && alive.load(Ordering::SeqCst)
        {
            crate::process::force_kill(*pid);
        }
    }

    /// Snapshot and subscription are taken under the same lock, so the live
    /// stream starts exactly at `snapshot.seq + 1`.
    pub fn attach(&self) -> Result<Attachment, SessionError> {
        let output = lock(&self.output)?;
        Ok(Attachment {
            snapshot: output.buffer.snapshot(),
            live: output.live.subscribe(),
        })
    }

    /// Adds text that did not come from the process, such as a separator.
    pub fn write_marker(&self, text: &str) -> Result<(), SessionError> {
        lock(&self.output)?.push(text.to_owned());
        Ok(())
    }

    pub fn write_input(&self, data: &str) -> Result<(), SessionError> {
        let process = lock(&self.process)?;
        let process = process.as_ref().ok_or(SessionError::NotRunning)?;
        process
            .input
            .send(data.as_bytes().to_vec())
            .map_err(|_| SessionError::NotRunning)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), SessionError> {
        lock(&self.output)?.repaint_until = Some(Instant::now() + REPAINT_WINDOW);
        let process = lock(&self.process)?;
        let process = process.as_ref().ok_or(SessionError::NotRunning)?;
        process
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| SessionError::Resize(error.to_string()))
    }
}

fn command(launch: &Launch) -> CommandBuilder {
    let mut command = match &launch.program {
        Some(program) => CommandBuilder::new(program),
        None => CommandBuilder::new_default_prog(),
    };
    command.args(&launch.args);
    command.cwd(&launch.cwd);
    if let Some(path) = &launch.path {
        command.env("PATH", path);
    }
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    for marker in INHERITED_AGENT_MARKERS {
        command.env_remove(marker);
    }
    command
}

fn pump_input(mut writer: Box<dyn Write + Send>, pending: &mpsc::Receiver<Vec<u8>>) {
    while let Ok(bytes) = pending.recv() {
        if writer
            .write_all(&bytes)
            .and_then(|()| writer.flush())
            .is_err()
        {
            return;
        }
    }
}

fn pump_output(mut reader: Box<dyn Read + Send>, output: &Mutex<Output>) {
    let mut decoder = Utf8Decoder::default();
    let mut bytes = vec![0u8; READ_CHUNK_BYTES];
    loop {
        let read = match reader.read(&mut bytes) {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        let data = decoder.decode(&bytes[..read]);
        if data.is_empty() {
            continue;
        }
        let Ok(mut output) = output.lock() else {
            return;
        };
        output.push(data);
    }
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, SessionError> {
    mutex.lock().map_err(|_| SessionError::Poisoned)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn sh(script: &str) -> Launch {
        Launch {
            program: Some("/bin/sh".into()),
            args: vec!["-c".into(), script.into()],
            cwd: std::env::temp_dir(),
            path: None,
        }
    }

    fn started(script: &str) -> Session {
        let session = Session::new(None);
        session.start(&sh(script), |_| {}).expect("start");
        session
    }

    fn wait_for_output(session: &Session, needle: &str) -> Snapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = session.attach().expect("attach").snapshot;
            if snapshot.data.contains(needle) || Instant::now() > deadline {
                return snapshot;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn captures_process_output_in_snapshot() {
        let session = started("printf cohors-ok");
        let snapshot = wait_for_output(&session, "cohors-ok");
        assert!(snapshot.data.contains("cohors-ok"));
        assert!(snapshot.seq >= 1);
    }

    #[test]
    fn forwards_input_to_process() {
        let session = started("read line; printf \"got:%s\" \"$line\"");
        session.write_input("ping\r").expect("write");
        let snapshot = wait_for_output(&session, "got:ping");
        assert!(snapshot.data.contains("got:ping"));
    }

    #[test]
    fn input_without_process_is_rejected() {
        let session = Session::new(None);
        assert!(matches!(
            session.write_input("x"),
            Err(SessionError::NotRunning)
        ));
    }

    #[test]
    fn reports_exit_code() {
        let session = Session::new(None);
        let (sender, receiver) = mpsc::channel();
        session
            .start(&sh("exit 3"), move |code| {
                let _ = sender.send(code);
            })
            .expect("start");
        let code = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("exit reported");
        assert_eq!(code, Some(3));
    }

    #[test]
    fn repaint_after_resize_is_not_activity() {
        let session = started("trap 'printf repaint' WINCH; read line");
        thread::sleep(Duration::from_millis(200));
        let before = session.last_output().expect("last output");
        session.resize(100, 30).expect("resize");
        assert!(wait_for_output(&session, "repaint").data.contains("repaint"));
        assert_eq!(session.last_output(), Some(before));
    }

    #[test]
    fn terminate_stops_the_process() {
        let session = started("sleep 30");
        assert!(session.is_alive());
        session.terminate();
        let deadline = Instant::now() + Duration::from_secs(5);
        while session.is_alive() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!session.is_alive());
    }

    #[test]
    fn restart_keeps_previous_output() {
        let session = started("printf first");
        wait_for_output(&session, "first");
        session.write_marker("|marker|").expect("marker");
        session
            .start(&sh("printf second"), |_| {})
            .expect("restart");
        let snapshot = wait_for_output(&session, "second");
        let first = snapshot.data.find("first").expect("first");
        let marker = snapshot.data.find("|marker|").expect("marker");
        assert!(first < marker && marker < snapshot.data.find("second").expect("second"));
    }

    #[test]
    fn writes_output_to_transcript() {
        let path = std::env::temp_dir().join(format!("cohors-transcript-{}", uuid::Uuid::new_v4()));
        let session = Session::new(Some(File::create(&path).expect("create")));
        session.start(&sh("printf logged"), |_| {}).expect("start");
        wait_for_output(&session, "logged");
        let transcript = std::fs::read_to_string(&path).expect("read");
        assert!(transcript.contains("logged"));
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn live_stream_continues_after_snapshot_seq() {
        let session = started("read line; printf after");
        let mut attachment = session.attach().expect("attach");
        session.write_input("x\r").expect("write");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let chunk = runtime
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(5), attachment.live.recv()).await
            })
            .expect("timeout")
            .expect("chunk");
        assert_eq!(chunk.seq, attachment.snapshot.seq + 1);
    }
}
