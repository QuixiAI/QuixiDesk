//! In-turn approvals: keep Manual useful without silently granting actions.
use super::*;
use std::sync::atomic::AtomicU8;
use std::time::{Duration, Instant};

pub(crate) struct PendingToolApproval {
    session_id: String,
    request: Value,
    pub(crate) decision: Arc<AtomicU8>,
    selected_subagent_model: Arc<parking_lot::Mutex<Option<(String, Option<String>)>>>,
    interaction_reply: Arc<parking_lot::Mutex<Option<Value>>>,
}

fn canonical_or_parent(path: &Path) -> Option<PathBuf> {
    if let Ok(p) = path.canonicalize() {
        return Some(p);
    }
    let name = path.file_name()?;
    let parent = canonical_or_parent(path.parent()?)?;
    Some(parent.join(name))
}

// Resolve existing symlinks and normalize missing descendants before checking scope.
#[cfg(test)]
fn file_access_outside_workspace(name:&str,input:&Value,cwd:&str)->bool {
    file_access_outside_workspace_with_spill(name, input, cwd, None)
}

fn file_access_outside_workspace_with_spill(
    name: &str,
    input: &Value,
    cwd: &str,
    spill_allowance: Option<(&Path, &str)>,
) -> bool {
    let check_one = |raw: &str, allow_spill: bool| -> bool {
        let p = Path::new(raw);
        let joined = if p.is_absolute() {
            p.to_path_buf()
        } else {
            Path::new(cwd).join(p)
        };
        if allow_spill {
            if let Some((data_dir, sid)) = spill_allowance {
                if crate::spill::is_session_spill_path(data_dir, sid, &joined) {
                    return false;
                }
            }
        }
        match (Path::new(cwd).canonicalize(), canonical_or_parent(&joined)) {
            (Ok(root), Some(resolved)) => !resolved.starts_with(root),
            _ => true,
        }
    };

    match name {
        "Read" | "Write" | "Edit" | "Lsp" => {
            let raw = input["file_path"].as_str().unwrap_or(".");
            check_one(raw, name == "Read")
        }
        "ListDirectory" | "Grep" | "Glob" => {
            let raw = input["path"].as_str().unwrap_or(".");
            check_one(raw, name == "Grep")
        }
        "TerminalOpen" => match input.get("cwd").and_then(Value::as_str) {
            Some(raw) if !raw.trim().is_empty() => check_one(raw, false),
            _ => false,
        },
        "WorkingDirectory" => {
            if input["action"].as_str() == Some("set") {
                check_one(input["path"].as_str().unwrap_or("."), false)
            } else {
                false
            }
        }
        "ApplyPatch" => {
            let patch = input["patch"].as_str().unwrap_or("");
            match crate::code_tools::parse_patch_operations(patch, Path::new(cwd)) {
                Ok(ops) => ops.iter().any(|op| {
                    let p = match op {
                        crate::code_tools::PatchFileOp::Add { path, .. }
                        | crate::code_tools::PatchFileOp::Update { path, .. }
                        | crate::code_tools::PatchFileOp::Delete { path } => path,
                    };
                    check_one(&p.to_string_lossy(), false)
                }),
                Err(_) => false,
            }
        }
        "Present" => input["files"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|f| check_one(f["path"].as_str().unwrap_or("."), false))),
        _ => false,
    }
}

impl QuixiEngine {
    fn permission_event(&self, session_id: &str, kind: &str) {
        let service = if self.agent_sessions.read().contains_key(session_id) {
            "LocalAgentModeSessions"
        } else { "LocalSessions" };
        self.emit_eipc("quixi.web", service, "onEvent", json!({"type":kind,"sessionId":session_id}));
    }

    pub fn pending_tool_approvals(&self, session_id: &str) -> Value {
        json!(self.pending_approvals.read().values()
            .filter(|p|p.session_id==session_id && matches!(p.decision.load(Ordering::SeqCst),0|3)).map(|p|p.request.clone()).collect::<Vec<_>>())
    }

    pub fn resolve_tool_approval(&self, session_id: &str, request_id: &str, decision: &str) -> Value {
        let (signal, file_edit, request, subagent_choice, interaction_reply) = {
            let pending=self.pending_approvals.read();
            let Some(p)=pending.get(request_id).filter(|p|p.session_id==session_id) else {
                return json!({"error":"This approval is no longer pending."});
            };
            (
                p.decision.clone(),
                matches!(p.request["toolName"].as_str(),Some("Write"|"Edit"|"ApplyPatch")) && p.request["outsideWorkspace"]!=true,
                p.request.clone(),
                p.selected_subagent_model.clone(),
                p.interaction_reply.clone(),
            )
        };
        if request["kind"] == "plan_review" {
            let (code, payload) = if decision == "approve" || decision == "allow_once" {
                (1u8, json!({"approved": true}))
            } else if let Some(feedback) = decision.strip_prefix("revise:") {
                (2u8, json!({"approved": false, "feedback": feedback.trim()}))
            } else if decision == "deny" {
                (2u8, json!({"approved": false}))
            } else {
                return json!({"error": "Invalid plan review decision."});
            };
            if signal.compare_exchange(0, 3, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                return json!({"error": "This plan review was already answered."});
            }
            *interaction_reply.lock() = Some(payload);
            signal.store(code, Ordering::SeqCst);
            self.permission_event(session_id, "tool_permission_changed");
            return json!({"ok": true});
        }
        if request["kind"] == "ask_user" {
            let (code, payload) = if let Some(raw_json) = decision.strip_prefix("answer:") {
                match serde_json::from_str::<Value>(raw_json) {
                    Ok(v) => (1u8, v),
                    Err(_) => return json!({"error": "Invalid question answer payload."}),
                }
            } else if decision == "deny" {
                (2u8, json!({"dismissed": true}))
            } else {
                return json!({"error": "Invalid question response."});
            };
            if signal.compare_exchange(0, 3, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                return json!({"error": "This question was already answered."});
            }
            *interaction_reply.lock() = Some(payload);
            signal.store(code, Ordering::SeqCst);
            self.permission_event(session_id, "tool_permission_changed");
            return json!({"ok": true});
        }
        if request["kind"] == "subagent_model" {
            let parent_model = request["parentModel"].as_str().unwrap_or_default().to_string();
            let requested_model = request["requestedModel"].as_str().unwrap_or_default().to_string();
            let requested_provider = request["requestedProvider"].as_str().map(str::to_string);
            let parsed: Result<(u8, Option<(String, Option<String>)>, Option<(String, String)>), String> = match decision {
                "deny" => Ok((2, None, None)),
                "use_parent_model" => Ok((1, Some((parent_model, None)), None)),
                "allow_once" if !requested_model.is_empty() && requested_provider.is_some() => {
                    Ok((1, Some((requested_model, requested_provider)), None))
                }
                "allow_always" if !requested_model.is_empty() && requested_provider.is_some() => {
                    let pid = requested_provider.clone().unwrap();
                    Ok((1, Some((requested_model.clone(), Some(pid.clone()))), Some((pid, requested_model))))
                }
                _ if decision.starts_with("select_once:") || decision.starts_with("select_always:") => {
                    let always = decision.starts_with("select_always:");
                    let rest = decision.split_once(':').map(|(_, r)| r).unwrap_or("");
                    if let Some((pid, mid)) = rest.split_once(':') {
                        let pid = pid.trim();
                        let mid = mid.trim();
                        if !pid.is_empty() && !mid.is_empty() {
                            let valid = self
                                .provider_manager
                                .available_subagent_models()
                                .into_iter()
                                .any(|m| m["providerId"] == pid && m["model"] == mid);
                            if valid {
                                Ok((
                                    1,
                                    Some((mid.to_string(), Some(pid.to_string()))),
                                    always.then(|| (pid.to_string(), mid.to_string())),
                                ))
                            } else {
                                Err("Choose a tool-capable model from a configured provider.".into())
                            }
                        } else {
                            Err("Choose a valid subagent model.".into())
                        }
                    } else {
                        Err("Invalid subagent model choice.".into())
                    }
                }
                _ => Err("Invalid approval choice.".into()),
            };
            let (answer, chosen, persist) = match parsed {
                Ok(v) => v,
                Err(msg) => return json!({"error": msg}),
            };
            if signal.compare_exchange(0,3,Ordering::SeqCst,Ordering::SeqCst).is_err() {
                return json!({"error":"This approval was already answered."});
            }
            if let Some((pid, mid)) = persist {
                if let Err(err) = self.provider_manager.save_subagent_selection(&pid, &mid) {
                    signal.store(0, Ordering::SeqCst);
                    return json!({"error": err});
                }
            }
            *subagent_choice.lock() = chosen;
            signal.store(answer, Ordering::SeqCst);
            self.permission_event(session_id, "tool_permission_changed");
            return json!({"ok":true});
        }
        let answer=match decision {
            "allow_once" => 1,
            "allow_file_edits" if file_edit => 1,
            "deny" => 2,
            _ => return json!({"error":"Invalid approval choice."}),
        };
        // Reserve the request before changing mode, so it cannot be resolved twice.
        if signal.compare_exchange(0,3,Ordering::SeqCst,Ordering::SeqCst).is_err() {
            return json!({"error":"This approval was already answered."});
        }
        if decision=="allow_file_edits" {
            if let Some(s)=self.agent_sessions.write().get_mut(session_id) {
                s.permission_mode="acceptEdits".into(); self.persist_agent_session(s);
            } else if let Some(s)=self.code_sessions.write().get_mut(session_id) {
                s.permission_mode="acceptEdits".into(); self.persist_code_session(s);
            }
        }
        signal.store(answer,Ordering::SeqCst);
        self.permission_event(session_id,"tool_permission_changed");
        json!({"ok":true})
    }

    pub(crate) fn request_subagent_model_approval(
        &self,
        session_id: &str,
        call_id: &str,
        title: &str,
        parent_model: &str,
        requested_model: &str,
        requested_provider: Option<&str>,
        cancelled: &AtomicBool,
    ) -> Result<(String, Option<String>), String> {
        if cancelled.load(Ordering::SeqCst) {
            return Err("Task stopped before delegation.".into());
        }
        let request_id = Uuid::new_v4().to_string();
        let signal = Arc::new(AtomicU8::new(0));
        let selected = Arc::new(parking_lot::Mutex::new(None));
        let available = self.provider_manager.available_subagent_models();
        self.pending_approvals.write().insert(
            request_id.clone(),
            PendingToolApproval {
                session_id: session_id.into(),
                decision: signal.clone(),
                selected_subagent_model: selected.clone(),
                interaction_reply: Arc::new(parking_lot::Mutex::new(None)),
                request: json!({
                    "requestId": request_id,
                    "sessionId": session_id,
                    "toolCallId": call_id,
                    "toolName": "Task",
                    "kind": "subagent_model",
                    "title": title,
                    "parentModel": parent_model,
                    "requestedModel": requested_model,
                    "requestedProvider": requested_provider,
                    "availableModels": available,
                    "input": {
                        "title": title,
                        "model": requested_model,
                    },
                }),
            },
        );
        self.permission_event(session_id, "tool_permission_requested");
        let start = Instant::now();
        let outcome = loop {
            if cancelled.load(Ordering::SeqCst) {
                break 2;
            }
            let decision = signal.load(Ordering::SeqCst);
            if decision == 1 || decision == 2 {
                break decision;
            }
            if start.elapsed() > Duration::from_secs(30 * 60) {
                break 2;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        self.pending_approvals.write().remove(&request_id);
        self.permission_event(session_id, "tool_permission_changed");
        if outcome != 1 || cancelled.load(Ordering::SeqCst) {
            return Err("Subagent model request was not approved or the task was stopped. No subagent ran.".into());
        }
        let choice = selected.lock().clone();
        choice.ok_or_else(|| "Subagent model choice was not recorded.".into())
    }

    pub(crate) fn cancel_tool_approvals(&self, session_id: &str) {
        self.pending_approvals.write().retain(|_,p| {
            if p.session_id==session_id {p.decision.store(2,Ordering::SeqCst);false} else {true}
        });
    }

    fn run_plan_review(
        &self,
        session_id: &str,
        call_id: &str,
        input: &Value,
        cancelled: &AtomicBool,
    ) -> (String, bool) {
        if !self.collaboration.is_plan_active(session_id) {
            return (
                "ExitPlanMode is only callable while Plan Mode is active.".into(),
                true,
            );
        }
        let plan = input["plan"].as_str().unwrap_or("").trim();
        if !plan.starts_with('#') {
            return (
                "ExitPlanMode requires a complete Markdown plan starting with a `#` heading.".into(),
                true,
            );
        }
        let plan_title = plan
            .lines()
            .next()
            .unwrap_or("Plan")
            .trim_start_matches('#')
            .trim()
            .chars()
            .take(120)
            .collect::<String>();
        let now_ms = chrono::Utc::now().timestamp_millis();
        self.collaboration.mutate(session_id, |st| {
            st.plan_mode.last_plan = Some(plan.to_string());
            st.plan_mode.plan_title = Some(plan_title);
            st.plan_mode.plan_status = Some("reviewing".into());
            st.plan_mode.updated_at = now_ms;
        });
        self.emit_session_collab_updated(session_id);
        let request_id = Uuid::new_v4().to_string();
        let signal = Arc::new(AtomicU8::new(0));
        let reply = Arc::new(parking_lot::Mutex::new(None));
        self.pending_approvals.write().insert(
            request_id.clone(),
            PendingToolApproval {
                session_id: session_id.into(),
                decision: signal.clone(),
                selected_subagent_model: Arc::new(parking_lot::Mutex::new(None)),
                interaction_reply: reply.clone(),
                request: json!({
                    "requestId": request_id,
                    "sessionId": session_id,
                    "toolCallId": call_id,
                    "toolName": "ExitPlanMode",
                    "kind": "plan_review",
                    "plan": plan,
                    "input": { "plan": plan },
                }),
            },
        );
        self.permission_event(session_id, "tool_permission_requested");
        let start = Instant::now();
        let outcome = loop {
            if cancelled.load(Ordering::SeqCst) {
                break 2;
            }
            let d = signal.load(Ordering::SeqCst);
            if d == 1 || d == 2 {
                break d;
            }
            if start.elapsed() > Duration::from_secs(30 * 60) {
                break 2;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        self.pending_approvals.write().remove(&request_id);
        self.permission_event(session_id, "tool_permission_changed");
        if cancelled.load(Ordering::SeqCst) {
            return ("Task stopped during plan review.".into(), true);
        }
        let payload = reply.lock().clone().unwrap_or_else(|| json!({}));
        if outcome == 1 {
            self.collaboration.mutate(session_id, |st| {
                st.plan_mode.active = false;
                st.plan_mode.plan_status = Some("approved".into());
                st.plan_mode.updated_at = chrono::Utc::now().timestamp_millis();
            });
            self.emit_session_collab_updated(session_id);
            (
                json!({"approved": true, "note": "Plan approved by user; Plan Mode is now off. Proceed with implementation."}).to_string(),
                false,
            )
        } else if let Some(fb) = payload["feedback"].as_str().filter(|s| !s.is_empty()) {
            self.collaboration.mutate(session_id, |st| {
                st.plan_mode.plan_status = Some("changes_requested".into());
                st.plan_mode.updated_at = chrono::Utc::now().timestamp_millis();
            });
            self.emit_session_collab_updated(session_id);
            (
                format!("Keep planning — user requested revisions:\n{fb}\n\nRevise the plan and submit via ExitPlanMode."),
                true,
            )
        } else {
            (
                json!({"approved": false, "note": "Plan review dismissed; remain in Plan Mode and await user instructions."}).to_string(),
                false,
            )
        }
    }

    fn run_ask_user_question(
        &self,
        session_id: &str,
        call_id: &str,
        input: &Value,
        cancelled: &AtomicBool,
    ) -> (String, bool) {
        let Some(questions) = input["questions"].as_array().filter(|q| !q.is_empty() && q.len() <= 4) else {
            return ("AskUserQuestion requires 1 to 4 items in `questions`.".into(), true);
        };
        let normalized_questions: Vec<Value> = questions
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let mut item = q.clone();
                if item.get("id").and_then(Value::as_str).is_none_or(|s| s.trim().is_empty()) {
                    item["id"] = json!(format!("q{}", i + 1));
                }
                item
            })
            .collect();
        let timed_secs = input
            .get("timeout_seconds")
            .and_then(Value::as_u64)
            .map(|s| s.clamp(1, 300));

        let request_id = Uuid::new_v4().to_string();
        let signal = Arc::new(AtomicU8::new(0));
        let reply = Arc::new(parking_lot::Mutex::new(None));
        self.pending_approvals.write().insert(
            request_id.clone(),
            PendingToolApproval {
                session_id: session_id.into(),
                decision: signal.clone(),
                selected_subagent_model: Arc::new(parking_lot::Mutex::new(None)),
                interaction_reply: reply.clone(),
                request: json!({
                    "requestId": request_id,
                    "sessionId": session_id,
                    "toolCallId": call_id,
                    "toolName": "AskUserQuestion",
                    "kind": "ask_user",
                    "questions": normalized_questions,
                    "timeoutSeconds": timed_secs,
                    "input": input,
                }),
            },
        );
        self.permission_event(session_id, "tool_permission_requested");
        let wait_limit = timed_secs
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(30 * 60));
        let start = Instant::now();
        let mut timed_out = false;
        let outcome = loop {
            if cancelled.load(Ordering::SeqCst) {
                break 2;
            }
            let d = signal.load(Ordering::SeqCst);
            if d == 1 || d == 2 {
                break d;
            }
            if start.elapsed() >= wait_limit {
                timed_out = timed_secs.is_some();
                break 0;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        self.pending_approvals.write().remove(&request_id);
        self.permission_event(session_id, "tool_permission_changed");
        if cancelled.load(Ordering::SeqCst) {
            return ("Task stopped while waiting for user answer.".into(), true);
        }
        if timed_out && outcome == 0 {
            self.collaboration
                .record_continued_question(session_id, call_id, normalized_questions);
            self.emit_session_collab_updated(session_id);
            return (
                json!({
                    "pending": true,
                    "callId": call_id,
                    "note": "Foreground wait elapsed without an answer. The question card remains open for a late reply; continue independent non-blocked work, and do not treat pending as approval or confirmation."
                })
                .to_string(),
                false,
            );
        }
        if outcome == 1 {
            let ans = reply.lock().clone().unwrap_or_else(|| json!({"answers": []}));
            (ans.to_string(), false)
        } else {
            ("User dismissed the question without answering.".into(), true)
        }
    }

    pub(crate) fn execute_tool_with_approval(&self, session_id: &str, call_id: &str, name: &str,
        input: &Value, cwd: &str, fallback_mode: &str, cancelled: &AtomicBool) -> (String,bool) {
        self.execute_tool_with_approval_and_mcp_identity(session_id,call_id,name,input,cwd,fallback_mode,cancelled,None)
    }
    pub(crate) fn execute_tool_with_approval_and_mcp_identity(&self, session_id:&str,call_id:&str,name:&str,input:&Value,cwd:&str,fallback_mode:&str,cancelled:&AtomicBool,expected:Option<&str>)->(String,bool){
        if cwd.trim().is_empty() && crate::chat_workflows::needs_workspace(name) {
            return ("No folder is selected for this conversation. Ask the user to choose a folder before reading files, making edits, or running commands. No action was performed.".into(),true);
        }
        if let Err(error)=agent_tools::validate_input(name,input) {return (error,true);}
        if cancelled.load(Ordering::SeqCst) {return ("Task stopped. No action performed.".into(),true);}

        // Monotonic Plan Mode guard: block mutations & shell execution while planning.
        if self.collaboration.is_plan_active(session_id)
            && matches!(
                name,
                agent_tools::TOOL_BASH
                    | agent_tools::TOOL_WRITE
                    | agent_tools::TOOL_EDIT
                    | crate::code_tools::TOOL_APPLY_PATCH
                    | crate::code_tools::TOOL_CREATE_WORKTREE
                    | "TerminalOpen"
                    | "TerminalSend"
            )
        {
            return (
                format!(
                    "Blocked by Plan Mode: `{name}` is disabled while planning. Use read-only inspection tools (`Read`, `Grep`, `Glob`, `ListDirectory`, `Lsp`) and submit your finished plan via `ExitPlanMode`."
                ),
                true,
            );
        }

        if name == crate::cordis_host::TOOL_CORDIS_INSPECT_LIST {
            let platform = input.get("platform").and_then(Value::as_str);
            return (self.cordis.cordis_inspect_list(platform).to_string(), false);
        }
        if name == crate::cordis_host::TOOL_CORDIS_INSPECT_QUERY {
            let q = input.get("query").and_then(Value::as_str).unwrap_or("");
            return (self.cordis.cordis_inspect_query(q).to_string(), false);
        }
        if name == crate::cordis_host::TOOL_PROPOSE_ACTION {
            return self.execute_propose_action_tool(session_id, input);
        }
        if name == crate::cordis_host::TOOL_WEB_SEARCH {
            return match self
                .cordis
                .runtime
                .root_context()
                .try_get(crate::cordis_host::capabilities::WEB_SERVICE)
            {
                Some(web) => {
                    let queries: Vec<String> = input
                        .get("queries")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(str::to_owned))
                                .collect()
                        })
                        .or_else(|| {
                            input
                                .get("query")
                                .and_then(Value::as_str)
                                .map(|s| vec![s.to_owned()])
                        })
                        .unwrap_or_default();
                    match web.search_multi(&queries) {
                        Ok(out) => (
                            crate::spill::maybe_spill_tool_result(
                                &self.data_dir,
                                session_id,
                                name,
                                out,
                            ),
                            false,
                        ),
                        Err(err) => (err, true),
                    }
                }
                None => ("WebSearch plugin is currently disabled.".into(), true),
            };
        }

        // Dedicated interactive collaboration flows:
        if name == crate::collaboration::TOOL_EXIT_PLAN_MODE {
            return self.run_plan_review(session_id, call_id, input, cancelled);
        }
        if name == crate::collaboration::TOOL_ASK_USER {
            return self.run_ask_user_question(session_id, call_id, input, cancelled);
        }
        if name == crate::collaboration::TOOL_TODO_WRITE {
            return match self.collaboration.write_todos(session_id, input) {
                Ok(v) => {
                    self.emit_session_collab_updated(session_id);
                    (v.to_string(), false)
                }
                Err(e) => (e, true),
            };
        }
        if matches!(
            name,
            crate::collaboration::TOOL_GET_GOAL
                | crate::collaboration::TOOL_CREATE_GOAL
                | crate::collaboration::TOOL_UPDATE_GOAL
        ) {
            let action = match name {
                crate::collaboration::TOOL_GET_GOAL => "get",
                crate::collaboration::TOOL_CREATE_GOAL => "create",
                _ => input["action"].as_str().unwrap_or(""),
            };
            return match self.collaboration.manage_goal(session_id, action, input) {
                Ok(v) => {
                    self.emit_session_collab_updated(session_id);
                    (v.to_string(), false)
                }
                Err(e) => (e, true),
            };
        }
        if matches!(
            name,
            crate::collaboration::TOOL_TEAM_CREATE
                | crate::collaboration::TOOL_TEAM_LIST
                | crate::collaboration::TOOL_TEAM_UPDATE
        ) {
            let res = match name {
                crate::collaboration::TOOL_TEAM_LIST => {
                    let st = self.collaboration.get(session_id);
                    Ok(json!({ "tasks": crate::collaboration::enrich_team_tasks(&st.team.tasks) }))
                }
                crate::collaboration::TOOL_TEAM_CREATE => {
                    self.collaboration.team_create_task(session_id, input)
                }
                _ => self.collaboration.team_update_task(session_id, input),
            };
            return match res {
                Ok(v) => {
                    self.emit_session_collab_updated(session_id);
                    (v.to_string(), false)
                }
                Err(e) => (e, true),
            };
        }

        let mcp_workspace=self.mcp_workspace(cwd);
        let execution_cwd=if matches!(name,agent_tools::TOOL_MCP_CALL|agent_tools::TOOL_MCP_DISCOVER) {mcp_workspace.to_str().unwrap_or(cwd)} else {cwd};
        let mcp_identity=if name==agent_tools::TOOL_MCP_CALL {
            match self.mcp_host.server_identity(input["server"].as_str().unwrap_or_default()) {
                Ok(identity)=>{if expected.is_some_and(|v|v!=identity){return ("MCP server configuration changed after the tool was advertised. Discover it again before requesting execution.".into(),true);}Some(identity)},Err(error)=>return(format!("MCP server is unavailable: {error}"),true),
            }
        }else{None};
        let outside = file_access_outside_workspace_with_spill(
            name,
            input,
            cwd,
            Some((&self.data_dir, session_id)),
        );
        let needs=|mode:&str| agent_tools::requires_approval_with_host(name,input,mode,&self.mcp_host) || (outside&&!matches!(mode,"auto"|"bypassPermissions"));
        let mode=self.current_permission_mode(session_id,fallback_mode);
        let execute = |execution_mode: &str| -> (String, bool) {
            let raw_out = if name == "Artifact" {
                self.create_local_artifact(session_id, input)
            } else if name == crate::cordis_host::TOOL_PLUGIN_MANAGER {
                self.execute_plugin_manager_tool(input)
            } else if name == crate::workflows::TOOL_WORKING_DIRECTORY {
                self.execute_working_directory_tool(session_id, cwd, input)
            } else if name == crate::workflows::TOOL_SESSION_SEARCH {
                self.execute_session_search_tool(session_id, cwd, input)
            } else if name == crate::collaboration::TOOL_PRESENT {
                self.execute_present_tool(session_id, cwd, input)
            } else if name == agent_tools::TOOL_BASH
                && input.get("run_in_background").and_then(Value::as_bool) != Some(true)
                && !session_id.trim().is_empty()
                && self.cordis.is_row_active("tool-jobs-terminal")
                && self.cordis.bash_runtime_config().promote_timeout_to_job
            {
                let cmd_str = input["command"].as_str().unwrap_or("");
                if cmd_str.trim().is_empty() {
                    ("No command provided.".to_string(), true)
                } else if agent_tools::is_destructive_for_test(cmd_str)
                    && !matches!(execution_mode, "auto" | "bypassPermissions")
                {
                    (
                        format!(
                            "Blocked by QuixiDesk permission mode '{execution_mode}': this command looks destructive. Switch the session pill from Manual to Auto if that is really intended."
                        ),
                        true,
                    )
                } else {
                    let bash_cfg = self.cordis.bash_runtime_config();
                    let timeout_secs = input
                        .get("timeout_seconds")
                        .and_then(Value::as_u64)
                        .unwrap_or(bash_cfg.default_timeout_secs)
                        .clamp(1, 3600);
                    let policy = quixi_platform::sandbox::SandboxPolicy {
                        mode: quixi_platform::sandbox::SandboxMode::from_permission_mode(execution_mode),
                        workspace_root: PathBuf::from(cwd),
                        additional_writable_roots: vec![],
                        session_temp_dir: None,
                    };
                    match self.jobs.run_bash_with_timeout_promotion_and_progress(
                        session_id,
                        input["description"].as_str().unwrap_or(cmd_str),
                        Path::new(cwd),
                        cmd_str,
                        &self.data_dir,
                        Duration::from_secs(timeout_secs),
                        cancelled,
                        (!cwd.trim().is_empty()).then_some(&policy),
                        |line, elapsed_ms| {
                            self.emit_turn_activity(
                                session_id,
                                json!({
                                    "phase": "bash_output",
                                    "callId": call_id,
                                    "toolName": "Bash",
                                    "line": line,
                                    "elapsedMs": elapsed_ms,
                                }),
                            );
                        },
                    ) {
                        Ok((out, is_err, promoted_job)) => {
                            if promoted_job.is_some() {
                                self.emit_session_collab_updated(session_id);
                            } else if !is_err {
                                self.observe_forge_from_bash(session_id, cwd, cmd_str, &out);
                            }
                            (out, is_err)
                        }
                        Err(e) => (format!("Failed to run command: {e}"), true),
                    }
                }
            } else if name == agent_tools::TOOL_BASH
                && input.get("run_in_background").and_then(Value::as_bool) == Some(true)
            {
                let cmd_str = input["command"].as_str().unwrap_or("");
                if agent_tools::is_destructive_for_test(cmd_str)
                    && !matches!(execution_mode, "auto" | "bypassPermissions")
                {
                    (
                        format!("Blocked destructive background command in `{execution_mode}` mode."),
                        true,
                    )
                } else {
                    let policy = quixi_platform::sandbox::SandboxPolicy {
                        mode: quixi_platform::sandbox::SandboxMode::from_permission_mode(execution_mode),
                        workspace_root: PathBuf::from(cwd),
                        additional_writable_roots: vec![],
                        session_temp_dir: None,
                    };
                    match self.jobs.start_bash(
                        session_id,
                        input["description"].as_str().unwrap_or(cmd_str),
                        Path::new(cwd),
                        cmd_str,
                        &self.data_dir,
                        (!cwd.trim().is_empty()).then_some(&policy),
                    ) {
                        Ok(view) => {
                            self.emit_session_collab_updated(session_id);
                            (serde_json::to_string(&view).unwrap_or_default(), false)
                        }
                        Err(e) => (e, true),
                    }
                }
            } else if matches!(name, "JobList" | "JobOutput" | "JobKill") {
                match name {
                    "JobList" => (
                        serde_json::to_string(&self.jobs.list(session_id)).unwrap_or_default(),
                        false,
                    ),
                    "JobOutput" => {
                        let jid = input["job_id"].as_str().unwrap_or("");
                        let wait = input
                            .get("wait_seconds")
                            .and_then(Value::as_u64)
                            .map(|s| Duration::from_secs(s.min(30)));
                        match self.jobs.read_consuming(session_id, jid, wait, cancelled) {
                            Ok(v) => (v.to_string(), false),
                            Err(e) => (e, true),
                        }
                    }
                    _ => {
                        let jid = input["job_id"].as_str().unwrap_or("");
                        let reason = input.get("reason").and_then(Value::as_str);
                        match self.jobs.kill(session_id, jid, reason) {
                            Ok(v) => {
                                self.emit_session_collab_updated(session_id);
                                (v.to_string(), false)
                            }
                            Err(e) => (e, true),
                        }
                    }
                }
            } else if matches!(
                name,
                "TerminalOpen"
                    | "TerminalSend"
                    | "TerminalRead"
                    | "TerminalSignal"
                    | "TerminalClose"
                    | "TerminalList"
            ) {
                self.execute_terminal_tool(session_id, name, input, cwd, cancelled)
            } else if name == crate::code_tools::TOOL_CREATE_WORKTREE {
                let wt_name = input["name"].as_str().unwrap_or("");
                let enter = input.get("enter").and_then(Value::as_bool).unwrap_or(true);
                match quixi_workspace::changes::WorktreeManager::create_worktree(Path::new(cwd), wt_name) {
                    Ok(info) => {
                        let wt_path = info["path"].as_str().unwrap_or("").to_owned();
                        let wt_branch = info["branch"].as_str().unwrap_or("").to_owned();
                        let resolved_name = info["name"].as_str().unwrap_or(wt_name).to_owned();
                        if !wt_path.is_empty() {
                            self.collaboration.set_worktree(
                                session_id,
                                Some(crate::collaboration::WorktreeRecord {
                                    name: resolved_name,
                                    path: wt_path.clone(),
                                    branch: wt_branch,
                                    origin_cwd: cwd.to_owned(),
                                    created_at: chrono::Utc::now().timestamp_millis(),
                                }),
                            );
                            self.emit_session_collab_updated(session_id);
                        }
                        if enter && !wt_path.is_empty() {
                            if let Some(s) = self.agent_sessions.write().get_mut(session_id) {
                                s.cwd = wt_path.clone();
                                self.persist_agent_session(s);
                            } else if let Some(s) = self.code_sessions.write().get_mut(session_id) {
                                s.cwd = wt_path;
                                self.persist_code_session(s);
                            }
                        }
                        (info.to_string(), false)
                    }
                    Err(e) => (format!("CreateWorktree failed: {e}"), true),
                }
            } else {
                // Freshness check + pre-edit change capture for file mutations:
                if name == agent_tools::TOOL_EDIT {
                    let target = agent_tools::resolve_path(
                        input["file_path"].as_str().unwrap_or(""),
                        execution_cwd,
                    );
                    if let Err(stale) = self.observations.verify_not_stale(session_id, &target) {
                        return (stale, true);
                    }
                    self.capture_turn_baseline(session_id, &target);
                } else if name == agent_tools::TOOL_WRITE {
                    let target = agent_tools::resolve_path(
                        input["file_path"].as_str().unwrap_or(""),
                        execution_cwd,
                    );
                    self.capture_turn_baseline(session_id, &target);
                } else if name == crate::code_tools::TOOL_APPLY_PATCH {
                    if let Ok(ops) = crate::code_tools::parse_patch_operations(
                        input["patch"].as_str().unwrap_or(""),
                        Path::new(execution_cwd),
                    ) {
                        for op in &ops {
                            let p = match op {
                                crate::code_tools::PatchFileOp::Add { path, .. }
                                | crate::code_tools::PatchFileOp::Update { path, .. }
                                | crate::code_tools::PatchFileOp::Delete { path } => path,
                            };
                            self.capture_turn_baseline(session_id, p);
                        }
                    }
                }
                let res = agent_tools::execute_cancellable_with_mcp_identity(
                    name,
                    input,
                    execution_cwd,
                    execution_mode,
                    &self.mcp_host,
                    cancelled,
                    mcp_identity.as_deref(),
                );
                if !res.1 && matches!(name, agent_tools::TOOL_READ | agent_tools::TOOL_WRITE | agent_tools::TOOL_EDIT) {
                    let target = agent_tools::resolve_path(
                        input["file_path"].as_str().unwrap_or(""),
                        execution_cwd,
                    );
                    self.observations.record(session_id, &target);
                }
                res
            };
            (
                crate::spill::maybe_spill_tool_result(&self.data_dir, session_id, name, raw_out.0),
                raw_out.1,
            )
        };
        if !needs(&mode) {
            return execute(&mode);
        }
        // Smart Auto (`smartAuto`): same-model per-tool safety review (`dsh-experimental-auto-review`).
        // Destructive commands, outside-workspace paths, and external MCP calls always ask the human.
        let mut ai_review_note: Option<String> = None;
        if mode == "smartAuto"
            && self.cordis.is_row_active("auto-review")
            && !outside
            && name != agent_tools::TOOL_MCP_CALL
            && !(name == agent_tools::TOOL_BASH
                && agent_tools::is_destructive_for_test(input["command"].as_str().unwrap_or("")))
        {
            match self.evaluate_smart_auto_safety(session_id, name, input, cwd) {
                Ok(None) => return execute("smartAutoApproved"),
                Ok(Some(risk)) => ai_review_note = Some(risk),
                Err(fallback) => ai_review_note = Some(fallback),
            }
        }
        if cancelled.load(Ordering::SeqCst) {return ("Task stopped. No action performed.".into(),true);}
        let request_id=Uuid::new_v4().to_string();
        let signal=Arc::new(AtomicU8::new(0));
        self.pending_approvals.write().insert(request_id.clone(),PendingToolApproval {
            session_id:session_id.into(), decision:signal.clone(),
            selected_subagent_model: Arc::new(parking_lot::Mutex::new(None)),
            interaction_reply: Arc::new(parking_lot::Mutex::new(None)),
            request:json!({"requestId":request_id,"sessionId":session_id,"toolCallId":call_id,"toolName":name,"input":input,"cwd":cwd,"outsideWorkspace":outside,"mcpServerIdentity":mcp_identity,"aiReviewNote":ai_review_note,"subagent":self.subagent_for_call(session_id,call_id)}),
        });
        self.permission_event(session_id,"tool_permission_requested");
        let start=Instant::now();
        let outcome=loop {
            if cancelled.load(Ordering::SeqCst) {break 2;}
            let decision=signal.load(Ordering::SeqCst);
            if decision==1 || decision==2 {break decision;}
            if decision==0 && !needs(&self.current_permission_mode(session_id,fallback_mode)) {break 1;}
            if start.elapsed()>Duration::from_secs(30*60) {break 2;}
            std::thread::sleep(Duration::from_millis(50));
        };
        self.pending_approvals.write().remove(&request_id);
        self.permission_event(session_id,"tool_permission_changed");
        if outcome!=1 || cancelled.load(Ordering::SeqCst) {
            return ("The action was not approved or the task was stopped. No action was performed. Do not retry it without a new user request.".into(),true);
        }
        execute("auto")
    }

    /// Runs a prefix-stable same-model safety review for `smartAuto` mode.
    /// Returns `Ok(None)` when explicitly classified `ALLOW:`, or `Ok(Some(reason))` / `Err(_)` (fail-closed) to ask the user.
    fn evaluate_smart_auto_safety(
        &self,
        session_id: &str,
        tool_name: &str,
        input: &Value,
        cwd: &str,
    ) -> Result<Option<String>, String> {
        let model = self
            .agent_sessions
            .read()
            .get(session_id)
            .map(|s| s.model.clone())
            .or_else(|| self.code_sessions.read().get(session_id).map(|s| s.model.clone()))
            .filter(|m| !m.trim().is_empty());
        let args_preview: String = serde_json::to_string(input)
            .unwrap_or_default()
            .chars()
            .take(2_000)
            .collect();
        let review_messages = vec![
            json!({
                "role": "system",
                "content": "You are a strict fail-closed tool safety reviewer for a local coding workspace. Reply on a single line starting with `ALLOW:` ONLY for routine, non-destructive workspace builds, tests, lints, git read/branch checks, or safe local page reads. Reply `ASK: <short risk explanation>` for anything that deletes data, pushes/publishes externally, installs untrusted packages, modifies system settings, leaks secrets, or has uncertain blast radius."
            }),
            json!({
                "role": "user",
                "content": format!("Workspace: {cwd}\nTool: {tool_name}\nArguments: {args_preview}")
            }),
        ];
        let turn = self
            .provider_manager
            .complete_turn_with_tools(&review_messages, &json!([]), model.as_deref(), |_| {})
            .ok_or_else(|| "Smart Auto safety reviewer unavailable; requesting human approval.".to_string())?;
        if turn.stream_error.is_some() || !turn.tool_calls.is_empty() {
            return Err("Smart Auto safety check inconclusive; requesting human approval.".into());
        }
        let first_line = turn.text.lines().next().unwrap_or("").trim();
        if first_line.starts_with("ALLOW:") || first_line == "ALLOW" {
            Ok(None)
        } else if let Some(reason) = first_line.strip_prefix("ASK:") {
            Ok(Some(reason.trim().to_string()))
        } else {
            Ok(Some(if first_line.is_empty() {
                "Safety review required manual confirmation.".into()
            } else {
                first_line.chars().take(200).collect()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Arc<QuixiEngine>,String,PathBuf) {
        let root=std::env::temp_dir().join(format!("quixi-approval-{}",Uuid::new_v4()));
        let engine=QuixiEngine::new_with_data_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),root.join("data"));
        fs::create_dir_all(root.join("work")).unwrap();
        let sid=engine.start_agent_session(&json!({"cwd":root.join("work"),"userSelectedFolders":[root.join("work")],"permissionMode":"default"}));
        (engine,sid,root)
    }
    fn pending(engine:&QuixiEngine,sid:&str)->Value {
        let until=Instant::now()+Duration::from_secs(3);
        loop {let p=engine.pending_tool_approvals(sid);if !p.as_array().unwrap().is_empty(){return p[0].clone();}assert!(Instant::now()<until,"approval did not appear");std::thread::sleep(Duration::from_millis(10));}
    }
    #[test]
    fn artifact_obeys_manual_denial_and_auto_permission() {
        let (engine, sid, root) = fixture();
        let e = engine.clone(); let id = sid.clone();
        let worker = std::thread::spawn(move || e.execute_tool_with_approval(&id, "preview", "Artifact",
            &json!({"title":"Preview", "html":"<h1>Local</h1>"}), "", "default", &AtomicBool::new(false)));
        let request = pending(&engine, &sid);
        assert!(engine.artifacts.read().is_empty());
        engine.resolve_tool_approval(&sid, request["requestId"].as_str().unwrap(), "deny");
        assert!(worker.join().unwrap().1);
        assert!(engine.artifacts.read().is_empty());
        // No registered session: the explicit fallback mode is used.
        assert!(!engine.execute_tool_with_approval("auto-preview", "preview2", "Artifact",
            &json!({"title":"Preview", "html":"<h1>Local</h1>"}), "", "auto", &AtomicBool::new(false)).1);
        assert_eq!(engine.artifacts.read().len(), 1);
        drop(engine); fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manual_edit_waits_for_exact_approval_and_resumes_once() {
        let (engine,sid,root)=fixture();let cancelled=engine.begin_turn(&sid);let e=engine.clone();let id=sid.clone();let cwd=root.join("work");let path=cwd.join("README.md");
        let worker=std::thread::spawn(move|| e.execute_tool_with_approval(&id,"call-one","Write",&json!({"file_path":"README.md","content":"approved"}),cwd.to_str().unwrap(),"default",&cancelled));
        let p=pending(&engine,&sid);assert!(!path.exists());assert_eq!(p["input"]["content"],"approved");
        let request=p["requestId"].as_str().unwrap();assert!(engine.resolve_tool_approval("wrong-session",request,"allow_once")["error"].is_string());assert!(!path.exists());
        assert_eq!(engine.resolve_tool_approval(&sid,request,"allow_once")["ok"],true);
        assert!(!worker.join().unwrap().1);assert_eq!(fs::read_to_string(&path).unwrap(),"approved");
        assert_eq!(engine.agent_sessions.read()[&sid].permission_mode,"default");
        assert!(engine.resolve_tool_approval(&sid,request,"allow_once")["error"].is_string());
        drop(engine);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn allow_file_edits_resumes_task_but_keeps_shell_approval() {
        let (engine,sid,root)=fixture();let cancelled=engine.begin_turn(&sid);let e=engine.clone();let id=sid.clone();let cwd=root.join("work");let next_cwd=cwd.clone();
        let worker=std::thread::spawn(move||e.execute_tool_with_approval(&id,"edit-one","Write",&json!({"file_path":"README.md","content":"one"}),cwd.to_str().unwrap(),"default",&cancelled));
        let p=pending(&engine,&sid);assert_eq!(engine.resolve_tool_approval(&sid,p["requestId"].as_str().unwrap(),"allow_file_edits")["ok"],true);assert!(!worker.join().unwrap().1);
        assert_eq!(engine.current_permission_mode(&sid,"default"),"acceptEdits");
        assert!(!engine.execute_tool_with_approval(&sid,"edit-two","Write",&json!({"file_path":"example.txt","content":"two"}),next_cwd.to_str().unwrap(),"default",&AtomicBool::new(false)).1);
        assert!(agent_tools::requires_approval("Bash",&json!({"command":"echo test"}),"acceptEdits"));
        assert!(engine.pending_tool_approvals(&sid).as_array().unwrap().is_empty());drop(engine);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn denial_and_stop_never_perform_the_write() {
        let (engine,sid,root)=fixture();
        for deny in [true,false] {
            let cancelled=engine.begin_turn(&sid);let e=engine.clone();let id=sid.clone();let cwd=root.join("work");
            let worker=std::thread::spawn(move||e.execute_tool_with_approval(&id,"blocked","Write",&json!({"file_path":"never.txt","content":"no"}),cwd.to_str().unwrap(),"default",&cancelled));
            let p=pending(&engine,&sid);
            if deny {assert_eq!(engine.resolve_tool_approval(&sid,p["requestId"].as_str().unwrap(),"deny")["ok"],true);} else {engine.cancel_turn(&sid);}
            assert!(worker.join().unwrap().1);assert!(!root.join("work/never.txt").exists());assert!(engine.pending_tool_approvals(&sid).as_array().unwrap().is_empty());
        }
        drop(engine);fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)] mod scope_tests {use super::*;
 #[test] fn parent_paths_and_symlinks_require_approval(){
 let root=std::env::temp_dir().join(format!("quixi-scope-{}",Uuid::new_v4()));fs::create_dir_all(root.join("work")).unwrap();fs::write(root.join("outside"),"private").unwrap();let cwd=root.join("work");let cwd=cwd.to_str().unwrap();
 assert!(file_access_outside_workspace("Read",&json!({"file_path":"../outside"}),cwd));assert!(!file_access_outside_workspace("Write",&json!({"file_path":"new/nested.txt"}),cwd));
 #[cfg(unix)]{std::os::unix::fs::symlink(root.join("outside"),root.join("work/link")).unwrap();assert!(file_access_outside_workspace("Read",&json!({"file_path":"link"}),cwd));}
 fs::remove_dir_all(root).unwrap();}
}
