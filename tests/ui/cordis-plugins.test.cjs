// Copyright (c) 2026 QuixiAI. MIT license.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { JSDOM } = require("jsdom");
const fs = require("node:fs");
const path = require("node:path");

const wait = () => new Promise((resolve) => setTimeout(resolve, 45));

async function createCordisUiFixture() {
  const dom = new JSDOM(
    '<!doctype html><html data-theme="dark"><body><div id="app"></div><div id="toasts"></div></body></html>',
    {
      url: "https://app.localhost/new",
      runScripts: "outside-only",
      pretendToBeVisual: true,
    },
  );
  const w = dom.window;
  w.TextEncoder = TextEncoder;
  w.matchMedia = () => ({ matches: true, addEventListener() {} });
  w.HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  w.HTMLDialogElement.prototype.close = function () {
    this.open = false;
    this.dispatchEvent(new w.Event("close"));
  };
  w.URL.createObjectURL = () => "blob:https://app.localhost/mock-zip";
  w.URL.revokeObjectURL = () => {};

  const calls = [];
  const cordisState = {
    profile: "default",
    hmr: true,
    defaultPreset: "standard",
    activeRowIds: [
      "system-prompt",
      "tools-registry",
      "tool-web",
      "ui-views",
      "ui-secret-mask",
      "forge-github",
      "action-cards",
    ],
    activeClientBundles: [],
    fibers: [
      { id: "ui-views", state: { phase: "active" } },
      { id: "ui-secret-mask", state: { phase: "active" } },
    ],
    bundles: [
      {
        id: "quixi-experimental-ui-pack",
        version: "3.6556.16",
        title: "Trajectory, Statistics & Secret Masking UI",
        description: "Client slot views and credential masking.",
        category: "Experimental",
        official: true,
        experimental: true,
        listedByDefault: true,
        enabled: true,
        locked: false,
        removable: false,
        source: "built-in",
        hasReadme: true,
        rows: [
          {
            id: "ui-views",
            plugin: "dsh-client-ui-views",
            title: "Trajectory, Statistics & Turn Rail",
            description: "Alternate conversation tabs",
            enabled: true,
            locked: false,
            phase: "active",
            error: null,
            config: { defaultTranscriptView: "standard" },
            configSchema: {
              fields: [{ key: "defaultTranscriptView", label: "Default mode", type: "string" }],
            },
          },
          {
            id: "ui-secret-mask",
            plugin: "dsh-client-ui-secret-mask",
            title: "User Bubble Credential Masking",
            description: "Masks API keys in user bubbles",
            enabled: true,
            locked: false,
            phase: "active",
            error: null,
            config: {},
          },
        ],
      },
    ],
  };

  const sessions = [
    {
      sessionId: "sess-cordis-1",
      title: "Kernel Audit",
      isRunning: false,
      isArchived: false,
      isStarred: false,
      cwd: "/workspace/repo",
      effortLevel: "medium",
      permissionMode: "auto",
      lastActivityAt: 500,
      bufferedMessages: [
        {
          type: "user",
          uuid: "u-1",
          message: {
            role: "user",
            content: [{ type: "text", text: "Check AWS key AKIAIOSFODNN7EXAMPLE please" }],
          },
        },
        {
          type: "assistant",
          uuid: "a-1",
          message: {
            role: "assistant",
            content: [{ type: "text", text: "Checked the configuration safely." }],
          },
        },
      ],
    },
  ];

  const subagentsBySession = {};
  const pendingBySession = {};
  const artifactsList = [];
  const collabBySession = {
    "sess-cordis-1": {
      sessionId: "sess-cordis-1",
      agentPreset: "creator",
      todos: [],
      planMode: { active: false },
      questions: [],
      deliverables: [],
      worktree: {
        name: "wt-1",
        path: "/workspace/repo/.agents/worktrees/wt-1",
        branch: "quixi/wt-1",
        originCwd: "/workspace/repo",
      },
      forgePrs: [
        {
          number: 42,
          title: "Adopt Cordis Spatiotemporal Spine",
          url: "https://github.com/QuixiAI/QuixiDesk/pull/42",
          state: "OPEN",
          mergeable: "MERGEABLE",
          headRef: "feat/cordis",
          baseRef: "main",
          checksPassed: 6,
          checksFailed: 0,
          checksPending: 0,
        },
      ],
      actionCards: [
        {
          id: "card-1",
          action: "create_personalized_page",
          title: "Latency Radar",
          brief: "Right-sidebar step latency widget.",
          details: "# Latency Radar\nLive p95 chart.",
          status: "pending",
          placement: "sidebar-right",
        },
      ],
      stepTelemetry: [
        {
          turnIndex: 0,
          stepIndex: 0,
          model: "deepseek-chat",
          ttftMs: 180,
          modelMs: 620,
          toolMs: 95,
          inputTokens: 1200,
          outputTokens: 140,
          cacheReadTokens: 3600,
          cacheWriteTokens: 0,
          reasoningTokens: 0,
          contextTokens: 4940,
          toolNames: ["RunCode"],
          ptcSubcalls: [{ id: "sub_1", tool: "Read", durationMs: 12 }],
        },
      ],
      team: { enabled: false, tasks: [] },
      contextMeter: {
        inputTokens: 1200,
        outputTokens: 140,
        cacheReadTokens: 3600,
        projectedTokens: 4940,
        contextWindow: 128000,
        compactions: 0,
      },
    },
  };

  const providers = {
    configuredCount: 1,
    hasConfiguredProvider: true,
    activeProvider: "custom",
    activeModel: "deepseek-chat",
    providers: [
      {
        id: "custom",
        name: "Custom",
        configured: true,
        selectedModel: "deepseek-chat",
        baseUrl: "http://127.0.0.1:9999/v1",
        apiFormat: "openai",
        models: [{ id: "deepseek-chat", label: "DeepSeek Chat" }],
      },
    ],
  };

  const rpc = async (service, member, args = []) => {
    calls.push({ service, member, args });
    if (service === "CordisPlugins") {
      if (member === "snapshot") return structuredClone(cordisState);
      if (member === "reportClientManifest") return { ok: true };
      if (member === "listPresets") {
        return {
          defaultPreset: cordisState.defaultPreset,
          presets: [
            { id: "standard", title: "Standard", badge: "Default", guideMarkdown: "Standard guide" },
            { id: "ptc", title: "PTC (RunCode)", badge: "Code SDK", guideMarkdown: "PTC guide" },
            { id: "minimal", title: "Minimal", badge: "SWE-Bench", guideMarkdown: "Minimal guide" },
            { id: "creator", title: "Creator", badge: "Cordis", guideMarkdown: "Creator guide" },
          ],
        };
      }
      if (member === "readPresetYaml") return { id: args[0], yaml: `preset: ${args[0]}\n` };
      if (member === "setPluginEnabled") {
        const [rowId, enabled] = args;
        for (const b of cordisState.bundles) {
          for (const r of b.rows) {
            if (r.id === rowId) {
              r.enabled = enabled;
              r.phase = enabled ? "active" : "inactive";
            }
          }
        }
        if (enabled) {
          if (!cordisState.activeRowIds.includes(rowId)) cordisState.activeRowIds.push(rowId);
        } else {
          cordisState.activeRowIds = cordisState.activeRowIds.filter((x) => x !== rowId);
        }
        return structuredClone(cordisState);
      }
      if (member === "installUserBundle") {
        const { manifest, clientJs } = args[0];
        cordisState.bundles.push({
          ...manifest,
          enabled: true,
          removable: true,
          source: "local",
          hasReadme: true,
          rows: manifest.rows.map((r) => ({ ...r, enabled: true, phase: "active" })),
        });
        cordisState.activeClientBundles.push({
          id: manifest.id,
          title: manifest.title,
          version: manifest.version,
          clientScript: clientJs,
          declarative: manifest.declarative,
        });
        return structuredClone(cordisState);
      }
    }
    if (member === "getAll") return service === "LocalSessions" ? [] : structuredClone(sessions);
    if (member === "getAllSpaces") return [];
    if (member === "getSession") return structuredClone(sessions.find((s) => s.sessionId === args[0]));
    if (member === "getCollabState") return structuredClone(collabBySession[args[0]] || null);
    if (member === "searchWorkspaceFiles") {
      const [, query] = args;
      const all = [
        { name: "src", relativePath: "src", path: "/workspace/repo/src", isDir: true },
        { name: "main.rs", relativePath: "src/main.rs", path: "/workspace/repo/src/main.rs", isDir: false },
        { name: "README.md", relativePath: "README.md", path: "/workspace/repo/README.md", isDir: false },
      ];
      return all.filter((f) => !query || f.relativePath.includes(query));
    }
    if (member === "listDirectoryAtCwd") {
      const [dir] = args;
      if (dir === "/workspace/repo/src") {
        return [{ name: "main.rs", path: "/workspace/repo/src/main.rs", isDir: false }];
      }
      return [
        { name: "src", path: "/workspace/repo/src", isDir: true },
        { name: "README.md", path: "/workspace/repo/README.md", isDir: false },
      ];
    }
    if (member === "getWorkingTreeStatus") {
      return {
        files: [{ path: "src/main.rs", status: "modified", additions: 2, deletions: 1 }],
        additions: 2,
        deletions: 1,
      };
    }
    if (member === "manageGoal") {
      const [sid, action, payload] = args;
      if (action === "edit" && collabBySession[sid]?.goal) {
        collabBySession[sid].goal.objective = payload.objective;
        collabBySession[sid].goal.maxGoalRounds = payload.max_goal_rounds;
        collabBySession[sid].goal.revision += 1;
      }
      return { ok: true, goal: collabBySession[sid]?.goal };
    }
    if (member === "listSubagents") return structuredClone(subagentsBySession[args[0]] || []);
    if (member === "setOutputStyle") {
      const [sid, style, instr] = args;
      if (collabBySession[sid]) {
        collabBySession[sid].outputStyle = style;
        collabBySession[sid].outputStyleInstructions = instr || null;
      }
      return structuredClone(collabBySession[sid] || null);
    }
    if (member === "getPendingMessages") {
      return structuredClone(pendingBySession[args[0]] || []);
    }
    if (member === "reorderPendingMessage") {
      const [sid, uuid] = args;
      const q = pendingBySession[sid] || [];
      const idx = q.findIndex((m) => m.messageUuid === uuid);
      if (idx > 0) {
        const [item] = q.splice(idx, 1);
        q.unshift(item);
      }
      return { reordered: true, index: 0, messageUuid: uuid };
    }
    if (member === "cancelPendingMessage") {
      const [sid, uuid] = args;
      const q = pendingBySession[sid] || [];
      const idx = q.findIndex((m) => m.messageUuid === uuid);
      const cancelled = idx >= 0;
      if (cancelled) q.splice(idx, 1);
      return { cancelled, messageUuid: uuid };
    }
    if (member === "listJobs" || member === "getToolPermissions") return [];
    if (member === "start") {
      const created = {
        sessionId: args[0].isArchived ? "sess-aside-1" : "sess-started",
        title: args[0].title,
        isRunning: !args[0].isArchived,
        isArchived: Boolean(args[0].isArchived),
        isStarred: false,
        cwd: args[0].cwd || "",
        effortLevel: args[0].effortLevel || "medium",
        permissionMode: args[0].permissionMode || "default",
        bufferedMessages: [],
        lastActivityAt: 999,
      };
      sessions.unshift(created);
      return created;
    }
    if (member === "sendMessageV2") {
      const target = sessions.find((x) => x.sessionId === args[0]);
      if (target?.isArchived) {
        target.bufferedMessages.push(
          { type: "user", message: { content: [{ type: "text", text: args[1].text }] } },
          { type: "assistant", message: { content: [{ type: "text", text: "Side chat answer: use WKWebView userScript." }] } },
        );
        target.isRunning = false;
      }
      return { ok: true };
    }
    if (member === "steerAllPending") return { steered: 2 };
    if (member === "stop") {
      const s = sessions.find((x) => x.sessionId === args[0]);
      if (s) s.isRunning = false;
      return { ok: true };
    }
    if (member === "forkSessionBranch") {
      const branched = {
        ...sessions[0],
        sessionId: "sess-branched-1",
        title: "Kernel Audit (1)",
      };
      sessions.unshift(branched);
      return { sessionId: branched.sessionId, title: branched.title };
    }
    if (member === "resolveActionCard") {
      const [sid, cardId, decision] = args;
      const card = collabBySession[sid].actionCards.find((c) => c.id === cardId);
      card.status = decision === "undo" ? "undone" : "accepted";
      if (card.status === "accepted") {
        cordisState.activeClientBundles = [
          {
            id: "latency-radar",
            title: "Latency Radar",
            version: "1.0.0",
            clientScript: `window.QuixiCordis?.definePlugin({ id: "latency-radar", setup(ctx){ ctx.registerPersonalizedSurface({ id: "latency-radar", title: "Latency Radar", details: "p95 ok", placement: "sidebar-right" }); } });`,
          },
        ];
      } else {
        cordisState.activeClientBundles = [];
      }
      return { ok: true, card };
    }
    if (member === "getAllArtifacts") return structuredClone(artifactsList);
    if (
      member === "listLocalSkills" ||
      member === "getLocalMcpServers" ||
      member === "getAllScheduledTasks"
    ) {
      return [];
    }
    return { ok: true };
  };

  const api = async (route, body) => {
    calls.push({ route, body });
    if (route === "/api/quixi/providers") return structuredClone(providers);
    if (route === "/api/quixi/workspace/read") {
      return {
        path: body?.path,
        contents: body?.path?.endsWith(".md")
          ? "# Repo Guide\n\nFast native workspace."
          : "fn main() {\n    println!(\"target line\");\n}\n",
      };
    }
    if (route === "/api/quixi/workspace/diff") {
      return {
        path: "/workspace/repo/src/main.rs",
        relativePath: "src/main.rs",
        status: "modified",
        added: 2,
        deleted: 1,
        binary: false,
        hunks: [
          {
            header: "@@ -1,3 +1,4 @@",
            oldStart: 1,
            newStart: 1,
            lines: [
              { kind: "ctx", oldLine: 1, newLine: 1, text: "fn main() {" },
              { kind: "del", oldLine: 2, newLine: null, text: "    println!(\"old\");" },
              { kind: "add", oldLine: null, newLine: 2, text: "    println!(\"new 1\");" },
              { kind: "add", oldLine: null, newLine: 3, text: "    println!(\"new 2\");" },
            ],
          },
        ],
      };
    }
    if (route.startsWith("/api/quixi/artifacts/")) {
      const id = decodeURIComponent(route.slice("/api/quixi/artifacts/".length));
      return structuredClone(artifactsList.find((a) => a.id === id) || { error: "Not found" });
    }
    return { ok: true };
  };

  w.quixi = {
    rpc,
    api,
    copy: async (text) => calls.push({ clipboard: text }),
    origin: "https://app.localhost",
  };

  for (const name of [
    "cordis-runtime",
    "cordis-ui-plugins",
    "workspace-workflows",
    "schedule-workflows",
    "capability-workflows",
    "chat-workflows",
    "app",
  ]) {
    const file = path.join(__dirname, "../../resources/ui/" + name + ".js");
    w.eval(fs.readFileSync(file, "utf8"));
  }
  await wait();
  return { w, calls, sessions, collabBySession, subagentsBySession, pendingBySession, artifactsList, cordisState, close: () => dom.window.close() };
}

test("Sidebar Plugins page toggles Cordis fibers live and unmounts/remounts UI slots without reload", async () => {
  const f = await createCordisUiFixture();
  const { w, calls } = f;
  try {

  // Open existing conversation: `ui-views` and `ui-secret-mask` are active!
  const convBtn = Array.from(w.document.querySelectorAll("button")).find((b) =>
    b.textContent.includes("Kernel Audit"),
  );
  convBtn.click();
  await wait();

  // 1. Secret masking waterfall masked `AKIAIOSFODNN7EXAMPLE`:
  const convText = w.document.getElementById("conversation").textContent;
  assert.ok(!convText.includes("AKIAIOSFODNN7EXAMPLE"), "AWS key must be masked while ui-secret-mask is active");
  assert.ok(convText.includes("AKIAIO••••••••PLE"));

  // 2. Trajectory and Statistics tabs are mounted by `ui-conversation-views`:
  const trajTab = Array.from(w.document.querySelectorAll('[role="tab"]')).find(
    (b) => b.textContent.trim() === "Trajectory",
  );
  assert.ok(trajTab, "Trajectory tab should be mounted");
  trajTab.click();
  await wait();
  assert.ok(
    w.document.querySelector('[aria-label="Trajectory view"]').textContent.includes("RunCode"),
    "Trajectory view renders step & PTC RunCode telemetry",
  );

  const statsTab = Array.from(w.document.querySelectorAll('[role="tab"]')).find(
    (b) => b.textContent.trim() === "Statistics",
  );
  statsTab.click();
  await wait();
  assert.ok(
    w.document.querySelector('[aria-label="Statistics view"]').textContent.includes("75%"),
    "Statistics view renders cache hit % and step table",
  );

  // 3. Navigate to Sidebar -> Plugins and live-disable `ui-views`:
  const pluginsNav = Array.from(w.document.querySelectorAll("nav button")).find(
    (b) => b.getAttribute("aria-label") === "Plugins",
  );
  assert.ok(pluginsNav, "Sidebar Plugins button must exist");
  pluginsNav.click();
  await wait();

  // Switch to Experimental tab and toggle off `ui-views`:
  const expTab = Array.from(w.document.querySelectorAll('[role="tab"]')).find(
    (b) => b.textContent.trim() === "Experimental",
  );
  expTab.click();
  await wait();

  const viewsToggle = w.document.querySelector(
    'input[aria-label="Enable plugin Trajectory, Statistics & Turn Rail"]',
  );
  assert.ok(viewsToggle);
  viewsToggle.checked = false;
  viewsToggle.dispatchEvent(new w.Event("change"));
  await wait();

  assert.ok(calls.some((c) => c.service === "CordisPlugins" && c.member === "setPluginEnabled" && c.args[0] === "ui-views"));

  // Return to conversation: Trajectory/Statistics tab bar is cleanly unmounted without reload!
  const convBtn2 = Array.from(w.document.querySelectorAll("button")).find((b) =>
    b.textContent.includes("Kernel Audit"),
  );
  convBtn2.click();
  await wait();
  const trajAfterUnload = Array.from(w.document.querySelectorAll('[role="tab"]')).find(
    (b) => b.textContent.trim() === "Trajectory",
  );
  assert.equal(trajAfterUnload, undefined, "Disabling ui-views row must unmount Trajectory tab immediately");
  } finally {
    f.close();
  }
});

test("Composer Effort & Agent Preset controls, Action Cards, Forge PR pill, Esc-Esc stop, and Branching work", async () => {
  const f = await createCordisUiFixture();
  const { w, calls, sessions } = f;
  try {

  // 1. Select Effort = high and Preset = ptc on New Task, then send:
  const effortSel = w.document.querySelector('select[aria-label="Reasoning effort"]');
  const presetSel = w.document.querySelector('select[aria-label="Agent preset"]');
  assert.ok(effortSel && presetSel);
  effortSel.value = "high";
  effortSel.dispatchEvent(new w.Event("change"));
  presetSel.value = "ptc";
  presetSel.dispatchEvent(new w.Event("change"));
  await wait();

  const prompt = w.document.getElementById("prompt");
  prompt.value = "Audit kernel fibers";
  prompt.dispatchEvent(new w.Event("input"));
  w.document.getElementById("send-button").click();
  await wait();

  const startCall = calls.find((c) => c.member === "start");
  assert.equal(startCall.args[0].effortLevel, "high");
  assert.equal(startCall.args[0].agentPreset, "ptc");

  // 2. While session is running: Cmd+Enter on empty prompt steers all queued messages, and Esc Esc stops turn!
  const activePrompt = w.document.getElementById("prompt");
  activePrompt.value = "";
  activePrompt.dispatchEvent(
    new w.KeyboardEvent("keydown", { key: "Enter", metaKey: true, bubbles: true }),
  );
  await wait();
  assert.ok(calls.some((c) => c.member === "steerAllPending"));

  activePrompt.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  activePrompt.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  await wait();
  assert.ok(calls.some((c) => c.member === "stop"));

  // 3. Open `Kernel Audit` session: verify Forge PR #42 pill, Action Card live mount + Undo, and Branching!
  sessions[0].isRunning = false;
  const kernelBtn = Array.from(w.document.querySelectorAll("button")).find((b) =>
    b.textContent.includes("Kernel Audit"),
  );
  kernelBtn.click();
  await wait();

  assert.ok(
    w.document.querySelector(".cordis-pr-pill")?.textContent.includes("PR #42"),
    "GitHub Forge PR #42 pill is rendered",
  );

  const createCardBtn = Array.from(w.document.querySelectorAll("button")).find(
    (b) => b.textContent.trim() === "Create & Mount Live",
  );
  assert.ok(createCardBtn, "Action Card Create button is rendered");
  createCardBtn.click();
  await wait();

  // Personalized right-sidebar tab button "Latency Radar" is now live-mounted in session heading!
  assert.ok(
    Array.from(w.document.querySelectorAll(".session-heading button")).some(
      (b) => b.textContent.trim() === "Latency Radar",
    ),
    "Accepting Action Card hot-mounts personalized UI plugin without reload",
  );

  // Branch conversation (`Kernel Audit (1)`):
  const branchBtn = Array.from(w.document.querySelectorAll(".message-actions button")).find(
    (b) => b.textContent.trim() === "Branch",
  );
  assert.ok(branchBtn);
  branchBtn.click();
  await wait();
  assert.ok(calls.some((c) => c.member === "forkSessionBranch"));
  } finally {
    f.close();
  }
});

test("ModelRetryItem and TurnErrorItem render DeepSeek Harness retry/error rows and upgrade legacy 503 JSON prose", async () => {
  const f = await createCordisUiFixture();
  const { w, sessions } = f;
  try {
    sessions[0].bufferedMessages.push(
      {
        type: "llm_retry",
        uuid: "r-1",
        retryId: "chain-503",
        retry: 1,
        maxRetries: 10,
        mode: "normal",
        delayMs: 1000,
        retryState: "scheduled",
        failure: {
          code: "SERVER",
          status: 503,
          message: "Google AI Studio (via OpenRouter, HTTP 503): This model is currently experiencing high demand.",
        },
      },
      {
        type: "llm_retry_started",
        uuid: "r-1-start",
        retryId: "chain-503",
        retry: 1,
      },
      {
        type: "assistant",
        uuid: "legacy-err-1",
        message: {
          role: "assistant",
          content: [
            {
              type: "text",
              text: 'Provider error: OpenRouter returned HTTP 503 for model `google/gemini-4-carbon-eap`: {"error":{"message":"Provider returned error","code":503,"metadata":{"raw":"{\\n \\"error\\": {\\n \\"code\\": 503,\\n \\"message\\": \\"This model is currently experiencing high demand. Spikes in demand are usually temporary.\\"\\n }\\n}","provider_name":"Google AI Studio"}},"user_id":"org_hidden"}',
            },
          ],
        },
      },
    );

    const kernelBtn = Array.from(w.document.querySelectorAll("button")).find((b) =>
      b.textContent.includes("Kernel Audit"),
    );
    kernelBtn.click();
    await wait();

    const retryRow = w.document.querySelector(".dsh-retry-row");
    assert.ok(retryRow, "ModelRetryItem disclosure row must render");
    assert.ok(retryRow.textContent.includes("Retried model request (1/10)"));

    const errorRow = w.document.querySelector(".dsh-turn-error-row");
    assert.ok(errorRow, "TurnErrorItem card must replace legacy Provider error prose");
    assert.ok(errorRow.textContent.includes("Upstream model temporarily unavailable"));
    assert.ok(errorRow.textContent.includes("Google AI Studio (via OpenRouter, HTTP 503)"));
    assert.ok(errorRow.textContent.includes("SERVER"));
    assert.ok(!errorRow.textContent.includes("org_hidden"), "Raw JSON user_id noise must be stripped");
  } finally {
    f.close();
  }
});

test("Input triggers (/ and @), Quick Jump launcher, clickable prose file:line preview, Split/Unified diff review, Goal editor, Subagents pill, and Artifact Studio work", async () => {
  const f = await createCordisUiFixture();
  const { w, calls, sessions, collabBySession, subagentsBySession, artifactsList } = f;
  try {
    sessions[0].bufferedMessages.push({
      type: "assistant",
      uuid: "a-file-ref",
      message: {
        role: "assistant",
        content: [{ type: "text", text: "Updated `src/main.rs:2` and created an interactive artifact." }],
      },
    });
    collabBySession["sess-cordis-1"].goal = {
      id: "goal-1",
      revision: 1,
      objective: "Ship full test coverage",
      phase: "active",
      activation: "armed",
      roundsStarted: 1,
      maxGoalRounds: 10,
    };
    subagentsBySession["sess-cordis-1"] = [
      { id: "sub-1", description: "Audit IPC routes", status: "completed", model: "deepseek-reasoner", output: "All routes verified." },
    ];
    artifactsList.push({
      id: "art-demo-1",
      name: "Metrics Dashboard",
      title: "Metrics Dashboard",
      html: "<!doctype html><html><body><h1>Metrics</h1></body></html>",
      revision: 2,
      updatedAt: 1000,
      createdBySessionId: "sess-cordis-1",
    });

    // Reload artifacts & open conversation:
    const kernelBtn = Array.from(w.document.querySelectorAll("button")).find((b) =>
      b.textContent.includes("Kernel Audit"),
    );
    kernelBtn.click();
    await wait();
    await wait();

    // 1. Subagent lineage pill renders in session heading and opens lineage modal:
    const subBtn = Array.from(w.document.querySelectorAll(".session-heading button")).find((b) =>
      b.textContent.includes("Subagents · 1"),
    );
    assert.ok(subBtn, "Subagent lineage pill must render when child subagents exist");
    subBtn.click();
    await wait();
    assert.match(w.document.querySelector("dialog[open]").textContent, /Audit IPC routes/);
    w.document.querySelector("dialog[open]").close();

    // 2. Goal strip renders Edit button and submits manageGoal("edit", ...):
    const goalEditBtn = Array.from(w.document.querySelectorAll(".goal-bar button")).find(
      (b) => b.textContent.trim() === "Edit",
    );
    assert.ok(goalEditBtn, "Goal bar must offer inline Edit action");
    goalEditBtn.click();
    await wait();
    const objField = w.document.querySelector('dialog[open] textarea[name="objective"]');
    assert.ok(objField);
    objField.value = "Ship 100% deterministic tests";
    objField.closest("form").dispatchEvent(new w.Event("submit", { bubbles: true, cancelable: true }));
    await wait();
    assert.ok(
      calls.some((c) => c.member === "manageGoal" && c.args[1] === "edit" && c.args[2].objective === "Ship 100% deterministic tests"),
      "Submitting goal edit must call manageGoal with edit action",
    );

    // 3. Caret `@` input-trigger supports keyboard ArrowDown, Tab directory drill-down, and Enter completion:
    const prompt = w.document.getElementById("prompt");
    prompt.value = "@";
    prompt.setSelectionRange(1, 1);
    prompt.dispatchEvent(new w.Event("input", { bubbles: true }));
    await wait();
    const triggers = w.document.getElementById("composer-triggers");
    assert.equal(triggers.hidden, false, "@ trigger menu must open");
    // First item is `@src/` (Directory): pressing Tab drills into `@src/`!
    prompt.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }));
    await wait();
    assert.equal(prompt.value, "@src/", "Tab on a directory candidate must drill into @src/");
    assert.equal(triggers.hidden, false, "Menu remains open after directory drill-down");
    // Pressing Enter now selects `@src/main.rs`:
    prompt.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    await wait();
    assert.equal(prompt.value, "@src/main.rs ");
    assert.equal(triggers.hidden, true);

    // 4. Clickable prose `src/main.rs:2` inline-code link opens docked preview with highlighted line 2:
    const fileChip = w.document.querySelector("code.prose-file-link");
    assert.ok(fileChip, "Inline code file:line reference must be marked clickable");
    assert.equal(fileChip.textContent, "src/main.rs:2");
    fileChip.click();
    await wait();
    await wait();
    const targetRow = w.document.querySelector(".drawer .dock-code-row.is-target-line");
    assert.ok(targetRow, "Clicking src/main.rs:2 must highlight line 2 in docked preview");
    assert.match(targetRow.textContent, /target line/);

    // 5. Changes tab in dock opens Split / Unified Git Diff viewer:
    const changesTab = Array.from(w.document.querySelectorAll(".drawer .dock-tab")).find(
      (b) => b.textContent.trim() === "Changes",
    );
    changesTab.click();
    await wait();
    const changeRow = w.document.querySelector(".drawer .dock-change-row");
    assert.ok(changeRow);
    changeRow.click();
    await wait();
    const diffTable = w.document.querySelector("dialog[open] .dsh-diff-table.split");
    assert.ok(diffTable, "Split side-by-side diff table must open by default");
    const unifiedBtn = Array.from(w.document.querySelectorAll("dialog[open] button")).find(
      (b) => b.textContent.trim() === "Unified",
    );
    unifiedBtn.click();
    assert.ok(w.document.querySelector("dialog[open] .dsh-diff-table.unified"), "Unified diff toggle must work");
    w.document.querySelector("dialog[open]").close();

    // 6. Quick Jump launcher opens from topbar pill and filters entries:
    const quickJump = w.document.querySelector(".quick-jump-pill");
    assert.ok(quickJump);
    quickJump.click();
    await wait();
    const launcher = w.document.querySelector("dialog.dsh-quick-launcher[open]");
    assert.ok(launcher, "Quick Jump command launcher must open");
    w.document.querySelector("dialog.dsh-quick-launcher[open]").close();

    // 7. Artifacts page + Artifact Studio responsive viewport sizer (`Desktop` / `Tablet` / `Mobile`) + Inline chat artifact card:
    const artNavBtn = Array.from(w.document.querySelectorAll("aside nav button")).find(
      (b) => b.textContent.trim() === "Artifacts",
    );
    artNavBtn.click();
    await wait();
    await wait();
    const openPreviewBtn = Array.from(w.document.querySelectorAll("button")).find(
      (b) => b.textContent.trim() === "Open preview",
    );
    assert.ok(openPreviewBtn);
    openPreviewBtn.click();
    await wait();
    const stage = w.document.querySelector("dialog[open] .artifact-stage");
    const frame = stage?.querySelector("iframe.preview-frame");
    assert.ok(frame);
    assert.equal(frame.getAttribute("sandbox"), "allow-scripts allow-forms");
    const mobileBtn = Array.from(w.document.querySelectorAll("dialog[open] button")).find(
      (b) => b.textContent.trim() === "Mobile",
    );
    mobileBtn.click();
    assert.equal(stage.dataset.viewport, "mobile");
    w.document.querySelector("dialog[open]").close();

    // Returning to conversation shows inline `.dsh-inline-artifact-card` ("Open Studio") in turn tail:
    Array.from(w.document.querySelectorAll("button")).find((b) => b.textContent.includes("Kernel Audit")).click();
    await wait();
    assert.ok(
      w.document.querySelector(".dsh-inline-artifact-card"),
      "Session artifacts must render inline Open Studio card in conversation tail",
    );
  } finally {
    f.close();
  }
});

test("Back/Forward topbar history, floating Find-in-Page case toggle, Output Styles, Warm/Serif themes, Side Chat (/btw), queued reorder/edit, and Artifact Console work", async () => {
  const f = await createCordisUiFixture();
  try {
    const { w, calls } = f;
    f.pendingBySession["sess-cordis-1"] = [
      { messageUuid: "q-1", text: "First queued step", delivery: "queue", held: false },
      { messageUuid: "q-2", text: "Urgent second step", delivery: "queue", held: false },
    ];
    f.artifactsList.push({
      id: "art-console-1",
      name: "Dashboard Preview",
      title: "Dashboard Preview",
      html: "<!doctype html><h1>Dashboard</h1>",
      revision: 1,
      updatedAt: Date.now(),
      createdBySessionId: "sess-cordis-1",
    });
    f.sessions[0].bufferedMessages = [
      {
        type: "assistant",
        uuid: "a-find-1",
        message: { content: [{ type: "text", text: "Needle alpha and needle beta." }] },
      },
    ];

    // 1. Navigate to conversation -> Settings -> Back -> Forward via topbar history:
    Array.from(w.document.querySelectorAll("button")).find((b) => b.textContent.includes("Kernel Audit")).click();
    await wait();
    await wait();
    Array.from(w.document.querySelectorAll("aside footer button")).find((b) => b.getAttribute("aria-label") === "Settings").click();
    await wait();

    // 2. Warm Studio themes & Editorial Serif reading typography in Settings:
    const warmPaperBtn = w.document.querySelector('button[aria-label="Use warm-light theme"]');
    assert.ok(warmPaperBtn, "Warm Paper theme swatch must render");
    warmPaperBtn.click();
    assert.equal(w.document.documentElement.dataset.theme, "warm-light");
    const fontSel = w.document.querySelector('select[aria-label="Reading typography"]');
    assert.ok(fontSel, "Reading typography selector must render");
    fontSel.value = "serif";
    fontSel.dispatchEvent(new w.Event("change", { bubbles: true }));
    assert.equal(w.document.documentElement.dataset.proseFont, "serif");

    // Topbar Back & Forward buttons navigate cleanly:
    const backBtn = w.document.querySelector('.topbar-nav button[aria-label="Back"]');
    assert.ok(backBtn && !backBtn.disabled, "Back button must be enabled after navigation");
    backBtn.click();
    await wait();
    assert.equal(w.location.pathname, "/chat/sess-cordis-1");
    const fwdBtn = w.document.querySelector('.topbar-nav button[aria-label="Forward"]');
    assert.ok(fwdBtn && !fwdBtn.disabled, "Forward button must be enabled after going back");
    fwdBtn.click();
    await wait();
    assert.equal(w.location.pathname, "/settings");
    w.document.querySelector('.topbar-nav button[aria-label="Back"]').click();
    await wait();
    await wait();

    // 3. Floating Find-in-Conversation (`⌘F`) + Case-sensitivity (`Aa`) toggle:
    w.dispatchEvent(new w.KeyboardEvent("keydown", { key: "f", ctrlKey: true, metaKey: true, bubbles: true, cancelable: true }));
    await wait();
    const findBar = w.document.getElementById("conversation-find");
    assert.ok(findBar?.classList.contains("floating-find-pill"), "Mod+F must open floating find pill");
    const findInput = findBar.querySelector("input");
    findInput.value = "Needle";
    findInput.dispatchEvent(new w.Event("input", { bubbles: true }));
    assert.equal(w.document.querySelectorAll("mark[data-find]").length, 2, "Case-insensitive by default");
    const caseBtn = findBar.querySelector('button[aria-label="Match case"]');
    caseBtn.click();
    assert.equal(w.document.querySelectorAll("mark[data-find]").length, 1, "Aa toggle narrows to exact case match");
    findBar.querySelector('button[aria-label="Close find"]').click();

    // 4. Response Output Styles (`✦ Style: Normal` -> `Concise` via `setOutputStyle` RPC):
    const stylePill = w.document.querySelector(".dsh-style-pill");
    assert.ok(stylePill, "Response Style pill must render in composer footer");
    stylePill.click();
    await wait();
    const useConcise = Array.from(w.document.querySelectorAll("dialog[open] button")).find(
      (b) => b.textContent.trim() === "Use Concise",
    );
    assert.ok(useConcise, "Style modal must offer Concise preset");
    useConcise.click();
    await wait();
    assert.ok(calls.some((c) => c.member === "setOutputStyle" && c.args[1] === "concise"));
    assert.match(w.document.querySelector(".dsh-style-pill").textContent, /Style: Concise/);

    // 5. Queued follow-up messages: `↑ Top` reorder + `Edit` back into composer:
    const pendingRows = w.document.querySelectorAll(".pending-message");
    assert.equal(pendingRows.length, 2);
    const moveTopBtn = pendingRows[1].querySelector('button[aria-label="Move to top"]');
    assert.ok(moveTopBtn, "Second queued message must expose Move to top");
    moveTopBtn.click();
    await wait();
    assert.ok(calls.some((c) => c.member === "reorderPendingMessage" && c.args[1] === "q-2"));
    const editBtn = w.document.querySelector('.pending-message button[aria-label="Edit queued message"]');
    editBtn.click();
    await wait();
    assert.equal(w.document.getElementById("prompt").value, "Urgent second step");
    w.document.getElementById("prompt").value = "";

    // 6. Side Chat (`/btw`) quick aside drawer + archived aside session + Insert into prompt:
    const sideBtn = Array.from(w.document.querySelectorAll(".session-heading button")).find(
      (b) => b.textContent.trim() === "Side chat",
    );
    assert.ok(sideBtn, "Session heading must expose Side chat button");
    sideBtn.click();
    await wait();
    const sideDrawer = w.document.getElementById("dsh-side-chat");
    assert.ok(sideDrawer, "Side Chat drawer must open");
    const sideInput = sideDrawer.querySelector("textarea");
    sideInput.value = "How do userScripts work?";
    Array.from(sideDrawer.querySelectorAll("button")).find((b) => b.textContent.trim() === "Ask").click();
    await wait();
    await wait();
    assert.match(sideDrawer.textContent, /Side chat answer: use WKWebView userScript/);
    assert.ok(
      !Array.from(w.document.querySelectorAll("#recents .session-link")).some((b) => b.textContent.includes("Aside ·")),
      "Archived aside session must not clutter main sidebar recents",
    );
    Array.from(sideDrawer.querySelectorAll("button")).find((b) => b.textContent.trim() === "Insert into prompt").click();
    assert.match(w.document.getElementById("prompt").value, /WKWebView userScript/);

    // 7. Artifact Studio Console error capture + "Fix error in chat" + Light/Dark surface toggle:
    w.document.querySelector(".dsh-inline-artifact-card button").click();
    await wait();
    const darkSurf = Array.from(w.document.querySelectorAll("dialog[open] .artifact-surface-switch button")).find(
      (b) => b.textContent.trim() === "Dark",
    );
    assert.ok(darkSurf);
    darkSurf.click();
    assert.equal(w.document.querySelector("dialog[open] .artifact-stage").dataset.surface, "dark");
    w.dispatchEvent(
      new w.MessageEvent("message", {
        data: { type: "quixi:artifact-console", level: "error", text: "ReferenceError: chartData is not defined" },
      }),
    );
    const consoleTab = w.document.querySelector('dialog[open] button[aria-label="Artifact console"]');
    assert.match(consoleTab.textContent, /Console \(1 err\)/);
    consoleTab.click();
    const fixBtn = Array.from(w.document.querySelectorAll("dialog[open] button")).find(
      (b) => b.textContent.trim() === "Fix error in chat",
    );
    assert.ok(fixBtn, "Console pane must offer 1-click Fix error in chat");
    fixBtn.click();
    await wait();
    assert.match(w.document.getElementById("prompt").value, /ReferenceError: chartData is not defined/);

    // 8. Recent items expose full-name hover tooltips, left sidebar drag-resizes, and Retry runs in place without cloning Recent:
    const recentLink = w.document.querySelector("#recents .session-link");
    assert.equal(recentLink.title, "Kernel Audit", "Recent items must expose full title tooltip on hover");
    const resizer = w.document.querySelector(".sidebar .sidebar-resizer");
    assert.ok(resizer, "Left sidebar must render drag-resize splitter");
    resizer.dispatchEvent(new w.MouseEvent("pointerdown", { button: 0, clientX: 248, bubbles: true }));
    w.dispatchEvent(new w.MouseEvent("pointermove", { clientX: 312, bubbles: true }));
    w.dispatchEvent(new w.MouseEvent("pointerup", { clientX: 312, bubbles: true }));
    assert.equal(w.document.documentElement.style.getPropertyValue("--sidebar-width"), "312px");
    assert.equal(w.localStorage.getItem("quixi.sidebarWidth"), "312");
    resizer.dispatchEvent(new w.MouseEvent("dblclick", { bubbles: true }));
    assert.equal(w.document.documentElement.style.getPropertyValue("--sidebar-width"), "248px");

    const recentsBeforeRetry = w.document.querySelectorAll("#recents .recent-row").length;
    const msgRetryBtn = Array.from(w.document.querySelectorAll(".message-actions button")).find(
      (b) => b.textContent.trim() === "Retry",
    );
    assert.ok(msgRetryBtn);
    msgRetryBtn.click();
    await wait();
    assert.ok(
      calls.some((c) => c.member === "retryFailedTurn" && c.args[0] === "sess-cordis-1" && c.args[1] === "a-find-1"),
      "Message Retry must invoke retryFailedTurn in place instead of forkMessage",
    );
    assert.equal(
      w.document.querySelectorAll("#recents .recent-row").length,
      recentsBeforeRetry,
      "Retry must never create a new item in Recent",
    );
  } finally {
    f.close();
  }
});
