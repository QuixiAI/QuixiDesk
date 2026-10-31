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
  mcpInteractions = [],
  interactionError = null,
  resourceCatalog = {resources:[],resourceTemplates:[],prompts:[],roots:[]},
  resourceText = "Resource fixture text",
  resourceUpdates = [],
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
  sendGate = null,
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
  w.TextEncoder = TextEncoder;
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
  const pendingMessages = [];
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
    if (member === "deleteSpace") {
      const index = projects.findIndex(p => p.id === args[0]);
      if (index < 0) return false;
      projects.splice(index, 1);
      return true;
    }
    if (member === "delete") {
      const index = sessions.findIndex(s => s.sessionId === args[0]);
      if (index < 0) return false;
      sessions.splice(index, 1);
      return true;
    }
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
        cwd: args[0].cwd ?? "",
        userSelectedFolders: args[0].userSelectedFolders || [],
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
    if (member === "cancelPendingMessage" || member === "guidePendingMessage") {
      const index = pendingMessages.findIndex(m => m.messageUuid === args[1]);
      if (index < 0) return {error:"No longer pending"};
      const session = sessions.find(s => s.sessionId === args[0]);
      if (member === "cancelPendingMessage") { pendingMessages.splice(index, 1); return {cancelled:true}; }
      if (session.isRunning) { pendingMessages[index].delivery = "steer"; pendingMessages[index].held = false; }
      else { pendingMessages.splice(index, 1); session.isRunning = true; }
      return {dispatched:true};
    }
    if (member === "getPendingMessages") return structuredClone(pendingMessages);
    if (member === "sendMessageV2") {
      if(sendGate)await sendGate;
      const s = sessions.find((s) => s.sessionId === args[0]);
      if (s.isRunning) {
        pendingMessages.push(args[1]);
        return {dispatched:true, queued:true, delivery:args[1].delivery};
      }
      s.isRunning = true;
      s.bufferedMessages.push({
        type: "user",
        uuid: "user-one",
        message: { content: [{ type: "text", text: args[1].text }] },
      });
      return { dispatched: true };
    }
    if (member === "forkMessage") {
      const original=sessions.find(s=>s.sessionId===args[0]);
      const branch=structuredClone(original);branch.sessionId="fixture-branch";branch.forkedFrom={sessionId:original.sessionId,messageUuid:args[1].messageUuid};
      branch.bufferedMessages=[{type:"user",uuid:"edited-one",message:{content:[{type:"text",text:args[1].text||original.bufferedMessages[0].message.content[0].text}]}}];
      sessions.push(branch);return structuredClone(branch);
    }
    if (member === "stop") {
      pendingMessages.forEach(m => m.held = true);
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
    if (member === "browseFolder") return pickedFolder;
    if (member === "pendingInteractions") return structuredClone(mcpInteractions);
    if (member === "respondInteraction") {
      if (interactionError) return {error:interactionError};
      const index=mcpInteractions.findIndex(item=>item.id===args[0]);
      if(index<0)return {error:"Request expired"};
      const item=mcpInteractions[index];
      if(item.mode==="url"&&args[1]==="accept"){item.status="waiting_external";return {ok:true,url:item.url};}
      mcpInteractions.splice(index,1);return {ok:true};
    }
    if (member === "setToolEnabled") {
      if(mcpActionError)return {error:mcpActionError};
      mcpServers.find(s=>s.name===args[0]).tools.find(t=>t.name===args[1]).enabled=args[2];return {ok:true};
    }
    if(member==="browseResources")return structuredClone(resourceCatalog);
    if(member==="readResource")return {contents:[{uri:args[1],text:typeof resourceText==="function"?resourceText():resourceText}]};
    if(member==="getPrompt")return {messages:[{role:"user",content:{type:"text",text:"Prompt for "+args[2].topic}}]};
    if(member==="expandResourceTemplate")return {uri:args[1].replace("{name}",encodeURIComponent(args[2].name))};
    if(member==="resourceUpdates")return structuredClone(resourceUpdates);
    if(["subscribeResource","setRoots"].includes(member))return {ok:true};
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
  for (const name of ["workspace-workflows", "schedule-workflows", "capability-workflows", "chat-workflows"]) { const file=path.join(__dirname, "../../resources/ui/"+name+".js"); if(fs.existsSync(file)) w.eval(fs.readFileSync(file,"utf8")); }
  w.eval(source.replace('  function plainBody(text) {', '  window.testPlainBody = plainBody;\n  function plainBody(text) {'));
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

test("composer arrows recall prompts and restore drafts without stealing editing keys", async () => {
  const f = await fixture();
  try {
    f.sessions[0].bufferedMessages = [
      {type:"user",message:{content:[{type:"text",text:"First prompt"}]}},
      {type:"assistant",message:{content:[{type:"text",text:"Not history"}]}},
      {type:"user",message:{content:[{type:"text",text:"Second prompt"}]}},
    ];
    await f.click("Existing conversation");
    const input = f.w.document.querySelector("#prompt");
    const key = (name, options={}) => {
      const e = new f.w.KeyboardEvent("keydown", {key:name,bubbles:true,cancelable:true,...options});
      input.dispatchEvent(e); return e.defaultPrevented;
    };
    f.type("#prompt", "Unsent draft");
    assert.equal(key("ArrowUp"),true);
    assert.equal(input.value,"Second prompt");
    key("ArrowUp"); assert.equal(input.value,"First prompt");
    key("ArrowUp"); assert.equal(input.value,"First prompt");
    key("ArrowDown"); assert.equal(input.value,"Second prompt");
    key("ArrowDown"); assert.equal(input.value,"Unsent draft");
    f.type("#prompt", "line one\nline two");
    input.setSelectionRange(input.value.length,input.value.length);
    assert.equal(key("ArrowUp"),false);
    input.setSelectionRange(0,0);
    assert.equal(key("ArrowUp",{isComposing:true}),false);
    assert.equal(key("ArrowUp",{shiftKey:true}),false);
    input.setSelectionRange(0,4);
    assert.equal(key("ArrowUp"),false);
    input.setSelectionRange(0,0);
    key("ArrowUp"); assert.equal(input.value,"Second prompt");
    key("ArrowDown"); assert.equal(input.value,"line one\nline two");
    await f.click("New task");
    const fresh = f.w.document.querySelector("#prompt");
    fresh.dispatchEvent(new f.w.KeyboardEvent("keydown",{key:"ArrowUp",bubbles:true}));
    assert.equal(fresh.value,"");
  } finally { f.close(); }
});

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
    const call = f.calls.find((c) => c.member === "sendMessageV2");
    assert.ok(call);
    assert.equal(call.args[1].text, "A real test prompt");
    assert.equal(f.w.document.querySelector("#prompt").value, "");
    assert.equal(
      f.w.document.querySelector("#stop-button").textContent,
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
    assert.equal(f.calls.filter((c) => c.member === "sendMessageV2").length, 1);
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
    await new Promise(resolve => f.w.requestAnimationFrame(resolve));
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
      await f.click("Manage folders");
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
    await f.click("Manage folders");
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
    assert.ok(!f.calls.some(c=>c.member === "sendMessageV2"));
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
      assert.equal(f.calls.filter((c) => c.member === "sendMessageV2").length, 0);
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
    await f.click("Tools & skills");await f.click("Advanced JSON");
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
    await f.click("Tools & skills");await f.click("Advanced JSON");
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

const pendingRequest = (overrides={}) => ({id:"request-1",server:"fixture",mode:"form",message:"Choose project details",status:"pending",expiresAt:Date.now()+60000,requestedSchema:{type:"object",properties:{name:{type:"string",title:"Club name"},seats:{type:"integer",minimum:1},active:{type:"boolean"}},required:["name"]},...overrides});
const refreshRequests = async f => {f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-interaction",payload:{}}}));await wait();};

test("MCP elicitation appears outside Tools, escapes content and preserves typed input during refresh",async()=>{
  const requests=[pendingRequest({server:"<img src=x onerror=owned()>",message:"<script>owned()</script>"})];
  const f=await fixture({mcpInteractions:requests});try{
    assert.ok(f.w.document.querySelector("#mcp-interactions"));assert.equal(f.w.document.querySelector("#mcp-interactions img"),null);
    await f.click("Review request");const modal=f.w.document.querySelector("dialog");assert.equal(modal.querySelector("script"),null);
    await f.click("Send response");assert.equal(f.calls.filter(c=>c.member==="respondInteraction").length,0);
    const input=modal.querySelector('input[type="text"]');input.value="Chess club";input.focus();
    await refreshRequests(f);assert.equal(modal.querySelector('input[type="text"]'),input);assert.equal(input.value,"Chess club");assert.equal(f.w.document.activeElement,input);
    modal.querySelector('input[type="number"]').value="4";modal.querySelector('input[type="checkbox"]').checked=true;
    await f.click("Send response");const response=f.calls.find(c=>c.member==="respondInteraction");assert.equal(response.args[1],"accept");assert.deepEqual(JSON.parse(JSON.stringify(response.args[2])),{name:"Chess club",seats:4,active:true});
    assert.equal(f.w.document.querySelector("dialog"),null);
  }finally{f.close();}
});

for(const action of ["Decline","Cancel","Close"]){test(`MCP form ${action} never submits entered values`,async()=>{
  const f=await fixture({mcpInteractions:[pendingRequest()]});try{
    await f.click("Review request");f.w.document.querySelector('dialog input[type="text"]').value="Unsent details";
    if(action==="Close")f.w.document.querySelector("dialog").close();else await f.click(action);
    await wait();const response=f.calls.find(c=>c.member==="respondInteraction");assert.equal(response.args[1],action==="Decline"?"decline":"cancel");assert.equal(response.args[2],null);
  }finally{f.close();}
});}

test("MCP external requests require explicit domain review and waiting controls remain hidden with real CSS",async()=>{
  const requests=[pendingRequest({mode:"url",url:"https://accounts.example.invalid/consent",host:"accounts.example.invalid"})];
  const f=await fixture({mcpInteractions:requests});try{
    const style=f.w.document.createElement("style");style.textContent=fs.readFileSync(path.join(__dirname,"../../resources/ui/app.css"),"utf8");f.w.document.head.append(style);
    assert.equal(f.calls.filter(c=>c.route==="/__eipc__/open-external").length,0);
    await f.click("Review request");assert.match(f.w.document.querySelector(".mcp-external-destination strong").textContent,/accounts.example.invalid/);
    assert.equal(f.calls.filter(c=>c.route==="/__eipc__/open-external").length,0);
    await f.click("Open external page");assert.equal(f.calls.filter(c=>c.route==="/__eipc__/open-external").length,1);
    assert.match(f.w.document.querySelector("#mcp-interactions").textContent,/Waiting for the external step/);
    await f.click("Review request");const modal=f.w.document.querySelector("dialog");const accept=Array.from(modal.querySelectorAll("button")).find(b=>b.textContent==="Open external page");assert.equal(f.w.getComputedStyle(accept).display,"none");
    await f.click("Dismiss request");assert.equal(f.calls.at(-2)?.args?.[1]==="accept",false);
  }finally{f.close();}
});

test("MCP invalid URL requests cannot open external handlers and expiration closes an unanswered form",async()=>{
  const requests=[pendingRequest({mode:"url",url:"file:///etc/passwd"})];const f=await fixture({mcpInteractions:requests});try{
    await f.click("Review request");assert.match(f.w.document.querySelector('dialog [role="alert"]').textContent,/Only HTTP/);const accept=Array.from(f.w.document.querySelectorAll("dialog button")).find(b=>b.textContent==="Open external page");assert.equal(accept.disabled,true);
    requests.splice(0);await refreshRequests(f);assert.equal(f.w.document.querySelector("dialog"),null);assert.equal(f.calls.filter(c=>c.member==="respondInteraction").length,0);
    assert.equal(f.calls.filter(c=>c.route==="/__eipc__/open-external").length,0);
  }finally{f.close();}
});

test("MCP tool policies expose app-only status and persist explicit toggles",async()=>{
  const servers=[{name:"fixture",status:"ready",isConnected:true,toolsEnumerated:true,tools:[{name:"app_tool",enabled:true,modelVisible:false,inputSchema:{type:"object"}}]}];const f=await fixture({mcpServers:servers});try{
    await f.click("Tools & skills");await f.click("View tools for fixture");assert.match(f.w.document.querySelector(".mcp-tool").textContent,/App only/);
    const toggle=f.w.document.querySelector('[aria-label="Enable app_tool on fixture"]');toggle.click();await wait();assert.deepEqual(Array.from(f.calls.find(c=>c.member==="setToolEnabled").args),["fixture","app_tool",false]);assert.equal(servers[0].tools[0].enabled,false);
  }finally{f.close();}
});

test("MCP resources require preview and explicit attachment, subscriptions require review before sending updates",async()=>{
  let text="Original context";const updates=[];const f=await fixture({mcpServers:[{name:"fixture",status:"ready",tools:[]}],resourceCatalog:{resources:[{name:"Guide",uri:"fixture://guide",description:"<script>bad()</script>"}]},resourceText:()=>text,resourceUpdates:updates});try{
    await f.click("Add MCP context");const selector=f.w.document.querySelector('[aria-label="MCP context server"]');selector.value="fixture";selector.dispatchEvent(new f.w.Event("change"));await wait();
    assert.equal(f.w.document.querySelector(".mcp-context-results script"),null);await f.click("Preview");assert.equal(f.w.document.querySelector("#attachment-chips").textContent,"");
    await f.click("Add to message");assert.match(f.w.document.querySelector("#attachment-chips").textContent,/fixture: Guide/);
    await f.click("Follow changes");assert.equal(f.calls.find(c=>c.member==="subscribeResource").args[3],true);
    text="Updated context";updates.push({server:"fixture",uri:"fixture://guide",version:1});await refreshRequests(f);assert.match(f.w.document.querySelector("#attachment-chips").textContent,/Review update/);
    f.type("#prompt","Use this context");await f.click("Send");assert.equal(f.calls.filter(c=>c.member==="start").length,0);
    await f.click("Review update");assert.match(f.w.document.querySelector(".mcp-context-preview").textContent,/Updated context/);await f.click("Use this version");await f.click("Send");
    assert.ok(f.calls.some(c=>c.member==="start"));const sent=f.calls.find(c=>c.member==="sendMessageV2");assert.match(JSON.stringify(sent?.args||f.calls.find(c=>c.member==="start").args),/Updated context/);
  }finally{f.close();}
});

test("MCP templates and prompt arguments are previewed without executing or sending a message",async()=>{
  const f=await fixture({mcpServers:[{name:"fixture",status:"ready",tools:[]}],resourceCatalog:{resourceTemplates:[{name:"Member",uriTemplate:"fixture://members/{name}"}],prompts:[{name:"Brief",arguments:[{name:"topic",required:true}]}]}});try{
    await f.click("Add MCP context");const select=f.w.document.querySelector('[aria-label="MCP context server"]');select.value="fixture";select.dispatchEvent(new f.w.Event("change"));await wait();
    await f.click("Preview");f.type('dialog:last-of-type input',"Chess club");
    // The parent picker also has Preview buttons, so submit the argument form directly.
    const form=f.w.document.querySelector("dialog:last-of-type form");if(form){form.dispatchEvent(new f.w.Event("submit",{bubbles:true,cancelable:true}));await wait();}
    assert.ok(f.calls.some(c=>c.member==="expandResourceTemplate"&&c.args[2].name==="Chess club"));assert.ok(f.calls.some(c=>c.member==="readResource"&&c.args[1]==="fixture://members/Chess%20club"));assert.equal(f.calls.filter(c=>c.member==="start").length,0);
  }finally{f.close();}
});

test("MCP workspace roots are server-scoped and use the native folder picker",async()=>{
  const f=await fixture({mcpServers:[{name:"fixture",status:"ready",isConnected:true,tools:[]}],resourceCatalog:{roots:[{uri:"file:///workspace",name:"workspace"}]},pickedFolder:"/second"});try{
    await f.click("Tools & skills");await f.click("Workspace folders for fixture");await f.click("Add folder");await f.click("Save folders");const saved=f.calls.find(c=>c.member==="setRoots");assert.equal(saved.args[0],"fixture");assert.deepEqual(Array.from(saved.args[2]),["/workspace","/second"]);
  }finally{f.close();}
});

test("MCP configuration repair preserves an unrelated invalid server and healthy controls remain usable",async()=>{
  const config={broken:{command:42},healthy:{command:"healthy-server"}};
  const f=await fixture({mcpConfig:config,mcpServers:[{name:"broken",status:"failed",tools:[],error:{kind:"configuration",message:"command must be a string"}},{name:"healthy",status:"ready",isConnected:true,toolsEnumerated:true,tools:[{name:"read"}]}]});try{
    await f.click("Tools & skills");assert.match(f.w.document.querySelector('[data-server="broken"]').textContent,/command must be a string/);assert.equal(f.w.document.querySelector('[data-server="healthy"] [data-mcp-action="refresh"]').hidden,false);
    await f.click("Advanced JSON");const textarea=f.w.document.querySelector("dialog textarea");const next=JSON.parse(textarea.value);next.mcpServers.healthy.args=["--fixed"];textarea.value=JSON.stringify(next);f.w.document.querySelector("dialog form").dispatchEvent(new f.w.Event("submit",{bubbles:true,cancelable:true}));await wait();
    const saved=f.calls.find(c=>c.member==="setMcpServerConfigs");assert.equal(saved.args[0].broken.command,42);assert.equal(saved.args[0].healthy.args[0],"--fixed");
  }finally{f.close();}
});

test("MCP disabled-tool save failures restore the checkbox and leave an actionable error",async()=>{
  const f=await fixture({mcpServers:[{name:"fixture",status:"ready",isConnected:true,tools:[{name:"read",enabled:true}]}],mcpActionError:"Could not save tool policy"});try{
    await f.click("Tools & skills");await f.click("View tools for fixture");const toggle=f.w.document.querySelector('[aria-label="Enable read on fixture"]');toggle.click();await wait();assert.equal(toggle.checked,true);assert.equal(toggle.disabled,false);assert.match(f.w.document.querySelector("#toasts").textContent,/Could not save tool policy/);
  }finally{f.close();}
});

test("MCP server response failures keep form values and allow a deliberate retry",async()=>{
  const f=await fixture({mcpInteractions:[pendingRequest()],interactionError:"The request expired on the server"});try{
    await f.click("Review request");const input=f.w.document.querySelector('dialog input[type="text"]');input.value="Chess club";await f.click("Send response");assert.equal(input.value,"Chess club");assert.match(f.w.document.querySelector('dialog [role="alert"]').textContent,/expired/);assert.ok(f.w.document.querySelector("dialog"));
  }finally{f.close();}
});

test("MCP titled enums and multiple choices submit their wire values, not display labels",async()=>{
  const request=pendingRequest({requestedSchema:{type:"object",properties:{level:{type:"string",oneOf:[{const:"basic",title:"Getting started"},{const:"advanced",title:"Experienced"}]},topics:{type:"array",items:{anyOf:[{const:"chess",title:"Chess games"},{const:"go",title:"Go games"}]}}},required:["level","topics"]}});
  const f=await fixture({mcpInteractions:[request]});try{
    await f.click("Review request");const modal=f.w.document.querySelector("dialog");assert.match(modal.textContent,/Getting started/);modal.querySelector("select").value="1";modal.querySelector('fieldset input').checked=true;await f.click("Send response");const response=f.calls.find(c=>c.member==="respondInteraction");assert.deepEqual(JSON.parse(JSON.stringify(response.args[2])),{level:"advanced",topics:["chess"]});
  }finally{f.close();}
});

test("MCP automatic retry countdown and exhausted circuit offer clear recovery",async()=>{
  const servers=[{name:"fixture",status:"failed",isConnected:false,tools:[],retryAttempt:2,nextRetryAt:Date.now()+10000,error:{message:"Connection closed"}}];const f=await fixture({mcpServers:servers});try{
    await f.click("Tools & skills");assert.match(f.w.document.querySelector(".mcp-retry-status").textContent,/Retrying in \d+s · attempt 3/);
    Object.assign(servers[0],{status:"disconnected",nextRetryAt:0,circuitOpen:true});f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-updated",payload:{}}}));await wait();
    assert.match(f.w.document.querySelector(".mcp-retry-status").textContent,/Automatic retries paused/);const reconnect=f.w.document.querySelector('[aria-label="Reconnect fixture"]');assert.equal(reconnect.hidden,false);await f.click("Reconnect fixture");assert.ok(f.calls.some(c=>c.member==="reconnect"));
  }finally{f.close();}
});

test("MCP catalog disclosure survives policy-triggered reconnect and its transient empty catalog",async()=>{
  const tools=[{name:"read",enabled:true}];const servers=[{name:"fixture",status:"ready",isConnected:true,tools}];const f=await fixture({mcpServers:servers});try{
    await f.click("Tools & skills");await f.click("View tools for fixture");f.w.document.querySelector(".mcp-tool").open=true;
    Object.assign(servers[0],{status:"initializing",tools:[]});f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-updated",payload:{}}}));await wait();
    Object.assign(servers[0],{status:"ready",tools});f.w.dispatchEvent(new f.w.CustomEvent("quixi:event",{detail:{channel:"quixi:mcp-updated",payload:{}}}));await wait();assert.equal(f.w.document.querySelector(".mcp-tool").open,true);
  }finally{f.close();}
});


test("folderless tasks never invent a workspace and edit creates a linked branch", async()=>{
 const f=await fixture();try{
  f.type("#prompt","Original club plan");await f.click("Send");
  const start=f.calls.find(c=>c.member==="start");assert.equal(start.args[0].cwd,"");assert.equal(start.args[0].userSelectedFolders.length,0);
  await f.click("Stop");await f.click("Edit");
  const input=f.type('textarea[name="text"]',"Revised club plan");input.closest("form").dispatchEvent(new f.w.Event("submit",{bubbles:true,cancelable:true}));await wait();
  const fork=f.calls.find(c=>c.member==="forkMessage");assert.equal(fork.args[1].text,"Revised club plan");assert.equal(f.sessions.find(s=>s.sessionId==="fixture-new").bufferedMessages[0].message.content[0].text,"Original club plan");
  assert.match(f.w.document.querySelector("#conversation").textContent,/Revised club plan/);await f.click("Original conversation");assert.match(f.w.document.querySelector("#conversation").textContent,/Original club plan/);
 }finally{f.close();}
});

test("personal and project drafts stay separate across project navigation",async()=>{
 const f=await fixture();try{
  f.type("#prompt","Personal draft");await f.click("Projects");await f.click("Open project");await f.click("Start task");
  assert.equal(f.w.document.querySelector("#prompt").value,"");f.type("#prompt","Project draft");await f.click("New task");assert.equal(f.w.document.querySelector("#prompt").value,"Personal draft");
  await f.click("Projects");await f.click("Open project");await f.click("Start task");assert.equal(f.w.document.querySelector("#prompt").value,"Project draft");
 }finally{f.close();}
});

test("unified search finds project instructions and opens the project page",async()=>{
 const f=await fixture();try{
  await f.click("Search");f.type("#search-input","PRIVATE-PROJECT-A");await new Promise(r=>setTimeout(r,350));
  assert.match(f.w.document.querySelector("#search-results").textContent,/Project A/);await f.click("Project A");assert.ok(f.w.document.querySelector(".project-context"));
 }finally{f.close();}
});


test("late submission does not clear another conversation draft",async()=>{
 let release;const gate=new Promise(r=>{release=r;});const f=await fixture({sendGate:gate});try{
  f.type("#prompt","First submitted message");f.w.document.querySelector("#send-button").click();await wait();
  await f.click("Existing conversation");f.type("#prompt","Keep this other draft");release();await wait();await wait();
  assert.match(f.w.location.pathname,/fixture-one/);assert.equal(f.w.document.querySelector("#prompt").value,"Keep this other draft");
 }finally{release();f.close();}
});

test("recents offer direct deletion with cancel and confirmed removal", async () => {
  const f = await fixture();
  try {
    await f.click("Delete conversation: Existing conversation");
    assert.match(f.w.document.querySelector("dialog").textContent, /cannot be undone/);
    await f.click("Cancel");
    assert.equal(f.sessions.length, 1);
    assert.equal(f.calls.filter(c => c.member === "delete").length, 0);
    await f.click("Delete conversation: Existing conversation");
    await f.click("Delete");
    assert.equal(f.sessions.length, 0);
    assert.equal(f.w.document.querySelectorAll(".recent-row").length, 0);
    assert.equal(f.calls.filter(c => c.member === "delete").length, 1);
  } finally { f.close(); }
});

test("project deletion is direct and preserves conversations", async () => {
  const f = await fixture();
  try {
    await f.click("Projects");
    await f.click("Delete project: Project A");
    assert.match(f.w.document.querySelector("dialog").textContent, /Conversations and files.*kept/);
    await f.click("Cancel");
    assert.equal(f.projects.length, 1);
    await f.click("Open project");
    await f.click("Delete project");
    await f.click("Delete");
    assert.equal(f.projects.length, 0);
    assert.equal(f.sessions.length, 1);
    assert.equal(f.w.document.body.dataset.page, "projects");
  } finally { f.close(); }
});

test("search results and open conversations expose direct delete controls", async () => {
  const f = await fixture();
  try {
    await f.click("Search");
    assert.ok(f.w.document.querySelector("#search-results [aria-label=\"Delete conversation: Existing conversation\"]"));
    assert.ok(f.w.document.querySelector("#search-results [aria-label=\"Delete project: Project A\"]"));
    await f.click("Existing conversation");
    await f.click("Delete conversation");
    await f.click("Delete");
    assert.equal(f.w.document.body.dataset.page, "new");
    assert.equal(f.sessions.length, 0);
  } finally { f.close(); }
});

test("composer follow-up controls are visually hidden when idle and track only the selected session", async () => {
  const f = await fixture();
  try {
    // Load real author styles: checking .hidden alone misses button display overrides.
    const style = f.w.document.createElement("style");
    style.textContent = fs.readFileSync(path.join(__dirname, "../../resources/ui/app.css"), "utf8");
    f.w.document.head.append(style);
    const assertControls = (active) => {
      for (const id of ["stop-button", "send-options"]) {
        const control = f.w.document.getElementById(id);
        assert.ok(control, id);
        assert.equal(control.hidden, !active, id + " hidden state");
        assert.equal(f.w.getComputedStyle(control).display === "none", !active, id + " computed visibility");
      }
      assert.equal(f.w.document.getElementById("send-button").textContent, active ? "Queue" : "Send");
    };
    const notify = async (session, active, type) => {
      session.isRunning = active;
      f.w.dispatchEvent(new f.w.CustomEvent("quixi:event", {
        detail: {channel: "LocalAgentModeSessions", payload: {sessionId: session.sessionId, type}},
      }));
      await new Promise(resolve => setTimeout(resolve, 100));
    };
    assertControls(false); // New task.
    await f.click("Existing conversation");
    assertControls(false); // Existing idle session.
    f.type("#prompt", "Start work");
    await f.click("Send");
    assertControls(true);
    await f.click("New task");
    assertControls(false); // Another session is still running.
    await notify(f.sessions[0], true, "update");
    assertControls(false); // Background updates must not expose controls.
    await f.click("Existing conversation");
    assertControls(true);
    await notify(f.sessions[0], false, "close");
    assertControls(false); // Natural completion without a full composer render.
    await notify(f.sessions[0], true, "update");
    assertControls(true);
    await notify(f.sessions[0], false, "error");
    assertControls(false);
    f.type("#prompt", "Restart work");
    await f.click("Send");
    assertControls(true);
    await f.click("Stop");
    assertControls(false);
  } finally { f.close(); }
});

test("active composer sends steering immediately and queues followups without stopping", async () => {
  const f = await fixture();
  try {
    f.sessions[0].isRunning = true;
    await f.click("Existing conversation");
    f.type("#prompt", "Use the smaller change instead");
    f.w.document.querySelector("#send-options").open = true;
    await f.click("Guide current task");
    let sent = f.calls.filter(c => c.member === "sendMessageV2");
    assert.equal(sent.length, 1);
    assert.equal(sent[0].args[1].delivery, "steer");
    assert.equal(f.w.document.querySelector("#prompt").value, "");
    assert.match(f.w.document.querySelector("#pending-messages").textContent, /Guidance pending/);
    f.type("#prompt", "Then write a summary");
    await f.click("Queue");
    sent = f.calls.filter(c => c.member === "sendMessageV2");
    assert.equal(sent.length, 2);
    assert.equal(sent[1].args[1].delivery, "queue");
    assert.match(f.w.document.querySelector("#pending-messages").textContent, /Then write a summary/);
    assert.equal(f.calls.filter(c => c.member === "stop").length, 0);
    assert.equal(f.sessions[0].bufferedMessages.length, 0);
    f.type("#prompt", "One more followup").dispatchEvent(new f.w.KeyboardEvent("keydown", {key:"Enter", bubbles:true}));
    await wait();
    assert.equal(f.calls.filter(c => c.member === "sendMessageV2").length, 3);
    await f.click("Stop");
    assert.match(f.w.document.querySelector("#pending-messages").textContent, /On hold/);
    assert.equal(f.w.document.querySelector("#send-options").hidden, true);
    assert.equal(f.w.document.querySelectorAll(".pending-message").length, 3);
    await f.click("Remove");
    assert.equal(f.w.document.querySelectorAll(".pending-message").length, 2);
    await f.click("Send now");
    assert.equal(f.w.document.querySelectorAll(".pending-message").length, 1);
    assert.match(f.w.document.querySelector("#pending-messages").textContent, /On hold/);
  } finally { f.close(); }
});

test("queued cards can guide the current task or be removed without stopping", async () => {
  const f = await fixture();
  try {
    f.sessions[0].isRunning = true;
    await f.click("Existing conversation");
    f.type("#prompt", "A queued correction");
    await f.click("Queue");
    await f.click("Guide task");
    assert.match(f.w.document.querySelector("#pending-messages").textContent, /Guidance pending/);
    assert.equal(f.calls.filter(c => c.member === "guidePendingMessage").length, 1);
    await f.click("Remove");
    assert.equal(f.w.document.querySelectorAll(".pending-message").length, 0);
    assert.equal(f.calls.filter(c => c.member === "stop").length, 0);
    assert.equal(f.w.document.querySelector("#send-button").textContent, "Queue");
  } finally { f.close(); }
});

test("internal delegation failures are returned to parent, not user attention alerts", async () => {
  const diagnostic = "Subagent context budget reached; narrow the assignment.";
  const f = await fixture({subagents:[{agentId:"child-budget",title:"Inspect backend",status:"failed",error:diagnostic,toolCalls:9,failedToolCalls:2,messages:[]}]});
  try {
    await f.click("Existing conversation");
    const panel = f.w.document.querySelector(".subagent-panel");
    assert.match(panel.textContent, /Returned to agent/);
    assert.match(panel.textContent, /Returned to parent agent/);
    assert.doesNotMatch(panel.textContent, /Needs attention|context budget|2 failed/);
    assert.equal(panel.querySelectorAll(".error-text").length, 0);
    await f.click("View activity");
    assert.match(f.w.document.querySelector("dialog").textContent, /Internal diagnostic/);
    assert.match(f.w.document.querySelector("dialog").textContent, /context budget/);
  } finally { f.close(); }
});

test("markdown safely handles separators and malformed streaming input", async () => {
 const f = await fixture();
 try {
  for (const text of ["# title\u2028- item\u2029> quote", "<script>alert(1)</script>", "*".repeat(30000), "[".repeat(30000), "```js\nconst a = 1", "**unfinished", "[unfinished]("]) {
   const body = f.w.testPlainBody(text);
   assert.ok(body.childNodes.length);
   assert.equal(body.querySelector("script"), null);
  }
  const large = "x".repeat(200001);
  assert.equal(f.w.testPlainBody(large).querySelector("pre").textContent, large);
  assert.ok(f.w.testPlainBody("[safe](https://example.com)").querySelector("a"));
  assert.equal(f.w.testPlainBody("[bad](javascript:evil)").querySelector("a"), null);
 } finally { f.close(); }
});

test("clipboard images support WebKit files fallback without duplicates", async () => {
 const f = await fixture();
 try {
  const file = new f.w.File(["png"], "paste.png", {type:"image/png"});
  const text = new f.w.File(["text"], "note.txt", {type:"text/plain"});
  const get = f.w.QuixiChat.clipboardImages;
  assert.equal(get({items:[{kind:"file",type:"image/png",getAsFile:()=>file}],files:[file]}).length, 1);
  assert.equal(get({files:[file,text]})[0], file);
  assert.equal(get(null).length, 0);
 } finally { f.close(); }
});


test("pasting images queues validated attachments and preserves accompanying text", async () => {
 const f = await fixture();
 try {
  const bytes = new Uint8Array([137,80,78,71,13,10,26,10]);
  const file = new f.w.File([bytes], "clipboard.png", {type:"image/png"});
  file.arrayBuffer = async () => bytes.buffer;
  const prompt = f.type("#prompt", "Before After");
  prompt.setSelectionRange(7, 7);
  const event = new f.w.Event("paste", {bubbles:true, cancelable:true});
  Object.defineProperty(event, "clipboardData", {value:{files:[file],getData:()=>"image "}});
  prompt.dispatchEvent(event);
  assert.equal(event.defaultPrevented, true);
  assert.equal(prompt.value, "Before image After");
  await wait();
  assert.match(f.w.document.body.textContent, /clipboard.png/);
  await f.click("Send");
  const sent = f.calls.find(c => c.member === "sendMessageV2");
  assert.ok(sent);
  assert.match(JSON.stringify(sent.args), /image\/png/);
  assert.match(JSON.stringify(sent.args), /iVBORw0KGgo=/);
 } finally { f.close(); }
});

test("ordinary clipboard text retains native paste behavior", async () => {
 const f = await fixture();
 try {
  const event = new f.w.Event("paste", {bubbles:true, cancelable:true});
  Object.defineProperty(event, "clipboardData", {value:{files:[],getData:()=>"plain text"}});
  f.w.document.querySelector("#prompt").dispatchEvent(event);
  assert.equal(event.defaultPrevented, false);
 } finally { f.close(); }
});
