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
    if (member === "listSubagents" || member === "listJobs" || member === "getToolPermissions" || member === "getPendingMessages") return [];
    if (member === "start") {
      const created = {
        sessionId: "sess-started",
        title: args[0].title,
        isRunning: true,
        isArchived: false,
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
    if (member === "sendMessageV2") return { ok: true };
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
    if (
      member === "listLocalSkills" ||
      member === "getLocalMcpServers" ||
      member === "getAllArtifacts" ||
      member === "getAllScheduledTasks"
    ) {
      return [];
    }
    return { ok: true };
  };

  const api = async (route) => {
    if (route === "/api/quixi/providers") return structuredClone(providers);
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
  return { w, calls, sessions, cordisState, close: () => dom.window.close() };
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
