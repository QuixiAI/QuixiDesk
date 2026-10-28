# QuixiDesk end to end test suite design

QuixiDesk needs tests that launch the packaged Rust application, operate its WKWebView and native controls, and verify the resulting files, requests, sessions, and process state. The highest priorities are preventing actions after Stop, enforcing the permission mode shown to the user, preserving conversations, and making provider and tool failures understandable.

**Status:** proposed suite and implementation specification, based on `main` at `7651df1`, extended through interactive exploration of Claude for Mac 4.29.0 (260974). This document does not claim the proposed runner, fixtures, hooks, CI jobs, or cases already exist. The catalog includes both regression coverage for implemented behavior and acceptance targets that may initially expose missing functionality. A capability must be implemented and pass its acceptance cases before being advertised as tested.

## Test architecture

Use three complementary layers. Keep their results separate so a passing service test cannot conceal a broken UI.

The native UI lane requires full Xcode and its UI-test tooling, an unlocked logged-in desktop, and a provisioned test account. Xcode Command Line Tools alone are sufficient for ordinary Rust builds but are not the proposed native UI-test environment. Prove this setup on the intended CI host before making native checks required.

| Layer | What actually runs | Main purpose | Execution |
| --- | --- | --- | --- |
| Rust contracts and integration | Engine, IPC router, parser, persistence and helper protocols | Exhaustive state transitions, malformed input and transport boundaries | Every change; no desktop required for pure tests |
| Packaged application E2E | Actual `.app`, actual WKWebView, renderer, preload, engine and controlled local dependencies | User journeys, rendering, cancellation, permission enforcement, restart recovery | Every PR on a logged-in macOS test host |
| Native and live qualification | Packaged app with native dialogs, real TCC decisions, controlled desktop apps and selected real providers | OS integration, shipping bundle, OAuth and external compatibility | Nightly subset and release gate on dedicated Macs |

The second layer is the main E2E suite. Operate the UI through macOS accessibility with an Xcode UI-test runner; reuse the existing application bridge for bounded DOM observations and diagnostics. Put protocol fixtures, filesystem assertions and report aggregation in a Rust test-support package. An initial bridge-driven implementation can accelerate coverage, but label those cases `bridge-e2e` until their critical actions also pass through native input. An AX smoke test is a prerequisite to committing to a particular native driver.

Do not depend on the interactive agent runtime or a paid model for routine CI. A small developer command should launch the same tests locally. The current computer-use tooling remains useful for exploratory investigations and reproductions.

### Evidence already available in the repository

| Existing evidence | What it proves | What it does not prove |
| --- | --- | --- |
| [`quixi-ipc` tests](../../crates/quixi-ipc/src/lib.rs) | Services, 1,400 catalog members, session lifecycle and event checks | Correct screen state, real permissions or actual parity for every channel |
| [`agent_tools.rs`](../../crates/quixi-engine/src/agent_tools.rs) tests | Argument validation, duplicate result reuse and permission decisions | Correct permission pill, real GUI action or cancellation under load |
| [`providers.rs`](../../crates/quixi-engine/src/providers.rs) and [`provider_tool_responses.rs`](../../crates/quixi-engine/src/provider_tool_responses.rs) tests | Stream parsing, catalog behavior, Responses item mapping | Live API compatibility and renderer state throughout a turn |
| `QUIXI_E2E_TEST` in [`main.rs`](../../crates/quixi-app/src/main.rs) | Timed route diagnostics and some IPC exercises | An assertion-based suite with reliable completion, isolation or failure exit codes |
| `/state`, `/action`, screenshot bridge in `main.rs` | Inspection and interaction with a running application | That the runner reached the intended build rather than another process |
| `is_headless_test_env` in [`quixi-platform`](../../crates/quixi-platform/src/lib.rs) | Deterministic picker substitutes | Native picker behavior; the E2E flag deliberately changes these paths |
| [`package-macos-app.sh`](../../scripts/package-macos-app.sh) | Bundle construction, resource copying and ad-hoc signing attempts | Successful signing, relocation, absence of missing resources or safe installation |

The architecture and design documents contain reconstruction material for Claude.app. Treat executable QuixiDesk source and product requirements as the authority for this suite, rather than assuming every feature in those documents exists in QuixiDesk.

### Journey boundaries

A genuine application E2E case starts at a user control and ends with an independent observable effect. For a tool write, that means composer → renderer → preload → EIPC → engine → provider request → tool → filesystem → result event → visible response. Verify file bytes with the runner, not by asking the same model whether the write succeeded.

Setup may seed fixture files and provider configuration. Seeding must not perform the operation being tested. A session-creation case must create through the composer; a restart case may seed an old transcript. Direct IPC calls belong in integration tests unless explicitly checking the bridge itself.

## Isolation and harness prerequisites

Build these before enabling unattended tool execution. Fail setup rather than running against the user's normal profile.

### A per-test sandbox

Each case receives a run UUID, case UUID, disposable directory, profile, workspace, log directory and ports. Use a dedicated macOS test account or disposable VM for native cases. The desktop agent must never target a developer's ordinary desktop during unattended CI.

Proposed directory layout:

```text
run/<run-id>/<case-id>/
  profile/                 app data and configuration
  home/                    synthetic home contents
  workspace/               working files and local Git repository
  website-data/            isolated WKWebView storage
  dependencies/            mock-provider and MCP state
  artifacts/               screenshots, AX trees, events and reports
  manifest.json            exact binary, resources, fixture and environment identity
```

Required changes and safeguards:

1. Introduce an explicit app-data root shared by engine, providers, MCP, netstack, transcripts and helpers. A proposed `QUIXI_DATA_DIR` is a new interface, not an existing option. Verify every persistence path uses it.
2. Retain `QUIXI_WORKSPACE` for fixture selection and `QUIXI_RESOURCES_DIR` for resource resolution. Never infer test storage from the original build checkout.
3. Give the test build an isolated WKWebView website data store and a unique test bundle identifier. Verify cookies, local storage, cache, downloads and persisted permission preferences are independent.
4. Set a controlled `HOME`, `TMPDIR` and environment allowlist, but do not assume this isolates `dirs::home_dir`, Keychain, TCC, browser storage, login shells or Apple services. Inventory and verify their effective paths and identities.
5. Remove production API keys, refresh tokens, inherited proxy configuration, cloud credentials and external MCP configuration from deterministic runs. Never clone the developer's real `providers.json` into CI.
6. Replace the fixed automation port with a selectable or dynamically allocated port. Add a handshake containing PID, bundle path, binary hash, source revision, profile root and run UUID. Refuse any mismatch; do not silently connect to an older app on port 19455.
7. Do not package into the normal `dist/` directory or copy helpers into the real `~/.local/bin` during tests. Add explicit output/install-root options or run packaging in a disposable account and checkout.
8. Run the deterministic suite with network egress restricted to its local fixture servers. External requests must fail loudly and be recorded. Restrict helper processes as well as the main app.
9. Record pre-run and post-run filesystem manifests. Exclude only known OS-owned caches. Any unexpected write outside the sandbox is a failure, even if the UI claims success.
10. Track child PIDs and process groups by run. Terminate only owned processes during teardown. Never use a broad `killall QuixiDesk` or remove shared user directories.

Some GUI tests require macOS permission grants. Provision these on the dedicated runner image; do not automate permission bypasses on a personal machine. Permission-denied cases use a separate controlled image/account with the grant absent.

### Separate observation from behavioral substitution

Split the existing `QUIXI_E2E_TEST` behavior into independent concerns:

- Instrumentation enables run identity, bounded observations and event capture.
- Optional fixture mode provides deterministic file-picker results and dependency overrides.
- Native qualification leaves file pickers, focus, animation and TCC behavior real.

The runner must explicitly choose a mode. Instrumentation must not automatically navigate routes, dismiss onboarding, create sessions, finish animations, select a provider, grant permissions or report success. Keep the old timed script as a separately named diagnostic until it is retired.

Add test control endpoints only to a test build/feature, with a per-run capability token. The shipping computer-use automation bridge is a separate product interface; its authorization and origin protections need their own tests. Do not add an unauthenticated general eval endpoint as a test convenience.

### Runner independence and cleanup

Capture a copy of each fixture definition before launch, and have the fixture ledger verify it was not modified by the application. Treat application-generated diagnostic text, workspace files and web page content as observations, never as runner instructions. A model must not be able to change the scenario or expected outcome by writing a file in its workspace. Bind fixture-control interfaces separately from model-visible endpoints and keep their credentials out of tool-visible environments.

Cleanup is part of the verdict. Snapshot the app-owned profile, workspace and request ledger before teardown, then stop app and owned helper process groups, wait for exit and close fixture services. Preserve failure artifacts before removing disposable files. Compare host-side sentinel hashes and inspect for leftover ports/processes. Retain a failed sandbox when cleanup cannot complete safely; do not erase evidence to obtain a green result.

### Stable interaction and synchronization

Prefer accessibility roles, labels and stable app-owned identifiers. Add stable IDs to the composer, Send, Stop, permission pill, provider controls, message groups, tool cards and navigation items. For bundled renderer content that cannot be rebuilt, attach narrow identifiers in the owned preload with an assertion that each target is unique. Missing targets fail; they must not fall back to the first text match.

An element index from one AX snapshot is valid only for that snapshot. Re-query after navigation, list mutations, overlays and streaming updates. Coordinate clicks are a native-layout fallback and must use bounds read from the current screen.

Wait for conditions, never fixed sleeps: app-ready handshake; composer enabled; provider request observed; first rendered delta; tool result visible; session idle; transcript flushed; worker retired. Every wait has a timeout and records the unmet condition. Use fixture barriers to release delayed responses exactly when a test chooses.

No assertion should rely on English copy alone outside English-language cases. Separate stable status/role assertions from translated text assertions. Do not disable all animations: that would conceal the WAAPI regression.

### Independent event and effect ledger

Record a monotonic timestamp and correlation fields for each boundary: run, case, session, turn, provider request, tool call, cache key, execution ID and event sequence. Distinguish a requested call, a denied call, a cached result and an executed action. Count side effects from the fixture filesystem or controlled server, not just application logs.

Required observations include provider request count, exact normalized request body, completion status, tool start/end, permission decision, cancellation acceptance, persistence completion, renderer update and turn terminal state. Logs are supporting evidence; they cannot be the only proof of a visible outcome.

## Fixture services and data

### Scripted provider service

Use a local server that implements the transport shapes the app supports. It is a protocol fixture, not an LLM. It selects scenarios using a test-control channel, not instructions in user-visible chat. Each request is checked against the expected role order, model, auth placeholder, tool definitions and prior call results. Unexpected requests fail the case.

Provide these profiles:

| Fixture profile | Controlled behavior |
| --- | --- |
| Text | Empty, single and many text deltas; Unicode split across transport reads; large output |
| Thinking | Reasoning-only delay, reasoning then text, interleaved metadata, long thinking |
| Tools | Single call, fragmented arguments, multiple indexes, parallel-looking batch, multiple rounds |
| Responses | Text and function-call items; call IDs; reasoning/encrypted items; completed output |
| Errors | HTTP 400/401/403/404/408/429/500/502/503; headers and bounded error body |
| Stream faults | Disconnect before headers, mid-event, after complete arguments, after finish, and between calls |
| Completion faults | EOF without finish, `[DONE]` without successful finish, `length`, refusal, incomplete/failed Responses |
| Retry | One 401 followed by refresh success or failure; repeated 503; rate-limit response |
| Repetition | Same call ID; new ID with identical arguments; changed object-key order; mixed cached and fresh batch |
| Isolation | Two interleaved sessions with distinct nonces and tool outputs |
| Cancellation | Barriers before first delta, before completed call, between actions and before final reply |
| Limits | Tool round cap, output-size cap, slow stream and request deadline |

Exercise byte-level packet fragmentation and SSE framing independently. A chunk split is not an SSE event split. Include comments, CRLF, empty lines, optional whitespace, usage-only events and multiple `data:` lines according to the supported protocol. Add a valid baseline for every fault mutation so a rejecting parser does not pass by rejecting everything.

OpenAI OAuth qualification uses the Responses path. Introduce a test-only endpoint/auth transport seam for local Responses and refresh fixtures; do not send fake tokens to a real cloud service. Keep real OAuth credentials out of deterministic tests.

The Anthropic wire format needs its own scenarios where supported. If a provider currently falls back to text-only, assert the explicit capability state; never count a text response as tool-loop coverage.

### MCP fixtures

Provide a tiny deterministic stdio MCP server with tools for reading a value, appending one nonce, delaying a result, returning a structured error, returning `isError: true`, producing an image and crashing. Record each invocation independently. Include malformed JSON-RPC, initialization failure, timeout, partial stdout, stderr noise and child exit. Test names, aliases and UUIDs with the same permission expectations.

Use a disposable native target application for real computer-use qualification. It displays labeled buttons, a text field, a counter and a draft area. Actions change its visible counter and a test-owned ledger. This proves the click or typing happened in the intended app without using Mail, sending messages or closing personal windows.

### Workspace fixtures

Create deterministic local Git repositories: clean; tracked modification; staged modification; untracked file; conflict; branch; detached HEAD; Unicode paths; spaces; symlink; large tree. Git identities are test identities. No remotes point to real repositories. Where push tests are needed, use a local bare repository.

Provide UTF-8, CRLF, empty and large text files, binary files, permission-denied paths, a symlink escaping the root and a read-only directory. Document whether each tool is workspace-restricted or intentionally allowed to use approved absolute paths; do not assume the SafeFs policy is applied to every agent tool.

Web fixtures include forms, delayed resources, a redirect, a server error, link navigation, a download, a small canvas, a dynamic DOM and adversarial page text. Host everything locally. Document viewer fixtures should be generated minimal files with known content: PDF, DOCX, PPTX, CSV and PNG. Do not depend on proprietary extraction artifacts.

Persistence fixtures include valid current profiles, supported legacy model IDs, old transcripts, partially written JSON, a truncated last record, duplicate IDs, unknown fields and incompatible future versions. Keep corrupted fixtures distinguishable from real user data.

## Test priority and execution policy

- **P0:** data loss, unauthorized side effects, wrong profile/build, post-cancel actions, broken launch, lost or misattributed conversation. Blocks merge/release when implemented coverage fails.
- **P1:** core workflow failure, incorrect provider/tool behavior, persistence failure, unusable UI. Blocks merge for the affected capability.
- **P2:** broader compatibility, accessibility, noncritical layout and extended recovery. Required nightly/release according to the capability.

A priority is impact, not the number of times a case runs. Native-TCC and live-provider P0 cases may run on scheduled infrastructure rather than every PR; their latest qualifying evidence is still a release requirement.

The following catalog assigns IDs for issue tracking, executable tests and reports. Parameterized variants share an ID with a named variant in reports. The rows describe required outcomes, not current test results.

## Test catalog

### Launch and packaging

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| BOOT-01 | P0 | Clean tracked checkout plus lockfile builds/tests without `reference/` or `asar-extracted/`; embedded IPC defaults are present. |
| BOOT-02 | P0 | Fresh profile opens a usable window; onboarding is visible and keyboard reachable; no fabricated task appears. |
| BOOT-03 | P1 | Returning profile opens the intended view with no repeated onboarding or duplicated sessions. |
| BOOT-04 | P0 | Packaging from a path containing spaces and non-ASCII characters produces the right version, binary and helpers. |
| BOOT-05 | P0 | Copy `.app` elsewhere, make source checkout unavailable, launch with no resource override; all core views and tools work. |
| BOOT-06 | P1 | Missing/corrupt required resources produce an actionable startup error rather than an indefinitely blank window. |
| BOOT-07 | P1 | Run from Finder-style environment with minimal PATH; helpers resolve from bundle and shell failures are explicit. |
| BOOT-08 | P0 | Concurrent app/test instance cannot attach to another profile or automation bridge; collision fails safely. |
| BOOT-09 | P1 | Bundle signature verifies; required helper binaries and resources are present; missing items fail the packaging gate. |
| BOOT-10 | P1 | Minimum supported OS and Intel/Apple Silicon builds launch; architecture mismatch is reported. |

### Navigation and workspace selection

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| NAV-01 | P1 | New, recents, projects, artifacts, scheduled and customize controls open their intended views without blank panels. |
| NAV-02 | P1 | Back/Forward restore correct task, selection and composer draft without resubmitting a prompt. |
| NAV-03 | P1 | Open saved session through a deep route and after restart; correct session loads once. |
| NAV-04 | P1 | Unknown route, missing session or deleted item shows a recoverable empty/not-found state. |
| NAV-05 | P0 | Switch workspaces with same filenames; tools and context use the selected root, verified by distinct contents. |
| NAV-06 | P1 | Add/remove working folder updates UI and persisted selection; cancel native folder picker leaves selection unchanged. |
| NAV-07 | P1 | Workspace path with spaces/Unicode and missing/deleted workspace produce correct behavior and explicit errors. |
| NAV-08 | P2 | Sidebar collapse, panel resize, window resize and navigation retain a usable composer and focus. |

### Provider configuration

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| PROV-01 | P1 | Fresh profile with zero providers cannot silently use an external account or claim a real generated answer. |
| PROV-02 | P1 | Save local Custom Endpoint through the UI; selected provider/model drive the next request and survive restart. |
| PROV-03 | P1 | Invalid URL, blank model, unsupported format and invalid credentials produce field/test errors without losing input. |
| PROV-04 | P0 | Secrets remain masked in the form, app logs, event diagnostics, screenshots and exported reports. |
| PROV-05 | P1 | Test Connection reports actual fixture success/failure and latency, not unconditional success. |
| PROV-06 | P1 | Model picker exposes available models for each configured auth mode and persists session choice. |
| PROV-07 | P1 | Disconnect active provider updates usable models and does not keep issuing requests with its credentials. |
| PROV-08 | P1 | Change provider during one running turn; that turn remains consistently routed, and next turn uses the new selection. |
| PROV-09 | P1 | Migrate legacy model IDs without losing session history; unknown IDs get an explicit supported fallback. |
| PROV-10 | P1 | OAuth cancel/deny/expiry/401 refresh are recoverable; one allowed refresh cannot loop indefinitely. |
| PROV-11 | P1 | Each supported transport exposes truthful capabilities; a text-only fallback is not presented as executable tools. |
| PROV-12 | P2 | Local scan handles available, absent and unreachable servers without freezing configuration. |

### Composer and conversations

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| CHAT-01 | P1 | Type/send a prompt; one user message and one intended provider turn appear with matching text and IDs. |
| CHAT-02 | P1 | Enter sends; the supported newline shortcut inserts a newline; pasted multiline text is preserved. |
| CHAT-03 | P1 | Empty/whitespace prompt and double-click Send cannot create duplicate turns. |
| CHAT-04 | P1 | Follow-up appears as a new user/assistant pair and sends the right history once. |
| CHAT-05 | P1 | Edit/restart-from-message uses the documented history boundary; abandoned output is not sent as current context. |
| CHAT-06 | P0 | Two sessions run concurrently; deltas, results, permissions and titles never cross session IDs. |
| CHAT-07 | P1 | Switch away during a turn and return; transcript, unread state and running state agree. |
| CHAT-08 | P1 | Long messages, code fences, Markdown, Unicode, emoji and RTL text render and copy correctly. |
| CHAT-09 | P1 | Composer draft survives supported navigation; user scrolling up is respected while a response streams. |
| CHAT-10 | P1 | A failed or stopped turn accepts a later prompt without stale spinner, cached failure or appended prior answer. |

### Streaming and error handling

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| STREAM-01 | P1 | Text arrives incrementally; completion settles the spinner and enables input exactly once. |
| STREAM-02 | P1 | Thinking appears during a delayed answer without corrupting text-block types or duplicating content. |
| STREAM-03 | P1 | Arbitrary TCP/SSE chunk boundaries preserve text, argument JSON, Unicode and call indexes. |
| STREAM-04 | P0 | EOF without valid successful completion cannot execute any accumulated tool request. |
| STREAM-05 | P0 | A complete first call followed by a malformed/truncated second call cannot partially execute the batch. |
| STREAM-06 | P1 | HTTP error is displayed once in its own turn; request count matches the explicit retry policy. |
| STREAM-07 | P1 | Transport error before/after first delta produces a terminal failure and preserves already-visible text appropriately. |
| STREAM-08 | P1 | Rate limit/refusal/authentication failure are distinguishable and do not masquerade as successful task completion. |
| STREAM-09 | P1 | Unknown metadata and supported optional events do not corrupt valid output; malformed required events fail explicitly. |
| STREAM-10 | P1 | Long thinking and slow output stay responsive within deadlines; total timeout clears running state. |
| STREAM-11 | P0 | Late deltas/results from a replaced or cancelled worker cannot modify the current turn. |
| STREAM-12 | P1 | Empty successful output has a truthful empty-result state; no fabricated claim that work was done. |

### Agent tools

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| TOOL-01 | P1 | Bash returns independent stdout, stderr and exit status; nonzero exit is a tool error, not success. |
| TOOL-02 | P1 | Read respects line offset/limit and handles missing, binary, large and unreadable files predictably. |
| TOOL-03 | P0 | Write creates exact bytes and intended parents only; missing content never becomes an accidental empty overwrite. |
| TOOL-04 | P0 | Edit requires one exact match; zero/multiple matches leave the original file byte-identical. |
| TOOL-05 | P1 | A dependent Write → Read → Edit → Read sequence uses actual prior results across rounds. |
| TOOL-06 | P0 | Identical calls under new IDs, reordered argument keys and within one batch execute once per turn. |
| TOOL-07 | P1 | A mixed batch containing cached and new calls executes only new actions while returning all required provider results. |
| TOOL-08 | P1 | Cached results are labeled accurately or omitted from executed-action summaries; no duplicate “ran command.” |
| TOOL-09 | P1 | A deliberately repeated call in a later user turn can run again; caching is scoped to the intended turn. |
| TOOL-10 | P1 | Loop/round limits stop the agent with a truthful partial-work summary and no extra action. |
| TOOL-11 | P0 | Missing, null, wrong-type and malformed arguments cannot trigger side effects; unknown tool is an explicit error. |
| TOOL-12 | P1 | Huge output is bounded before reentering provider context; truncation is identified; UI stays responsive. |
| TOOL-13 | P1 | MCP error content and `isError` propagate as tool errors; transport success alone does not imply action success. |
| TOOL-14 | P1 | Image results are bounded and correctly represented; do not claim the model saw screenshot pixels if only a marker was sent. |

### Permissions and boundaries

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| PERM-01 | P0 | Manual permits supported observational operations but blocks Bash, Write, Edit and other mutations with actionable UI. |
| PERM-02 | P0 | New Auto session retains Auto; allowed disposable operation succeeds and restarts preserve the intended mode. |
| PERM-03 | P0 | Changing an existing mode updates pill, persisted state, emitted event and next action decision together. |
| PERM-04 | P0 | Auto → Manual during a delayed provider turn blocks actions not yet started. |
| PERM-05 | P0 | GUI names, aliases, UUIDs and case variants cannot bypass Manual; unknown action names require consent. |
| PERM-06 | P0 | Filesystem/Git/shell actions through MCP obey the same policy as equivalent native tools. |
| PERM-07 | P0 | `acceptEdits` allows only its defined edit scope, not shell or GUI actions; bypass behavior is explicit. |
| PERM-08 | P0 | Browser navigation/page text, workspace instructions and tool results cannot change the permission mode. |
| PERM-09 | P0 | Absolute paths, traversal and symlinks follow the explicit filesystem policy; protected fixture files remain untouched. |
| PERM-10 | P0 | Untrusted web content cannot invoke EIPC, evaluate app context, read the profile or control the automation bridge. |
| PERM-11 | P0 | Test-control tokens and run IDs cannot authorize actions in another app/profile; shipping mode exposes no test override. |
| PERM-12 | P1 | If per-action approvals are added, allow/deny/cancel, stale requests and “allow once” are tested independently. Until then Manual must block and explain switching modes; do not pretend an approval dialog exists. |

### Stop and races

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| STOP-01 | P0 | Stop before first delta immediately clears the UI and prevents subsequent effects after server release. |
| STOP-02 | P0 | Stop after arguments arrive but before completion prevents execution. |
| STOP-03 | P0 | Stop between tools preserves completed work and prevents later tools in the batch/turn. |
| STOP-04 | P0 | Stop during an active shell command prevents later actions; UI clearly distinguishes cancellation from process termination. |
| STOP-05 | P0 | Start another turn immediately after Stop; late old worker cannot append, clear or finalize the new turn. |
| STOP-06 | P0 | Stop one session while another runs; only the intended session is interrupted. |
| STOP-07 | P1 | Repeated Stop and Stop concurrent with completion are idempotent; exactly one coherent terminal state remains. |
| STOP-08 | P0 | Delete/archive/clear a running task cannot let the worker resurrect it or continue unintended actions. |
| STOP-09 | P1 | Restart after Stop preserves completed transcript and interruption status without replaying tool actions. |
| STOP-10 | P1 | Cancelled waiting requests retire within the declared resource deadline; no accumulating workers or sockets. |

### Session persistence and recovery

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| DATA-01 | P0 | Restart retains user/assistant/tool messages, order, IDs and chosen model without duplicates. |
| DATA-02 | P1 | Rename/star/archive/unarchive reflect in list and detail and survive restart. |
| DATA-03 | P0 | Delete persists; later update/get/restart never recreates the deleted session. |
| DATA-04 | P1 | Search and recents find the right session/content with Unicode and identical titles. |
| DATA-05 | P0 | Crash during a write preserves the last valid data; partial records are handled without deleting unrelated sessions. |
| DATA-06 | P0 | Storage full/read-only/permission error is visible; UI never claims data was saved when persistence failed. |
| DATA-07 | P1 | Corrupt one transcript/config while keeping another valid; damaged item is isolated with a recovery path. |
| DATA-08 | P0 | Relaunch a formerly running session; state is interrupted/recoverable, not eternally running or auto-replayed. |
| DATA-09 | P1 | Supported profile upgrade preserves data; future/unsupported format produces a recoverable error. |
| DATA-10 | P0 | Both LocalAgentModeSessions and LocalSessions satisfy the same lifecycle and cancellation invariants. |

### Workspace and Code

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| WORK-01 | P1 | File tree/list/filter show fixture contents and correctly handle empty, hidden and large directories. |
| WORK-02 | P1 | Git status, branch, commits and diff match an independent `git` query on the local fixture. |
| WORK-03 | P1 | Staged/unstaged/conflict/untracked/deleted/binary changes are correctly distinguished in the UI. |
| WORK-04 | P0 | Shell/PTY runs in selected cwd, not build root or another session; environment contains no real credentials. |
| WORK-05 | P1 | PTY input/output, Unicode, resize and process exit remain usable; closing session retires owned PTY. |
| WORK-06 | P1 | Source/doc viewers open the selected file and show missing/unreadable errors without changing content. |
| WORK-07 | P1 | CLI transcript import/search uses only the fixture home, tolerates malformed files and does not duplicate imports. |
| WORK-08 | P0 | Explicit destructive Git operations in eligible mode affect only a local test repository and preserve declared boundaries. |

### Browser

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| WEB-01 | P1 | Open/close pane and create/select/close tabs; active title/URL/history match visible content. |
| WEB-02 | P1 | Navigate local page, follow link and go Back/Forward; fixture server records the correct requests. |
| WEB-03 | P1 | Form input, submit and click references affect the intended live page, not only a synthetic DOM representation. |
| WEB-04 | P1 | Dynamic DOM update invalidates stale references safely; no click on an unintended element. |
| WEB-05 | P1 | Redirect, unreachable host, bad TLS and server error remain understandable and recoverable. |
| WEB-06 | P0 | Browser page and app renderer remain separate privilege contexts; adversarial page cannot control the app. |
| WEB-07 | P1 | Download goes to approved fixture path with correct bytes; cancel does not leave a misleading completed download. |
| WEB-08 | P1 | WebFetch navigates the intended tab/pane and returns actual title/text with accurate errors. |
| WEB-09 | P2 | Persistence policy for tabs/cookies/history is honored on restart; separate profiles do not share them. |
| WEB-10 | P1 | JavaScript-dependent page and screenshots agree with displayed content; unsupported evaluation is explicit. |

### Native computer use and MCP

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| NATIVE-01 | P1 | TCC allowed/denied status matches real runner state; denial gives actionable guidance. |
| NATIVE-02 | P0 | GUI click/typing changes only the disposable target app; target identity and counter prove the effect. |
| NATIVE-03 | P1 | Screenshot/zoom returns valid image dimensions and corresponds to the current controlled desktop. |
| NATIVE-04 | P1 | Retina scaling, secondary display, window move and focus change do not misroute input. |
| NATIVE-05 | P0 | Unexpected target/focus loss fails or revalidates before typing; no input reaches another app. |
| NATIVE-06 | P1 | Native file/folder picker select/cancel and save panel behave correctly with substitutions disabled. |
| NATIVE-07 | P1 | MCP initialize/list/call, error, timeout, crash and restart are visible and leave no orphan child. |
| NATIVE-08 | P0 | Model tool cannot call an unconfigured/disconnected external MCP server through a permissive fallback. |
| NATIVE-09 | P1 | Chrome native messaging helper handles frame lengths, partial reads and errors against a fixture client. |
| NATIVE-10 | P2 | Actual browser extension/helper connection is qualified on a dedicated profile separately from helper protocol tests. |

### Projects and organization

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| PROJECT-01 | P1 | Create/rename/open a project through UI; list, detail and persisted ID agree. |
| PROJECT-02 | P1 | Add/remove folder and instruction document; next relevant turn receives correct context. |
| PROJECT-03 | P0 | Two projects with conflicting instructions/files remain isolated by selected scope. |
| PROJECT-04 | P1 | Attach/detach task to project/space updates navigation and survives restart without duplication. |
| PROJECT-05 | P0 | Delete project follows declared cascade/retention behavior and cannot silently delete unrelated files. |
| PROJECT-06 | P1 | Empty/error states and unsupported project operations are explicit rather than successful no-ops. |

### Artifacts and documents

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| ART-01 | P1 | Create/open artifact with known nonce; list and viewer show the same content. |
| ART-02 | P1 | Rename/star/delete/persist artifact changes the intended item only. |
| ART-03 | P1 | HTML/chart/diagram resources load offline from the relocated bundle without a development checkout. |
| ART-04 | P0 | Untrusted artifact script cannot access profile data, EIPC, arbitrary files or application automation. |
| ART-05 | P1 | PDF/DOCX/PPTX/CSV/image fixtures display known content without blank, frozen or wrongly selected views. |
| ART-06 | P1 | Export/download yields correct format, bytes and destination; cancelled save leaves prior file unchanged. |
| ART-07 | P1 | Invalid/large/unsupported document fails recoverably without corrupting the session. |
| ART-08 | P2 | Zoom/scroll/resize/popout and returning to chat preserve selection and accessibility. |

### Scheduled tasks

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| SCHED-01 | P1 | Create/edit task stores exact prompt, workspace, model, schedule and permission mode. |
| SCHED-02 | P1 | Enable/disable/delete persist and update list/detail without phantom tasks. |
| SCHED-03 | P0 | Trigger a due task using a controlled clock; one real run occurs and UI shows the corresponding result. |
| SCHED-04 | P0 | Disabled/deleted/not-yet-due task cannot run; restart/duplicate tick cannot double-execute a due occurrence. |
| SCHED-05 | P1 | Timezone and DST transitions follow an explicit skip/repeat policy; next-run display agrees with execution. |
| SCHED-06 | P1 | App asleep/offline/closed at due time follows the declared missed-run policy, not an accidental catch-up storm. |
| SCHED-07 | P0 | Scheduled tool actions use saved permission/workspace scope and cannot inherit a different session's Auto mode. |
| SCHED-08 | P1 | Provider failure and running-task cancellation produce a truthful failed/interrupted run record. |

Scheduled execution cases require an actual scheduler and clock seam. Current task CRUD and cron storage are not evidence that due-time execution exists. Keep unavailable execution cases as explicit capability gaps, with tracked issues; do not silently skip them in release reports.

### Settings and customization

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| SET-01 | P1 | Locale/theme changes apply and survive restart without resetting provider or session settings. |
| SET-02 | P1 | Skills/plugins/connectors list reflects fixture configuration; enable/disable affects the next turn. |
| SET-03 | P0 | Invalid extension/plugin input cannot execute installation hooks or modify arbitrary paths. |
| SET-04 | P1 | Broken MCP configuration is isolated and repairable through the UI. |
| SET-05 | P1 | About version/build information matches the actual bundle and test manifest. |
| SET-06 | P2 | Unsupported settings are visibly unavailable; no toggle claims success while changing nothing. |

### Accessibility and visual behavior

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| UX-01 | P1 | Keyboard-only user can start/stop a task, change mode, choose provider and dismiss dialogs. |
| UX-02 | P1 | Focus enters/leaves overlays predictably; Escape/cancel restores prior focus; no keyboard trap. |
| UX-03 | P1 | Streaming output and terminal status have useful accessible names without repeated announcement storms. |
| UX-04 | P2 | English plus long-text, CJK and RTL locale samples preserve labels, controls and layout; all shipped locales load. |
| UX-05 | P1 | Tool headers and consecutive rounds do not overlap, duplicate or falsely report actions, including during animation. |
| UX-06 | P1 | Composer and Stop remain usable at minimum supported window size and increased text size. |
| UX-07 | P2 | Light/dark/high-contrast/reduced-motion states remain readable and preserve functionality. |
| UX-08 | P1 | Screenshots at fixed sizes show no blank panels, clipped dialogs or inaccessible permission controls. |

### Responsiveness and durability

| ID | Priority | Scenario and required result |
| --- | --- | --- |
| PERF-01 | P1 | Cold/warm launch reaches usable composer within the agreed hardware-specific budget. |
| PERF-02 | P1 | During 30 seconds of thinking, typing, navigation and Stop remain responsive. |
| PERF-03 | P1 | Finite healthy animation progresses naturally; stalled animation recovers once; no create/finish loop. |
| PERF-04 | P1 | 100 consecutive local turns cause no unbounded memory, worker, socket, tab or helper growth. |
| PERF-05 | P1 | Large transcript/file tree stays usable; scroll and send are bounded and output does not freeze the UI. |
| PERF-06 | P0 | Renderer crash or app kill while a task runs cannot replay side effects automatically on restart. |
| PERF-07 | P1 | Sleep/wake, offline/online and fixture restart recover without duplicate requests or stuck task state. |
| PERF-08 | P1 | Broken automation bridge cannot turn a hung renderer into a passing test through a healthy Rust process. |

## Claude UI exploration additions

Interactive exploration of Claude for Mac **4.29.0 (260974)** identified the following additional cases. The [observation register](CLAUDE_UI_EXPLORATION.md) records the inspected controls, exploration limits and provenance. The reference build is newer than QuixiDesk's bundled renderer baseline. These cases are proposed requirements, not results from QuixiDesk or proof that every reference feature should be copied.

**Core** means a correctness boundary for existing app concepts. **Conditional** applies when the feature is implemented/advertised. **Policy** requires an explicit product decision before the expected behavior becomes a gate. Every row has a priority and observable oracle. Fixture-based failure variants are designed from the inspected UI, not claimed as behavior exercised in Claude.

### Composer context and effort

Evidence: OBS-02, OBS-03, OBS-05, OBS-12. These extend CHAT, PROV, PROJECT and NATIVE with the concrete context-selection states the original catalog did not specify.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| COMP-01 | P1 | Core | Select several fixture attachments, including duplicate names from different folders. Chips retain identity, processing state and remove action; next request contains exactly the intended ready attachments. |
| COMP-02 | P0 | Core | Remove one attachment or cancel a nested picker before Send. Its bytes/path never enter the provider request, project context or later session; earlier draft and selections remain intact. |
| COMP-03 | P1 | Core | Send during slow attachment extraction; either wait for completion or disable Send with a clear reason. Unsupported, oversized and failed files expose recoverable per-file errors without discarding good attachments. |
| COMP-04 | P1 | Core | Paste an image/file and drag/drop the same fixture. Equivalent content produces equivalent context; dragging over a non-drop target does not navigate the app or send a prompt. |
| COMP-05 | P1 | Conditional | Open a nested skill menu by pointer and keyboard. Right/Left and Escape enter/leave one level at a time, restore focus, and never accidentally select the next item or invoke a skill. |
| COMP-06 | P1 | Conditional | Choose a prompt-scoped skill/design system/plugin versus its Manage/Browse action. Only the chosen context enters the next request; viewing a management page neither installs nor invokes it. |
| COMP-07 | P1 | Core | Select a project/folder, dismiss the chooser, navigate away/back, then Send. Context chips, selected root and independent tool cwd agree; cancelling leaves the prior selection unchanged. |
| COMP-08 | P1 | Conditional | Ordinary Enter selects a context item; the supported alternate default shortcut changes only the default. Changing the default must not run a task, submit the draft or retarget an already-running session. |
| COMP-09 | P1 | Conditional | Change model and effort independently. Valid effort reaches the provider request and survives the declared scope; an unsupported effort on another model gets an explicit reset/validation state, never a stale hidden value. |
| COMP-10 | P1 | Conditional | Toggle Research and move between ordinary chat, project and Code composers. Mode, tool budget and UI indication agree; unsupported surfaces cannot silently inherit a prior Research request or extra cost. |

### Voice and dictation

Evidence: OBS-06 and OBS-15. Use a synthetic audio source and a dedicated runner account; do not record ambient user audio.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| VOICE-01 | P1 | Conditional | Choose system default versus a named input. Device identity follows the selected policy on restart/hot-plug; missing device is explicit and does not silently capture from another source. |
| VOICE-02 | P0 | Conditional | Deny microphone access, close the chooser or cancel recording. No audio/transcript upload occurs and no prompt is submitted; existing draft survives. |
| VOICE-03 | P1 | Conditional | Dictate a known fixture phrase into an existing multiline draft. Text is inserted exactly once at the documented cursor/replacement position; it is not automatically sent. |
| VOICE-04 | P1 | Conditional | Start/stop dictation twice, handle empty audio and a transcription error. Recording indicator and input state settle; capture resources are released and retry does not duplicate text. |
| VOICE-05 | P0 | Conditional | Switch session/window or quit while recording. Capture stops or remains visibly bound to its original destination; audio cannot land in a different conversation or continue invisibly after cancellation. |
| VOICE-06 | P1 | Conditional | Distinguish voice conversation, dictation and push-to-talk shortcuts. Each opens its own intended mode, handles key release/cancel and avoids triggering Send/newline or a hidden background recording. |

### Discovery and staged setup

Evidence: OBS-10 and OBS-11. Extend SET-02/03/04 with user-intent boundaries and staged configuration. Installation/authorization tests use disposable fixtures and controlled authorization services.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| DISC-01 | P1 | Conditional | Switch Yours/Discover and Skills/Connectors/Plugins with active query and filters. Results, counts and selected tab match the declared per-view persistence policy; no installed/discovery identity mix-up. |
| DISC-02 | P1 | Conditional | Search/filter/sort a deterministic catalog; return zero results, loading, offline and malformed-card variants. Empty/error states remain distinct; clearing restores the correct catalog without installing anything. |
| DISC-03 | P0 | Conditional | View a detail card or navigate a category versus Add versus Add-and-try. Only explicit approved installation changes the installed set; Add-and-try uses the selected item once, not a similarly named catalog entry. |
| DISC-04 | P0 | Conditional | Cancel upload/create/record-skill setup at every supported stage. No partial install, background screen capture, hook execution or prompt submission remains. |
| DISC-05 | P1 | Conditional | Custom connector step 1 validates required name and supported URL before continuation. Editing invalid fields does not erase valid input; empty form cannot advance. |
| DISC-06 | P0 | Conditional | Cancel before authorization in a staged custom-connector flow. No server contact, credential grant, saved partial connector or duplicate record occurs unless explicitly disclosed by the chosen design. |
| DISC-07 | P0 | Conditional | Controlled authorize/deny/timeout/back/retry sequence binds returned credentials to the original connector/profile and state nonce. Returning from another connector's callback cannot authorize the wrong entry. |
| DISC-08 | P1 | Conditional | Revoke/disconnect or fail a dependency of an installed skill/plugin. UI distinguishes installed from usable; composer/task capabilities refresh without a stale enabled tool. |
| DISC-09 | P0 | Conditional | Catalog descriptions and remote icons are untrusted data. Rendering a card or detail cannot execute scripts, alter approval policy or leak a profile identifier to an unapproved resource host. |

### Artifact gallery and entry points

Evidence: OBS-07 and OBS-17. These complement artifact execution/export tests with discovery, ownership and accessibility behavior.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| GALLERY-01 | P0 | Conditional | All/Yours/Shared with you use fixture ownership and access grants. Item visibility, detail access and search enforce authorization independently; moving tabs cannot expose another account's private item. |
| GALLERY-02 | P1 | Core | Combine query, owner, type, date, sharing and pinned filters. The visible intersection and counts match fixture data; clearing one constraint preserves the others. |
| GALLERY-03 | P1 | Core | Switch list/card views under active filters/sort and with a selected item. IDs, order, selection and open target remain identical; layout choice does not change data. |
| GALLERY-04 | P1 | Conditional | Pin/unpin and hide-from-recents are distinct from deletion. Hidden item remains in authorized gallery/search; pin state persists without duplicate cards or unexpected sharing changes. |
| GALLERY-05 | P1 | Conditional | New Chat/Docs/Slides/Design/Dashboard launches the appropriate empty draft and capability state. Merely opening New or focusing a type card cannot create/save a hidden artifact. |
| GALLERY-06 | P1 | Conditional | Card keyboard entry, internal Start-with-prompt control and Escape return preserve focus and accessible naming; pressing Enter on an inner action cannot also open/create the parent card. |
| GALLERY-07 | P1 | Core | Open search results with identical titles and different artifact IDs/types. Detail, action menu, pin and export address the selected ID even after sorting or metadata refresh. |
| GALLERY-08 | P2 | Conditional | Dismiss a feature announcement then revisit/restart. Announcement lifetime follows the documented policy; dismissal does not hide artifacts, change selection or unexpectedly reopen an external homepage. |

### Scheduled task configuration and triggers

Evidence: OBS-08 and OBS-09. Execution cases remain conditional on actual scheduler/trigger support. Use local webhook/API fixtures and notification sinks, not personal services.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| TRIGGER-01 | P1 | Core | Manual setup and template setup initialize exact prompt, frequency/time, model and permission scope. Editing a template must not change the reusable template or silently inherit the unrelated chat's defaults. |
| TRIGGER-02 | P0 | Core | Close/Cancel a task draft, including after opening advanced tabs. No persisted task, due occurrence, webhook registration, notification subscription or provider request exists afterward. |
| TRIGGER-03 | P1 | Conditional | Switch manual setup to setup-with-assistant and back. Define which draft fields transfer, preserve supported values, and require an explicit final Create; switching modes cannot schedule a task. |
| TRIGGER-04 | P1 | Core | Native frequency/time controls handle keyboard entry, locale, 12/24-hour display and invalid hours/minutes. Serialized schedule/next-run timezone match the visible value without noon/midnight inversion. |
| TRIGGER-05 | P0 | Conditional | Add/remove multiple triggers. One occurrence has a stable deduplication key; overlapping schedule/webhook/API deliveries cannot double-run the same occurrence under the declared policy. |
| TRIGGER-06 | P1 | Conditional | Switch trigger type between Schedule/GitHub/API. Show the correct validation/configuration fields, remove inactive secrets/endpoints, and reject an incomplete active trigger. Only the type menu was observed; these deeper forms need implementation-specific inspection. |
| TRIGGER-07 | P0 | Conditional | Signed webhook/API trigger rejects invalid auth, replay, wrong task/profile and deleted/disabled task. Accepted requests create one attributable run and cannot override stored permission/workspace scope. |
| TRIGGER-08 | P0 | Conditional | Require-this-computer and repository/project choices agree with run eligibility and session type. An unavailable local machine cannot be silently replaced with a cloud run with different access; repository selection cannot retarget another task's checkout. |
| TRIGGER-09 | P0 | Conditional | Task-scoped connector availability is enforced at execution. No-connectors empty state is truthful; permission revoked after scheduling prevents access and yields an understandable run failure. |
| TRIGGER-10 | P0 | Conditional | Notification master switch and per-channel choices produce the specified number of messages only to fixture sinks. Disabled channels send none; retries deduplicate delivery, and summaries contain no unintended secrets or full transcripts. |

### Code launch and workspace layouts

Evidence: OBS-13 and OBS-14. Preserve the distinction between a local folder, a named environment, a cloud environment and a project.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| CODEUI-01 | P0 | Core | Switch main/Code mode with unsent drafts and selections. Each mode restores its own context, model, effort and permission policy; no draft is submitted or sent to the other environment. |
| CODEUI-02 | P0 | Conditional | Search/select a local or cloud environment with similar names. Request routing, displayed environment, cwd and credential scope agree; an offline environment cannot silently fall back to local execution. |
| CODEUI-03 | P1 | Conditional | New/edit-environment submenu and cancel preserve the original environment selection and draft; inspecting configuration cannot launch a session or connection. |
| CODEUI-04 | P0 | Conditional | Worktree mode creates an isolated worktree only at the documented commit point. Cancelling launch creates no abandoned branch/worktree; active worktree writes cannot modify the main checkout. |
| CODEUI-05 | P0 | Conditional | Worktree folder policy handles changed location, disk full, stale branch, missing repository and uncommitted work. Archive/cleanup never removes unpushed or uncommitted fixture changes contrary to the retention policy. |
| CODEUI-06 | P1 | Core | Terminal/Browser/Files panels can open, switch and close without losing prompt, session identity, selected file or running process state. Cross-panel actions target the currently bound session. |
| CODEUI-07 | P1 | Conditional | Keyboard tile movement/split preview commits only on Enter, cancels on Escape, and keeps focus/selection. Preview must not duplicate the session, terminal process or provider turn. |
| CODEUI-08 | P1 | Conditional | Repository-grouped recents, group/sort/show, new section and reset-to-defaults preserve session identity and the documented preference scope independently from main-mode recents. |
| CODEUI-09 | P1 | Conditional | Automatically linked sessions for unrelated tasks follow the configured ask/stay/link policy. Linked task context, permissions and history are explicit; declining cannot silently spawn a second agent. |

### Desktop settings and native lifecycle

Evidence: OBS-01, OBS-14 and OBS-15. Run setting changes and native app-control qualification only on dedicated accounts/VMs with known restoration procedures.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| DESK-01 | P1 | Core | Native Settings shortcut/menu opens one correct window/overlay; search routes to the intended section, Close restores focus, and native/web About versions agree with the packaged binary. |
| DESK-02 | P1 | Conditional | Configured quick-access shortcut opens one small composer; repeated invocation, Escape and focus restoration work without duplicate windows, task submission or clipboard loss. |
| DESK-03 | P1 | Conditional | Voice/dictation/quick-access shortcuts detect conflicts and respect enablement. Changing a shortcut does not leave both old and new registrations active or intercept unrelated typing. |
| DESK-04 | P1 | Conditional | Menu-bar visibility, window close, hide, reopen, login startup and Quit follow the declared lifecycle. Test process exit versus background residence explicitly; closing a window must not falsely imply tasks stopped. |
| DESK-05 | P0 | Conditional | Change storage location with existing artifacts/tasks and running work. Migration verifies contents, handles permissions/disk-full/interruptions, rolls back safely, and never loses or leaves misleading split copies of saved work. |
| DESK-06 | P1 | Conditional | Keep-awake policy applies only while the relevant app/task conditions hold, respects battery policy and releases the assertion after completion/Stop/Quit. Tests verify OS assertions rather than relying on a switch label. |
| DESK-07 | P0 | Conditional | Denied-app list and background/full-control modes enforce the declared app boundary. Acquiring full control requires the chosen consent policy; denial blocks the action and cannot be bypassed with aliases or another tool path. |
| DESK-08 | P0 | Conditional | After success, failure, Stop or target-app crash, release input control and restore previously hidden test windows as promised. No global key/mouse capture or hidden-window state remains stuck. |
| DESK-09 | P0 | Conditional | Remote-control default-off, eligible-folder listing and inbound session settings enforce explicit opt-in and folder scope. Disabled mode cannot publish machine/folder metadata or start an inbound agent; test with an isolated fixture service. |

### Typed search and list preferences

Evidence: OBS-16. Search predicates and command actions need separate oracles; use synthetic fixture names and never rely on real recent conversations.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| FIND-01 | P1 | Core | All/Artifacts/Sessions/Code/Projects/Scheduled filter query results by type and authorized identity; same-title results retain their correct icon, route and action target. |
| FIND-02 | P1 | Core | Left/Right type changes, Up/Down selection, Escape and supported alternate action shortcut operate the palette without modifying composer text or firing the default action twice. |
| FIND-03 | P1 | Core | Rapid query/type changes with out-of-order server replies cannot replace current results with stale ones. Empty/loading/error states and selected result remain coherent. |
| FIND-04 | P0 | Core | Search navigation and command actions have distinct intent. Opening a result must not execute a command, create a task or submit a prompt; destructive actions still require their own product approval path. |
| FIND-05 | P1 | Core | Recents status/time/group/sort combine correctly and remain independent of search filters and Code grouping. Archived/running/unread fixtures appear in the intended views without duplication. |
| FIND-06 | P2 | Core | Close/reopen search and move between modes with active filters. Remembered versus reset query/selection matches the documented policy and focus returns to the original control. |

### Privacy and scope decisions exposed by the reference UI

Evidence: OBS-04, OBS-14, OBS-15 and OBS-17. Decide these contracts before claiming parity; UI wording alone is not a security guarantee.

| ID | Priority | Scope | Scenario and required result |
| --- | --- | --- | --- |
| PRIV-01 | P0 | Policy | Define incognito persistence, history, memory, telemetry and provider transmission separately. Test absence from the declared local/shared surfaces after close/restart, and explicitly disclose any external provider request that still occurs. |
| PRIV-02 | P0 | Policy | Define browser cookie scope: shared, per session or until quit. Controlled login cookies follow that scope across tabs/sessions/restarts; a supposedly isolated session never inherits another session's credentials. |
| PRIV-03 | P0 | Policy | Define allowed-site grant scope across origin, port, scheme, redirect and session lifetime. A grant to one fixture origin cannot silently authorize another; revoke takes effect before the next action. |
| PRIV-04 | P0 | Policy | Manual/Auto/bypass labels describe the actual contract. If QuixiDesk keeps blocking rather than per-action approval, its copy must not promise an approval dialog; any future allow-once/site/tool grant has bounded, revocable scope. |
| PRIV-05 | P1 | Conditional | Usage banners, reset times and model/effort estimates remain consistent with fixture limits. Exhaustion disables or explains unavailable work without silently charging another provider, switching account or dropping the draft. |
| PRIV-06 | P0 | Conditional | Auto-archive/PR lifecycle settings preserve running sessions and uncommitted/unpushed work under the chosen retention policy. Simulated merge/close events cannot trigger destructive cleanup of the wrong worktree or branch. |

### Additional execution specifications

These extend the original twelve specifications without expanding the initial 24-case smoke gate automatically.

**SPEC M Nested attachment cancellation.** In a fixture project-creation draft, enter a unique name/description, open Add context, then cancel the real native Open sheet. Verify the web form, draft values and focus are intact; no upload request, context object or project exists. Repeat Escape on the outer form and reopen it according to the documented draft-retention policy. Run with native substitutions disabled. Covers COMP-02, COMP-07 and the nested-modal gap in NATIVE-06/PROJECT-02.

**SPEC N Gallery filtering and typed search.** Seed artifacts with the same title but distinct IDs, ownership, type, date and pin state; include one inaccessible private fixture. Combine gallery predicates, switch list/card view, and check IDs/counts independently. Open typed search, select Artifacts, rapidly change query while responses are delayed, and navigate to one result. Verify only the selected authorized ID opens and no action was executed. Covers GALLERY-01/02/03/07 and FIND-01/03/04.

**SPEC O Unsaved task and delivery boundaries.** Open manual scheduled-task setup and choose task model/permissions, frequency and multiple supported triggers. Inspect General/Connectors/Notifications while preserving draft values, then Cancel. Verify no scheduled record, trigger registration, provider request or notification subscription exists. Separately create a fixture task explicitly, then exercise disabled-channel suppression and deduplicated delivery using local sinks. Covers TRIGGER-01/02/04/05/09/10. Trigger/notification behavior is conditional on implementation, not established by the reference form.

**SPEC P Environment and worktree identity.** Create two disposable local repositories and a fixture remote environment with the same display name. Select one, enable worktree mode, open/switch Files and Terminal panels, and start a controlled tool turn. Verify the exact environment ID, cwd, branch/worktree and filesystem effect; cancel a second draft launch and verify no branch/worktree/session was created. Add a denied/offline remote variant that must not fall back to the local machine. Covers CODEUI-01/02/04/05/06.

**SPEC Q Browser and app permission scope.** On a dedicated desktop, use two local origins and a disposable target app. Exercise the chosen site-cookie/app-control policy, revoke the grant while a provider response is delayed, then release the response. Confirm no late unauthorized action, no credential leakage to the other scope, and complete input/window restoration after Stop. Covers PRIV-02/03, DESK-07/08 and existing PERM/STOP invariants. The test policy must be selected before implementation; it must not inherit the developer account's grants.

### Rollout of the exploration additions

Add **COMP-02, FIND-03 and FIND-04** to the next expanded smoke gate once their fixture/UI prerequisites are implemented. Keep the initial 24-case gate unchanged until its measured runtime and driver stability are known. Prioritize Core cases plus explicit permission/privacy wording decisions first. Add Conditional cases as their capabilities ship; report absent advertised features as gaps.

Discovery/install, real voice, remote control, storage migration, worktree cleanup and notification tests belong on controlled infrastructure. Do not activate them on a personal account merely to complete a reference-UI checklist. Provision dedicated fixtures and permissions, and require independent side-effect oracles just as in the original suite.


## Detailed specifications for the first release gate

These are executable specifications for the initial critical paths. Use controlled barriers; the sample durations below are deadlines, not sleeps.

### SPEC A Launch and configure a fresh app

**Cases:** BOOT-02, PROV-01, PROV-02, PROV-05.

1. Create empty profile and workspace; start local provider with zero received requests.
2. Launch exact packaged app, verify run/build/profile handshake, and wait for native window plus onboarding.
3. Attempt completion without configuration. Assert actionable setup state and zero external requests.
4. Enter local Custom Endpoint and model through provider UI. Test connection using a distinct fixture scenario.
5. Save/set active; return to workspace; select model. Send a unique prompt.
6. Server asserts model and user message, emits two deltas and success. UI shows progressive text and final reply.
7. Assert exactly the allowed test-connection and chat requests, no tools, no unexpected files and idle task state.
8. Relaunch without resetting the profile. Assert saved provider/model and one persisted conversation.

### SPEC B Manual blocks mutations and permits reads

**Cases:** PERM-01, PERM-05, PERM-06, TOOL-11.

Seed `readable.txt`, `protected.txt`, a counter and filesystem manifests. Start a Manual task. Fixture requests Read, Bash append, Write overwrite, Edit, GUI left-click and equivalent MCP filesystem mutation, each with a distinct call ID.

Assert Read returns known bytes; each mutation receives an error/blocked result; protected hashes and counter remain unchanged; the native target app counter stays zero; no child shell starts. The UI must indicate denied operations rather than presenting them as completed work. Repeat representative names through UUID and case aliases. A missing argument must fail before asking for elevated permission.

If the product adds explicit per-action approvals, extend this spec with allow once, deny, cancel, stale approval and scope lifetime. The current policy instead requires switching the session to Auto; the suite must not invent approval UI.

### SPEC C Auto and mode changes are coherent

**Cases:** PERM-02, PERM-03, PERM-04.

Start with Auto selected before first prompt. Assert returned session metadata, pill and persisted mode all say Auto. The local provider requests a harmless file append and the controlled GUI counter action; each happens once. Change to Manual through the current session pill; verify `permission_mode_changed` and saved state. Send the same action in a new turn; assert denied, counter unchanged and no stale Auto badge.

For the race variant, hold the response just before a valid tool completion, change Auto → Manual, then release the response. The decision must read the updated mode. Restart and check the intended mode again.

### SPEC D Stop before provider completes

**Cases:** STOP-01, STOP-02, STREAM-11.

1. Start Auto task whose provider emits thinking, then waits at `before_tool_completion`.
2. Wait until the actual Stop control is enabled and the fixture barrier has been reached. Verify sentinel file absent.
3. Click Stop with native input. Record `stop_accepted` time and assert interrupted UI, editable composer and inactive session.
4. Release a valid Write call and normal completion from the old provider request.
5. Wait for the old request to finish and worker-retired evidence. Assert file still absent, zero tool-start records and no appended assistant answer.
6. Reload transcript and restart app. Confirm interruption persists without running state or replayed action.

Never pass this case by checking immediately after Stop; the old response must be released and given an opportunity to trigger the bug.

### SPEC E Stop between tools and replace the turn

**Cases:** STOP-03, STOP-05, STOP-06.

Provider returns first disposable append. Hold execution or the next provider round at a defined barrier. After the first action completes, Stop; start a new turn with a different nonce before releasing the old second action. The second old write must never occur. The new turn completes normally. Assert separate IDs, exact provider request counts, one terminal event per turn and unchanged new running state when the old request finishes. Repeat while an unrelated session is running.

A tool already running at Stop is a different contract. Until shell process cancellation exists, it may finish, but no subsequent action may start. The UI and release notes must not promise process termination. Resource-retirement timeouts should expose blocked sockets or workers rather than waiting indefinitely.

### SPEC F Duplicate requests do not duplicate effects

**Cases:** TOOL-06, TOOL-07, TOOL-08, TOOL-09.

Turn 1 requests `append nonce-A`, then asks again with a new call ID and reordered JSON fields, mixed with a fresh Read. A same-batch variant repeats the same action twice. Assert file has one appended nonce, ledger has one execution, both required provider call IDs receive consistent results and the renderer records one executed action. No cached result is presented as a second shell execution.

Turn 2 deliberately requests the same append. Assert the file now has two entries. This prevents an overly broad cache from disabling legitimate future work. Also test error/denial caching against the documented policy and ensure a later turn can retry after permission changes.

### SPEC G Reject a truncated batch

**Cases:** STREAM-04, STREAM-05, TOOL-11.

Use a valid two-call baseline. Emit the complete first Write arguments, then partial second Write arguments and disconnect before successful completion. Assert zero files created and both original files unchanged. Repeat with `[DONE]` but no finish, `finish_reason: length`, malformed JSON, wrong argument types, missing call IDs, Responses incomplete/failed and a stream read error. The UI reports a recoverable provider failure, not a successful empty turn. A new valid turn must still work.

### SPEC H OAuth Responses tool loop

**Cases:** PROV-10, TOOL-05, DATA-01.

Local transport fixture receives an OAuth-shaped Responses request. Validate flattened function definitions, instructions, input messages, placeholder account identity and tool capabilities. Return a completed function-call item plus reasoning item. Validate that the next request preserves call IDs, original output/reasoning items and `function_call_output`, with no lossy conversion into a user message. Continue Write → Read and final text. Validate final UI and file bytes.

Repeat 401 → one refresh → success, expired refresh, malformed account identity, cancelled/incomplete response and interleaved reasoning/text. These are deterministic protocol tests. A separate small live qualification signs into a dedicated test account and verifies one harmless tool loop; do not infer live compatibility from fixtures alone.

### SPEC I Provider error belongs to the right turn

**Cases:** STREAM-06, CHAT-04, CHAT-10.

Complete a successful initial task, then send a follow-up. Server returns HTTP 503 with a unique error marker. Assert one additional generation request, a visible follow-up user message, a separate terminal error response and unchanged earlier answer. There must be no retry through the text-only fallback, duplicated error or success summary. Send a third prompt using a successful fixture and assert recovery with the right history.

A 401 refresh case has a different explicit request budget. Account for generation and refresh separately; “one request” is not a universal retry rule.

### SPEC J Multi-session persistence without leakage

**Cases:** CHAT-06, DATA-01, DATA-03, DATA-10.

Launch both session types against distinguishable workspace roots. Interleave provider deltas and tool calls. Rename one session, star another and stop only one. Reload both transcripts, relaunch, search by each nonce and verify modes, workspace, model and messages. Delete one, issue a stale update and restart again. The deleted task cannot reappear, and the survivor's files and transcript must be byte-correct.

### SPEC K Relocated shipping bundle

**Cases:** BOOT-04, BOOT-05, BOOT-07, ART-03.

Build/package in disposable checkout and synthetic home. Copy `.app` to another path with spaces, make the checkout unavailable and launch with `QUIXI_RESOURCES_DIR` unset and minimal PATH. Open chat, provider configuration, browser and a local artifact using bundled libraries. Run a harmless helper command. Assert required resources resolved inside the bundle, no source symlinks, correct version/signature and no unexpected downloads. Run once without any test instrumentation to detect dependence on test hooks.

Ad-hoc signature success is a local packaging check. It is not evidence of Developer ID signing, notarization or Gatekeeper distribution acceptance; add those gates when such distribution is supported.

### SPEC L Responsiveness during thinking and tool rounds

**Cases:** PERF-02, PERF-03, UX-05.

Fixture emits thinking over a controlled interval while the runner types into a draft, opens/closes an app-owned overlay and measures input acknowledgment. Capture animation samples before, during and after several tool rounds. Assert healthy animation progresses, zero-time stalled animation recovers at most once, labels never overlap at sampled transition points, and Stop works. Use renderer heartbeat plus native input acknowledgments and measured WebContent CPU; a responsive HTTP bridge alone is insufficient.

## Performance budgets and visual assertions

The following are initial acceptance budgets to calibrate on a named reference Mac. Record OS, architecture, display scale, build profile and fixture timings with every sample. Establish baselines before making budgets blocking.

| Measurement | Proposed initial budget |
| --- | --- |
| Cold launch to usable composer, after build | p95 at most 5 seconds |
| Local fixture first text visibly rendered after server flush | p95 at most 500 ms |
| Text-input acknowledgment during thinking | p95 at most 150 ms; no sample above 1 second |
| Stop click to interrupted UI | p95 at most 500 ms; at most 2 seconds hard deadline |
| New actions after Stop accepted | Exactly zero; not a percentile |
| Native fixture tool side effects | Exactly the approved execution count |
| Waiting provider worker after cancellation | Retires by the explicit transport/resource deadline; initially bound by current request timeout, with a tracked target for earlier cancellation |
| 100-turn memory trend | Investigate monotonic growth; initial gate: final quiescent memory at most 25% above warmed baseline, subject to measured calibration |
| Thinking-only CPU | No sustained near-one-core saturation from animation; provisional idle-thinking WebContent target below 20% of one core on reference hardware |

Avoid blanket pixel equality. Pin window dimensions, scale, theme and locale; mask only timestamps, random IDs and approved dynamic areas. Assert semantic layout first: text/card bounds do not overlap, controls are inside the viewport, headers have nonzero size, permission pill is visible. Then compare selected screenshots. Preserve animations in the transition suite; reduce motion only in its dedicated accessibility variant.

CPU percentages and memory need platform-aware measurement of the app, WebContent and owned helpers. Distinguish virtual memory, resident memory and cached allocations. Record samples around warm-up and quiescence so model/network latency is not confused with app regressions.

## Platform and dependency matrix

Run the primary deterministic suite on Apple Silicon with a pinned supported macOS/Xcode image. Run a smaller full-journey matrix on Intel where supported. Resolve the current minimum OS discrepancy between README prerequisites and bundle metadata before setting a release matrix; test the chosen minimum and current supported release explicitly.

Use pairwise combinations for OS × architecture × theme × locale × window size, not the full Cartesian product. Always run permission, cancellation and persistence invariants for both session types. Test transport families independently from provider catalogs: OpenAI-compatible, Responses OAuth and Anthropic-compatible. A shared transport fixture does not prove every vendor's current model/API support.

Live provider matrix: configured transport families, actual account/auth mode, one short text turn, one supported tool loop, error/expiry recovery when safely reproducible. Keep a spending cap and stop on unexpected action plans. Cloud outages are classified as external failures, not silently converted into product passes.

Feature-specific release qualification includes native pickers/TCC, real browser interactions, helper/extension connection, supported document viewers and actual scheduled execution if offered.

## CI gates and failure policy

### Proposed lanes

Start the application smoke gate with these 24 catalog cases: BOOT-02, BOOT-05, NAV-01, PROV-02, PROV-06, CHAT-01, CHAT-04, CHAT-06, STREAM-01, STREAM-04, STREAM-05, STREAM-06, TOOL-05, TOOL-06, TOOL-08, TOOL-11, PERM-01, PERM-02, PERM-03, STOP-01, STOP-05, DATA-01, DATA-03 and DATA-10. Parameterization produces more than 24 executions. BOOT-01 runs in the Rust/build lane. Expand the smoke selection when measured runtime allows; do not remove invariant checks merely to meet a duration target.

| Lane | Trigger | Scope | Target duration |
| --- | --- | --- | --- |
| Rust verification | Every PR | Existing contracts plus new state/transport regressions; build and all-targets checks | Under 5 minutes after cache warm-up |
| App smoke | Every PR with macOS UI runner | SPEC A–I reduced variants, J for both sessions, core navigation | Under 12 minutes |
| Full deterministic | Nightly and before release | All implemented bridge/app cases, persistence faults, provider matrix and long sessions | Under 45 minutes, measured before enforcement |
| Native qualification | Nightly subset; release full | TCC states, native dialogs, disposable target app, display/focus, shipping launch | Capacity-dependent; explicit deadline per case |
| Live providers | Scheduled and release | Small dedicated-account compatibility checks with spend cap | Separate from merge-time deterministic lane |

These are proposed lanes and targets; no such CI jobs are currently created by this design.

Each test runs serially within one interactive desktop. Parallelize protocol tests freely and app tests across isolated macOS VMs/accounts/hosts. Do not run multiple GUI drivers on the same desktop or assume dynamic ports isolate mouse/keyboard focus.

The runner records passed, failed, infrastructure-error, unsupported and quarantined separately. Unsupported/quarantined cases are never counted as passes. A release capability coverage report must show unavailable advertised behavior as a gap requiring a decision.

Do not automatically rerun a failing product test until green. One diagnostic retry in a fresh sandbox may classify a suspected infrastructure problem, but the first failure and artifacts remain visible. Never retry a potentially mutating step in the same sandbox. Quarantine requires owner, issue, expiry and compensating coverage; P0 authorization/data-loss failures block rather than being quarantined away.

Use explicit timeouts with graceful artifact capture and owned-process teardown. The top-level runner must return nonzero for a failed required assertion, timeout, missing required result or cleanup/isolation failure. Missing native permissions are an infrastructure failure, not a successful skip.

### Failure artifacts

Write one folder per case containing:

- Run/build manifest: source revision, binary/resource hashes, bundle path, platform, fixture versions and selected variants.
- Timestamped action log and per-step assertions, including exact unmet wait condition.
- Before/failure/after screenshots and accessibility tree; short video ring buffer for animation, focus and timing cases.
- Redacted provider requests/SSE timeline and dependency request counts. Preserve framing/byte boundaries needed for transport reproduction.
- Correlated EIPC, session, turn, tool, permission and cancellation events.
- Transcript and relevant persisted state; filesystem before/after diff and independent side-effect ledger.
- App/WebContent/helper PIDs, exit codes, crash reports and CPU/memory samples when relevant.
- Machine-readable results plus a concise HTML report linking failures to the relevant artifacts and reproduction command.

Do not put real tokens, Authorization headers, cookies, full personal transcripts or uncontrolled desktop screenshots into CI artifacts. Redaction is structural, has its own tests and runs before upload. Restrict screenshot capture to the controlled test desktop. Set retention policies, for example 7 days for passing runs and 30 days for failures, according to repository access policy.

## Implementation layout and rollout

Proposed repository layout:

```text
crates/quixi-test-support/     Rust fixtures, protocol servers and effect assertions
crates/quixi-engine/tests/    engine/transport integration regressions
crates/quixi-ipc/tests/       event and state integration coverage
tests/e2e/                    application journeys
   native/                   Xcode UI-test target and launch configuration
   scenarios/                named scenario definitions keyed by catalog ID
   fixtures/                 workspaces, websites, documents and old profiles
   reports/                  schema and report formatter
scripts/e2e/                 build, run, diagnose and owned-process cleanup
.github/workflows/           protocol, macOS smoke and qualification lanes
```

Actual runner commands should be introduced with the implementation, for example `make e2e-smoke`, `make e2e-full`, `make e2e-native` and `make e2e-live`. These targets do not exist yet. Existing verification remains `cargo test --workspace`, `cargo check --workspace --all-targets` and the package script in an isolated account/checkout.

### Phase 1 Make the harness safe and trustworthy

Implement profile/website-data isolation, configurable ports, run handshake, stable selectors, explicit readiness, app/process manifest, fixture server, teardown and nonzero failure exits. Separate timed diagnostics and substituted native dialogs from instrumentation. Add runner self-tests: wrong build, wrong profile, port collision, missing permission, fixture crash, hung renderer and teardown failure must all fail as intended.

Deliver one fresh-profile journey that passes through the composer and proves exact provider request and file bytes. Qualify native accessibility interaction before scaling the catalog.

### Phase 2 Protect the agent loop

Implement SPEC B–I for both session types, including interrupted streams, mode-change races, repeated calls, actual request budgets and blocked side effects. Land the regressions from issues #1/#4 and PR #3 as named, deterministic tests. Add the app smoke lane and require it for changes to engine, providers, IPC, preload or renderer patches.

### Phase 3 Protect saved work and the shipping app

Implement SPEC J/K, crash/persistence fault fixtures, clean-checkout packaging, relocated launch and core project/artifact/settings journeys. Add browser state versus real-rendering assertions and native picker qualification. Require package evidence for changes to scripts, resources and helper resolution.

### Phase 4 Broaden compatibility and soak

Add platform/localization/accessibility variants, display/focus/TCC states, live-provider qualification, long transcripts and 100-turn soak. Implement scheduled execution/timezone scenarios when the scheduler is available. Set measured performance budgets and nightly trend reporting.

Do not set a calendar estimate until the native driver and storage-isolation spikes are complete. The most useful first milestone is a small trustworthy gate, not hundreds of tests that only exercise API return values.

## Completion criteria for the suite

The suite is ready to enforce when a clean clone on a provisioned test Mac can run the smoke lane with no developer credentials, ignored directories, existing app process or manual setup. Each required case must produce a machine-readable result and a reproducible artifact bundle. The runner must detect deliberate injected regressions, including ignored Stop, wrong permission mode, duplicated execution, truncated-call execution, repeated HTTP fallback, wrong workspace and missing bundled resource.

For release, require all applicable P0/P1 cases, current native/platform evidence, explicit decisions for capability gaps, no new leak trend, and a successful relocated shipping-bundle run without behavioral test substitutions. A successful 1,400-channel sweep or green mocked-provider suite alone is insufficient.

## Source references

Repository anchors for implementing the harness:

- [`main.rs`](../../crates/quixi-app/src/main.rs): custom protocol, bridge, `QUIXI_E2E_TEST`, resource discovery and WebView setup.
- [`quixi-engine/lib.rs`](../../crates/quixi-engine/src/lib.rs): profile construction, turns, cancellation, scheduled task persistence and API routes.
- [`agent_tools.rs`](../../crates/quixi-engine/src/agent_tools.rs): permission decisions, execution and per-turn cache.
- [`providers.rs`](../../crates/quixi-engine/src/providers.rs) and [`provider_tool_responses.rs`](../../crates/quixi-engine/src/provider_tool_responses.rs): provider transport and completion validation.
- [`quixi-ipc/lib.rs`](../../crates/quixi-ipc/src/lib.rs): actual service operations, event names and existing tests.
- [`quixi-platform/lib.rs`](../../crates/quixi-platform/src/lib.rs): native permission/picker behavior and browser model.
- [`quixi-workspace/lib.rs`](../../crates/quixi-workspace/src/lib.rs): filesystem, Git, PTY and shell behavior.
- [`quixi-proto/paths.rs`](../../crates/quixi-proto/src/paths.rs): home/workspace defaults.
- [`package-macos-app.sh`](../../scripts/package-macos-app.sh): package/install side effects and resource strategy.

Apple documents passing per-launch environment variables through [`XCUIApplication.launchEnvironment`](https://developer.apple.com/documentation/xcuiautomation/xcuiapplication/launchenvironment). Use that facility to select the test profile and run identity; verify effective isolation inside the application rather than assuming environment configuration alone is sufficient.
