/* Copyright (c) 2026 QuixiAI. MIT license. Original QuixiDesk workspace. */
(() => {
  "use strict";
  const { api, rpc, copy } = window.quixi;
  const root = document.getElementById("app");
  const state = {
    page: "new",
    id: null,
    sessions: [],
    projects: [],
    artifacts: [],
    schedules: [],
    providers: null,
    draft: "",
    attachments: [],
    folder: "",
    project: null,
    permission: "default",
    browser: false,
    browserState: null,
    streams: new Map(),
    transcripts: new Map(),
    drafts: new Map(),
    attachmentDrafts: new Map(),
    collapsed: localStorage.getItem("quixi.sidebar") === "closed",
    settingsTab: "general",
  };
  const icons = {
    plus: "M12 5v14M5 12h14",
    search: "M21 21l-5-5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
    projects: "M3 6h7l2 3h9v11H3zM3 6V4h7l2 2",
    artifacts: "M4 3h12l4 4v14H4zM8 12h8M8 16h5",
    schedule: "M12 7v5l4 2M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0",
    tools: "M5 4l15 15M16 4l4 4L8 20H4v-4z",
    settings: "M4 7h16M4 17h16M8 4v6M16 14v6",
    sidebar: "M3 4h18v16H3zM8 4v16",
    arrow: "M5 12h14M13 6l6 6-6 6",
    stop: "M6 6h12v12H6z",
    browser: "M3 4h18v16H3zM3 9h18M7 6h.01M10 6h.01",
    folder: "M3 6h7l2 3h9v11H3z",
    close: "M6 6l12 12M18 6L6 18",
    file: "M5 3h9l5 5v13H5zM14 3v6h5",
    more: "M5 12h.01M12 12h.01M19 12h.01",
    check: "M4 12l5 5L20 6",
    back: "M15 5l-7 7 7 7",
    star: "M12 3l3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1z",
    code: "M8 6l-6 6 6 6M16 6l6 6-6 6",
    refresh: "M20 8a9 9 0 1 0 1 7M20 3v6h-6",
    spark: "M12 3v18M3 12h18M6 6l12 12M18 6L6 18",
  };
  function icon(name, size = 18) {
    const node = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    for (const [k, v] of Object.entries({
      width: size,
      height: size,
      viewBox: "0 0 24 24",
      fill: "none",
      stroke: "currentColor",
      "stroke-width": "1.6",
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
      "aria-hidden": "true",
    }))
      node.setAttribute(k, v);
    const path = document.createElementNS(node.namespaceURI, "path");
    path.setAttribute("d", icons[name] || icons.file);
    node.append(path);
    return node;
  }
  function el(tag, attrs = {}, ...children) {
    const n = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
      if (v === undefined || v === null || v === false) continue;
      if (k === "class") n.className = v;
      else if (k.startsWith("on"))
        n.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === "value") n.value = v;
      else if (k === "checked" || k === "disabled" || k === "selected")
        n[k] = !!v;
      else n.setAttribute(k, v === true ? "" : v);
    }
    for (const child of children.flat(Infinity))
      if (child !== null && child !== undefined && child !== false)
        n.append(
          child instanceof Node
            ? child
            : document.createTextNode(String(child)),
        );
    return n;
  }
  function button(label, fn, cls = "", name) {
    return el(
      "button",
      {
        type: "button",
        class: cls,
        onClick: async (event) => {
          try {
            await fn(event);
          } catch (e) {
            toast(e.message, true);
          }
        },
      },
      name ? icon(name) : null,
      label,
    );
  }
  const ib = (label, name, fn) => {
    const b = button("", fn, "icon quiet", name);
    b.setAttribute("aria-label", label);
    b.title = label;
    return b;
  };
  function toast(text, error = false) {
    const n = el("div", { class: "toast" + (error ? " error" : "") }, text);
    document.getElementById("toasts").append(n);
    setTimeout(() => n.remove(), 6500);
  }
  function select(options, value, change, attrs = {}) {
    const n = el("select", {
      ...attrs,
      onChange: (e) => change?.(e.target.value),
    });
    for (const [v, label] of options)
      n.append(
        el(
          "option",
          { value: v, selected: String(v) === String(value) },
          label,
        ),
      );
    return n;
  }
  function field(label, control, hint) {
    const id = "field-" + crypto.randomUUID();
    control.id = id;
    return el(
      "div",
      {},
      el("label", { for: id }, label),
      control,
      hint ? el("div", { class: "hint" }, hint) : null,
    );
  }
  function empty(title, description, action) {
    return el(
      "div",
      { class: "empty" },
      icon("spark", 28),
      el("h2", {}, title),
      el("p", {}, description),
      action,
    );
  }
  function date(ms) {
    return ms
      ? new Date(ms).toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
        })
      : "";
  }
  function project() {
    return state.projects.find((p) => p.id === state.project);
  }
  function service(s) {
    return s?._kind === "code" ? "LocalSessions" : "LocalAgentModeSessions";
  }
  function session() {
    return state.sessions.find((s) => s.sessionId === state.id);
  }
  function running(s) {
    return s?._kind === "code" ? !!s.turnRunning : !!s?.isRunning;
  }
  function draftKey() {
    return state.id || "new";
  }
  function saveDraft() {
    state.attachmentDrafts.set(draftKey(), state.attachments.slice());
    const value = document.getElementById("prompt")?.value;
    if (value !== undefined) {
      state.drafts.set(draftKey(), value);
      sessionStorage.setItem("quixi.draft." + draftKey(), value);
    }
  }
  function getDraft() {
    return (
      state.drafts.get(draftKey()) ??
      sessionStorage.getItem("quixi.draft." + draftKey()) ??
      ""
    );
  }
  function currentFolder() {
    return (
      state.folder || project()?.folders?.[0]?.path || session()?.cwd || ""
    );
  }
  function dialog(title, body, { wide = false } = {}) {
    const d = el("dialog", { class: wide ? "wide" : "" });
    d.append(
      el(
        "div",
        { class: "dialog-head" },
        el("h2", {}, title),
        ib("Close dialog", "close", () => d.close()),
      ),
      el("div", { class: "dialog-body" }, body),
    );
    d.addEventListener("close", () => d.remove());
    d.addEventListener("click", (e) => {
      if (e.target === d) {
        const r = d.getBoundingClientRect();
        if (
          e.clientX < r.left ||
          e.clientX > r.right ||
          e.clientY < r.top ||
          e.clientY > r.bottom
        )
          d.close();
      }
    });
    document.body.append(d);
    d.showModal();
    return d;
  }
  function confirmDialog(title, text, action, label = "Delete") {
    const d = dialog(title, [
      el("p", {}, text),
      el(
        "div",
        { class: "form-actions" },
        button("Cancel", () => d.close(), "quiet"),
        button(
          label,
          async () => {
            await action();
            d.close();
          },
          "danger",
        ),
      ),
    ]);
  }
  function formDialog(
    title,
    fields,
    save,
    { wide = false, submit = "Save" } = {},
  ) {
    const controls = {};
    const body = el("form", {});
    const error = el("div", { class: "form-error", role: "alert" });
    for (const f of fields) {
      let c;
      if (f.options) c = select(f.options, f.value, null, { name: f.name });
      else
        c = el(f.multiline ? "textarea" : "input", {
          name: f.name,
          type: f.type || "text",
          value: f.value || "",
          required: f.required,
          placeholder: f.placeholder || "",
          ...(f.attrs || {}),
        });
      controls[f.name] = c;
      body.append(field(f.label, c, f.hint));
    }
    const saveButton = el(
      "button",
      { type: "submit", class: "primary" },
      submit,
    );
    body.append(
      error,
      el(
        "div",
        { class: "form-actions" },
        button("Cancel", () => d.close(), "quiet"),
        saveButton,
      ),
    );
    body.addEventListener("submit", async (e) => {
      e.preventDefault();
      error.textContent = "";
      saveButton.disabled = true;
      try {
        await save(
          Object.fromEntries(
            Object.entries(controls).map(([k, v]) => [k, v.value]),
          ),
        );
        d.close();
      } catch (err) {
        error.textContent = err.message;
      } finally {
        saveButton.disabled = false;
      }
    });
    const d = dialog(title, body, { wide });
    return d;
  }
  async function loadSessions() {
    const [a, b] = await Promise.all([
      rpc("LocalAgentModeSessions", "getAll"),
      rpc("LocalSessions", "getAll"),
    ]);
    state.sessions = [
      ...(a || []).map((s) => ({ ...s, _kind: "task" })),
      ...(b || []).map((s) => ({ ...s, _kind: "code" })),
    ].sort((a, b) => b.lastActivityAt - a.lastActivityAt);
    renderRecents();
  }
  async function loadProjects() {
    state.projects = (await rpc("CoworkSpaces", "getAllSpaces")) || [];
  }
  async function loadProviders() {
    state.providers = await api("/api/quixi/providers");
  }
  function navigate(page, id = null, { replace = false } = {}) {
    saveDraft();
    page = String(page).replace(/^\//, "");
    if (page.startsWith("chat/")) {
      id = page.slice(5);
      page = "chat";
    }
    if (page.startsWith("cowork/")) {
      id = page.slice(7);
      page = "chat";
    }
    if (page.startsWith("settings/")) page = "settings";
    const pages = [
      "new",
      "chat",
      "search",
      "projects",
      "artifacts",
      "schedules",
      "tools",
      "settings",
    ];
    if (!pages.includes(page)) page = "new";
    state.page = page;
    state.id = id;
    state.attachments = state.attachmentDrafts.get(draftKey())?.slice() || [];
    const path =
      page === "chat" ? "/chat/" + encodeURIComponent(id) : "/" + page;
    if (location.pathname !== path)
      history[replace ? "replaceState" : "pushState"]({}, "", path);
    render();
    if (page === "chat") openTranscript().catch((e) => toast(e.message, true));
    else if (page === "projects")
      loadProjects()
        .then(() => renderPage())
        .catch((e) => toast(e.message, true));
    else if (page === "artifacts") loadArtifacts();
    else if (page === "schedules") loadSchedules();
    else if (page === "tools") loadTools();
    else if (page === "search")
      setTimeout(() => document.getElementById("search-input")?.focus(), 0);
  }
  window.quixiNavigate = (page) => navigate(page);
  addEventListener("popstate", () => {
    const path = location.pathname.slice(1);
    navigate(path, null, { replace: true });
  });
  function render() {
    document.title =
      state.page === "chat"
        ? (session()?.title || "Task") + " · QuixiDesk"
        : "QuixiDesk";
    const nav = [
      ["search", "Search", "search"],
      ["projects", "Projects", "projects"],
      ["artifacts", "Artifacts", "artifacts"],
      ["schedules", "Schedules", "schedule"],
      ["tools", "Tools & skills", "tools"],
    ];
    const sidebar = el(
      "aside",
      { class: "sidebar" },
      el(
        "div",
        { class: "brand" },
        el("img", { src: "/quixi-mark.svg", alt: "" }),
        el(
          "span",
          {},
          "QuixiDesk",
          el("span", { class: "brand-caption" }, "WORKSPACE"),
        ),
      ),
      el(
        "nav",
        { "aria-label": "Main navigation" },
        button(
          [icon("plus"), el("span", { class: "nav-label" }, "New task")],
          () => {
            state.project = null;
            state.folder = "";
            navigate("new");
          },
          "new-task",
        ),
        ...nav.map(([page, label, ic]) =>
          button(
            [icon(ic), el("span", { class: "nav-label" }, label)],
            () => navigate(page),
            state.page === page ? "active" : "",
          ),
        ),
      ),
      el("div", { class: "recent", id: "recents" }),
      el(
        "footer",
        {},
        button(
          [icon("settings"), el("span", { class: "nav-label" }, "Settings")],
          () => navigate("settings"),
          "quiet",
        ),
        ib("Toggle sidebar", "sidebar", () => {
          state.collapsed = !state.collapsed;
          localStorage.setItem(
            "quixi.sidebar",
            state.collapsed ? "closed" : "open",
          );
          render();
        }),
      ),
    );
    const crumb =
      state.page === "chat"
        ? session()?.title || "Task"
        : {
            new: "Your workspace",
            search: "Search",
            projects: "Projects",
            artifacts: "Private artifacts",
            schedules: "Schedules",
            tools: "Tools & skills",
            settings: "Settings",
          }[state.page];
    const top = el(
      "header",
      { class: "topbar" },
      ib("Back", "back", () => history.back()),
      el("div", { class: "crumb" }, "Workspace / ", el("strong", {}, crumb)),
      button(
        `Providers${state.providers?.configuredCount ? " · " + state.providers.configuredCount : ""}`,
        () => providerDirectory(),
        "quiet",
      ),
      ib("Toggle QuixiDesk Browser", "browser", () => toggleBrowser()),
    );
    root.replaceChildren(
      el(
        "div",
        { class: "shell" + (state.collapsed ? " collapsed" : "") },
        sidebar,
        el(
          "main",
          { class: "main" },
          top,
          el("div", {
            id: "page-host",
            class:
              state.page === "new" || state.page === "chat"
                ? "chat-layout"
                : "page",
          }),
        ),
      ),
    );
    root.removeAttribute("aria-busy");
    renderRecents();
    renderPage();
  }
  function renderRecents() {
    const target = document.getElementById("recents");
    if (!target) return;
    const visible = state.sessions.filter((s) => !s.isArchived);
    const pinned = visible.filter((s) => s.isStarred);
    const rest = visible.filter((s) => !s.isStarred).slice(0, 35);
    target.replaceChildren();
    for (const [label, list] of [
      ["Pinned", pinned],
      ["Recent", rest],
    ])
      if (list.length) {
        target.append(el("div", { class: "section-label" }, label));
        for (const s of list)
          target.append(
            el(
              "div",
              {
                class:
                  "recent-row" + (state.id === s.sessionId ? " current" : ""),
              },
              button(
                [
                  running(s) ? el("span", { class: "running-dot" }) : null,
                  s.title || "Untitled task",
                ],
                () => {
                  state.project = s.spaceId || null;
                  state.folder = s.cwd || "";
                  navigate("chat", s.sessionId);
                },
                "session-link",
              ),
              button("···", () => sessionMenu(s), "more"),
            ),
          );
      }
    if (!visible.length)
      target.append(
        el(
          "p",
          { class: "hint", style: "padding:0 10px" },
          "Your conversations will appear here.",
        ),
      );
  }
  function renderPage() {
    const host = document.getElementById("page-host");
    if (!host) return;
    const handlers = {
      new: chatPage,
      chat: chatPage,
      search: searchPage,
      projects: projectsPage,
      artifacts: artifactsPage,
      schedules: schedulesPage,
      tools: toolsPage,
      settings: settingsPage,
    };
    handlers[state.page]?.(host);
    if (state.browser) renderBrowser(host);
  }
  function chatPage(host) {
    const s = session();
    const main = el("section", { class: "chat-main" });
    if (s)
      main.append(
        el(
          "div",
          { class: "session-heading" },
          el("h2", {}, s.title || "Untitled task"),
          button("Files & context", () => contextDrawer()),
          ib("Task actions", "more", () => sessionMenu(s)),
        ),
      );
    const conversation = el("div", {
      class: "conversation",
      id: "conversation",
      "aria-label": "Conversation",
      "aria-live": "off",
    });
    if (!s)
      conversation.append(
        el(
          "div",
          { class: "welcome" },
          el(
            "div",
            { class: "eyebrow" },
            project() ? project().name : "A LITTLE SPACE TO THINK BIG",
          ),
          el("h1", {}, "Bring your next idea to life."),
          el(
            "p",
            {},
            "Think through a question, build something useful, or work with the files in your workspace.",
          ),
          el(
            "div",
            { class: "suggestions" },
            ...[
              [
                "Plan my next project",
                "Help me make a practical plan for my next project. Start by asking about the outcome and constraints.",
                "projects",
              ],
              [
                "Understand a codebase",
                "Help me understand this workspace. Start with a read-only overview of the project structure.",
                "code",
              ],
              [
                "Make a useful artifact",
                "Help me create a private local HTML artifact. Ask what it should do before building it.",
                "artifacts",
              ],
            ].map(([label, prompt, ic]) =>
              button([icon(ic), label], () => {
                document.getElementById("prompt").value = prompt;
                saveDraft();
                document.getElementById("prompt").focus();
              }),
            ),
          ),
        ),
      );
    main.append(conversation, composer());
    host.replaceChildren(main);
    renderChips();
    if (s) renderTranscript();
  }
  function composer() {
    const s = session();
    const ta = el("textarea", {
      id: "prompt",
      placeholder: running(s)
        ? "Write a follow-up while this task runs…"
        : "Write your prompt to QuixiDesk",
      "aria-label": "Write your prompt to QuixiDesk",
      rows: 3,
      value: getDraft(),
      onInput: () => saveDraft(),
      onKeydown: (e) => {
        if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
          e.preventDefault();
          if (!running(session())) sendPrompt();
        }
      },
    });
    const fileInput = el("input", {
      type: "file",
      multiple: true,
      id: "attachment-input",
      class: "hidden",
      accept:
        ".txt,.md,.json,.csv,.log,.html,.js,.ts,.tsx,.jsx,.rs,.py,.css,.toml,.yaml,.yml,.xml,.sql,.sh",
      onChange: async (e) => {
        for (const file of e.target.files) {
          if (file.size > 1_000_000) {
            toast(file.name + " exceeds the 1 MB attachment limit.", true);
            continue;
          }
          const text = await file.text();
          if (text.includes("\u0000")) {
            toast(file.name + " is not a supported text attachment.", true);
            continue;
          }
          state.attachments.push({ name: file.name, text });
        }
        renderChips();
        e.target.value = "";
      },
    });
    const mode = select(
      [
        ["default", "Manual"],
        ["acceptEdits", "Allow file edits"],
        ["auto", "Auto"],
      ],
      s?.permissionMode || state.permission,
      async (v) => {
        state.permission = v;
        if (s) await rpc(service(s), "setPermissionMode", [s.sessionId, v]);
      },
      { "aria-label": "Tool permission mode" },
    );
    const send = button(
      running(s) ? "Stop" : "Send",
      () => (running(session()) ? stopTask() : sendPrompt()),
      running(s) ? "danger" : "primary",
      running(s) ? "stop" : "arrow",
    );
    send.id = "send-button";
    return el(
      "div",
      { class: "composer-wrap" },
      el(
        "div",
        { class: "composer" },
        el("div", { class: "chips", id: "attachment-chips" }),
        ta,
        fileInput,
        el(
          "div",
          { class: "composer-tools" },
          ib("Attach text files", "plus", () => fileInput.click()),
          button(
            [
              icon("folder", 14),
              currentFolder()
                ? currentFolder().split(/[\\/]/).filter(Boolean).pop()
                : "Workspace",
            ],
            () => folderDialog(),
            "workspace-switch",
          ),
          mode,
          el("span", { class: "spacer" }),
          button("Model", () => modelPicker(), "quiet"),
          send,
        ),
      ),
      el(
        "p",
        { class: "composer-note" },
        "Enter to send · Shift+Enter for a new line · Review important results",
      ),
    );
  }
  function renderChips() {
    const target = document.getElementById("attachment-chips");
    target?.replaceChildren(
      ...state.attachments.map((f, i) =>
        el(
          "span",
          { class: "chip" },
          icon("file", 12),
          f.name,
          button(
            "×",
            () => {
              state.attachments.splice(i, 1);
              renderChips();
            },
            "quiet",
          ),
        ),
      ),
    );
  }
  async function folderDialog() {
    const current = currentFolder();
    const input = el("input", {
      value: current,
      placeholder: "Absolute workspace folder path",
      "aria-label": "Workspace folder path",
    });
    const error = el("div", { class: "form-error", role: "alert" });
    const browse = button("Browse folders", async () => {
      const folder = await rpc(
        "LocalAgentModeSessions",
        "browseAndAddTrustedFolder",
        ["Choose a workspace folder"],
      );
      if (folder) input.value = folder;
      else toast("No folder selected. You can also enter an absolute path.");
    });
    const d = dialog("Choose workspace", [
      el(
        "p",
        {},
        "Tasks can inspect this folder. Manual mode blocks shell commands and changes until you choose a broader permission mode.",
      ),
      field("Folder path", input),
      browse,
      error,
      el(
        "div",
        { class: "form-actions" },
        button("Cancel", () => d.close(), "quiet"),
        button(
          "Use folder",
          async () => {
            try {
              const path = input.value.trim();
              const checked = await api("/api/quixi/workspace/inspect", {
                path,
              });
              if (!checked.exists)
                throw new Error("Enter an existing absolute directory.");
              state.folder = checked.path;
              state.project =
                state.projects.find((p) =>
                  p.folders.some((f) => f.path === state.folder),
                )?.id || null;
              const s = session();
              if (s) {
                await rpc(service(s), "updateSession", [
                  s.sessionId,
                  {
                    userSelectedFolders: [state.folder],
                    spaceId: state.project,
                  },
                ]);
              }
              d.close();
              renderPage();
            } catch (e) {
              error.textContent = e.message;
            }
          },
          "primary",
        ),
      ),
    ]);
  }
  async function sendPrompt() {
    const input = document.getElementById("prompt");
    if (!input) return;
    const text = input.value.trim();
    if (!text) return;
    if (!state.providers?.hasConfiguredProvider) {
      providerDirectory();
      toast("Connect a provider before sending a task.");
      return;
    }
    if (running(session())) return;
    const b = document.getElementById("send-button");
    if (b) b.disabled = true;
    try {
      let s = session();
      let message = text;
      if (state.attachments.length)
        message +=
          "\n\nAttached text files:\n" +
          state.attachments
            .map((f) => `\n--- ${f.name} ---\n${f.text}`)
            .join("\n");
      if (!s) {
        const opts = {
          title: Array.from(text).slice(0, 80).join(""),
          permissionMode: state.permission,
          spaceId: state.project,
          userSelectedFolders: currentFolder() ? [currentFolder()] : undefined,
          cwd: currentFolder() || undefined,
        };
        s = await rpc("LocalAgentModeSessions", "start", [opts]);
        s._kind = "task";
        state.sessions.unshift(s);
        state.id = s.sessionId;
        state.page = "chat";
        history.pushState({}, "", "/chat/" + encodeURIComponent(state.id));
      }
      const uuid = crypto.randomUUID();
      await rpc(service(s), "sendMessage", [
        s.sessionId,
        message,
        null,
        null,
        uuid,
      ]);
      state.drafts.delete(draftKey());
      sessionStorage.removeItem("quixi.draft." + draftKey());
      sessionStorage.removeItem("quixi.draft.new");
      state.drafts.delete("new");
      state.attachments = [];
      state.attachmentDrafts.delete("new");
      state.attachmentDrafts.delete(draftKey());
      s.isRunning = true;
      state.streams.set(s.sessionId, { text: "", thinking: "" });
      await refreshSession(s.sessionId);
      render();
      document.getElementById("prompt")?.focus();
    } catch (e) {
      toast(e.message, true);
    } finally {
      if (b) b.disabled = false;
    }
  }
  async function stopTask() {
    const s = session();
    if (!s) return;
    await rpc(service(s), "stop", [s.sessionId]);
    state.streams.delete(s.sessionId);
    await refreshSession(s.sessionId);
    renderTranscript();
    updateSendButton();
  }
  function updateSendButton() {
    const b = document.getElementById("send-button");
    if (!b) return;
    const active = running(session());
    b.className = active ? "danger" : "primary";
    b.replaceChildren(
      icon(active ? "stop" : "arrow"),
      active ? "Stop" : "Send",
    );
    b.disabled = false;
    const input = document.getElementById("prompt");
    if (input)
      input.placeholder = active
        ? "Write a follow-up while this task runs…"
        : "Write your prompt to QuixiDesk";
  }
  async function refreshSession(id) {
    const old = state.sessions.find((s) => s.sessionId === id);
    if (!old) return;
    const value = await rpc(service(old), "getSession", [id]);
    if (value) {
      Object.assign(old, value);
      if (old._kind === "code")
        state.transcripts.set(
          id,
          (await rpc("LocalSessions", "getTranscript", [id])) || [],
        );
    }
    renderRecents();
  }
  async function openTranscript() {
    await refreshSession(state.id);
    renderTranscript();
    updateSendButton();
  }
  function textBlocks(message) {
    const content = message?.message?.content || message?.content || [];
    return typeof content === "string"
      ? [{ type: "text", text: content }]
      : Array.isArray(content)
        ? content
        : [];
  }
  function plainBody(text) {
    const box = el("div", { class: "body" });
    const pieces = String(text || "").split(/(```[\s\S]*?```)/g);
    for (const piece of pieces) {
      if (piece.startsWith("```")) {
        const code = piece.replace(/^```[^\n]*\n?/, "").replace(/```$/, "");
        box.append(el("pre", {}, el("code", {}, code)));
      } else {
        const re =
          /\[([^\]]+)\]\((app:\/\/localhost\/cowork-artifact\/([a-zA-Z0-9-]+))\)/g;
        let last = 0,
          m;
        while ((m = re.exec(piece))) {
          const artifactId = m[3];
          box.append(
            document.createTextNode(piece.slice(last, m.index)),
            el(
              "a",
              {
                href: "#",
                onClick: (e) => {
                  e.preventDefault();
                  openArtifact(artifactId);
                },
              },
              m[1],
            ),
          );
          last = re.lastIndex;
        }
        box.append(document.createTextNode(piece.slice(last)));
      }
    }
    return box;
  }
  function renderTranscript() {
    const target = document.getElementById("conversation");
    const s = session();
    if (!target || !s) return;
    const nearBottom =
      target.scrollHeight - target.scrollTop - target.clientHeight < 90;
    const oldScroll = target.scrollTop;
    const messages =
      s._kind === "code"
        ? state.transcripts.get(s.sessionId) || []
        : s.bufferedMessages || [];
    const seen = new Set();
    const result = [];
    for (const message of messages) {
      if (message.type === "result" || message.type === "stream_event")
        continue;
      const id = message.uuid;
      if (id && seen.has(id)) continue;
      if (id) seen.add(id);
      const blocks = textBlocks(message);
      const texts = blocks
        .filter((b) => b.type === "text")
        .map((b) => b.text || "")
        .join("\n");
      const role = message.type === "user" ? "user" : "assistant";
      const tools = blocks.filter(
        (b) => b.type === "tool_use" || b.type === "tool_result",
      );
      if (texts) {
        const article = el(
          "article",
          { class: "message " + role },
          el("div", { class: "role" }, role === "user" ? "You" : "QuixiDesk"),
          plainBody(texts),
          el(
            "div",
            { class: "message-actions" },
            button(
              "Copy",
              async () => {
                await copy(texts);
                toast("Copied message.");
              },
              "quiet",
            ),
          ),
        );
        result.push(article);
      }
      for (const tool of tools) {
        const isResult = tool.type === "tool_result";
        const failed = !!tool.is_error;
        const content = isResult
          ? typeof tool.content === "string"
            ? tool.content
            : JSON.stringify(tool.content, null, 2)
          : JSON.stringify(tool.input || {}, null, 2);
        const title = isResult
          ? failed
            ? "Tool action failed"
            : "Tool result"
          : tool.name || "Tool call";
        result.push(
          el(
            "details",
            { class: "tool-card" + (failed ? " failed" : "") },
            el("summary", {}, icon(failed ? "close" : "tools", 14), title),
            el("pre", {}, content || "(empty result)"),
          ),
        );
      }
    }
    const stream = state.streams.get(s.sessionId);
    if (stream && running(s)) {
      if (stream.thinking)
        result.push(
          el(
            "details",
            { class: "thinking" },
            el("summary", {}, "Thinking"),
            stream.thinking,
          ),
        );
      if (stream.text)
        result.push(
          el(
            "article",
            { class: "message assistant" },
            el("div", { class: "role" }, "QuixiDesk"),
            plainBody(stream.text),
          ),
        );
      result.push(
        el(
          "div",
          { class: "live-status", role: "status" },
          el("span", { class: "pulse" }),
          "Working…",
        ),
      );
    }
    if (s.error)
      result.push(
        el(
          "div",
          { class: "notice error-text", role: "alert" },
          typeof s.error === "string" ? s.error : JSON.stringify(s.error),
        ),
      );
    if (!result.length)
      result.push(
        empty(
          "Ready when you are",
          "Send a prompt to start this conversation.",
        ),
      );
    target.replaceChildren(...result);
    if (nearBottom) target.scrollTop = target.scrollHeight;
    else target.scrollTop = oldScroll;
  }
  async function sessionMenu(s) {
    const d = dialog("Task actions", [
      el("p", {}, s.title),
      el(
        "div",
        { class: "field-stack" },
        button("Rename", () => {
          d.close();
          formDialog(
            "Rename task",
            [
              {
                name: "title",
                label: "Task title",
                value: s.title,
                required: true,
              },
            ],
            async (values) => {
              await rpc(service(s), "updateSession", [
                s.sessionId,
                { title: values.title.trim() },
              ]);
              await loadSessions();
              render();
            },
          );
        }),
        button(s.isStarred ? "Unpin task" : "Pin task", async () => {
          await rpc(service(s), "updateSession", [
            s.sessionId,
            { isStarred: !s.isStarred },
          ]);
          await loadSessions();
          d.close();
        }),
        button(s.isArchived ? "Restore task" : "Archive task", async () => {
          await rpc(service(s), s.isArchived ? "unarchive" : "archive", [
            s.sessionId,
          ]);
          await loadSessions();
          d.close();
          if (state.id === s.sessionId) navigate("new");
          else renderPage();
        }),
        button(
          "Delete task",
          () => {
            d.close();
            confirmDialog(
              "Delete conversation?",
              `Permanently remove “${s.title}” and its stored recovery copy?`,
              async () => {
                await rpc(service(s), "delete", [s.sessionId]);
                await loadSessions();
                if (state.id === s.sessionId) navigate("new");
                else renderPage();
              },
            );
          },
          "danger",
        ),
      ),
    ]);
  }
  function header(title, description, action) {
    return el(
      "div",
      { class: "page-header" },
      el("div", {}, el("h1", {}, title), el("p", {}, description)),
      action,
    );
  }
  function searchPage(host) {
    const input = el("input", {
      id: "search-input",
      type: "search",
      placeholder: "Search conversation titles and contents",
      "aria-label": "Search conversations",
    });
    const results = el("div", { id: "search-results" });
    const filter = select(
      [
        ["all", "All conversations"],
        ["active", "Active"],
        ["archived", "Archived"],
        ["pinned", "Pinned"],
      ],
      "all",
      () => search(),
    );
    let timer,
      sequence = 0;
    const search = async () => {
      const q = input.value.trim(),
        token = ++sequence;
      let matches = [];
      try {
        if (q) {
          const found = await rpc("LocalAgentModeSessions", "searchSessions", [
            q,
            100,
          ]);
          matches = state.sessions
            .filter(
              (s) =>
                (found || []).some((r) => r.sessionId === s.sessionId) ||
                s.title.toLowerCase().includes(q.toLowerCase()),
            )
            .map((s) => ({
              ...s,
              snippet:
                (found || []).find((r) => r.sessionId === s.sessionId)
                  ?.snippet || "",
            }));
        } else matches = state.sessions;
        matches = matches.filter(
          (s) =>
            filter.value === "all" ||
            (filter.value === "archived" && s.isArchived) ||
            (filter.value === "active" && !s.isArchived) ||
            (filter.value === "pinned" && s.isStarred),
        );
        if (token !== sequence) return;
        results.replaceChildren(
          el(
            "p",
            { class: "result-count" },
            `${matches.length} conversation${matches.length === 1 ? "" : "s"}`,
          ),
          ...matches.map((s) =>
            el(
              "div",
              { class: "list-row search-result" },
              el(
                "div",
                { class: "grow" },
                button(
                  s.title || "Untitled",
                  () => navigate("chat", s.sessionId),
                  "subtle-link",
                ),
                s.snippet ? el("p", { class: "snippet" }, s.snippet) : null,
              ),
              el("small", {}, date(s.lastActivityAt)),
              ib("Task actions", "more", () => sessionMenu(s)),
            ),
          ),
        );
        if (!matches.length)
          results.append(
            empty("No matches", "Try a different phrase or filter."),
          );
      } catch (e) {
        if (token === sequence)
          results.replaceChildren(el("p", { class: "error-text" }, e.message));
      }
    };
    input.addEventListener("input", () => {
      clearTimeout(timer);
      timer = setTimeout(search, 160);
    });
    host.replaceChildren(
      header(
        "Find your work",
        "Search local conversation titles and message text.",
      ),
      el("div", { class: "toolbar" }, input, filter),
      results,
    );
    search();
  }
  function projectsPage(host) {
    host.replaceChildren(
      header(
        "Projects",
        "Group related work, folders, and instructions.",
        button("New project", () => projectEditor(), "primary", "plus"),
      ),
    );
    const cards = el("div", { class: "cards" });
    for (const p of state.projects)
      cards.append(
        el(
          "article",
          { class: "card" },
          icon("projects", 22),
          el("h3", { style: "margin-top:16px" }, p.name),
          el(
            "p",
            {},
            p.description || "A workspace for related conversations.",
          ),
          el(
            "small",
            {},
            `${p.folders?.length || 0} folders · ${state.sessions.filter((s) => s.spaceId === p.id).length} conversations`,
          ),
          el(
            "div",
            { class: "card-actions" },
            button("Open project", () => projectDetail(p)),
            button("Edit", () => projectEditor(p), "quiet"),
          ),
        ),
      );
    host.append(
      state.projects.length
        ? cards
        : empty(
            "A place for your next project",
            "Add instructions and folders once, then reuse them across tasks.",
            button("Create project", () => projectEditor(), "primary"),
          ),
    );
  }
  function projectEditor(p) {
    formDialog(
      p ? "Edit project" : "New project",
      [
        { name: "name", label: "Project name", value: p?.name, required: true },
        { name: "description", label: "Description", value: p?.description },
        {
          name: "instructions",
          label: "Project instructions",
          value: p?.instructions,
          multiline: true,
          hint: "Included in the context of tasks started inside this project.",
        },
      ],
      async (values) => {
        if (!values.name.trim()) throw new Error("Enter a project name.");
        await rpc(
          "CoworkSpaces",
          p ? "updateSpace" : "createSpace",
          p ? [p.id, values] : [values],
        );
        await loadProjects();
        renderPage();
      },
    );
  }
  function projectDetail(p) {
    const rows = el(
      "div",
      { class: "field-stack" },
      ...(p.folders || []).map((f) =>
        el(
          "div",
          { class: "list-row" },
          icon("folder"),
          el("span", { class: "path grow" }, f.path),
          button(
            "Remove",
            async () => {
              await rpc("CoworkSpaces", "removeFolderFromSpace", [
                p.id,
                f.path,
              ]);
              await loadProjects();
              d.close();
              projectDetail(state.projects.find((x) => x.id === p.id));
            },
            "quiet",
          ),
        ),
      ),
    );
    const d = dialog(p.name, [
      el("p", {}, p.description),
      p.instructions
        ? el(
            "details",
            {},
            el("summary", {}, "Project instructions"),
            el("pre", {}, p.instructions),
          )
        : null,
      el("h3", {}, "Folders"),
      rows,
      button("Add folder", () =>
        formDialog(
          "Add project folder",
          [{ name: "path", label: "Absolute folder path", required: true }],
          async (values) => {
            const check = await api("/api/quixi/workspace/inspect", {
              path: values.path,
            });
            if (!check.exists) throw new Error("Choose an existing directory.");
            await rpc("CoworkSpaces", "addFolderToSpace", [p.id, check.path]);
            await loadProjects();
            d.close();
            projectDetail(state.projects.find((x) => x.id === p.id));
          },
        ),
      ),
      el("hr"),
      el(
        "div",
        { class: "form-actions" },
        button(
          "Delete project",
          () => {
            d.close();
            confirmDialog(
              "Delete project?",
              `Remove “${p.name}”? Conversations and files remain on disk.`,
              async () => {
                await rpc("CoworkSpaces", "deleteSpace", [p.id]);
                if (state.project === p.id) state.project = null;
                await loadProjects();
                renderPage();
              },
            );
          },
          "danger",
        ),
        button(
          "Start task",
          () => {
            state.project = p.id;
            state.folder = p.folders?.[0]?.path || "";
            d.close();
            navigate("new");
          },
          "primary",
        ),
      ),
    ]);
  }
  async function loadArtifacts() {
    try {
      state.artifacts = (await rpc("CoworkArtifacts", "getAllArtifacts")) || [];
      if (state.page === "artifacts") renderPage();
    } catch (e) {
      toast(e.message, true);
    }
  }
  function artifactsPage(host) {
    host.replaceChildren(
      header(
        "Private artifacts",
        "Local HTML previews created during your tasks.",
      ),
    );
    const cards = el("div", { class: "cards" });
    for (const a of state.artifacts)
      cards.append(
        el(
          "article",
          { class: "card" },
          icon("artifacts", 22),
          el("h3", { style: "margin-top:16px" }, a.name || a.title),
          el("p", {}, a.description || "Private local artifact"),
          el("small", {}, date(a.updatedAt)),
          el(
            "div",
            { class: "card-actions" },
            button("Open preview", () => openArtifact(a.id), "primary"),
            button(
              "Rename",
              () =>
                formDialog(
                  "Rename artifact",
                  [
                    {
                      name: "name",
                      label: "Name",
                      value: a.name || a.title,
                      required: true,
                    },
                  ],
                  async (v) => {
                    if (
                      !(await rpc("CoworkArtifacts", "setArtifactName", [
                        a.id,
                        v.name,
                      ]))
                    )
                      throw new Error("Rename failed.");
                    await loadArtifacts();
                  },
                ),
              "quiet",
            ),
            button(
              "Delete",
              () =>
                confirmDialog(
                  "Delete artifact?",
                  `Permanently remove “${a.name || a.title}”?`,
                  async () => {
                    if (
                      !(await rpc("CoworkArtifacts", "deleteArtifact", [a.id]))
                    )
                      throw new Error("Deletion failed.");
                    await loadArtifacts();
                  },
                ),
              "danger",
            ),
          ),
        ),
      );
    host.append(
      state.artifacts.length
        ? cards
        : empty(
            "Nothing here yet",
            "Ask a task to create a private local HTML artifact.",
          ),
    );
  }
  async function openArtifact(id) {
    const a = await api("/api/quixi/artifacts/" + encodeURIComponent(id));
    const frame = el("iframe", {
      class: "preview-frame",
      sandbox: "allow-scripts allow-forms",
      referrerpolicy: "no-referrer",
      title: a.name || a.title || "Artifact preview",
    });
    frame.srcdoc = a.html || "";
    dialog(
      a.name || a.title || "Artifact",
      [
        el(
          "p",
          {},
          "Private local preview · isolated from the application and its APIs.",
        ),
        frame,
      ],
      { wide: true },
    );
  }
  async function loadSchedules() {
    try {
      state.schedules =
        (await rpc("CoworkScheduledTasks", "getAllScheduledTasks")) || [];
      if (state.page === "schedules") renderPage();
    } catch (e) {
      toast(e.message, true);
    }
  }
  function schedulesPage(host) {
    host.replaceChildren(
      header(
        "Schedules",
        "Save repeatable task definitions.",
        button("New schedule", () => scheduleEditor(), "primary", "plus"),
      ),
      el(
        "div",
        { class: "notice" },
        "Automatic execution is not available in this build. Saved definitions can be started manually.",
      ),
    );
    const list = el("div", { class: "list" });
    for (const t of state.schedules.filter((t) => t.status !== "deleted"))
      list.append(
        el(
          "div",
          { class: "list-row" },
          icon("schedule"),
          el(
            "div",
            { class: "grow" },
            el("h3", {}, t.displayName || t.name),
            el("p", {}, t.cronExpression || t.cron),
            el("small", {}, t.enabled ? "Enabled definition" : "Paused"),
          ),
          button("Run now", () => {
            state.folder = t.cwd || "";
            state.permission = t.permissionMode || "default";
            navigate("new");
            document.getElementById("prompt").value = t.prompt || "";
            saveDraft();
          }),
          button("Edit", () => scheduleEditor(t), "quiet"),
          button(
            t.enabled ? "Pause" : "Enable",
            async () => {
              await rpc("CoworkScheduledTasks", "updateScheduledTaskStatus", [
                t.id,
                t.enabled ? "disabled" : "enabled",
              ]);
              await loadSchedules();
            },
            "quiet",
          ),
          button(
            "Delete",
            () =>
              confirmDialog(
                "Delete schedule?",
                `Remove “${t.displayName || t.name}”?`,
                async () => {
                  await rpc("CoworkScheduledTasks", "deleteScheduledTask", [
                    t.id,
                  ]);
                  await loadSchedules();
                },
              ),
            "danger",
          ),
        ),
      );
    host.append(
      list.children.length
        ? list
        : empty(
            "Keep a useful prompt handy",
            "Save a prompt and schedule definition for later use.",
          ),
    );
  }
  function scheduleEditor(t) {
    formDialog(
      t ? "Edit schedule" : "New schedule",
      [
        { name: "name", label: "Name", value: t?.displayName, required: true },
        {
          name: "prompt",
          label: "Prompt",
          value: t?.prompt,
          multiline: true,
          required: true,
        },
        {
          name: "cronExpression",
          label: "Cron expression",
          value: t?.cronExpression || "0 9 * * *",
          hint: "Five fields: minute, hour, day, month, weekday. Automatic dispatch is not implemented.",
        },
        {
          name: "cwd",
          label: "Workspace folder",
          value: t?.cwd || currentFolder(),
        },
      ],
      async (v) => {
        if (v.cronExpression.trim().split(/\s+/).length !== 5)
          throw new Error("Enter a five-field cron expression.");
        await rpc(
          "CoworkScheduledTasks",
          t ? "updateScheduledTask" : "createScheduledTask",
          [{ ...v, id: t?.id, enabled: t?.enabled ?? true }],
        );
        await loadSchedules();
      },
    );
  }
  let toolsData = { skills: [], servers: [] };
  async function loadTools() {
    try {
      const [skills, servers] = await Promise.all([
        rpc("LocalAgentModeSessions", "listLocalSkills"),
        rpc("LocalAgentModeSessions", "getLocalMcpServers"),
      ]);
      toolsData = { skills: skills || [], servers: servers || [] };
      if (state.page === "tools") renderPage();
    } catch (e) {
      toast(e.message, true);
    }
  }
  function toolsPage(host) {
    host.replaceChildren(
      header(
        "Tools & skills",
        "Inspect the instructions and tool connections available to your tasks.",
      ),
    );
    host.append(el("h2", {}, "Skills"));
    const skills = el("div", { class: "cards" });
    for (const skill of toolsData.skills)
      skills.append(
        el(
          "article",
          { class: "card" },
          el("h3", {}, skill.displayName || skill.name),
          el("p", {}, skill.description),
          button(
            "Read instructions",
            async () => {
              const files = await rpc(
                "LocalAgentModeSessions",
                "getLocalSkillFiles",
                [skill.skillId],
              );
              dialog(
                skill.name,
                (files || []).map((f) =>
                  el(
                    "details",
                    { open: f.path === "SKILL.md" },
                    el("summary", {}, f.path),
                    el("pre", {}, f.content || ""),
                  ),
                ),
                { wide: true },
              );
            },
            "quiet",
          ),
        ),
      );
    host.append(
      skills,
      el("hr"),
      el(
        "div",
        { class: "row" },
        el("h2", { class: "grow" }, "MCP connections"),
        button("Configure connections", () => mcpEditor(), "quiet"),
      ),
      el(
        "p",
        { class: "muted" },
        "Configured tool definitions are listed below. A connection is verified when a tool call succeeds.",
      ),
    );
    const list = el("div", { class: "list" });
    for (const s of toolsData.servers)
      list.append(
        el(
          "div",
          { class: "list-row" },
          icon("tools"),
          el(
            "div",
            { class: "grow" },
            el("h3", {}, s.name),
            el(
              "p",
              {},
              s.description || `${s.tools?.length || 0} tool definitions`,
            ),
          ),
          el("span", { class: "tag" }, "Configured"),
        ),
      );
    host.append(list);
  }
  async function mcpEditor() {
    const config = await rpc(
      "MCP",
      "getMcpServersConfig",
      [],
      "quixi.settings",
    );
    formDialog(
      "MCP configuration",
      [
        {
          name: "json",
          label: "Server configuration (JSON)",
          value: JSON.stringify({ mcpServers: config }, null, 2),
          multiline: true,
          attrs: { rows: 15 },
          hint: "External MCP commands run with your account permissions. Add only trusted commands.",
        },
      ],
      async (values) => {
        const parsed = JSON.parse(values.json);
        if (
          !parsed.mcpServers ||
          typeof parsed.mcpServers !== "object" ||
          Array.isArray(parsed.mcpServers)
        )
          throw new Error("Provide a mcpServers object.");
        for (const [name, entry] of Object.entries(parsed.mcpServers)) {
          if (
            !entry ||
            typeof entry.command !== "string" ||
            !entry.command.trim()
          )
            throw new Error(`A command is required for ${name}.`);
        }
        await rpc(
          "MCP",
          "setMcpServerConfigs",
          [parsed.mcpServers],
          "quixi.settings",
        );
        await loadTools();
      },
      { wide: true },
    );
  }
  function settingsPage(host) {
    const tabs = [
      ["general", "General"],
      ["providers", "Providers"],
      ["privacy", "Privacy & data"],
      ["about", "About"],
    ];
    const content = el("section", {});
    host.replaceChildren(
      header("Settings", "Make the workspace yours."),
      el(
        "div",
        { class: "settings-layout" },
        el(
          "nav",
          { class: "settings-nav", "aria-label": "Settings sections" },
          ...tabs.map(([id, label]) =>
            button(
              label,
              () => {
                state.settingsTab = id;
                renderPage();
              },
              state.settingsTab === id ? "active" : "",
            ),
          ),
        ),
        content,
      ),
    );
    if (state.settingsTab === "general") {
      content.append(
        el("h2", {}, "Appearance"),
        field(
          "Theme",
          select(
            [
              ["dark", "Dark"],
              ["light", "Light"],
              ["system", "System"],
            ],
            localStorage.getItem("quixi.theme") || "dark",
            (v) => {
              localStorage.setItem("quixi.theme", v);
              applyTheme();
            },
          ),
        ),
        el("h2", { style: "margin-top:30px" }, "New tasks"),
        field(
          "Default permission mode",
          select(
            [
              ["default", "Manual"],
              ["acceptEdits", "Allow file edits"],
              ["auto", "Auto"],
            ],
            state.permission,
            (v) => {
              state.permission = v;
              localStorage.setItem("quixi.permission", v);
            },
          ),
          "Manual blocks shell execution and file changes. Auto permits tool actions with the built-in destructive-command guard.",
        ),
        el(
          "div",
          { class: "notice" },
          "The original interface currently uses English. System fonts and native keyboard shortcuts are supported.",
        ),
      );
    } else if (state.settingsTab === "providers") {
      content.append(
        el("h2", {}, "Model providers"),
        el("p", {}, `${state.providers?.configuredCount || 0} configured`),
        button("Manage providers", () => providerDirectory(), "primary"),
      );
    } else if (state.settingsTab === "privacy") {
      content.append(
        el("h2", {}, "Local conversations"),
        el(
          "p",
          { class: "muted" },
          "Conversations, projects, and provider settings are stored on this device. Sending a prompt transmits its contents and task context to the provider you select.",
        ),
        el("h3", {}, "Storage protection"),
        el(
          "p",
          { class: "muted" },
          "A recovery copy is created if normal saving fails. Keep the app open if both locations fail; the persistent storage notice offers retry and a recovery copy.",
        ),
        button("Inspect storage status", async () => {
          const status = await api("/api/quixi/storage/status");
          dialog(
            "Conversation storage",
            el("pre", {}, JSON.stringify(status, null, 2)),
          );
        }),
        el("hr"),
        button("View archived conversations", () => {
          navigate("search");
          const filter = document.querySelector("#page-host select");
          if (filter) {
            filter.value = "archived";
            filter.dispatchEvent(new Event("change"));
          }
        }),
      );
    } else
      content.append(
        el("img", {
          src: "/quixi-mark.svg",
          alt: "QuixiDesk",
          width: 58,
          height: 58,
        }),
        el("h2", { style: "margin-top:20px" }, "QuixiDesk"),
        el(
          "p",
          { class: "muted" },
          "An independent native AI workspace built with Rust.",
        ),
        el("p", {}, "Original interface and task instructions · MIT license"),
        el(
          "p",
          { class: "muted" },
          "Uses the system webview and optional model providers. Native operating-system capabilities vary by platform.",
        ),
        button(
          "Open project website",
          () => openExternal("https://github.com/QuixiAI/QuixiDesk"),
          "quiet",
        ),
      );
  }
  function applyTheme() {
    const choice = localStorage.getItem("quixi.theme") || "dark";
    document.documentElement.dataset.theme =
      choice === "system"
        ? matchMedia("(prefers-color-scheme:dark)").matches
          ? "dark"
          : "light"
        : choice;
  }
  async function providerDirectory() {
    await loadProviders();
    const body = el("div", {});
    const draw = () => {
      body.replaceChildren(
        el(
          "p",
          {},
          "Connect the model services you want to use. Credentials stay in the local provider settings; requests go directly to the selected endpoint.",
        ),
        el(
          "div",
          { class: "row" },
          el(
            "span",
            { class: "tag" },
            `${state.providers.configuredCount} configured`,
          ),
          button(
            "Detect local servers",
            async () => {
              const r = await api("/api/quixi/providers/detect-local", {});
              if (r.error) throw new Error(r.error);
              await loadProviders();
              draw();
            },
            "quiet",
          ),
        ),
        el(
          "div",
          { class: "cards", style: "margin-top:20px" },
          ...state.providers.providers.map((p) =>
            el(
              "article",
              {
                class:
                  "card provider-card" +
                  (p.id === state.providers.activeProvider ? " selected" : ""),
              },
              el(
                "div",
                { class: "provider-header" },
                el(
                  "span",
                  { class: "provider-badge", "aria-hidden": "true" },
                  p.name.slice(0, 1),
                ),
                el(
                  "div",
                  {},
                  el("h3", {}, p.name),
                  el(
                    "small",
                    {},
                    p.configured
                      ? "Configured"
                      : p.category === "local"
                        ? "Local service"
                        : "Not connected",
                  ),
                ),
              ),
              el(
                "p",
                {},
                p.category === "local"
                  ? "Connect to a model service on your computer."
                  : p.id === "custom"
                    ? "Use your own compatible model endpoint."
                    : "Connect directly to this provider.",
              ),
              el(
                "div",
                { class: "card-actions" },
                button(
                  "Configure",
                  () =>
                    providerEditor(p, async () => {
                      await loadProviders();
                      draw();
                    }),
                  "primary",
                ),
                p.configured
                  ? button(
                      "Use provider",
                      async () => {
                        const r = await api(
                          "/api/quixi/providers/complete-onboarding",
                          {
                            activeProvider: p.id,
                            activeModel: p.selectedModel,
                          },
                        );
                        if (r.error) throw new Error(r.error);
                        await loadProviders();
                        toast("Active provider updated.");
                        draw();
                      },
                      "quiet",
                    )
                  : null,
              ),
            ),
          ),
        ),
      );
    };
    draw();
    dialog("Model providers", body, { wide: true });
  }
  async function providerEditor(p, changed) {
    const error = el("div", { class: "form-error", role: "alert" });
    const key = el("input", {
      type: "password",
      autocomplete: "new-password",
      placeholder: p.configured
        ? "Leave blank to keep the stored credential"
        : p.keyPlaceholder || "API key",
      "aria-label": "API key",
    });
    const base = el("input", {
      type: "url",
      value: p.baseUrl || "",
      required: true,
    });
    const models = [
      ...(p.models || []).map((m) => [m.id, m.label || m.id]),
      ...(p.customModels || [])
        .filter((id) => !p.models?.some((m) => m.id === id))
        .map((id) => [id, id]),
    ];
    const model =
      p.id === "custom"
        ? el("input", {
            value: p.selectedModel || "",
            required: true,
            placeholder: "Model ID",
          })
        : select(models, p.selectedModel);
    const format = select(
      [
        ["openai", "Chat completions API"],
        ["anthropic", "Messages API"],
      ],
      p.apiFormat || "openai",
    );
    const content = el(
      "div",
      {},
      el(
        "p",
        {},
        p.category === "local"
          ? "Connect to a model service on your computer."
          : p.id === "custom"
            ? "Use your own compatible model endpoint."
            : "Connect directly to this provider.",
      ),
      field(
        "API key",
        key,
        p.apiKeyMasked
          ? "Stored credential: " + p.apiKeyMasked
          : "Never paste a key into a conversation.",
      ),
      field("Endpoint", base),
      field("Model", model),
    );
    if (p.id === "custom") content.append(field("API format", format));
    const payload = () => ({
      providerId: p.id,
      ...(key.value ? { apiKey: key.value } : {}),
      baseUrl: base.value.trim(),
      selectedModel: model.value.trim(),
      apiFormat: format.value,
      setAsActive: true,
    });
    let oauthTimer;
    const actions = el(
      "div",
      { class: "form-actions" },
      button("Cancel", () => d.close(), "quiet"),
      button(
        "Test connection",
        async () => {
          try {
            error.textContent = "Testing…";
            const r = await api("/api/quixi/providers/test", payload());
            error.textContent = r.ok
              ? "Connection succeeded."
              : r.error || r.message || "Connection failed.";
          } catch (e) {
            error.textContent = e.message;
          }
        },
        "quiet",
      ),
      button(
        "Save & use provider",
        async () => {
          try {
            if (p.id === "custom" && !model.value.trim())
              throw new Error("Enter a model ID.");
            const r = await api("/api/quixi/providers/save", payload());
            if (r.error) throw new Error(r.error);
            const r2 = await api("/api/quixi/providers/complete-onboarding", {
              activeProvider: p.id,
              activeModel: model.value,
            });
            if (r2.error) throw new Error(r2.error);
            await loadProviders();
            await changed?.();
            d.close();
            toast("Provider saved.");
          } catch (e) {
            error.textContent = e.message;
          }
        },
        "primary",
      ),
    );
    if (p.supportsOauth || p.supportsOAuth) {
      const oauth = el("div", {});
      content.append(
        el("hr"),
        button(
          "Sign in with your account",
          async () => {
            try {
              const r = await api("/api/quixi/providers/oauth/start", {
                providerId: p.id,
                openBrowser: false,
              });
              if (r.error) throw new Error(r.error);
              const url = r.verificationUrl || r.authorizeUrl;
              oauth.replaceChildren(
                el(
                  "p",
                  {},
                  "Complete sign-in in your browser, then check the connection.",
                ),
                r.userCode ? el("pre", {}, r.userCode) : null,
                url
                  ? el(
                      "div",
                      { class: "row" },
                      button(
                        "Open verification page",
                        () => openExternal(url),
                        "primary",
                      ),
                      button("Copy link", async () => {
                        await copy(url);
                        toast("Verification link copied.");
                      }),
                    )
                  : null,
                button("Check sign-in", async () => {
                  const v = await api("/api/quixi/providers/oauth/poll", {
                    providerId: p.id,
                    forceComplete: true,
                  });
                  if (v.error) throw new Error(v.error);
                  await loadProviders();
                  await changed?.();
                  if (
                    state.providers.providers.find((x) => x.id === p.id)
                      ?.configured
                  ) {
                    toast("Account connected.");
                    d.close();
                  } else toast("Sign-in is still pending.");
                }),
              );
            } catch (e) {
              error.textContent = e.message;
            }
          },
          "quiet",
        ),
        oauth,
      );
    }
    content.append(error, actions);
    const d = dialog(p.name, content);
    d.addEventListener("close", () => {
      clearInterval(oauthTimer);
      key.value = "";
    });
  }
  async function modelPicker() {
    await loadProviders();
    const providers = state.providers.providers.filter((p) => p.configured);
    if (!providers.length) {
      providerDirectory();
      return;
    }
    let selected = state.providers.activeProvider;
    const ps = select(
      providers.map((p) => [p.id, p.name]),
      selected,
      (v) => {
        selected = v;
        draw();
      },
      { "aria-label": "Provider" },
    );
    const models = el("div", {});
    const d = dialog("Choose model", [field("Provider", ps), models]);
    function draw() {
      const p = providers.find((x) => x.id === selected) || providers[0];
      models.replaceChildren(
        ...p.models.map((m) =>
          button(
            [
              el("span", { class: "grow" }, m.label || m.id),
              m.id === p.selectedModel ? icon("check", 14) : null,
            ],
            async () => {
              const r = await api("/api/quixi/providers/save", {
                providerId: p.id,
                selectedModel: m.id,
                setAsActive: true,
              });
              if (r.error) throw new Error(r.error);
              if (session())
                await rpc(service(session()), "setModel", [state.id, m.id]);
              await loadProviders();
              d.close();
              toast("Model updated.");
            },
            "list-row",
          ),
        ),
      );
    }
    draw();
  }
  async function openExternal(url) {
    const parsed = new URL(url);
    if (!["https:", "http:"].includes(parsed.protocol))
      throw new Error("Only HTTP and HTTPS links can be opened.");
    await api("/__eipc__/open-external", { url: parsed.href });
  }
  async function toggleBrowser() {
    state.browser = !state.browser;
    if (state.browser) state.browserState = await api("/api/quixi/browser");
    renderPage();
  }
  async function browserAction(action, extra = {}) {
    state.browserState = await api("/api/quixi/browser", {
      action,
      tabId: state.browserState?.selectedTabId,
      ...extra,
    });
    const host = document.getElementById("page-host");
    renderBrowser(host);
  }
  let browserRenderVersion = 0;
  async function renderBrowser(host) {
    const version = ++browserRenderVersion;
    const data = state.browserState || (await api("/api/quixi/browser"));
    state.browserState = data;
    const selected = (data.tabs || []).find(
      (t) => t.tabId === data.selectedTabId || t.id === data.selectedTabId,
    );
    const address = el("input", {
      value: selected?.url || "",
      placeholder: "https://example.com",
      "aria-label": "Browser address",
      onKeydown: (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          browserAction("navigate", { url: address.value });
        }
      },
    });
    const frame = el("iframe", {
      sandbox: "allow-scripts allow-forms",
      referrerpolicy: "no-referrer",
      title: "Isolated browser content",
      id: "browser-frame",
    });
    frame.srcdoc = await fetch(
      "/api/quixi/browser/render?tab_id=" +
        encodeURIComponent(data.selectedTabId || 1),
    ).then((r) => r.text());
    if (version !== browserRenderVersion || !state.browser || !host.isConnected)
      return;
    const panel = el(
      "aside",
      { class: "browser-panel" },
      el(
        "div",
        { class: "browser-head" },
        icon("browser", 16),
        el("strong", {}, "QuixiDesk Browser"),
        ib("Close browser", "close", () => {
          state.browser = false;
          panel.remove();
        }),
      ),
      el(
        "div",
        { class: "browser-tabs" },
        ...(data.tabs || []).map((t) =>
          button(
            t.title || "New tab",
            () => browserAction("select_tab", { tabId: t.tabId || t.id }),
            data.selectedTabId === (t.tabId || t.id) ? "active" : "",
          ),
        ),
        ib("New browser tab", "plus", () => browserAction("new_tab")),
      ),
      el(
        "div",
        { class: "browser-controls" },
        ib("Browser back", "back", () => browserAction("back")),
        ib("Browser forward", "arrow", () => browserAction("forward")),
        ib("Reload browser", "refresh", () => browserAction("reload")),
        button("Close tab", () => browserAction("close_tab"), "quiet"),
      ),
      el(
        "form",
        {
          class: "browser-address",
          onSubmit: (e) => {
            e.preventDefault();
            browserAction("navigate", { url: address.value }).catch((e) =>
              toast(e.message, true),
            );
          },
        },
        address,
        el("button", { type: "submit" }, "Go"),
      ),
      frame,
      el(
        "div",
        { class: "browser-status" },
        "Isolated page snapshot · remote content cannot access conversations.",
      ),
    );
    host.querySelector(".browser-panel")?.remove();
    host.append(panel);
  }
  addEventListener("message", (e) => {
    const frame = document.getElementById("browser-frame");
    if (
      !frame ||
      e.source !== frame.contentWindow ||
      e.origin !== "null" ||
      !e.data ||
      typeof e.data !== "object"
    )
      return;
    const data = e.data;
    if (data.type === "quixi-browser-click" && typeof data.ref === "string")
      browserAction("click_ref", { ref: data.ref });
    if (data.type === "quixi-browser-navigate" && typeof data.url === "string")
      browserAction("navigate", { url: data.url });
  });
  async function contextDrawer() {
    const host = document.getElementById("page-host");
    host.querySelector(".drawer")?.remove();
    const s = session();
    if (!s) return;
    const folder = s.cwd || currentFolder();
    const files = await rpc("LocalSessions", "listDirectoryAtCwd", [folder]);
    const drawer = el(
      "aside",
      { class: "drawer" },
      el(
        "div",
        { class: "row" },
        el("h3", { class: "grow" }, "Files & context"),
        ib("Close context", "close", () => drawer.remove()),
      ),
      el("p", { class: "path" }, folder),
      project()?.instructions
        ? el(
            "details",
            {},
            el("summary", {}, "Project instructions"),
            el("pre", {}, project().instructions),
          )
        : null,
    );
    for (const f of files || [])
      drawer.append(
        button(
          [icon(f.isDir || f.isDirectory ? "folder" : "file", 14), f.name],
          async () => {
            if (f.isDir || f.isDirectory) {
              const items = await rpc("LocalSessions", "listDirectoryAtCwd", [
                f.path,
              ]);
              dialog(
                f.name,
                items.map((x) => el("p", { class: "path" }, x.path)),
              );
              return;
            }
            const r = await api("/api/quixi/workspace/read", { path: f.path });
            dialog(
              f.name,
              el(
                "pre",
                { class: "code-preview" },
                ...r.contents
                  .split("\n")
                  .map((line) =>
                    el("span", { class: "code-line" }, line || " "),
                  ),
              ),
              { wide: true },
            );
          },
          "file-button",
        ),
      );
    if (!files?.length)
      drawer.append(
        el("p", { class: "muted" }, "This folder has no visible entries."),
      );
    host.append(drawer);
  }
  const refreshTimers = new Map();
  addEventListener("quixi:event", ({ detail: { channel, payload } }) => {
    if (channel === "quixi:browser-updated") {
      state.browserState = payload;
      if (state.browser) renderBrowser(document.getElementById("page-host"));
      return;
    }
    if (channel === "quixi:artifact-preview") {
      loadArtifacts();
      return;
    }
    if (
      !payload ||
      (!channel.includes("LocalAgentModeSessions") &&
        !channel.includes("LocalSessions"))
    )
      return;
    const id = payload.sessionId;
    if (!id) return;
    const message = payload.message;
    if (message?.type === "stream_event") {
      let stream = state.streams.get(id);
      if (!stream) {
        stream = { text: "", thinking: "" };
        state.streams.set(id, stream);
      }
      const event = message.event;
      if (event?.type === "message_start") {
        stream.text = "";
        stream.thinking = "";
      }
      if (event?.delta?.type === "text_delta")
        stream.text += event.delta.text || "";
      if (event?.delta?.type === "thinking_delta")
        stream.thinking += event.delta.thinking || "";
      if (state.id === id) renderTranscript();
      return;
    }
    if (
      message?.type === "assistant" ||
      payload.type === "close" ||
      payload.type === "error"
    )
      state.streams.delete(id);
    clearTimeout(refreshTimers.get(id));
    refreshTimers.set(
      id,
      setTimeout(async () => {
        try {
          await refreshSession(id);
          if (state.id === id) {
            renderTranscript();
            updateSendButton();
          }
        } catch (e) {
          toast(e.message, true);
        } finally {
          refreshTimers.delete(id);
        }
      }, 50),
    );
  });
  async function boot() {
    applyTheme();
    state.permission = localStorage.getItem("quixi.permission") || "default";
    document.body.dataset.platform = /Mac/.test(navigator.platform)
      ? "macos"
      : /Win/.test(navigator.platform)
        ? "windows"
        : "linux";
    try {
      await Promise.all([loadSessions(), loadProjects(), loadProviders()]);
      const defaults = await rpc(
        "LocalAgentModeSessions",
        "getDefaultWorkspaceFolders",
      );
      state.folder = defaults?.[0] || "";
      const path = decodeURIComponent(location.pathname.slice(1));
      navigate(path || "new", null, { replace: true });
      if (!state.providers.hasConfiguredProvider)
        toast("Welcome. Connect a provider to start your first task.");
    } catch (e) {
      root.replaceChildren(
        el(
          "div",
          { class: "page" },
          el("h1", {}, "Unable to open the workspace"),
          el("p", { class: "error-text" }, e.message),
          button("Try again", () => boot(), "primary"),
        ),
      );
    }
  }
  addEventListener("keydown", (e) => {
    const command = e.metaKey || e.ctrlKey;
    if (command && e.key.toLowerCase() === "n") {
      e.preventDefault();
      navigate("new");
    } else if (command && e.key.toLowerCase() === "k") {
      e.preventDefault();
      navigate("search");
    } else if (command && e.key === ",") {
      e.preventDefault();
      navigate("settings");
    }
  });
  matchMedia("(prefers-color-scheme:dark)").addEventListener("change", () =>
    applyTheme(),
  );
  // Refresh persisted state after returning from a background native window.
  addEventListener("focus", () => {
    if (state.sessions.length)
      loadSessions()
        .then(() => {
          if (state.page === "chat") openTranscript();
        })
        .catch(() => {});
  });
  boot();
})();
