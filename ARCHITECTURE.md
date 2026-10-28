# QuixiDesk architecture

QuixiDesk is a native Rust desktop workspace with an original HTML, CSS, and JavaScript interface. The interface uses system fonts and repository-owned SVG icons. No extracted application renderer or proprietary font bundle is required.

## Native host

`quixi-app` creates the Tao window and Wry system webview. It serves `resources/ui` through a local application protocol, installs `native_bridge.js` in the main frame, and dispatches native events on the UI thread. Windows uses WebView2's virtual HTTP origin; macOS and Linux use the application scheme directly.

The bridge exposes application APIs and the `quixi.web` RPC namespace. Background work runs outside the UI thread. Native close requests check session storage before stopping the event loop. macOS menu commands use the same close guard. Windows shutdown also posts a native quit message so an inactive desktop does not need a repaint to finish closing.

## Workspace engine

`quixi-engine` owns conversation state, projects, local artifacts, provider selection, turn cancellation, and storage status. `quixi-ipc` adapts interface requests to engine operations. `quixi-proto` defines the serialized session and project records.

`quixi-workspace` provides file discovery, Git inspection, shell execution, and transcript search. `quixi-platform` implements OS services and browser snapshots. `quixi-mcp` exposes built-in tools and configured external stdio tools. `quixi-netstack` manages network policy and provider credentials.

The interface renders stored messages and incremental streaming events. Tool results keep their failure state visible. Manual mode blocks shell execution and mutations; broader modes are selected explicitly.

## Local persistence

Normal app data resides in the user's `.quixi` directory. Session records use temporary-file writes, synchronization, and atomic replacement. If primary saving fails, the engine attempts a recovery record. A persistent interface notice reports whether a durable recovery copy exists. If neither location can save, normal quit remains blocked.

Startup compares recovery and primary revisions before restoring sessions. Retry saves only affected conversations. Code-session records include their transcripts. Conversation deletion removes recovery records as well as primary records to prevent resurrection on restart.

## Content isolation

User artifact HTML and browser snapshots render in sandboxed frames without same-origin privileges. Native API requests accept only the platform's application origin. External page content cannot invoke workspace RPC or read the parent document. Artifact publishing is not automatic.

## Providers and integrations

Model protocols, authentication, endpoint validation, model catalogs, and streaming parsers live in `quixi-engine/src/providers.rs`. Provider names and API identifiers belong to that integration boundary. Credentials are masked in public provider state and are not inserted into the conversation DOM.

## Test profiles

`--test-profile` selects separate app data, account fixtures, workspace defaults, and web storage. `--mock-account` supplies a synthetic account. This is application-state isolation, not an operating-system sandbox. Tests requiring independent OS permissions use a disposable account or VM.

## Resource provenance

The application interface, icons, and bundled task instructions are maintained in this repository. Optional artifact libraries are sourced from versioned upstream npm packages, with integrity values and licenses under `resources/artifact-libs`. Keep upstream notices with redistributed dependencies. Asset ownership and distribution terms require review independently of the Rust workspace license.
