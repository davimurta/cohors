//! Files that may hold secrets or private work (token, transcripts) are
//! created readable by the owner only.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub fn create_dir(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// Writes atomically: a reader never sees a partial file.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
    temp_name.push(".tmp");
    let temp_path = path.with_file_name(temp_name);
    // A leftover temp file could have looser permissions, so never reuse it.
    match fs::remove_file(&temp_path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut file = options().write(true).create_new(true).open(&temp_path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(&temp_path, path)
}

pub fn open_append(path: &Path) -> io::Result<File> {
    options().append(true).create(true).open(path)
}

/// Read and write access, which Windows requires to lock a file.
pub fn open_read_write(path: &Path) -> io::Result<File> {
    options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

#[cfg(unix)]
fn options() -> OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options.mode(0o600);
    options
}

/// Windows profile folders are already private to the user.
#[cfg(not(unix))]
fn options() -> OpenOptions {
    OpenOptions::new()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("cohors-private-{}", uuid::Uuid::new_v4()))
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).expect("metadata").permissions().mode() & 0o777
    }

    #[test]
    fn files_and_dirs_are_owner_only() {
        let dir = temp_dir().join("nested");
        create_dir(&dir).expect("create dir");
        let file = dir.join("data.json");
        write_atomic(&file, b"{}").expect("write");
        let log = dir.join("log.txt");
        open_append(&log).expect("open");
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&log), 0o600);
        fs::remove_dir_all(temp_dir()).ok();
    }

    #[test]
    fn atomic_write_replaces_contents() {
        let dir = temp_dir();
        create_dir(&dir).expect("create dir");
        let file = dir.join("data.json");
        write_atomic(&file, b"first").expect("write");
        write_atomic(&file, b"second").expect("rewrite");
        assert_eq!(fs::read(&file).expect("read"), b"second");
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
