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
    stopped: new Set(),
    approvals: new Map(),
    subagents: new Map(),
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
      session()?.cwd || state.folder || project()?.folders?.[0]?.path || ""
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
    d.setAttribute("aria-label", title);
    d.tabIndex = -1;
    const initialFocus =
      d.querySelector("input:not([type=hidden]), textarea, select") || d;
    initialFocus.focus();
    if (initialFocus instanceof HTMLTextAreaElement) {
      initialFocus.setSelectionRange(0, 0);
      initialFocus.scrollTop = 0;
    }
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
    if (state.providers.credentialStorageError) toast("Credentials could not be saved or unlocked: " + state.providers.credentialStorageError, true);
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
    if (page === "chat") {
      const opened = session();
      state.project = opened?.spaceId || null;
      state.folder = opened?.cwd || "";
    }
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
    const navigationButton = (label, name, action, cls = "") => {
      const control = button(
        [icon(name), el("span", { class: "nav-label" }, label)],
        action,
        cls,
      );
      control.setAttribute("aria-label", label);
      control.title = label;
      return control;
    };
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
          el("span", { class: "brand-caption" }, "Personal workspace"),
        ),
      ),
      el(
        "nav",
        { "aria-label": "Main navigation" },
        navigationButton(
          "New task",
          "plus",
          () => {
            state.project = null;
            state.folder = "";
            navigate("new");
          },
          "new-task",
        ),
        ...nav.map(([page, label, ic]) =>
          navigationButton(
            label,
            ic,
            () => navigate(page),
            state.page === page ? "active" : "",
          ),
        ),
      ),
      el("div", { class: "recent", id: "recents" }),
      el(
        "footer",
        {},
        navigationButton(
          "Settings",
          "settings",
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
      el("div", { class: "crumb" }, el("strong", {}, crumb)),
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
    document.body.dataset.page = state.page;
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
  let conversationFind = null;
  function closeConversationFind() {
    document.getElementById("conversation-find")?.remove();
    for (const mark of document.querySelectorAll("#conversation mark[data-find]")) mark.replaceWith(document.createTextNode(mark.textContent));
    conversationFind = null;
    document.getElementById("prompt")?.focus();
  }
  function refreshConversationFind(scroll = true) {
    if (!conversationFind) return;
    const target = document.getElementById("conversation");
    if (!target || conversationFind.session !== state.id) { closeConversationFind(); return; }
    for (const mark of target.querySelectorAll("mark[data-find]")) mark.replaceWith(document.createTextNode(mark.textContent));
    target.normalize();
    const query = conversationFind.input.value.toLocaleLowerCase();
    const matches = [];
    if (query) {
      const walker = document.createTreeWalker(target, NodeFilter.SHOW_TEXT);
      const nodes = []; while (walker.nextNode()) nodes.push(walker.currentNode);
      for (const node of nodes) {
        if (node.parentElement.closest("button,textarea,input,script,style")) continue;
        const text = node.textContent; const lower = text.toLocaleLowerCase();
        let at = 0, pos = lower.indexOf(query); if (pos < 0) continue;
        const fragment = document.createDocumentFragment();
        while (pos >= 0 && matches.length < 2000) {
          fragment.append(document.createTextNode(text.slice(at, pos)));
          const mark = el("mark", {"data-find":"true"}, text.slice(pos, pos + query.length));
          fragment.append(mark); matches.push(mark); at = pos + query.length; pos = lower.indexOf(query, at);
        }
        fragment.append(document.createTextNode(text.slice(at))); node.replaceWith(fragment);
      }
    }
    conversationFind.matches = matches;
    conversationFind.index = matches.length ? ((conversationFind.index % matches.length) + matches.length) % matches.length : 0;
    conversationFind.status.textContent = matches.length ? `${conversationFind.index + 1} of ${matches.length}${matches.length === 2000 ? "+" : ""}` : "No matches";
    const current = matches[conversationFind.index];
    if (current) { current.classList.add("find-current"); if (scroll) { for (let p=current.parentElement; p && p!==target; p=p.parentElement) if (p.tagName==="DETAILS") p.open=true; current.scrollIntoView?.({block:"center"}); } }
  }
  function stepConversationFind(delta) { if (conversationFind) { conversationFind.index += delta; refreshConversationFind(); } }
  function openConversationFind() {
    if (!session()) return;
    if (conversationFind && document.getElementById("conversation-find")) { conversationFind.input.focus(); conversationFind.input.select(); return; }
    const input = el("input", {type:"search",placeholder:"Find in this conversation", "aria-label":"Find in conversation"});
    const status = el("span", {role:"status", "aria-live":"polite"}, "No matches");
    const bar = el("div", {id:"conversation-find",class:"conversation-find",role:"search"},input,status,
      button("Previous match",()=>stepConversationFind(-1),"quiet"),button("Next match",()=>stepConversationFind(1),"quiet"),button("Close find",closeConversationFind,"quiet","close"));
    conversationFind={input,status,index:0,matches:[],session:state.id};
    input.addEventListener("input",()=>{conversationFind.index=0;refreshConversationFind();});
    input.addEventListener("keydown",e=>{if(e.key==="Enter"){e.preventDefault();stepConversationFind(e.shiftKey?-1:1);}else if(e.key==="Escape"){e.preventDefault();closeConversationFind();}});
    document.getElementById("conversation").before(bar);input.focus();
  }
  function toolResultView(content) {
    let media;try{media=JSON.parse(content);}catch{}
    if(media?._quixi_tool_media) {
      let label=media.text;try{label=JSON.parse(label).content?.map(p=>p.text||"").filter(Boolean).join("\n")||label;}catch{}
      return el("div",{},el("p",{},label),...(media.images||[]).map(p=>el("img",{src:p.image_url.url,alt:"Screenshot returned by tool",class:"tool-screenshot",loading:"lazy"})));
    }
    return el("pre",{},typeof content === "string" ? content : JSON.stringify(content,null,2));
  }
  function chatPage(host) {
    const s = session();
    const main = el("section", { class: "chat-main" + (s ? "" : " is-new") });
    if (s)
      main.append(
        el(
          "div",
          { class: "session-heading" },
          el("h2", { class: "visually-hidden" }, s.title || "Untitled task"),
          el(
            "span",
            { class: "context-label" },
            icon("folder", 14),
            currentFolder().split(/[\\/]/).filter(Boolean).pop() || "Workspace",
          ),
          button("Find in conversation", () => openConversationFind(), "quiet", "search"),
          button("Files & context", () => contextDrawer(), "quiet"),
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
            project() ? project().name : "YOUR WORKSPACE",
          ),
          el("h1", {}, "Where should we start?"),
          el(
            "p",
            {},
            "Explore a question, work with your files, or make something useful.",
          ),
          el(
            "div",
            { class: "suggestions" },
            ...[
              [
                "Plan my next project",
                "Help me make a practical plan for my next project. Start by asking about the outcome and constraints.",
                "projects",
                "Turn an idea into next steps",
              ],
              [
                "Understand a codebase",
                "Help me understand this workspace. Start with a read-only overview of the project structure.",
                "code",
                "Get oriented in your workspace",
              ],
              [
                "Make a useful artifact",
                "Help me create a private local HTML artifact. Ask what it should do before building it.",
                "artifacts",
                "Build something you can use",
              ],
            ].map(([label, prompt, ic, description]) =>
              button(
                [
                  icon(ic, 17),
                  el(
                    "span",
                    {},
                    el("strong", {}, label),
                    el("small", {}, description),
                  ),
                ],
                () => {
                  document.getElementById("prompt").value = prompt;
                  saveDraft();
                  document.getElementById("prompt").focus();
                },
              ),
            ),
          ),
        ),
      );
    main.append(conversation, composer());
    if (!s) main.append(conversation.querySelector(".suggestions"));
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
    const chooseModel = button(
      [
        el("span", {}, state.providers?.activeModel || "Choose model"),
        el("span", { class: "chevron", "aria-hidden": "true" }, "⌄"),
      ],
      () => modelPicker(),
      "quiet model-picker-trigger",
    );
    chooseModel.setAttribute("aria-label", "Choose model");
    chooseModel.title = state.providers?.activeModel || "Choose model";
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
          el("span", { class: "model-control" }, chooseModel),
          send,
        ),
      ),
      el(
        "p",
        { class: "composer-note" },
        "Enter to send · Shift+Enter for a new line",
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
      placeholder: "e.g. ~/Projects/my-project",
      "aria-label": "Workspace folder path",
    });
    const error = el("div", { class: "form-error", role: "alert" });
    const browse = button("Choose folder", async () => {
      const folder = await rpc(
        "LocalAgentModeSessions",
        "browseAndAddTrustedFolder",
        ["Choose a workspace folder"],
      );
      if (folder) input.value = folder;
      else toast("No folder selected. You can also enter a path below.");
    });
    const d = dialog("Choose workspace", [
      el(
        "p",
        {},
        "Tasks can inspect this folder. In Manual mode, QuixiDesk asks before changing files or running commands.",
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
                throw new Error("Choose an existing folder.");
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
                await loadSessions();
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
    if (!state.providers.activeModel) {
      modelPicker();
      toast("Choose a model before sending a task.");
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
      state.stopped.delete(s.sessionId);
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
    state.stopped.add(s.sessionId);
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
    let old = state.sessions.find((s) => s.sessionId === id);
    if (!old) { await loadSessions(); old = state.sessions.find((s) => s.sessionId === id); }
    if (!old) return;
    const value = await rpc(service(old), "getSession", [id]);
    if (value) {
      Object.assign(old, value);
      state.approvals.set(
        id,
        (await rpc(service(old), "getToolPermissions", [id])) || [],
      );
      const children = await rpc(service(old), "listSubagents", [id]);
      state.subagents.set(id, Array.isArray(children) ? children : []);
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
  // Render a small, safe Markdown subset as DOM nodes. Model text is never HTML.
  function inlineText(text) {
    const out = document.createDocumentFragment();
    const pattern =
      /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\n]+\*|\[[^\]\n]+\]\([^\s)]+\))/g;
    let last = 0;
    for (const match of text.matchAll(pattern)) {
      out.append(document.createTextNode(text.slice(last, match.index)));
      const token = match[0];
      if (token.startsWith("`")) out.append(el("code", {}, token.slice(1, -1)));
      else if (token.startsWith("**"))
        out.append(el("strong", {}, token.slice(2, -2)));
      else if (token.startsWith("*"))
        out.append(el("em", {}, token.slice(1, -1)));
      else {
        const [, label, url] = token.match(/^\[([^\]]+)\]\((.+)\)$/);
        const artifact = url.match(
          /^app:\/\/localhost\/cowork-artifact\/([a-zA-Z0-9-]+)$/,
        );
        if (artifact || /^https?:\/\//i.test(url)) {
          out.append(
            el(
              "a",
              {
                href: artifact ? "#" : url,
                onClick: (event) => {
                  event.preventDefault();
                  (artifact
                    ? openArtifact(artifact[1])
                    : openExternal(url)
                  ).catch((error) => toast(error.message, true));
                },
              },
              label,
            ),
          );
        } else out.append(document.createTextNode(token));
      }
      last = match.index + token.length;
    }
    out.append(document.createTextNode(text.slice(last)));
    return out;
  }
  function plainBody(text) {
    const box = el("div", { class: "body" });
    const lines = String(text || "")
      .replace(/\r\n/g, "\n")
      .split("\n");
    const cells = (line) =>
      line
        .trim()
        .replace(/^\||\|$/g, "")
        .split("|")
        .map((cell) => cell.trim());
    for (let i = 0; i < lines.length;) {
      const line = lines[i];
      if (!line.trim()) {
        i++;
        continue;
      }
      if (/^\s*```/.test(line)) {
        const language = line.trim().slice(3).trim() || "Code";
        const code = [];
        i++;
        while (i < lines.length && !/^\s*```/.test(lines[i]))
          code.push(lines[i++]);
        if (i < lines.length) i++;
        const value = code.join("\n");
        box.append(
          el(
            "div",
            { class: "code-block" },
            el(
              "div",
              { class: "code-head" },
              el("span", {}, language),
              button("Copy code", () => copy(value), "quiet"),
            ),
            el("pre", {}, el("code", {}, value)),
          ),
        );
      } else if (/^#{1,6}\s/.test(line)) {
        const [, hashes, title] = line.match(/^(#{1,6})\s+(.*)$/);
        box.append(
          el("h" + Math.min(hashes.length + 1, 6), {}, inlineText(title)),
        );
        i++;
      } else if (
        i + 1 < lines.length &&
        line.includes("|") &&
        /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(lines[i + 1])
      ) {
        const table = el(
          "table",
          {},
          el(
            "thead",
            {},
            el(
              "tr",
              {},
              ...cells(line).map((c) => el("th", {}, inlineText(c))),
            ),
          ),
        );
        const body = el("tbody", {});
        i += 2;
        while (i < lines.length && lines[i].trim() && lines[i].includes("|"))
          body.append(
            el(
              "tr",
              {},
              ...cells(lines[i++]).map((c) => el("td", {}, inlineText(c))),
            ),
          );
        table.append(body);
        box.append(el("div", { class: "table-wrap" }, table));
      } else if (/^\s*([-*+] |\d+\. )/.test(line)) {
        const ordered = /^\s*\d+\. /.test(line);
        const list = el(
          ordered ? "ol" : "ul",
          ordered ? { start: line.match(/\d+/)[0] } : {},
        );
        const pattern = ordered ? /^\s*\d+\. (.*)$/ : /^\s*[-*+] (.*)$/;
        while (i < lines.length && pattern.test(lines[i]))
          list.append(
            el("li", {}, inlineText(lines[i++].replace(pattern, "$1"))),
          );
        box.append(list);
      } else if (/^>\s?/.test(line)) {
        const quote = [];
        while (i < lines.length && /^>\s?/.test(lines[i]))
          quote.push(lines[i++].replace(/^>\s?/, ""));
        box.append(el("blockquote", {}, inlineText(quote.join("\n"))));
      } else if (/^\s*([-*_])(?:\s*\1){2,}\s*$/.test(line)) {
        box.append(el("hr"));
        i++;
      } else {
        const paragraph = [lines[i++]];
        while (
          i < lines.length &&
          lines[i].trim() &&
          !/^\s*(#{1,6}\s|```|[-*+] |\d+\. |>)/.test(lines[i]) &&
          !(
            i + 1 < lines.length &&
            lines[i].includes("|") &&
            /^\s*\|?\s*:?-{3,}/.test(lines[i + 1])
          )
        )
          paragraph.push(lines[i++]);
        box.append(el("p", {}, inlineText(paragraph.join("\n"))));
      }
    }
    return box;
  }
  function permissionCard(request, s) {
    const edit = ["Write", "Edit"].includes(request.toolName) && !request.outsideWorkspace;
    const input = request.input || {};
    const card = el(
      "section",
      { class: "permission-card", "aria-label": "Permission requested" },
      request.subagent ? el("p", {class:"hint"}, "Subagent: " + request.subagent.title + " · Approval applies to this conversation") : null,
      el("h3", {}, request.outsideWorkspace ? "Allow access outside this workspace?" : edit ? "Allow file changes?" : "Allow this action?"),
      el(
        "p",
        {},
        edit
          ? (request.toolName === "Write" ? "Create or update " : "Edit ") +
              (input.file_path || "file")
          : input.file_path || input.path || input.description ||
              input.command ||
              [request.toolName, input.server, input.tool]
                .filter(Boolean)
                .join(" · "),
      ),
      el(
        "details",
        {},
        el("summary", {}, edit ? "Review change" : "Review action"),
        el(
          "pre",
          {},
          request.toolName === "Write"
            ? input.content || "(empty file)"
            : JSON.stringify(input, null, 2),
        ),
      ),
      edit
        ? el(
            "p",
            { class: "hint" },
            "Allow file edits applies to this conversation. Shell commands will still ask.",
          )
        : null,
    );
    const decide = async (decision) => {
      const buttons = [...card.querySelectorAll("button")];
      buttons.forEach((b) => (b.disabled = true));
      try {
        const r = await rpc(service(s), "resolveToolPermission", [
          s.sessionId,
          request.requestId,
          decision,
        ]);
        if (r?.error) throw new Error(r.error);
        await refreshSession(s.sessionId);
        renderTranscript();
        updateSendButton();
        const mode = document.querySelector(
          'select[aria-label="Tool permission mode"]',
        );
        if (mode) mode.value = session()?.permissionMode || state.permission;
      } finally {
        buttons.forEach((b) => (b.disabled = false));
      }
    };
    card.append(
      el(
        "div",
        { class: "permission-actions" },
        button(
          "Allow once",
          () => decide("allow_once"),
          edit ? "quiet" : "primary",
        ),
        edit
          ? button(
              "Allow file edits",
              () => decide("allow_file_edits"),
              "primary",
            )
          : null,
        button("Deny", () => decide("deny"), "quiet"),
      ),
    );
    return card;
  }
  function toolActivity(group, active, opened) {
    const failed = group.steps.filter((s) => s.result?.is_error).length;
    const fileOnly = group.steps.every((s) =>
      ["Read", "ListDirectory", "Grep"].includes(s.call?.name),
    );
    const label = fileOnly
      ? active
        ? "Exploring files"
        : "File activity"
      : active
        ? "Working with tools"
        : "Tool activity";
    const node = group.node;
    node.className = "activity-group" + (failed ? " has-failures" : "");
    node.dataset.disclosure = group.key;
    node.open = opened.has(group.key);
    node.append(
      el(
        "summary",
        {},
        icon(fileOnly ? "folder" : "tools", 14),
        el("span", { class: "activity-label" }, label),
        el(
          "span",
          { class: "activity-count" },
          `${group.steps.length} action${group.steps.length === 1 ? "" : "s"}`,
        ),
        failed
          ? el("span", { class: "activity-failed" }, `${failed} failed`)
          : null,
        active
          ? el("span", {
              class: "activity-running",
              "aria-label": "In progress",
            })
          : null,
        el("span", { class: "activity-chevron", "aria-hidden": "true" }, "›"),
      ),
    );
    const rows = el("div", { class: "activity-steps" });
    for (const step of group.steps) {
      const call = step.call,
        result = step.result;
      const bad = !!result?.is_error;
      const input = call?.input || {};
      const path = input.file_path || input.path || sPath();
      const basename = String(path).split(/[\\/]/).filter(Boolean).pop() || ".";
      let title = call?.name || (bad ? "Tool action failed" : "Tool result");
      if (call?.name === "Read") title = "Read " + basename;
      else if (call?.name === "ListDirectory")
        title =
          "List " +
          (path === "."
            ? sPath().split(/[\\/]/).filter(Boolean).pop() || "folder"
            : basename);
      else if (call?.name === "Grep")
        title = "Search " + (input.query || "workspace");
      else if (call?.name === "Bash")
        title = input.description || "Run command";
      else if (["Write", "Edit"].includes(call?.name))
        title = call.name + " " + basename;
      else if (call?.name === "McpCall")
        title =
          [input.server, input.tool].filter(Boolean).join(" · ") ||
          "Connected tool";
      const awaiting = state.approvals
        .get(session()?.sessionId)
        ?.some((r) => r.toolCallId === step.id);
      const status = result
        ? bad
          ? "Failed"
          : "Completed"
        : awaiting
          ? "Awaiting approval"
          : active
            ? "Running…"
            : "No result recorded";
      const key = group.key + "/" + step.id;
      const row = el(
        "details",
        {
          class: "tool-card tool-step" + (bad ? " failed" : ""),
          "data-disclosure": key,
          open: opened.has(key),
        },
        el(
          "summary",
          {},
          icon(bad ? "close" : result ? "check" : "tools", 12),
          el("span", { class: "tool-step-title", title }, title),
          el("small", { class: "tool-step-status" }, status),
        ),
      );
      if (call)
        row.append(
          el(
            "div",
            { class: "tool-step-body" },
            el("small", {}, "Input"),
            el("pre", {}, JSON.stringify(input, null, 2)),
          ),
        );
      if (result)
        row.append(
          el(
            "div",
            { class: "tool-step-body" },
            el("small", {}, bad ? "Error" : "Result"),
            toolResultView(result.content),
          ),
        );
      rows.append(row);
    }
    node.append(rows);
    function sPath() {
      return session()?.cwd || currentFolder() || ".";
    }
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
    const opened = new Set(
      [...target.querySelectorAll("details[open][data-disclosure]")].map(
        (n) => n.dataset.disclosure,
      ),
    );
    const groups = [];
    let group = null,
      turnKey = "initial";
    for (const [messageIndex, message] of messages.entries()) {
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
      if (texts && role === "user") {
        group = null;
        turnKey = id || "message-" + messageIndex;
      }
      if (texts) {
        const article = el(
          "article",
          { class: "message " + role },
          el(
            "div",
            { class: "role" },
            role === "user"
              ? "You"
              : message.message?.stop_reason === "user_stop"
                ? "QuixiDesk · Stopped"
                : "QuixiDesk",
          ),
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
      for (const [toolIndex, tool] of tools.entries()) {
        if (!group) {
          group = {
            key: "activity-" + turnKey,
            node: el("details"),
            steps: [],
            byId: new Map(),
          };
          groups.push(group);
          result.push(group.node);
        }
        const key =
          tool.id ||
          tool.tool_use_id ||
          `unlinked-${messageIndex}-${toolIndex}`;
        let step = group.byId.get(key);
        if (!step) {
          step = { id: key };
          group.byId.set(key, step);
          group.steps.push(step);
        }
        if (tool.type === "tool_use") step.call = tool;
        else step.result = tool;
      }
    }
    for (const activity of groups)
      toolActivity(activity, activity === group && running(s), opened);
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
    const children = state.subagents.get(s.sessionId) || [];
    if (children.length) {
      const panel = el("details", {class:"subagent-panel", "data-disclosure":"subagents", open:opened.has("subagents")},
        el("summary", {}, "Subagents · " + children.length + (children.some(c=>c.status==="running") ? " · Working" : children.some(c=>["failed", "interrupted"].includes(c.status) || c.failedToolCalls > 0) ? " · Needs attention" : children.some(c=>c.status==="cancelled") ? " · Stopped" : " · Complete")));
      for (const child of children) panel.append(el("div", {class:"subagent-row"},
        el("div", {class:"grow"}, el("strong", {}, child.title),
          el("p", {class:"hint"}, [child.status, child.readOnly ? "Read-only" : "Workspace tools", child.model, `${child.toolCalls || 0} tool call${child.toolCalls === 1 ? "" : "s"}`, child.failedToolCalls ? `${child.failedToolCalls} failed` : ""].filter(Boolean).join(" · ")),
          child.error ? el("p", {class:"error-text"}, child.error) : null),
        button("View activity", async()=>{
          const task = await rpc(service(s), "getSubagent", [s.sessionId, child.agentId]);
          if (!task) throw new Error("Subagent activity is unavailable.");
          const body = el("div", {class:"subagent-history"}, el("p", {class:"hint"}, "Assignment and recorded activity"), plainBody(task.prompt || ""));
          for (const message of task.messages || []) {
            if (message.content) body.append(el("details", {}, el("summary", {}, message.role === "tool" ? (message.is_error ? "Tool failed" : "Tool result") : "Subagent response"), el("pre", {}, typeof message.content === "string" ? message.content : JSON.stringify(message.content))));
            for (const call of message.tool_calls || []) body.append(el("details", {}, el("summary", {}, call.function.name), el("pre", {}, call.function.arguments)));
          }
          dialog(task.title, body);
        }, "quiet")));
      result.push(panel);
    }
    for (const request of state.approvals.get(s.sessionId) || [])
      result.push(permissionCard(request, s));
    if (state.stopped.has(s.sessionId) && !running(s))
      result.push(
        el(
          "div",
          { class: "live-status", role: "status" },
          icon("stop", 14),
          "Response stopped.",
        ),
      );
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
    refreshConversationFind(false);
    if (nearBottom) target.scrollTop = target.scrollHeight;
    else target.scrollTop = oldScroll;
  }
  async function sessionMenu(s) {
    s = state.sessions.find((current) => current.sessionId === s.sessionId) || s;
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
          { class: "card project-card" },
          el("div", { class: "card-symbol" }, icon("projects", 21)),
          el("h3", { style: "margin-top:16px" }, p.name),
          el(
            "p",
            {},
            p.description || "A workspace for related conversations.",
          ),
          el(
            "small",
            {},
            `${p.folders?.length || 0} ${p.folders?.length === 1 ? "folder" : "folders"} · ${state.sessions.filter((s) => s.spaceId === p.id).length} ${state.sessions.filter((s) => s.spaceId === p.id).length === 1 ? "conversation" : "conversations"}`,
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
    const folderError = el("p", { class: "form-error", role: "alert" });
    const addFolder = async (path) => {
      const check = await api("/api/quixi/workspace/inspect", { path });
      if (!check.exists) throw new Error("Choose an existing folder.");
      if (p.folders?.some((folder) => folder.path === check.path))
        throw new Error("This folder is already in the project.");
      const added = await rpc("CoworkSpaces", "addFolderToSpace", [p.id, check.path]);
      if (!added) throw new Error("The folder could not be added. Try again.");
      await loadProjects();
      d.close();
      projectDetail(state.projects.find((x) => x.id === p.id));
    };
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
      el("div", { class: "form-actions" },
      button("Add folder", async (event) => {
        const button = event.currentTarget;
        button.disabled = true;
        folderError.textContent = "";
        try {
          const path = await rpc("LocalAgentModeSessions", "browseAndAddTrustedFolder", ["Add a project folder"]);
          if (path) await addFolder(path);
        } catch (error) { folderError.textContent = error.message; }
        finally { button.disabled = false; }
      }, "primary", "folder"),
      button("Enter path", () =>
        formDialog(
          "Add project folder",
          [{ name: "path", label: "Folder path", required: true,
            placeholder: "e.g. ~/Projects/my-project", hint: "Paste a full path or use ~ for your home folder." }],
          async (values) => addFolder(values.path),
          { submit: "Add folder" },
        ),
      "quiet")),
      folderError,
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
  function artifactThumbnail(artifact) {
    const preview = button(
      "",
      () => openArtifact(artifact.id),
      "artifact-thumbnail",
    );
    preview.setAttribute(
      "aria-label",
      "Preview " + (artifact.name || artifact.title || "artifact"),
    );
    if (artifact.html) {
      // Gallery tiles are inert and offline. Only the explicit preview runs scripts.
      const frame = el("iframe", {
        sandbox: "",
        referrerpolicy: "no-referrer",
        loading: "lazy",
        tabindex: -1,
        "aria-hidden": "true",
        title: "Artifact thumbnail",
      });
      frame.srcdoc =
        `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:; form-action 'none'; base-uri 'none'">` +
        artifact.html;
      preview.append(frame);
    } else preview.append(icon("artifacts", 36));
    return preview;
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
          { class: "card artifact-card" },
          artifactThumbnail(a),
          el("h3", { style: "margin-top:16px" }, a.name || a.title),
          el("p", {}, a.description || "Private local artifact"),
          el("small", {}, date(a.updatedAt)),
          el(
            "div",
            { class: "card-actions" },
            button("Open preview", () => openArtifact(a.id)),
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
              "quiet danger",
            ),
          ),
        ),
      );
    host.append(
      state.artifacts.length
        ? cards
        : empty(
            "Your ideas, made tangible",
            "Interactive pages and tools you create in a conversation will appear here.",
            button(
              "Create your first artifact",
              () => {
                navigate("new");
                document.getElementById("prompt").value =
                  "Help me create an interactive HTML artifact. Ask me what I would like to build.";
                saveDraft();
                document.getElementById("prompt").focus();
              },
              "primary",
              "plus",
            ),
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
        "Run repeatable work on a schedule.",
        button("New schedule", () => scheduleEditor(), "primary", "plus"),
      ),
      el(
        "div",
        { class: "notice" },
        "Schedules run while QuixiDesk is open, using this computer’s time zone. Missed times are skipped. Manual-mode tools still ask for approval. Edit and save older definitions to activate scheduling.",
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
            el("small", { class: "schedule-detail" }, t.enabled && t.dispatchVersion === 1 ? "Next: " + new Date(t.nextRunAt).toLocaleString([], {month:"short",day:"numeric",hour:"numeric",minute:"2-digit"}) : t.enabled ? "Save to activate" : "Paused"),
            t.lastRunMessage ? el("small", { class: "schedule-detail" }, t.lastRunMessage) : null,
          ),
          t.lastSessionId ? button("View last run", async()=>{await loadSessions();navigate("chat",t.lastSessionId);},"quiet") : null,
          button("Run now", async () => {
            try {
              const r = await rpc("CoworkScheduledTasks", "runScheduledTask", [t.id]);
              if (r.error) throw new Error(r.error);
              await loadSessions(); navigate("chat", r.sessionId);
            } catch (e) { toast(e.message, true); }
          }),
          button("Edit", () => scheduleEditor(t), "quiet"),
          button(
            t.enabled ? "Pause" : "Enable",
            async () => {
              try {
                const result = await rpc("CoworkScheduledTasks", "updateScheduledTaskStatus", [
                  t.id,
                  t.enabled ? "disabled" : "enabled",
                ]);
                if (result === false || result?.error) throw new Error(result?.error || "Schedule could not be updated. Check that its settings can be saved.");
                await loadSchedules();
              } catch (e) { toast(e.message, true); }
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
          hint: "Five cron fields: minute, hour, day, month, weekday (0–7 or SUN–SAT). Uses this computer’s local time.",
        },
        {
          name: "cwd",
          label: "Workspace folder",
          value: t?.cwd || currentFolder(),
          hint: "Leave blank to use the default workspace. Home shortcuts such as ~/Projects are accepted.",
        },
      ],
      async (v) => {
        if (v.cronExpression.trim().split(/\s+/).length !== 5)
          throw new Error("Enter a five-field cron expression.");
        const result = await rpc(
          "CoworkScheduledTasks",
          t ? "updateScheduledTask" : "createScheduledTask",
          [{ ...v, id: t?.id, enabled: t?.enabled ?? true }],
        );
        if (result.error) throw new Error(result.error);
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
    const skillNames = {
      docx: "Word documents",
      pdf: "PDF editing",
      "pdf-reading": "Read PDFs",
      pptx: "Presentations",
      xlsx: "Spreadsheets",
      "frontend-design": "Interface design",
    };
    for (const skill of toolsData.skills)
      skills.append(
        el(
          "article",
          { class: "card skill-card" },
          el(
            "div",
            { class: "skill-heading" },
            el(
              "span",
              { class: "skill-symbol" },
              icon(skill.name === "frontend-design" ? "code" : "file", 18),
            ),
            el(
              "h3",
              {},
              skillNames[skill.name] || skill.displayName || skill.name,
            ),
          ),
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
                skillNames[skill.skillId] || skill.name,
                (files || []).map((f) =>
                  el(
                    "details",
                    { open: f.path === "SKILL.md" },
                    el("summary", {}, f.path),
                    f.path === "SKILL.md"
                      ? plainBody(
                          (f.content || "").replace(
                            /^---\r?\n[\s\S]*?\r?\n---\r?\n/,
                            "",
                          ),
                        )
                      : el("pre", {}, f.content || ""),
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
    const content = el("section", { class: "settings-content" });
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
        el(
          "p",
          { class: "muted" },
          "Choose a look that feels right for your workspace.",
        ),
        el(
          "div",
          { class: "theme-options", role: "group", "aria-label": "Theme" },
          ...[
            ["light", "Light"],
            ["dark", "Dark"],
            ["system", "System"],
          ].map(([value, label]) => {
            const choice = button(
              [
                el(
                  "span",
                  { class: "theme-swatch " + value, "aria-hidden": "true" },
                  el("span"),
                  el("span"),
                ),
                el(
                  "span",
                  { class: "theme-label" },
                  label,
                  (localStorage.getItem("quixi.theme") || "dark") === value
                    ? icon("check", 14)
                    : null,
                ),
              ],
              () => {
                localStorage.setItem("quixi.theme", value);
                applyTheme();
                renderPage();
              },
              "theme-option" +
                ((localStorage.getItem("quixi.theme") || "dark") === value
                  ? " selected"
                  : ""),
            );
            choice.setAttribute("aria-label", "Use " + value + " theme");
            choice.setAttribute(
              "aria-pressed",
              String((localStorage.getItem("quixi.theme") || "dark") === value),
            );
            return choice;
          }),
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
          "Manual asks before file changes and commands. Allow file edits permits changes for the task; commands still ask. Auto permits tool actions.",
        ),
        el(
          "div",
          { class: "notice" },
          "Language: English. Use ⌘/Ctrl+N for a new task, ⌘/Ctrl+K to search, and ⌘/Ctrl+, for settings.",
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
          const issues = status.issues || [];
          dialog("Conversation storage", [
            el(
              "div",
              { class: "notice" },
              issues.length
                ? `${issues.length} conversation${issues.length === 1 ? " needs" : "s need"} attention. Use the storage notice to retry saving or export a recovery copy.`
                : "No storage problems reported.",
            ),
            ...issues.map((issue) =>
              el(
                "p",
                {},
                issue.recoverySaved
                  ? "A recovery copy is saved for an affected conversation."
                  : "An affected conversation is currently held in memory. Keep the app open until it is saved.",
              ),
            ),
            el(
              "details",
              {},
              el("summary", {}, "Technical details"),
              el("pre", {}, JSON.stringify(status, null, 2)),
            ),
          ]);
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
    const body = el("div", { class: "provider-directory" });
    let query = "";
    const draw = () => {
      body.replaceChildren(
        el(
          "p",
          {},
          "Connect the model services you want to use. Credentials stay in the local provider settings; requests go directly to the selected endpoint.",
        ),
        el(
          "div",
          { class: "toolbar" },
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
        el("input", {
          type: "search",
          spellcheck: false,
          autocapitalize: "off",
          autocorrect: "off",
          value: query,
          class: "provider-filter",
          placeholder: "Find a provider…",
          "aria-label": "Filter providers",
          onInput: (e) => {
            query = e.target.value;
            for (const card of body.querySelectorAll(".provider-card"))
              card.classList.toggle(
                "hidden",
                !card.dataset.name.includes(query.toLowerCase()),
              );
            body
              .querySelector(".provider-empty")
              ?.classList.toggle(
                "hidden",
                !!body.querySelector(".provider-card:not(.hidden)"),
              );
          },
        }),
        el(
          "div",
          { class: "cards" },
          ...state.providers.providers
            .slice()
            .sort((a, b) => Number(b.configured) - Number(a.configured))
            .map((p) =>
              el(
                "article",
                {
                  class:
                    "card provider-card" +
                    (!p.name.toLowerCase().includes(query.toLowerCase())
                      ? " hidden"
                      : "") +
                    (p.id === state.providers.activeProvider
                      ? " selected"
                      : ""),
                  "data-name": p.name.toLowerCase(),
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
                      p.id === state.providers.activeProvider && p.configured
                        ? "Active provider"
                        : p.configured
                          ? "Connected"
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
                    "",
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
        el(
          "p",
          { class: "provider-empty hidden hint", role: "status" },
          "No providers match. Try another name.",
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
      el("p", { class: "hint" }, "Models are loaded from the provider when you save. Choose a model in the chat dropdown; unadvertised models can also be added there."),
    );
    if (p.id === "custom") content.append(field("API format", format));
    const payload = () => ({
      providerId: p.id,
      ...(key.value ? { apiKey: key.value } : {}),
      baseUrl: base.value.trim(),
      apiFormat: format.value,
      refreshModels: true,
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
              ? r.message || "Connection succeeded."
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
            error.textContent = "Saving provider and loading models…";
            const r = await api("/api/quixi/providers/save", payload());
            if (r.error) throw new Error(r.error);
            const r2 = await api("/api/quixi/providers/complete-onboarding", {
              activeProvider: p.id,
              activeModel: r.activeModel || "",
            });
            if (r2.error) throw new Error(r2.error);
            await loadProviders();
            await changed?.();
            d.close();
            toast(r.modelDiscovery?.ok === false
              ? "Provider saved. Could not load its models: " + r.modelDiscovery.message
              : "Provider saved. Models are available in the chat dropdown.", r.modelDiscovery?.ok === false);
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
    if (p.configured && ["custom", "openrouter"].includes(p.id))
      content.append(
        button(
          "Manage models",
          () => {
            d.close();
            modelPicker(p.id);
          },
          "quiet",
        ),
      );
    content.append(error, actions);
    const d = dialog(p.name, content);
    d.addEventListener("close", () => {
      clearInterval(oauthTimer);
      key.value = "";
    });
  }
  async function modelPicker(providerId) {
    await loadProviders();
    let providers = state.providers.providers.filter((p) => p.configured);
    if (!providers.length) {
      providerDirectory();
      return;
    }
    let selected = providerId || state.providers.activeProvider;
    const ps = select(
      providers.map((p) => [p.id, p.name]),
      selected,
      (v) => {
        selected = v;
        query.value = "";
        catalogStatus.textContent = "";
        draw();
      },
      { "aria-label": "Provider" },
    );
    const models = el("div", { class: "model-options" });
    const additions = el("div", { class: "manual-models" });
    const query = el("input", {
      type: "search",
      placeholder: "Find a model…",
      "aria-label": "Filter models",
      onInput: () => draw(),
    });
    const catalogStatus = el("p", { class: "hint", role: "status" });
    const refresh = button("Refresh models", async () => {
      refresh.disabled = true;
      catalogStatus.textContent = "Loading models…";
      try {
        const r = await api("/api/quixi/providers/test", { providerId: selected });
        await loadProviders();
        providers = state.providers.providers.filter((p) => p.configured);
        catalogStatus.textContent = r.message || (r.ok ? "Models loaded." : "Could not load models.");
        draw();
      } catch (e) { catalogStatus.textContent = e.message; }
      finally { refresh.disabled = false; }
    }, "quiet");
    const d = dialog("Choose model", [
      field("Provider", ps),
      query,
      refresh,
      catalogStatus,
      models,
      additions,
    ]);
    function draw() {
      const p = providers.find((x) => x.id === selected) || providers[0];
      models.replaceChildren(
        ...[...(p.models || [])]
          .filter((m) => `${m.id} ${m.label || ""}`.toLowerCase().includes(query.value.trim().toLowerCase()))
          .sort(
            (a, b) =>
              Number(b.id === p.selectedModel) -
                Number(a.id === p.selectedModel) ||
              Number(p.customModels?.includes(b.id)) -
                Number(p.customModels?.includes(a.id)),
          )
          .map((m) =>
            button(
              [
                el(
                  "span",
                  { class: "grow" },
                  m.label || m.id,
                  p.customModels?.includes(m.id)
                    ? el(
                        "small",
                        { class: "hint model-origin" },
                        "Added manually",
                      )
                    : null,
                ),
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
                renderPage();
                toast("Model updated.");
              },
              "list-row",
            ),
          ),
      );
      if (!models.childElementCount)
        models.append(el("p", { class: "hint" }, query.value.trim()
          ? "No matching models. Try another search."
          : "No models loaded yet. Refresh the catalog or add an unadvertised model below."));
      additions.replaceChildren();
      if (["custom", "openrouter"].includes(p.id)) {
        const input = el("input", {
          placeholder: "e.g. organization/model-name",
          "aria-label": "Model ID",
          autocomplete: "off",
          autocapitalize: "off",
          spellcheck: "false",
        });
        const error = el("div", { class: "form-error", role: "alert" });
        const submit = button(
          "Add model",
          async () => {
            const id = input.value.trim();
            if (!id) {
              error.textContent = "Enter a model ID.";
              input.focus();
              return;
            }
            submit.disabled = true;
            try {
              const r = await api("/api/quixi/providers/save", {
                providerId: p.id,
                customModel: id,
                selectCustomModel: false,
                setAsActive: false,
              });
              if (r.error) throw new Error(r.error);
              await loadProviders();
              providers = state.providers.providers.filter((p) => p.configured);
              query.value = "";
              draw();
              toast("Model saved. Select it above when you’re ready.");
            } catch (e) {
              error.textContent = e.message;
            } finally {
              submit.disabled = false;
            }
          },
          "quiet",
        );
        input.addEventListener("keydown", (e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            submit.click();
          }
        });
        additions.append(
          el("hr"),
          el("details", {},
          el("summary", {}, "Add an unadvertised model"),
          el(
            "p",
            { class: "hint" },
            "Use an exact model ID even if this provider doesn’t advertise it. Adding a model keeps your current selection.",
          ),
          field("Model ID", input),
          error,
          submit,
          ),
        );
      }
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
    if (channel.includes("ScheduledTasks")) {
      Promise.all([loadSessions(), loadSchedules()]).catch(e=>toast(e.message,true));
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
    if (command && e.key.toLowerCase() === "f" && state.page === "chat") {
      e.preventDefault(); openConversationFind();
    } else if (command && e.key.toLowerCase() === "g" && conversationFind) {
      e.preventDefault(); stepConversationFind(e.shiftKey ? -1 : 1);
    } else if (command && e.key.toLowerCase() === "n") {
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
