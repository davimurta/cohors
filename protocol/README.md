# Cohors protocol

JSON messages over a WebSocket on `127.0.0.1`. The daemon writes its port and token to `daemon.json` in the user data directory (owner-only permissions). Only allowlisted `Origin` values may connect.

TypeScript types live in `index.ts`; the Rust side is `daemon/src/protocol.rs`.

## Handshake

The first client message must be `hello` within 5 seconds:

```json
{ "type": "hello", "token": "<token>", "protocolVersion": 1 }
```

The daemon answers `ready` with its versions, or `error` (`unauthorized`, `unsupported_protocol`) and closes the connection.

## Sessions

| Client message | Effect |
| --- | --- |
| `session.list` | Replies `session.list.result` |
| `session.attach {id}` | Replies `session.snapshot {id, seq, data}`, then streams `session.output {id, seq, data}` |
| `session.detach {id}` | Stops the output stream for that session |
| `session.input {id, data}` | Writes to the session PTY |
| `session.resize {id, cols, rows}` | Resizes the session PTY |

Output chunks carry a `seq` that increases by one per chunk within a session. The snapshot holds the recent output ring buffer and the `seq` of its last chunk, and the live stream continues at `seq + 1`, with no gaps or duplicates. A client that falls behind receives a fresh snapshot.

Failures are reported as `error {code, message, ref?}`, where `ref` is the session id when one is involved.
