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

async function fixture({ artifacts = [], providerId = "custom" } = {}) {
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
    if (member === "getAllArtifacts") return structuredClone(artifacts);
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
      return { ok: true };
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
  test(`manual models can be saved without switching and then selected: ${providerId}`, async () => {
    const f = await fixture({ providerId });
    try {
      await f.click("Choose model");
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
