# Cohors

Desktop app that manages multiple terminal AI agent sessions (Claude Code and plain shells) in one place.

A Rust daemon owns the PTYs and keeps sessions alive when the app is closed. The Tauri app connects to it over a local, token protected WebSocket and renders each session with xterm.js.

Status: early development.

## License

Apache-2.0. See [LICENSE](LICENSE).
