---
name: mcp-troubleshooting
description: Diagnose and fix MCP configuration/runtime issues across supported integrations using agents CLI checks.
---

When MCP servers are failing or not visible:

1. Run `agents status --fast` to inspect configured vs enabled integrations.
2. Run `agents mcp test` for schema/config validation.
3. Run `agents mcp test --runtime` to check integration CLI/runtime connectivity.
4. Run `agents doctor` for environment and binary diagnostics.
5. Apply the smallest safe fix, then run `agents sync` and verify with `agents sync --check`.

Troubleshooting priorities:
- Missing binaries or PATH issues.
- Invalid server names/transport fields.
- Missing required environment variables or secrets.
- Target mismatch (server not routed to expected integration).
