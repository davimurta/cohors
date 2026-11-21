# Backlog

Work agreed on but not scheduled in a milestone yet.

## Projects

Group sessions under projects, each with a predefined root folder, a Claude account and an SSH key.

- A project has a name, a root folder, an optional Claude account and an optional SSH key.
- New sessions in a project start in its root (or a folder inside it) and inherit its account and key.
- Claude account: a Claude Code config folder (the default `~/.claude`, or another one such as `~/.claude-work`), passed to the agent as `CLAUDE_CONFIG_DIR`. The app lists folders that hold a logged-in account and shows their email and organization, read from the folder's `.claude.json`. Cohors never reads or stores credentials.
- SSH key: a private key path under `~/.ssh`, applied as `GIT_SSH_COMMAND="ssh -i <key> -o IdentitiesOnly=yes"` for both Claude and shell sessions. Only the path is stored.
- Resume and the summary (`claude -p --resume`) must run with the same `CLAUDE_CONFIG_DIR` as the session, since Claude stores the conversation under that folder.
- The sidebar groups sessions by project.
