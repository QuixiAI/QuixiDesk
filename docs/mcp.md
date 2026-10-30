# MCP lifecycle redesign and comparison criteria

QuixiDesk's MCP implementation is being verified against the October 29, 2026 static REA inspection of Claude Desktop 4.4212.0. The comparison concerns initialization, discovery, readiness, failure propagation and the surrounding connection lifecycle. It does not establish equality with every Claude account feature or every future server.

The REA report is `/tmp/claude-mcp-20261029/report.html` in the investigation environment. Its primary retained records are `ev_245c5ac8b184381866f628721b2e7c370be43a6067d6fe3022c43ac51d1eb11c` and `ev_e6c3f2131e48da8eff89f15aa8600ba7a7a020bb4c0fc20bb409ba456786c13b`. Findings are static source observations; the large shared main chunk was inspected manually after its analysis worker exceeded its heap. Dynamic account flags and universal live-refresh behavior were not established.

## Connecting a server

Open **Tools & skills → Configure MCP** and add a server entry. Existing command/args/env configurations remain valid. For REA:

```json
{
  "rea": {
    "command": "npx",
    "args": ["-y", "rea-agents@6.7.0", "mcp"]
  }
}
```

Use **Connect** to initialize and discover immediately, or ask the agent to use the server. The agent uses `McpDiscover` for actual server/tool discovery, then `McpCall` with the discovered name and full argument schema. Discovery does not execute a tool. External tool calls still require approval in Manual mode, even when a server labels a tool read-only. Approvals are bound to the server configuration; changing its command, URL, or environment invalidates an outstanding approval.

Remote examples use `{"transport":"http","url":"https://example.com/mcp"}` or `{"transport":"sse","url":"https://example.com/sse"}`. Optional `headers` configure static authentication. OAuth servers expose **Sign in** after an authentication challenge; authorization opens in the user's browser and returns through a local loopback callback. Tokens are encrypted with an OS credential-store key and refresh automatically. Cancelling a new authorization leaves existing credentials intact. **Forget sign-in** removes saved authorization and disconnects. A public pre-registered OAuth client ID can be supplied as `oauthClientId` when a server does not support dynamic registration.

The application configuration is `~/.quixi/quixi_desktop_config.json` (`data/quixi_desktop_config.json` inside a test profile). Environment values and headers are never included in connection status. Configured executable paths containing directory components are used literally; bare commands are resolved through the configured/process PATH plus common installation locations. The exact command `quixi-mcp-builtin` selects an in-process built-in adapter; an absolute executable path is treated as an external server.

| Setting | Default | Behavior |
|---|---:|---|
| `enabled` | `true` | Disabled servers cannot initialize or execute |
| `connectTimeoutMs` | 15000 | Transport and initialization deadline |
| `discoveryTimeoutMs` | 60000 | Complete catalog discovery deadline |
| `requestTimeoutMs` | 120000 | Individual tool/resource/prompt deadline |
| `idleTimeoutMs` | 300000 | Park idle connections; `0` disables parking |
| `maxPages` | 128 | Fail explicitly if discovery cannot complete within this bound |
| `maxTools` | 10000 | Maximum items in each catalog |
| `lifecycle` | `initialize` | Ordered legacy handshake; `discover` selects modern negotiation; `auto` permits protocol-defined fallback |
| `cwd` | Selected workspace | Working directory for the connection |

Connections are isolated by server configuration and workspace. Concurrent users of the same connection share initialization. Active requests prevent idle parking. Parking retains a visibly stale catalog and reconnects on the next use; explicit Disconnect stays disconnected until Connect. Failed mutations are never silently replayed. Invalid configuration saves preserve the existing file, and removed/replaced connections cannot publish results into a new connection's state.

Only a complete, validated catalog is published as ready. Empty catalogs and resource-only servers can be ready. Repeated pagination cursors, duplicate tool names, malformed schemas, and discovery failures produce explicit failures. Full schemas and metadata are preserved; model-facing discovery uses bounded pages and returns an unusually large individual schema intact. Catalog-change notifications trigger refresh. Unsupported legacy placeholder connectors are not advertised as working tools.

## Acceptance matrix

The tests referenced here are in `crates/quixi-mcp/tests/lifecycle.rs`. The verification section records the completed runs and remaining checks.

| Area | Observed Claude implementation | QuixiDesk acceptance criterion | Evidence/test |
|---|---|---|---|
| Initialization order | Starts transport, awaits and validates initialize, sends initialized, then discovers tools | Same ordered exchange; no tool request before handshake | `ordered_handshake_persistent_process_and_full_schema` |
| Persistent stdio | Retains child process and continuously decodes messages | Repeated calls share a process and state; process is owned and cleaned up | Stateful counter/PID assertion |
| Protocol mismatch | Rejects unsupported or malformed initialization | Explicit failure before tools/list | `invalid_initialization_never_reaches_discovery` |
| Reconnect generations | Connection lifecycle owns current transport | Old monitor tasks cannot overwrite a newly ready connection | `reconnect_monitor_generations_cannot_overwrite_the_current_ready_session` |
| Connection deduplication | Joins pending connection attempts | Concurrent callers create one server process | `concurrent_discovery_joins_one_connection_attempt` |
| Schema discovery | Real tools/list responses supply definitions | Preserve full input/output schemas and metadata rather than replace them with generic tools | Schema metadata assertion |
| Pagination | Modern path fetches pages with default 64-page limit; SSH path uses older single-page client | All supported transports complete pagination with configurable bounds and explicit cursor-cycle failure | `discovers_every_page_and_rejects_incomplete_catalogs` |
| Bounded model discovery | Returns discovered definitions through tool views | Page limits bound output while preserving a full oversized individual schema | `discovery_pages_bound_output_without_truncating_large_schemas` |
| Empty tools | Local tool manager rejects zero tools; remote helper may accept empty catalog | Capabilities and an empty successful catalog remain distinct from failure | `missing_tools_capability_and_empty_catalog_are_distinct_from_connection_failure` |
| Catalog changes | SDK invalidation exists; universal UI/model refresh not established | A list-changed notification updates the exposed catalog without reconnecting | `notification_refresh_reaches_the_catalog_without_reconnecting`, HTTP refresh test |
| Error propagation | Tool/transport failures return errors; no fabricated success fallback | Unknown server, unknown tool and failed calls cannot return synthetic success | Unknown routing and tool error tests |
| Schema validation | Some adapters lose JSON Schema fidelity | Validate arguments before execution and structured output before reporting success | `invalid_schema_and_invalid_structured_output_are_errors`, invalid input assertion |
| Separate deadlines | Connection, discovery and request deadlines exist | Configurable bounded deadlines with useful failure state | `initialization_discovery_and_tool_deadlines_are_bounded` |
| Cancellation | Request-ID correlation and abort propagation | Stop sends cancellation and prevents a late result; healthy connection remains reusable | `stopping_request_cancels_it_without_destroying_the_connection` |
| Idle sessions | Parks inactive sessions while retaining discoverable state | Parked catalogs are marked stale; use reconnects; active requests prevent parking | `idle_connections_park_with_stale_catalog_and_reconnect_on_use`, `active_requests_are_not_parked_after_idle_deadline` |
| Workspace isolation | Connection context follows workspace | Different workspaces never share server process state | `different_workspaces_do_not_share_process_state` |
| Application shutdown | Owns and tears down server children | Shutdown returns after terminating processes for all workspace connections | `shutdown_reaps_all_owned_workspace_processes_before_returning` |
| Explicit disconnect | State owns teardown and reconnection | Disconnected server stays disconnected until user reconnects or configuration changes | `explicit_disconnect_requires_reconnect_and_process_state_resets` |
| Configuration changes | Reconciles removals and suppresses late results | Removing configuration stops dispatch; invalid save preserves previous configuration | `removing_config_prevents_late_reconnection_and_invalid_save_preserves_config` |
| HTTP transport | Streamable HTTP and SSE paths | Both transports initialize, discover and execute actual tools | HTTP and legacy SSE tests |
| Authentication state | Distinguishes needs-auth and disconnected states | HTTP authorization challenge shows needs_auth; configured credentials work without leaking into status | `http_auth_failure_is_needs_auth_and_configured_credentials_can_connect` |
| Resource and prompt inventories | Discovers supported capabilities | Complete paginated catalogs, actual reads and prompt retrieval, and explicit discovery failure | `resources_and_prompts_use_the_established_server_session`, `resource_and_prompt_pagination_is_complete_and_failures_are_explicit` |
| Modern lifecycle | Multiple client generations coexist | Explicit modern discovery, legacy compatibility, and no fallback after a modern capability rejection | `modern_discovery_and_explicit_legacy_fallback_use_correct_lifecycle`, `modern_capability_rejection_does_not_fall_back_to_legacy` |
| Legacy SSE isolation | Remote endpoint handling | Same-origin endpoint, bounded event parser, no credential-forwarding redirect, and HTTP failure propagation | `legacy_sse_rejects_cross_origin_endpoint_and_preserves_http_auth_errors`, SSE parser unit tests |
| UI readiness | Connected and parked catalogs are distinct | Show actual lifecycle and stale catalog separately; controls perform actual actions | UI suite and native app verification |
| Agent access | Schema-driven tools and routing maps | Agent discovers actual names/schemas before dispatch; discovery errors visible to agent | Engine provider fixture tests |

## Required boundaries and honest comparisons

The state machine should distinguish configured, starting, initializing, discovering, ready, refreshing, failed, disconnected, disabled and needs_auth. Catalog freshness and connection state are separate facts: cached definitions must never imply a healthy connection.

A per-server request timeout is not permission to repeat a failed mutation. Automatic reconnect may prepare a subsequent request; it must not silently replay an in-flight tool call whose effects are unknown.

Tool descriptions, server instructions, annotations and returned content remain third-party data. They do not grant user permission. A server's read-only hint alone must not bypass QuixiDesk's permission policy.

Full OAuth user flows and protected token persistence have a separate module and verification suite. Native browser authorization, SSH/WSL transport execution, roots changes, and elicitation require separate verification before claiming parity in those areas. Resource/prompt inventories and modern lifecycle negotiation are covered by the tests above; the tests do not establish every resource template or modern subscription feature. Merely depending on an SDK with such features is not evidence that QuixiDesk has wired and tested them.

## Verification status

The October 29 MCP and engine core snapshot was copied into isolated directories and tested with actual Python subprocesses and loopback HTTP/SSE servers. The archive SHA-256 was `5497cda4300504a7872bbf005635e5d0086983af1d63f16e94792f284979c22f`. No production MCP configuration, credentials or installed app was modified by these tests.

| Platform | MCP verification | Engine verification |
|---|---|---|
| macOS | 16 unit + 35 lifecycle tests passed | Full workspace: 153 tests passed, including 79 engine and 4 helper tests |
| Linux (`eric-5090`) | Shutdown supplement: 16 unit + 35 lifecycle tests passed in release mode; helper 4 stdio tests passed | Core snapshot: 79 library tests passed |
| Windows (`win10`) | Shutdown supplement: 16 unit + 35 lifecycle tests passed in release mode; helper 4 stdio tests passed | Core snapshot: 79 library tests passed |

A later shutdown and standalone-helper supplement used archive SHA-256 `a664bf34fda697491bc5795f7538b8ca850aff9c396dd9a74329322e5a227763`. It reran the complete MCP suite and `cargo test --locked --release -p quixi-mcp-builtin` on both remote platforms; engine code was unchanged from its 79-test run. Its logs are `/tmp/quixi-mcp-validation-linux-shutdown.log` and `/tmp/quixi-mcp-validation-windows-shutdown.log` on the investigation Mac.

Remote commands were `cargo test --locked --release -p quixi-mcp` and `cargo test --locked --release -p quixi-engine --lib`. Final remote logs were retained in `/tmp/quixi-mcp-validation-linux-final.log` and `/tmp/quixi-mcp-validation-windows-final.log` on the investigation Mac. The Windows process-cleanup assertion uses a native process lookup; the same test uses `kill -0` on Unix. Fixture self-checks also passed for stdio, Streamable HTTP, and legacy SSE.

Native macOS verification used the Computer Use skill against an isolated rebuilt app. It verified REA 6.7.0 discovery of 207 tools, full-schema search, refresh, disconnect/reconnect, stale catalogs, missing-command and timeout errors, detailed configuration validation, truthful unsupported legacy connectors, and clean GUI quit. A separate live REA probe successfully called binary_session. The UI regression suite passed 37 tests. Native report and seven screenshots are retained in `/tmp/quixi-mcp-redesign/`; no real remote OAuth account was authorized. Passing these controlled tests does not establish universal Claude feature parity.
