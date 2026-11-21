# Cohors

Desktop app that manages multiple terminal AI agent sessions (Claude Code and plain shells) in one place.

A Rust daemon owns the PTYs and keeps sessions alive when the app is closed. The Tauri app connects to it over a local, token protected WebSocket and renders each session with xterm.js.

Status: early development.

## Development

Requirements: Rust (stable), Node.js 24 and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).

From the repository root:

```sh
# Terminal 1: the daemon (writes its port and token to the user data directory)
cd daemon && cargo run

# Terminal 2: the app
cd app && npm install && npm run tauri dev
```

`npm run dev` serves the same UI in a browser at http://localhost:5173, which is handy for UI work.

Checks: `cargo fmt`, `cargo clippy` and `cargo test` in `daemon/`; `npm run build`, `npm run lint` and `npm test` in `app/`.

## License

Apache-2.0. See [LICENSE](LICENSE).
