# `Claude.app` (`v3.6556.2`) Reverse Engineering & REA Handoff Guide

**Last Updated:** 2026-10-26  
**Target:** `Claude.app` `3.6556.2` (`@ant/desktop`, commit `20c019a1e61fa5b687616af57d31f97bb3181723`, Electron `44.9.3`, Chromium `152.0.7977.167`, Node `v24.22.0`)  
**Workspace Root:** `/Users/eric/quixi-desk`

This document provides everything a new agent needs to immediately operate the REA (`rea-agents` CLI and `rea` MCP server) toolchain, companion reverse-engineering tools, and pre-extracted artifacts in `/Users/eric/quixi-desk` without repeating setup or hitting environment pitfalls.

---

## 1. Workspace Layout (`/Users/eric/quixi-desk`)

Everything needed to analyze `Claude.app`—even if the DMG is unmounted—is persisted in `/Users/eric/quixi-desk`:

| Path | Contents |
|---|---|
| `/Users/eric/quixi-desk/ARCHITECTURE.md` | Complete 13-section (47 KB) architecture specification of `Claude.app` `v3.6556.2` covering the bundle, V8 `.jsc` cache, Electron windows/views, `@ant/ipc-codegen` (`$eipc_message$`) router, 8 workers, `.node` addons, helpers, Cowork Linux VM (`smol-bin`), Computer Use, MCP/DXT, and hidden subsystems (`@ant/clawd-quest`, `buddy`, `midnightOwl`, `tearOffHalo`, `sableOtter`, `wakeScheduler`). |
| `/Users/eric/quixi-desk/asar-extracted/` | Full extraction of `/Volumes/Claude/Claude.app/Contents/Resources/app.asar` (`.vite/build/`, `.vite/renderer/`, `compile-cache/`, `node_modules/`, `resources/`). |
| `/Users/eric/quixi-desk/reference/ipc-catalog-1400-members.txt` | All 1,400 `$eipc_message$` channels across 2 UUIDs (`5de9b164-...` and `fcb91d11-...`), 19 namespaces, and 102 services, with source chunk & preload locations. |
| `/Users/eric/quixi-desk/reference/vite-chunks-464-map.txt` | Complete inventory of all 464 `.vite/build/` main-process files (`index.pre.js`, `index.js`, and 462 `index.chunk-*.js` modules) with byte sizes, exported symbols, and dependency edges. |
| `/Users/eric/quixi-desk/reference/ipc-sender-validators.txt` | Extracted origin and frame validation functions (`senderFrame.parent === null`, `claude.ai` origin checks) from the main-process IPC router. |
| `/Users/eric/quixi-desk/binaries/addons/` | Native `.node` addons (`arm64` Mach-O): `swift_addon.node` (25 MB), `computer_use.node` (2.2 MB), `claude-native-binding.node` (5.5 MB), `pty.node` (198 KB), `spawn-helper` (117 KB). |
| `/Users/eric/quixi-desk/binaries/helpers-arm64/` | Pre-thinned single-architecture (`arm64`) Mach-O slices of all helper binaries (ready for both **Ghidra** and **Hopper**): `disclaimer` (69 KB), `permission-fixer` (103 KB), `app-cu-helper` (979 KB), `chrome-native-host` (1.1 MB), `claude-ssh-askpass` (2.4 MB), `claude-ios-sim` (9.5 MB). |
| `/Users/eric/quixi-desk/binaries/smol-bin-arm64/` | Extracted contents of the Cowork Linux VM `smol-bin.arm64.img` exFAT image (`arm64` static Linux Go `1.25.8` ELF binaries): `sdk-daemon` (`coworkd`, 15 MB), `sandbox-helper` (2.1 MB), `cli-wrapper` (2.3 MB), `claude-shim` (2.2 MB), and `srt-settings.json`. |

### Original DMG & Mounted Bundle
- **DMG Path:** `/Users/eric/Downloads/Claude.dmg`
- **Mounted Path:** `/Volumes/Claude/Claude.app`
- If `/Volumes/Claude` is not mounted (e.g., after a reboot), remount it read-only with:
  ```bash
  hdiutil attach -nobrowse -readonly ~/Downloads/Claude.dmg
  ```

---

## 2. REA Toolchain Configuration & Critical Gotchas

Both **Hopper Disassembler** (`5.17.1`) and **Ghidra** (`12.1.4` + native `mac_arm_64/decompile` + OpenJDK `21.0.13`) are installed, configured, and verified healthy (`12/12` checks pass in `rea-agents doctor`).

### 2.1 Environment Variables for Shell Commands (`run_command`)
Because tool calls execute in fresh non-interactive `zsh` subshells (which do not automatically source `~/.zshrc`), **always prefix shell commands** that invoke `npx rea-agents`, Ghidra, Hopper, or Homebrew tools with:

```bash
export GHIDRA_INSTALL_DIR="/Users/eric/Applications/ghidra_12.1.4_PUBLIC"
export JAVA_HOME="/Users/eric/Library/Java/JavaVirtualMachines/jdk-21.0.13+9/Contents/Home"
export PATH="/opt/homebrew/bin:$HOME/.local/bin:$JAVA_HOME/bin:$PATH"
```

Verify provider health at any time with:
```bash
export GHIDRA_INSTALL_DIR="/Users/eric/Applications/ghidra_12.1.4_PUBLIC"
export JAVA_HOME="/Users/eric/Library/Java/JavaVirtualMachines/jdk-21.0.13+9/Contents/Home"
export PATH="/opt/homebrew/bin:$HOME/.local/bin:$JAVA_HOME/bin:$PATH"
npx rea-agents doctor --provider hopper --provider ghidra --client antigravity
```

### 2.2 Four Critical Rules When Using REA

1. **Never pass container formats (`.dmg`, `.app`, `.asar`, `.img`) to `rea-agents analyze` or `open_binary`:**
   - Both Ghidra and Hopper reject archives/disk images with `code: target_kind_unsupported`.
   - Always pass an **extracted Mach-O executable, `.node` addon, `.dylib`, or ELF binary** (such as the files in `/Users/eric/quixi-desk/binaries/`).
2. **Ghidra (`--provider ghidra`) requires single-architecture Mach-O binaries:**
   - The original helpers in `/Volumes/Claude/Claude.app/Contents/Helpers/*` (`disclaimer`, `permission-fixer`, `app-cu-helper`, `chrome-native-host`, `claude-ssh-askpass`) are **Universal (`x86_64` + `arm64`)** fat binaries.
   - **Hopper (`--provider hopper`)** handles universal Mach-O binaries directly.
   - **Ghidra (`--provider ghidra`)** rejects universal Mach-O binaries with `code: architecture_unsupported` (`Multiple architectures are present`).
   - **Solution:** Point Ghidra at `/Users/eric/quixi-desk/binaries/helpers-arm64/<name>` (already thinned via `lipo -thin arm64`), `/Users/eric/quixi-desk/binaries/addons/<name>.node` (already single-arch `arm64`), or `/Users/eric/quixi-desk/binaries/smol-bin-arm64/<name>` (already single-arch `arm64` Linux ELF).
3. **Hopper AppleEvents TCC Permission (`-1743`):**
   - `Antigravity.app` $\to$ `Hopper Disassembler.app` AppleEvents TCC permission is already granted.
   - *Background if TCC ever resets after a macOS/app upgrade:* `/Users/eric/.local/bin/hopper` sends a custom `open executable` AppleEvent and lacks the `com.apple.private.tcc.allow-prompting` entitlement, while `Antigravity.app` lacks `NSAppleEventsUsageDescription` in its `Info.plist`. If `hopper` ever fails with `-1743`, run once via `/usr/bin/osascript` (which possesses `com.apple.private.tcc.allow-prompting`) to trigger the TCC dialog:
     ```bash
     /usr/bin/osascript -e 'tell application "Hopper Disassembler" to open executable "/Users/eric/quixi-desk/binaries/helpers-arm64/disclaimer"'
     ```
4. **Upgrading `rea-agents`:**
   - `rea-agents` is installed in the npm `npx` cache (`~/.npm/_npx/.../node_modules/rea-agents`), so `npx rea-agents --update` reports `unknown-install-method`. To run the newest release when needed, invoke `npx rea-agents@latest ...`.

---

## 3. Using the `rea` MCP Server (`call_mcp_tool`)

All `rea` MCP tools are lazy-loaded; their JSON schemas live in `/Users/eric/.gemini/antigravity/mcp/rea/<toolName>.json`, and best practices are in `/Users/eric/.gemini/antigravity/mcp/rea/instructions.md`.

To invoke any `rea` tool, read its schema file if unsure of parameters, then call `call_mcp_tool` with `ServerName: "rea"` and `ToolName: "<toolName>"`.

### 3.1 Most Useful `rea` MCP Tools for `Claude.app`

#### A. Native Binary Decompilation & Call Graph (Hopper / Ghidra)
- `binary_overview`: High-level summary of segments, imports, exports, and entry points.
  ```json
  {
    "ServerName": "rea",
    "ToolName": "binary_overview",
    "Arguments": {
      "filePath": "/Users/eric/quixi-desk/binaries/helpers-arm64/disclaimer"
    }
  }
  ```
- `list_procedures` / `search_procedures`: Paginate or regex-search functions in the binary.
- `procedure_pseudo_code`: Decompile a procedure to C-like pseudocode (pass `address` or `name` and `filePath`).
- `procedure_assembly`: Disassemble a procedure.
- `procedure_callees` / `procedure_callers` / `xrefs`: Navigate cross-references and call graphs.
- `analyze_function` / `batch_decompile`: Deep function analysis or multi-function decompilation in one call.
- `search_strings`: Search strings in the binary with address cross-references.

#### B. macOS / Mach-O / Swift / Objective-C / Rust / Go Specialized Tools
- `inspect_macho`: Load commands, segments, UUIDs, linked dylibs, rpaths.
- `inspect_signature`: Code signature, Team ID, entitlements, and notarization flags.
- `inspect_plist`: Parse any binary or XML `.plist` (`Info.plist`, LaunchServices, etc.).
- `get_objc_classes` / `get_objc_protocols`: Objective-C class and protocol metadata (`swift_addon.node`, `computer_use.node`, `claude-ios-sim`).
- `analyze_swift_types` / `demangle_swift`: Recover Swift nominal type descriptors, protocol conformances, and demangled symbols (`swift_addon.node`, `computer_use.node`, `permission-fixer`).
- `inspect_rust_binary`: Extract Rust compiler version, crate dependency graph, and panic strings (`claude-native-binding.node`, `app-cu-helper`, `chrome-native-host`, `claude-ssh-askpass`).
- `inspect_go_binary`: Extract Go compiler version, module dependencies, build info, and package/function symbol table (`claude-ios-sim`, `github-mcp-server`, `swift_addon.node`'s embedded Go gVisor runtime, and the 4 `smol-bin` Linux ELF binaries: `sdk-daemon`, `sandbox-helper`, `cli-wrapper`, `claude-shim`).

#### C. Electron & JavaScript Application Analysis
- `inspect_electron_fuses`: Inspect Electron fuse bits on `/Volumes/Claude/Claude.app`.
- `analyze_javascript_application`: Analyze extracted Electron/Vite bundle topology in `/Users/eric/quixi-desk/asar-extracted`.
- `trace_application_feature` / `trace_javascript_semantics`: Trace IPC channels, preload bridges, and feature call paths across JS chunks.
- `list_electron_targets` / `inspect_electron_page` / `observe_javascript_runtime`: Attach to a live running Electron target if dynamic inspection is requested.

#### D. Dynamic Instrumentation (Frida / Rizin / LLDB)
- `list_frida_processes`, `start_frida_session`, `load_frida_script`, `instrument_with_frida`, `observe_native_calls`.
- `execute_rizin_command`: Run non-interactive Rizin (`rz-bin` / `rizin -qc ...`) commands on any binary.

---

## 4. Companion CLI Tools Installed on This Machine

All of the following CLI tools are installed in `/opt/homebrew/bin` or `/Users/eric/.local/bin`:

| Tool | Path & Version | Primary Use on `Claude.app` |
|---|---|---|
| **`ipsw`** | `/opt/homebrew/bin/ipsw` (`3.1.667`) | Best-in-class Apple binary tool: `ipsw macho info`, `ipsw swift-dump <bin>` (dumps Swift classes/structs/enums/methods from `swift_addon.node` & `computer_use.node`), `ipsw class-dump <bin>`, `ipsw ent <bin>`. |
| **`rizin` / `rz-bin`** | `/opt/homebrew/bin/rizin` (`0.8.2`) | Fast CLI disassembly, CFG, strings (`rz-bin -z`), imports/exports (`rz-bin -i`, `rz-bin -E`), binary diffing (`rz-diff`). |
| **`frida` / `frida-trace`** | `/opt/homebrew/bin/frida` (`17.9.1`) | Dynamic hooking of Objective-C, Swift, N-API (`napi_define_properties`), `AF_VSOCK`, and Mach calls. |
| **`bloaty`** | `/opt/homebrew/bin/bloaty` | Section/symbol size profiling of Mach-O and ELF binaries. |
| **`fq`** | `/opt/homebrew/bin/fq` (`0.17.0`) | `jq`-style binary structure decoder. |
| **`webcrack`** | `/opt/homebrew/bin/webcrack` (`2.15.1`) | De-minifies and unpacks bundled/minified JS files (`webcrack <chunk.js> -o <dir>`). |
| **`ast-grep`** | `/opt/homebrew/bin/ast-grep` (`0.42.1`) | Structural AST search across `.vite/build/*.js` (`ast-grep --lang js -p '...'`). |
| **`biome`** | `/opt/homebrew/bin/biome` | Ultra-fast JS formatter (`biome format --write <file.js>`) to make single-line Vite chunks readable. |
| **`asar`** | `/opt/homebrew/bin/asar` (`4.1.0`) | Pack/unpack Electron `.asar` archives. |
| **`wabt` & `binaryen`** | `/opt/homebrew/bin/wasm2wat`, `wasm-decompile`, `wasm-objdump`, `wasm-dis` | Inspect WebAssembly modules (e.g. `.vite/build/frame-local-worker/html_rewriter_bg.wasm`). |
| **`binwalk` & `unsquashfs`** | `/opt/homebrew/bin/binwalk`, `unsquashfs` | Firmware/disk image carving and rootfs inspection. |

---

## 5. Quick-Reference Matrix of All `Claude.app` Native Binaries

Every native binary in `Claude.app` is listed below with its pre-extracted path in `/Users/eric/quixi-desk/binaries/`, architecture, language, and recommended inspection commands:

| Binary | Pre-Extracted Path (`/Users/eric/quixi-desk/binaries/...`) | Format & Arch | Language / Runtime | Best Inspection Tools |
|---|---|---|---|---|
| **`swift_addon.node`** | `addons/swift_addon.node` | Mach-O `arm64` bundle (25 MB) | Swift + ObjC + embedded Go `cgo` (`VZGvisorNetworking`) | `ipsw swift-dump`, `inspect_go_binary`, Ghidra/Hopper (`procedure_pseudo_code`), `xcrun swift-demangle` |
| **`computer_use.node`** | `addons/computer_use.node` | Mach-O `arm64` bundle (2.2 MB) | Swift + ObjC (`ScreenCaptureKit`, `ApplicationServices`, `Carbon`) | `ipsw swift-dump`, Ghidra/Hopper (`procedure_pseudo_code`) |
| **`claude-native-binding.node`** | `addons/claude-native-binding.node` | Mach-O `arm64` dylib (5.5 MB) | Rust (`napi-rs`, `ring`, `x25519-dalek`, `Security.framework`) | `inspect_rust_binary`, Ghidra/Hopper (`napi_register_module_v1`, `claude_native::*`) |
| **`pty.node` & `spawn-helper`** | `addons/pty.node`, `addons/spawn-helper` | Mach-O `arm64` (198 KB / 117 KB) | C++ / C (`forkpty`, `posix_spawnp`) | Ghidra/Hopper (`procedure_pseudo_code`) |
| **`disclaimer`** | `helpers-arm64/disclaimer` | Mach-O `arm64` exec (69 KB) | C (`responsibility_spawnattrs_setdisclaim`, `kqueue`) | Ghidra/Hopper (`_main` at `0x100003154`) |
| **`permission-fixer`** | `helpers-arm64/permission-fixer` | Mach-O `arm64` exec (103 KB) | Swift (`ServiceManagement` `SMJobSubmit`, `Security`) | `ipsw swift-dump`, Ghidra/Hopper (`_main` at `0x100002ac0`) |
| **`app-cu-helper`** | `helpers-arm64/app-cu-helper` | Mach-O `arm64` exec (979 KB) | Rust (`objc2`, `core-graphics`, `core-foundation`) | `inspect_rust_binary`, Ghidra/Hopper |
| **`chrome-native-host`** | `helpers-arm64/chrome-native-host` | Mach-O `arm64` exec (1.1 MB) | Rust (`tokio`, Unix domain socket fanout) | `inspect_rust_binary`, Ghidra/Hopper |
| **`claude-ssh-askpass`** | `helpers-arm64/claude-ssh-askpass` | Mach-O `arm64` exec (2.4 MB) | Rust (`tokio`, `serde_json`, `clap`) | `inspect_rust_binary`, Ghidra/Hopper |
| **`claude-ios-sim`** | `helpers-arm64/claude-ios-sim` | Mach-O `arm64` exec (9.5 MB) | Go `1.25.7` (`cgo`) + ObjC (`AppKit`, `WebKit`) | `inspect_go_binary`, `get_objc_classes`, Ghidra/Hopper |
| **`sdk-daemon` (`coworkd`)** | `smol-bin-arm64/sdk-daemon` | Linux ELF `arm64` static (15 MB) | Go `1.25.8` (`github.com/anthropics/anthropic-Using/coworkd`, `mdlayher/vsock`, `creack/pty`) | `inspect_go_binary`, Ghidra/Hopper |
| **`sandbox-helper`** | `smol-bin-arm64/sandbox-helper` | Linux ELF `arm64` static (2.1 MB) | Go `1.25.8` (`bwrap`, `seccomp` BPF, HTTP MITM / SOCKS5 proxy) | `inspect_go_binary`, Ghidra/Hopper |
| **`cli-wrapper`** | `smol-bin-arm64/cli-wrapper` | Linux ELF `arm64` static (2.3 MB) | Go `1.25.8` (`coworkd/cmd/cli-wrapper`) | `inspect_go_binary`, Ghidra/Hopper |
| **`claude-shim`** | `smol-bin-arm64/claude-shim` | Linux ELF `arm64` static (2.2 MB) | Go `1.25.8` (forwards guest `claude` CLI calls over vsock/RPC) | `inspect_go_binary`, Ghidra/Hopper |

---

## 6. Navigating the JavaScript / Electron Layer (`/Users/eric/quixi-desk/asar-extracted`)

### 6.1 Key Entry Files
1. **`.vite/build/index.pre.js`** (37.5 KB):
   - Entry point declared in `package.json` (`"main": ".vite/build/index.pre.js"`).
   - Enforces anti-debugging check (`process.env.NODE_OPTIONS` or `--inspect` exits immediately when `app.isPackaged` is true).
   - Monkey-patches `Module.prototype._compile` to load pre-compiled V8 bytecode snapshots from `compile-cache/*.{arm64,x64}.jsc`.
   - Loads `index.js` $\to$ `index.chunk-jPKPzV-P.js` (6.73 MB core main-process bundle).
2. **`.vite/build/index.chunk-jPKPzV-P.js`** (6.73 MB):
   - Core main-process orchestrator. Lazily loads the other 461 `index.chunk-*.js` modules.
   - Use `/Users/eric/quixi-desk/reference/vite-chunks-464-map.txt` to immediately locate which chunk implements any subsystem without grepping 464 files.
3. **15 Preload Scripts in `.vite/build/`:**
   - `mainWindow.js`, `mainView.js`, `quickWindow.js`, `aboutWindow.js`, `buddy.js`, `designWindow.js`, `findInPage.js`, `localExecConsent.js`, `artifactShell.js`, `coworkArtifact.js`, `claudePagePreview.js`, `computerUseTeach.js`, `computerUseWatchRecord.js`, `watchRecordChooser.js`, `sableOtterHud.js`.
4. **8 Workers in `.vite/build/`:**
   - `file-index-worker/fileIndexWorker.js` (`utilityProcess`, Rust `ignore` walker + FNV-1a trigram index)
   - `frame-local-worker/frameLocalWorker.js` (`utilityProcess`, Cloudflare `lol-html` `html_rewriter_bg.wasm`)
   - `heavy-work-worker/heavyWorkWorker.js` (`utilityProcess`, 40 RPC tasks: git/worktree ops, JSONL parsing, file hashing, image resizing, tar/zip)
   - `mcp-runtime/directMcpHost.js` & `mcp-runtime/nodeHost.js` (`utilityProcess`, executes in-process & DXT MCP servers)
   - `pty-host/ptyHostWorker.js` (`utilityProcess`, manages `node-pty` sessions)
   - `shell-path-worker/shellPathWorker.js` (`utilityProcess`, resolves user login shell `PATH`)
   - `stall-sampler-worker/stallSamplerWorker.js` (`worker_threads`, attaches `inspector.Session` `Profiler` to sample V8 stacks on $\ge 250\text{ ms}$ main-thread stalls)
   - `transcript-search-worker/transcriptSearchWorker.js` (`utilityProcess`, full-text search across `~/.claude/projects/**/*.jsonl`)

### 6.2 How to Format / De-minify Any Chunk for Deep Reading
To turn any minified Vite chunk into formatted, readable JS in a scratch directory:
```bash
export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"
mkdir -p /Users/eric/quixi-desk/scratch
cp /Users/eric/quixi-desk/asar-extracted/.vite/build/index.chunk-1E6-bK6z.js /Users/eric/quixi-desk/scratch/
biome format --write /Users/eric/quixi-desk/scratch/index.chunk-1E6-bK6z.js
```
Or run `webcrack` to simplify bundled/minified idioms (`!0` $\to$ `true`, comma sequences $\to$ statements):
```bash
webcrack /Users/eric/quixi-desk/asar-extracted/.vite/build/index.chunk-1E6-bK6z.js -o /Users/eric/quixi-desk/scratch/unpacked-chunk
```

### 6.3 Important Note on V8 Bytecode Cache (`compile-cache/*.jsc`) & Electron Fuses
If you ever run a modified copy of `Claude.app` for dynamic analysis:
1. **`compile-cache/*.jsc` override `.js` files:** `index.pre.js` intercepts `Module.prototype._compile` and feeds V8 cached bytecode from `compile-cache/<basename>.arm64.jsc` if the `.js` byte length matches `.manifest.json`. Always delete or disable `compile-cache/` when modifying `.vite/build/*.js`.
2. **Electron Fuses & Code Signature:** `OnlyLoadAppFromAsar` and `EnableEmbeddedAsarIntegrityValidation` are enabled on `/Volumes/Claude/Claude.app`. Repacking `app.asar` requires flipping the ASAR integrity fuse (or updating the `ElectronAsarIntegrity` hash in `Info.plist`) and re-signing the cloned `.app` bundle with `codesign --force --deep --sign -` (note: ad-hoc signing drops `com.apple.security.virtualization` unless entitlements are stripped or signed with a provisioning profile).

---

## 7. Ready-to-Run Recipes for Common Next-Step Investigations

1. **Decompile a Swift/ObjC/C/Rust function with Ghidra or Hopper via REA CLI:**
   ```bash
   export GHIDRA_INSTALL_DIR="/Users/eric/Applications/ghidra_12.1.4_PUBLIC"
   export JAVA_HOME="/Users/eric/Library/Java/JavaVirtualMachines/jdk-21.0.13+9/Contents/Home"
   export PATH="/opt/homebrew/bin:$HOME/.local/bin:$JAVA_HOME/bin:$PATH"
   npx rea-agents analyze --provider ghidra /Users/eric/quixi-desk/binaries/helpers-arm64/permission-fixer
   ```
2. **Dump all Swift classes, structs, enums, and method addresses in `swift_addon.node` or `computer_use.node`:**
   ```bash
   /opt/homebrew/bin/ipsw swift-dump /Users/eric/quixi-desk/binaries/addons/swift_addon.node
   ```
3. **Inspect the Cowork Linux VM daemon (`sdk-daemon` / `coworkd`) RPC handlers:**
   - Call `call_mcp_tool` with `ServerName: "rea"`, `ToolName: "inspect_go_binary"`, and `Arguments: {"filePath": "/Users/eric/quixi-desk/binaries/smol-bin-arm64/sdk-daemon"}`, or decompile `coworkd/internal/daemon.*` functions in Hopper/Ghidra.
4. **Trace any `$eipc_message$` IPC method from Renderer $\to$ Preload $\to$ Main Handler:**
   - Look up the service or method name in `/Users/eric/quixi-desk/reference/ipc-catalog-1400-members.txt` to get the exact `index.chunk-*.js` file where `ipcMain.handle` is registered.
