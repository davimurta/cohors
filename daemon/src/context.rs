//! Project context: a rules file and a plan file that every Claude session
//! of a project reads when it starts, resumes or compacts.
//!
//! Claude gets the folder through `--add-dir`, so it can read and update the
//! files, and a `SessionStart` hook (see [`crate::hooks`]) that runs this
//! binary with [`PRINT_FLAG`]. The hook reads the files at that moment, so
//! edits made between sessions reach resumed conversations too.

use std::fs;
use std::io;
use std::path::Path;

use crate::private_fs;

pub const RULES_FILE: &str = "regras.md";
pub const PLAN_FILE: &str = "plano.md";
/// `cohors-daemon --print-context <dir>` prints the context for the hook.
pub const PRINT_FLAG: &str = "--print-context";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContextFiles {
    pub rules: String,
    pub plan: String,
}

/// Missing files read as empty: a new project has no context yet.
pub fn read(dir: &Path) -> io::Result<ContextFiles> {
    Ok(ContextFiles {
        rules: read_file(&dir.join(RULES_FILE))?,
        plan: read_file(&dir.join(PLAN_FILE))?,
    })
}

pub fn write(dir: &Path, files: &ContextFiles) -> io::Result<()> {
    private_fs::create_dir(dir)?;
    private_fs::write_atomic(&dir.join(RULES_FILE), files.rules.as_bytes())?;
    private_fs::write_atomic(&dir.join(PLAN_FILE), files.plan.as_bytes())
}

/// The text Claude receives. Empty when both files are blank, so a project
/// without context adds nothing to the conversation.
pub fn render(dir: &Path, files: &ContextFiles) -> String {
    let sections: Vec<String> = [
        ("Rules", RULES_FILE, &files.rules),
        ("Plan", PLAN_FILE, &files.plan),
    ]
    .into_iter()
    .filter(|(_, _, text)| !text.trim().is_empty())
    .map(|(title, file, text)| format!("## {title} ({file})\n\n{}\n", text.trim_end()))
    .collect();
    if sections.is_empty() {
        return String::new();
    }
    format!(
        "# Project context\n\nThe user keeps these files for this project in {}. Follow the rules and use the plan as the current development plan.\n\n{}",
        dir.display(),
        sections.join("\n")
    )
}

/// Hook entry point: prints the context and never fails the hook, since a
/// failing hook would show an error in every new Claude session.
pub fn print(dir: &Path) {
    match read(dir) {
        Ok(files) => print!("{}", render(dir, &files)),
        Err(error) => eprintln!("failed to read context in {}: {error}", dir.display()),
    }
}

fn read_file(path: &Path) -> io::Result<String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("cohors-context-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn missing_files_read_as_empty() {
        let dir = temp_dir();
        assert_eq!(read(&dir).expect("read"), ContextFiles::default());
    }

    #[test]
    fn write_then_read() {
        let dir = temp_dir();
        let files = ContextFiles {
            rules: "Use tabs.".into(),
            plan: "1. Login".into(),
        };
        write(&dir, &files).expect("write");
        assert_eq!(read(&dir).expect("read"), files);
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn renders_only_filled_sections() {
        let dir = Path::new("/ctx");
        assert_eq!(render(dir, &ContextFiles::default()), "");
        let blank_plan = ContextFiles {
            rules: "Use tabs.\n\n".into(),
            plan: "  \n".into(),
        };
        let text = render(dir, &blank_plan);
        assert!(text.contains("in /ctx."));
        assert!(text.contains("## Rules (regras.md)\n\nUse tabs.\n"));
        assert!(!text.contains("## Plan"));
    }
}
