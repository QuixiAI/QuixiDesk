# QuixiDesk Agent Instructions

- `QuixiDesk` is a native Rust desktop workspace (`tao` + `wry` `WKWebView`). Work from the repository root (this directory); never hard-code an absolute checkout path.
- Build: `make` (release) or `make app` (builds + packages `dist/QuixiDesk.app`); `make run` starts the app from cargo; `make test` runs `cargo test --workspace`.
- The workspace folder the engine opens defaults to the source checkout; override with `QUIXI_WORKSPACE=/path/to/folder`. Path defaults all live in `crates/quixi-proto/src/paths.rs`.
- `reference/` holds optional reverse-engineering artifacts (1,400-channel EIPC catalogue, exact schemas). It is gitignored; builds fall back to `crates/quixi-ipc/assets/ipc-exact-schemas.json` and the 1,400-channel parity sweep test self-skips when the catalogue is absent.
- MCP servers configured via `.agents/agents.json` and `~/.gemini/config/mcp_config.json`:
  - `computer-use`: `$HOME/.local/bin/quixi-mcp-builtin --computer-use` (screenshots, GUI clicks, typing, keyboard shortcuts, accessibility inspection via `127.0.0.1:19455` + OS CoreGraphics).
  - `browser-use`: `$HOME/.local/bin/quixi-mcp-builtin --browser-use` (browser automation, DOM inspection, tab control).
  - `filesystem`: `@modelcontextprotocol/server-filesystem`
  - `fetch`: `uvx mcp-server-fetch`
  - `git`: `uvx mcp-server-git`
  - `rea`: `rea-agents@6.7.0 mcp`
  - `zvec_grep`: `zg server --stdio`
  - `adrafinil`: `/Applications/Adrafinil.app/Contents/Helpers/adrafinil mcp`
