//! Reads the conversation log Claude Code keeps for each session:
//! `<config dir>/projects/<folder>/<session id>.jsonl`, one JSON object per
//! line. Only the parts Cohors shows are read: the text of user and
//! assistant messages, one line per tool call, and token usage. Thinking,
//! tool results and side chains (subagents) are left out.
//!
//! The format belongs to Claude Code and is not documented, so unknown
//! lines are skipped instead of failing the whole read.

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const PROJECTS_DIR: &str = "projects";
/// Longest text kept per message; longer ones end with "…".
const MAX_TEXT_CHARS: usize = 20_000;
/// Longest tool call summary.
const MAX_TOOL_CHARS: usize = 160;
/// Tool input fields that describe a call best, in order of preference.
const TOOL_FIELDS: [&str; 8] = [
    "description",
    "command",
    "file_path",
    "pattern",
    "url",
    "query",
    "prompt",
    "action",
];
/// User messages that Claude Code writes itself (slash commands, their
/// output, reminders), not things the user typed.
const GENERATED_PREFIXES: [&str; 6] = [
    "<command-",
    "<local-command-",
    "<system-reminder>",
    "<bash-",
    "<task-notification>",
    "Caveat:",
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Text,
    /// A tool call, summarized in one line ("Bash: npm test").
    Tool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub role: Role,
    pub kind: EntryKind,
    pub text: String,
    pub timestamp: Option<String>,
}

/// Tokens of every model request in the conversation.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    /// Models used, in order of first use.
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Conversation {
    pub entries: Vec<Entry>,
    pub usage: Usage,
}

impl Conversation {
    /// Whether any message text contains `query`, ignoring case.
    pub fn mentions(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.entries.iter().any(|entry| {
            entry.kind == EntryKind::Text && entry.text.to_lowercase().contains(&query)
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    is_sidechain: bool,
    #[serde(default)]
    is_meta: bool,
    #[serde(default)]
    is_compact_summary: bool,
    timestamp: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    content: Option<Content>,
    usage: Option<LineUsage>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<serde_json::Value>),
}

#[derive(Clone, Copy, Default, Deserialize)]
struct LineUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

/// The log of `session_id` under a Claude config folder, if Claude wrote one.
/// Claude names the project folder after the working directory; searching
/// all of them avoids copying its naming rules.
pub fn find_log(config_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let file_name = format!("{session_id}.jsonl");
    fs::read_dir(config_dir.join(PROJECTS_DIR))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(&file_name))
        .find(|path| path.is_file())
}

pub fn read(path: &Path) -> io::Result<Conversation> {
    let reader = BufReader::new(fs::File::open(path)?);
    let mut entries = Vec::new();
    // One request can span several lines, each repeating its usage; the
    // last line has the final count.
    let mut requests: HashMap<String, LineUsage> = HashMap::new();
    let mut models: Vec<String> = Vec::new();
    for line in reader.lines() {
        let Ok(line) = serde_json::from_str::<Line>(&line?) else {
            continue;
        };
        if line.is_sidechain || line.is_meta || line.is_compact_summary {
            continue;
        }
        let Some(message) = line.message else {
            continue;
        };
        let role = match line.kind.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => continue,
        };
        if role == Role::Assistant {
            if let (Some(id), Some(usage)) = (&message.id, message.usage) {
                requests.insert(id.clone(), usage);
            }
            if let Some(model) = &message.model
                && !model.starts_with('<')
                && !models.contains(model)
            {
                models.push(model.clone());
            }
        }
        let timestamp = line.timestamp;
        for (kind, text) in message.content.map(content_entries).unwrap_or_default() {
            if role == Role::User && is_generated(&text) {
                continue;
            }
            entries.push(Entry {
                role,
                kind,
                text,
                timestamp: timestamp.clone(),
            });
        }
    }
    let usage = requests.values().fold(
        Usage {
            models,
            ..Usage::default()
        },
        |mut total, request| {
            total.input_tokens += request.input_tokens;
            total.output_tokens += request.output_tokens;
            total.cache_read_tokens += request.cache_read_input_tokens;
            total.cache_creation_tokens += request.cache_creation_input_tokens;
            total
        },
    );
    Ok(Conversation { entries, usage })
}

fn content_entries(content: Content) -> Vec<(EntryKind, String)> {
    let blocks = match content {
        Content::Text(text) => return text_entry(&text).into_iter().collect(),
        Content::Blocks(blocks) => blocks,
    };
    blocks
        .iter()
        .filter_map(|block| match block["type"].as_str()? {
            "text" => text_entry(block["text"].as_str()?),
            "tool_use" => Some((EntryKind::Tool, tool_summary(block))),
            _ => None,
        })
        .collect()
}

fn text_entry(text: &str) -> Option<(EntryKind, String)> {
    let text = text.trim();
    (!text.is_empty()).then(|| (EntryKind::Text, truncate(text, MAX_TEXT_CHARS)))
}

fn tool_summary(block: &serde_json::Value) -> String {
    let name = block["name"].as_str().unwrap_or("Ferramenta");
    let detail = TOOL_FIELDS
        .iter()
        .find_map(|field| block["input"][field].as_str())
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "));
    match detail {
        Some(detail) if !detail.is_empty() => {
            truncate(&format!("{name}: {detail}"), MAX_TOOL_CHARS)
        }
        _ => name.to_owned(),
    }
}

fn is_generated(text: &str) -> bool {
    GENERATED_PREFIXES
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

fn truncate(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_log(lines: &[serde_json::Value]) -> (PathBuf, PathBuf) {
        let config = std::env::temp_dir().join(format!("cohors-claude-{}", uuid::Uuid::new_v4()));
        let folder = config.join(PROJECTS_DIR).join("-dev-api");
        fs::create_dir_all(&folder).expect("dir");
        let path = folder.join("abc.jsonl");
        let text: Vec<String> = lines.iter().map(ToString::to_string).collect();
        fs::write(&path, text.join("\n") + "\n{not json\n").expect("write");
        (config, path)
    }

    fn assistant(id: &str, content: serde_json::Value, output: u64) -> serde_json::Value {
        json!({
            "type": "assistant",
            "timestamp": "2026-11-22T10:00:01Z",
            "message": {
                "id": id,
                "model": "claude-opus-5-5",
                "content": content,
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": output,
                    "cache_read_input_tokens": 100,
                    "cache_creation_input_tokens": 50
                }
            }
        })
    }

    #[test]
    fn reads_messages_tools_and_usage() {
        let (config, path) = write_log(&[
            json!({"type": "permission-mode", "permissionMode": "default"}),
            json!({"type": "user", "timestamp": "2026-11-22T10:00:00Z",
                   "message": {"role": "user", "content": "Add login"}}),
            json!({"type": "user", "message": {"content": "<command-name>/clear</command-name>"}}),
            json!({"type": "user", "isMeta": true, "message": {"content": "meta"}}),
            assistant("m1", json!([{"type": "thinking", "thinking": "hmm"}]), 5),
            assistant("m1", json!([{"type": "text", "text": "On it."}]), 7),
            assistant(
                "m1",
                json!([{"type": "tool_use", "name": "Bash",
                "input": {"command": "npm   test", "timeout": 1}}]),
                9,
            ),
            json!({"type": "user", "message": {"content": [
                {"type": "tool_result", "content": "ok"}]}}),
            json!({"type": "assistant", "isSidechain": true, "message": {
                "id": "side", "content": [{"type": "text", "text": "subagent"}]}}),
            assistant("m2", json!([{"type": "text", "text": "Done."}]), 3),
        ]);
        assert_eq!(find_log(&config, "abc"), Some(path.clone()));
        assert_eq!(find_log(&config, "other"), None);

        let conversation = read(&path).expect("read");
        let texts: Vec<(Role, EntryKind, &str)> = conversation
            .entries
            .iter()
            .map(|entry| (entry.role, entry.kind, entry.text.as_str()))
            .collect();
        assert_eq!(
            texts,
            [
                (Role::User, EntryKind::Text, "Add login"),
                (Role::Assistant, EntryKind::Text, "On it."),
                (Role::Assistant, EntryKind::Tool, "Bash: npm test"),
                (Role::Assistant, EntryKind::Text, "Done."),
            ]
        );
        assert_eq!(
            conversation.usage,
            Usage {
                input_tokens: 20,
                output_tokens: 12,
                cache_read_tokens: 200,
                cache_creation_tokens: 100,
                models: vec!["claude-opus-5-5".into()],
            }
        );
        assert!(conversation.mentions("LOGIN"));
        assert!(!conversation.mentions("npm test"));
        fs::remove_dir_all(config).expect("cleanup");
    }

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        assert_eq!(truncate("ação", 2), "aç…");
        assert_eq!(truncate("ok", 5), "ok");
    }
}
