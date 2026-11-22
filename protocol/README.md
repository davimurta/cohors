# Cohors protocol

JSON messages over a WebSocket on `127.0.0.1`. The daemon writes its port, token and pid to `daemon.json` in the user data directory (owner-only permissions). Only allowlisted `Origin` values may connect.

TypeScript types live in `index.ts`; the Rust side is `daemon/src/protocol.rs`.

## Handshake

The first client message must be `hello` within 5 seconds:

```json
{ "type": "hello", "token": "<token>", "protocolVersion": 2 }
```

The daemon answers `ready` with its versions and whether `claude` is in its PATH (`claudeAvailable`), or `error` (`unauthorized`, `unsupported_protocol`) and closes the connection.

## Sessions

| Client message | Effect |
| --- | --- |
| `session.list` | Replies `session.list.result` with every session's metadata, in creation order |
| `session.create {agent, cwd, name?}` | Starts `claude --session-id <uuid>` or the user's shell in `cwd`. Every client receives `session.created {session}`. A blank name becomes the folder name |
| `session.resume {id}` | Restarts an `exited` or `interrupted` session: `claude --resume <agentSessionId>` or a new shell, in the same folder and under the same id. Every client receives `session.updated {session}` |
| `session.finish {id, withSummary}` | Moves any unfinished session to `finished` and stops its process. For Claude sessions with `withSummary`, `summaryStatus` becomes `pending` and the daemon runs `claude -p --resume <agentSessionId>` with the session's folder, account and SSH key. The result is saved to `summary.md` and sent as `session.summary {id, status, text}` |
| `session.summarize {id}` | Generates the summary again (after `failed`, or when finished without one) |
| `session.skipSummary {id}` | Cancels a pending summary or gives up on a failed one; `summaryStatus` becomes `none` |
| `session.getSummary {id}` | Replies `session.summary` with the saved text |
| `session.notes {id, notes}` | Saves the notes; every client receives `session.updated` |
| `session.conversation {id}` | Replies `session.conversation.result {id, entries, usage}` from Claude Code's own log of the conversation (`<config dir>/projects/*/<agentSessionId>.jsonl`): user and assistant text, one entry per tool call (`"Bash: npm test"`), and the token totals. Thinking, tool results and subagents are left out. Both fields are null for shells and for Claude sessions without a log |
| `session.search {query}` | Replies `session.search.result {query, ids}`: finished sessions whose name, notes, summary or conversation text contain the query, ignoring case. A blank query returns no ids |
| `session.worktree.status {id}` | Replies `session.worktree.result {id, status, branchDeleted: null}` with `status {exists, changes, commits}` |
| `session.worktree.remove {id}` | Only for a session that no longer runs. Fails with `worktree_dirty` when there are uncommitted changes. Removes the folder and deletes the branch when it has no commits of its own (`branchDeleted: true`), or keeps it (`false`). Replies `session.worktree.result` |
| `session.rename {id, name}` | Renames any session, finished ones included. The name is trimmed; a blank one is `bad_request`. Every client receives `session.updated` |
| `session.transcript {id}` | Replies `session.transcript.result {id, data, truncated}` with the last 2 MB of output |
| `session.delete {id}` | Deletes a finished session's folder; every client receives `session.deleted {id}` |
| `session.attach {id}` | Replies `session.snapshot {id, seq, data}`, then streams `session.output {id, seq, data}` |
| `session.detach {id}` | Stops the output stream for that session |
| `session.input {id, data}` | Writes to the session PTY |
| `session.resize {id, cols, rows}` | Resizes the session PTY |

Output chunks carry a `seq` that increases by one per chunk within a session. The snapshot holds the recent output ring buffer and the `seq` of its last chunk, and the live stream continues at `seq + 1`, with no gaps or duplicates. A client that falls behind receives a fresh snapshot.

Failures are reported as `error {code, message, ref?}`. `ref` is the session id when one is involved, or the request type (`session.create`) otherwise. `session.create` fails with `invalid_cwd` when the folder does not exist and `agent_not_found` when `claude` is not installed. `session.resume` fails with `invalid_state` when the session is not exited or interrupted, `session.finish` when it is already finished, and `session.delete` when it is not finished or its summary is still pending.

## Daemon lifecycle

- One daemon per user: it holds an exclusive lock on `daemon.lock` while alive.
- `cohors-daemon` runs in the foreground. `cohors-daemon --detach` starts it in its own process group, logging to `daemon.log`, and returns once it accepts connections (or right away if one is already running). `cohors-daemon --stop` asks the running daemon to shut down and kills it if it does not.
- `daemon.shutdown`, SIGTERM and Ctrl+C all shut down cleanly: `daemon.json` is removed, running sessions are saved as `interrupted`, their processes get SIGHUP and, after 3 seconds, a hard kill.
- The daemon reads PATH from the user's login shell, so agents are found even when the app was opened from the Finder.

## Session states and storage

Each session lives in `sessions/<id>/` under the user data directory: `meta.json` (the `SessionMeta` above), `transcript.log` (raw terminal output, appended across resumes with a separator line) and, once generated, `summary.md`. All are owner-only. The transcript rotates to `transcript.1.log` at 20 MB, so a session keeps at most about 40 MB of output.

- `running` becomes `idle` after 10 seconds without output, with `idleSince` set to the last output, and back to `running` on the next output. The daemon checks every 2 seconds and sends `session.updated` only on a change.
- `running` or `idle` becomes `exited` when the process ends on its own, with `exitCode` and `endedAt` set and `session.updated` sent.
- On startup the daemon has no processes, so sessions saved as `running` or `idle` become `interrupted`, with `endedAt` set to the transcript's last write.
- A summary left `pending` by a daemon that stopped becomes `failed` on the next startup.
- Resuming keeps the session's output buffer, so attached clients keep streaming without re-attaching.

## Projects

A project groups sessions and gives them a root folder, a Claude account and an SSH key. Projects are saved in `projects.json` in the user data directory.

| Client message | Effect |
| --- | --- |
| `accounts.list` | Replies `accounts.list.result {claudeAccounts, sshKeys}`: the default Claude account plus every `~/.claude*` folder with a logged-in account (email and organization read from its `.claude.json`), and private keys in `~/.ssh` that have a `.pub` pair. Tokens and key contents are never read |
| `project.list` | Replies `project.list.result {projects}` |
| `project.create {project}` / `project.update {id, project}` / `project.delete {id}` | Every client receives the new `project.list.result`. The root must exist, and the account and key must be ones `accounts.list` returns |
| `project.context.get {id}` | Replies `project.context.result {id, dir, rules, plan}`; missing files read as empty |
| `project.context.save {id, rules, plan}` | Writes `regras.md` and `plano.md` and replies `project.context.result` to the requester |

`session.create {projectId}` copies the project's account and key into the session. Its processes run with `CLAUDE_CONFIG_DIR` set to the account folder (removed for the default account) and, when a key is set, `GIT_SSH_COMMAND="ssh -i <key> -o IdentitiesOnly=yes"`. Shells get the same variables. Resume reuses the values stored in the session, so later project edits do not move a conversation to another account. Deleting a project leaves its sessions untouched.

### Project context

Each project has a context folder with `regras.md` (rules) and `plano.md` (plan). By default it is `contexts/<projectId>` in the user data directory; `contextDir` points it at a folder of the user's choice instead. Deleting a project removes the default folder and leaves a chosen one alone.

`session.create {projectId}` also stores the folder in the session as `contextDir` (creating the default one). Claude sessions start and resume with `--add-dir <contextDir>`, so Claude can read and edit the files, and `--settings` with a `SessionStart` hook that runs `cohors-daemon --print-context <contextDir>`. The hook prints both files under a short header, so Claude gets the current text on every start, resume and compaction. Blank files print nothing. When the folder no longer exists the context is left out. Shells ignore the context.

### Waiting for the user

Claude sessions also get hooks that run `cohors-daemon --attention <state>`: `Stop` reports `done`, `Notification` reports `input` and `UserPromptSubmit` reports `clear`. The hook finds its session through `COHORS_SESSION_ID`, set in every session process, and writes the state to `sessions/<id>/attention`. The daemon checks those files every 2 seconds and sets `attention` in the session (`done`, `input` or null), announced with `session.updated`. A session already waiting keeps its first reason. `session.input`, resume, exit and finish clear it. The hook prints nothing and never fails.

## Terminal state on attach

When the ring buffer drops old output, the modes that output had set (alternate screen, bracketed paste, mouse, cursor visibility, keyboard protocol) are re-emitted at the start of the snapshot. After a snapshot, the client resizes the PTY one row smaller and back, since the kernel only signals a size change, so the agent repaints its screen.

### Worktrees

`session.create {worktree: true}` isolates the session in a git worktree of the repository holding `cwd`: a new branch `cohors/<name>-<short id>` from the current commit, checked out in `worktrees/<repo>-<short id>` in the user data directory. The session runs there (in the matching subfolder when `cwd` is one) and stores `worktree {repo, path, branch, base}`. Errors: `not_a_repository`, `no_commits`, `git_failed`. Finishing a session leaves the worktree; the client offers to remove it.
