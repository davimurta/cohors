use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Finds `name` in the directories of `path` (a PATH-style list).
pub fn find_in(path: &OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|dir| candidate(&dir, name))
}

pub fn find(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| find_in(&path, name))
}

#[cfg(unix)]
fn candidate(dir: &Path, name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let file = dir.join(name);
    let metadata = std::fs::metadata(&file).ok()?;
    (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(file)
}

#[cfg(windows)]
fn candidate(dir: &Path, name: &str) -> Option<PathBuf> {
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".into());
    extensions
        .split(';')
        .map(|extension| dir.join(format!("{name}{extension}")))
        .find(|file| file.is_file())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cohors-exe-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create dir");
        dir
    }

    #[test]
    fn finds_executable_file() {
        let dir = temp_dir();
        let file = dir.join("tool");
        fs::write(&file, "").expect("write");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).expect("chmod");
        assert_eq!(find_in(dir.as_os_str(), "tool"), Some(file));
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn skips_non_executable_file() {
        let dir = temp_dir();
        let file = dir.join("tool");
        fs::write(&file, "").expect("write");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(find_in(dir.as_os_str(), "tool"), None);
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = temp_dir();
        assert_eq!(find_in(dir.as_os_str(), "absent"), None);
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
