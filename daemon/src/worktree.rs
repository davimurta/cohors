//! Git worktrees that isolate a session: its own branch and folder, so two
//! agents can work on the same repository without touching each other's
//! files. Worktrees live in the Cohors data folder, which keeps the
//! repository itself clean.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::executable;

const GIT: &str = "git";
/// Branches are `cohors/<session name>-<short id>`.
const BRANCH_PREFIX: &str = "cohors/";
const MAX_SLUG: usize = 40;
const SHORT_ID: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("git was not found in PATH")]
    GitNotFound,
    #[error("not a git repository: {0}")]
    NotARepository(String),
    #[error("the repository has no commits yet")]
    NoCommits,
    #[error("the worktree has uncommitted changes")]
    Dirty,
    #[error("git {command} failed: {message}")]
    Git { command: String, message: String },
    #[error("failed to run git: {0}")]
    Io(#[from] std::io::Error),
}

/// Stored in the session; enough to inspect and remove the worktree later.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    /// The main repository's top folder.
    pub repo: String,
    pub path: String,
    pub branch: String,
    /// Commit the branch started from.
    pub base: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStatus {
    /// False once the folder was removed, here or by hand.
    pub exists: bool,
    /// Uncommitted changes, untracked files included.
    pub changes: u32,
    /// Commits on the branch since it started.
    pub commits: u32,
}

/// What `remove` did with the branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovedBranch {
    /// Deleted: it had no commits of its own.
    Deleted,
    /// Kept, since it holds commits.
    Kept,
}

pub struct Git {
    program: PathBuf,
}

impl Git {
    pub fn find(path: &OsStr) -> Result<Self, WorktreeError> {
        executable::find_in(path, GIT)
            .map(|program| Self { program })
            .ok_or(WorktreeError::GitNotFound)
    }

    /// Creates a worktree for the repository holding `cwd`, on a new branch
    /// from its current commit. Returns it and the folder inside it that
    /// matches `cwd`, so a session started in a subfolder stays there.
    pub fn create(
        &self,
        cwd: &Path,
        worktrees_dir: &Path,
        name: &str,
        session_id: &str,
    ) -> Result<(Worktree, PathBuf), WorktreeError> {
        let top = self
            .run(cwd, &["rev-parse", "--show-toplevel"])
            .map_err(|_| WorktreeError::NotARepository(cwd.display().to_string()))?;
        let top = PathBuf::from(top);
        let base = self
            .run(&top, &["rev-parse", "HEAD"])
            .map_err(|_| WorktreeError::NoCommits)?;
        let short_id: String = session_id.chars().take(SHORT_ID).collect();
        let branch = format!("{BRANCH_PREFIX}{}-{short_id}", slug(name));
        let repo_name = top.file_name().map_or_else(
            || "repo".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        fs::create_dir_all(worktrees_dir)?;
        let path = worktrees_dir.join(format!("{repo_name}-{short_id}"));
        let path_text = path.display().to_string();
        self.run(&top, &["worktree", "add", "-b", &branch, &path_text, &base])?;
        let relative = relative_inside(cwd, &top);
        let session_cwd = relative.map_or_else(|| path.clone(), |relative| path.join(relative));
        let worktree = Worktree {
            repo: top.display().to_string(),
            path: path_text,
            branch,
            base,
        };
        Ok((worktree, session_cwd))
    }

    pub fn status(&self, worktree: &Worktree) -> Result<WorktreeStatus, WorktreeError> {
        let path = Path::new(&worktree.path);
        if !path.is_dir() {
            return Ok(WorktreeStatus {
                exists: false,
                changes: 0,
                commits: 0,
            });
        }
        let changes = self.run(path, &["status", "--porcelain"])?.lines().count();
        let range = format!("{}..HEAD", worktree.base);
        let commits = self.run(path, &["rev-list", "--count", &range])?;
        Ok(WorktreeStatus {
            exists: true,
            changes: u32::try_from(changes).unwrap_or(u32::MAX),
            commits: commits.parse().unwrap_or(0),
        })
    }

    /// Removes a clean worktree. Its branch goes too when it has no commits
    /// of its own; otherwise it stays, so no work is lost.
    pub fn remove(&self, worktree: &Worktree) -> Result<RemovedBranch, WorktreeError> {
        let status = self.status(worktree)?;
        if status.changes > 0 {
            return Err(WorktreeError::Dirty);
        }
        let repo = Path::new(&worktree.repo);
        if status.exists {
            self.run(repo, &["worktree", "remove", &worktree.path])?;
        } else {
            // Removed by hand: drop git's record of it.
            self.run(repo, &["worktree", "prune"])?;
        }
        if status.commits > 0 {
            return Ok(RemovedBranch::Kept);
        }
        // The branch tip is the base commit, so nothing is lost even if git
        // thinks the branch is unmerged.
        self.run(repo, &["branch", "-D", &worktree.branch])?;
        Ok(RemovedBranch::Deleted)
    }

    fn run(&self, dir: &Path, args: &[&str]) -> Result<String, WorktreeError> {
        let output = Command::new(&self.program)
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .output()?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
        }
        Err(WorktreeError::Git {
            command: args.first().copied().unwrap_or_default().to_owned(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

/// `cwd` relative to the repository top, when it is a subfolder of it.
fn relative_inside(cwd: &Path, top: &Path) -> Option<PathBuf> {
    // Git reports resolved paths (/private/tmp on macOS for /tmp).
    let cwd = fs::canonicalize(cwd).ok()?;
    let top = fs::canonicalize(top).ok()?;
    let relative = cwd.strip_prefix(&top).ok()?;
    (!relative.as_os_str().is_empty()).then(|| relative.to_path_buf())
}

/// Lowercase ASCII letters, digits and single dashes, for a branch name.
fn slug(name: &str) -> String {
    let mut slug = String::new();
    for character in name.chars().flat_map(char::to_lowercase) {
        let character = match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        };
        if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= MAX_SLUG {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "sessao".to_owned()
    } else {
        slug.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_in(dir: &Path, args: &[&str]) {
        let status = Command::new(GIT)
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=Cohors",
                "-c",
                "user.email=cohors@example.com",
            ])
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    }

    fn temp_repo() -> PathBuf {
        let root = std::env::temp_dir().join(format!("cohors-git-{}", uuid::Uuid::new_v4()));
        let repo = root.join("api");
        fs::create_dir_all(repo.join("src")).expect("dir");
        git_in(&repo, &["init", "-q"]);
        fs::write(repo.join("src").join("main.rs"), "fn main() {}\n").expect("file");
        git_in(&repo, &["add", "."]);
        git_in(&repo, &["commit", "-q", "-m", "init"]);
        root
    }

    fn git() -> Git {
        Git::find(&std::env::var_os("PATH").unwrap_or_default()).expect("git in PATH")
    }

    #[test]
    fn slugs_are_safe_branch_names() {
        assert_eq!(slug("Login com Google!"), "login-com-google");
        assert_eq!(slug("  ação / v2 "), "acao-v2");
        assert_eq!(slug("***"), "sessao");
        assert!(slug(&"x".repeat(80)).len() <= MAX_SLUG);
    }

    #[test]
    fn clean_worktree_is_removed_with_its_branch() {
        let root = temp_repo();
        let git = git();
        let id = uuid::Uuid::new_v4().to_string();
        let (worktree, cwd) = git
            .create(
                &root.join("api").join("src"),
                &root.join("worktrees"),
                "Login",
                &id,
            )
            .expect("create");
        assert!(worktree.branch.starts_with("cohors/login-"));
        assert!(cwd.ends_with("src"));
        assert!(cwd.join("main.rs").is_file());
        let status = git.status(&worktree).expect("status");
        assert_eq!(
            (status.exists, status.changes, status.commits),
            (true, 0, 0)
        );

        assert_eq!(
            git.remove(&worktree).expect("remove"),
            RemovedBranch::Deleted
        );
        assert!(!Path::new(&worktree.path).exists());
        assert!(!git.status(&worktree).expect("status").exists);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn work_is_never_lost() {
        let root = temp_repo();
        let git = git();
        let id = uuid::Uuid::new_v4().to_string();
        let (worktree, _) = git
            .create(&root.join("api"), &root.join("worktrees"), "Billing", &id)
            .expect("create");
        let path = PathBuf::from(&worktree.path);
        fs::write(path.join("notes.txt"), "draft").expect("change");
        assert_eq!(git.status(&worktree).expect("status").changes, 1);
        assert!(matches!(git.remove(&worktree), Err(WorktreeError::Dirty)));

        git_in(&path, &["add", "."]);
        git_in(&path, &["commit", "-q", "-m", "notes"]);
        assert_eq!(git.status(&worktree).expect("status").commits, 1);
        assert_eq!(git.remove(&worktree).expect("remove"), RemovedBranch::Kept);
        git_in(
            &root.join("api"),
            &["rev-parse", "--verify", &worktree.branch],
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_folders_outside_a_repository() {
        let dir = std::env::temp_dir().join(format!("cohors-plain-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("dir");
        let result = git().create(&dir, &dir.join("worktrees"), "x", "12345678");
        assert!(matches!(result, Err(WorktreeError::NotARepository(_))));
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
