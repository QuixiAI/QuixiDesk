# QuixiDesk desktop experience audit

## Implemented

The selected conversation already controls the steering/follow-up buttons' `hidden` property in `resources/ui/app.js`. However, the global author `button { display: inline-flex; }` style overrides browser hidden styling. `resources/ui/app.css` now explicitly hides `.composer-tools button[hidden]`.

`tests/ui/original-ui.test.cjs` adds computed-visibility coverage using the actual stylesheet: idle/new task, run start, session switches, background updates, completion, error, restart and Stop.

## Follow-up implementation: truthful setup status

`crates/quixi-platform/src/lib.rs` now checks accessibility and screen capture access using native, non-prompting macOS preflight APIs. A negative check is `not_granted`, not an invented distinction between denied and not-yet-requested. Microphone and global automation remain `unknown`; automation access is target-specific.

Explicit permission-request IPC calls open the matching macOS Privacy settings pane and return a fresh status check, not assumed consent. They do not implement an in-app native microphone prompt; users must recheck after changing settings. Unknown methods and failed settings launches reject the IPC call.

`crates/quixi-ipc/src/lib.rs` now rejects unsupported Chrome extension installation detection, lifecycle and connection/activity tracking instead of returning fabricated success. The internal browser manager seeds a built-in/Chrome Bridge record, so it is not independent evidence of a live external extension. This does not implement external extension setup or alter the separate in-app browser tools.

Regression tests cover status mapping and full IPC error propagation, including protection against success-schema defaults. Automated tests do not open Privacy settings or request consent.

## Audit findings and recommended order (original survey; first item partially addressed above)

### P0: Truthful permission and browser setup status

`crates/quixi-platform/src/lib.rs:330` returns granted unconditionally for accessibility, screen recording, microphone and automation. `crates/quixi-ipc/src/lib.rs:2398` returns granted for permission-request methods without performing a request.

Nearby ChromeExtension installation/launch/restart methods return true or succeeded; the catch-all returns installed/connected. The connected-extension list does read actual browser state. These endpoints therefore cannot be used as proof of operational readiness.

Recommendation: native permission checks, explicit consent requests, settings links and rechecks. Distinguish unknown, denied, restricted and granted; unsupported operations must not report success. Keep in-app browser readiness distinct from external-browser attachment. Test absent/revoked permissions, missing extensions and disconnected browsers.

### P0: Screenshot scope does not match the app tool contract

A live `app_screenshot` request targeting QuixiDesk returned a display screenshot. `crates/quixi-mcp/src/lib.rs:2009` routes screenshot, app_screenshot, framebuffer_screenshot and preview_screenshot through the same `capture_screenshot_base64()` call. This branch does not use an app/window selector, although the app screenshot description promises background capture of a granted application window.

Recommendation: implement scoped window/preview capture or return unsupported. Never silently fall back to broader desktop capture. Test background windows, invalid/closed windows, multiple displays and Retina scaling. This is both a reliability and privacy concern.

### P1: MCP setup still exposes technical plumbing

`resources/ui/capability-workflows.js:60–160` asks for an MCP URL or executable and arguments; advanced environment/header settings require JSON. Validation, conflict preservation, enable/remove and sign-in guidance already exist. No presets or dependency checks were found in this editor; this is not a claim about their absence everywhere.

Recommendation: capability presets or package previews, prerequisite checks, validated settings fields, protected secrets and a clear test/repair action. Retain advanced configuration and explicit installation/trust review.

### P1: Connection readiness is not capability readiness

The reviewed capability UI defines Ready around initialization/discovery, not successful OS access or browser attachment. `resources/ui/app.js:2410–2450` hides non-catalog controls for built-in connections. Built-in management enhancement is skipped unless unsupported, leaving a recovery gap for other built-in failures.

Recommendation: distinguish connection health from actual capability health and provide supported repair actions. Tests should cover discoverable-but-unusable tools. Health checks must not silently perform disruptive actions.

### P1: Coding needs integrated change review

Searching the reviewed `resources/ui` sources found no diff-review integration. Backend Git diff, stats and file-content endpoints exist in `crates/quixi-ipc/src/lib.rs:1724–1764`.

Recommendation: task-level Changes panel, file diffs and test results, followed by explicit user-selected commit/revert operations. Preserve pre-existing user modifications. Test untracked/binary files, conflicts and concurrent task changes. This focused review does not establish complete Claude Code parity or workspace-wide absence of worktree support.

### P2: Summaries should distinguish attempts from unresolved failures

The observed UI prominently showed a failed-action count while work continued. `resources/ui/app.js:1369–1396` counts failed activity entries.

Recommendation: retain diagnostic history but separately summarize unresolved blockers and recovered attempts. Do not assume recovery merely because a later operation succeeded.

## Claude.app inspiration via REA

Inspected `/Applications/Claude.app/Contents/Resources/app.asar` using REA 6.7.0 static JavaScript analysis and supplemental bounded read-only archive excerpts. No Claude code was executed or copied into QuixiDesk.

Artifact SHA-256: `ce90267cb07b08d1e09c99be0e78abf39be2c06742a92367e54086b96f08caa3`.

Selected sources:
- `.vite/build/computerUseTeach.js`
- `.vite/build/computerUseWatchRecord.js`
- `.vite/build/claudePagePreview.js`
- `.vite/build/index.chunk-JbCN_A_h.js`
- `.vite/build/index.chunk-C49IFSIS.js`

Observed patterns worth adopting independently:
- Real microphone status checks, denied/restricted branches, System Settings links, permission requests and graceful recording without voiceover. Apply the check/explain/request/recheck pattern to QuixiDesk permission setup.
- MCP extension dxt/mcpb preview, manifest/signature information, validated installation and sensitive configuration handling. Make setup approachable without hiding trust decisions.
- Explicit computer-use working/show/hide and recording states. Make scope, progress and exit controls first-class rather than burying them in tool logs.
- Embedded preview bridge source/origin checks. Keep preview and desktop-control boundaries explicit.

REA evidence:
- `ev_e4e6cc0f2d75f9aabb0e90fc6d2c76cc065314ce1964e436a3f8bb2d38a6b70f`
- `ev_6db65f36c1abd36e0ab4598dc24a462a108ae70b346162ca63a318a7acc1eff4`

Limitations: initial integrity validation rejected unpacked native companions. Static inspection continued explicitly under record-and-continue, preserving mismatches as untrusted observations; their cause is unverified. Analysis was bounded and partial, with retention limits reached. These are implementation clues, not vendor integrity, runtime or security assurance. Claude window capture and Git-review implementations were not established by this selection.

## Verification and scope

- UI suite: 88 passed, 0 failed.
- `cargo build --workspace --locked`: passed with existing cocoa deprecation and block future-compatibility warnings.
- The running app was inspected but not restarted with patched assets. Native WKWebView visual verification remains outstanding; computed-style regression tests verify the visibility fix.
- The composer fix and the truthful-setup follow-up described above are implemented. Remaining recommendations, including scoped screenshots and guided onboarding, are not implemented.
- No deployment, commit or app restart was performed. This is a focused survey, not a complete feature-parity or fresh-install certification.
