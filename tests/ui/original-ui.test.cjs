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

async function fixture() {
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
  const providers = {
    configuredCount: 1,
    hasConfiguredProvider: true,
    activeProvider: "custom",
    activeModel: "fixture-model",
    providers: [
      {
        id: "custom",
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
    if (member === "getAllSpaces")
      return [
        {
          id: "project-a",
          name: "Project A",
          instructions: "PRIVATE-PROJECT-A",
          folders: [{ path: "/project-a" }],
        },
      ];
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
    if (route === "/api/quixi/providers") return providers;
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
      return { exists: true, path: body.path };
    if (route === "/api/quixi/providers/save") return { ok: true };
    throw Error("Unexpected API: " + route);
  };
  w.quixi = { rpc, api, copy: async () => {}, origin: "https://app.localhost" };
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
  return { w, sessions, calls, click, type, close: () => dom.window.close() };
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
      "QuixiDeskWORKSPACE",
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
