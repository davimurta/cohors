//! Apps opened from the Finder or a desktop launcher get a minimal PATH, so
//! the daemon asks the user's login shell for the real one.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

const START: &str = "__COHORS_PATH_START__";
const END: &str = "__COHORS_PATH_END__";

/// The login shell PATH followed by any entries of `current` it lacks.
pub async fn resolve(current: Option<OsString>) -> OsString {
    let login = login_shell_path().await;
    merge(login.as_deref(), current.as_deref())
}

#[cfg(unix)]
async fn login_shell_path() -> Option<String> {
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::process::Command;

    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
    // Markers separate PATH from anything the shell startup files print.
    // Works in sh, bash, zsh and fish (which joins PATH with colons when quoted).
    let script = format!("printf '{START}%s{END}' \"$PATH\"");
    let output = Command::new(shell)
        .args(["-l", "-c", &script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(5), output).await;
    match output {
        Ok(Ok(output)) => between_markers(&String::from_utf8_lossy(&output.stdout)),
        _ => {
            eprintln!("could not read the login shell PATH, using the inherited one");
            None
        }
    }
}

#[cfg(windows)]
async fn login_shell_path() -> Option<String> {
    // GUI apps on Windows already inherit the user's PATH.
    None
}

fn between_markers(output: &str) -> Option<String> {
    let start = output.find(START)? + START.len();
    let length = output[start..].find(END)?;
    Some(output[start..start + length].to_owned())
}

fn merge(first: Option<&str>, second: Option<&OsStr>) -> OsString {
    let mut entries: Vec<PathBuf> = Vec::new();
    let lists = [first.map(OsStr::new), second];
    for list in lists.into_iter().flatten() {
        for entry in std::env::split_paths(list) {
            if !entry.as_os_str().is_empty() && !entries.contains(&entry) {
                entries.push(entry);
            }
        }
    }
    std::env::join_paths(entries).unwrap_or_default()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn extracts_path_between_markers() {
        let output = format!("welcome!\n{START}/usr/bin:/opt/bin{END}");
        assert_eq!(
            between_markers(&output).as_deref(),
            Some("/usr/bin:/opt/bin")
        );
        assert_eq!(between_markers("no markers"), None);
    }

    #[test]
    fn merge_keeps_order_and_drops_duplicates() {
        let merged = merge(Some("/a:/b"), Some(OsStr::new("/b:/c::")));
        assert_eq!(merged, OsString::from("/a:/b:/c"));
    }

    #[test]
    fn merge_works_without_login_path() {
        assert_eq!(merge(None, Some(OsStr::new("/x"))), OsString::from("/x"));
    }

    #[tokio::test]
    async fn resolves_a_path_from_the_login_shell() {
        let path = resolve(None).await;
        assert!(!path.is_empty());
    }
}
