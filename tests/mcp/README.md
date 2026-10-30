# MCP integration fixtures

`fixture_server.py` is a separate MCP peer implemented with Python's standard library. It does not execute arbitrary commands or access tool-supplied file paths. Its state, PID, protocol log and HTTP session state let tests distinguish real persistent connections from successful-looking stubs.

Run the integration suite from the repository root:

```sh
cargo test -p quixi-mcp --test lifecycle
```

Python 3 must be installed. Set `QUIXI_MCP_TEST_PYTHON` to its executable if it is not available as `python3` (`python` on Windows). Every test uses disposable app data and a loopback endpoint or an owned child process. No production MCP configuration or credentials are used.

The fixture accepts:

- `--transport stdio` (default), `http` (Streamable HTTP), or `sse` (legacy HTTP+SSE).
- `--scenario normal`, `paginated`, `cursor_loop`, `duplicate_tools`, `missing_tools_capability`, `zero_tools`, `unsupported_version`, `invalid_initialize`, `invalid_schema`, `complex_schema`, `large_catalog`, `external_schema`, `invalid_output`, `initialize_timeout`, `discovery_timeout`, `discovery_error`, `refresh_error`, `http_401`, `sse_post_401`, `sse_cross_origin`, `resource_prompt_pages`, `resources_error`, `modern`, `modern_rejection`, `malformed_stdout`, or `list_change`.
- `--log /path/to/events.jsonl` to record requests, initialization ordering, cancellation and process identity.
- `--pid-file /path/to/pid` and `--port-file /path/to/port` for test harness coordination.
- `--port N` for HTTP fixtures; zero chooses an available loopback port.
- `--token TOKEN` for the authenticated HTTP fixture (default is the non-secret literal `fixture-token`).

Tools include a stateful counter, echo with input constraints, a structured output schema, catalog-change notification, cancellable delay, deliberate server exit, explicit tool failure and JSON-RPC failure. Resources and prompts are also advertised to permit capability-specific discovery checks.

The server itself can be exercised by sending initialize, waiting for its response, sending notifications/initialized, then tools/list. It records any pre-initialization requests as an ordering violation. A syntactically valid `tools/call` for a tool named `tools/list` returns an error.

These fixtures establish application behavior under controlled server responses. A passing fixture suite does not establish compatibility with every third-party server or the quality of an LLM's tool-selection decisions. Real REA discovery and harmless execution are a separate validation step.
