/* Copyright (c) 2026 QuixiAI. MIT license. */
(() => {
  "use strict";

  const isObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const configError = (result) => {
    if (result?.error) throw new Error(typeof result.error === "string" ? result.error : result.error.message || "The request failed.");
    return result;
  };
  function readStringMap(value, label) {
    let parsed;
    try { parsed = JSON.parse(value.trim() || "{}"); }
    catch { throw new Error(`${label} must be a JSON object with text values.`); }
    if (!isObject(parsed) || Object.values(parsed).some((value) => typeof value !== "string"))
      throw new Error(`${label} must be a JSON object with text values.`);
    return parsed;
  }
  function validateName(name) {
    if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,79}$/.test(name) || ["constructor", "prototype", "__proto__"].includes(name))
      throw new Error("Use a name of up to 80 letters, numbers, dots, underscores or hyphens, starting with a letter or number.");
    return name;
  }
  function remoteUrl(value) {
    let url;
    try { url = new URL(value.trim()); } catch { throw new Error("Enter the server’s full http:// or https:// URL."); }
    if (!["https:", "http:"].includes(url.protocol) || url.username || url.password || url.hash)
      throw new Error("Use an HTTP or HTTPS URL without a password or fragment. Put authentication headers in Advanced options.");
    return url.href;
  }

  function create(ctx) {
    const { el, button, field, select, dialog, confirmDialog, header, plainBody, toast, rpc, api } = ctx;
    let activeRecognition = null;
    let speechGeneration = 0;
    let speakingText = null;
    const speechControls = new Set();

    async function getConfig() {
      const config = configError(await rpc("MCP", "getMcpServersConfig", [], "quixi.settings"));
      if (!isObject(config)) throw new Error("Connection configuration could not be read. Nothing was changed.");
      return config;
    }
    async function saveConfig(config) {
      const result = configError(await rpc("MCP", "setMcpServerConfigs", [config], "quixi.settings"));
      if (result?.ok !== true) throw new Error("The connection configuration was not saved.");
      await ctx.reloadTools();
    }
    async function changeConnection(name, change) {
      const config = await getConfig();
      if (!Object.hasOwn(config, name)) throw new Error("This connection was removed. Refresh the page and try again.");
      const next = change(config[name]);
      if (next === undefined) delete config[name]; else config[name] = next;
      await saveConfig(config);
    }
    function removeConnection(name) {
      confirmDialog("Remove connection?", `Remove “${name}” from QuixiDesk? This does not uninstall the server or delete its files.`,
        () => changeConnection(name, () => undefined), "Remove connection");
    }

    async function connectionEditor(name = null) {
      const initial = await getConfig();
      if (name !== null && !Object.hasOwn(initial, name)) throw new Error("This connection no longer exists.");
      const original = name === null ? undefined : initial[name];
      const entry = isObject(original) ? original : {};
      const legacy = name !== null && ctx.getToolsData().servers?.some((server) => server.name === name && server.error?.kind === "unsupported");
      const baseline = JSON.stringify(original);
      const nameInput = el("input", { value: name || "", placeholder: "my-connection", disabled: name !== null, autocomplete: "off" });
      const kind = entry.transport || (entry.url ? "http" : "stdio");
      const unsupported = !["stdio", "http", "sse"].includes(kind);
      const transport = select([["http", "Remote server (HTTP)"], ["stdio", "Local command"], ["sse", "Remote server (legacy SSE)"]], unsupported || legacy ? "http" : (name === null ? "http" : kind), () => updateTransport());
      const urlInput = el("input", { type: "url", value: entry.url || "", placeholder: "https://service.example/mcp", autocomplete: "off", spellcheck: "false" });
      const command = el("input", { value: entry.command || "", placeholder: "npx", autocomplete: "off", spellcheck: "false" });
      const argsHost = el("div", { class: "connection-arguments" });
      const argumentInputs = [];
      function addArgument(value = "") {
        const input = el("input", { value, "aria-label": "Command argument", placeholder: "One argument, for example -y", spellcheck: "false" });
        const row = el("div", { class: "row" }, input, button("Remove", () => { argumentInputs.splice(argumentInputs.indexOf(input), 1); row.remove(); }, "quiet"));
        argumentInputs.push(input); argsHost.append(row); return input;
      }
      (Array.isArray(entry.args) ? entry.args : []).forEach(addArgument);
      const env = el("textarea", { value: JSON.stringify(entry.env || {}, null, 2), rows: 4, spellcheck: "false" });
      const headers = el("textarea", { value: JSON.stringify(entry.headers || {}, null, 2), rows: 4, spellcheck: "false" });
      const cwd = el("input", { value: entry.cwd || "", placeholder: "Use the task’s folder", spellcheck: "false" });
      const enabled = el("input", { type: "checkbox", checked: entry.enabled !== false });
      const local = el("div", { class: "connection-local" }, field("Command", command, "The executable to start. Arguments are separate below; shell syntax is not evaluated."),
        el("label", {}, "Arguments"), argsHost, button("Add argument", () => addArgument().focus(), "quiet"),
        el("p", { class: "hint" }, "Local servers run with your account’s permissions. Only add commands you trust."));
      const remote = el("div", { class: "connection-remote" }, field("Server URL", urlInput, "Use the MCP endpoint supplied by the service. If it requires sign-in, the connection will offer Sign in after discovery."));
      const localAdvanced = el("div", {}, field("Working folder (optional)", cwd), field("Environment variables (JSON)", env));
      const remoteAdvanced = el("div", {}, field("HTTP headers (JSON)", headers, "Optional. Header values are stored in the local connection configuration."));
      const advanced = el("details", {}, el("summary", {}, "Advanced options"), localAdvanced, remoteAdvanced,
        el("p", { class: "hint" }, "Other existing options, tool restrictions and timeouts are preserved. Use Advanced JSON on the connections page to edit them."));
      const error = el("p", { class: "error-text", role: "alert", hidden: true });
      const save = button("Save connection", submit, "primary");
      const modal = dialog(name === null ? "Add connection" : `Edit ${name}`, [
        name !== null && !isObject(original) ? el("p", { class: "notice" }, "This connection has invalid configuration. Saving this form replaces that invalid entry.") : null,
        unsupported ? el("p", { class: "notice" }, "This transport is not supported by the form. Choose a supported transport or edit Advanced JSON.") : null,
        legacy ? el("p", { class: "notice" }, "This legacy connector is unavailable. Replace it with the URL or command of a working MCP server, or remove the connection.") : null,
        field("Connection name", nameInput), field("Connection type", transport), remote, local, advanced,
        el("label", { class: "mcp-checkbox" }, enabled, "Enable this connection"), error,
        el("div", { class: "form-actions" }, button("Cancel", () => modal.close(), "quiet"), save),
      ]);
      function updateTransport() {
        const isLocal = transport.value === "stdio";
        local.hidden = !isLocal; remote.hidden = isLocal;
        localAdvanced.hidden = !isLocal; remoteAdvanced.hidden = isLocal;
      }
      updateTransport();
      async function submit() {
        error.hidden = true; save.disabled = true;
        try {
          const key = validateName(nameInput.value.trim());
          const next = { ...entry, transport: transport.value, enabled: enabled.checked };
          if (transport.value === "stdio") {
            if (!command.value.trim()) throw new Error("Enter the command that starts this server.");
            next.command = command.value.trim(); next.args = argumentInputs.map((input) => input.value);
            next.env = readStringMap(env.value, "Environment variables");
            if (cwd.value.trim()) next.cwd = cwd.value.trim(); else delete next.cwd;
            delete next.url; delete next.headers;
          } else {
            next.url = remoteUrl(urlInput.value); next.headers = readStringMap(headers.value, "HTTP headers");
            delete next.command; delete next.args; delete next.env; delete next.cwd;
          }
          // Re-read to preserve unrelated changes and refuse to overwrite an edit made elsewhere.
          const latest = await getConfig();
          if (name === null && Object.hasOwn(latest, key)) throw new Error("That connection name already exists. Choose another name.");
          if (name !== null && JSON.stringify(latest[name]) !== baseline)
            throw new Error("This connection changed while you were editing it. Close this dialog and reopen it to review the latest settings.");
          latest[key] = next;
          await saveConfig(latest); modal.close();
          toast("Connection saved. Its status shows whether discovery is complete.");
        } catch (problem) { error.textContent = problem.message; error.hidden = false; }
        finally { save.disabled = false; }
      }
      return modal;
    }

    function enhanceConnections() {
      const list = document.getElementById("mcp-connections");
      if (!list) return;
      const servers = ctx.getToolsData().servers || [];
      for (const row of list.querySelectorAll("[data-server]")) {
        const server = servers.find((value) => value.name === row.dataset.server);
        if (!server || (server.builtin && server.error?.kind !== "unsupported")) continue;
        const management = row.querySelector(".connection-management");
        if (management) {
          management.querySelector(".connection-toggle").textContent = server.status === "disabled" ? "Enable" : "Disable";
          continue;
        }
        row.append(el("div", { class: "connection-management row" },
          button(server.error?.kind === "unsupported" ? "Replace connection" : "Edit connection", () => connectionEditor(server.name), "quiet"),
          button(server.status === "disabled" ? "Enable" : "Disable", async () => {
            await changeConnection(server.name, (value) => {
              if (!isObject(value)) throw new Error("Edit this invalid connection before enabling or disabling it.");
              return { ...value, enabled: value.enabled === false };
            });
          }, "quiet connection-toggle"),
          button("Remove", () => removeConnection(server.name), "quiet")));
      }
    }
    async function readSkill(skill) {
      const files = configError(await rpc("LocalAgentModeSessions", "getLocalSkillFiles", [skill.skillId || skill.name]));
      if (!Array.isArray(files) || !files.length) throw new Error("This skill’s instructions could not be loaded.");
      return files;
    }
    function skillsSection(skills) {
      const names={docx:"Word documents",pdf:"PDF editing","pdf-reading":"Read PDFs",pptx:"Presentations",xlsx:"Spreadsheets","frontend-design":"Interface design"};
      const query = el("input", { type: "search", placeholder: "Search skills", "aria-label": "Search skills" });
      const results = el("div", { class: "cards" });
      const count = el("p", { class: "hint", role: "status" });
      function draw() {
        const needle = query.value.trim().toLocaleLowerCase();
        const matches = skills.filter((skill) => `${skill.displayName || ""} ${skill.name || ""} ${skill.description || ""}`.toLocaleLowerCase().includes(needle));
        count.textContent = `${matches.length} ${matches.length === 1 ? "skill" : "skills"}`;
        results.replaceChildren(...matches.map((skill) => el("article", { class: "card skill-card" },
          el("h3", {}, skill.displayName || names[skill.name] || skill.name), el("p", {}, skill.description || "Instructions for a task workflow."),
          button("Try in a new task", async () => { const files = await readSkill(skill); await ctx.startSkill(skill, files); }, "primary"),
          button("Read instructions", async () => {
            const files = await readSkill(skill);
            dialog(skill.displayName || skill.name, files.map((file) => el("details", { open: file.path === "SKILL.md" }, el("summary", {}, file.path),
              file.path === "SKILL.md" ? plainBody((file.content || "").replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, "")) : el("pre", {}, file.content || ""))), { wide: true });
          }, "quiet"))));
        if (!matches.length) results.append(el("p", { class: "muted" }, needle ? "No skills match this search." : "No skills are installed."));
      }
      query.addEventListener("input", draw); draw();
      return el("section", { class: "skills-section" }, el("h2", {}, "Skills"),
        el("p", { class: "muted" }, "Start a task with a skill’s instructions attached, then describe what you want to do."), query, count, results);
    }
    function renderTools(host) {
      const data = ctx.getToolsData();
      host.replaceChildren(header("Tools & skills", "Connect services and start tasks with reusable instructions."),
        el("div", { class: "row" }, el("h2", { class: "grow" }, "Connections"),
          button("Add connection", () => connectionEditor(), "primary"), button("Advanced JSON", ctx.advancedMcpEditor, "quiet")),
        el("p", { class: "muted" }, "Ready means initialization and tool discovery completed. Sign-in requests, failures and last-known catalogs stay visible."),
        el("p", { id: "mcp-load-error", class: "error-text", role: "alert", hidden: !data.error }, data.error || ""),
        el("div", { id: "mcp-connections", class: "mcp-connections" }), el("hr"), skillsSection(data.skills || []));
      ctx.updateMcpConnections(); enhanceConnections();
    }

    function speechSupported() { return typeof window.SpeechSynthesisUtterance === "function" && typeof window.speechSynthesis?.speak === "function"; }
    function syncSpeechControls() {
      for (const control of speechControls) {
        if (!control.node.isConnected) { speechControls.delete(control); continue; }
        control.node.textContent = speakingText === control.text ? "Stop reading" : "Read aloud";
        control.node.setAttribute("aria-pressed", String(speakingText === control.text));
      }
    }
    function stopSpeaking() { speechGeneration++; speakingText = null; window.speechSynthesis?.cancel(); syncSpeechControls(); }
    function readAloudControl(text) {
      if (!speechSupported() || !text?.trim()) return null;
      const node = button(speakingText === text ? "Stop reading" : "Read aloud", () => {
        if (speakingText === text) { stopSpeaking(); return; }
        stopSpeaking(); speakingText = text; const generation = speechGeneration;
        const chunks = text.match(/[\s\S]{1,900}(?:\s|$)|[\s\S]{1,900}/g) || [text];
        const next = () => {
          if (generation !== speechGeneration) return;
          const part = chunks.shift();
          if (!part) { speakingText = null; syncSpeechControls(); return; }
          const utterance = new window.SpeechSynthesisUtterance(part);
          utterance.onend = next;
          utterance.onerror = (event) => { if (generation !== speechGeneration) return; stopSpeaking(); if (!["canceled", "interrupted"].includes(event.error)) toast("Read aloud could not play. Check your system voice and audio settings.", true); };
          window.speechSynthesis.speak(utterance);
        };
        syncSpeechControls(); next();
      }, "quiet");
      node.setAttribute("aria-pressed", String(speakingText === text));
      speechControls.add({ node, text });
      return node;
    }
    const recognitionConstructor = () => window.SpeechRecognition || window.webkitSpeechRecognition;
    function dictationControl(textarea) {
      const node = button("Dictate", () => {
        if (activeRecognition) { activeRecognition.stop(); return; }
        const Recognition = recognitionConstructor();
        if (typeof Recognition !== "function") {
          dialog("Use system dictation", [el("p", {}, "This webview does not provide in-app speech recognition. You can still use your operating system’s dictation in the prompt."),
            el("p", {}, "Focus the prompt, then use Dictation from macOS Keyboard settings, Windows voice typing (Win+H), or your Linux desktop’s speech-input tool."),
            el("p", { class: "hint" }, "QuixiDesk has not started recording audio.")]);
          return;
        }
        const notice = dialog("Dictate into your prompt", [el("p", {}, "Your webview’s speech service will transcribe your microphone. Depending on the platform, audio may be processed by an online speech service. Review the text before sending."),
          button("Start dictation", () => { notice.close(); start(Recognition); }, "primary")]);
      }, "quiet");
      node.title = typeof recognitionConstructor() === "function" ? "Dictate text into the prompt" : "System dictation help";
      function start(Recognition) {
        if (!textarea.isConnected) return;
        let recognition;
        try { recognition = new Recognition(); recognition.continuous = true; recognition.interimResults = false; recognition.lang = document.documentElement.lang || navigator.language || "en-US"; }
        catch { toast("Speech recognition could not start. Use your operating system’s dictation instead.", true); return; }
        activeRecognition = recognition;
        const cleanup = () => { if (activeRecognition === recognition) activeRecognition = null; node.textContent = "Dictate"; node.setAttribute("aria-pressed", "false"); };
        recognition.onstart = () => { node.textContent = "Stop dictation"; node.setAttribute("aria-pressed", "true"); };
        recognition.onresult = (event) => {
          if (activeRecognition !== recognition || !textarea.isConnected) { recognition.abort(); return; }
          const transcripts = [];
          for (let i = event.resultIndex || 0; i < event.results.length; i++) if (event.results[i].isFinal) transcripts.push(event.results[i][0].transcript);
          const spoken = transcripts.join(" ").trim(); if (!spoken) return;
          const at = textarea.selectionStart ?? textarea.value.length, end = textarea.selectionEnd ?? at;
          const prefix = at && !/\s/.test(textarea.value[at - 1]) ? " " : "";
          const suffix = end < textarea.value.length && !/\s/.test(textarea.value[end]) ? " " : "";
          textarea.setRangeText(prefix + spoken + suffix, at, end, "end");
          textarea.dispatchEvent(new Event("input", { bubbles: true }));
        };
        recognition.onerror = (event) => {
          cleanup();
          const messages = { "not-allowed": "Microphone access was denied. Enable microphone permission in your operating system or use system dictation.", "audio-capture": "No usable microphone was found. Check your microphone connection and settings.", network: "The speech service could not connect. Check your network or use system dictation.", "no-speech": "No speech was detected. Try again when you are ready." };
          if (event.error !== "aborted") toast(messages[event.error] || "Dictation stopped. Your existing prompt is preserved.", true);
        };
        recognition.onend = cleanup;
        try { textarea.focus(); recognition.start(); } catch { cleanup(); toast("Speech recognition could not start. Use system dictation instead.", true); }
      }
      return node;
    }
    function stopAudio() {
      stopSpeaking();
      const recognition = activeRecognition; activeRecognition = null;
      if (recognition) try { recognition.abort(); } catch { /* Already ended. */ }
    }
    window.addEventListener("pagehide", stopAudio);
    async function renderDesktopStatus(host) {
      const status = el("p", { role: "status", class: "hint" }, "Reading application status…");
      host.append(el("section", { class: "desktop-status" }, el("h2", {}, "Application & audio"), status,
        el("p", {}, speechSupported() ? "Read aloud is available on assistant messages through the system webview’s speech service." : "Read aloud is not available in this webview. Your system’s accessibility reader may still be available."),
        el("p", {}, typeof recognitionConstructor() === "function" ? "Dictation is available from the composer. The speech service may require network access and microphone permission." : "Use system dictation in the prompt. This webview does not expose in-app speech recognition."),
        el("p", { class: "hint" }, "Keyboard shortcuts work while QuixiDesk is focused: ⌘/Ctrl+N new task · ⌘/Ctrl+K search · ⌘/Ctrl+, settings."),
        button("View available releases", () => ctx.openExternal("https://github.com/QuixiAI/QuixiDesk/releases"), "quiet")));
      const results = await Promise.allSettled([rpc("AboutWindow", "getBuildProps"), api("/api/quixi/storage/status")]);
      if (!status.isConnected) return;
      const version = results[0].status === "fulfilled" ? results[0].value?.appVersion : null;
      const storage = results[1].status === "fulfilled" ? results[1].value : null;
      status.textContent = `${typeof version === "string" && version ? `QuixiDesk ${version}` : "Version unavailable"} · ${Array.isArray(storage?.issues) ? storage.issues.length ? "Storage needs attention — inspect Privacy & data before closing." : "No storage problems reported." : "Storage status unavailable."}`;
    }

    return { renderTools, enhanceConnections, connectionEditor, renderDesktopStatus, dictationControl, readAloudControl, stopAudio };
  }
  window.QuixiCapabilityWorkflows = { create };
})();
