//! Claude accounts and SSH keys available to projects. Only metadata is
//! read: the account email from `.claude.json` and key file names. Tokens
//! and private keys are never opened.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::protocol::{ClaudeAccount, SshKey};

const ACCOUNT_FILE: &str = ".claude.json";
const DEFAULT_CONFIG_DIR: &str = ".claude";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountFile {
    oauth_account: Option<OauthAccount>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OauthAccount {
    email_address: Option<String>,
    organization_name: Option<String>,
}

/// The default account (no `CLAUDE_CONFIG_DIR`) first, then every other
/// `~/.claude*` folder that holds a logged-in account.
pub fn claude_accounts(home: &Path) -> Vec<ClaudeAccount> {
    // Without CLAUDE_CONFIG_DIR, Claude Code keeps the account in ~/.claude.json.
    let (email, organization) = read_account(&home.join(ACCOUNT_FILE)).unwrap_or_default();
    let mut accounts = vec![ClaudeAccount {
        config_dir: None,
        email,
        organization,
    }];
    let Ok(entries) = fs::read_dir(home) else {
        return accounts;
    };
    let mut others: Vec<ClaudeAccount> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_extra_config_dir(path))
        .filter_map(|dir| {
            let (email, organization) = read_account(&dir.join(ACCOUNT_FILE))?;
            email.as_ref()?;
            Some(ClaudeAccount {
                config_dir: Some(dir.display().to_string()),
                email,
                organization,
            })
        })
        .collect();
    others.sort_by(|a, b| a.config_dir.cmp(&b.config_dir));
    accounts.extend(others);
    accounts
}

/// Private keys in `~/.ssh`: files that have a matching `.pub` next to them.
pub fn ssh_keys(home: &Path) -> Vec<SshKey> {
    let dir = home.join(".ssh");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut keys: Vec<SshKey> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "pub"))
        .filter_map(|public| {
            let private = public.with_extension("");
            private.is_file().then(|| SshKey {
                name: file_name(&private),
                path: private.display().to_string(),
            })
        })
        .collect();
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    keys
}

pub fn is_known_config_dir(home: &Path, dir: &str) -> bool {
    claude_accounts(home)
        .iter()
        .any(|account| account.config_dir.as_deref() == Some(dir))
}

pub fn is_known_ssh_key(home: &Path, path: &str) -> bool {
    ssh_keys(home).iter().any(|key| key.path == path)
}

fn is_extra_config_dir(path: &Path) -> bool {
    let name = file_name(path);
    path.is_dir() && name.starts_with(DEFAULT_CONFIG_DIR) && name != DEFAULT_CONFIG_DIR
}

fn read_account(path: &PathBuf) -> Option<(Option<String>, Option<String>)> {
    let bytes = fs::read(path).ok()?;
    let file: AccountFile = serde_json::from_slice(&bytes).ok()?;
    let account = file.oauth_account?;
    Some((account.email_address, account.organization_name))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home() -> PathBuf {
        let home = std::env::temp_dir().join(format!("cohors-home-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&home).expect("home");
        home
    }

    fn write_account(path: &Path, email: &str, organization: &str) {
        let json = serde_json::json!({
            "oauthAccount": { "emailAddress": email, "organizationName": organization },
            "otherSettings": true
        });
        fs::write(path, json.to_string()).expect("write account");
    }

    #[test]
    fn lists_default_and_extra_accounts() {
        let home = temp_home();
        write_account(&home.join(ACCOUNT_FILE), "me@personal.dev", "Personal");
        let work = home.join(".claude-work");
        fs::create_dir_all(&work).expect("work dir");
        write_account(&work.join(ACCOUNT_FILE), "me@work.dev", "Work Inc");
        fs::create_dir_all(home.join(".claude-empty")).expect("empty dir");
        fs::create_dir_all(home.join(DEFAULT_CONFIG_DIR)).expect("default dir");

        let accounts = claude_accounts(&home);
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].config_dir, None);
        assert_eq!(accounts[0].email.as_deref(), Some("me@personal.dev"));
        assert_eq!(accounts[1].config_dir, Some(work.display().to_string()));
        assert_eq!(accounts[1].organization.as_deref(), Some("Work Inc"));
        assert!(is_known_config_dir(&home, &work.display().to_string()));
        assert!(!is_known_config_dir(&home, "/elsewhere"));
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn default_account_is_listed_even_when_logged_out() {
        let home = temp_home();
        let accounts = claude_accounts(&home);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email, None);
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn lists_private_keys_with_a_public_pair() {
        let home = temp_home();
        let ssh = home.join(".ssh");
        fs::create_dir_all(&ssh).expect("ssh dir");
        for file in [
            "id_work",
            "id_work.pub",
            "id_old",
            "config",
            "known_hosts",
            "lone.pub",
        ] {
            fs::write(ssh.join(file), "").expect("write");
        }
        let keys = ssh_keys(&home);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "id_work");
        assert!(is_known_ssh_key(&home, &keys[0].path));
        fs::remove_dir_all(home).expect("cleanup");
    }
}
