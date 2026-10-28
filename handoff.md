# QuixiDesk maintenance handoff

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
