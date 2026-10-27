# QuixiDesk Agent Instructions

- `QuixiDesk` is a native Rust desktop workspace (`tao` + `wry` `WKWebView`) located at `/Users/eric/quixi-desk` and packaged at `/Users/eric/quixi-desk/dist/QuixiDesk.app`.
- MCP servers configured via `.agents/agents.json` and `~/.gemini/config/mcp_config.json`:
  - `computer-use`: `/Users/eric/.local/bin/quixi-mcp-builtin --computer-use` (screenshots, GUI clicks, typing, keyboard shortcuts, accessibility inspection via `127.0.0.1:19455` + OS CoreGraphics).
  - `browser-use`: `/Users/eric/.local/bin/quixi-mcp-builtin --browser-use` (browser automation, DOM inspection, tab control).
  - `filesystem`: `@modelcontextprotocol/server-filesystem`
  - `fetch`: `uvx mcp-server-fetch`
  - `git`: `uvx mcp-server-git`
  - `rea`: `rea-agents@6.7.0 mcp`
  - `zvec_grep`: `zg server --stdio`
  - `adrafinil`: `/Applications/Adrafinil.app/Contents/Helpers/adrafinil mcp`
