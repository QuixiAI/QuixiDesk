// Copyright (c) 2026 QuixiAI. MIT license.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { JSDOM } = require("jsdom");
const fs = require("node:fs");
const path = require("node:path");
const source = fs.readFileSync(path.join(__dirname, "../../resources/ui/capability-workflows.js"), "utf8");
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

function fixture({ config = {}, servers = [], skills = [], recognition = false, speech = false } = {}) {
  const dom = new JSDOM('<!doctype html><html lang="en"><body><main></main></body></html>', { url: "https://app.localhost/tools", runScripts: "outside-only" });
  const w = dom.window, doc = w.document;
  const calls = [], notices = [], speechCalls = [], skillCalls = [];
  let current = structuredClone(config), data = { servers, skills, error: "" };
  function el(tag, attrs = {}, ...children) {
    const node = doc.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {
      if (value === undefined || value === null || value === false) continue;
      if (key === "class") node.className = value;
      else if (key.startsWith("on")) node.addEventListener(key.slice(2).toLowerCase(), value);
      else if (["value", "checked", "disabled"].includes(key)) node[key] = value;
      else node.setAttribute(key, value === true ? "" : value);
    }
    for (const child of children.flat(Infinity)) if (child != null && child !== false) node.append(child instanceof w.Node ? child : String(child));
    return node;
  }
  function button(label, fn, cls) {
    return el("button", { class: cls, onclick: async () => { try { await fn(); } catch (error) { notices.push(error.message); } } }, label);
  }
  function dialog(title, children) {
    const node = el("dialog", {}, el("h2", {}, title), children);
    node.close = () => { node.remove(); node.dispatchEvent(new w.Event("close")); };
    doc.body.append(node); return node;
  }
  if (speech) {
    w.SpeechSynthesisUtterance = class { constructor(text) { this.text = text; } };
    w.speechSynthesis = { speak: (utterance) => speechCalls.push(utterance), cancel: () => speechCalls.push("cancel") };
  }
  const instances = [];
  if (recognition) w.SpeechRecognition = class {
    constructor() { instances.push(this); }
    start() { this.onstart?.(); }
    stop() { this.onend?.(); }
    abort() { this.onend?.(); }
  };
  let module;
  const ctx = {
    el, button, dialog, toast: (message) => notices.push(message),
    field: (label, control, hint) => el("label", {}, el("span", {}, label), control, hint ? el("small", {}, hint) : null),
    select: (options, value, change) => el("select", { onchange: (event) => change(event.target.value) }, options.map(([id, label]) => el("option", { value: id, selected: id === value }, label))),
    confirmDialog: (title, text, action, label) => dialog(title, [el("p", {}, text), button(label, action)]),
    header: (title, text) => el("header", {}, el("h1", {}, title), el("p", {}, text)),
    plainBody: (text) => el("p", {}, text),
    getToolsData: () => data,
    updateMcpConnections: () => {
      const list = doc.getElementById("mcp-connections");
      list?.replaceChildren(...data.servers.map((server) => el("article", { "data-server": server.name }, server.name)));
    },
    reloadTools: async () => {}, advancedMcpEditor: () => {},
    startSkill: async (skill, files) => skillCalls.push({ skill, files }),
    rpc: async (service, member, args = []) => {
      calls.push({ service, member, args });
      if (member === "getMcpServersConfig") return structuredClone(current);
      if (member === "setMcpServerConfigs") { current = structuredClone(args[0]); return { ok: true }; }
      if (member === "getLocalSkillFiles") return [{ path: "SKILL.md", content: "# Real skill\nUse the actual instructions." }];
      if (member === "getBuildProps") return { appVersion: "3.7.1", commitHash: "fake legacy field deliberately ignored" };
      throw new Error(`Unexpected RPC: ${member}`);
    },
    api: async () => ({ issues: [] }),
    openExternal: () => {},
  };
  w.eval(source); module = w.QuixiCapabilityWorkflows.create(ctx);
  const host = doc.querySelector("main");
  const click = async (label, scope = doc) => {
    const node = [...scope.querySelectorAll("button")].find((node) => node.textContent === label);
    assert.ok(node, `button ${label} exists`); node.click(); await tick(); await tick();
  };
  const input = (label) => {
    const node = [...doc.querySelectorAll("label")].find((node) => node.firstChild?.textContent === label);
    assert.ok(node, `field ${label} exists`); return node.querySelector("input,textarea,select");
  };
  return { dom, w, doc, module, host, click, input, calls, notices, speechCalls, skillCalls, instances, getConfig: () => current, setConfig: (value) => { current = value; }, setServers: (servers) => { data.servers = servers; } };
}

test("individual connection edits preserve unknown policy options and concurrent edits to other connections", async () => {
  const f = fixture({ config: { alpha: { url: "https://a.example/mcp", transport: "http", disabledTools: ["delete"], requestTimeoutMs: 9000, headers: { Authorization: "local-test" } }, beta: { command: "old" } } });
  await f.module.connectionEditor("alpha");
  f.input("Server URL").value = "https://new.example/mcp";
  f.setConfig({ ...f.getConfig(), beta: { command: "updated elsewhere" } });
  await f.click("Save connection");
  assert.equal(f.getConfig().alpha.url, "https://new.example/mcp");
  assert.deepEqual(f.getConfig().alpha.disabledTools, ["delete"]);
  assert.equal(f.getConfig().alpha.requestTimeoutMs, 9000);
  assert.equal(f.getConfig().alpha.headers.Authorization, "local-test");
  assert.equal(f.getConfig().beta.command, "updated elsewhere");
  assert.equal(f.doc.querySelector("dialog"), null);
  f.dom.window.close();
});

test("a conflicting change to the edited connection is not overwritten", async () => {
  const f = fixture({ config: { alpha: { url: "https://a.example/mcp" } } });
  await f.module.connectionEditor("alpha");
  f.input("Server URL").value = "https://b.example/mcp";
  f.setConfig({ alpha: { url: "https://c.example/mcp" } });
  await f.click("Save connection");
  assert.equal(f.getConfig().alpha.url, "https://c.example/mcp");
  assert.match(f.doc.querySelector('[role="alert"]').textContent, /changed while you were editing/);
  assert.equal(f.calls.filter((call) => call.member === "setMcpServerConfigs").length, 0);
  f.dom.window.close();
});

test("connection forms reject unsafe URLs and duplicate names without saving", async () => {
  const f = fixture({ config: { alpha: { command: "server" } } });
  await f.module.connectionEditor();
  f.input("Connection name").value = "alpha";
  f.input("Server URL").value = "javascript:alert(1)";
  await f.click("Save connection");
  assert.match(f.doc.querySelector('[role="alert"]').textContent, /HTTP or HTTPS/);
  f.input("Server URL").value = "https://remote.example/mcp";
  await f.click("Save connection");
  assert.match(f.doc.querySelector('[role="alert"]').textContent, /already exists/);
  assert.equal(f.calls.filter((call) => call.member === "setMcpServerConfigs").length, 0);
  f.dom.window.close();
});

test("local arguments preserve spaces and connection disable preserves configuration", async () => {
  const f = fixture({ config: { local: { command: "node", args: ["/folder with spaces/server.js", ""], env: { TOKEN: "test-only" }, enabled: true } }, servers: [{ name: "local", status: "ready" }] });
  await f.module.connectionEditor("local");
  await f.click("Save connection");
  assert.deepEqual(f.getConfig().local.args, ["/folder with spaces/server.js", ""]);
  f.module.renderTools(f.host); await f.click("Disable");
  assert.equal(f.getConfig().local.enabled, false);
  assert.equal(f.getConfig().local.env.TOKEN, "test-only");
  f.setServers([{ name: "local", status: "disabled" }]); f.module.enhanceConnections();
  await f.click("Enable"); assert.equal(f.getConfig().local.enabled, true);
  f.dom.window.close();
});

test("searchable skills load actual instructions before starting a task; no unsupported toggle is shown", async () => {
  const f = fixture({ skills: [{ skillId: "pdf", name: "PDF reader", description: "Read documents" }, { skillId: "code", name: "Code review", description: "Review source" }] });
  f.module.renderTools(f.host);
  const query = f.doc.querySelector('[aria-label="Search skills"]'); query.value = "documents"; query.dispatchEvent(new f.w.Event("input"));
  assert.equal(f.doc.querySelectorAll(".skill-card").length, 1);
  await f.click("Try in a new task");
  assert.equal(f.skillCalls[0].skill.skillId, "pdf");
  assert.match(f.skillCalls[0].files[0].content, /actual instructions/);
  assert.equal(f.doc.querySelector('.skills-section input[type="checkbox"]'), null);
  f.dom.window.close();
});

test("failed legacy built-ins have a replacement path that becomes a real remote connection", async () => {
  const f = fixture({ config: { pdf: { command: "quixi-mcp-builtin", args: ["--pdf"] } }, servers: [{ name: "pdf", builtin: true, status: "failed", error: { kind: "unsupported" } }] });
  f.module.renderTools(f.host); await f.click("Replace connection");
  assert.match(f.doc.querySelector("dialog").textContent, /legacy connector is unavailable/);
  f.input("Server URL").value = "https://documents.example/mcp";
  await f.click("Save connection");
  assert.equal(f.getConfig().pdf.transport, "http");
  assert.equal(f.getConfig().pdf.command, undefined);
  assert.equal(f.getConfig().pdf.args, undefined);
  f.dom.window.close();
});

test("unsupported dictation offers OS guidance and does not claim to record", async () => {
  const f = fixture(); const area = f.doc.createElement("textarea"); f.host.append(area, f.module.dictationControl(area));
  await f.click("Dictate");
  assert.match(f.doc.querySelector("dialog").textContent, /has not started recording/);
  assert.equal(f.instances.length, 0);
  assert.equal(f.module.readAloudControl("hello"), null);
  f.dom.window.close();
});

test("dictation inserts finalized text without sending, preserves typed input, and stops on navigation cleanup", async () => {
  const f = fixture({ recognition: true }); const area = f.doc.createElement("textarea"); area.value = "Review";
  f.host.append(area, f.module.dictationControl(area)); area.setSelectionRange(6, 6);
  let inputEvents = 0; area.addEventListener("input", () => inputEvents++);
  await f.click("Dictate"); assert.equal(f.instances.length, 0);
  await f.click("Start dictation"); const recognizer = f.instances[0];
  const result = [{ transcript: "the proposal" }]; result.isFinal = true;
  recognizer.onresult({ resultIndex: 0, results: [result] });
  assert.equal(area.value, "Review the proposal"); assert.equal(inputEvents, 1);
  f.module.stopAudio(); recognizer.onresult({ resultIndex: 0, results: [result] });
  assert.equal(area.value, "Review the proposal");
  f.dom.window.close();
});

test("read aloud can be canceled and late callbacks cannot restart playback", async () => {
  const f = fixture({ speech: true }); f.host.append(f.module.readAloudControl("Words ".repeat(300)));
  await f.click("Read aloud");
  const utterance = f.speechCalls.find((value) => typeof value === "object"); assert.ok(utterance);
  await f.click("Stop reading"); const count = f.speechCalls.length;
  utterance.onend(); assert.equal(f.speechCalls.length, count);
  assert.equal(f.host.querySelector("button").textContent, "Read aloud");
  f.dom.window.close();
});

test("desktop status uses version and live storage while avoiding fabricated update or OS state", async () => {
  const f = fixture(); await f.module.renderDesktopStatus(f.host);
  assert.match(f.host.textContent, /QuixiDesk 3.7.1/);
  assert.match(f.host.textContent, /No storage problems reported/);
  assert.doesNotMatch(f.host.textContent, /fake legacy|up.to.date|Sequoia/);
  assert.equal(f.calls.some((call) => call.member === "getSystemInfo"), false);
  f.dom.window.close();
});
