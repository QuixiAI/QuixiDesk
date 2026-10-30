// Copyright (c) 2026 QuixiAI. MIT license.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { JSDOM } = require("jsdom");
const fs = require("node:fs");
const path = require("node:path");
const source = fs.readFileSync(
  path.join(__dirname, "../../resources/ui/app.js"),
  "utf8",
);
const wait = () => new Promise((resolve) => setTimeout(resolve, 40));

async function fixture({
  artifacts = [],
  subagents = [],
  mcpServers = [],
  mcpConfig = {},
  mcpActionError = null,
  mcpSaveError = null,
  mcpAuthUrl = "https://example.invalid/oauth/authorize?state=fixture",
  mcpAuthDelayed = false,
  providerId = "custom",
  permissions = [],
  schedules = [],
  catalogModels = [],
  pickedFolder = null,
  folderPaths = {},
} = {}) {
  const dom = new JSDOM(
    '<!doctype html><div id="app"></div><div id="toasts"></div>',
    {
      url: "https://app.localhost/new",
      runScripts: "outside-only",
      pretendToBeVisual: true,
    },
  );
  const w = dom.window;
  w.fetch = async () => ({
    text: async () => "<p>Isolated browser fixture</p>",
  });
  w.matchMedia = () => ({ matches: true, addEventListener() {} });
  w.HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  w.HTMLDialogElement.prototype.close = function () {
    this.open = false;
    this.dispatchEvent(new w.Event("close"));
  };
  const sessions = [
    {
      sessionId: "fixture-one",
      title: "Existing conversation",
      isRunning: false,
      isArchived: false,
      isStarred: false,
      cwd: "/workspace",
      bufferedMessages: [],
      lastActivityAt: 100,
      permissionMode: "default",
    },
  ];
  const calls = [];
  const projects = [{id:"project-a",name:"Project A",instructions:"PRIVATE-PROJECT-A",folders:[{path:"/project-a"}]}];
  const providers = {
    configuredCount: 1,
    hasConfiguredProvider: true,
    activeProvider: providerId,
    activeModel: "fixture-model",
    providers: [
      {
        id: providerId,
        name: "Custom endpoint",
        configured: true,
        selectedModel: "fixture-model",
        baseUrl: "http://127.0.0.1:18775/v1",
        apiFormat: "openai",
        models: [{ id: "fixture-model", label: "Fixture model" }],
      },
    ],
  };
  const rpc = async (service, member, args = []) => {
    calls.push({ service, member, args });
    if (member === "getAll")
      return service === "LocalSessions" ? [] : structuredClone(sessions);
    if (member === "getAllSpaces") return structuredClone(projects);
    if (member === "browseAndAddTrustedFolder") return pickedFolder;
    if (member === "addFolderToSpace") {
      const project = projects.find((p) => p.id === args[0]);
      project.folders.push({path:args[1]});
      return structuredClone(project);
    }
    if (member === "getDefaultWorkspaceFolders") return ["/workspace"];
    if (member === "getSession")
      return structuredClone(sessions.find((s) => s.sessionId === args[0]));
    if (member === "start") {
      const s = {
        ...sessions[0],
        sessionId: "fixture-new",
        title: args[0].title,
        spaceId: args[0].spaceId,
        cwd: args[0].cwd || "/workspace",
        bufferedMessages: [],
      };
      sessions.push(s);
      return structuredClone(s);
    }
    if (member === "listSubagents") return structuredClone(subagents.map(({messages,prompt,...row}) => row));
    if (member === "getSubagent") return structuredClone(subagents.find(c=>c.agentId===args[1]));
    if (member === "getToolPermissions") return structuredClone(permissions);
    if (member === "resolveToolPermission") {
      const index = permissions.findIndex((p) => p.requestId === args[1]);
      if (index < 0) return { error: "No pending approval" };
      permissions.splice(index, 1);
      if (args[2] === "allow_file_edits")
        sessions.find((s) => s.sessionId === args[0]).permissionMode =
          "acceptEdits";
      return { ok: true };
    }
    if (member === "sendMessage") {
      const s = sessions.find((s) => s.sessionId === args[0]);
      s.isRunning = true;
      s.bufferedMessages.push({
        type: "user",
        uuid: "user-one",
        message: { content: [{ type: "text", text: args[1] }] },
      });
      return { dispatched: true };
    }
    if (member === "stop") {
      sessions.find((s) => s.sessionId === args[0]).isRunning = false;
      return true;
    }
    if (member === "searchSessions")
      return sessions
        .filter((s) => s.title.toLowerCase().includes(args[0].toLowerCase()))
        .map((s) => ({ sessionId: s.sessionId, snippet: "Matched message" }));
    if (member === "getAllArtifacts") return structuredClone(artifacts);
    if (member === "getAllScheduledTasks") return structuredClone(schedules);
    if (member === "runScheduledTask") {
      const task=schedules.find(t=>t.id===args[0]);const id="scheduled-conversation";
      sessions.push({sessionId:id,title:task.displayName,cwd:task.cwd,spaceId:null,permissionMode:"default",isRunning:true,bufferedMessages:[]});
      return {sessionId:id};
    }
    if (member === "getLocalMcpServers") return structuredClone(mcpServers);
    if (member === "getMcpServersConfig") return structuredClone(mcpConfig);
    if (member === "setMcpServerConfigs") {
      if (mcpSaveError) return {error:mcpSaveError};
      return {ok:true};
    }
    if (service === "MCP" && ["connect", "reconnect", "refresh", "disconnect", "authenticate", "cancelAuthentication", "forgetAuthentication"].includes(member)) {
      if (mcpActionError) return {error:mcpActionError};
      const server = mcpServers.find(s=>s.name===args[0]);
      if (!server) return {error:"Unknown server"};
      if (member === "forgetAuthentication") { server.hasAuth=false; server.status="disconnected"; server.isConnected=false; return {ok:true}; }
      if (member === "authenticate") {
        server.status = "authenticating";
        server.authAttempt = `attempt-${calls.length}`;
        if (!mcpAuthDelayed) server.authorizationUrl = mcpAuthUrl;
        return {status:"authenticating"};
      }
      server.status = member === "cancelAuthentication" ? "needs_auth" : member === "disconnect" ? "disconnected" : member === "refresh" ? "refreshing" : "starting";
      server.isConnected = false;
      return {ok:true};
    }
    if (
      member === "listLocalSkills" ||
      member === "getLocalMcpServers" ||
      member === "getAllArtifacts" ||
      member === "getAllScheduledTasks"
    )
      return [];
    return null;
  };
  const api = async (route, body) => {
    calls.push({ route, body });
    if (route === "/__eipc__/open-external") return {ok:true};
    if (route === "/api/quixi/providers") return providers;
    if (route === "/api/quixi/providers/complete-onboarding") return providers;
    if (route === "/api/quixi/providers/test") return { ok: true, message: `Connected (${catalogModels.length} models available)`, discoveredModels: catalogModels };
    if (route === "/api/quixi/browser")
      return {
        selectedTabId: 1,
        tabs: [
          {
            tabId: 1,
            url: "https://example.invalid",
            title: "Browser fixture",
          },
        ],
      };
    if (route.startsWith("/api/quixi/artifacts/"))
      return {
        name: "Untrusted preview",
        html: '<script>parent.document.body.innerHTML="OWNED"</script>',
      };
    if (route === "/api/quixi/workspace/inspect")
      return { exists: true, path: folderPaths[body.path] || body.path };
    if (route === "/api/quixi/providers/save") {
      const p = providers.providers.find((p) => p.id === body.providerId);
      if (body.customModel) {
        const id = body.customModel.trim();
        p.customModels ||= [];
        if (!p.customModels.includes(id)) p.customModels.push(id);
        if (!p.models.some((m) => m.id === id))
          p.models.push({ id, label: id });
      }
      if (body.selectedModel) p.selectedModel = body.selectedModel;
      if (body.setAsActive) {
        providers.activeProvider = p.id;
        providers.activeModel = p.selectedModel;
      }
      if (body.refreshModels) {
        p.models = catalogModels.map((id) => ({id, label:id}));
        p.selectedModel = catalogModels[0] || "";
        providers.activeModel = p.selectedModel;
      }
      return structuredClone(providers);
    }
    throw Error("Unexpected API: " + route);
  };
  w.quixi = {
    rpc,
    api,
    copy: async (text) => calls.push({ clipboard: text }),
    origin: "https://app.localhost",
  };
  w.eval(source);
  await wait();
  const click = async (text) => {
    const b = Array.from(w.document.querySelectorAll("button")).find(
      (b) =>
        b.textContent.trim() === text || b.getAttribute("aria-label") === text,
    );
    assert.ok(b, "Missing button " + text);
    b.click();
    await wait();
    return b;
  };
  const type = (selector, value) => {
    const input = w.document.querySelector(selector);
    assert.ok(input, selector);
    input.value = value;
    input.dispatchEvent(new w.Event("input", { bubbles: true }));
    return input;
  };
  return { w, sessions, projects, calls, click, type, close: () => dom.window.close() };
}

test("unsent drafts survive navigation and provider names are not application branding", async () => {
  const f = await fixture();
  try {
    f.type("#prompt", "Draft café 🌲");
    await f.click("Search");
    await f.click("New task");
    assert.equal(f.w.document.querySelector("#prompt").value, "Draft café 🌲");
    assert.equal(
      f.w.document.querySelector(".brand").textContent,
      "QuixiDeskPersonal workspace",
    );
    assert.equal(
      f.w.document.querySelector("#send-button").textContent,
      "Send",
    );
  } finally {
    f.close();
  }
});

test("sending clears the correct draft and leaves Stop available during an active task", async () => {
  const f = await fixture();
  try {
    f.type("#prompt", "A real test prompt");
    await f.click("Send");
    const call = f.calls.find((c) => c.member === "sendMessage");
    assert.ok(call);
    assert.equal(call.args[1], "A real test prompt");
    assert.equal(f.w.document.querySelector("#prompt").value, "");
    assert.equal(
      f.w.document.querySelector("#send-button").textContent,
      "Stop",
    );
    await f.click("Stop");
    assert.equal(
      f.w.document.querySelector("#send-button").textContent,
      "Send",
    );
    assert.equal(
      f.w.document.querySelector("#prompt").placeholder,
      "Write your prompt to QuixiDesk",
    );
    assert.equal(f.calls.filter((c) => c.member === "sendMessage").length, 1);
  } finally {
    f.close();
  }
});

test("failed tool results are visible and replayed user messages are not duplicated", async () => {
  const f = await fixture();
  try {
    const s = f.sessions[0];
    s.bufferedMessages = [
      {
        type: "user",
        uuid: "same-user",
        message: { content: [{ type: "text", text: "Inspect this" }] },
      },
      {
        type: "user",
        uuid: "same-user",
        message: { content: [{ type: "text", text: "Inspect this" }] },
      },
      {
        type: "user",
        uuid: "tool-result",
        message: {
          content: [
            {
              type: "tool_result",
              is_error: true,
              content: "Permission denied",
            },
          ],
        },
      },
    ];
    f.w.quixiNavigate("chat/fixture-one");
    await wait();
    assert.equal(f.w.document.querySelectorAll(".message.user").length, 1);
    const failed = f.w.document.querySelector(".tool-card.failed");
    assert.ok(failed);
    assert.match(failed.textContent, /Tool action failed.*Permission denied/);
  } finally {
    f.close();
  }
});

test("artifact HTML is placed only in a sandbox without same-origin privileges", async () => {
  const f = await fixture();
  try {
    f.sessions[0].bufferedMessages = [
      {
        type: "assistant",
        message: {
          content: [
            {
              type: "text",
              text: "[Preview](app://localhost/cowork-artifact/fixture-artifact)",
            },
          ],
        },
      },
    ];
    f.w.quixiNavigate("chat/fixture-one");
    await wait();
    f.w.document.querySelector(".message .body a").click();
    await wait();
    const frame = f.w.document.querySelector("iframe");
    assert.ok(frame);
    assert.equal(frame.getAttribute("sandbox"), "allow-scripts allow-forms");
    assert.ok(!frame.getAttribute("sandbox").includes("allow-same-origin"));
    assert.match(frame.srcdoc, /parent.document/);
    assert.notEqual(f.w.document.body.textContent, "OWNED");
  } finally {
    f.close();
  }
});

test("streaming updates do not replace a follow-up being typed", async () => {
  const f = await fixture();
  try {
    f.type("#prompt", "Start working");
    await f.click("Send");
    f.type("#prompt", "Keep this follow-up");
    f.w.dispatchEvent(
      new f.w.CustomEvent("quixi:event", {
        detail: {
          channel: "quixi.web.LocalAgentModeSessions",
          payload: {
            sessionId: "fixture-new",
            message: {
              type: "stream_event",
              event: {
                type: "content_block_delta",
                delta: { type: "text_delta", text: "Streaming answer" },
              },
            },
          },
        },
      }),
    );
    assert.equal(
      f.w.document.querySelector("#prompt").value,
      "Keep this follow-up",
    );
    assert.match(
      f.w.document.querySelector("#conversation").textContent,
      /Streaming answer/,
    );
  } finally {
    f.close();
  }
});

test("choosing a project passes its context into the new task", async () => {
  const f = await fixture();
  try {
    await f.click("Projects");
    await wait();
    await f.click("Open project");
    await f.click("Start task");
    f.type("#prompt", "Project scoped work");
    await f.click("Send");
    const start = f.calls.find((c) => c.member === "start");
    assert.equal(start.args[0].spaceId, "project-a");
    assert.equal(start.args[0].cwd, "/project-a");
  } finally {
    f.close();
  }
});

test("adding a project folder opens the native picker and cancellation changes nothing", async () => {
  for (const pickedFolder of [null, "/native/College club"]) {
    const f = await fixture({pickedFolder});
    try {
      await f.click("Projects");
      await f.click("Open project");
      await f.click("Add folder");
      assert.ok(f.calls.some((c) => c.member === "browseAndAddTrustedFolder"));
      const added = f.calls.filter((c) => c.member === "addFolderToSpace");
      assert.equal(added.length, pickedFolder ? 1 : 0);
      if (pickedFolder) {
        assert.equal(added[0].args[1], pickedFolder);
        assert.match(f.w.document.querySelector("dialog").textContent, /native\/College club/);
      } else assert.equal(f.projects[0].folders.length, 1);
    } finally { f.close(); }
  }
});

test("project folder entry accepts home shorthand and saves the resolved path", async () => {
  const f = await fixture({folderPaths:{"~/College club":"/home/test/College club"}});
  try {
    await f.click("Projects");
    await f.click("Open project");
    await f.click("Enter path");
    const input = f.type('input[name="path"]', "~/College club");
    assert.ok(input.placeholder.includes("~/"));
    const form = input.closest("form");
    form.dispatchEvent(new f.w.Event("submit", {bubbles:true,cancelable:true}));
    await wait();
    const added = f.calls.find((c) => c.member === "addFolderToSpace");
    assert.equal(added.args[1], "/home/test/College club");
  } finally { f.close(); }
});

test("running a schedule clears the previously selected project", async () => {
  const f = await fixture({ schedules: [{id: "schedule-one", displayName: "Checklist", prompt: "Independent task", cwd: "/scheduled-work", enabled: true}] });
  try {
    await f.click("Projects");
    await f.click("Open project");
    await f.click("Start task");
    await f.click("Schedules");
    await f.click("Run now");
    assert.ok(f.calls.some(c=>c.member === "runScheduledTask" && c.args[0] === "schedule-one"));
    assert.ok(!f.calls.some(c=>c.member === "sendMessage"));
    assert.equal(f.sessions.find(s=>s.sessionId==="scheduled-conversation").spaceId,null);
    assert.match(f.w.document.body.textContent,/scheduled-work/);
  } finally { f.close(); }
});

test("opening a search result restores that conversation's workspace", async () => {
  const f = await fixture();
  try {
    await f.click("Projects");
    await f.click("Open project");
    await f.click("Start task");
    await f.click("Search");
    const result = f.w.document.querySelector("#search-results button");
    assert.ok(result);
    result.click();
    await wait();
    await f.click("workspace");
    assert.equal(f.w.document.querySelector('[aria-label="Workspace folder path"]').value, "/workspace");
  } finally { f.close(); }
});

test("out-of-order browser snapshots cannot create duplicate panels", async () => {
  const f = await fixture();
  try {
    await f.click("Toggle QuixiDesk Browser");
    assert.equal(f.w.document.querySelectorAll(".browser-panel").length, 1);
    const pending = [];
    f.w.fetch = () => new Promise((resolve) => pending.push(resolve));
    for (const revision of [1, 2]) {
      f.w.dispatchEvent(
        new f.w.CustomEvent("quixi:event", {
          detail: {
            channel: "quixi:browser-updated",
            payload: {
              revision,
              selectedTabId: 1,
              tabs: [
                {
                  tabId: 1,
                  url: `https://example.invalid/${revision}`,
                  title: `Snapshot ${revision}`,
                },
              ],
            },
          },
        }),
      );
    }
    assert.equal(pending.length, 2);
    pending[1]({ text: async () => "<p>Newest snapshot</p>" });
    await wait();
    pending[0]({ text: async () => "<p>Outdated snapshot</p>" });
    await wait();
    assert.equal(f.w.document.querySelectorAll(".browser-panel").length, 1);
    assert.equal(
      f.w.document.querySelector(".browser-address input").value,
      "https://example.invalid/2",
    );
    assert.match(
      f.w.document.querySelector("#browser-frame").srcdoc,
      /Newest snapshot/,
    );
  } finally {
    f.close();
  }
});

test("completion events for concurrent tasks refresh both running indicators", async () => {
  const f = await fixture();
  try {
    f.sessions[0].isRunning = true;
    f.sessions.push({
      ...f.sessions[0],
      sessionId: "fixture-two",
      title: "Second task",
    });
    f.w.dispatchEvent(new f.w.Event("focus"));
    await wait();
    assert.equal(f.w.document.querySelectorAll(".running-dot").length, 2);
    for (const s of f.sessions) {
      s.isRunning = false;
      f.w.dispatchEvent(
        new f.w.CustomEvent("quixi:event", {
          detail: {
            channel: "quixi.web.LocalAgentModeSessions",
            payload: { type: "close", sessionId: s.sessionId },
          },
        }),
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.equal(f.w.document.querySelectorAll(".running-dot").length, 0);
  } finally {
    f.close();
  }
});

test("formatted responses remain inert and code copy preserves the source", async () => {
  const f = await fixture();
  try {
    const source = 'const sample = "<script>";';
    const text =
      "# Summary\n\n**Useful** and `readable`\n\n- One\n- Two\n\n| Task | State |\n| --- | --- |\n| Review | Ready |\n\n> Keep it small.\n\n```js\n" +
      source +
      '\n```\n\n<img src=x onerror="window.injected=true">\n<script>window.injected=true</script>\n[unsafe](javascript:alert(1))\n\n# ';
    f.sessions[0].bufferedMessages = [
      { type: "assistant", message: { content: [{ type: "text", text }] } },
    ];
    f.w.quixiNavigate("chat/fixture-one");
    await wait();
    const body = f.w.document.querySelector(".message .body");
    assert.equal(body.querySelector("h2").textContent, "Summary");
    assert.equal(body.querySelectorAll("li").length, 2);
    assert.equal(body.querySelectorAll("td").length, 2);
    assert.equal(body.querySelector("strong").textContent, "Useful");
    assert.equal(
      body.querySelectorAll('script,img,a[href^="javascript:"]').length,
      0,
    );
    assert.equal(f.w.injected, undefined);
    await f.click("Copy code");
    assert.equal(f.calls.find((c) => c.clipboard).clipboard, source);
  } finally {
    f.close();
  }
});

test("separated numbered paragraphs retain their original numbering", async () => {
  const f = await fixture();
  try {
    f.sessions[0].bufferedMessages = [
      {
        type: "assistant",
        message: {
          content: [
            {
              type: "text",
              text: "1. First step\n\n2. Second step\n\n5. A later step",
            },
          ],
        },
      },
    ];
    await f.click("Existing conversation");
    assert.deepEqual(
      Array.from(f.w.document.querySelectorAll("ol"), (n) => n.start),
      [1, 2, 5],
    );
  } finally {
    f.close();
  }
});

test("theme choices update the workspace and survive page navigation", async () => {
  const f = await fixture();
  try {
    await f.click("Settings");
    await f.click("Use light theme");
    assert.equal(f.w.document.documentElement.dataset.theme, "light");
    await f.click("New task");
    await f.click("Settings");
    assert.equal(
      f.w.document
        .querySelector('[aria-label="Use light theme"]')
        .getAttribute("aria-pressed"),
      "true",
    );
    await f.click("Use system theme");
    assert.equal(f.w.document.documentElement.dataset.theme, "dark");
  } finally {
    f.close();
  }
});

test("gallery thumbnails are offline and cannot run artifact scripts", async () => {
  const f = await fixture({
    artifacts: [
      {
        id: "fixture-artifact",
        name: "A preview",
        html: '<script>parent.document.body.textContent="OWNED"</script><h1>Preview</h1>',
      },
    ],
  });
  try {
    await f.click("Artifacts");
    const frame = f.w.document.querySelector(".artifact-thumbnail iframe");
    assert.ok(frame);
    assert.equal(frame.getAttribute("sandbox"), "");
    assert.equal(frame.getAttribute("referrerpolicy"), "no-referrer");
    assert.match(frame.srcdoc, /default-src 'none'/);
    assert.match(frame.srcdoc, /form-action 'none'/);
    assert.notEqual(f.w.document.body.textContent, "OWNED");
    await f.click("Preview A preview");
    assert.ok(f.w.document.querySelector("dialog .preview-frame"));
  } finally {
    f.close();
  }
});

for (const providerId of ["custom", "openrouter"])
  test(`provider setup discovers all models without a Model ID: ${providerId}`, async () => {
    const catalogModels = Array.from({length:80}, (_,i) => `catalog/model-${String(i).padStart(2,"0")}`);
    const f = await fixture({providerId, catalogModels});
    try {
      await f.click("Settings");
      await f.click("Providers");
      await f.click("Manage providers");
      await f.click("Configure");
      assert.equal(f.w.document.querySelector('[placeholder="Model ID"]'), null);
      assert.equal(f.w.document.querySelector('[aria-label="Model"]'), null);
      await f.click("Save & use provider");
      const save = f.calls.find((c) => c.body?.refreshModels);
      assert.ok(save);
      assert.equal(save.body.selectedModel, undefined);
      assert.equal(save.body.refreshModels, true);
      await f.click("Close dialog");
      await f.click("New task");
      await f.click("Choose model");
      assert.equal(f.w.document.querySelectorAll('.model-options button').length, 80);
      f.type('[aria-label="Filter models"]', "model-79");
      assert.equal(f.w.document.querySelectorAll('.model-options button').length, 1);
      await f.click("catalog/model-79");
      const selected = f.calls.find((c) => c.body?.selectedModel === "catalog/model-79");
      assert.equal(selected.body.setAsActive, true);
    } finally { f.close(); }
  });

for (const providerId of ["custom", "openrouter"])
  test(`manual models can be saved without switching and then selected: ${providerId}`, async () => {
    const f = await fixture({ providerId });
    try {
      await f.click("Choose model");
      f.w.document.querySelector(".manual-models summary").click();
      await f.click("Add model");
      assert.match(
        f.w.document.querySelector(".form-error").textContent,
        /Enter a model ID/,
      );
      f.type('input[aria-label="Model ID"]', "  private/hidden:latest  ");
      await f.click("Add model");
      const save = f.calls.find((c) => c.body?.customModel);
      assert.deepEqual(JSON.parse(JSON.stringify(save.body)), {
        providerId,
        customModel: "private/hidden:latest",
        selectCustomModel: false,
        setAsActive: false,
      });
      assert.equal(
        f.w.document
          .querySelector(".model-control button")
          .textContent.replace("⌄", "")
          .trim(),
        "fixture-model",
      );
      const row = Array.from(
        f.w.document.querySelectorAll(".model-options button"),
      ).find((b) => b.textContent.includes("private/hidden:latest"));
      assert.match(row.textContent, /Added manually/);
      row.click();
      await wait();
      assert.ok(
        f.calls.some(
          (c) =>
            c.body?.selectedModel === "private/hidden:latest" &&
            c.body.setAsActive,
        ),
      );
    } finally {
      f.close();
    }
  });

test("tool calls and results share one compact disclosure that survives streaming", async () => {
  const f = await fixture();
  try {
    const s = f.sessions[0];
    s.isRunning = true;
    s.bufferedMessages = [
      {
        type: "user",
        uuid: "prompt-compact",
        message: { content: [{ type: "text", text: "Explore this project" }] },
      },
      {
        type: "assistant",
        uuid: "calls",
        message: {
          content: [0, 1, 2, 3, 4].map((i) => ({
            type: "tool_use",
            id: "call-" + i,
            name: i ? "Read" : "ListDirectory",
            input: i ? { file_path: "file-" + i + ".md" } : { path: "." },
          })),
        },
      },
      {
        type: "user",
        uuid: "results",
        message: {
          content: [4, 3, 2, 1, 0].map((i) => ({
            type: "tool_result",
            tool_use_id: "call-" + i,
            is_error: i === 3,
            content: i === 3 ? "Permission denied" : "Contents " + i,
          })),
        },
      },
    ];
    await f.click("Existing conversation");
    const group = f.w.document.querySelector(".activity-group");
    assert.ok(group);
    assert.equal(group.open, false);
    assert.match(
      group.querySelector("summary").textContent,
      /5 actions1 failed/,
    );
    assert.equal(f.w.document.querySelectorAll(".tool-step").length, 5);
    assert.match(
      f.w.document.querySelector(".tool-step.failed").textContent,
      /Read file-3.md.*Permission denied/s,
    );
    group.open = true;
    group.querySelector(".tool-step").open = true;
    f.w.dispatchEvent(
      new f.w.CustomEvent("quixi:event", {
        detail: {
          channel: "quixi.web.LocalAgentModeSessions",
          payload: {
            sessionId: s.sessionId,
            message: {
              type: "stream_event",
              event: {
                type: "content_block_delta",
                delta: {
                  type: "text_delta",
                  text: "The project looks useful.",
                },
              },
            },
          },
        },
      }),
    );
    assert.equal(f.w.document.querySelector(".activity-group").open, true);
    assert.equal(f.w.document.querySelector(".tool-step").open, true);
    assert.equal(f.w.document.querySelectorAll(".activity-group").length, 1);
  } finally {
    f.close();
  }
});
for (const decision of ["Allow file edits", "Deny"])
  test(`pending edits can ${decision} without resending the prompt`, async () => {
    const request = {
      requestId: "approval-one",
      toolCallId: "write-one",
      toolName: "Write",
      input: {
        file_path: "README.md",
        content: "<script>not executable</script>",
      },
    };
    const f = await fixture({ permissions: [request] });
    try {
      f.sessions[0].isRunning = true;
      f.sessions[0].bufferedMessages = [
        {
          type: "assistant",
          message: {
            content: [
              {
                type: "tool_use",
                id: "write-one",
                name: "Write",
                input: request.input,
              },
            ],
          },
        },
      ];
      await f.click("Existing conversation");
      const card = f.w.document.querySelector(".permission-card");
      assert.ok(card);
      assert.equal(card.querySelector("script"), null);
      assert.match(card.textContent, /README.md/);
      await f.click(decision);
      assert.equal(f.w.document.querySelector(".permission-card"), null);
      assert.equal(f.calls.filter((c) => c.member === "sendMessage").length, 0);
      const call = f.calls.find((c) => c.member === "resolveToolPermission");
      assert.equal(call.args[1], "approval-one");
      assert.equal(
        call.args[2],
        decision === "Deny" ? "deny" : "allow_file_edits",
      );
      assert.equal(
        f.w.document.querySelector('select[aria-label="Tool permission mode"]')
          .value,
        decision === "Deny" ? "default" : "acceptEdits",
      );
    } finally {
      f.close();
    }
  });

test("stopped response text stays readable after reopening a conversation", async () => {
  const f = await fixture();
  try {
    f.sessions[0].bufferedMessages = [
      {
        type: "assistant",
        message: {
          stop_reason: "user_stop",
          content: [
            {
              type: "text",
              text: "A partial response that must remain saved.",
            },
          ],
        },
      },
    ];
    await f.click("Existing conversation");
    assert.match(
      f.w.document.querySelector(".message.assistant").textContent,
      /QuixiDesk · Stopped.*partial response/s,
    );
    await f.click("Search");
    await f.click("Existing conversation");
    assert.match(
      f.w.document.querySelector(".message.assistant").textContent,
      /partial response/,
    );
  } finally {
    f.close();
  }
});


test("conversation find highlights, navigates, closes and preserves text", async()=>{
  const f=await fixture();try{
    f.sessions[0].bufferedMessages=[{type:"assistant",message:{content:[{type:"text",text:"Alpha needle beta needle gamma."}]}}];
    await f.click("Existing conversation");await f.click("Find in conversation");
    f.type('#conversation-find input','needle');
    assert.equal(f.w.document.querySelectorAll('mark[data-find]').length,2);
    assert.match(f.w.document.querySelector('#conversation-find [role=status]').textContent,/1 of 2/);
    await f.click("Next match");assert.match(f.w.document.querySelector('#conversation-find [role=status]').textContent,/2 of 2/);
    await f.click("Close find");assert.equal(f.w.document.querySelectorAll('mark[data-find]').length,0);assert.match(f.w.document.querySelector('#conversation').textContent,/Alpha needle beta needle gamma/);
  }finally{f.close();}
});


test("subagent activity stays collapsed and separate from the parent transcript", async () => {
  const f = await fixture({subagents:[{agentId:"child-one", title:"Review README", status:"completed", readOnly:true, model:"fixture-model", toolCalls:1, prompt:"Inspect README", messages:[{role:"assistant",content:"PRIVATE CHILD OUTPUT <script>window.owned=true</script>"},{role:"tool",content:"Read token 42"}]}]});
  try {
    await f.click("Existing conversation");
    const panel = f.w.document.querySelector(".subagent-panel");
    assert.ok(panel);
    assert.equal(panel.open, false);
    assert.match(panel.textContent, /completed · Read-only/);
    assert.doesNotMatch(f.w.document.querySelector("main").textContent, /PRIVATE CHILD OUTPUT/);
    panel.open = true;
    await f.click("View activity");
    const history = f.w.document.querySelector(".subagent-history");
    assert.match(history.textContent, /PRIVATE CHILD OUTPUT/);
    assert.match(history.textContent, /Assignment and recorded activity/);
    assert.equal(history.querySelectorAll("script").length, 0);
    assert.equal(f.w.owned, undefined);
    assert.ok(f.calls.some(c=>c.member==="getSubagent" && c.args[0]==="fixture-one" && c.args[1]==="child-one"));
  } finally { f.close(); }
});


test("MCP readiness and stale catalogs are shown honestly and failures can be retried", async () => {
  const servers = [{name:"rea",status:"failed",isConnected:false,toolsEnumerated:true,catalogStale:true,tools:[{name:"inspect",inputSchema:{type:"object"}}],error:{kind:"spawn",message:"Could not start npx"}}];
  const f = await fixture({mcpServers:servers,mcpActionError:"Executable was not found"});
  try {
    await f.click("Tools & skills");
    const row = f.w.document.querySelector('[data-server="rea"]');
    assert.equal(row.querySelector(".mcp-state").textContent,"Failed");
    assert.match(row.textContent,/last-known catalog · connection not verified/);
    assert.match(row.textContent,/Could not start npx/);
    assert.equal(row.querySelector('[data-mcp-action="connect"]').hidden,true);
    await f.click("Retry rea");
    assert.match(f.w.document.getElementById("toasts").textContent,/Executable was not found/);
    assert.equal(row.querySelector(".mcp-state").textContent,"Failed");
    assert.ok(f.calls.some(c=>c.service==="MCP" && c.member==="reconnect" && c.args[0]==="rea"));
  } finally { f.close(); }
});

test("MCP connect and status refresh preserve row identity and catalog controls", async () => {
  const servers = [{name:"rea",status:"configured",isConnected:false,toolsEnumerated:false,tools:[],transport:"stdio",cwd:"/tmp/project",generation:0}];
  const f = await fixture({mcpServers:servers});
  try {
    await f.click("Tools & skills");
    const row = f.w.document.querySelector('[data-server="rea"]');
    assert.equal(row.querySelector(".mcp-state").textContent,"Not connected");
    const diagnostics = row.querySelector(".mcp-diagnostics"); diagnostics.open = true;
    assert.match(diagnostics.textContent,/Transport: stdio/);
    await f.click("Connect rea");
    assert.equal(row.querySelector(".mcp-state").textContent,"Starting");
    assert.equal(row.querySelector('[data-mcp-action="disconnect"]').hidden,false);
    Object.assign(servers[0],{status:"ready",isConnected:true,toolsEnumerated:true,tools:[{name:"analyze",description:"Inspect a binary",inputSchema:{type:"object",properties:{path:{type:"string"}}}}]});
    f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-updated",payload:{}}}));
    await wait();
    assert.equal(f.w.document.querySelector('[data-server="rea"]'),row);
    assert.equal(row.querySelector(".mcp-diagnostics"),diagnostics);
    assert.equal(diagnostics.open,true);
    assert.equal(row.querySelector(".mcp-state").textContent,"Ready");
    assert.match(row.textContent,/1 tool discovered/);
    assert.equal(row.querySelector('[data-mcp-action="catalog"]').disabled,false);
    await f.click("Disconnect rea");
    assert.equal(row.querySelector(".mcp-state").textContent,"Disconnected");
    assert.match(row.textContent,/last-known catalog/);
  } finally { f.close(); }
});

test("MCP catalog preserves full schemas, escapes tool content and keeps search/disclosure state", async () => {
  const schema={type:"object",properties:{mode:{oneOf:[{const:"fast"},{const:"complete"}]}},additionalProperties:false,$defs:{nested:{type:"string",pattern:"^[a-z]+$"}}};
  const servers=[{name:"rea",status:"ready",isConnected:true,toolsEnumerated:true,tools:[{name:"analyze",description:"<script>window.owned=true</script> Analyze an artifact",inputSchema:schema,outputSchema:{type:"object"},annotations:{readOnlyHint:true}},{name:"close",description:"Close a session",inputSchema:{type:"object"}}]}];
  const f=await fixture({mcpServers:servers});
  try {
    await f.click("Tools & skills");await f.click("View tools for rea");
    const search=f.w.document.querySelector('[aria-label="Search tools for rea"]');
    search.value="analy";search.dispatchEvent(new f.w.Event("input",{bubbles:true}));search.focus();
    let detail=f.w.document.querySelector(".mcp-tool");detail.open=true;
    assert.equal(f.w.document.querySelectorAll(".mcp-tool").length,1);
    assert.deepEqual(JSON.parse(detail.querySelector("pre").textContent).inputSchema,schema);
    assert.equal(detail.querySelectorAll("script").length,0);assert.equal(f.w.owned,undefined);
    servers[0].tools[0].description="Updated analysis description";
    f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-updated",payload:{}}}));await wait();
    assert.equal(f.w.document.activeElement,search);assert.equal(search.value,"analy");
    detail=f.w.document.querySelector(".mcp-tool");assert.equal(detail.open,true);assert.match(detail.textContent,/Updated analysis/);
  } finally { f.close(); }
});

test("MCP remote configuration accepts HTTP and keeps backend save errors visible", async () => {
  const f=await fixture({mcpConfig:{rea:{command:"npx",args:["rea-agents","mcp"]}},mcpSaveError:"Existing configuration is unreadable; no changes saved."});
  try {
    await f.click("Tools & skills");await f.click("Configure connections");
    const config={mcpServers:{remote:{transport:"http",url:"https://example.invalid/mcp",headers:{Authorization:"test-fixture"},enabled:true}}};
    f.type("dialog textarea",JSON.stringify(config));
    f.w.document.querySelector("dialog form").dispatchEvent(new f.w.Event("submit",{bubbles:true,cancelable:true}));await wait();
    assert.ok(f.calls.some(c=>c.member==="setMcpServerConfigs" && c.args[0].remote.url===config.mcpServers.remote.url));
    assert.match(f.w.document.querySelector("dialog .form-error").textContent,/Existing configuration is unreadable/);
    assert.equal(f.w.document.querySelector("dialog").open,true);
  } finally { f.close(); }
});

test("MCP polling stops after leaving the Tools page", async () => {
  const f=await fixture({mcpServers:[{name:"rea",status:"configured",tools:[]}]});
  try {
    await f.click("Tools & skills");await f.click("New task");
    const count=f.calls.filter(c=>c.member==="getLocalMcpServers").length;
    await new Promise(resolve=>setTimeout(resolve,2100));
    assert.equal(f.calls.filter(c=>c.member==="getLocalMcpServers").length,count);
  } finally { f.close(); }
});


test("MCP malformed configuration cannot open an empty replacement editor", async () => {
  const f=await fixture({mcpConfig:{error:"MCP configuration is not valid JSON"}});
  try {
    await f.click("Tools & skills");await f.click("Configure connections");
    assert.match(f.w.document.getElementById("toasts").textContent,/configuration is not valid JSON/);
    assert.equal(f.w.document.querySelector("dialog textarea"),null);
    assert.equal(f.calls.some(c=>c.member==="setMcpServerConfigs"),false);
  } finally { f.close(); }
});


test("MCP OAuth opens the browser, remains pending until discovery and can be cancelled", async () => {
  const servers=[{name:"remote",status:"needs_auth",isConnected:false,toolsEnumerated:false,tools:[],error:{kind:"auth",message:"Sign in to continue"}}];
  const f=await fixture({mcpServers:servers});
  try {
    await f.click("Tools & skills");await f.click("Sign in remote");
    const row=f.w.document.querySelector('[data-server="remote"]');
    assert.equal(row.querySelector(".mcp-state").textContent,"Waiting for sign-in");
    assert.match(row.querySelector(".mcp-auth-hint").textContent,/Complete sign-in in your browser/);
    assert.ok(f.calls.some(c=>c.route==="/__eipc__/open-external" && c.body.url.startsWith("https://example.invalid/oauth/authorize")));
    assert.doesNotMatch(row.querySelector(".mcp-catalog-count").textContent,/discovered/);
    await f.click("Open sign-in page for remote");
    assert.equal(f.calls.filter(c=>c.route==="/__eipc__/open-external").length,2);
    await f.click("Cancel sign-in remote");
    assert.equal(row.querySelector(".mcp-state").textContent,"Sign-in required");
    assert.equal(row.querySelector('[data-mcp-action="cancelAuthentication"]').hidden,true);
    assert.ok(f.calls.some(c=>c.member==="cancelAuthentication"));
    await f.click("Sign in remote");
    Object.assign(servers[0],{status:"ready",isConnected:true,toolsEnumerated:true,error:null});
    f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-updated",payload:{}}}));await wait();
    assert.equal(row.querySelector(".mcp-state").textContent,"Ready");
    assert.equal(row.querySelector('[data-mcp-action="openAuth"]').hidden,true);
  } finally { f.close(); }
});

test("MCP OAuth rejects non-web authorization URLs and builtins have no connection controls", async () => {
  const f=await fixture({mcpAuthUrl:"file:///tmp/unsafe",mcpServers:[{name:"remote",status:"needs_auth",tools:[]},{name:"builtin",builtin:true,status:"ready",isConnected:true,toolsEnumerated:true,tools:[{name:"read",inputSchema:{type:"object"}}]}]});
  try {
    await f.click("Tools & skills");await f.click("Sign in remote");
    assert.match(f.w.document.getElementById("toasts").textContent,/Only HTTP and HTTPS/);
    assert.equal(f.calls.some(c=>c.route==="/__eipc__/open-external"),false);
    const builtin=f.w.document.querySelector('[data-server="builtin"]');
    for (const button of builtin.querySelectorAll('[data-mcp-action]')) assert.equal(button.hidden,button.dataset.mcpAction!=="catalog");
  } finally { f.close(); }
});


test("MCP asynchronous authorization opens only once when metadata becomes ready", async () => {
  const servers=[{name:"remote",status:"needs_auth",tools:[]}];
  const f=await fixture({mcpServers:servers,mcpAuthDelayed:true});
  try {
    await f.click("Tools & skills");await f.click("Sign in remote");
    assert.match(f.w.document.querySelector(".mcp-auth-hint").textContent,/Preparing secure sign-in/);
    assert.equal(f.calls.filter(c=>c.route==="/__eipc__/open-external").length,0);
    servers[0].authorizationUrl="https://example.invalid/oauth/authorize?state=later";
    for (let i=0;i<3;i++) {
      f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-updated",payload:{}}}));await wait();
    }
    assert.equal(f.calls.filter(c=>c.route==="/__eipc__/open-external").length,1);
    await f.click("Cancel sign-in remote");
    assert.equal(f.w.document.querySelector('[data-mcp-action="openAuth"]').hidden,true);
  } finally { f.close(); }
});


test("MCP lifecycle controls stay visually hidden with the real stylesheet", async () => {
  const f=await fixture({mcpServers:[{name:"rea",status:"ready",isConnected:true,toolsEnumerated:true,tools:[]}]});
  try {
    const style=f.w.document.createElement("style");style.textContent=fs.readFileSync(path.join(__dirname,"../../resources/ui/app.css"),"utf8");f.w.document.head.append(style);
    await f.click("Tools & skills");
    const row=f.w.document.querySelector('[data-server="rea"]');
    assert.equal(f.w.getComputedStyle(row.querySelector('[data-mcp-action="authenticate"]')).display,"none");
    assert.equal(f.w.getComputedStyle(row.querySelector('[data-mcp-action="connect"]')).display,"none");
    assert.notEqual(f.w.getComputedStyle(row.querySelector('[data-mcp-action="disconnect"]')).display,"none");
  } finally { f.close(); }
});

test("MCP forget-sign-in requires explicit confirmation and updates connection state", async () => {
  const f=await fixture({mcpServers:[{name:"remote",status:"ready",hasAuth:true,isConnected:true,toolsEnumerated:true,tools:[]}]});
  try {
    await f.click("Tools & skills");await f.click("Forget sign-in for remote");
    assert.equal(f.calls.some(c=>c.member==="forgetAuthentication"),false);
    assert.match(f.w.document.querySelector("dialog").textContent,/remove its saved sign-in credentials/);
    f.w.document.querySelector("dialog button.danger").click();await wait();
    assert.ok(f.calls.some(c=>c.member==="forgetAuthentication" && c.args[0]==="remote"));
    const row=f.w.document.querySelector('[data-server="remote"]');
    assert.equal(row.querySelector(".mcp-state").textContent,"Disconnected");
    assert.equal(row.querySelector('[data-mcp-action="forgetAuthentication"]').hidden,true);
  } finally { f.close(); }
});
