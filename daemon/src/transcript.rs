//! Raw terminal output of a session on disk. The file is rotated once it
//! grows past a limit, keeping the previous one, so long sessions use at
//! most about twice the limit.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::private_fs;

pub const FILE_NAME: &str = "transcript.log";
const PREVIOUS_FILE_NAME: &str = "transcript.1.log";
const ROTATE_AT_BYTES: u64 = 20 * 1024 * 1024;

pub struct Transcript {
    dir: PathBuf,
    /// None only for the moment of a rotation.
    file: Option<File>,
    size: u64,
    rotate_at: u64,
}

impl Transcript {
    pub fn open(dir: &Path) -> io::Result<Self> {
        Self::open_with_limit(dir, ROTATE_AT_BYTES)
    }

    fn open_with_limit(dir: &Path, rotate_at: u64) -> io::Result<Self> {
        private_fs::create_dir(dir)?;
        let file = private_fs::open_append(&dir.join(FILE_NAME))?;
        let size = file.metadata()?.len();
        Ok(Self {
            dir: dir.to_path_buf(),
            file: Some(file),
            size,
            rotate_at,
        })
    }

    pub fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.size > 0 && self.size + bytes.len() as u64 > self.rotate_at {
            self.rotate()?;
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("transcript is closed"))?;
        file.write_all(bytes)?;
        self.size += bytes.len() as u64;
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        // Windows cannot rename a file that is still open.
        self.file = None;
        fs::rename(self.dir.join(FILE_NAME), self.dir.join(PREVIOUS_FILE_NAME))?;
        self.file = Some(private_fs::open_append(&self.dir.join(FILE_NAME))?);
        self.size = 0;
        Ok(())
    }
}

/// The last `max_bytes` of the transcript, reaching into the rotated file
/// when the current one is shorter.
pub fn read_tail(dir: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let current = tail_of(&dir.join(FILE_NAME), max_bytes)?;
    let missing = max_bytes.saturating_sub(current.len() as u64);
    let mut bytes = if missing > 0 {
        tail_of(&dir.join(PREVIOUS_FILE_NAME), missing)?
    } else {
        Vec::new()
    };
    bytes.extend(current);
    Ok(bytes)
}

fn tail_of(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(max_bytes)))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("cohors-transcript-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn rotates_and_reads_across_files() {
        let dir = temp_dir();
        let mut transcript = Transcript::open_with_limit(&dir, 10).expect("open");
        transcript.append(b"0123456789").expect("first");
        transcript.append(b"abcde").expect("second rotates");
        assert_eq!(
            fs::read(dir.join(PREVIOUS_FILE_NAME)).expect("old"),
            b"0123456789"
        );
        assert_eq!(fs::read(dir.join(FILE_NAME)).expect("new"), b"abcde");
        assert_eq!(read_tail(&dir, 8).expect("tail"), b"789abcde");
        assert_eq!(read_tail(&dir, 3).expect("tail"), b"cde");
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn reopening_continues_the_size() {
        let dir = temp_dir();
        Transcript::open_with_limit(&dir, 10)
            .expect("open")
            .append(b"12345678")
            .expect("write");
        let mut reopened = Transcript::open_with_limit(&dir, 10).expect("reopen");
        reopened.append(b"xyz").expect("write rotates");
        assert_eq!(fs::read(dir.join(FILE_NAME)).expect("new"), b"xyz");
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn missing_transcript_reads_empty() {
        assert!(read_tail(&temp_dir(), 10).expect("tail").is_empty());
    }
}
