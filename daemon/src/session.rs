use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::broadcast;

use crate::ring_buffer::{RingBuffer, Snapshot};
use crate::utf8::Utf8Decoder;

const RING_BUFFER_BYTES: usize = 1024 * 1024;
const LIVE_CHANNEL_CHUNKS: usize = 1024;
const READ_CHUNK_BYTES: usize = 16 * 1024;
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
    #[error("session input is closed")]
    InputClosed,
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
}

/// A live snapshot plus a receiver for every chunk after it.
pub struct Attachment {
    pub snapshot: Snapshot,
    pub live: broadcast::Receiver<OutputChunk>,
}

struct Output {
    buffer: RingBuffer,
    live: broadcast::Sender<OutputChunk>,
}

pub struct Session {
    output: Arc<Mutex<Output>>,
    input: mpsc::Sender<Vec<u8>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
}

impl Session {
    pub fn spawn(launch: &Launch) -> Result<Self, SessionError> {
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

        let (live, _) = broadcast::channel(LIVE_CHANNEL_CHUNKS);
        let output = Arc::new(Mutex::new(Output {
            buffer: RingBuffer::new(RING_BUFFER_BYTES),
            live,
        }));

        let reader_output = Arc::clone(&output);
        thread::spawn(move || {
            pump_output(reader, &reader_output);
            match child.wait() {
                Ok(status) => eprintln!("session process exited: {status:?}"),
                Err(error) => eprintln!("failed to wait for session process: {error}"),
            }
        });

        // Writes block when the child stops reading, so they get their own
        // thread instead of stalling the async runtime.
        let (input, pending_input) = mpsc::channel();
        thread::spawn(move || pump_input(writer, &pending_input));

        Ok(Self {
            output,
            input,
            master: Mutex::new(pair.master),
        })
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

    pub fn write_input(&self, data: &str) -> Result<(), SessionError> {
        self.input
            .send(data.as_bytes().to_vec())
            .map_err(|_| SessionError::InputClosed)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), SessionError> {
        lock(&self.master)?
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
        let seq = output.buffer.push(data.clone());
        // No receivers simply means nobody is attached right now.
        let _ = output.live.send(OutputChunk { seq, data });
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
        }
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
        let session = Session::spawn(&sh("printf cohors-ok")).expect("spawn");
        let snapshot = wait_for_output(&session, "cohors-ok");
        assert!(snapshot.data.contains("cohors-ok"));
        assert!(snapshot.seq >= 1);
    }

    #[test]
    fn forwards_input_to_process() {
        let session = Session::spawn(&sh("read line; printf \"got:%s\" \"$line\"")).expect("spawn");
        session.write_input("ping\r").expect("write");
        let snapshot = wait_for_output(&session, "got:ping");
        assert!(snapshot.data.contains("got:ping"));
    }

    #[test]
    fn live_stream_continues_after_snapshot_seq() {
        let session = Session::spawn(&sh("read line; printf after")).expect("spawn");
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
