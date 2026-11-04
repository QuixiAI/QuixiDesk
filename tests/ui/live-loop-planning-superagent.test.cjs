// Copyright (c) 2026 QuixiAI. MIT license.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { JSDOM } = require("jsdom");
const fs = require("node:fs");
const path = require("node:path");

const wait = (ms = 45) => new Promise((resolve) => setTimeout(resolve, ms));

async function createUiFixture() {
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
    activeRowIds: ["system-prompt", "tools-registry", "dsh-superagent"],
    activeClientBundles: [],
    fibers: [{ id: "dsh-superagent", state: { phase: "active" } }],
    bundles: [],
    presets: [
      { id: "standard", title: "Standard", badge: "Default", description: "Native loop" },
      { id: "dsh-superagent-router", title: "Superagent Main Session", badge: "Orchestrator", description: "Main router" },
      { id: "dsh-superagent-worker", title: "Superagent Executor", badge: "Worker", description: "Worker executor" },
    ],
  };

  const sessionState = {
    sessionId: "local_loop_1",
    title: "Refactor Provider Stream",
    cwd: "/tmp/workspace",
    model: "deepseek-reasoner",
    effortLevel: "high",
    permissionMode: "auto",
    isRunning: true,
    createdAt: Date.now() - 15000,
    lastActivityAt: Date.now(),
    bufferedMessages: [
      {
        uuid: "u-1",
        type: "user",
        message: { role: "user", content: [{ type: "text", text: "Upgrade provider layer and run tests." }] },
      },
      {
        uuid: "a-1",
        type: "assistant",
        message: {
          role: "assistant",
          content: [
            {
              type: "tool_use",
              id: "tc_bash_1",
              name: "Bash",
              input: { command: "cargo test --workspace", description: "Run full workspace test suite" },
            },
          ],
        },
      },
    ],
  };

  const collabState = {
    agentPreset: "standard",
    planMode: {
      active: true,
      planTitle: "AISDK & Superagent Rollout Plan",
      planStatus: "approved",
      planMarkdown: "# Rollout Plan\n\n1. Integrate `aisdk` streaming\n2. Surface live loop actions\n3. Ship `dsh-superagent`",
    },
    todos: [
      { id: "t1", content: "Integrate aisdk provider builders", status: "completed" },
      { id: "t2", content: "Surface live Bash & tool-preparing status", status: "in_progress" },
      { id: "t3", content: "Verify 6-tab Superagent panel", status: "pending" },
    ],
    deliverables: [],
    questions: [],
  };

  const superagentSnap = {
    paused: false,
    masterId: "local_superagent_main",
    policyVersion: "9f82a10bc31e",
    settingsRevision: 2,
    promptRevision: 1,
    attentionCount: 1,
    overview: {
      person: [
        {
          id: "tkt_flaky01",
          short_name: "fix-flaky-timer",
          title: "Fix flaky timer spec",
          stage: "person/answer",
          origin_thread: "github:42",
          question: "Should timer retries cap at 3 or 5?",
          canRetry: false,
        },
      ],
      problems: [],
      flow: { received: 4, filtered: 1, judged: 3, accepted: 1, dropped: 0 },
      executor: { running: 1, maxWorkers: 4, queued: 0, idle: 0 },
    },
    sources: {
      github: { enabled: true, repository: "QuixiAI/QuixiDesk", agentAccount: "quixi-bot", pollIntervalSeconds: 60 },
      feishu: { enabled: false },
    },
    threads: [{ key: "github:42", title: "#42 Fix flaky timer", subscriptions: ["tkt_flaky01"], decisionIds: ["dec_1"] }],
    triggers: [
      {
        trigger: { id: "trg_42", source: "github", scope: "issues", actor: "alice", addressed: "mention", title: "#42 Fix flaky timer", text: "@quixi-bot please fix timer" },
        disposition: { decisionId: "dec_1" },
      },
    ],
    decisions: [
      { id: "dec_1", thread: "github:42", kind: "accept", wouldAccept: false, reason: "Explicit request accepted", policyVersion: "9f82a10bc31e", ticketId: "tkt_flaky01" },
    ],
    tickets: [
      {
        id: "tkt_flaky01",
        short_name: "fix-flaky-timer",
        title: "Fix flaky timer spec",
        stage: "person/answer",
        origin_thread: "github:42",
        question: "Should timer retries cap at 3 or 5?",
        canRetry: false,
        mandate: { strength: "explicit", policyVersion: "9f82a10bc31e" },
        assignments: [
          {
            id: "asg_1",
            status: "running",
            directory: "/tmp/superagent/work/fix-flaky-timer",
            reports: [{ state: "progress", summary: "Reproduced race in timer spec." }],
          },
        ],
      },
    ],
    settings: {
      policy: {
        acceptMode: "observe",
        rules: [{ id: "ci", name: "CI Failure", instruction: "Investigate CI break", enabled: true }],
        sources: { github: { allowed: [{ id: "alice", name: "Alice" }], bots: ["dependabot[bot]"], admission: {} } },
      },
      github: { enabled: true, repository: "QuixiAI/QuixiDesk", agentAccount: "quixi-bot", pollIntervalSeconds: 60 },
      executor: { maxWorkers: 4, workRoot: "/tmp/superagent/work" },
    },
    prompts: {
      revision: 1,
      config: {
        sharedNotes: "Always run cargo test.",
        orchestrator: { instructions: "Route crisply." },
        worker: { instructions: "Verify before reporting ready." },
      },
      orchestratorFull: "# Orchestrator Prompt Preview",
    },
  };

  const rpc = async (service, method, args = []) => {
    calls.push({ service, method, args });
    if (service === "CordisPlugins" && method === "snapshot") return structuredClone(cordisState);
    if (service === "CordisPlugins" && method === "listPresets") return structuredClone(cordisState.presets);
    if (service === "Superagent" && method === "snapshot") return structuredClone(superagentSnap);
    if (service === "Superagent" && method === "search") {
      return { tickets: superagentSnap.tickets, threads: [], items: [] };
    }
    if (method === "getAll") {
      return service === "LocalSessions" ? [] : [structuredClone(sessionState)];
    }
    if (method === "getSession") {
      return structuredClone(sessionState);
    }
    if (method === "getCollabState") {
      return structuredClone(collabState);
    }
    if (
      method === "getAllSpaces" ||
      method === "getAllArtifacts" ||
      method === "getAllScheduledTasks" ||
      method === "listLocalSkills" ||
      method === "getLocalMcpServers" ||
      method === "listSubagents" ||
      method === "listJobs" ||
      method === "getToolPermissions" ||
      method === "getPendingMessages"
    ) {
      return [];
    }
    return { ok: true };
  };

  w.quixi = {
    rpc,
    api: async () => ({
      hasConfiguredProvider: true,
      activeProvider: "deepseek",
      activeModel: "deepseek-reasoner",
      providers: [{ id: "deepseek", name: "DeepSeek", connected: true, models: ["deepseek-reasoner"] }],
    }),
    copy: async () => {},
    origin: "https://app.localhost",
  };

  for (const name of [
    "cordis-runtime",
    "cordis-ui-plugins",
    "superagent-plugin",
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
  return { w, calls, close: () => dom.window.close() };
}

test("Live agentic loop surfaces tool-preparing, live Bash output, elapsed clock, upgraded Todo/Plan dock, and 6-tab Superagent panel", async () => {
  const f = await createUiFixture();
  const { w } = f;
  try {
    // 1. Open active running session:
    const sessBtn = Array.from(w.document.querySelectorAll("button")).find((b) =>
      b.textContent.includes("Refactor Provider Stream"),
    );
    assert.ok(sessBtn, "Session button should render in sidebar");
    sessBtn.click();
    await wait(70);

    // Active process group auto-opens and displays DeepSeek Harness category + detail:
    const activeGroup = w.document.querySelector(".activity-group.is-live");
    assert.ok(activeGroup && activeGroup.open, "Active tool process group must auto-open during running turn");
    assert.ok(
      activeGroup.textContent.includes("Running commands") &&
        activeGroup.textContent.includes("Run full workspace test suite"),
      "Process group summary should show DeepSeek Harness category and live tool detail",
    );

    // Running turn bar with tabular elapsed clock is visible:
    const turnBar = w.document.querySelector(".dsh-running-turn-bar");
    assert.ok(turnBar, "Bottom live running turn status bar must be visible while running");
    assert.match(
      w.document.querySelector('[data-elapsed-clock="true"]').textContent,
      /^\d+:\d{2}$/,
    );

    // Emit live `bash_output` event -> surfaces scrolling stdout tail in both the step card and turn bar!
    w.dispatchEvent(
      new w.CustomEvent("quixi:event", {
        detail: {
          channel: "LocalAgentModeSessions_$_onEvent",
          payload: {
            sessionId: "local_loop_1",
            type: "turn_activity",
            activity: {
              phase: "bash_output",
              toolId: "tc_bash_1",
              line: "test superagent::tests::superagent_lifecycle ... ok",
            },
          },
        },
      }),
    );
    await wait(50);
    assert.ok(
      w.document.getElementById("conversation").textContent.includes("▸ test superagent::tests::superagent_lifecycle ... ok"),
      "Live Bash stdout tail must stream directly to the conversation UI",
    );

    // Emit `tool_preparing` stream event from `aisdk` -> surfaces `Preparing Write · superagent/engine.rs`!
    w.dispatchEvent(
      new w.CustomEvent("quixi:event", {
        detail: {
          channel: "LocalAgentModeSessions_$_onEvent",
          payload: {
            sessionId: "local_loop_1",
            message: {
              type: "stream_event",
              event: {
                type: "tool_preparing",
                id: "tc_write_2",
                name: "Write",
                total_bytes: 2048,
                arguments_preview: '{"file_path":"/tmp/workspace/superagent/engine.rs","content":"..."',
              },
            },
          },
        },
      }),
    );
    await wait(50);
    const barAfterPrep = w.document.querySelector(".dsh-running-turn-bar").textContent;
    assert.ok(
      barAfterPrep.includes("Preparing Write") && barAfterPrep.includes("superagent/engine.rs") && barAfterPrep.includes("2.0 KB"),
      `Expected Preparing Write detail in turn bar, got: ${barAfterPrep}`,
    );

    // 2. Verify Upgraded Planning & Task Progress Dock above composer:
    const todoDock = w.document.querySelector(".dsh-todo-dock");
    assert.ok(todoDock, "Upgraded Todo progress dock should render above composer");
    assert.ok(todoDock.textContent.includes("Tasks 1/3"));
    assert.ok(todoDock.textContent.includes("▸ Surface live Bash & tool-preparing status"));

    const planPill = w.document.querySelector(".dsh-plan-pill-bar");
    assert.ok(planPill && planPill.textContent.includes("Plan Mode"), "Active Plan Mode pill bar should render");

    const planCard = w.document.querySelector(".dsh-plan-card-bar");
    assert.ok(planCard && planCard.textContent.includes("AISDK & Superagent Rollout Plan"), "Persistent Plan card bar should render");

    // 3. Navigate to Sidebar -> `Superagent (1)` and verify all 6 tabs!
    const saNavBtn = Array.from(w.document.querySelectorAll("button")).find((b) =>
      b.textContent.includes("Superagent (1)"),
    );
    assert.ok(saNavBtn, "Sidebar should mount Superagent navigation entry with human-attention count");
    saNavBtn.click();
    await wait(70);

    const saPage = w.document.querySelector(".sa-page");
    assert.ok(saPage, "Superagent 6-tab page should render");
    assert.ok(saPage.textContent.includes("Fix flaky timer spec"), "Overview tab should highlight ticket needing human answer");
    assert.ok(saPage.textContent.includes("Should timer retries cap at 3 or 5?"));

    // Switch to Execution tab -> verify Ticket detail, Mandate, and Worker Assignment report:
    const execTabBtn = Array.from(saPage.querySelectorAll(".sa-tabs button")).find((b) =>
      b.textContent.startsWith("Execution"),
    );
    execTabBtn.click();
    await wait(40);
    assert.ok(saPage.textContent.includes("Reproduced race in timer spec."));
    assert.ok(saPage.textContent.includes("Dispatch Worker Turn"));
  } finally {
    f.close();
  }
});
