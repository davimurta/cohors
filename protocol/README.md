# Cohors protocol

JSON messages over a WebSocket on `127.0.0.1`. The daemon writes its port and token to `daemon.json` in the user data directory (owner-only permissions). Only allowlisted `Origin` values may connect.

TypeScript types live in `index.ts`; the Rust side is `daemon/src/protocol.rs`.

## Handshake

The first client message must be `hello` within 5 seconds:

```json
{ "type": "hello", "token": "<token>", "protocolVersion": 1 }
```

The daemon answers `ready` with its versions and whether `claude` is in its PATH (`claudeAvailable`), or `error` (`unauthorized`, `unsupported_protocol`) and closes the connection.

## Sessions

| Client message | Effect |
| --- | --- |
| `session.list` | Replies `session.list.result` with every session's metadata, in creation order |
| `session.create {agent, cwd, name?}` | Starts `claude --session-id <uuid>` or the user's shell in `cwd`. Every client receives `session.created {session}`. A blank name becomes the folder name |
| `session.resume {id}` | Restarts an `exited` or `interrupted` session: `claude --resume <agentSessionId>` or a new shell, in the same folder and under the same id. Every client receives `session.updated {session}` |
| `session.attach {id}` | Replies `session.snapshot {id, seq, data}`, then streams `session.output {id, seq, data}` |
| `session.detach {id}` | Stops the output stream for that session |
| `session.input {id, data}` | Writes to the session PTY |
| `session.resize {id, cols, rows}` | Resizes the session PTY |

Output chunks carry a `seq` that increases by one per chunk within a session. The snapshot holds the recent output ring buffer and the `seq` of its last chunk, and the live stream continues at `seq + 1`, with no gaps or duplicates. A client that falls behind receives a fresh snapshot.

Failures are reported as `error {code, message, ref?}`. `ref` is the session id when one is involved, or the request type (`session.create`) otherwise. `session.create` fails with `invalid_cwd` when the folder does not exist and `agent_not_found` when `claude` is not installed. `session.resume` fails with `invalid_state` when the session is not exited or interrupted.

## Session states and storage

Each session lives in `sessions/<id>/` under the user data directory: `meta.json` (the `SessionMeta` above) and `transcript.log` (raw terminal output, appended across resumes with a separator line). Both are owner-only.

- `running` becomes `exited` when the process ends on its own, with `exitCode` and `endedAt` set and `session.updated` sent.
- On startup the daemon has no processes, so sessions saved as `running` become `interrupted`, with `endedAt` set to the transcript's last write.
- Resuming keeps the session's output buffer, so attached clients keep streaming without re-attaching.

## Terminal state on attach

When the ring buffer drops old output, the modes that output had set (alternate screen, bracketed paste, mouse, cursor visibility, keyboard protocol) are re-emitted at the start of the snapshot. After a snapshot, the client resizes the PTY one row smaller and back, since the kernel only signals a size change, so the agent repaints its screen.
