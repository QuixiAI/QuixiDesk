/* Copyright (c) 2026 QuixiAI. MIT license.
 * Modular Spatiotemporal Client UI Plugins (`cordis-ui-plugins.js`).
 * Every feature registers solely through `window.QuixiCordis.definePlugin` & `ctx.slot(...)`
 * so disabling a bundle/row in Sidebar -> Plugins unmounts its UI cleanly in place.
 */
(() => {
  "use strict";

  const cordis = window.QuixiCordis;
  if (!cordis) return;

  const isMac = () => /Mac|iPhone|iPad/.test(navigator.platform || "");
  const modLabel = () => (isMac() ? "⌘" : "Ctrl+");

  function safeStorageGet(key, fallback) {
    try {
      const raw = localStorage.getItem(key);
      return raw ? JSON.parse(raw) : fallback;
    } catch {
      return fallback;
    }
  }

  function safeStorageSet(key, val) {
    try {
      localStorage.setItem(key, JSON.stringify(val));
    } catch {}
  }

  // =========================================================================
  // 1. `ui-shortcuts` — Editable Keyboard Shortcuts & Font Size Steppers
  // =========================================================================
  cordis.definePlugin({
    id: "ui-shortcuts",
    title: "Keyboard Shortcuts & Font Scale Service",
    provide: ["shortcuts"],
    setup(ctx) {
      const DEFAULTS = [
        { id: "palette.open", label: "Quick jump & command launcher", combo: "Mod+P" },
        { id: "find.in_page", label: "Find in conversation", combo: "Mod+F" },
        { id: "sidechat.open", label: "Open Side Chat quick aside (/btw)", combo: "Mod+Shift+B" },
        { id: "nav.back", label: "Navigate back", combo: "Mod+[" },
        { id: "nav.forward", label: "Navigate forward", combo: "Mod+]" },
        { id: "shortcuts.open", label: "Keyboard shortcuts reference", combo: "Mod+/" },
        { id: "settings.open", label: "Open Settings", combo: "Mod+," },
        { id: "sidebar.toggle", label: "Toggle left sidebar", combo: "Mod+B" },
        { id: "session.new", label: "Start new task", combo: "Mod+N" },
        { id: "session.search", label: "Search conversations", combo: "Mod+K" },
        { id: "session.fork", label: "Branch into new conversation", combo: "Mod+Alt+F" },
        { id: "plugins.open", label: "Open Plugins manager", combo: "Mod+Shift+P" },
      ];

      let overrides = safeStorageGet("quixi.keybindings.v1", {});
      let fonts = safeStorageGet("quixi.fontSizes.v1", { content: 14, code: 12, terminal: 13 });
      let zoomPct = Number(safeStorageGet("quixi.zoomPct.v1", 100)) || 100;

      const applyFonts = () => {
        const rootStyle = document.documentElement.style;
        rootStyle.setProperty("--dsh-content-font-size", `${fonts.content}px`);
        rootStyle.setProperty("--dsh-code-font-size", `${fonts.code}px`);
        rootStyle.setProperty("--dsh-terminal-font-size", `${fonts.terminal}px`);
        rootStyle.zoom = zoomPct === 100 ? "" : `${zoomPct}%`;
      };
      applyFonts();

      const formatCombo = (combo) => {
        if (!combo) return "Unbound";
        return combo.replace("Mod+", modLabel()).replace("Alt+", isMac() ? "⌥" : "Alt+").replace("Shift+", isMac() ? "⇧" : "Shift+");
      };

      const eventToCombo = (e) => {
        if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return null;
        const parts = [];
        if (isMac() ? e.metaKey : e.ctrlKey) parts.push("Mod");
        if (e.altKey) parts.push("Alt");
        if (e.shiftKey) parts.push("Shift");
        const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
        parts.push(key);
        return parts.join("+");
      };

      function openQuickLauncher() {
        const app = ctx.app();
        if (!app) return;
        const { el, button, icon, dialog, navigate, state, rpc, service, toast } = app;
        const cur = app.session();
        const searchInput = el("input", {
          type: "search",
          placeholder: "Jump to a conversation, workspace file, page, or command…",
          "aria-label": "Quick jump query",
        });
        const resultsHost = el("div", { class: "dsh-palette-list", role: "listbox" });
        let entries = [];
        let selIdx = 0;
        let fileHits = [];
        let reqGen = 0;
        let modal;

        const syncHighlight = () => {
          resultsHost.querySelectorAll(".dsh-palette-item").forEach((n, idx) => {
            const on = idx === selIdx;
            n.classList.toggle("is-active", on);
            n.setAttribute("aria-selected", String(on));
            if (on) n.scrollIntoView?.({ block: "nearest" });
          });
        };

        const buildEntries = (qRaw) => {
          const q = qRaw.trim().toLowerCase();
          const match = (title, sub = "") => !q || title.toLowerCase().includes(q) || sub.toLowerCase().includes(q);
          const groups = [];

          const actions = [
            { title: "New task", sub: "Start a fresh workspace task", icon: "plus", badge: formatCombo("Mod+N"), run: () => { app.saveDraft(); state.project = null; state.folder = ""; navigate("new"); } },
            ...(cur ? [
              { title: "Toggle Plan Mode", sub: "Read-only exploration & plan approval gate", icon: "spark", badge: "/plan", run: async () => {
                const next = !Boolean(state.collab.get(cur.sessionId)?.planMode?.active);
                const updated = await rpc(service(cur), "setPlanMode", [cur.sessionId, next]);
                if (updated) state.collab.set(cur.sessionId, updated);
                app.renderPage();
                toast(next ? "Plan Mode enabled." : "Plan Mode disabled.");
              } },
              { title: "Compact conversation context", sub: "Summarize older messages to reclaim context window", icon: "code", badge: "/compact", run: async () => {
                const r = await rpc(service(cur), "compactSession", [cur.sessionId]);
                if (r?.error) return toast(r.error, true);
                await app.refreshSession(cur.sessionId);
                app.renderPage();
                toast(r?.compacted ? `Saved ~${r.savedTokens} tokens.` : (r?.message || "Already compact."));
              } },
              { title: "Open Files & Context dock", sub: "Browse files, diffs, terminal, and team tasks", icon: "folder", badge: "Dock", run: () => app.contextDrawer?.("files") },
              { title: "Find in conversation", sub: "Highlight and jump through matches in this chat", icon: "search", badge: formatCombo("Mod+F"), run: () => app.openConversationFind?.() },
              { title: "Open Side Chat aside (/btw)", sub: "Ask a tangent question without polluting this chat", icon: "chat", badge: "/btw", run: () => cordis._services.get("sideChat")?.value?.openSideChat?.("") },
              { title: "Inspect working tree changes", sub: "Open Changes & Side-by-Side Diff dock", icon: "code", badge: "Diff", run: () => app.contextDrawer?.("changes") },
              { title: "Branch into a new conversation", sub: "Non-destructive fork of current transcript", icon: "plus", badge: formatCombo("Mod+Alt+F"), run: () => app.forkConversationBranch?.(cur) },
            ] : []),
            { title: "Choose response output style", sub: "Normal, Concise, Explanatory, Formal, Learning, or Custom", icon: "spark", badge: "/style", run: () => cordis._services.get("sideChat")?.value?.openStylePicker?.() },
            { title: "Switch color theme (Dark / Light / Warm / System)", sub: `Current theme: ${state.theme || "system"}`, icon: "spark", badge: "Theme", run: () => {
              const order = ["system", "dark", "light", "warm-dark", "warm-light"];
              state.theme = order[(order.indexOf(state.theme || "system") + 1) % order.length];
              try { localStorage.setItem("quixi.theme", state.theme); } catch {}
              app.applyTheme?.();
              toast(`Theme: ${state.theme}`);
            } },
            { title: "Open Superagent 6-tab panel", sub: "Overview, Source, Trigger, Eval, Execution, Settings", icon: "bot", badge: "Page", run: () => navigate("superagent") },
            { title: "Open Plugins manager", sub: "Toggle spatiotemporal Cordis bundles live", icon: "grid", badge: formatCombo("Mod+Shift+P"), run: () => navigate("plugins") },
            { title: "Open Artifacts studio", sub: "Preview, export, and iterate on HTML artifacts", icon: "artifacts", badge: "Page", run: () => navigate("artifacts") },
            { title: "Open Tools & MCP servers", sub: "Configure MCP servers and workspace skills", icon: "tools", badge: "Page", run: () => navigate("tools") },
            { title: "Open Settings", sub: "Providers, appearance, typography, and shortcuts", icon: "settings", badge: formatCombo("Mod+,"), run: () => navigate("settings") },
            { title: "Open full Search page", sub: "Deep full-text search across transcripts", icon: "search", badge: formatCombo("Mod+K"), run: () => navigate("search") },
          ].filter((a) => match(a.title, a.sub));
          if (actions.length) groups.push(["Actions & Navigation", actions.slice(0, q ? 8 : 6)]);

          const convs = state.sessions
            .filter((s) => !s.isArchived && match(s.title || "Untitled", s.cwd || ""))
            .slice(0, 8)
            .map((s) => ({
              title: s.title || "Untitled task",
              sub: s.cwd ? s.cwd.split(/[\\/]/).filter(Boolean).pop() : "Conversation",
              icon: s.isStarred ? "star" : "file",
              badge: s.isRunning ? "Running" : "Chat",
              run: () => navigate("chat", s.sessionId),
            }));
          if (convs.length) groups.push(["Recent Conversations", convs]);

          const arts = (state.artifacts || [])
            .filter((a) => match(a.name || a.title || "", a.description || ""))
            .slice(0, 4)
            .map((a) => ({
              title: a.name || a.title || "Artifact",
              sub: `Revision ${a.revision || 1}`,
              icon: "artifacts",
              badge: "Artifact",
              run: () => app.openArtifact?.(a.id),
            }));
          if (arts.length) groups.push(["Artifacts", arts]);

          if (fileHits.length) {
            groups.push([
              "Workspace Files",
              fileHits.slice(0, 8).map((f) => {
                const rel = f.relativePath || f.name;
                return {
                  title: rel,
                  sub: f.isDir ? "Directory" : "Preview in dock",
                  icon: f.isDir ? "folder" : "file",
                  badge: "File",
                  run: () => {
                    if (f.isDir && cur) {
                      state.dockBrowsePath = f.path;
                      app.contextDrawer?.("files");
                    } else {
                      void app.openWorkspaceFileInDock?.(f.path || rel, null);
                    }
                  },
                };
              }),
            ]);
          }

          entries = [];
          selIdx = 0;
          const frag = document.createDocumentFragment();
          for (const [heading, rows] of groups) {
            frag.append(el("div", { class: "trigger-group-heading" }, heading));
            for (const row of rows) {
              const idx = entries.length;
              entries.push(row);
              const btn = button(
                [
                  el("span", { class: "trigger-option-main" },
                    icon(row.icon || "spark", 14),
                    el("span", {},
                      el("strong", {}, row.title),
                      row.sub ? el("small", { class: "dsh-palette-sub" }, " · " + row.sub) : null,
                    ),
                  ),
                  row.badge ? el("span", { class: "cordis-keycap" }, row.badge) : null,
                ],
                () => {
                  modal.close();
                  void row.run();
                },
                "dsh-palette-item" + (idx === 0 ? " is-active" : ""),
              );
              btn.setAttribute("role", "option");
              btn.setAttribute("aria-selected", String(idx === 0));
              btn.addEventListener("mouseenter", () => {
                selIdx = idx;
                syncHighlight();
              });
              frag.append(btn);
            }
          }
          if (!entries.length) {
            frag.append(el("p", { class: "hint" }, "No matching commands, conversations, or workspace files."));
          }
          resultsHost.replaceChildren(frag);
        };

        searchInput.addEventListener("input", async () => {
          const val = searchInput.value;
          const gen = ++reqGen;
          const folder = cur?.cwd || app.currentFolder();
          if (folder && val.trim().length >= 1) {
            const found = (await rpc(cur ? service(cur) : "LocalAgentModeSessions", "searchWorkspaceFiles", [folder, val.trim()])) || [];
            if (gen !== reqGen) return;
            fileHits = found;
          } else {
            fileHits = [];
          }
          buildEntries(val);
        });

        searchInput.addEventListener("keydown", (e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            if (!entries.length) return;
            e.preventDefault();
            selIdx = (selIdx + (e.key === "ArrowDown" ? 1 : -1) + entries.length) % entries.length;
            syncHighlight();
          } else if (e.key === "Enter" && entries[selIdx]) {
            e.preventDefault();
            const chosen = entries[selIdx];
            modal.close();
            void chosen.run();
          }
        });

        modal = dialog("Quick Jump", [searchInput, resultsHost], { wide: true });
        modal.classList.add("dsh-quick-launcher");
        buildEntries("");
        requestAnimationFrame(() => searchInput.focus());
      }

      const runCommand = (id) => {
        const app = ctx.app();
        if (!app) return;
        if (id === "palette.open") openQuickLauncher();
        else if (id === "shortcuts.open") openShortcutsModal();
        else if (id === "settings.open") app.navigate("settings");
        else if (id === "sidebar.toggle") {
          app.state.collapsed = !app.state.collapsed;
          app.render();
        } else if (id === "session.new") {
          app.saveDraft();
          app.state.project = null;
          app.state.folder = "";
          app.navigate("new");
        } else if (id === "session.search") app.navigate("search");
        else if (id === "find.in_page") {
          if (app.session()) app.openConversationFind?.();
          else app.navigate("search");
        } else if (id === "sidechat.open") {
          cordis._services.get("sideChat")?.value?.openSideChat?.("");
        } else if (id === "nav.back") {
          app.navBack?.();
        } else if (id === "nav.forward") {
          app.navForward?.();
        } else if (id === "plugins.open") app.navigate("plugins");
        else if (id === "session.fork") {
          const cur = app.session();
          if (cur) app.forkConversationBranch?.(cur);
          else app.toast("Open a conversation first to branch it.");
        }
      };

      function openShortcutsModal() {
        const app = ctx.app();
        if (!app) return;
        const { el, button, dialog, toast } = app;
        let filterText = "";
        let recordingId = null;
        const searchInput = el("input", {
          placeholder: "Filter actions or shortcuts…",
          "aria-label": "Filter shortcuts",
        });
        const listHost = el("div", { class: "cordis-shortcuts-list" });

        const draw = () => {
          const q = filterText.trim().toLowerCase();
          const rows = DEFAULTS.filter((d) => {
            const eff = overrides[d.id] ?? d.combo;
            return !q || d.label.toLowerCase().includes(q) || eff.toLowerCase().includes(q);
          }).map((d) => {
            const eff = overrides[d.id] ?? d.combo;
            const isRec = recordingId === d.id;
            const recordBtn = button(
              isRec ? "Press keys…" : formatCombo(eff),
              () => {
                recordingId = isRec ? null : d.id;
                draw();
              },
              "quiet cordis-keycap",
            );
            return el(
              "div",
              { class: "cordis-shortcut-row" },
              el("span", {}, d.label),
              el(
                "div",
                { class: "cordis-row-actions" },
                recordBtn,
                overrides[d.id] && overrides[d.id] !== d.combo
                  ? button("Reset", () => {
                      delete overrides[d.id];
                      safeStorageSet("quixi.keybindings.v1", overrides);
                      draw();
                    }, "quiet")
                  : null,
              ),
            );
          });
          listHost.replaceChildren(...rows);
        };

        searchInput.addEventListener("input", () => {
          filterText = searchInput.value;
          draw();
        });

        const modal = dialog("Keyboard shortcuts", [
          el("p", { class: "hint" }, "Click any shortcut badge to record a new keybinding. In the composer: Cmd/Ctrl+Enter steers while busy (or steers all queued messages when empty); Esc Esc stops the current turn."),
          searchInput,
          listHost,
          el(
            "div",
            { class: "dialog-actions" },
            button("Restore All Defaults", () => {
              overrides = {};
              safeStorageSet("quixi.keybindings.v1", overrides);
              recordingId = null;
              draw();
              toast("Restored default keyboard shortcuts.");
            }, "quiet"),
            button("Close", () => modal.close(), "primary"),
          ),
        ]);

        const onModalKey = (e) => {
          if (!recordingId) return;
          e.preventDefault();
          e.stopPropagation();
          if (e.key === "Escape") {
            recordingId = null;
            draw();
            return;
          }
          const combo = eventToCombo(e);
          if (!combo) return;
          overrides[recordingId] = combo;
          safeStorageSet("quixi.keybindings.v1", overrides);
          recordingId = null;
          draw();
        };
        modal.addEventListener("keydown", onModalKey, true);
        draw();
      }

      const onWindowKeyDown = (e) => {
        if (e.isComposing) return;
        const mod = isMac() ? e.metaKey : e.ctrlKey;
        if (mod && !e.altKey && e.key === "0") {
          fonts = { content: 14, code: 12, terminal: 13 };
          zoomPct = 100;
          safeStorageSet("quixi.fontSizes.v1", fonts);
          safeStorageSet("quixi.zoomPct.v1", zoomPct);
          applyFonts();
          e.preventDefault();
          return;
        }
        if (mod && !e.altKey && (e.key === "=" || e.key === "+" || e.key === "-")) {
          const delta = e.key === "-" ? -1 : 1;
          fonts = {
            content: Math.max(10, Math.min(22, fonts.content + delta)),
            code: Math.max(10, Math.min(18, fonts.code + delta)),
            terminal: Math.max(10, Math.min(20, fonts.terminal + delta)),
          };
          safeStorageSet("quixi.fontSizes.v1", fonts);
          applyFonts();
          e.preventDefault();
          return;
        }
        if (document.querySelector("dialog[open]")) return;
        const combo = eventToCombo(e);
        if (!combo) return;
        for (const d of DEFAULTS) {
          const bound = overrides[d.id] ?? d.combo;
          if (bound && bound.toLowerCase() === combo.toLowerCase()) {
            e.preventDefault();
            runCommand(d.id);
            return;
          }
        }
      };

      window.addEventListener("keydown", onWindowKeyDown);
      ctx.effect("window-keydown-shortcuts", () => () => window.removeEventListener("keydown", onWindowKeyDown));

      // Full-window File Drag-and-Drop Overlay (`dsh-client-ui-file-drag`):
      let dragDepth = 0;
      let dropBanner = null;
      const ensureDropBanner = () => {
        if (!dropBanner) {
          dropBanner = document.createElement("div");
          dropBanner.className = "dsh-drop-overlay";
          dropBanner.hidden = true;
          dropBanner.innerHTML = `<div class="dsh-drop-card"><strong>Drop files or images to attach</strong><span>PNG, JPEG, GIF, WebP, or UTF-8 text files</span></div>`;
          document.body.appendChild(dropBanner);
        }
        return dropBanner;
      };
      const hasFileTransfer = (ev) => Array.from(ev.dataTransfer?.types || []).includes("Files");
      const onDragEnter = (ev) => {
        if (!hasFileTransfer(ev)) return;
        dragDepth += 1;
        ensureDropBanner().hidden = false;
      };
      const onDragOver = (ev) => {
        if (!hasFileTransfer(ev)) return;
        ev.preventDefault();
      };
      const onDragLeave = (ev) => {
        if (!hasFileTransfer(ev)) return;
        dragDepth = Math.max(0, dragDepth - 1);
        if (dragDepth === 0 && dropBanner) dropBanner.hidden = true;
      };
      const onDrop = (ev) => {
        if (!hasFileTransfer(ev)) return;
        ev.preventDefault();
        dragDepth = 0;
        if (dropBanner) dropBanner.hidden = true;
        const files = ev.dataTransfer?.files;
        if (!files?.length) return;
        const app = ctx.app();
        if (!app) return;
        if (app.state.page !== "new" && app.state.page !== "chat") app.navigate("new");
        app.queueFiles?.(files);
        app.toast(`Attached ${files.length} file(s).`);
      };
      window.addEventListener("dragenter", onDragEnter);
      window.addEventListener("dragover", onDragOver);
      window.addEventListener("dragleave", onDragLeave);
      window.addEventListener("drop", onDrop);
      ctx.effect("window-file-drag", () => () => {
        window.removeEventListener("dragenter", onDragEnter);
        window.removeEventListener("dragover", onDragOver);
        window.removeEventListener("dragleave", onDragLeave);
        window.removeEventListener("drop", onDrop);
        dropBanner?.remove();
        dropBanner = null;
      });

      const svc = {
        openShortcutsModal,
        openQuickLauncher,
        getFontSizes: () => ({ ...fonts }),
        setFontSizes(next) {
          fonts = {
            content: Math.max(10, Math.min(22, Number(next.content) || 14)),
            code: Math.max(10, Math.min(18, Number(next.code) || 12)),
            terminal: Math.max(10, Math.min(20, Number(next.terminal) || 13)),
          };
          safeStorageSet("quixi.fontSizes.v1", fonts);
          applyFonts();
        },
        setZoomPct(nextPct) {
          zoomPct = Math.max(80, Math.min(150, Math.round(Number(nextPct) || 100)));
          safeStorageSet("quixi.zoomPct.v1", zoomPct);
          applyFonts();
        },
      };
      ctx.provide("shortcuts", svc);

      ctx.slot("settings.sections", {
        order: 35,
        label: "Typography & Shortcuts",
        render: ({ el, button }) => {
          const makeStepper = (label, axis) => {
            const valSpan = el("strong", {}, `${fonts[axis]}px`);
            return el(
              "div",
              { class: "cordis-row-item" },
              el("span", {}, label),
              el(
                "div",
                { class: "cordis-row-actions" },
                button("−", () => {
                  svc.setFontSizes({ ...fonts, [axis]: fonts[axis] - 1 });
                  valSpan.textContent = `${fonts[axis]}px`;
                }, "quiet"),
                valSpan,
                button("+", () => {
                  svc.setFontSizes({ ...fonts, [axis]: fonts[axis] + 1 });
                  valSpan.textContent = `${fonts[axis]}px`;
                }, "quiet"),
              ),
            );
          };
          const zoomSpan = el("strong", {}, `${zoomPct}%`);
          const zoomRow = el(
            "div",
            { class: "cordis-row-item" },
            el("span", {}, "Interface zoom scale (⌘0 to reset)"),
            el(
              "div",
              { class: "cordis-row-actions" },
              button("−", () => {
                svc.setZoomPct(zoomPct - 10);
                zoomSpan.textContent = `${zoomPct}%`;
              }, "quiet"),
              zoomSpan,
              button("+", () => {
                svc.setZoomPct(zoomPct + 10);
                zoomSpan.textContent = `${zoomPct}%`;
              }, "quiet"),
              button("100%", () => {
                svc.setZoomPct(100);
                zoomSpan.textContent = "100%";
              }, "quiet"),
            ),
          );
          return el(
            "section",
            { class: "cordis-bundle-card" },
            el("div", { class: "cordis-bundle-head" },
              el("div", {},
                el("h3", {}, "Typography, Zoom & Keyboard Shortcuts"),
                el("p", { class: "hint" }, "Independent font scales for prose, code blocks, and terminals, interface zoom, and customizable keybindings."),
              ),
              button(`Edit shortcuts (${modLabel()}/)`, () => openShortcutsModal(), "quiet"),
            ),
            zoomRow,
            makeStepper("Conversation text size", "content"),
            makeStepper("Code & diff size", "code"),
            makeStepper("Terminal size", "terminal"),
          );
        },
      });
    },
  });

  // =========================================================================
  // 2. `ui-plugin-manager` — Sidebar `Plugins` Manager & Live Fiber Inventory
  // =========================================================================
  cordis.definePlugin({
    id: "ui-plugin-manager",
    title: "Sidebar Plugins Manager & Live Bundle Reconciler",
    setup(ctx) {
      let activeTab = "installed"; // "installed" | "experimental" | "custom"
      let searchQuery = "";

      ctx.slot("sidebar.nav", {
        order: 60,
        label: "Plugins",
        render: ({ navigationButton, state, navigate }) =>
          navigationButton(
            "Plugins",
            "grid",
            () => navigate("plugins"),
            state.page === "plugins" ? "active" : "",
          ),
      });

      function openInstallBundleModal(app, refreshFn) {
        const { el, button, field, dialog, toast, rpc, navigate } = app;
        const idInput = el("input", { placeholder: "my-sidebar-notes", autocomplete: "off" });
        const titleInput = el("input", { placeholder: "Workspace Notes Panel", autocomplete: "off" });
        const descInput = el("input", { placeholder: "Adds a custom right-sidebar tab and prompt guidance." });
        const placementSelect = app.select(
          [
            ["sidebar-right", "Right sidebar tab"],
            ["page", "Global sidebar page"],
          ],
          "sidebar-right",
          () => {},
        );
        const promptInput = el("textarea", {
          rows: 3,
          placeholder: "Optional system-prompt section contributed while enabled…",
        });
        const detailsInput = el("textarea", {
          rows: 4,
          placeholder: "Markdown or notes content displayed in the personalized surface…",
        });
        const errP = el("p", { class: "error-text", hidden: true });

        const modal = dialog("Add or Create Plugin Bundle", [
          el("p", { class: "hint" }, "Install a hot-swappable Cordis bundle into your local profile (`$DATA_DIR/plugins/`), or open Creator Mode so the agent builds one for you."),
          field("Bundle ID", idInput),
          field("Display Title", titleInput),
          field("Description", descInput),
          field("UI Placement", placementSelect),
          field("System Prompt Contribution (optional)", promptInput),
          field("Panel Markdown Body", detailsInput),
          errP,
          el(
            "div",
            { class: "dialog-actions" },
            button("Let Agent Create in Creator Mode", () => {
              modal.close();
              app.state.draftPreset = "creator";
              app.navigate("new");
              const ta = document.getElementById("prompt");
              if (ta && !ta.value.trim()) {
                ta.value = "Create and install a new QuixiDesk plugin bundle for my workflow: ";
                app.saveDraft();
                ta.focus();
              }
              toast("Switched new task to Creator (`Cordis`) preset.");
            }, "quiet"),
            button("Cancel", () => modal.close(), "quiet"),
            button("Install & Enable Now", async () => {
              errP.hidden = true;
              const bid = idInput.value.trim();
              const title = titleInput.value.trim() || bid;
              if (!bid) {
                errP.textContent = "Enter a bundle ID (letters, digits, hyphens).";
                errP.hidden = false;
                return;
              }
              const promptSections = promptInput.value.trim()
                ? [{ name: `user:${bid}:prompt`, order: 9500, text: promptInput.value.trim() }]
                : [];
              const clientJs = `window.QuixiCordis?.definePlugin({
  id: ${JSON.stringify(bid)},
  title: ${JSON.stringify(title)},
  setup(ctx) {
    ctx.registerPersonalizedSurface({
      id: ${JSON.stringify(bid)},
      title: ${JSON.stringify(title)},
      brief: ${JSON.stringify(descInput.value.trim())},
      details: ${JSON.stringify(detailsInput.value || "# " + title)},
      placement: ${JSON.stringify(placementSelect.value)},
    });
  }
});`;
              const manifest = {
                id: bid,
                version: "0.1.0",
                title,
                description: descInput.value.trim() || "User-installed Cordis plugin bundle.",
                category: "Custom",
                official: false,
                experimental: false,
                listedByDefault: true,
                defaultEnabled: true,
                locked: false,
                declarative: {
                  placement: placementSelect.value,
                  promptSections,
                },
                rows: [
                  {
                    id: `${bid}-main`,
                    plugin: `user-bundle:${bid}`,
                    title,
                    description: descInput.value.trim() || "Custom plugin fiber",
                    disabled: false,
                    locked: false,
                    isolate: {},
                    config: {},
                  },
                ],
              };
              const res = await rpc("CordisPlugins", "installUserBundle", [
                { manifest, clientJs, readmeMd: detailsInput.value || `# ${title}` },
              ]);
              if (res?.error) {
                errP.textContent = res.error;
                errP.hidden = false;
                return;
              }
              cordis.syncFromHostSnapshot(res);
              modal.close();
              toast(`Installed and live-mounted "${title}".`);
              await refreshFn();
            }, "primary"),
          ),
        ], { wide: true });
      }

      function openRowConfigDialog(app, row, refreshFn) {
        const { el, button, field, dialog, toast, rpc } = app;
        const fields = row.configSchema?.fields || [];
        const current = { ...(row.config || {}) };
        const controls = new Map();

        const formNodes = fields.map((f) => {
          if (f.type === "boolean") {
            const cb = el("input", { type: "checkbox", checked: Boolean(current[f.key]) });
            controls.set(f, () => cb.checked);
            return el("label", { class: "mcp-checkbox" }, cb, " " + f.label);
          }
          const inp = el("input", {
            type: f.type === "password" ? "password" : f.type === "number" ? "number" : "text",
            value: current[f.key] ?? "",
            ...(f.min !== undefined ? { min: String(f.min) } : {}),
            ...(f.max !== undefined ? { max: String(f.max) } : {}),
          });
          controls.set(f, () => (f.type === "number" ? Number(inp.value) : inp.value));
          return field(f.label, inp);
        });

        const modal = dialog(`Configure ${row.title}`, [
          el("p", { class: "hint" }, `Plugin row "${row.id}" (${row.plugin}). Saving hot-reloads this Fiber in place.`),
          ...formNodes,
          el(
            "div",
            { class: "dialog-actions" },
            button("Cancel", () => modal.close(), "quiet"),
            button("Save & Reconcile Live", async () => {
              const nextCfg = { ...current };
              for (const [f, getter] of controls.entries()) {
                nextCfg[f.key] = getter();
              }
              const snap = await rpc("CordisPlugins", "setPluginConfig", [row.id, nextCfg]);
              if (snap?.error) {
                toast(snap.error, true);
                return;
              }
              cordis.syncFromHostSnapshot(snap);
              modal.close();
              toast(`Updated "${row.title}" live.`);
              await refreshFn();
            }, "primary"),
          ),
        ]);
      }

      async function openBundleReadmeDialog(app, bundle) {
        const { el, button, dialog, plainBody, rpc } = app;
        const detail = await rpc("CordisPlugins", "readBundleDetail", [bundle.id]);
        const md = detail?.markdown || bundle.description || "No README available.";
        const modal = dialog(bundle.title, [
          el("p", { class: "hint" }, `${bundle.id} · v${bundle.version} · Source: ${bundle.source}`),
          plainBody(md),
          el("div", { class: "dialog-actions" }, button("Close", () => modal.close(), "primary")),
        ], { wide: true });
      }

      ctx.slot("main.page", {
        key: "plugins",
        label: "Plugins",
        render: (app) => {
          const { el, button, header, empty, toast, rpc, confirmDialog } = app;
          const container = el("div", { class: "cordis-plugins-page" });
          const listHost = el("div", { class: "cordis-bundle-grid" });

          const searchInput = el("input", {
            value: searchQuery,
            placeholder: "Filter bundles, rows, services, or capabilities…",
            "aria-label": "Filter plugins",
          });

          const tabBar = el("div", { class: "cordis-segmented", role: "tablist" });
          const renderTabs = () => {
            tabBar.replaceChildren(
              ...[
                ["installed", "Built-in & Active"],
                ["experimental", "Experimental"],
                ["custom", "Personalized & Custom"],
              ].map(([id, label]) =>
                el(
                  "button",
                  {
                    type: "button",
                    role: "tab",
                    class: activeTab === id ? "active" : "",
                    "aria-selected": String(activeTab === id),
                    onClick: () => {
                      activeTab = id;
                      renderTabs();
                      drawList(cordis.hostSnapshot());
                    },
                  },
                  label,
                ),
              ),
            );
          };

          const refreshFromHost = async () => {
            const snap = await rpc("CordisPlugins", "snapshot", []);
            if (snap && !snap.error) {
              cordis.syncFromHostSnapshot(snap);
              drawList(snap);
            }
          };

          const drawList = (snap) => {
            if (!snap || !Array.isArray(snap.bundles)) {
              listHost.replaceChildren(el("p", { class: "hint" }, "Loading Cordis fiber graph…"));
              return;
            }
            const q = searchQuery.trim().toLowerCase();
            const filtered = snap.bundles.filter((b) => {
              if (b.listedByDefault === false && !b.enabled) return false;
              if (activeTab === "experimental" && !b.experimental) return false;
              if (activeTab === "custom" && b.official) return false;
              if (activeTab === "installed" && b.experimental) return false;
              if (!q) return true;
              return (
                b.title.toLowerCase().includes(q) ||
                b.id.toLowerCase().includes(q) ||
                b.description.toLowerCase().includes(q) ||
                (b.rows || []).some(
                  (r) =>
                    r.title.toLowerCase().includes(q) ||
                    r.id.toLowerCase().includes(q) ||
                    r.plugin.toLowerCase().includes(q),
                )
              );
            });

            if (!filtered.length) {
              listHost.replaceChildren(
                empty(
                  activeTab === "custom" ? "No custom plugins installed yet" : "No matching plugins",
                  activeTab === "custom"
                    ? "Click “Add plugin” to author a bundle or ask Creator Mode to build a personalized panel."
                    : "Try clearing the filter or switching tabs.",
                  button("Add plugin", () => openInstallBundleModal(app, refreshFromHost), "primary"),
                ),
              );
              return;
            }

            listHost.replaceChildren(
              ...filtered.map((bundle) => {
                const bundleToggle = el("input", {
                  type: "checkbox",
                  class: "cordis-switch",
                  checked: Boolean(bundle.enabled),
                  disabled: Boolean(bundle.locked),
                  "aria-label": `Enable bundle ${bundle.title}`,
                });
                bundleToggle.addEventListener("change", async () => {
                  bundleToggle.disabled = true;
                  const nextSnap = await rpc("CordisPlugins", "setBundleEnabled", [
                    bundle.id,
                    bundleToggle.checked,
                  ]);
                  if (nextSnap?.error) {
                    toast(nextSnap.error, true);
                    bundleToggle.checked = !bundleToggle.checked;
                    bundleToggle.disabled = Boolean(bundle.locked);
                    return;
                  }
                  cordis.syncFromHostSnapshot(nextSnap);
                  toast(
                    `${bundle.title} ${bundleToggle.checked ? "enabled" : "disabled"} live (zero restart).`,
                  );
                  drawList(nextSnap);
                });

                const rowsEl = el(
                  "div",
                  { class: "cordis-rows-list" },
                  ...(bundle.rows || []).map((row) => {
                    const rowToggle = el("input", {
                      type: "checkbox",
                      class: "cordis-switch",
                      checked: Boolean(row.enabled),
                      disabled: Boolean(row.locked || !bundle.enabled),
                      "aria-label": `Enable plugin ${row.title}`,
                    });
                    rowToggle.addEventListener("change", async () => {
                      rowToggle.disabled = true;
                      const nextSnap = await rpc("CordisPlugins", "setPluginEnabled", [
                        row.id,
                        rowToggle.checked,
                      ]);
                      if (nextSnap?.error) {
                        toast(nextSnap.error, true);
                        rowToggle.checked = !rowToggle.checked;
                        rowToggle.disabled = Boolean(row.locked || !bundle.enabled);
                        return;
                      }
                      cordis.syncFromHostSnapshot(nextSnap);
                      toast(`${row.title} ${rowToggle.checked ? "mounted" : "unloaded"} live.`);
                      drawList(nextSnap);
                    });

                    return el(
                      "div",
                      { class: "cordis-row-item" },
                      el(
                        "div",
                        { class: "cordis-row-meta" },
                        el("span", {
                          class: `cordis-phase-dot ${row.phase || "inactive"}`,
                          title: `Fiber phase: ${row.phase}`,
                        }),
                        el(
                          "div",
                          {},
                          el("strong", {}, row.title),
                          el("small", { class: "hint" }, ` · ${row.id} (${row.phase})`),
                          row.error ? el("p", { class: "error-text" }, row.error) : null,
                        ),
                      ),
                      el(
                        "div",
                        { class: "cordis-row-actions" },
                        row.configSchema
                          ? button("Configure", () => openRowConfigDialog(app, row, refreshFromHost), "quiet")
                          : null,
                        rowToggle,
                      ),
                    );
                  }),
                );

                return el(
                  "article",
                  { class: "cordis-bundle-card", "data-bundle-id": bundle.id },
                  el(
                    "div",
                    { class: "cordis-bundle-head" },
                    el(
                      "div",
                      {},
                      el(
                        "div",
                        { class: "cordis-bundle-title-row" },
                        el("h3", {}, bundle.title),
                        el("span", { class: "cordis-badge" }, `v${bundle.version}`),
                        bundle.official
                          ? el("span", { class: "cordis-badge official" }, "Official")
                          : el("span", { class: "cordis-badge" }, "Custom"),
                        bundle.experimental
                          ? el("span", { class: "cordis-badge experimental" }, "Experimental")
                          : null,
                        bundle.locked ? el("span", { class: "cordis-badge" }, "Core Spine") : null,
                      ),
                      el("p", { class: "hint" }, bundle.description),
                    ),
                    el(
                      "div",
                      { class: "cordis-row-actions" },
                      bundle.hasReadme
                        ? button("Details", () => openBundleReadmeDialog(app, bundle), "quiet")
                        : null,
                      bundle.removable
                        ? button("Uninstall", () => {
                            confirmDialog(
                              `Uninstall ${bundle.title}?`,
                              `Unload all fibers in "${bundle.id}" and remove its bundle folder from your profile?`,
                              async () => {
                                const nextSnap = await rpc("CordisPlugins", "removeUserBundle", [bundle.id]);
                                if (nextSnap?.error) throw new Error(nextSnap.error);
                                cordis.syncFromHostSnapshot(nextSnap);
                                toast(`Uninstalled "${bundle.title}".`);
                                drawList(nextSnap);
                              },
                              "Uninstall",
                            );
                          }, "quiet")
                        : null,
                      bundleToggle,
                    ),
                  ),
                  rowsEl,
                );
              }),
            );
          };

          searchInput.addEventListener("input", () => {
            searchQuery = searchInput.value;
            drawList(cordis.hostSnapshot());
          });

          renderTabs();
          container.append(
            header(
              "Plugins",
              "Spatiotemporal Cordis bundles — toggle Host fibers, capabilities, guards, and UI slots live without restarting QuixiDesk.",
              el(
                "div",
                { class: "cordis-row-actions" },
                button("Refresh", () => refreshFromHost(), "quiet"),
                button("Add plugin", () => openInstallBundleModal(app, refreshFromHost), "primary", "plus"),
              ),
            ),
            el("div", { class: "cordis-plugins-toolbar" }, tabBar, searchInput),
            listHost,
          );

          if (cordis.hostSnapshot()) {
            drawList(cordis.hostSnapshot());
          } else {
            void refreshFromHost();
          }
          return container;
        },
      });

      // Read-only Host + Client Fiber Inventory in Settings (`ui-settings-plugin-inventory`):
      ctx.slot("settings.sections", {
        order: 40,
        label: "Built-in Cordis Inventory",
        render: ({ el, button, navigate }) => {
          const snap = cordis.hostSnapshot();
          const fibers = snap?.fibers || [];
          const client = cordis.inspectClient();
          return el(
            "section",
            { class: "cordis-bundle-card" },
            el(
              "div",
              { class: "cordis-bundle-head" },
              el(
                "div",
                {},
                el("h3", {}, "Cordis Spatiotemporal Fiber Inventory"),
                el(
                  "p",
                  { class: "hint" },
                  `${fibers.filter((f) => f.state?.phase === "active").length}/${fibers.length} Host Rust fibers active · ${client.fibers.filter((f) => f.state === "active").length} Client UI fibers active · ${client.slots.length} typed UI slots.`,
                ),
              ),
              button("Manage in Plugins", () => navigate("plugins"), "quiet"),
            ),
          );
        },
      });
    },
  });

  // =========================================================================
  // 3. `ui-model-effort-and-presets` — Effort Selector, Agent Presets & Worktree
  // =========================================================================
  cordis.definePlugin({
    id: "ui-model-effort-and-presets",
    title: "Composer Reasoning Effort, Agent Presets & Worktree Controls",
    setup(ctx) {
      let cachedPresets = null;

      async function ensurePresets(rpc) {
        if (cachedPresets) return cachedPresets;
        try {
          const res = await rpc("CordisPlugins", "listPresets", []);
          if (res?.presets) cachedPresets = res;
        } catch {}
        return (
          cachedPresets || {
            defaultPreset: "standard",
            presets: [
              { id: "standard", title: "Standard", badge: "Default" },
              { id: "ptc", title: "PTC (RunCode)", badge: "Code SDK" },
              { id: "minimal", title: "Minimal", badge: "SWE-Bench" },
              { id: "creator", title: "Creator", badge: "Cordis" },
            ],
          }
        );
      }

      ctx.slot("composer.controls", {
        order: 20,
        label: "Effort & Preset Controls",
        render: (app) => {
          const { el, button, session, state, service, rpc, toast, dialog, plainBody } = app;
          const s = session();
          const currentEffort = s ? s.effortLevel || s.effort || "medium" : state.draftEffort || "medium";
          const effortSel = el(
            "select",
            {
              class: "cordis-composer-select",
              "aria-label": "Reasoning effort",
              title: "Reasoning effort level",
            },
            ...[
              ["off", "Off"],
              ["low", "Low"],
              ["medium", "Medium"],
              ["high", "High"],
              ["max", "Max"],
            ].map(([val, label]) =>
              el("option", { value: val, ...(currentEffort === val ? { selected: true } : {}) }, label),
            ),
          );
          effortSel.value = currentEffort;
          effortSel.addEventListener("change", async () => {
            const val = effortSel.value;
            const cur = session();
            if (cur) {
              cur.effortLevel = val;
              cur.effort = val;
              await rpc(service(cur), "setEffort", [cur.sessionId, val]);
              toast(`Reasoning effort set to ${val}.`);
            } else {
              state.draftEffort = val;
            }
          });

          const activePreset = s
            ? state.collab.get(s.sessionId)?.agentPreset || "standard"
            : state.draftPreset || cordis.hostSnapshot()?.defaultPreset || "standard";

          const presetSel = el(
            "select",
            {
              class: "cordis-composer-select",
              "aria-label": "Agent preset",
              title: "Declarative Agent Preset (`Standard`, `PTC`, `Minimal`, `Creator`)",
            },
            ...[
              ["standard", "Standard"],
              ["ptc", "PTC"],
              ["minimal", "Minimal"],
              ["creator", "Creator"],
            ].map(([val, label]) =>
              el("option", { value: val, ...(activePreset === val ? { selected: true } : {}) }, label),
            ),
          );
          presetSel.value = activePreset;
          presetSel.addEventListener("change", async () => {
            const val = presetSel.value;
            const cur = session();
            if (cur) {
              const updated = await rpc(service(cur), "setAgentPreset", [cur.sessionId, val]);
              if (updated && !updated.error) state.collab.set(cur.sessionId, updated);
              toast(`Session preset switched to "${val}".`);
            } else {
              state.draftPreset = val;
              toast(`Staged "${val}" preset for this new task.`);
            }
          });

          const presetHelpBtn = button("?", async () => {
            const roster = await ensurePresets(rpc);
            const curId = presetSel.value;
            const info = roster.presets.find((p) => p.id === curId);
            const yamlRes = await rpc("CordisPlugins", "readPresetYaml", [curId]);
            const d = dialog(`Agent Preset: ${info?.title || curId}`, [
              plainBody(info?.guideMarkdown || info?.description || ""),
              yamlRes?.yaml
                ? el("details", {}, el("summary", {}, "View Cordis composition YAML"), el("pre", { class: "code-preview" }, yamlRes.yaml))
                : null,
              el(
                "div",
                { class: "dialog-actions" },
                button("Set as Default for New Tasks", async () => {
                  await rpc("CordisPlugins", "setDefaultPreset", [curId]);
                  cachedPresets = null;
                  toast(`"${info?.title || curId}" is now the default new-task preset.`);
                }, "quiet"),
                button("Close", () => d.close(), "primary"),
              ),
            ]);
          }, "quiet");
          presetHelpBtn.title = "Inspect Agent Preset guide and YAML composition";
          presetHelpBtn.setAttribute("aria-label", "Inspect Agent Preset");

          const nodes = [presetSel, presetHelpBtn, effortSel];

          // On blank New Task with a folder selected, offer isolated Git Worktree toggle (`client-ui-forge`):
          if (!s && app.currentFolder()) {
            const wtCheckbox = el("input", {
              type: "checkbox",
              checked: Boolean(state.draftWorktree),
              "aria-label": "Isolate in Git worktree",
            });
            wtCheckbox.addEventListener("change", () => {
              state.draftWorktree = wtCheckbox.checked;
            });
            const wtLabel = el(
              "label",
              {
                class: "cordis-context-pill",
                title: "Create an isolated checkout under .agents/worktrees/ for this task",
              },
              wtCheckbox,
              "Worktree",
            );
            nodes.push(wtLabel);
          }

          return nodes;
        },
      });
    },
  });

  // =========================================================================
  // 4. `ui-conversation-views` — `Chat` | `Trajectory` | `Statistics` & Work Details
  // =========================================================================
  cordis.definePlugin({
    id: "ui-conversation-views",
    title: "Trajectory, Statistics & Work Details Folding",
    hostRowId: "ui-views",
    setup(ctx) {
      let currentViewBySession = new Map();
      let workDetailsMode = safeStorageGet("quixi.workDetails.v1", "standard");

      ctx.slot("conversation.views", {
        order: 10,
        label: "Conversation Views",
        render: (app) => {
          const s = app.session();
          if (!s) return null;
          const { el } = app;
          const activeView = currentViewBySession.get(s.sessionId) || "chat";

          const tabs = el(
            "div",
            { class: "cordis-segmented", role: "tablist", "aria-label": "Conversation views" },
            ...[
              ["chat", "Chat"],
              ["trajectory", "Trajectory"],
              ["statistics", "Statistics"],
            ].map(([id, label]) =>
              el(
                "button",
                {
                  type: "button",
                  role: "tab",
                  class: activeView === id ? "active" : "",
                  "aria-selected": String(activeView === id),
                  onClick: () => {
                    currentViewBySession.set(s.sessionId, id);
                    app.renderPage();
                  },
                },
                label,
              ),
            ),
          );

          const wdSelect = el(
            "select",
            {
              class: "cordis-composer-select",
              "aria-label": "Work details mode",
              title: "Control intermediate tool/thinking folding in Chat",
            },
            ...[
              ["compact", "Details: Compact"],
              ["standard", "Details: Standard"],
              ["detailed", "Details: Detailed"],
              ["verbose", "Details: Verbose"],
            ].map(([val, label]) =>
              el("option", { value: val, ...(workDetailsMode === val ? { selected: true } : {}) }, label),
            ),
          );
          wdSelect.value = workDetailsMode;
          wdSelect.addEventListener("change", () => {
            workDetailsMode = wdSelect.value;
            safeStorageSet("quixi.workDetails.v1", workDetailsMode);
            const conv = document.getElementById("conversation");
            if (conv) conv.dataset.workDetails = workDetailsMode;
            app.renderTranscript();
          });

          return {
            activeView,
            workDetailsMode,
            inlineControls: tabs,
            trailingControl: activeView === "chat" ? wdSelect : null,
            bar: el("div", { class: "cordis-view-bar" }, tabs, activeView === "chat" ? wdSelect : el("span")),
            renderAlternate: () =>
              activeView === "trajectory"
                ? renderTrajectoryView(app, s)
                : activeView === "statistics"
                  ? renderStatisticsView(app, s)
                  : null,
          };
        },
      });

      function synthesizeStepsFromMessages(messages, defaultModel) {
        const out = [];
        let turnIdx = -1;
        let stepIdx = 0;
        for (const m of messages || []) {
          const blocks = Array.isArray(m.message?.content)
            ? m.message.content
            : Array.isArray(m.content)
              ? m.content
              : [];
          if (m.type === "user" && blocks.some((b) => b.type === "text")) {
            turnIdx += 1;
            stepIdx = 0;
          } else if (m.type === "assistant") {
            const tools = blocks.filter((b) => b.type === "tool_use");
            const texts = blocks.filter((b) => b.type === "text").map((b) => b.text || "").join("\n");
            if (!tools.length && !texts.trim()) continue;
            const usage = m.message?.usage || m.usage || {};
            out.push({
              turnIndex: Math.max(0, turnIdx),
              stepIndex: stepIdx++,
              model: m.message?.model || defaultModel || "model",
              ttftMs: usage.ttft_ms || 0,
              modelMs: usage.duration_ms || 0,
              toolMs: 0,
              inputTokens: usage.input_tokens || 0,
              outputTokens: usage.output_tokens || Math.max(1, Math.round(texts.length / 4)),
              cacheReadTokens: usage.cache_read_input_tokens || 0,
              cacheWriteTokens: usage.cache_creation_input_tokens || 0,
              reasoningTokens: usage.reasoning_tokens || 0,
              contextTokens: usage.input_tokens || 0,
              toolNames: tools.map((t) => t.name),
              toolInputs: tools.map((t) => ({ name: t.name, input: t.input })),
              ptcSubcalls: [],
            });
          }
        }
        return out;
      }

      function renderTrajectoryView(app, s) {
        const { el, empty } = app;
        const collab = app.state.collab.get(s.sessionId);
        const messages =
          s._kind === "code" ? app.state.transcripts.get(s.sessionId) || [] : s.bufferedMessages || [];
        const rawSteps = collab?.stepTelemetry || [];
        const steps = rawSteps.length ? rawSteps : synthesizeStepsFromMessages(messages, s.model);

        if (!steps.length) {
          return el(
            "div",
            { class: "cordis-alt-view", "aria-label": "Trajectory view" },
            empty("No trajectory steps yet", "Send a prompt to inspect step timing, tool calls, and nested PTC subcalls."),
          );
        }

        const totalMs = Math.max(
          1,
          steps.reduce((acc, st) => acc + (st.modelMs || 0) + (st.toolMs || 0), 0),
        );

        const timeline = steps.length
          ? el(
              "div",
              {},
              el("p", { class: "hint" }, `Timing Overview · ${steps.length} step(s) · Amber = TTFT, Blue = Decoding, Green = Tools`),
              el(
                "div",
                { class: "cordis-timeline-bar" },
                ...steps.flatMap((st, i) => {
                  const ttft = Math.min(st.ttftMs || 0, st.modelMs || 0);
                  const decode = Math.max(0, (st.modelMs || 0) - ttft);
                  const tool = st.toolMs || 0;
                  return [
                    ttft > 0
                      ? el("span", {
                          class: "cordis-span-ttft",
                          style: `width:${Math.max(1, (ttft / totalMs) * 100)}%`,
                          title: `Step ${i + 1} TTFT: ${ttft}ms`,
                        })
                      : null,
                    decode > 0
                      ? el("span", {
                          class: "cordis-span-model",
                          style: `width:${Math.max(1, (decode / totalMs) * 100)}%`,
                          title: `Step ${i + 1} Decode: ${decode}ms`,
                        })
                      : null,
                    tool > 0
                      ? el("span", {
                          class: "cordis-span-tool",
                          style: `width:${Math.max(1, (tool / totalMs) * 100)}%`,
                          title: `Step ${i + 1} Tools: ${tool}ms`,
                        })
                      : null,
                  ].filter(Boolean);
                }),
              ),
            )
          : null;

        const ledger = steps.map((st, idx) =>
          el(
            "details",
            { class: "cordis-bundle-card", open: idx === steps.length - 1 },
            el(
              "summary",
              {},
              el("strong", {}, `Turn ${st.turnIndex + 1} · Step ${st.stepIndex + 1}`),
              el(
                "span",
                { class: "hint" },
                ` — ${st.model} · TTFT ${st.ttftMs}ms · Model ${st.modelMs}ms · Tools ${st.toolMs}ms${st.toolNames?.length ? " · [" + st.toolNames.join(", ") + "]" : ""}`,
              ),
            ),
            el(
              "pre",
              { class: "code-preview" },
              JSON.stringify(
                {
                  tokens: {
                    input: st.inputTokens,
                    output: st.outputTokens,
                    cacheRead: st.cacheReadTokens,
                    cacheWrite: st.cacheWriteTokens,
                    reasoning: st.reasoningTokens,
                    contextOccupancy: st.contextTokens,
                  },
                  tools: st.toolNames,
                  ptcSubcalls: st.ptcSubcalls || [],
                },
                null,
                2,
              ),
            ),
          ),
        );

        return el("div", { class: "cordis-alt-view", "aria-label": "Trajectory view" }, timeline, ...ledger);
      }

      function renderStatisticsView(app, s) {
        const { el, empty } = app;
        const collab = app.state.collab.get(s.sessionId);
        const messages =
          s._kind === "code" ? app.state.transcripts.get(s.sessionId) || [] : s.bufferedMessages || [];
        const rawSteps = collab?.stepTelemetry || [];
        const steps = rawSteps.length ? rawSteps : synthesizeStepsFromMessages(messages, s.model);
        if (!steps.length) {
          return el(
            "div",
            { class: "cordis-alt-view", "aria-label": "Statistics view" },
            empty("No step statistics recorded yet", "Run a turn to inspect context occupancy, cache hit %, TTFT, and tok/s."),
          );
        }

        let sumIn = 0,
          sumOut = 0,
          sumCache = 0,
          sumModelMs = 0,
          sumToolMs = 0;

        const rows = steps.map((st, idx) => {
          sumIn += st.inputTokens || 0;
          sumOut += st.outputTokens || 0;
          sumCache += st.cacheReadTokens || 0;
          sumModelMs += st.modelMs || 0;
          sumToolMs += st.toolMs || 0;
          const denom = (st.inputTokens || 0) + (st.cacheReadTokens || 0);
          const cachePct = denom > 0 ? Math.round(((st.cacheReadTokens || 0) / denom) * 100) : 0;
          const tokPerSec =
            st.modelMs > 0 ? (((st.outputTokens || 0) * 1000) / st.modelMs).toFixed(1) : "—";
          return el(
            "tr",
            {},
            el("td", {}, `#${idx + 1} (T${st.turnIndex + 1}.${st.stepIndex + 1})`),
            el("td", {}, st.model || "—"),
            el("td", {}, String(st.contextTokens || st.inputTokens || 0)),
            el("td", {}, `${cachePct}%`),
            el("td", {}, `${st.ttftMs || 0}ms`),
            el("td", {}, `${st.modelMs || 0}ms`),
            el("td", {}, `${st.toolMs || 0}ms`),
            el("td", {}, `${tokPerSec}`),
          );
        });

        const totalDenom = sumIn + sumCache;
        const totalCachePct = totalDenom > 0 ? Math.round((sumCache / totalDenom) * 100) : 0;
        const avgTokSec = sumModelMs > 0 ? ((sumOut * 1000) / sumModelMs).toFixed(1) : "—";

        const table = el(
          "table",
          { class: "cordis-stats-table" },
          el(
            "thead",
            {},
            el(
              "tr",
              {},
              el("th", {}, "Step"),
              el("th", {}, "Model"),
              el("th", {}, "Context Tok"),
              el("th", {}, "Cache Hit"),
              el("th", {}, "TTFT"),
              el("th", {}, "Model Time"),
              el("th", {}, "Tool Time"),
              el("th", {}, "Out tok/s"),
            ),
          ),
          el("tbody", {}, ...rows),
          el(
            "tfoot",
            {},
            el(
              "tr",
              {},
              el("td", {}, `Total (${steps.length} steps)`),
              el("td", {}, "—"),
              el("td", {}, `${sumIn + sumOut} billed`),
              el("td", {}, `${totalCachePct}%`),
              el("td", {}, "—"),
              el("td", {}, `${sumModelMs}ms`),
              el("td", {}, `${sumToolMs}ms`),
              el("td", {}, `${avgTokSec}`),
            ),
          ),
        );

        return el("div", { class: "cordis-alt-view", "aria-label": "Statistics view" }, table);
      }
    },
  });

  // =========================================================================
  // 5. `ui-secret-mask` — Expanded User Bubble Credential Masking Waterfall
  // =========================================================================
  cordis.definePlugin({
    id: "ui-secret-mask",
    title: "User Bubble Credential Masking",
    hostRowId: "ui-secret-mask",
    setup(ctx) {
      const TOKEN_RE =
        /(-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----|\b(?:sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{16,}|AIza[0-9A-Za-z_-]{30,}|eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})\b)/g;

      ctx.onWaterfall("ui/mask-secrets", 10, (payload, next) => {
        const raw = String(payload?.raw ?? "");
        TOKEN_RE.lastIndex = 0;
        if (!TOKEN_RE.test(raw)) return next(payload);
        TOKEN_RE.lastIndex = 0;
        const masked = raw.replace(TOKEN_RE, (m) => {
          if (m.startsWith("-----BEGIN")) return "-----BEGIN PRIVATE KEY----- [MASKED] -----END PRIVATE KEY-----";
          return m.slice(0, 6) + "••••••••" + m.slice(-3);
        });
        return next({ ...payload, masked, hadSecrets: true });
      });
    },
  });

  // =========================================================================
  // 6. `ui-action-cards-and-forge` — Action Cards, Forge PRs, Context Ring & Fork
  // =========================================================================
  cordis.definePlugin({
    id: "ui-action-cards-and-forge",
    title: "Action Cards, GitHub Forge PRs, Personalized Drawer & Session Fork",
    setup(ctx) {
      // Turn-tail Action Cards (`client-ui-action-cards` + `creation-recommendations`):
      ctx.slot("chat.turn-tail", {
        order: 20,
        label: "Action Cards",
        render: (app) => {
          const s = app.session();
          if (!s) return null;
          const { el, button, select, rpc, service, toast } = app;
          const collab = app.state.collab.get(s.sessionId);
          const cards = (collab?.actionCards || []).filter((c) => c.status !== "replaced" && c.status !== "dismissed");
          const sessionArtifacts = (app.state.artifacts || []).filter((a) => a.createdBySessionId === s.sessionId);
          const outNodes = [];
          for (const art of sessionArtifacts) {
            outNodes.push(
              el(
                "div",
                { class: "dsh-inline-artifact-card", "aria-label": "Session artifact" },
                app.icon("artifacts", 16),
                el("div", { class: "grow" },
                  el("strong", {}, art.name || art.title || "Artifact"),
                  el("small", { class: "hint" }, ` · Interactive HTML Studio · Revision ${art.revision || 1}`),
                ),
                button("Open Studio", () => app.openArtifact?.(art.id), "primary"),
              ),
            );
          }

          if (!cards.length) return outNodes.length ? outNodes : null;

          const actionNodes = cards.map((card) => {
            let chosenPlacement = card.placement || "sidebar-right";
            const placementSel =
              card.action === "create_personalized_page" && card.status === "pending"
                ? select(
                    [
                      ["sidebar-right", "Place in: Right sidebar"],
                      ["page", "Place in: Global page"],
                      ["sidebar-left", "Place in: Left navigation"],
                    ],
                    chosenPlacement,
                    (v) => {
                      chosenPlacement = v;
                    },
                  )
                : null;

            const act = async (decision) => {
              const res = await rpc(service(s), "resolveActionCard", [
                s.sessionId,
                card.id,
                decision,
                chosenPlacement,
              ]);
              if (res?.error) {
                toast(res.error, true);
                return;
              }
              const freshSnap = await rpc("CordisPlugins", "snapshot", []);
              if (freshSnap && !freshSnap.error) cordis.syncFromHostSnapshot(freshSnap);
              await app.refreshSession(s.sessionId);
              app.render();
              toast(
                decision === "undo"
                  ? `Disabled "${card.title}" (files preserved for Restore).`
                  : decision === "dismiss"
                    ? "Dismissed recommendation."
                    : `Activated "${card.title}" live!`,
              );
            };

            return el(
              "section",
              { class: "cordis-action-card", "aria-label": "Recommended action" },
              el(
                "div",
                { class: "cordis-bundle-title-row" },
                el(
                  "span",
                  { class: "cordis-badge experimental" },
                  card.action === "enable_official_plugin" ? "Official Plugin" : "Personalized UI Plugin",
                ),
                el("strong", {}, card.title),
                el("small", { class: "hint" }, `(${card.status})`),
              ),
              el("p", {}, card.brief),
              el(
                "div",
                { class: "cordis-action-card-controls" },
                placementSel,
                card.status === "pending"
                  ? button(
                      card.action === "enable_official_plugin" ? "Enable" : "Create & Mount Live",
                      () => act("accept"),
                      "primary",
                    )
                  : null,
                card.status === "pending" ? button("Not now", () => act("dismiss"), "quiet") : null,
                card.status === "accepted" ? button("Undo", () => act("undo"), "quiet") : null,
                card.status === "undone" ? button("Restore", () => act("restore"), "primary") : null,
              ),
            );
          });
          return [...outNodes, ...actionNodes];
        },
      });

      // Session Header Badges: Worktree + GitHub Forge PRs + Right-Sidebar Personalized Tabs
      ctx.slot("session.heading.extras", {
        order: 20,
        label: "Worktree, Forge PRs & Personalized Tabs",
        render: (app) => {
          const s = app.session();
          if (!s) return null;
          const { el, button, dialog, toast, rpc } = app;
          const collab = app.state.collab.get(s.sessionId);
          const nodes = [];

          const subagents = app.state.subagents?.get(s.sessionId) || [];
          if (subagents.length) {
            const runningSubs = subagents.filter((c) => c.status === "running").length;
            const subLabel = runningSubs ? `Subagents · ${runningSubs} active` : `Subagents · ${subagents.length}`;
            nodes.push(
              button(
                subLabel,
                () => {
                  const listEl = el("div", { class: "cordis-bundle-grid" },
                    ...subagents.map((child) =>
                      el("div", { class: "cordis-bundle-card" },
                        el("div", { class: "cordis-bundle-title-row" },
                          el("span", { class: "cordis-phase-dot " + (child.status === "completed" ? "active" : child.status === "running" ? "loading" : "failed") }),
                          el("strong", {}, child.description || child.label || child.id),
                          el("span", { class: "cordis-badge" }, child.status || "settled"),
                          child.model ? el("small", { class: "hint" }, child.model) : null,
                        ),
                        child.output || child.summary
                          ? el("pre", { class: "code-preview" }, String(child.output || child.summary).slice(0, 4000))
                          : el("p", { class: "hint" }, child.status === "running" ? "Subagent is currently working…" : "Completed."),
                      ),
                    ),
                  );
                  const d = dialog("Subagent Lineage", [
                    listEl,
                    el("div", { class: "dialog-actions" }, button("Close", () => d.close(), "primary")),
                  ], { wide: true });
                },
                "quiet" + (runningSubs ? " active" : ""),
              ),
            );
          }

          if (collab?.worktree?.branch) {
            nodes.push(
              el(
                "span",
                {
                  class: "cordis-context-pill",
                  title: `Isolated Git worktree: ${collab.worktree.path} (origin: ${collab.worktree.originCwd})`,
                },
                `⎇ ${collab.worktree.branch}`,
              ),
            );
          }

          if (cordis.isHostRowActive("forge-github")) {
            const prs = collab?.forgePrs || [];
            const label = prs.length ? `PR #${prs[0].number} (${prs[0].state})` : "PRs";
            const prBtn = el(
              "button",
              {
                type: "button",
                class: "cordis-pr-pill",
                title: "Inspect GitHub Pull Requests, CI check rollups, and diffs",
                onClick: async () => {
                  const refreshed = await rpc("Forge", "refreshSession", [s.sessionId]);
                  const list = refreshed?.prs || prs;
                  const body = el("div", { class: "cordis-bundle-grid" });
                  if (!list.length) {
                    body.append(
                      el(
                        "p",
                        { class: "hint" },
                        refreshed?.error ||
                          "No open GitHub PR detected on the current branch yet. Push a branch or run `gh pr create` to bind one automatically.",
                      ),
                    );
                  } else {
                    for (const pr of list) {
                      const diffHost = el("div", {});
                      body.append(
                        el(
                          "div",
                          { class: "cordis-bundle-card" },
                          el(
                            "div",
                            { class: "cordis-bundle-title-row" },
                            el("strong", {}, `#${pr.number} ${pr.title}`),
                            el("span", { class: "cordis-badge official" }, pr.state),
                            el(
                              "span",
                              { class: "cordis-badge" },
                              `Checks: ✓${pr.checksPassed} ✗${pr.checksFailed} ⏳${pr.checksPending}`,
                            ),
                          ),
                          el("p", { class: "hint" }, `${pr.headRef} → ${pr.baseRef} · Mergeable: ${pr.mergeable} · ${pr.url}`),
                          el(
                            "div",
                            { class: "cordis-row-actions" },
                            button("View Diff", async () => {
                              const d = await rpc("Forge", "prDiff", [s.cwd || app.currentFolder(), pr.number]);
                              diffHost.replaceChildren(
                                el("pre", { class: "code-preview" }, d?.diff || d?.error || "No diff"),
                              );
                            }, "quiet"),
                            pr.state === "OPEN"
                              ? button("Squash & Merge", async () => {
                                  const m = await rpc("Forge", "mergePr", [
                                    s.cwd || app.currentFolder(),
                                    pr.number,
                                    "squash",
                                    null,
                                  ]);
                                  if (m?.error) toast(m.error, true);
                                  else toast(`Merged PR #${pr.number}!`);
                                }, "primary")
                              : null,
                          ),
                          diffHost,
                        ),
                      );
                    }
                  }
                  const d = dialog("GitHub Forge Pull Requests", [
                    body,
                    el("div", { class: "dialog-actions" }, button("Close", () => d.close(), "primary")),
                  ], { wide: true });
                },
              },
              label,
            );
            nodes.push(prBtn);
          }

          // Right-sidebar personalized tabs registered by Creator/user plugins:
          const rightTabs = cordis.slots.entries("sidebar.right.tab");
          for (const tab of rightTabs) {
            nodes.push(
              button(
                tab.label || tab.key || "Plugin Panel",
                () => {
                  const content = tab.render(app, tab);
                  const d = dialog(tab.label || "Plugin Panel", [
                    content,
                    el("div", { class: "dialog-actions" }, button("Close", () => d.close(), "primary")),
                  ], { wide: true });
                },
                "quiet",
              ),
            );
          }

          return nodes;
        },
      });

      // Below-Composer Context Occupancy Ring + Cache-Hit % + Tok/s (`composer.footer` slot):
      ctx.slot("composer.footer", {
        order: 10,
        label: "Context Occupancy & Telemetry Bar",
        render: (app) => {
          const s = app.session();
          const { el, button } = app;
          const shortcuts = cordis._services.get("shortcuts")?.value;
          const collab = s ? app.state.collab.get(s.sessionId) : null;
          const meter = collab?.contextMeter;
          const steps = collab?.stepTelemetry || [];
          const lastStep = steps.length ? steps[steps.length - 1] : null;

          const win = meter?.contextWindow || 128000;
          const used = meter?.projectedTokens || 0;
          const pct = Math.min(100, Math.round((used / Math.max(1, win)) * 100));
          const inTok = (meter?.inputTokens || 0) + (meter?.cacheReadTokens || 0);
          const cacheHit = inTok > 0 ? Math.round(((meter?.cacheReadTokens || 0) / inTok) * 100) : 0;
          const tokSec =
            lastStep && lastStep.modelMs > 0
              ? `${(((lastStep.outputTokens || 0) * 1000) / lastStep.modelMs).toFixed(1)} tok/s`
              : null;

          const openTelemetryModal = () => {
            if (!s) return;
            const { dialog } = app;
            const d = dialog("Session Activity & Cache Telemetry", [
              el("div", { class: "sa-grid-cards" },
                el("div", { class: "sa-stat-card" }, el("small", {}, "Context Window"), el("strong", { class: "sa-stat-num" }, `${pct}%`), el("span", { class: "hint" }, `${used.toLocaleString()} / ${win.toLocaleString()} tok`)),
                el("div", { class: "sa-stat-card" }, el("small", {}, "KV Cache Hit"), el("strong", { class: "sa-stat-num" }, `${cacheHit}%`), el("span", { class: "hint" }, `${(meter?.cacheReadTokens || 0).toLocaleString()} cached tok`)),
                el("div", { class: "sa-stat-card" }, el("small", {}, "Steps Recorded"), el("strong", { class: "sa-stat-num" }, String(steps.length)), el("span", { class: "hint" }, tokSec || `${meter?.compactions || 0} compactions`)),
              ),
              el("div", { class: "dialog-actions" }, button("Close", () => d.close(), "primary")),
            ]);
          };

          return el(
            "div",
            { class: "cordis-composer-footer" },
            el(
              "div",
              { class: "cordis-row-actions" },
              s
                ? el(
                    "button",
                    {
                      type: "button",
                      class: "cordis-context-pill cordis-stat-trigger",
                      "data-composer-stat": "usage",
                      title: `Context window: ${used.toLocaleString()} / ${win.toLocaleString()} tokens (${meter?.compactions || 0} compactions)`,
                      onClick: openTelemetryModal,
                    },
                    `◔ Context ${pct}%`,
                  )
                : null,
              s && inTok > 0
                ? el(
                    "button",
                    {
                      type: "button",
                      class: "cordis-context-pill cordis-stat-trigger",
                      "data-composer-stat": "cache",
                      onClick: openTelemetryModal,
                    },
                    `⚡ Cache ${cacheHit}%`,
                  )
                : null,
              s && steps.length > 0
                ? el(
                    "button",
                    {
                      type: "button",
                      class: "cordis-context-pill cordis-stat-trigger",
                      "data-composer-stat": "activity",
                      onClick: openTelemetryModal,
                    },
                    `${steps.length} step${steps.length === 1 ? "" : "s"}${tokSec ? " · " + tokSec : ""}`,
                  )
                : null,
            ),
            shortcuts
              ? button(`Shortcuts (${modLabel()}/)`, () => shortcuts.openShortcutsModal(), "quiet")
              : null,
          );
        },
      });

      // Session Menu Extra Actions: Branch Conversation (`Title (1)`) & Download `.zip` Session Log
      ctx.slot("session.menu.actions", {
        order: 10,
        label: "Branch & Zip Export",
        render: ({ app, sessionRecord, closeMenu }) => {
          const { button } = app;
          return [
            button("Branch into a new conversation", async () => {
              closeMenu();
              await app.forkConversationBranch?.(sessionRecord);
            }),
            button("Download session log (.zip)", async () => {
              closeMenu();
              await app.downloadSessionZip?.(sessionRecord);
            }),
          ];
        },
      });
    },
  });

  // =========================================================================
  // 7. `ui-side-chat-and-styles` — Side Chat (/btw) Drawer & Output Styles
  // =========================================================================
  cordis.definePlugin({
    id: "ui-side-chat-and-styles",
    title: "Side Chat Quick Aside (/btw) & Response Output Styles",
    provide: ["sideChat"],
    setup(ctx) {
      const BUILTIN_STYLES = [
        { id: "normal", label: "Normal", desc: "Balanced, clear technical responses tailored to the task." },
        { id: "concise", label: "Concise", desc: "Short, direct answers that lead with the conclusion or code." },
        { id: "explanatory", label: "Explanatory", desc: "Walks through mechanisms, trade-offs, and design rationale." },
        { id: "formal", label: "Formal", desc: "Polished prose suited for specs, RFCs, and executive summaries." },
        { id: "learning", label: "Learning", desc: "Collaborative teaching style highlighting mental models and why." },
      ];

      let customStyles = safeStorageGet("quixi.customStyles.v1", []);
      const asideSessions = new Map(); // parentKey -> asideSessionId

      function allStyles() {
        return [...BUILTIN_STYLES, ...(Array.isArray(customStyles) ? customStyles : [])];
      }

      function currentStyleId(app) {
        const s = app.session();
        if (s) return app.state.collab.get(s.sessionId)?.outputStyle || "normal";
        return app.state.draftOutputStyle || "normal";
      }

      async function applyStyle(app, styleObj) {
        const s = app.session();
        const customInstr = styleObj.instructions || null;
        if (s) {
          const updated = await app.rpc(app.service(s), "setOutputStyle", [
            s.sessionId,
            styleObj.id,
            customInstr,
          ]);
          if (updated && !updated.error) app.state.collab.set(s.sessionId, updated);
          app.renderPage();
          app.toast(`Response style: ${styleObj.label}`);
        } else {
          app.state.draftOutputStyle = styleObj.id;
          app.state.draftOutputStyleInstructions = customInstr;
          app.renderPage();
          app.toast(`Staged "${styleObj.label}" style for new task.`);
        }
      }

      function openStylePicker() {
        const app = ctx.app();
        if (!app) return;
        const { el, button, dialog, formDialog, toast } = app;
        const activeId = currentStyleId(app);
        const listHost = el("div", { class: "cordis-bundle-grid" });
        let modal;

        const draw = () => {
          const curId = currentStyleId(app);
          listHost.replaceChildren(
            ...allStyles().map((st) => {
              const isSelected = curId === st.id;
              const isCustom = Boolean(st.custom);
              return el(
                "div",
                { class: "cordis-bundle-card style-option-card" + (isSelected ? " is-selected" : "") },
                el(
                  "div",
                  { class: "cordis-bundle-title-row" },
                  el("strong", {}, st.label),
                  el("span", { class: "cordis-badge" + (isSelected ? " official" : "") }, isSelected ? "Active" : isCustom ? "Custom" : "Built-in"),
                ),
                el("p", { class: "hint" }, st.desc || st.instructions || ""),
                el(
                  "div",
                  { class: "cordis-row-actions" },
                  button(isSelected ? "Selected" : `Use ${st.label}`, async () => {
                    await applyStyle(app, st);
                    modal.close();
                  }, isSelected ? "quiet active" : "primary"),
                  isCustom
                    ? button("Delete", () => {
                        customStyles = customStyles.filter((c) => c.id !== st.id);
                        safeStorageSet("quixi.customStyles.v1", customStyles);
                        draw();
                      }, "quiet danger")
                    : null,
                ),
              );
            }),
          );
        };

        const addCustomBtn = button("+ Create custom style", () => {
          modal.close();
          formDialog(
            "Create custom response style",
            [
              { name: "label", label: "Style name", placeholder: "e.g. Staff Reviewer", required: true },
              {
                name: "instructions",
                label: "Style instructions",
                multiline: true,
                placeholder: "Describe tone, structure, verbosity, or formatting preferences…",
                required: true,
              },
            ],
            async (vals) => {
              const label = String(vals.label || "").trim();
              const instructions = String(vals.instructions || "").trim();
              if (!label || !instructions) throw new Error("Provide both a name and instructions.");
              const id = "custom-" + label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 32);
              const entry = { id, label, desc: instructions.slice(0, 120), instructions, custom: true };
              customStyles = [...customStyles.filter((c) => c.id !== id), entry];
              safeStorageSet("quixi.customStyles.v1", customStyles);
              await applyStyle(app, entry);
            },
            { wide: true, submit: "Save & use style" },
          );
        }, "quiet");

        draw();
        modal = dialog(
          "Response Output Styles",
          [
            el("p", { class: "hint" }, "Shape how QuixiDesk writes and structures responses without altering your prompt text."),
            listHost,
            el("div", { class: "dialog-actions" }, addCustomBtn, button("Close", () => modal.close(), "primary")),
          ],
          { wide: true },
        );
        void activeId;
      }

      async function openSideChat(seedPrompt = "") {
        const app = ctx.app();
        if (!app) return;
        const { el, button, ib, state, rpc, toast, plainBody, navigate } = app;
        const host = document.getElementById("page-host");
        if (!host) return;
        const existing = document.getElementById("dsh-side-chat");
        if (existing && !seedPrompt) {
          existing.remove();
          return;
        }
        existing?.remove();
        document.getElementById("context-drawer")?.remove();

        const parentSession = app.session();
        const parentKey = parentSession?.sessionId || "draft";
        let asideId = asideSessions.get(parentKey) || null;
        let pollTimer = null;

        const logHost = el("div", { class: "dsh-side-chat-log", role: "log", "aria-live": "polite" });
        const input = el("textarea", {
          class: "dsh-side-chat-input",
          rows: 2,
          placeholder: "Ask a quick side question (/btw)…",
          "aria-label": "Side chat question",
          value: seedPrompt || "",
        });
        const sendBtn = button("Ask", () => void submitAside(), "primary", "arrow");
        const promoteBtn = button("Open as full task", async () => {
          if (!asideId) return toast("Ask a question first to promote this side chat.");
          await rpc("LocalAgentModeSessions", "updateSession", [asideId, { isArchived: false }]);
          await app.loadSessions();
          closeAside();
          navigate("chat", asideId);
        }, "quiet");

        function closeAside() {
          clearTimeout(pollTimer);
          drawer.remove();
        }

        function extractText(msg) {
          const blocks = msg?.message?.content;
          if (!Array.isArray(blocks)) return "";
          return blocks
            .filter((b) => b?.type === "text" && !b.attachment)
            .map((b) => b.text || "")
            .join("\n")
            .trim();
        }

        async function renderAsideLog() {
          if (!asideId) {
            logHost.replaceChildren(
              el(
                "p",
                { class: "hint" },
                "Ask a quick side question without adding messages or token pressure to your main conversation.",
              ),
            );
            return;
          }
          const sess = await rpc("LocalAgentModeSessions", "getSession", [asideId]);
          const msgs = sess?.bufferedMessages || [];
          const stream = state.streams?.get(asideId);
          const nodes = [];
          for (const m of msgs) {
            if (m.type !== "user" && m.type !== "assistant") continue;
            const txt = extractText(m);
            if (!txt) continue;
            const insertBtn =
              m.type === "assistant"
                ? button("Insert into prompt", () => {
                    const mainTa = document.getElementById("prompt");
                    if (mainTa) {
                      mainTa.value = mainTa.value.trim() ? `${mainTa.value.trim()}\n\n${txt}` : txt;
                      app.saveDraft();
                      mainTa.focus();
                      toast("Inserted side-chat answer into main prompt.");
                    }
                  }, "quiet")
                : null;
            nodes.push(
              el(
                "div",
                { class: "dsh-side-msg is-" + m.type },
                el("small", { class: "dsh-side-role" }, m.type === "user" ? "You" : "Side Chat"),
                m.type === "assistant" ? plainBody(txt) : el("p", {}, txt),
                insertBtn ? el("div", { class: "cordis-row-actions" }, insertBtn) : null,
              ),
            );
          }
          if (sess?.isRunning) {
            nodes.push(
              el(
                "div",
                { class: "dsh-side-msg is-assistant is-streaming" },
                el("small", { class: "dsh-side-role" }, "Side Chat · thinking…"),
                stream?.text ? plainBody(stream.text) : el("p", { class: "hint" }, "Generating answer…"),
              ),
            );
            clearTimeout(pollTimer);
            pollTimer = setTimeout(() => {
              if (drawer.isConnected) void renderAsideLog();
            }, 320);
          }
          if (!nodes.length) {
            logHost.replaceChildren(el("p", { class: "hint" }, "Ready for your side question."));
          } else {
            logHost.replaceChildren(...nodes);
            logHost.scrollTop = logHost.scrollHeight;
          }
        }

        async function submitAside() {
          const q = input.value.trim();
          if (!q) return;
          if (!state.providers?.hasConfiguredProvider || !state.providers?.activeModel) {
            toast("Connect a provider and choose a model before asking.", true);
            return;
          }
          sendBtn.disabled = true;
          try {
            if (!asideId) {
              const started = await rpc("LocalAgentModeSessions", "start", [
                {
                  title: `Aside · ${q.slice(0, 36)}`,
                  isArchived: true,
                  permissionMode: "default",
                  cwd: app.currentFolder() || "",
                  userSelectedFolders: app.currentFolder() ? [app.currentFolder()] : [],
                  agentPreset: "minimal",
                },
              ]);
              if (!started?.sessionId) throw new Error(started?.error || "Could not start side chat.");
              asideId = started.sessionId;
              asideSessions.set(parentKey, asideId);
            }
            input.value = "";
            const res = await rpc("LocalAgentModeSessions", "sendMessageV2", [
              asideId,
              { text: q, attachments: [], messageUuid: crypto.randomUUID(), delivery: "queue" },
            ]);
            if (res?.error) throw new Error(res.error);
            await renderAsideLog();
            clearTimeout(pollTimer);
            pollTimer = setTimeout(() => {
              if (drawer.isConnected) void renderAsideLog();
            }, 250);
          } catch (err) {
            toast(err.message, true);
          } finally {
            sendBtn.disabled = false;
          }
        }

        input.addEventListener("keydown", (e) => {
          if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
            e.preventDefault();
            void submitAside();
          } else if (e.key === "Escape") {
            e.preventDefault();
            closeAside();
          }
        });

        const drawer = el(
          "aside",
          { id: "dsh-side-chat", class: "context-drawer dsh-side-chat", "aria-label": "Side chat quick aside" },
          el(
            "div",
            { class: "drawer-head" },
            el("strong", {}, "Side chat · /btw"),
            promoteBtn,
            ib("Close side chat", "close", closeAside),
          ),
          logHost,
          el("div", { class: "dsh-side-chat-composer" }, input, el("div", { class: "cordis-row-actions" }, sendBtn)),
        );
        host.append(drawer);
        await renderAsideLog();
        if (seedPrompt.trim()) {
          await submitAside();
        } else {
          input.focus();
        }
      }

      ctx.effect("side-chat-cleanup", () => () => {
        document.getElementById("dsh-side-chat")?.remove();
      });

      ctx.provide("sideChat", { openSideChat, openStylePicker });

      ctx.slot("session.heading.extras", {
        order: 15,
        label: "Side Chat Quick Aside",
        render: (app) => {
          if (!app.session()) return null;
          const b = app.button("Side chat", () => void openSideChat(""), "quiet", "chat");
          b.title = "Open Side Chat quick aside (/btw)";
          return b;
        },
      });

      ctx.slot("composer.footer", {
        order: 5,
        label: "Response Output Style Trigger",
        render: (app) => {
          const curId = currentStyleId(app);
          const found = allStyles().find((st) => st.id === curId);
          const label = found ? found.label : curId;
          const btn = app.el(
            "button",
            {
              type: "button",
              class: "cordis-context-pill cordis-stat-trigger dsh-style-pill" + (curId !== "normal" ? " is-custom-style" : ""),
              "aria-label": "Response style",
              title: "Choose response output style (/style)",
              onClick: openStylePicker,
            },
            `✦ Style: ${label}`,
          );
          return btn;
        },
      });
    },
  });
})();
