/* Copyright (c) 2026 QuixiAI. MIT license. Original QuixiDesk workspace. */
(() => {
  "use strict";
  const { api, rpc, copy } = window.quixi;
  const root = document.getElementById("app");
  const state = {
    page: "new",
    id: null,
    sessions: [],
    pendingMessages: new Map(),
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
    trash: "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7",
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
    return state.page === "chat" ? state.id : "new:" + (state.project || "personal");
  }
  function saveDraft() {
    const input=document.getElementById("prompt");
    if(!input)return;
    const key=input.dataset.draftKey || draftKey();
    state.attachmentDrafts.set(key,state.attachments.slice());
    state.drafts.set(key,input.value);
    sessionStorage.setItem("quixi.draft."+key,input.value);
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
      session() ? (session().userSelectedFolders?.[0] ?? session().cwd ?? "") : (state.folder || "")
    );
  }
  function dialog(title, body, { wide = false } = {}) {
    const opener = document.activeElement;
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
    d.addEventListener("close", () => { d.remove(); if(opener?.isConnected) opener.focus(); });
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
    let busy = false;
    const d = dialog(title, [
      el("p", {}, text),
      el(
        "div",
        { class: "form-actions" },
        button("Cancel", () => d.close(), "quiet"),
        button(
          label,
          async (event) => {
            if (busy) return;
            busy = true;
            const control = event.currentTarget;
            control.disabled = true;
            try {
              await action();
              d.close();
            } finally {
              busy = false;
              control.disabled = false;
            }
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
      if (saveButton.disabled) return;
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
    capabilityUI?.stopAudio();
    page = String(page).replace(/^\//, "");
    if (page.startsWith("chat/")) {
      id = page.slice(5);
      page = "chat";
    }
    if (page.startsWith("cowork/")) {
      id = page.slice(7);
      page = "chat";
    }
    if (page.startsWith("project/")) { id = decodeURIComponent(page.slice(8)); page = "project"; }
    if (page.startsWith("settings/")) page = "settings";
    const pages = [
      "new",
      "chat",
      "search",
      "projects",
      "project",
      "artifacts",
      "schedules",
      "tools",
      "settings",
    ];
    if (!pages.includes(page)) page = "new";
    if (page !== "tools") clearTimeout(toolsPollTimer);
    state.page = page;
    state.id = id;
    if (page === "project") { state.project=id; state.folder=state.projects.find(p=>p.id===id)?.folders?.[0]?.path||""; }
    if (page === "chat") {
      const opened = session();
      state.project = opened?.spaceId || null;
      state.folder = opened?.userSelectedFolders?.[0] ?? opened?.cwd ?? "";
    }
    state.attachments = state.attachmentDrafts.get(draftKey())?.slice() || [];
    const path =
      page === "chat" || page === "project" ? "/" + page + "/" + encodeURIComponent(id) : "/" + page;
    if (location.pathname !== path)
      history[replace ? "replaceState" : "pushState"]({}, "", path);
    render();
    if (page === "chat") openTranscript().catch((e) => toast(e.message, true));
    else if (page === "projects" || page === "project")
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
            project: project()?.name || "Project",
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
              ib(`Delete conversation: ${s.title || "Untitled task"}`, "trash", () => deleteSession(s)),
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
      search: workspaceUI.searchPage,
      projects: workspaceUI.projectsPage,
      project: workspaceUI.projectPage,
      artifacts: workspaceUI.artifactsPage,
      schedules: schedulesPage,
      tools: capabilityUI.renderTools,
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
            currentFolder().split(/[\\/]/).filter(Boolean).pop() || "No folder selected",
          ),
          project() ? button(project().name, () => navigate("project", project().id), "quiet project-chip", "projects") : null,
          s.forkedFrom ? button("Original conversation",()=>navigate("chat",s.forkedFrom.sessionId),"quiet") : null,
          button("Find in conversation", () => openConversationFind(), "quiet", "search"),
          button("Files & context", () => contextDrawer(), "quiet"),
          ib("Delete conversation", "trash", () => deleteSession(s)),
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
    // Navigation belongs to this composer, never another conversation's draft.
    let history = null, historyIndex = 0, historyDraft = "";
    const resetHistory = () => { history = null; };
    const recallPrompt = (e) => {
      if (!["ArrowUp", "ArrowDown"].includes(e.key) || e.isComposing ||
          e.shiftKey || e.ctrlKey || e.altKey || e.metaKey || ta.disabled ||
          ta.selectionStart !== ta.selectionEnd) return false;
      const up = e.key === "ArrowUp";
      if (up ? ta.value.slice(0, ta.selectionStart).includes("\n")
             : ta.value.slice(ta.selectionEnd).includes("\n")) return false;
      if (!history) {
        if (!up) return false;
        const current = session();
        const messages = current?._kind === "code"
          ? state.transcripts.get(current.sessionId) || []
          : current?.bufferedMessages || [];
        history = messages.filter(m => m.type === "user").map(m =>
          textBlocks(m).filter(b => b.type === "text" && !b.attachment)
            .map(b => b.text || "").join("\n")).filter(text => text.trim());
        history.push(...(state.pendingMessages.get(current?.sessionId) || [])
          .map(m => m.text || "").filter(text => text.trim()));
        historyIndex = history.length;
        historyDraft = ta.value;
      }
      if (!history.length) { resetHistory(); return false; }
      e.preventDefault();
      historyIndex = Math.max(0, Math.min(history.length, historyIndex + (up ? -1 : 1)));
      ta.value = historyIndex === history.length ? historyDraft : history[historyIndex];
      const caret = up ? 0 : ta.value.length;
      ta.setSelectionRange(caret, caret);
      saveDraft();
      return true;
    };
    const ta = el("textarea", {
      id: "prompt",
      "data-draft-key": draftKey(),
      disabled: pendingSubmissions.has(draftKey()),
      placeholder: running(s)
        ? "Write a follow-up while this task runs…"
        : "Write your prompt to QuixiDesk",
      "aria-label": "Write your prompt to QuixiDesk",
      rows: 3,
      value: getDraft(),
      onInput: () => { resetHistory(); saveDraft(); },
      onPaste: (event) => {
        if (pendingSubmissions.has(draftKey())) return;
        const clipboard = event.clipboardData;
        const files = window.QuixiChat.clipboardImages(clipboard);
        if (!files.length) return; // Leave ordinary text paste to the browser.
        event.preventDefault();
        resetHistory();
        const key = draftKey();
        const items = window.QuixiChat.queueFiles(files, () => { if (draftKey() === key) renderChips(); });
        state.attachments.push(...items);
        const text = clipboard?.getData("text/plain");
        if (text) ta.setRangeText(text, ta.selectionStart, ta.selectionEnd, "end");
        saveDraft(); renderChips();
      },
      onKeydown: (e) => {
        if (recallPrompt(e)) return;
        if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
          e.preventDefault();
          sendPrompt("queue");
        }
      },
    });
    const fileInput = el("input", {
      type: "file",
      multiple: true,
      id: "attachment-input",
      disabled: pendingSubmissions.has(draftKey()),
      class: "hidden",
      accept: window.QuixiChat.accept,
      onChange: (e) => {
        const key = draftKey();
        const items = window.QuixiChat.queueFiles(e.target.files, () => {
          if (draftKey() === key) renderChips();
        });
        state.attachments.push(...items);
        saveDraft(); renderChips(); e.target.value = "";
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
    const send = button(running(s) ? "Queue" : "Send", () => sendPrompt(), "primary", "arrow");
    send.id = "send-button";
    const stop = button("Stop", () => stopTask(), "quiet", "stop");
    stop.id = "stop-button";
    stop.title = "Stop this response. Pending messages are kept on hold.";
    stop.hidden = !running(s);
    const steer = button("Guide current task", () => {
      document.getElementById("send-options")?.removeAttribute("open");
      sendPrompt("steer");
    }, "quiet");
    steer.id = "steer-button";
    steer.title = "Apply at the next safe boundary; running tools finish first.";
    const options = el("details", {id:"send-options", class:"send-options"},
      el("summary", {"aria-label":"More send options", title:"More send options"}, "⌄"),
      el("div", {class:"send-options-menu"}, steer));
    options.hidden = !running(s);
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
          ib("Attach images or text files", "plus", () => fileInput.click()),
          capabilityUI.dictationControl(ta),
          ib("Add MCP context", "tools", () => mcpContextPicker()),
          button(
            [
              icon("folder", 14),
              currentFolder()
                ? currentFolder().split(/[\\/]/).filter(Boolean).pop()
                : "Choose folder",
            ],
            () => folderDialog(),
            "workspace-switch",
          ),
          mode,
          el("span", { class: "spacer" }),
          el("span", { class: "model-control" }, chooseModel),
          stop,
          send,
          options,
        ),
      ),
      el("div", {id:"pending-messages", "aria-label":"Pending messages"}, ...pendingMessageRows(s)),
      el("p", { class: "composer-note", id:"composer-note" },
        running(s) ? "Enter to queue · Shift+Enter for a new line" : "Enter to send · Shift+Enter for a new line"),
    );
  }
  function renderChips() {
    const target = document.getElementById("attachment-chips");
    target?.replaceChildren(
      ...state.attachments.map((f, i) =>
        el(
          "span",
          { class: "chip" },
          f.preview ? el("img", {src:f.preview,alt:"",class:"attachment-thumbnail"}) : icon("file", 12),
          f.name,
          f.status === "loading" ? " · Loading…" : null,
          f.status === "error" ? el("span",{role:"alert"},f.error) : null,
          f.status === "error" ? button("Retry",()=>window.QuixiChat.retryAttachment(f,renderChips),"quiet") : null,
          f.mcp?.kind === "resource" ? button(f.mcp.subscribed ? "Stop following" : "Follow changes", () => followMcpResource(f, !f.mcp.subscribed), "quiet") : null,
          f.mcp?.stale ? button("Review update", () => refreshMcpAttachment(f), "quiet mcp-stale-context") : null,
          button(
            "×",
            async () => {
              if (f.mcp?.subscribed) await followMcpResource(f, false);
              window.QuixiChat.cancelAttachment(f);
              const index=state.attachments.indexOf(f);
              if(index>=0)state.attachments.splice(index, 1);
              saveDraft();renderChips();
            },
            "quiet",
          ),
        ),
      ),
    );
  }
  async function folderDialog() {
    const anchor=document.querySelector(".workspace-switch")?.getBoundingClientRect();
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
      browse,
      current ? button("Use without a folder",async()=>{
        const s=session();if(s){const result=await rpc(service(s),"updateSession",[s.sessionId,{userSelectedFolders:[],cwd:"",spaceId:state.project}]);if(result?.error)throw new Error(result.error);await loadSessions();}state.folder="";d.close();renderPage();
      },"quiet") : null,
      field("Or enter a folder path", input),
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
    positionPopover(d,anchor);
  }
  const pendingSubmissions=new Set();
  async function sendPrompt(delivery = "queue") {
    const input = document.getElementById("prompt");
    if (!input) return;
    const text = input.value.trim();
    if (!text && !state.attachments.length) return;
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
    if (state.attachments.some(file => file.mcp?.stale)) { toast("Review the updated MCP context before sending this message.", true); return; }
    const b = document.getElementById("send-button");
    if (b?.disabled || pendingSubmissions.has(draftKey())) return;
    if (b) b.disabled = true;
    const originKey=draftKey();
    const attached=state.attachments.slice();
    pendingSubmissions.add(originKey);
    input.disabled=true;
    try {
      capabilityUI.stopAudio();
      const attachments=window.QuixiChat.attachmentsForSend(attached);
      let s = session();
      if (!s) {
        const opts = {
          title: Array.from(text || attached[0]?.name || "New task").slice(0, 80).join(""),
          permissionMode: state.permission,
          spaceId: state.project,
          userSelectedFolders: currentFolder() ? [currentFolder()] : [],
          cwd: currentFolder() || "",
        };
        s = await rpc("LocalAgentModeSessions", "start", [opts]);
        if(!s?.sessionId)throw new Error(s?.error||"Conversation could not be created.");
        s._kind = "task";
        state.sessions.unshift(s);
      }
      state.stopped.delete(s.sessionId);
      const uuid = crypto.randomUUID();
      const wasRunning = running(s);
      const accepted = await rpc(service(s), "sendMessageV2", [s.sessionId, {text, attachments, messageUuid:uuid, delivery}]);
      if (accepted?.error) throw new Error(accepted.error);
      if (accepted?.queued) toast(delivery === "steer" ? "Steering sent — applies at the next safe boundary." : "Follow-up queued.");
      const stillHere=document.getElementById("prompt") && draftKey()===originKey;
      for (const key of [originKey]) {
        state.drafts.delete(key);state.attachmentDrafts.delete(key);
        sessionStorage.removeItem("quixi.draft."+key);
      }
      for(const file of attached)if(file.mcp?.subscribed)mcpRequest("subscribeResource",[file.mcp.server,file.mcp.uri,file.mcp.cwd,false]).catch(error=>toast(error.message,true));
      if(stillHere){state.attachments=[];document.getElementById("prompt").value="";}
      s.isRunning = true;
      if (!wasRunning) state.streams.set(s.sessionId, { text: "", thinking: "" });
      await refreshSession(s.sessionId);
      if(document.getElementById("prompt") && draftKey()===originKey){pendingSubmissions.delete(originKey);navigate("chat",s.sessionId);document.getElementById("prompt")?.focus();}
    } catch (e) {
      toast(e.message, true);
    } finally {
      pendingSubmissions.delete(originKey);input.disabled=false;
      if(draftKey()===originKey){const current=document.getElementById("prompt");if(current)current.disabled=false;const file=document.getElementById("attachment-input");if(file)file.disabled=false;}
      if (b) b.disabled = false;
    }
  }
  async function forkMessage(s,message,text) {
    if(running(s)){toast("Stop the current task before creating a branch.",true);return;}
    const branch=await rpc(service(s),"forkMessage",[s.sessionId,{messageUuid:message.uuid,...(text!==undefined?{text}: {})}]);
    if(!branch?.sessionId)throw new Error(branch?.error||"Could not create conversation branch.");
    branch._kind=s._kind;state.sessions.unshift(branch);navigate("chat",branch.sessionId);toast("Created a new branch. Your original conversation is preserved.");
  }
  function editMessage(s,message) {
    formDialog("Edit in a new branch",[{name:"text",label:"Message",multiline:true,value:window.QuixiChat.editableText(message),hint:"The original conversation stays intact. Existing attachments are kept.",required:true}],async(values)=>{
      if(!values.text.trim())throw new Error("Enter a message.");await forkMessage(s,message,values.text.trim());
    },{submit:"Send edited message"});
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
  function pendingMessageRows(s) {
    return (state.pendingMessages.get(s?.sessionId) || []).map(m => {
      const act = async (method, control) => {
        control.disabled = true;
        try {
          const result = await rpc(service(s), method, [s.sessionId, m.messageUuid]);
          if (result?.error) throw new Error(result.error);
          if (result?.cancelled === false) toast("This message has already been delivered.");
          await refreshSession(s.sessionId);
          if (session()?.sessionId === s.sessionId) { renderTranscript(); updateSendButton(); }
        } catch (error) { toast(error.message, true); }
        finally { control.disabled = false; }
      };
      const remove = button("Remove", () => act("cancelPendingMessage", remove), "quiet");
      const guide = button(running(s) ? "Guide task" : "Send now", () => act("guidePendingMessage", guide), "quiet");
      guide.title = running(s) ? "Apply at the next safe boundary; running tools finish first." : "Send this held message; other held messages stay on hold.";
      return el("div", {class:"pending-message"},
        el("div", {class:"pending-message-content"},
          el("span", {class:"pending-message-label"}, m.held ? "On hold" : m.delivery === "steer" ? "Guidance pending" : "Up next"),
          el("span", {class:"pending-message-text"}, m.text || "Attachments")),
        ...(m.delivery !== "steer" || m.held || !running(s) ? [guide] : []), remove);
    });
  }
  function updateSendButton() {
    const b = document.getElementById("send-button");
    if (!b) return;
    const active = running(session());
    b.className = "primary";
    b.replaceChildren(icon("arrow"), active ? "Queue" : "Send");
    b.disabled = pendingSubmissions.has(draftKey());
    const stop = document.getElementById("stop-button");
    if (stop) stop.hidden = !active;
    const options = document.getElementById("send-options");
    if (options) { options.hidden = !active; if (!active) options.removeAttribute("open"); }
    const steer = document.getElementById("steer-button");
    if (steer) steer.disabled = b.disabled;
    const note = document.getElementById("composer-note");
    if (note) note.textContent = active ? "Enter to queue · Shift+Enter for a new line" : "Enter to send · Shift+Enter for a new line";
    document.getElementById("pending-messages")?.replaceChildren(...pendingMessageRows(session()));
    const input = document.getElementById("prompt");
    if (input) input.placeholder = active ? "Add a follow-up…" : "Write your prompt to QuixiDesk";
  }
  async function refreshSession(id) {
    let old = state.sessions.find((s) => s.sessionId === id);
    if (!old) { await loadSessions(); old = state.sessions.find((s) => s.sessionId === id); }
    if (!old) return;
    const pending = await rpc(service(old), "getPendingMessages", [id]);
    state.pendingMessages.set(id, Array.isArray(pending) ? pending : []);
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
    text = String(text ?? "");
    // Bound inline parsing work. Unmatched delimiters must not trigger an
    // unanchored regex scan of the entire remaining response at every character.
    const pattern = /^(?:`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\n]+\*|\[[^\]\n]+\]\([^\s)]+\))/;
    const matches = [];
    for (let i = 0; i < text.length;) {
      if (!"`*[".includes(text[i])) { i++; continue; }
      const match = text.slice(i, i + 1024).match(pattern);
      if (!match) { i++; continue; }
      match.index = i; matches.push(match); i += match[0].length;
    }
    let last = 0;
    for (const match of matches) {
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
    const normalized = String(text ?? "").replace(/\r\n?|[\u2028\u2029]/g, "\n");
    // Large responses remain complete and copyable without allocating an
    // unbounded Markdown DOM on every streaming update.
    if (normalized.length > 200000) {
      box.append(el("pre", {class:"plain-response"}, normalized));
      return box;
    }
    const lines = normalized.split("\n");
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
        .filter((b) => b.type === "text" && !b.attachment)
        .map((b) => b.text || "")
        .join("\n");
      const role = message.type === "user" ? "user" : "assistant";
      const tools = blocks.filter(
        (b) => b.type === "tool_use" || b.type === "tool_result",
      );
      if ((texts || window.QuixiChat.transcriptAttachments(message).length) && role === "user") {
        group = null;
        turnKey = id || "message-" + messageIndex;
      }
      const attachedFiles=window.QuixiChat.transcriptAttachments(message);
      if (texts || attachedFiles.length) {
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
          attachedFiles.length ? el("div",{class:"transcript-attachments"},...attachedFiles.map(f=>el("div",{class:"chip"},f.preview?el("img",{src:f.preview,alt:f.name,class:"transcript-image"}):icon("file",14),f.name))) : null,
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
            role === "assistant" ? capabilityUI.readAloudControl(texts) : null,
            role === "user" && id ? button("Edit",()=>editMessage(s,message),"quiet") : null,
            id ? button("Retry",()=>forkMessage(s,message),"quiet") : null,
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
        el("summary", {}, "Subagents · " + children.length + (children.some(c=>c.status==="running") ? " · Working" : children.some(c=>["failed", "interrupted"].includes(c.status)) ? " · Returned to agent" : children.some(c=>c.status==="cancelled") ? " · Stopped" : " · Complete")));
      for (const child of children) panel.append(el("div", {class:"subagent-row"},
        el("div", {class:"grow"}, el("strong", {}, child.title),
          el("p", {class:"hint"}, [["failed", "interrupted"].includes(child.status) ? "Returned to parent agent" : child.status, child.readOnly ? "Read-only" : "Workspace tools", child.model, `${child.toolCalls || 0} tool call${child.toolCalls === 1 ? "" : "s"}`].filter(Boolean).join(" · ")),
          null),
        button("View activity", async()=>{
          const task = await rpc(service(s), "getSubagent", [s.sessionId, child.agentId]);
          if (!task) throw new Error("Subagent activity is unavailable.");
          const body = el("div", {class:"subagent-history"}, el("p", {class:"hint"}, "Assignment and recorded activity"), plainBody(task.prompt || ""));
          if (task.error) body.append(el("p", {class:"hint"}, "Internal diagnostic (handled by the parent agent): " + task.error));
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
  function deleteSession(s) {
    confirmDialog(
      "Delete conversation?",
      `Permanently remove “${s.title || "Untitled task"}” and its stored recovery copy? This cannot be undone.`,
      async () => {
        const result = await rpc(service(s), "delete", [s.sessionId]);
        if (result?.error || result === false) throw Error(result?.error || "Conversation could not be deleted.");
        state.transcripts.delete(s.sessionId);
        state.drafts.delete(s.sessionId);
        state.attachmentDrafts.delete(s.sessionId);
        await loadSessions();
        if (state.id === s.sessionId) navigate("new");
        else renderPage();
        toast("Conversation deleted.");
      },
    );
  }
  function deleteProject(p) {
    confirmDialog("Delete project?",
      `Remove “${p.name}” and its project instructions? Conversations and files in its folders will be kept. This cannot be undone.`,
      async () => {
        const result = await rpc("CoworkSpaces", "deleteSpace", [p.id]);
        if (result?.error || result !== true) throw Error(result?.error || "Project could not be deleted.");
        if (state.project === p.id) state.project = null;
        await loadProjects();
        if (state.page === "project" && state.id === p.id) navigate("projects");
        else renderPage();
        toast("Project deleted. Conversations and files kept.");
      });
  }
  async function sessionMenu(s) {
    s = state.sessions.find((current) => current.sessionId === s.sessionId) || s;
    const d = dialog("Task actions", [
      el("p", {}, s.title),
      el(
        "div",
        { class: "field-stack" },
        button("Prompt diagnostics", async () => {
          try {
            const response = await fetch(`/api/quixi/prompt-context/${encodeURIComponent(s.sessionId)}`);
            const context = await response.json();
            if (!response.ok) throw Error(context.error || "Diagnostics unavailable");
            dialog("Prompt diagnostics", [
              el("p", {class:"hint"}, "Latest tool-enabled model request. Instruction text and tool arguments are not stored here."),
              el("pre", {class:"code-preview"}, JSON.stringify(context, null, 2)),
            ], {wide:true});
          } catch (error) { toast(error.message); }
        }),
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
            deleteSession(s);
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
        const saved = await rpc(
          "CoworkSpaces",
          p ? "updateSpace" : "createSpace",
          p ? [p.id, values] : [values],
        );
        if (saved?.error || !saved?.id) throw Error(saved?.error || "Project could not be saved.");
        await loadProjects();
        navigate("project", saved.id);
      },
    );
  }
  function projectDetail(p) { navigate("project", p.id); }
  function manageProjectFolders(p) {
    const folderError = el("p", { class: "form-error", role: "alert" });
    const addFolder = async (path) => {
      const check = await api("/api/quixi/workspace/inspect", { path });
      if (!check.exists) throw new Error("Choose an existing folder.");
      if (p.folders?.some((folder) => folder.path === check.path))
        throw new Error("This folder is already in the project.");
      const added = await rpc("CoworkSpaces", "addFolderToSpace", [p.id, check.path]);
      if (!added || added.error) throw new Error(added?.error || "The folder could not be added. Try again.");
      await loadProjects();
      d.close();
      manageProjectFolders(state.projects.find((x) => x.id === p.id));
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
              manageProjectFolders(state.projects.find((x) => x.id === p.id));
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
                const result=await rpc("CoworkSpaces", "deleteSpace", [p.id]);
                if(!result || result.error)throw new Error(result?.error || "Project could not be removed.");
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
  async function openArtifact(id) { return workspaceUI.openArtifact(id); }
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
            el("p", {}, window.QuixiSchedules.describe(t)),
            el("small", { class: "schedule-detail" }, window.QuixiSchedules.nextRunLabel(t)),
            el("small", { class: "schedule-detail" }, window.QuixiSchedules.runStatus(t)),
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
  function scheduleEditor(t) { return window.QuixiSchedules.createEditor(workflowContext, t); }
  // Server-originated requests stay outside the transcript until the user acts.
  let mcpInteractionTimer = null, mcpInteractionLoading = false;
  let mcpInteractionRows = [], mcpInteractionDialog = null;
  const mcpInteractionBusy = new Set();
  function safeMcpUrl(value) {
    let url;
    try { url = new URL(value); } catch { throw new Error("The server supplied an invalid URL."); }
    if (!["https:", "http:"].includes(url.protocol) || url.username || url.password) throw new Error("Only HTTP or HTTPS links without embedded credentials can be opened.");
    return url;
  }
  function interactionActive(item) { return item.status !== "completed" && Number(item.expiresAt || Infinity) > Date.now(); }
  async function pollMcpInteractions() {
    if (mcpInteractionLoading) return;
    mcpInteractionLoading = true;
    clearTimeout(mcpInteractionTimer);
    try {
      const result = await rpc("MCP", "pendingInteractions", [], "quixi.settings");
      if (result?.error) throw new Error(result.error);
      if (Array.isArray(result)) mcpInteractionRows = result;
      renderMcpInteractions();
      if (state.attachments.some(f => f.mcp?.subscribed)) await checkMcpResourceUpdates();
    } catch (error) {
      const notice = document.getElementById("mcp-interaction-status");
      if (notice) notice.textContent = "Could not refresh server requests. Retrying…";
    } finally {
      mcpInteractionLoading = false;
      mcpInteractionTimer = setTimeout(pollMcpInteractions, 1500);
    }
  }
  function renderMcpInteractions() {
    let tray = document.getElementById("mcp-interactions");
    const active = mcpInteractionRows.filter(interactionActive);
    const notices=mcpInteractionRows.filter(item=>interactionActive(item)||item.status==="completed");
    if (!tray && notices.length) {
      tray = el("aside", { id: "mcp-interactions", class: "mcp-interactions", "aria-label": "MCP server requests" }, el("p", {id:"mcp-interaction-status", role:"status", class:"hint"}));
      document.body.append(tray);
    }
    if (tray) {
      tray.querySelector("#mcp-interaction-status").textContent = "";
      const existing = new Map(Array.from(tray.querySelectorAll("[data-request]")).map(n => [n.dataset.request,n]));
      for (const request of notices) {
        let row = existing.get(request.id);
        if (!row) {
          row = el("section", { class: "mcp-interaction-notice", "data-request": request.id },
            el("strong", { class: "mcp-request-title" }), el("p", { class: "mcp-request-description" }),
            button("Review request", () => showMcpInteraction(request.id), "primary"));
          tray.append(row);
        }
        existing.delete(request.id);
        row.querySelector("strong").textContent = request.server;
        row.querySelector("button").hidden=request.status==="completed";
        row.querySelector("p").textContent = request.status === "completed" ? "External step completed. Retry your original action when ready." : request.status === "waiting_external" ? "Waiting for the external step to finish." : "Needs your response to continue.";
      }
      for (const row of existing.values()) row.remove();
      if (!notices.length) { tray.remove(); tray = null; }
    }
    if (mcpInteractionDialog) {
      const current = active.find(item => item.id === mcpInteractionDialog.id);
      if (!current) { mcpInteractionDialog.ended = true; mcpInteractionDialog.modal.close(); }
      else mcpInteractionDialog.update(current);
    }
  }
  function interactionFields(schema = {}) {
    const fields = el("div", { class: "mcp-interaction-fields" });
    const controls = [];
    for (const [name, definition] of Object.entries(schema.properties || {})) {
      const required = (schema.required || []).includes(name);
      const label = definition.title || name;
      const choices = definition.enum?.map(value=>({value,label:String(value)})) || definition.oneOf?.map(option=>({value:option.const,label:option.title||String(option.const)}));
      const multiChoices = definition.items?.enum?.map(value=>({value,label:String(value)})) || definition.items?.anyOf?.map(option=>({value:option.const,label:option.title||String(option.const)}));
      let input, read;
      if (definition.type === "boolean") {
        input = el("input", { type: "checkbox", checked: definition.default === true });
        read = () => input.checked;
        fields.append(el("label", { class: "mcp-checkbox" }, input, label));
      } else if (definition.type === "array" && Array.isArray(multiChoices)) {
        const options = multiChoices.map(({value,label}) => ({value,label,control:el("input",{type:"checkbox",checked:definition.default?.includes(value)})}));
        input = el("fieldset", {}, el("legend", {}, label + (required ? " (required)" : "")), options.map(({label,control})=>el("label",{class:"mcp-checkbox"},control,label)));
        read = () => options.filter(option=>option.control.checked).map(option=>option.value);
        fields.append(input);
      } else if (Array.isArray(choices)) {
        const selected = choices.findIndex(option=>option.value===definition.default);
        input = select([["", "Choose…"], ...choices.map((option,index)=>[String(index),option.label])], selected>=0?String(selected):"", null, {required});
        read = () => input.value === "" ? undefined : choices[Number(input.value)].value;
        fields.append(field(label + (required?" (required)":""),input,definition.description));
      } else if (["number", "integer"].includes(definition.type)) {
        input = el("input", { type: "number", step: definition.type === "integer" ? "1" : "any", min: definition.minimum, max: definition.maximum, required, value: definition.default ?? "" });
        read = () => input.value === "" ? undefined : Number(input.value);
        fields.append(field(label + (required?" (required)":""),input,definition.description));
      } else if (definition.type === "string") {
        input = el("input", {type:"text",required,value:definition.default || "",minlength:definition.minLength,maxlength:definition.maxLength,autocomplete:"off"});
        read = () => input.value === "" && !required ? undefined : input.value;
        fields.append(field(label + (required?" (required)":""),input,definition.description));
      } else { throw new Error(`Unsupported form field “${name}”. Decline the request and contact the server owner.`); }
      controls.push({name,definition,input,read,required});
    }
    return {fields, read: () => {
      const values = Object.create(null);
      for (const {name,definition,input,read,required} of controls) {
        const value=read();
        if (input.checkValidity && !input.checkValidity()) throw new Error(`Check the value for ${definition.title||name}.`);
        if (required && (value===undefined || value==="" || (Array.isArray(value)&&!value.length))) throw new Error(`${definition.title||name} is required.`);
        if (typeof value==="number" && (!Number.isFinite(value)||(definition.type==="integer"&&!Number.isInteger(value)))) throw new Error(`${name} must be a valid ${definition.type}.`);
        if (value!==undefined) values[name]=value;
      }
      return values;
    }};
  }
  function showMcpInteraction(id) {
    const request = mcpInteractionRows.find(item => item.id === id && interactionActive(item));
    if (!request) { toast("This server request has ended."); return; }
    if (mcpInteractionDialog?.id === id) { mcpInteractionDialog.modal.focus(); return; }
    if (mcpInteractionDialog) { mcpInteractionDialog.modal.focus(); toast("Finish the open server request first."); return; }
    const error = el("p", {class:"error-text",role:"alert"});
    const status = el("p", {class:"hint",role:"status"});
    let form = null, offeredUrl = null;
    const body = [el("p",{class:"mcp-request-message"},request.message || "The server needs your response.")];
    try {
      if (request.mode === "url") {
        offeredUrl=safeMcpUrl(request.url);
        body.push(el("div",{class:"mcp-external-destination"},el("strong",{},offeredUrl.host),el("p",{class:"path"},offeredUrl.href)),el("p",{class:"hint"},"Opening this page lets this server continue an external interaction. Check the destination before proceeding."));
      } else { form=interactionFields(request.requestedSchema);body.push(form.fields); }
    } catch (problem) { error.textContent=problem.message; }
    const invalidRequest=!!error.textContent;
    const view={id,modal:null,ended:false,answered:false,update:current=>{
      const waiting=current.status==="waiting_external";
      status.textContent=waiting?"Waiting for the server to confirm the external step. You can dismiss this notice; doing so does not cancel the external action.":`Expires ${new Date(current.expiresAt).toLocaleTimeString()}`;
      accept.hidden=waiting;decline.hidden=waiting;cancel.textContent=waiting?"Dismiss request":"Cancel";
    }};
    async function respond(action) {
      if (mcpInteractionBusy.has(id)) return;
      const current=mcpInteractionRows.find(item=>item.id===id&&interactionActive(item));
      if (!current) { error.textContent="This request has ended. No response was sent.";return; }
      try {
        const content=action==="accept" && form?form.read():null;
        if (action==="accept" && !form && !offeredUrl) throw new Error("This request cannot be accepted.");
        mcpInteractionBusy.add(id);view.modal.querySelectorAll("button").forEach(b=>b.disabled=true);error.textContent="";
        const result=await rpc("MCP","respondInteraction",[id,action,content],"quixi.settings");
        if (result?.error || result?.ok!==true) throw new Error(result?.error || "The server did not accept this response.");
        view.answered=true;
        if (action==="accept"&&offeredUrl) {
          const accepted=safeMcpUrl(result.url);
          if (accepted.href!==offeredUrl.href) throw new Error("The server destination changed. Review a new request before opening it.");
          await openExternal(accepted.href);
        }
        view.modal.close();await pollMcpInteractions();
      } catch(problem) {error.textContent=problem.message;if(view.answered){accept.hidden=true;decline.hidden=true;}} finally {mcpInteractionBusy.delete(id);view.modal?.querySelectorAll("button").forEach(b=>b.disabled=false);accept.disabled=invalidRequest;}
    }
    const accept=button(request.mode==="url"?"Open external page":"Send response",()=>respond("accept"),"primary");
    accept.disabled=!!error.textContent;
    const decline=button("Decline",()=>respond("decline"),"quiet");
    const cancel=button("Cancel",()=>respond("cancel"),"quiet");
    body.push(status,error,el("div",{class:"dialog-actions"},cancel,decline,accept));
    const modal=dialog(`Request from ${request.server}`,body);
    modal.classList.add("mcp-interaction-dialog");view.modal=modal;mcpInteractionDialog=view;view.update(request);
    modal.addEventListener("cancel",event=>{if(mcpInteractionBusy.has(id))event.preventDefault();});
    modal.addEventListener("click",event=>{if(event.target===modal&&mcpInteractionBusy.has(id))event.stopImmediatePropagation();},true);
    modal.addEventListener("close",()=>{
      if(mcpInteractionDialog===view)mcpInteractionDialog=null;
      if(!view.ended&&!view.answered&&!mcpInteractionBusy.has(id)&&request.status!=="waiting_external")mcpRequest("respondInteraction",[id,"cancel",null]).then(result=>{if(result?.ok!==true)throw new Error("The server did not confirm cancellation.");return pollMcpInteractions();}).catch(error=>{toast(error.message,true);pollMcpInteractions();});
    });
  }
  async function mcpRequest(member,args=[]) {
    const result=await rpc("MCP",member,args,"quixi.settings");
    if(result?.error)throw new Error(typeof result.error==="string"?result.error:result.error.message);
    return result;
  }
  async function toggleMcpTool(server,tool,enabled) {
    const result=await mcpRequest("setToolEnabled",[server,tool,enabled]);
    if(result?.ok!==true)throw new Error("The tool setting could not be saved.");
    await loadTools({poll:true});
  }
  function mcpText(result,kind) {
    const parts=[];
    const blocks=kind==="prompt"?(result.messages||[]).map(message=>({role:message.role,...message.content})):result.contents||[];
    for(const block of blocks){if(typeof block.text==="string")parts.push(`${block.role?block.role+": ":""}${block.text}`);}
    if(!parts.length)throw new Error("This item has no text content to attach. Binary resources are not supported as text attachments.");
    const text=parts.join("\n\n");
    if(text.includes("\u0000")||new TextEncoder().encode(text).length>1000000)throw new Error("This item exceeds the 1 MB text attachment limit or contains binary data.");
    return text;
  }
  function attachMcpContext(item) {
    const key=draftKey();
    const attachment={id:crypto.randomUUID(),name:item.name,text:item.text,mcp:{server:item.server,uri:item.uri,cwd:item.cwd,subscribed:false,version:item.version||0,kind:item.kind}};
    state.attachments.push(attachment);saveDraft();renderChips();
    toast(`Added ${item.name} to ${key==="new"?"the new task":"this task"}. It is sent only when you send your message.`);
  }
  async function previewMcpContext(server,item,kind,args={},onAttach=()=>{},isCurrent=()=>true) {
    const cwd=currentFolder(), origin=draftKey();
    let uri=item.uri;
    if(kind==="template") {const expanded=await mcpRequest("expandResourceTemplate",[server,item.uriTemplate,args,cwd]);uri=expanded?.uri;if(!uri)throw new Error("The resource template did not produce a URI.");}
    const result=kind==="prompt"?await mcpRequest("getPrompt",[server,item.name,args,cwd]):await mcpRequest("readResource",[server,uri,cwd]);
    if(!isCurrent())return;
    if(draftKey()!==origin)throw new Error("The active conversation changed. Open the context picker again.");
    const text=mcpText(result,kind);
    const name=item.title||item.name||uri;
    const preview=dialog(name,[el("p",{class:"hint"},`${server}${uri?" · "+uri:" · Prompt"}`),el("pre",{class:"code-preview mcp-context-preview"},text),el("p",{class:"hint"},"Review this server's content before adding it. It will become context for your next message."),el("div",{class:"dialog-actions"},button("Add to message",()=>{if(!isCurrent())throw new Error("This context picker was closed. Open it again to add context.");if(draftKey()!==origin)throw new Error("The active conversation changed. Open this context again for the current conversation.");attachMcpContext({name:`${server}: ${name}`,server,uri,cwd,text:`Source: ${server}${uri?" ("+uri+")":""}\n\n${text}`,kind:kind==="prompt"?"prompt":"resource"});preview.close();onAttach();},"primary"))],{wide:true});
  }
  async function mcpContextPicker(initialServer=null) {
    const list=await rpc("LocalAgentModeSessions","getLocalMcpServers");
    if(!Array.isArray(list))throw new Error("MCP servers could not be loaded.");
    const servers=list.filter(s=>!s.builtin);
    const options=[["","Choose a server"],...servers.map(s=>[s.name,s.name])];
    const selector=select(options,initialServer||"",()=>load(),{"aria-label":"MCP context server"});
    const search=el("input",{type:"search",placeholder:"Search resources and prompts","aria-label":"Search MCP context"});
    const status=el("p",{class:"hint",role:"status"}),error=el("p",{class:"error-text",role:"alert"}),results=el("div",{class:"mcp-context-results"});
    const refresh=button("Refresh",()=>load(),"quiet");
    let catalog=null,generation=0,closed=false,contextLimit=50;
    function render(){
      results.replaceChildren();if(!catalog)return;
      const query=search.value.trim().toLowerCase();
      for(const [label,kind,items] of [["Resources","resource",catalog.resources],["Resource templates","template",catalog.resourceTemplates],["Prompts","prompt",catalog.prompts]]){
        const matches=(items||[]).filter(item=>`${item.name||""} ${item.title||""} ${item.description||""} ${item.uri||item.uriTemplate||""}`.toLowerCase().includes(query));
        if(!matches.length)continue;
        results.append(el("h3",{},label));
        for(const item of matches.slice(0,contextLimit)){
          const title=item.title||item.name||item.uri||item.uriTemplate, server=selector.value;
          results.append(el("article",{class:"mcp-context-item"},el("div",{class:"grow"},el("strong",{},title),item.description?el("p",{class:"muted"},item.description):null),button("Preview",async()=>{
            const fields=kind==="prompt"?(item.arguments||[]).map(arg=>({name:arg.name,label:arg.title||arg.name,required:arg.required,hint:arg.description})):kind==="template"?Array.from(new Set(Array.from(item.uriTemplate.matchAll(/\{([^}]+)\}/g)).flatMap(match=>match[1].replace(/^[+#./;?&]/,"").split(",").map(v=>v.replace(/[:*].*$/,""))))).map(name=>({name,label:name,required:true})):[];
            if(fields.length)formDialog(title,fields,args=>previewMcpContext(server,item,kind,args,()=>modal.close(),()=>!closed),{submit:"Preview"});
            else await previewMcpContext(server,item,kind,{},()=>modal.close(),()=>!closed);
          },"quiet")));
        }
        if(matches.length>contextLimit)results.append(button(`Show more ${label.toLowerCase()} (${matches.length-contextLimit} remaining)`,()=>{contextLimit+=50;render();},"quiet"));
      }
      if(!results.childElementCount)results.append(el("p",{class:"muted"},query?"No context matches this search.":"This server has no resources, templates, or prompts."));
    }
    async function load(){
      const mine=++generation;catalog=null;contextLimit=50;results.replaceChildren();error.textContent="";
      if(!selector.value){status.textContent=servers.length?"Choose a server to browse its context.":"Configure an MCP server in Tools & skills to add external context.";return;}
      status.textContent="Loading resources and prompts…";refresh.disabled=true;
      try {const result=await mcpRequest("browseResources",[selector.value,currentFolder()]);if(mine!==generation||closed)return;catalog=result;status.textContent="Choose an item to preview before adding it to your message.";render();}
      catch(problem){if(mine===generation&&!closed){error.textContent=problem.message;status.textContent="Context is unavailable. Fix the connection and retry.";}}
      finally{if(mine===generation)refresh.disabled=false;}
    }
    search.addEventListener("input",()=>{contextLimit=50;render();});
    const modal=dialog("Add MCP context",[field("Server",selector),el("div",{class:"mcp-context-toolbar"},search,refresh),status,error,results],{wide:true});
    modal.addEventListener("close",()=>{closed=true;generation++;});
    await load();
  }
  async function checkMcpResourceUpdates(){
    const updates=await mcpRequest("resourceUpdates");if(!Array.isArray(updates))return;
    let changed=false;
    for(const file of state.attachments){if(!file.mcp?.subscribed)continue;const update=updates.find(u=>u.server===file.mcp.server&&u.uri===file.mcp.uri&&(!u.cwd||u.cwd===file.mcp.cwd));
      if(update&&Number(update.version||update.updatedAt||0)>Number(file.mcp.version||0)){file.mcp.availableVersion=Number(update.version||update.updatedAt);file.mcp.stale=true;changed=true;}}
    if(changed){renderChips();saveDraft();}
  }
  async function followMcpResource(file,enabled){
    const result=await mcpRequest("subscribeResource",[file.mcp.server,file.mcp.uri,file.mcp.cwd,enabled]);
    if(result?.ok!==true)throw new Error("Resource subscription was not accepted.");file.mcp.subscribed=enabled;saveDraft();renderChips();
  }
  async function refreshMcpAttachment(file){
    const version=file.mcp.availableVersion||file.mcp.version;
    const result=await mcpRequest("readResource",[file.mcp.server,file.mcp.uri,file.mcp.cwd]);
    const text=mcpText(result,"resource");
    const preview=dialog("Review updated context",[el("p",{class:"hint"},`${file.mcp.server} · ${file.mcp.uri}`),el("pre",{class:"code-preview mcp-context-preview"},text),el("div",{class:"dialog-actions"},button("Use this version",()=>{file.text=`Source: ${file.mcp.server} (${file.mcp.uri})\n\n${text}`;file.mcp.version=version;file.mcp.stale=(file.mcp.availableVersion||0)>version;saveDraft();renderChips();preview.close();},"primary"))],{wide:true});
  }
  async function mcpRootsEditor(name){
    const cwd=currentFolder();
    const catalog=await mcpRequest("browseResources",[name,cwd]);
    const paths=(catalog.roots||[]).map(root=>root.path||(()=>{try{return decodeURIComponent(new URL(root.uri).pathname);}catch{return "";}})()).filter(Boolean);
    const list=el("div",{class:"mcp-root-list"}),error=el("p",{class:"error-text",role:"alert"});
    const render=()=>list.replaceChildren(...paths.map((path,index)=>el("div",{class:"row"},el("span",{class:"path grow"},path),button("Remove",()=>{paths.splice(index,1);render();},"quiet"))));
    render();
    const modal=dialog(`Folders shared with ${name}`,[el("p",{class:"hint"},"These folders are advertised to this server as workspace roots. Changing roots does not grant permission to execute tools."),list,button("Add folder",async()=>{const path=await rpc("FileSystem","browseFolder",["Choose a folder for this MCP server"]);const value=typeof path==="string"?path:path?.path;if(value&&!paths.includes(value)){paths.push(value);render();}},"quiet"),error,el("div",{class:"dialog-actions"},button("Save folders",async()=>{try{const result=await mcpRequest("setRoots",[name,cwd,paths]);if(result?.ok!==true)throw new Error("Folders could not be updated.");modal.close();await loadTools({poll:true});}catch(problem){error.textContent=problem.message;}},"primary"))]);
  }
  let toolsData = { skills: [], servers: [], error: "" };
  let toolsPollTimer = null;
  let toolsLoadInFlight = null;
  let mcpCatalogDialog = null;
  const mcpBusy = new Set();
  const mcpAuthUrls = new Map();
  const mcpAuthRequested = new Set();
  const mcpAuthOpened = new Set();
  const mcpStatusLabels = {
    configured: "Not connected", starting: "Starting", initializing: "Initializing",
    discovering: "Discovering tools", ready: "Ready", refreshing: "Refreshing tools",
    failed: "Failed", disconnected: "Disconnected", disabled: "Disabled", needs_auth: "Sign-in required", authenticating: "Waiting for sign-in",
  };
  const mcpPending = (status) => ["starting", "initializing", "discovering", "refreshing"].includes(status);
  const mcpError = (server) => typeof server.error === "string" ? server.error : server.error?.message || "";
  async function loadTools({ poll = false } = {}) {
    if (toolsLoadInFlight) return toolsLoadInFlight;
    clearTimeout(toolsPollTimer);
    toolsLoadInFlight = (async () => {
      try {
        const [skills, servers] = await Promise.all([
          poll ? Promise.resolve(toolsData.skills) : rpc("LocalAgentModeSessions", "listLocalSkills"),
          rpc("LocalAgentModeSessions", "getLocalMcpServers"),
        ]);
        if (!Array.isArray(servers)) throw new Error("MCP connection status is unavailable. Retry or check your configuration.");
        toolsData = { skills: Array.isArray(skills) ? skills : [], servers, error: "" };
        if (state.page === "tools") {
          if (poll) updateMcpConnections();
          else renderPage();
        }
        mcpCatalogDialog?.update();
      } catch (e) {
        toolsData.error = e.message;
        if (state.page === "tools") updateMcpConnections();
        if (!poll) toast(e.message, true);
      } finally {
        toolsLoadInFlight = null;
        if (state.page === "tools") toolsPollTimer = setTimeout(() => loadTools({ poll: true }), 2000);
      }
    })();
    return toolsLoadInFlight;
  }
  async function mcpAction(name, action) {
    if (mcpBusy.has(name)) return;
    mcpBusy.add(name);
    updateMcpConnections();
    try {
      const result = await rpc("MCP", action, [name, currentFolder()], "quixi.settings");
      if (result?.error) throw new Error(typeof result.error === "string" ? result.error : result.error.message);
      if (action === "authenticate") {
        if (result?.status !== "authenticating" && typeof result?.url !== "string") throw new Error("The server did not start sign-in.");
        mcpAuthRequested.add(name);
        if (typeof result?.url === "string") mcpAuthUrls.set(name, result.url);
        await loadTools({ poll: true });
      } else {
        if (result?.ok !== true) throw new Error("The connection action was not accepted.");
        if (["cancelAuthentication", "forgetAuthentication"].includes(action)) { mcpAuthUrls.delete(name); mcpAuthRequested.delete(name); }
        await loadTools({ poll: true });
      }
    } finally {
      mcpBusy.delete(name);
      updateMcpConnections();
    }
  }
  function mcpConnectionRow(server) {
    const row = el("article", { class: "mcp-connection", "data-server": server.name });
    const controls = el("div", { class: "mcp-controls" });
    for (const [action, label] of [["connect", "Connect"], ["reconnect", "Retry"], ["refresh", "Refresh tools"], ["disconnect", "Disconnect"], ["authenticate", "Sign in"], ["cancelAuthentication", "Cancel sign-in"]]) {
      const control = button(label, () => mcpAction(server.name, action), "quiet");
      control.dataset.mcpAction = action;
      control.setAttribute("aria-label", `${label} ${server.name}`);
      controls.append(control);
    }
    const forget = button("Forget sign-in", () => confirmDialog("Forget sign-in?", `Disconnect “${server.name}” and remove its saved sign-in credentials? You can sign in again later.`, () => mcpAction(server.name, "forgetAuthentication"), "Forget sign-in"), "quiet");
    forget.dataset.mcpAction = "forgetAuthentication";
    forget.setAttribute("aria-label", `Forget sign-in for ${server.name}`);
    controls.append(forget);
    const browser = button("Open sign-in page", () => openExternal(mcpAuthUrls.get(server.name)), "quiet");
    browser.dataset.mcpAction = "openAuth";
    browser.setAttribute("aria-label", `Open sign-in page for ${server.name}`);
    controls.append(browser);
    const catalog = button("View tools", () => showMcpCatalog(server.name), "quiet");
    catalog.dataset.mcpAction = "catalog";
    catalog.setAttribute("aria-label", `View tools for ${server.name}`);
    controls.append(catalog);
    const context=button("Browse context",()=>mcpContextPicker(server.name),"quiet");context.dataset.mcpAction="context";context.setAttribute("aria-label",`Browse context for ${server.name}`);controls.append(context);
    const roots=button("Workspace folders",()=>mcpRootsEditor(server.name),"quiet");roots.dataset.mcpAction="roots";roots.setAttribute("aria-label",`Workspace folders for ${server.name}`);controls.append(roots);
    row.append(
      el("div", { class: "mcp-heading" }, icon("tools"), el("h3", { class: "grow" }, server.name), el("span", { class: "tag mcp-state", role: "status" })),
      el("p", { class: "muted mcp-catalog-count" }),
      el("p", { class: "hint mcp-last-success" }),
      el("p", { class: "hint mcp-retry-status", role: "status" }),
      el("p", { class: "hint mcp-auth-hint", role: "status" }),
      el("p", { class: "error-text mcp-error", role: "alert" }), controls,
      el("details", { class: "mcp-diagnostics" }, el("summary", {}, "Connection details"), el("pre", { class: "mcp-diagnostics-content" })),
    );
    return row;
  }
  function updateMcpConnections() {
    const list = document.getElementById("mcp-connections");
    if (!list) return;
    const error = document.getElementById("mcp-load-error");
    if (error) { error.textContent = toolsData.error; error.hidden = !toolsData.error; }
    const current = new Map(Array.from(list.children).map((row) => [row.dataset.server, row]));
    for (const server of toolsData.servers) {
      let row = current.get(server.name);
      if (!row) { row = mcpConnectionRow(server); list.append(row); }
      current.delete(server.name);
      const status = server.status || "configured";
      if (["ready", "failed", "disabled", "disconnected", "needs_auth"].includes(status)) {
        mcpAuthUrls.delete(server.name); mcpAuthRequested.delete(server.name);
      }
      if (status === "authenticating" && typeof server.authorizationUrl === "string" && server.authorizationUrl) mcpAuthUrls.set(server.name, server.authorizationUrl);
      const authUrl = mcpAuthUrls.get(server.name);
      if (status === "authenticating" && authUrl && mcpAuthRequested.has(server.name)) {
        const attempt = `${server.name}:${server.authAttempt || authUrl}`;
        if (!mcpAuthOpened.has(attempt)) {
          mcpAuthOpened.add(attempt);
          openExternal(authUrl).catch(error => toast(error.message, true));
        }
      }
      row.dataset.status = status;
      row.querySelector(".mcp-state").textContent = mcpStatusLabels[status] || "Unavailable";
      const count = Array.isArray(server.tools) ? server.tools.length : 0;
      const stale = server.catalogStale || (server.toolsEnumerated && !server.isConnected);
      row.querySelector(".mcp-catalog-count").textContent = server.toolsEnumerated
        ? `${count} ${count === 1 ? "tool" : "tools"}${stale ? " in last-known catalog · connection not verified" : " discovered"}`
        : (mcpPending(status) ? "Waiting for server initialization and tool discovery." : "Tool discovery has not completed.");
      const authHint = row.querySelector(".mcp-auth-hint");
      authHint.textContent = status === "authenticating" ? (authUrl ? "Complete sign-in in your browser. This connection will update automatically." : "Preparing secure sign-in… Your browser will open when the server is ready.") : "";
      authHint.hidden = !authHint.textContent;
      const last = row.querySelector(".mcp-last-success");
      const timestamp = server.lastSuccessAt;
      const date = timestamp ? new Date(timestamp) : null;
      last.textContent = date && !Number.isNaN(date.getTime()) ? "Last discovery: " + date.toLocaleString() : "";
      last.hidden = !last.textContent;
      const retry=row.querySelector(".mcp-retry-status");
      retry.textContent=server.circuitOpen?"Automatic retries paused after repeated failures. Reconnect when ready.":Number(server.nextRetryAt)>Date.now()?`Retrying in ${Math.max(1,Math.ceil((server.nextRetryAt-Date.now())/1000))}s · attempt ${Math.min(3,Number(server.retryAttempt||0)+1)}`:"";
      retry.hidden=!retry.textContent;
      const problem = row.querySelector(".mcp-error");
      const rawError=mcpError(server);
      problem.textContent = status==="needs_auth" && /(?:auth(?:orization)? required|HTTP 401)/i.test(rawError) ? "This server requires sign-in before its tools can be used." : rawError;
      problem.hidden = !problem.textContent;
      const diagnostics = row.querySelector(".mcp-diagnostics-content");
      diagnostics.textContent = [
        server.builtin ? "Built into QuixiDesk" : (server.transport ? `Transport: ${server.transport}` : ""),
        server.cwd ? `Workspace: ${server.cwd}` : "",
        server.serverInfo?.name ? `Server: ${server.serverInfo.name}${server.serverInfo.version ? " " + server.serverInfo.version : ""}` : "",
        server.error?.kind ? `Failure: ${server.error.kind}` : "",
        rawError ? `Error: ${rawError}` : "",
        server.generation != null ? `Catalog generation: ${server.generation}` : "",
        server.stderr ? `Recent server output:\n${server.stderr}` : "",
      ].filter(Boolean).join("\n");
      row.querySelector(".mcp-diagnostics").hidden = !diagnostics.textContent;
      for (const control of row.querySelectorAll("[data-mcp-action]")) {
        const action = control.dataset.mcpAction;
        if(action==="reconnect"){control.textContent=server.circuitOpen?"Reconnect":"Retry";control.setAttribute("aria-label",`${server.circuitOpen?"Reconnect":"Retry"} ${server.name}`);}
        control.hidden = server.builtin && action !== "catalog" ? true
          : server.error?.kind === "configuration" && action !== "catalog" ? true
          : action === "context" ? status !== "ready"
          : action === "roots" ? status !== "ready"
          : action === "connect" ? server.circuitOpen || !["configured", "disconnected"].includes(status)
          : action === "reconnect" ? status !== "failed" && !server.circuitOpen
          : action === "authenticate" ? status !== "needs_auth"
          : action === "cancelAuthentication" ? status !== "authenticating"
          : action === "openAuth" ? status !== "authenticating" || !mcpAuthUrls.has(server.name)
          : action === "forgetAuthentication" ? !server.hasAuth
          : action === "refresh" ? status !== "ready"
          : action === "disconnect" ? !(server.isConnected || mcpPending(status))
          : false;
        control.disabled = mcpBusy.has(server.name) || (action === "catalog" && count === 0);
      }
    }
    for (const row of current.values()) row.remove();
    if (!toolsData.servers.length) list.append(el("p", { class: "muted", "data-server": "" }, "No MCP servers are configured."));
    capabilityUI?.enhanceConnections();
  }
  function showMcpCatalog(name) {
    const search = el("input", { type: "search", placeholder: "Search tool names and descriptions", "aria-label": `Search tools for ${name}` });
    const status = el("p", { class: "hint", role: "status" });
    const results = el("div", { class: "mcp-tool-results" });
    let limit = 50, signature = "";
    const rememberedOpen = new Set();
    const render = () => {
      const server = toolsData.servers.find((server) => server.name === name);
      if (!server) { status.textContent = "This connection was removed."; results.replaceChildren(); return; }
      for(const node of results.querySelectorAll("details")){if(node.open)rememberedOpen.add(node.dataset.tool);else rememberedOpen.delete(node.dataset.tool);}
      const open = rememberedOpen;
      const query = search.value.trim().toLowerCase();
      const tools = (server.tools || []).filter((tool) => `${tool.name} ${tool.description || ""}`.toLowerCase().includes(query));
      status.textContent = `${mcpStatusLabels[server.status] || "Not connected"} · ${tools.length} matching ${tools.length === 1 ? "tool" : "tools"}${server.catalogStale || !server.isConnected ? " · Last-known catalog; execution may be unavailable" : ""}`;
      results.replaceChildren();
      for (const tool of tools.slice(0, limit)) {
        results.append(el("details", { class: "mcp-tool", "data-tool": tool.name, open: open.has(tool.name) },
          el("summary", {}, tool.name), tool.description ? el("p", {}, tool.description) : null,
          !server.builtin ? el("div",{class:"mcp-tool-policy"},el("label",{class:"mcp-checkbox"},el("input",{type:"checkbox",checked:tool.enabled!==false,"aria-label":`Enable ${tool.name} on ${name}`,onChange:async event=>{const control=event.target;control.disabled=true;try{await toggleMcpTool(name,tool.name,control.checked);}catch(problem){control.checked=!control.checked;toast(problem.message,true);}finally{control.disabled=false;}}}),"Enabled"),el("span",{class:"hint"},tool.enabled===false?"Disabled · unavailable to the model":tool.modelVisible===false||(Array.isArray(tool._meta?.ui?.visibility)&&tool._meta.ui.visibility.every(value=>value!=="model"))?"App only · hidden from the model":"Available to the model")) : null,
          el("pre", { class: "mcp-schema" }, JSON.stringify(tool, null, 2))));
      }
      if (!tools.length) results.append(el("p", { class: "muted" }, "No tools match this search."));
      if (tools.length > limit) results.append(button(`Show more (${tools.length - limit} remaining)`, () => { limit += 50; render(); }, "quiet"));
    };
    search.addEventListener("input", () => { limit = 50; render(); });
    const modal = dialog(`${name} tools`, [search, status, results], { wide: true });
    const view = { update: () => {
      const server = toolsData.servers.find((s) => s.name === name);
      const next = JSON.stringify(server);
      if (next !== signature) { signature = next; render(); }
    } };
    mcpCatalogDialog = view;
    modal.addEventListener("close", () => { if (mcpCatalogDialog === view) mcpCatalogDialog = null; });
    view.update();
  }
  async function mcpEditor() {
    const config = await rpc(
      "MCP",
      "getMcpServersConfig",
      [],
      "quixi.settings",
    );
    if (config?.error) throw new Error(typeof config.error === "string" ? config.error : config.error.message);
    formDialog(
      "MCP configuration",
      [
        {
          name: "json",
          label: "Server configuration (JSON)",
          value: JSON.stringify({ mcpServers: config }, null, 2),
          multiline: true,
          attrs: { rows: 15 },
          hint: "Use command/args/env for local servers, or transport: http (or sse), url and optional headers for remote servers. External commands run with your account permissions.",
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
        // The host validates changed entries independently, preserving an existing
        // invalid entry while the user repairs another connection.
        const result = await rpc("MCP", "setMcpServerConfigs", [parsed.mcpServers], "quixi.settings");
        if (result?.error) throw new Error(typeof result.error === "string" ? result.error : result.error.message);
        if (result?.ok !== true) throw new Error("MCP configuration could not be saved.");
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
      capabilityUI.renderDesktopStatus(content);
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
    const anchor=document.querySelector(".model-picker-trigger")?.getBoundingClientRect();
    const d = dialog("Choose model", [
      field("Provider", ps),
      query,
      refresh,
      catalogStatus,
      models,
      additions,
    ]);
    positionPopover(d, anchor);
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
  let streamFrame = null;
  function scheduleStreamRender() {
    if (streamFrame !== null) return;
    streamFrame = requestAnimationFrame(() => {
      streamFrame = null;
      if (state.page === "chat") renderTranscript();
    });
  }
  addEventListener("quixi:event", ({ detail: { channel, payload } }) => {
    if (channel === "quixi:open-tools") { navigate("tools"); return; }
    if (channel === "quixi:mcp-interaction") { pollMcpInteractions(); return; }
    if (channel === "quixi:mcp-updated") {
      if (state.page === "tools") loadTools({ poll: true });
      return;
    }
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
      if (state.id === id) scheduleStreamRender();
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
      state.folder = ""; // A new conversation has no implicit access to the source checkout.
      const path = decodeURIComponent(location.pathname.slice(1));
      navigate(path || "new", null, { replace: true });
      pollMcpInteractions();
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
      saveDraft();state.project=null;state.folder="";navigate("new");
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
  function positionPopover(d, anchor) {
    d.classList.add("workflow-popover");
    function place() {
      const width=Math.min(450,innerWidth-32),height=d.getBoundingClientRect().height;
      d.style.left=Math.max(16,Math.min(anchor?.left ?? innerWidth/2-width/2,innerWidth-width-16))+"px";
      d.style.top=Math.max(16,Math.min(anchor?anchor.top-height-10:80,innerHeight-height-16))+"px";
    }
    place();requestAnimationFrame(place);
    const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(place):null;
    observer?.observe(d);window.addEventListener("resize",place);
    d.addEventListener("close",()=>{observer?.disconnect();window.removeEventListener("resize",place);},{once:true});
  }
  const workflowContext={deleteSession,deleteProject,state,el,button,ib,icon,select,field,dialog,formDialog,confirmDialog,toast,api,rpc,copy,navigate,header,empty,date,currentFolder,loadSchedules,loadProjects,loadSessions,loadArtifacts,render,renderPage,saveDraft,renderChips,sessionMenu,projectEditor,manageProjectFolders,scheduleEditor,artifactThumbnail};
  const workspaceUI=window.QuixiWorkspace.create(workflowContext);
  const capabilityUI=window.QuixiCapabilityWorkflows.create({...workflowContext,plainBody,openExternal,getToolsData:()=>toolsData,reloadTools:()=>loadTools(),updateMcpConnections,advancedMcpEditor:()=>mcpEditor(),
    startSkill:async(skill,files)=>{saveDraft();navigate("new");state.attachments.push(...files.map(f=>({name:f.name||f.path||"SKILL.md",text:f.content||f.contents||""})));const input=document.getElementById("prompt");input.value=`Use the attached ${skill.name||skill.id} instructions to help me with: `+(input.value.trim()?"\n\n"+input.value:"");saveDraft();renderChips();input.focus();}});
  boot();
})();
