# QuixiDesk maintenance handoff

## Current handoff: composer redesign

### Request and status

The user approved replacing the clunky “Steer now” / “Queue follow-up” composer, then requested this handoff. Implementation and automated verification are complete; changes remain uncommitted. No build was installed and the running application was not restarted. Do not commit, publish, deploy, quit, restart, or disturb existing conversations without a new request.

### Implemented behavior

- One primary button: **Send** when idle, **Queue** while running. Enter uses that action; Shift+Enter inserts a newline.
- Running-only native `<details>` menu offers **Guide current task**. Guidance is applied at a safe loop boundary, not by interrupting a tool.
- A separate **Stop** button is visible while running.
- Pending cards show **Up next**, **Guidance pending**, or **On hold**, with **Remove**, **Guide task**, or idle **Send now** actions as appropriate.
- Stop holds pending entries instead of deleting them. Explicitly sending one held entry does not release the others. Conversation deletion still removes its queue.
- Pending queues are **memory-only**; restart/crash persistence was not implemented.

### Implementation map

- `resources/ui/app.js`: primary send/queue control, guidance disclosure, separate Stop, pending-card rendering and RPC handlers. Handlers check returned error objects, refresh session state, and update only the selected conversation.
- `resources/ui/app.css`: disclosure/menu and compact pending-card styles, hidden rules, keyboard focus outline.
- `crates/quixi-engine/src/chat_workflows.rs`: cancel/promote pending messages, held-entry behavior and workflow tests.
- `crates/quixi-engine/src/lib.rs`: pending-message state and cancellation behavior.
- `crates/quixi-engine/src/storage.rs`: deletion explicitly locks submission, cancels the signal, and removes pending state.
- `crates/quixi-engine/src/steering_tests.rs`: wire-test expectations include `held:false`.
- `crates/quixi-ipc/src/lib.rs`: exposes pending-message operations for both `LocalAgentModeSessions` and `LocalSessions`.
- `tests/ui/original-ui.test.cjs`: fixture and tests for queue/guide/remove/hold/send-now behavior.
- `docs/composer-delivery.md`: concise interaction and delivery contract.

### Backend contract

- `getPendingMessages(sessionId)` returns entries including `messageUuid`, `text`, `delivery`, and `held`.
- `cancelPendingMessage(sessionId, messageUuid)` removes only a pending entry; returns `{cancelled, messageUuid}`. An already-consumed UUID returns `cancelled:false`; invalid/missing sessions error.
- `guidePendingMessage(sessionId, messageUuid)` promotes a running entry to steering and clears its held flag, or dispatches that selected entry as a new idle turn. Other held entries remain held. It rejects a running-but-cancelled worker still stopping and a missing entry.
- Cancellation marks pending entries held under the submission lock before signalling cancellation. Steering consumption and automatic next-turn dispatch skip held entries; submissions accepted while cancellation is set are held too.
- RPC failures currently return JSON error objects; callers must check `result?.error`.

### Latest verification

Rerun after the final UI test addition:

- `node --test tests/ui/*.test.cjs`: **90 passed, 0 failed**.
- `cargo test --workspace`: all workspace tests passed.
- `make`: release build passed. Existing Cocoa deprecation and `block v0.1.6` future-compatibility warnings remain.
- `git diff --check`: passed before this documentation update.

Logs from these runs are in `/tmp/quixi-ui-all.log`, `/tmp/quixi-tests.log`, and `/tmp/quixi-build.log` (temporary, not durable artifacts).

### Limitations and optional follow-up

- No live visual inspection of the redesigned running app was performed; verification was automated UI/DOM tests plus Rust tests and build. Use an isolated fixture/preview if inspecting visually, rather than restarting the user's app.
- The native disclosure has no explicit outside-click/Escape dismissal handler.
- Initial active-composer placeholder and the dynamically updated placeholder use slightly different wording; optional polish only.
- UI fixture tests exercise sending one held message while retaining another; additional backend-specific idle held-dispatch coverage would strengthen regression protection.
- Earlier Claude.app investigation observed static pending-message behavior, not its live running composer. This is an independent redesign, not a verified visual copy; no proprietary code was copied.

### Working conventions

Use relative repository paths, preserve unrelated edits, and keep Rust on edition 2021. The shell is fish; wrap shell heredocs in `bash -lc` or use file tools. Existing maintenance notes follow.

The active application consists of the Rust workspace and the original interface under `resources/ui`. Build and install commands for macOS, Linux, and Windows are in `README.md`.

## Main entry points

- `crates/quixi-app/src/main.rs`: native window, resource routing, trusted origins, and close handling.
- `crates/quixi-app/src/native_bridge.js`: main-frame transport and local automation bridge.
- `resources/ui/app.js`: workspace navigation, conversations, projects, providers, artifacts, and tools.
- `resources/ui/app.css`: original interface styles and responsive layout.
- `crates/quixi-engine/src/storage.rs`: atomic session saves, recovery, retry, and export.
- `crates/quixi-engine/src/providers.rs`: model-provider integrations.
- `crates/quixi-ipc/src/lib.rs`: workspace RPC services.

## Development checks

Run `cargo test --locked --workspace` with `QUIXI_TEST_PROFILE` pointing at a disposable absolute directory. Build all release helpers with `cargo build --locked --release --workspace`. Regenerate bundled task archives with `python3 scripts/package-bundled-skills.py` after editing their sources.

On Windows, `scripts/test-windows-storage.ps1` drives the native app through its application bridge and checks recovery and close behavior. It uses a fresh fixture profile and leaves JSON evidence and logs in a temporary output directory.

For macOS test packaging, set `QUIXI_APP_BUNDLE` to a separate output path and `QUIXI_INSTALL_HELPERS=0` to avoid modifying the normal installed bundle or helper executable. Never test destructive storage scenarios on the user's real account data.

The full proposed E2E case catalog is in `docs/testing/E2E_TEST_PLAN.md`. Proposed cases are requirements, not claims that every feature is implemented or every test has passed.

## Distribution

Ship the UI, bootstrap defaults, original skills, resource licenses, executable, and helpers together. Validate each archive's contents and SHA-256 hashes. Keep optional artifact libraries' upstream notices and integrity records. Do not import or rename proprietary resources from another application.
