//! Working-directory switching, cross-session search, Ralph fresh-agent loops,
//! and deterministic multi-agent Workflows.
//!
//! Inspired by DeepSeek Harness (`dsh-working-directory`, `dsh-tool-session-query`,
//! `dsh-tool-ralph`, and `dsh-workflow-ptc`).

use super::*;

pub const TOOL_WORKING_DIRECTORY: &str = "WorkingDirectory";
pub const TOOL_SESSION_SEARCH: &str = "SessionSearch";
pub const TOOL_RALPH: &str = "Ralph";
pub const TOOL_WORKFLOW: &str = "Workflow";

const MAX_RALPH_ROUNDS: usize = 16;
const MAX_HANDOFF_CHARS: usize = 16_000;
const MAX_WORKFLOW_PHASES: usize = 6;
const MAX_WORKFLOW_TOTAL_STEPS: usize = 12;

pub fn workflow_tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": TOOL_WORKING_DIRECTORY,
                "description": "Read or change this conversation's active working directory for subsequent file and shell calls. Original workspace permission boundaries remain fixed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["get", "set"],
                            "description": "Read current directory (`get`) or switch to `path` (`set`)."
                        },
                        "path": {
                            "type": "string",
                            "description": "Target directory when action is `set` (relative to current directory or absolute)."
                        }
                    },
                    "required": ["action"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_SESSION_SEARCH,
                "description": "Search past conversations belonging to this same workspace or project for historical context, decisions, or fixes. Available in Manual mode without approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Keywords or phrase to match across titles and messages." },
                        "limit": { "type": "integer", "description": "Maximum results (1-20, default 8)." }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_RALPH,
                "description": "Run an iterative sequence of fresh-context child agents toward one immutable objective. Each round starts with a clean context window and receives only the objective plus the previous round's bounded handoff (`continue`, `complete`, or `blocked`), preventing context bloat on multi-step refactors or migrations.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "objective": { "type": "string", "description": "Complete, self-contained objective and acceptance criteria." },
                        "max_rounds": { "type": "integer", "description": "Maximum fresh-agent rounds (1-16, default 5)." },
                        "allow_edits": { "type": "boolean", "description": "Permit file edits and commands in each round under parent permissions (default false)." }
                    },
                    "required": ["objective"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_WORKFLOW,
                "description": "Orchestrate a structured multi-phase subagent pipeline (`parallel` fan-out or `sequential` stages). Earlier phase outputs are automatically passed as structured context to later phases.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Short workflow identifier, e.g. `review-and-verify`." },
                        "description": { "type": "string", "description": "One-line workflow summary." },
                        "phases": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "title": { "type": "string" },
                                    "mode": { "type": "string", "enum": ["parallel", "sequential"] },
                                    "steps": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": { "type": "string" },
                                                "prompt": { "type": "string" },
                                                "allow_edits": { "type": "boolean" }
                                            },
                                            "required": ["label", "prompt"]
                                        }
                                    }
                                },
                                "required": ["title", "steps"]
                            }
                        }
                    },
                    "required": ["name", "phases"],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RalphHandoff {
    status: String, // "continue" | "complete" | "blocked"
    summary: String,
    #[serde(default)]
    next_step: String,
}

fn parse_ralph_handoff(raw: &str) -> RalphHandoff {
    // Extract `<ralph-handoff>{...}</ralph-handoff>` or ```json {...} ``` or fallback to heuristic
    if let Some(start) = raw.find("<ralph-handoff>") {
        let after = &raw[start + "<ralph-handoff>".len()..];
        if let Some(end) = after.find("</ralph-handoff>") {
            if let Ok(parsed) = serde_json::from_str::<RalphHandoff>(after[..end].trim()) {
                return parsed;
            }
        }
    }
    if let Ok(parsed) = serde_json::from_str::<RalphHandoff>(raw.trim()) {
        return parsed;
    }
    let lower = raw.to_ascii_lowercase();
    let status = if lower.contains("\"status\": \"complete\"")
        || lower.contains("status: complete")
        || lower.contains("objective is complete")
    {
        "complete"
    } else if lower.contains("\"status\": \"blocked\"") || lower.contains("status: blocked") {
        "blocked"
    } else {
        "continue"
    };
    RalphHandoff {
        status: status.to_string(),
        summary: raw.chars().take(MAX_HANDOFF_CHARS / 2).collect(),
        next_step: String::new(),
    }
}

impl QuixiEngine {
    /// Reads or updates the session's effective `cwd` without changing original `user_selected_folders`.
    pub(crate) fn execute_working_directory_tool(
        &self,
        session_id: &str,
        current_cwd: &str,
        input: &Value,
    ) -> (String, bool) {
        let action = input["action"].as_str().unwrap_or("get");
        if action == "get" {
            let roots = self
                .agent_sessions
                .read()
                .get(session_id)
                .map(|s| s.user_selected_folders.clone())
                .unwrap_or_default();
            return (
                json!({
                    "cwd": current_cwd,
                    "permissionRoots": roots,
                })
                .to_string(),
                false,
            );
        }
        if action != "set" {
            return (format!("Invalid WorkingDirectory action `{action}`."), true);
        }
        let Some(raw) = input.get("path").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) else {
            return ("WorkingDirectory `set` requires a non-empty `path`.".into(), true);
        };
        let resolved = agent_tools::resolve_path(raw, current_cwd);
        let Ok(canon) = fs::canonicalize(&resolved) else {
            return (
                format!("Directory does not exist: {}", resolved.display()),
                true,
            );
        };
        if !canon.is_dir() {
            return (
                format!("Target is not a directory: {}", canon.display()),
                true,
            );
        }
        let new_cwd = canon.to_string_lossy().into_owned();
        if let Some(s) = self.agent_sessions.write().get_mut(session_id) {
            s.cwd = new_cwd.clone();
            self.persist_agent_session(s);
        } else if let Some(s) = self.code_sessions.write().get_mut(session_id) {
            s.cwd = new_cwd.clone();
            self.persist_code_session(s);
        }
        (
            json!({
                "cwd": new_cwd,
                "note": "Changed active working directory for subsequent tool calls; original workspace permission boundaries remain unchanged."
            })
            .to_string(),
            false,
        )
    }

    /// Searches past conversations authorized to the calling session's project or workspace root.
    pub(crate) fn execute_session_search_tool(
        &self,
        caller_session_id: &str,
        caller_cwd: &str,
        input: &Value,
    ) -> (String, bool) {
        let query = input["query"].as_str().unwrap_or("").trim();
        if query.is_empty() {
            return ("SessionSearch `query` must not be empty.".into(), true);
        }
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(8)
            .clamp(1, 20) as usize;
        let caller_space = self
            .agent_sessions
            .read()
            .get(caller_session_id)
            .and_then(|s| s.space_id.clone());
        let caller_canon = (!caller_cwd.trim().is_empty())
            .then(|| fs::canonicalize(caller_cwd).ok())
            .flatten();

        let needle = query.to_lowercase();
        let mut results = Vec::new();

        for s in self.agent_sessions.read().values() {
            if s.session_id == caller_session_id {
                continue;
            }
            let same_space = caller_space.is_some() && s.space_id == caller_space;
            let same_cwd = match (&caller_canon, (!s.cwd.trim().is_empty()).then(|| fs::canonicalize(&s.cwd).ok()).flatten()) {
                (Some(a), Some(b)) => *a == b || b.starts_with(a) || a.starts_with(&b),
                _ => caller_cwd.trim().is_empty() && s.cwd.trim().is_empty(),
            };
            if !same_space && !same_cwd {
                continue;
            }

            let mut matched_snippet: Option<String> = None;
            if s.title.to_lowercase().contains(&needle) {
                matched_snippet = Some(format!("Title match: {}", s.title));
            }
            if matched_snippet.is_none() {
                for m in &s.buffered_messages {
                    if !matches!(m["type"].as_str(), Some("user" | "assistant")) {
                        continue;
                    }
                    let Some(blocks) = m.pointer("/message/content").and_then(Value::as_array) else {
                        continue;
                    };
                    for b in blocks {
                        // Exclude reasoning/thinking blocks from semantic session search (`dsh-session-query`).
                        if b["type"] != "text" {
                            continue;
                        }
                        let Some(txt) = b["text"].as_str() else {
                            continue;
                        };
                        if let Some(byte_pos) = txt.to_lowercase().find(&needle) {
                            let char_idx = txt[..byte_pos].chars().count();
                            let start_c = char_idx.saturating_sub(60);
                            let snippet: String = txt.chars().skip(start_c).take(220).collect();
                            matched_snippet = Some(snippet.replace('\n', " "));
                            break;
                        }
                    }
                    if matched_snippet.is_some() {
                        break;
                    }
                }
            }
            if let Some(snippet) = matched_snippet {
                results.push((
                    s.last_activity_at,
                    json!({
                        "sessionId": s.session_id,
                        "title": s.title,
                        "lastActivityAt": s.last_activity_at,
                        "snippet": snippet,
                        "referenceSyntax": format!("@session:{}", s.session_id),
                    }),
                ));
            }
        }

        results.sort_by(|a, b| b.0.cmp(&a.0));
        let hits: Vec<Value> = results.into_iter().take(limit).map(|(_, v)| v).collect();
        (
            json!({
                "query": query,
                "count": hits.len(),
                "results": hits,
            })
            .to_string(),
            false,
        )
    }

    /// Executes a `Ralph` fresh-agent loop: sequential fresh child agents passing only a bounded
    /// `<= 16 KiB` handoff across rounds.
    pub(crate) fn run_ralph_loop(
        &self,
        session_id: &str,
        parent_call: &str,
        input: &Value,
        cwd: &str,
        mode: &str,
        parent_model: &str,
        cancelled: &AtomicBool,
    ) -> (String, bool) {
        let objective = input["objective"].as_str().unwrap_or("").trim();
        if objective.is_empty() || objective.len() > 16_000 {
            return (
                "Ralph requires a non-empty `objective` (up to 16,000 bytes).".into(),
                true,
            );
        }
        let max_rounds = input
            .get("max_rounds")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, MAX_RALPH_ROUNDS as u64) as usize;
        let allow_edits = input
            .get("allow_edits")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut prior_handoff: Option<RalphHandoff> = None;
        let mut history_summaries = Vec::new();

        for round in 1..=max_rounds {
            if cancelled.load(Ordering::SeqCst) {
                return ("Ralph loop stopped.".into(), true);
            }
            let handoff_section = match &prior_handoff {
                Some(h) => format!(
                    "\n\nPrevious round handoff (round {}/{}):\nSummary: {}\nSuggested next step: {}\n(Verify workspace state directly; prior agent text is advisory.)",
                    round - 1,
                    max_rounds,
                    h.summary.chars().take(MAX_HANDOFF_CHARS / 2).collect::<String>(),
                    h.next_step.chars().take(2_000).collect::<String>()
                ),
                None => String::new(),
            };
            let child_prompt = format!(
                "Ralph Fresh-Agent Iteration Round {round}/{max_rounds}\n\nImmutable Objective:\n{objective}{handoff_section}\n\nWork on the next concrete piece of this objective. At the very end of your response, include a `<ralph-handoff>` JSON block:\n<ralph-handoff>\n{{\"status\": \"continue\" | \"complete\" | \"blocked\", \"summary\": \"concise factual summary of completed work and verified state\", \"next_step\": \"exact next step if status is continue, or blocker reason\"}}\n</ralph-handoff>"
            );
            let title = format!("Ralph [{round}/{max_rounds}]");
            let child_input = json!({
                "title": title,
                "prompt": child_prompt.chars().take(15_900).collect::<String>(),
                "allow_edits": allow_edits,
            });
            let call_id = format!("{parent_call}:ralph:{round}");
            let (raw_out, failed) = self.run_subagent_with_context(
                session_id,
                &call_id,
                &child_input,
                cwd,
                mode,
                parent_model,
                &[],
                cancelled,
            );
            let child_json: Value = serde_json::from_str(&raw_out).unwrap_or_else(|_| json!({}));
            let answer_text = child_json["summary"]
                .as_str()
                .or_else(|| child_json["error"].as_str())
                .unwrap_or(&raw_out);
            if failed {
                return (
                    json!({
                        "status": "failed",
                        "roundsCompleted": round - 1,
                        "failedRound": round,
                        "error": answer_text,
                        "history": history_summaries,
                    })
                    .to_string(),
                    true,
                );
            }
            let handoff = parse_ralph_handoff(answer_text);
            history_summaries.push(json!({
                "round": round,
                "status": handoff.status,
                "summary": handoff.summary,
                "nextStep": handoff.next_step,
            }));
            match handoff.status.as_str() {
                "complete" => {
                    return (
                        json!({
                            "status": "complete",
                            "roundsUsed": round,
                            "summary": handoff.summary,
                            "history": history_summaries,
                        })
                        .to_string(),
                        false,
                    );
                }
                "blocked" => {
                    return (
                        json!({
                            "status": "blocked",
                            "roundsUsed": round,
                            "summary": handoff.summary,
                            "blocker": handoff.next_step,
                            "history": history_summaries,
                        })
                        .to_string(),
                        true,
                    );
                }
                _ => {
                    prior_handoff = Some(handoff);
                }
            }
        }

        (
            json!({
                "status": "round_cap_reached",
                "roundsUsed": max_rounds,
                "lastHandoff": prior_handoff.map(|h| json!({"summary": h.summary, "nextStep": h.next_step})),
                "history": history_summaries,
            })
            .to_string(),
            false,
        )
    }

    /// Executes a deterministic multi-phase `Workflow` (`parallel` or `sequential` subagent stages).
    pub(crate) fn run_structured_workflow(
        &self,
        session_id: &str,
        parent_call: &str,
        input: &Value,
        cwd: &str,
        mode: &str,
        parent_model: &str,
        cancelled: &AtomicBool,
    ) -> (String, bool) {
        let wf_name = input["name"].as_str().unwrap_or("").trim();
        let Some(phases) = input["phases"].as_array().filter(|p| !p.is_empty()) else {
            return ("Workflow requires a non-empty `phases` array.".into(), true);
        };
        if wf_name.is_empty() || phases.len() > MAX_WORKFLOW_PHASES {
            return (
                format!("Workflow requires `name` and 1..={MAX_WORKFLOW_PHASES} phases."),
                true,
            );
        }
        let total_steps: usize = phases
            .iter()
            .map(|p| p["steps"].as_array().map_or(0, Vec::len))
            .sum();
        if total_steps == 0 || total_steps > MAX_WORKFLOW_TOTAL_STEPS {
            return (
                format!("Workflow total steps must be between 1 and {MAX_WORKFLOW_TOTAL_STEPS}."),
                true,
            );
        }

        let mut phase_outputs: Vec<Value> = Vec::with_capacity(phases.len());

        for (p_idx, phase) in phases.iter().enumerate() {
            if cancelled.load(Ordering::SeqCst) {
                return ("Workflow cancelled.".into(), true);
            }
            let p_title = phase["title"].as_str().unwrap_or("Phase").trim();
            let p_mode = phase["mode"].as_str().unwrap_or("parallel");
            let Some(steps) = phase["steps"].as_array().filter(|s| !s.is_empty()) else {
                return (format!("Phase `{p_title}` has no steps."), true);
            };

            let prior_context = if phase_outputs.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\nPrior workflow phase results:\n{}",
                    serde_json::to_string_pretty(&phase_outputs)
                        .unwrap_or_default()
                        .chars()
                        .take(8_000)
                        .collect::<String>()
                )
            };

            let can_parallel = p_mode == "parallel"
                && steps.len() >= 2
                && steps
                    .iter()
                    .all(|st| st.get("allow_edits").and_then(Value::as_bool) != Some(true));

            let step_results: Vec<Value> = if can_parallel {
                std::thread::scope(|scope| {
                    let mut handles = Vec::new();
                    for (s_idx, step) in steps.iter().enumerate() {
                        let label = step["label"].as_str().unwrap_or("step").to_string();
                        let prompt = format!(
                            "{}{}",
                            step["prompt"].as_str().unwrap_or(""),
                            prior_context
                        )
                        .chars()
                        .take(15_900)
                        .collect::<String>();
                        let cid = format!("{parent_call}:wf:{p_idx}:{s_idx}");
                        let title = format!("{wf_name} · {p_title} · {label}")
                            .chars()
                            .take(150)
                            .collect::<String>();
                        handles.push((
                            label,
                            scope.spawn(move || {
                                self.run_subagent_with_context(
                                    session_id,
                                    &cid,
                                    &json!({"title": title, "prompt": prompt, "allow_edits": false}),
                                    cwd,
                                    mode,
                                    parent_model,
                                    &[],
                                    cancelled,
                                )
                            }),
                        ));
                    }
                    handles
                        .into_iter()
                        .map(|(label, h)| {
                            let (out, is_err) = h
                                .join()
                                .unwrap_or_else(|_| ("Step panicked".into(), true));
                            let parsed: Value = serde_json::from_str(&out).unwrap_or_else(|_| json!({}));
                            json!({
                                "label": label,
                                "ok": !is_err,
                                "summary": parsed.get("summary").cloned().unwrap_or_else(|| json!(out)),
                                "error": parsed.get("error").cloned().unwrap_or(Value::Null),
                            })
                        })
                        .collect()
                })
            } else {
                let mut seq_out = Vec::with_capacity(steps.len());
                for (s_idx, step) in steps.iter().enumerate() {
                    if cancelled.load(Ordering::SeqCst) {
                        return ("Workflow cancelled.".into(), true);
                    }
                    let label = step["label"].as_str().unwrap_or("step");
                    let edits = step.get("allow_edits").and_then(Value::as_bool).unwrap_or(false);
                    let prompt = format!(
                        "{}{}",
                        step["prompt"].as_str().unwrap_or(""),
                        prior_context
                    )
                    .chars()
                    .take(15_900)
                    .collect::<String>();
                    let cid = format!("{parent_call}:wf:{p_idx}:{s_idx}");
                    let title = format!("{wf_name} · {p_title} · {label}")
                        .chars()
                        .take(150)
                        .collect::<String>();
                    let (out, is_err) = self.run_subagent_with_context(
                        session_id,
                        &cid,
                        &json!({"title": title, "prompt": prompt, "allow_edits": edits}),
                        cwd,
                        mode,
                        parent_model,
                        &[],
                        cancelled,
                    );
                    let parsed: Value = serde_json::from_str(&out).unwrap_or_else(|_| json!({}));
                    seq_out.push(json!({
                        "label": label,
                        "ok": !is_err,
                        "summary": parsed.get("summary").cloned().unwrap_or_else(|| json!(out)),
                        "error": parsed.get("error").cloned().unwrap_or(Value::Null),
                    }));
                }
                seq_out
            };

            phase_outputs.push(json!({
                "phase": p_title,
                "mode": p_mode,
                "steps": step_results,
            }));
        }

        (
            json!({
                "workflow": wf_name,
                "phases": phase_outputs,
            })
            .to_string(),
            false,
        )
    }

    /// Exports a session transcript + collaboration metadata as portable Markdown (`/export`).
    pub fn export_session_markdown(&self, session_id: &str) -> Value {
        let (title, cwd, model, messages) = if let Some(s) = self.agent_sessions.read().get(session_id) {
            (
                s.title.clone(),
                s.cwd.clone(),
                s.model.clone(),
                s.buffered_messages.clone(),
            )
        } else if let Some(s) = self.code_sessions.read().get(session_id) {
            let tr = self
                .code_transcripts
                .read()
                .get(session_id)
                .cloned()
                .unwrap_or_default();
            (s.title.clone(), s.cwd.clone(), s.model.clone(), tr)
        } else {
            return json!({"error": "Conversation not found."});
        };

        let mut md = format!(
            "# {}\n\n- **Session ID**: `{}`\n- **Model**: `{}`\n- **Workspace**: `{}`\n\n---\n\n",
            if title.trim().is_empty() { "Untitled Conversation" } else { &title },
            session_id,
            model,
            if cwd.is_empty() { "(none)" } else { &cwd }
        );

        for m in &messages {
            match m["type"].as_str() {
                Some("user") => {
                    let Some(blocks) = m.pointer("/message/content").and_then(Value::as_array) else {
                        continue;
                    };
                    let texts: Vec<&str> = blocks
                        .iter()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect();
                    if !texts.is_empty() {
                        md.push_str(&format!("## You\n\n{}\n\n", texts.join("\n\n")));
                    }
                    for b in blocks.iter().filter(|b| b["type"] == "tool_result") {
                        let status = if b["is_error"] == true { "Error" } else { "Result" };
                        let out = b["content"].as_str().unwrap_or("");
                        md.push_str(&format!(
                            "<details><summary>Tool {} ({})</summary>\n\n```text\n{}\n```\n\n</details>\n\n",
                            status,
                            b["tool_use_id"].as_str().unwrap_or(""),
                            out.chars().take(4_000).collect::<String>()
                        ));
                    }
                }
                Some("assistant") => {
                    let Some(blocks) = m.pointer("/message/content").and_then(Value::as_array) else {
                        continue;
                    };
                    let texts: Vec<&str> = blocks
                        .iter()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect();
                    if !texts.is_empty() {
                        md.push_str(&format!("## QuixiDesk\n\n{}\n\n", texts.join("\n\n")));
                    }
                    for b in blocks.iter().filter(|b| b["type"] == "tool_use") {
                        md.push_str(&format!(
                            "- **Tool Call**: `{}` (`{}`)\n\n",
                            b["name"].as_str().unwrap_or("tool"),
                            b["id"].as_str().unwrap_or("")
                        ));
                    }
                }
                Some("compaction_checkpoint") => {
                    md.push_str(&format!(
                        "> **Context Checkpoint**:\n>\n> {}\n\n",
                        m["summary"].as_str().unwrap_or("").replace('\n', "\n> ")
                    ));
                }
                _ => {}
            }
        }

        json!({
            "sessionId": session_id,
            "title": title,
            "markdown": md,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_search_respects_workspace_boundaries_and_excludes_thinking_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let ws_a = dir.path().join("ws_a");
        let ws_b = dir.path().join("ws_b");
        fs::create_dir_all(&ws_a).unwrap();
        fs::create_dir_all(&ws_b).unwrap();
        let engine = QuixiEngine::new_with_data_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
            dir.path().join("data"),
        );

        let sid_a1 = engine.start_agent_session(&json!({"cwd": ws_a, "title": "Auth refactor"}));
        let sid_a2 = engine.start_agent_session(&json!({"cwd": ws_a, "title": "Current task"}));
        let sid_b1 = engine.start_agent_session(&json!({"cwd": ws_b, "title": "Other repo secret"}));

        engine.agent_sessions.write().get_mut(&sid_a1).unwrap().buffered_messages.push(json!({
            "type": "assistant",
            "message": {
                "content": [
                    { "type": "thinking", "thinking": "PRIVATE_THINKING_ONLY_TOKEN" },
                    { "type": "text", "text": "Fixed token refresh race in auth_middleware.rs" }
                ]
            }
        }));
        engine.agent_sessions.write().get_mut(&sid_b1).unwrap().buffered_messages.push(json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": "auth_middleware in secret repo B" }] }
        }));

        // Same workspace finds sid_a1, NOT sid_b1:
        let (out, err) = engine.execute_session_search_tool(
            &sid_a2,
            ws_a.to_str().unwrap(),
            &json!({"query": "auth_middleware"}),
        );
        assert!(!err);
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["count"], 1);
        assert_eq!(parsed["results"][0]["sessionId"], sid_a1);

        // Private thinking tokens are excluded from search:
        let (out_th, _) = engine.execute_session_search_tool(
            &sid_a2,
            ws_a.to_str().unwrap(),
            &json!({"query": "PRIVATE_THINKING_ONLY_TOKEN"}),
        );
        let parsed_th: Value = serde_json::from_str(&out_th).unwrap();
        assert_eq!(parsed_th["count"], 0);
    }
}
