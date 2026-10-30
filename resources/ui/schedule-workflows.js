/* Copyright (c) 2026 QuixiAI. MIT license. Original scheduling workflows. */
(() => {
  "use strict";
  const frequencies = [["daily", "Every day"], ["weekdays", "Weekdays"], ["weekly", "Every week"], ["hourly", "Every hour"], ["custom", "Custom schedule"]];
  const days = [["1", "Monday"], ["2", "Tuesday"], ["3", "Wednesday"], ["4", "Thursday"], ["5", "Friday"], ["6", "Saturday"], ["0", "Sunday"]];
  function parsePreset(expression = "0 9 * * *") {
    const fields = expression.trim().split(/\s+/);
    const [minute, hour, day, month, weekday] = fields;
    const numeric = (v, max) => /^\d+$/.test(v || "") && Number(v) <= max;
    const base = { frequency: "custom", time: "09:00", day: "1", minute: "0", custom: expression };
    if (fields.length !== 5 || day !== "*" || month !== "*" || !numeric(minute, 59)) return base;
    if (hour === "*" && weekday === "*") return { ...base, frequency: "hourly", minute: String(Number(minute)) };
    if (!numeric(hour, 23)) return base;
    const time = `${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`;
    if (weekday === "*") return { ...base, frequency: "daily", time };
    if (["1-5", "MON-FRI"].includes(weekday.toUpperCase())) return { ...base, frequency: "weekdays", time };
    if (numeric(weekday, 7)) return { ...base, frequency: "weekly", time, day: String(Number(weekday) % 7) };
    return { ...base, time };
  }
  function cronFromPreset({ frequency, time = "09:00", day = "1", minute = "0", custom = "" }) {
    if (frequency === "custom") return custom.trim();
    if (frequency === "hourly") {
      if (!/^\d+$/.test(minute) || Number(minute) > 59) throw new Error("Choose a minute between 0 and 59.");
      return `${Number(minute)} * * * *`;
    }
    if (!/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(time)) throw new Error("Choose a valid time.");
    const [hour, min] = time.split(":").map(Number);
    if (frequency === "weekly" && !/^[0-6]$/.test(day)) throw new Error("Choose a day of the week.");
    if (!["daily", "weekdays", "weekly"].includes(frequency)) throw new Error("Choose a frequency.");
    return `${min} ${hour} * * ${frequency === "weekdays" ? "1-5" : frequency === "weekly" ? day : "*"}`;
  }
  function describe(task) {
    const p = parsePreset(task.cronExpression || task.cron || "");
    const time = p.time;
    if (p.frequency === "daily") return `Every day at ${time}`;
    if (p.frequency === "weekdays") return `Weekdays at ${time}`;
    if (p.frequency === "weekly") return `Every ${days.find(d => d[0] === p.day)?.[1]} at ${time}`;
    if (p.frequency === "hourly") return `Every hour at :${p.minute.padStart(2, "0")}`;
    return `Custom: ${task.cronExpression || task.cron || "Not configured"}`;
  }
  function nextRunLabel(task) {
    if (!task.enabled) return "Paused";
    if (task.dispatchVersion !== 1) return "Edit and save to activate";
    if (!Number.isFinite(task.nextRunAt)) return "No next run — edit this schedule";
    return `Next: ${new Date(task.nextRunAt).toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}`;
  }
  function runStatus(task) {
    return ({ started: "Starting", running: "Running", waiting_approval: "Waiting for approval", finished: "Finished", failed: "Failed", blocked: "Could not start", interrupted: "Interrupted", skipped: "Skipped" })[task.lastRunStatus] || "Not run yet";
  }
  function createEditor(ctx, task = null) {
    const { state, el, button, field, select, dialog, rpc, api } = ctx;
    const preset = parsePreset(task?.cronExpression || "0 9 * * *");
    const initialProject = task?.spaceId ?? state.project ?? "";
    const name = el("input", { value: task?.displayName || task?.name || "", required: true, maxlength: 160, placeholder: "Morning project update", autocomplete: "off" });
    const prompt = el("textarea", { rows: 4, value: task?.prompt || "", required: true, placeholder: "What should QuixiDesk do each time?" });
    const frequency = select(frequencies, preset.frequency, () => updateSchedule(), { "aria-label": "Frequency" });
    const time = el("input", { type: "time", value: preset.time, onInput: () => updateSchedule() });
    const day = select(days, preset.day, () => updateSchedule());
    const minute = el("input", { type: "number", min: 0, max: 59, step: 1, value: preset.minute, onInput: () => updateSchedule() });
    const custom = el("input", { value: preset.custom, placeholder: "0 9 * * 1-5", autocomplete: "off", spellcheck: "false", onInput: () => updateSchedule() });
    const timeField = field("Time", time), dayField = field("Day", day), minuteField = field("Minutes past the hour", minute), cronField = field("Cron expression", custom, "Five fields: minute, hour, day, month, weekday. Sunday is 0 or 7.");
    const folder = el("input", { value: task ? (task.cwd || "") : (ctx.currentFolder?.() || ""), placeholder: "No folder selected", "aria-label": "Schedule folder" });
    const projectOptions = [["", "No project"], ...(state.projects || []).map(p => [p.id, p.name])];
    if (initialProject && !projectOptions.some(p => p[0] === initialProject)) projectOptions.push([initialProject, "Unavailable project — choose another"]);
    const project = select(projectOptions, initialProject, value => {
      const p = state.projects.find(p => p.id === value);
      if (p?.folders?.length === 1 && !folder.value.trim()) folder.value = p.folders[0].path;
      updateProjectHint(); updateContextSummary();
    });
    const projectHint = el("p", { class: "hint" });
    function updateProjectHint() {
      projectHint.textContent = project.value ? "Each run uses this project’s instructions. The folder below is its explicit working folder." : "Runs stay outside projects. Choose a folder only if the task needs local files.";
    }
    const provider = state.providers?.providers?.find(p => p.id === state.providers.activeProvider);
    const modelOptions = [["", "Use the active model at run time"]];
    for (const m of provider?.models || []) if (!modelOptions.some(o => o[0] === m.id)) modelOptions.push([m.id, m.label || m.id]);
    if (task?.model && !modelOptions.some(o => o[0] === task.model)) modelOptions.push([task.model, `${task.model} (saved model)`]);
    const model = select(modelOptions, task?.model || "");
    const permission = select([["default", "Manual — ask before changes"], ["auto", "Auto — allow workspace actions"]], task?.permissionMode || "default");
    const preview = el("div", { class: "schedule-preview", role: "status", "aria-live": "polite" });
    const error = el("div", { class: "form-error", role: "alert" });
    let timer = null, generation = 0, closed = false, validPreview = null;
    function expression() { return cronFromPreset({ frequency: frequency.value, time: time.value, day: day.value, minute: minute.value, custom: custom.value }); }
    async function refreshPreview() {
      const mine = ++generation;
      validPreview = null;
      let cron;
      try { cron = expression(); } catch (e) { preview.textContent = e.message; return; }
      preview.textContent = "Checking upcoming times…";
      try {
        const result = await rpc("CoworkScheduledTasks", "previewSchedule", [cron]);
        if (closed || mine !== generation) return;
        if (result?.error) throw new Error(result.error);
        if (!Array.isArray(result?.runs) || !result.runs.length) throw new Error("Could not calculate upcoming times.");
        validPreview = cron;
        preview.replaceChildren(el("strong", {}, "Upcoming runs"), el("ol", {}, result.runs.slice(0, 2).map(t => el("li", {}, new Date(t).toLocaleString([], { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZoneName: "short" })))), el("small", {}, `This computer’s time zone${result.timezone ? ` (${result.timezone})` : ""}.`));
      } catch (e) { if (!closed && mine === generation) preview.textContent = e.message; }
    }
    function updateSchedule() {
      timeField.hidden = !["daily", "weekdays", "weekly"].includes(frequency.value);
      dayField.hidden = frequency.value !== "weekly";
      minuteField.hidden = frequency.value !== "hourly";
      cronField.hidden = frequency.value !== "custom";
      ++generation; validPreview = null; clearTimeout(timer);
      timer = setTimeout(refreshPreview, 180);
    }
    const browse = button("Choose folder", async () => {
      const picked = await rpc("FileSystem", "browseFolder", ["Choose a folder for this schedule"]);
      const path = typeof picked === "string" ? picked : picked?.path;
      if (path) { folder.value = path; updateContextSummary(); }
    }, "quiet", "folder");
    const clearFolder = button("Clear", () => { folder.value = ""; updateContextSummary(); }, "quiet");
    const contextSummary = el("small", { class: "schedule-context-summary" });
    function updateContextSummary() {
      const selectedProject = state.projects.find(p => p.id === project.value);
      const folderName = folder.value.trim().split(/[\\/]/).filter(Boolean).pop();
      const modelLabel = model.selectedOptions?.[0]?.textContent || "Active model";
      contextSummary.textContent = [selectedProject?.name || "No project", folderName || "No folder", permission.value === "auto" ? "Auto" : "Manual", model.value ? modelLabel : "Active model"].join(" · ");
      contextSummary.title = folder.value.trim();
    }
    folder.addEventListener("input", updateContextSummary);
    permission.addEventListener("change", updateContextSummary);
    model.addEventListener("change", updateContextSummary);
    const contextDetails = el("details", { class: "schedule-context" },
      el("summary", {}, el("span", {}, "Context & permissions"), contextSummary),
      el("div", { class: "schedule-context-fields" },
        field("Project", project), projectHint,
        field("Working folder", folder), el("div", { class: "schedule-folder-actions" }, browse, clearFolder),
        el("div", { class: "schedule-time-grid" }, field("Model", model, provider ? `Models from ${provider.name}. Connections are checked when the run starts.` : "Configure a provider before the first run."), field("Permissions", permission))));
    const save = button(task ? "Save changes" : "Create schedule", async () => {
      error.textContent = "";
      save.disabled = true;
      try {
        if (!name.value.trim()) { name.focus(); throw new Error("Give this schedule a name."); }
        if (!prompt.value.trim()) { prompt.focus(); throw new Error("Describe what QuixiDesk should do."); }
        const cron = expression();
        if (validPreview !== cron) {
          const check = await rpc("CoworkScheduledTasks", "previewSchedule", [cron]);
          if (check?.error || !check?.runs?.length) throw new Error(check?.error || "Check this schedule’s times before saving.");
        }
        let cwd = folder.value.trim();
        if (cwd) {
          const checked = await api("/api/quixi/workspace/inspect", { path: cwd });
          if (!checked?.exists) { contextDetails.open = true; folder.focus(); throw new Error("Choose an existing folder, or clear it to run without local files."); }
          cwd = checked.path;
        }
        const result = await rpc("CoworkScheduledTasks", task ? "updateScheduledTask" : "createScheduledTask", [{ id: task?.id, name: name.value.trim(), prompt: prompt.value.trim(), cronExpression: cron, cwd, spaceId: project.value || null, model: model.value, permissionMode: permission.value, enabled: task?.enabled ?? true }]);
        if (!result || result.error) throw new Error(result?.error || "The schedule was not saved.");
        d.close();
        await ctx.loadSchedules();
      } catch (e) { error.textContent = e.message; }
      finally { save.disabled = false; }
    }, "primary");
    const d = dialog(task ? "Edit schedule" : "New schedule", [
      field("Name", name), field("Instructions", prompt),
      el("div", { class: "schedule-time-grid" }, field("Frequency", frequency), timeField, dayField, minuteField), cronField, preview,
      contextDetails,
      el("p", { class: "hint" }, "Keep QuixiDesk open for scheduled runs. Times missed while the app is closed or asleep are skipped, not replayed. Manual mode waits for approval before changes or commands."),
      error,
    ]);
    d.append(el("div", { class: "form-actions schedule-editor-actions" }, button("Cancel", () => d.close(), "quiet"), save));
    d.classList.add("schedule-editor");
    d.addEventListener("close", () => { closed = true; generation++; clearTimeout(timer); });
    updateProjectHint(); updateContextSummary(); updateSchedule();
    return d;
  }
  window.QuixiSchedules = { createEditor, parsePreset, cronFromPreset, describe, nextRunLabel, runStatus };
})();
