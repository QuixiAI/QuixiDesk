//! Collaboration, planning, goals, user questions, deliverables, team tasks, and loop-hygiene guards.
//!
//! Inspired by DeepSeek Harness (`dsh-tool-todo`, `dsh-plan-mode`, `dsh-goal`, `dsh-user-questions`,
//! `dsh-deliverables`, `dsh-agent-team`, `dsh-token-meter`, and `dsh-repeat-tool-reminder`).

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub const TOOL_TODO_WRITE: &str = "TodoWrite";
pub const TOOL_EXIT_PLAN_MODE: &str = "ExitPlanMode";
pub const TOOL_ASK_USER: &str = "AskUserQuestion";
pub const TOOL_PRESENT: &str = "Present";
pub const TOOL_CREATE_GOAL: &str = "CreateGoal";
pub const TOOL_GET_GOAL: &str = "GetGoal";
pub const TOOL_UPDATE_GOAL: &str = "UpdateGoal";
pub const TOOL_TEAM_CREATE: &str = "TeamTaskCreate";
pub const TOOL_TEAM_LIST: &str = "TeamTaskList";
pub const TOOL_TEAM_UPDATE: &str = "TeamTaskUpdate";

pub const PLAN_MODE_GUIDANCE: &str = r#"You are in Plan Mode. Stay in Plan Mode until the user approves your plan through `ExitPlanMode` or disables Plan Mode.
- Explore first using non-mutating inspection tools (`Read`, `ListDirectory`, `Grep`, `Glob`, `Lsp`, `McpDiscover`).
- Do NOT modify files or execute commands (`Write`, `Edit`, `ApplyPatch`, `Bash`, `CreateWorktree` are blocked while Plan Mode is active; their schemas remain listed only for request-cache stability).
- Use `AskUserQuestion` only when a product or architectural choice genuinely belongs to the user and cannot be answered by inspecting the workspace.
- Make the plan decision-complete: goal & success criteria, affected files/subsystems, data-flow/interface changes, edge cases, and verification steps.
- When ready, call `ExitPlanMode` with the complete Markdown plan starting with a `#` heading as the final tool call in your response."#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    pub status: String, // "pending" | "in_progress" | "completed"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanModeState {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_status: Option<String>, // "reviewing" | "approved" | "changes_requested"
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalSnapshot {
    pub id: String,
    pub revision: u64,
    pub objective: String,
    pub phase: String, // "active" | "paused" | "blocked" | "complete"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_message: Option<String>,
    pub rounds_started: u32,
    pub max_goal_rounds: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuedQuestion {
    pub call_id: String,
    pub state: String, // "continued" | "settled"
    pub questions: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub answers: Vec<Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentedFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDeliverableRecord {
    pub turn_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presented_files: Vec<PresentedFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_changes: Option<quixi_workspace::changes::TurnChangesSummary>,
    pub recorded_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamTask {
    pub id: String,
    pub revision: u64,
    pub subject: String,
    pub description: String,
    pub status: String, // "pending" | "in_progress" | "completed" | "deleted"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub write_scopes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamBoardState {
    pub enabled: bool,
    #[serde(default)]
    pub next_seq: u64,
    #[serde(default)]
    pub tasks: Vec<TeamTask>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMeterState {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub pressure_tokens: u64,
    pub projected_tokens: u64,
    pub context_window: u64,
    pub compactions: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTelemetryRecord {
    pub turn_index: usize,
    pub step_index: usize,
    pub model: String,
    pub ttft_ms: u64,
    pub model_ms: u64,
    pub tool_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub context_tokens: u64,
    #[serde(default)]
    pub tool_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ptc_subcalls: Vec<Value>,
    pub recorded_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionCardRecord {
    pub id: String,
    pub action: String, // "create_personalized_page" | "enable_official_plugin"
    pub title: String,
    pub brief: String,
    pub details: String,
    pub status: String, // "pending" | "accepted" | "dismissed" | "replaced" | "undone"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_bundle_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgePrRecord {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String, // "OPEN" | "MERGED" | "CLOSED"
    pub is_draft: bool,
    pub mergeable: String,
    pub head_ref: String,
    pub base_ref: String,
    pub review_decision: String,
    pub checks_passed: u32,
    pub checks_failed: u32,
    pub checks_pending: u32,
    pub binding_source: String, // "created" | "referenced" | "discovered"
    #[serde(default)]
    pub dismissed: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRecord {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub origin_cwd: String,
    pub created_at: i64,
}

fn default_agent_preset() -> String {
    "standard".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCollabState {
    #[serde(default = "default_agent_preset")]
    pub agent_preset: String,
    #[serde(default)]
    pub todos: Vec<TodoItem>,
    #[serde(default)]
    pub plan_mode: PlanModeState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<GoalSnapshot>,
    #[serde(default)]
    pub questions: Vec<ContinuedQuestion>,
    #[serde(default)]
    pub deliverables: Vec<TurnDeliverableRecord>,
    #[serde(default)]
    pub step_telemetry: Vec<StepTelemetryRecord>,
    #[serde(default)]
    pub action_cards: Vec<ActionCardRecord>,
    #[serde(default)]
    pub forge_prs: Vec<ForgePrRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeRecord>,
    #[serde(default)]
    pub team: TeamBoardState,
    #[serde(default)]
    pub context_meter: ContextMeterState,
    #[serde(default)]
    pub title_pinned: bool,
    #[serde(default)]
    pub title_generated: bool,
}

pub struct SessionCollaborationStore {
    data_dir: PathBuf,
    states: RwLock<HashMap<String, SessionCollabState>>,
    /// Process-local goal continuation arming; cold restarts never auto-continue without user resume.
    armed_goals: RwLock<HashSet<String>>,
}

impl SessionCollaborationStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(data_dir.join("session_state"));
        Self {
            data_dir,
            states: RwLock::new(HashMap::new()),
            armed_goals: RwLock::new(HashSet::new()),
        }
    }

    fn path_for(&self, session_id: &str) -> PathBuf {
        let hash = format!("{:x}", Sha256::digest(session_id.as_bytes()));
        self.data_dir
            .join("session_state")
            .join(format!("{hash}.json"))
    }

    pub fn get(&self, session_id: &str) -> SessionCollabState {
        if let Some(existing) = self.states.read().get(session_id) {
            return existing.clone();
        }
        let loaded = fs::read(self.path_for(session_id))
            .ok()
            .and_then(|b| serde_json::from_slice::<SessionCollabState>(&b).ok())
            .unwrap_or_default();
        self.states
            .write()
            .insert(session_id.to_string(), loaded.clone());
        loaded
    }

    pub fn mutate<R>(
        &self,
        session_id: &str,
        f: impl FnOnce(&mut SessionCollabState) -> R,
    ) -> R {
        let mut cur = self.get(session_id);
        let out = f(&mut cur);
        if let Ok(bytes) = serde_json::to_vec(&cur) {
            let _ = crate::storage::atomic_write(&self.path_for(session_id), &bytes);
        }
        self.states.write().insert(session_id.to_string(), cur);
        out
    }

    pub fn delete_session(&self, session_id: &str) {
        self.states.write().remove(session_id);
        self.armed_goals.write().remove(session_id);
        let _ = fs::remove_file(self.path_for(session_id));
    }

    pub fn is_plan_active(&self, session_id: &str) -> bool {
        self.get(session_id).plan_mode.active
    }

    pub fn is_title_pinned(&self, session_id: &str) -> bool {
        self.get(session_id).title_pinned
    }

    pub fn set_title_pinned(&self, session_id: &str, pinned: bool) {
        self.mutate(session_id, |st| {
            st.title_pinned = pinned;
        });
    }

    pub fn is_title_generated(&self, session_id: &str) -> bool {
        self.get(session_id).title_generated
    }

    pub fn set_title_generated(&self, session_id: &str, generated: bool) {
        self.mutate(session_id, |st| {
            st.title_generated = generated;
        });
    }

    pub fn set_plan_mode(&self, session_id: &str, active: bool) -> SessionCollabState {
        self.mutate(session_id, |st| {
            st.plan_mode.active = active;
            st.clone()
        })
    }

    pub fn is_goal_armed(&self, session_id: &str) -> bool {
        self.armed_goals.read().contains(session_id)
    }

    pub fn set_goal_armed(&self, session_id: &str, armed: bool) {
        if armed {
            self.armed_goals.write().insert(session_id.to_string());
        } else {
            self.armed_goals.write().remove(session_id);
        }
    }

    pub fn client_view(&self, session_id: &str) -> Value {
        let st = self.get(session_id);
        let armed = self.is_goal_armed(session_id);
        let mut goal_val = serde_json::to_value(&st.goal).unwrap_or(Value::Null);
        if let Some(obj) = goal_val.as_object_mut() {
            obj.insert("activation".into(), json!(if armed { "armed" } else { "disarmed" }));
        }
        let enriched_tasks = enrich_team_tasks(&st.team.tasks);
        let preset = if st.agent_preset.trim().is_empty() {
            "standard".to_string()
        } else {
            st.agent_preset.clone()
        };
        let active_prs: Vec<&ForgePrRecord> =
            st.forge_prs.iter().filter(|p| !p.dismissed).collect();
        json!({
            "sessionId": session_id,
            "agentPreset": preset,
            "todos": st.todos,
            "planMode": st.plan_mode,
            "goal": goal_val,
            "questions": st.questions,
            "deliverables": st.deliverables,
            "stepTelemetry": st.step_telemetry,
            "actionCards": st.action_cards,
            "forgePrs": active_prs,
            "worktree": st.worktree,
            "team": {
                "enabled": st.team.enabled,
                "tasks": enriched_tasks,
            },
            "contextMeter": st.context_meter,
        })
    }

    pub fn agent_preset(&self, session_id: &str) -> String {
        let p = self.get(session_id).agent_preset;
        if p.trim().is_empty() {
            "standard".into()
        } else {
            p
        }
    }

    pub fn set_agent_preset(&self, session_id: &str, preset: &str) {
        let normalized = preset.trim().to_ascii_lowercase();
        let val = if normalized.is_empty() {
            "standard".to_owned()
        } else {
            normalized
        };
        self.mutate(session_id, |st| {
            st.agent_preset = val;
        });
    }

    pub fn record_step_telemetry(&self, session_id: &str, rec: StepTelemetryRecord) {
        self.mutate(session_id, |st| {
            st.step_telemetry.push(rec);
            if st.step_telemetry.len() > 512 {
                let drain = st.step_telemetry.len() - 512;
                st.step_telemetry.drain(0..drain);
            }
        });
    }

    pub fn propose_action_card(
        &self,
        session_id: &str,
        action: &str,
        title: &str,
        brief: &str,
        details: &str,
        placement: Option<String>,
        target_bundle_id: Option<String>,
    ) -> ActionCardRecord {
        let now = chrono::Utc::now().timestamp_millis();
        let card = ActionCardRecord {
            id: format!("card-{}", &Uuid::new_v4().to_string().replace('-', "")[..12]),
            action: action.to_owned(),
            title: title.chars().take(120).collect(),
            brief: brief.chars().take(320).collect(),
            details: details.chars().take(4_000).collect(),
            status: "pending".into(),
            placement,
            target_bundle_id,
            created_at: now,
        };
        let cloned = card.clone();
        self.mutate(session_id, |st| {
            // Newer pending cards supersede unstarted earlier cards (`dsh-action-cards` rule).
            for prev in &mut st.action_cards {
                if prev.status == "pending" {
                    prev.status = "replaced".into();
                }
            }
            st.action_cards.push(card);
        });
        cloned
    }

    pub fn update_action_card_status(
        &self,
        session_id: &str,
        card_id: &str,
        status: &str,
        placement: Option<String>,
        target_bundle_id: Option<String>,
    ) -> Option<ActionCardRecord> {
        let mut updated = None;
        self.mutate(session_id, |st| {
            if let Some(card) = st.action_cards.iter_mut().find(|c| c.id == card_id) {
                card.status = status.to_owned();
                if let Some(p) = placement {
                    card.placement = Some(p);
                }
                if let Some(b) = target_bundle_id {
                    card.target_bundle_id = Some(b);
                }
                updated = Some(card.clone());
            }
        });
        updated
    }

    pub fn upsert_forge_pr(&self, session_id: &str, pr: ForgePrRecord) {
        self.mutate(session_id, |st| {
            if let Some(existing) = st.forge_prs.iter_mut().find(|p| p.number == pr.number && p.url == pr.url) {
                let dismissed = existing.dismissed;
                *existing = pr;
                existing.dismissed = dismissed;
            } else {
                st.forge_prs.push(pr);
            }
        });
    }

    pub fn set_worktree(&self, session_id: &str, wt: Option<WorktreeRecord>) {
        self.mutate(session_id, |st| {
            st.worktree = wt;
        });
    }

    pub fn write_todos(&self, session_id: &str, input: &Value) -> Result<Value, String> {
        let arr = input
            .get("todos")
            .and_then(Value::as_array)
            .ok_or("TodoWrite requires a `todos` array.")?;
        if arr.len() > 100 {
            return Err("TodoWrite accepts at most 100 items.".into());
        }
        let mut items = Vec::with_capacity(arr.len());
        let mut seen_contents = HashSet::new();
        let mut pending = 0usize;
        let mut in_progress = 0usize;
        let mut completed = 0usize;
        for entry in arr {
            let content = entry
                .get("content")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or("Error: invalid todo: `content` must be a non-empty string")?;
            let capped: String = content.chars().take(400).collect();
            if !seen_contents.insert(capped.clone()) {
                return Err(format!("Error: invalid todos: duplicate content \"{capped}\""));
            }
            let status = entry
                .get("status")
                .and_then(Value::as_str)
                .ok_or("Each todo item needs `status` (`pending`, `in_progress`, or `completed`).")?;
            match status {
                "pending" => pending += 1,
                "in_progress" => in_progress += 1,
                "completed" => completed += 1,
                _ => return Err(format!("Invalid todo status `{status}`.")),
            }
            items.push(TodoItem {
                content: capped,
                status: status.to_string(),
            });
        }
        let count = items.len();
        self.mutate(session_id, |st| {
            st.todos = items;
        });
        Ok(json!({
            "ok": true,
            "count": count,
            "pending": pending,
            "inProgress": in_progress,
            "completed": completed,
            "summary": format!("Updated todo list: {pending} pending, {in_progress} in progress, {completed} completed.")
        }))
    }

    pub fn manage_goal(
        &self,
        session_id: &str,
        action: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let now = chrono::Utc::now().timestamp_millis();
        match action {
            "create" => {
                let objective = payload
                    .get("objective")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or("Goal `objective` must not be empty.")?;
                let max_rounds = payload
                    .get("max_goal_rounds")
                    .or_else(|| payload.get("maxGoalRounds"))
                    .and_then(Value::as_u64)
                    .unwrap_or(25)
                    .clamp(1, 256) as u32;
                let res = self.mutate(session_id, |st| {
                    if let Some(ref cur) = st.goal {
                        if cur.phase != "complete" {
                            return Err("An unfinished goal already exists. Edit, pause, resume, complete, or clear it first.".to_string());
                        }
                    }
                    let goal = GoalSnapshot {
                        id: format!("goal-{}", &Uuid::new_v4().to_string()[..8]),
                        revision: 1,
                        objective: objective.chars().take(4000).collect(),
                        phase: "active".into(),
                        blocked_code: None,
                        blocked_message: None,
                        rounds_started: 0,
                        max_goal_rounds: max_rounds,
                        created_at: now,
                        updated_at: now,
                    };
                    st.goal = Some(goal.clone());
                    Ok(goal)
                })?;
                self.set_goal_armed(session_id, true);
                Ok(serde_json::to_value(res).unwrap())
            }
            "get" => {
                let st = self.get(session_id);
                Ok(json!({
                    "goal": st.goal,
                    "activation": if self.is_goal_armed(session_id) { "armed" } else { "disarmed" }
                }))
            }
            "clear" => {
                self.set_goal_armed(session_id, false);
                self.mutate(session_id, |st| {
                    st.goal = None;
                });
                Ok(json!({"cleared": true}))
            }
            "edit" | "pause" | "resume" | "complete" | "block" => {
                let expected_rev = payload.get("revision").and_then(Value::as_u64);
                let updated = self.mutate(session_id, |st| {
                    let Some(ref mut g) = st.goal else {
                        return Err("No active goal exists in this conversation.".to_string());
                    };
                    if let Some(exp) = expected_rev {
                        if g.revision != exp {
                            return Err(format!(
                                "STALE_GOAL_REVISION: expected {exp}, current is {}. Call GetGoal first.",
                                g.revision
                            ));
                        }
                    }
                    match action {
                        "edit" => {
                            if let Some(obj) = payload
                                .get("objective")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                            {
                                g.objective = obj.chars().take(4000).collect();
                            }
                            if let Some(cap) = payload
                                .get("max_goal_rounds")
                                .or_else(|| payload.get("maxGoalRounds"))
                                .and_then(Value::as_u64)
                            {
                                g.max_goal_rounds = cap.clamp(1, 256) as u32;
                            }
                        }
                        "pause" => {
                            g.phase = "paused".into();
                        }
                        "resume" => {
                            if g.rounds_started >= g.max_goal_rounds {
                                return Err("Goal round cap reached; increase max_goal_rounds first.".into());
                            }
                            g.phase = "active".into();
                            g.blocked_code = None;
                            g.blocked_message = None;
                        }
                        "complete" => {
                            g.phase = "complete".into();
                            g.blocked_code = None;
                            g.blocked_message = None;
                        }
                        "block" => {
                            g.phase = "blocked".into();
                            g.blocked_code = Some(
                                payload
                                    .get("code")
                                    .and_then(Value::as_str)
                                    .unwrap_or("needs-input")
                                    .to_string(),
                            );
                            g.blocked_message = Some(
                                payload
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Goal blocked awaiting input.")
                                    .to_string(),
                            );
                        }
                        _ => {}
                    }
                    g.revision += 1;
                    g.updated_at = now;
                    Ok(g.clone())
                })?;
                match action {
                    "resume" => self.set_goal_armed(session_id, true),
                    "pause" | "complete" | "block" => self.set_goal_armed(session_id, false),
                    _ => {}
                }
                Ok(serde_json::to_value(updated).unwrap())
            }
            other => Err(format!("Unsupported goal action `{other}`.")),
        }
    }

    /// Claims one autonomous continuation round if the session has an armed active goal with remaining budget.
    pub fn claim_next_goal_round(&self, session_id: &str) -> Option<(u32, u32, String)> {
        if !self.is_goal_armed(session_id) {
            return None;
        }
        let outcome = self.mutate(session_id, |st| {
            let g = st.goal.as_mut()?;
            if g.phase != "active" {
                return None;
            }
            if g.rounds_started >= g.max_goal_rounds {
                g.phase = "blocked".into();
                g.blocked_code = Some("round-budget-exhausted".into());
                g.blocked_message = Some(format!(
                    "Reached maximum goal rounds ({}).",
                    g.max_goal_rounds
                ));
                g.revision += 1;
                return None;
            }
            g.rounds_started += 1;
            g.updated_at = chrono::Utc::now().timestamp_millis();
            Some((g.rounds_started, g.max_goal_rounds, g.objective.clone()))
        });
        if outcome.is_none() {
            self.set_goal_armed(session_id, false);
        }
        outcome
    }

    pub fn record_continued_question(&self, session_id: &str, call_id: &str, questions: Vec<Value>) {
        self.mutate(session_id, |st| {
            if !st.questions.iter().any(|q| q.call_id == call_id) {
                st.questions.push(ContinuedQuestion {
                    call_id: call_id.to_string(),
                    state: "continued".into(),
                    questions,
                    answers: vec![],
                    created_at: chrono::Utc::now().timestamp_millis(),
                });
            }
        });
    }

    pub fn settle_continued_question(
        &self,
        session_id: &str,
        call_id: &str,
        answers: Vec<Value>,
    ) -> Result<(), String> {
        self.mutate(session_id, |st| {
            let Some(q) = st.questions.iter_mut().find(|q| q.call_id == call_id) else {
                return Err("Question is no longer open.".to_string());
            };
            if q.state == "settled" {
                return Err("Question was already answered.".to_string());
            }
            q.state = "settled".into();
            q.answers = answers;
            Ok(())
        })
    }

    pub fn record_turn_deliverables(
        &self,
        session_id: &str,
        assistant_uuid: Option<String>,
        presented_files: Vec<PresentedFile>,
        workspace_changes: Option<quixi_workspace::changes::TurnChangesSummary>,
    ) {
        if presented_files.is_empty() && workspace_changes.is_none() {
            return;
        }
        self.mutate(session_id, |st| {
            let next_idx = st.deliverables.last().map_or(1, |d| d.turn_index + 1);
            st.deliverables.push(TurnDeliverableRecord {
                turn_index: next_idx,
                assistant_uuid,
                presented_files,
                workspace_changes,
                recorded_at: chrono::Utc::now().timestamp_millis(),
            });
            if st.deliverables.len() > 25 {
                let drain = st.deliverables.len() - 25;
                st.deliverables.drain(..drain);
            }
        });
    }

    pub fn update_context_meter(
        &self,
        session_id: &str,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_read: Option<u64>,
        cache_write: Option<u64>,
        reasoning: Option<u64>,
        projected_tokens: u64,
        context_window: u64,
        compacted: bool,
    ) {
        self.mutate(session_id, |st| {
            let m = &mut st.context_meter;
            if let Some(inp) = input_tokens {
                m.input_tokens = inp;
            }
            if let Some(out) = output_tokens {
                m.output_tokens = out;
            }
            if let Some(cr) = cache_read {
                m.cache_read_tokens = cr;
            }
            if let Some(cw) = cache_write {
                m.cache_write_tokens = cw;
            }
            if let Some(r) = reasoning {
                m.reasoning_tokens = r;
            }
            if input_tokens.is_some() || cache_read.is_some() || cache_write.is_some() {
                m.pressure_tokens = m.input_tokens + m.cache_read_tokens + m.cache_write_tokens;
            }
            m.projected_tokens = projected_tokens;
            m.context_window = context_window;
            if compacted {
                m.compactions += 1;
            }
        });
    }

    pub fn set_team_enabled(&self, session_id: &str, enabled: bool) -> Value {
        self.mutate(session_id, |st| {
            st.team.enabled = enabled;
            json!({
                "enabled": st.team.enabled,
                "tasks": enrich_team_tasks(&st.team.tasks),
            })
        })
    }

    pub fn team_create_task(&self, session_id: &str, input: &Value) -> Result<Value, String> {
        let subject = input
            .get("subject")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Task `subject` must not be empty.")?;
        let description = input
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let blocked_by: Vec<String> = input
            .get("blocked_by")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let write_scopes: Vec<String> = input
            .get("write_scopes")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim_matches('/').replace('\\', "/"))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        self.mutate(session_id, |st| {
            if st.team.tasks.len() >= 256 {
                return Err("Team task board limit (256) reached.".into());
            }
            for dep in &blocked_by {
                if !st
                    .team
                    .tasks
                    .iter()
                    .any(|t| &t.id == dep && t.status != "deleted")
                {
                    return Err(format!("Unknown or deleted blocker task `{dep}`."));
                }
            }
            st.team.enabled = true;
            st.team.next_seq += 1;
            let task = TeamTask {
                id: format!("task-{}", st.team.next_seq),
                revision: 1,
                subject: subject.chars().take(200).collect(),
                description: description.chars().take(4000).collect(),
                status: "pending".into(),
                owner_id: None,
                blocked_by,
                write_scopes,
            };
            st.team.tasks.push(task.clone());
            let views = enrich_team_tasks(&st.team.tasks);
            Ok(views
                .into_iter()
                .find(|v| v["id"] == task.id)
                .unwrap_or_else(|| json!(task)))
        })
    }

    pub fn team_update_task(&self, session_id: &str, input: &Value) -> Result<Value, String> {
        let id = input
            .get("id")
            .and_then(Value::as_str)
            .ok_or("Missing task `id`.")?;
        let expected_rev = input.get("revision").and_then(Value::as_u64);
        self.mutate(session_id, |st| {
            let mut candidate = st.team.tasks.clone();
            let Some(pos) = candidate.iter().position(|t| t.id == id) else {
                return Err(format!("Unknown team task `{id}`."));
            };
            if let Some(exp) = expected_rev {
                if candidate[pos].revision != exp {
                    return Err(format!(
                        "STALE_TASK_REVISION: expected {exp}, current is {}.",
                        candidate[pos].revision
                    ));
                }
            }
            if let Some(status) = input.get("status").and_then(Value::as_str) {
                if !matches!(status, "pending" | "in_progress" | "completed" | "deleted") {
                    return Err(format!("Invalid task status `{status}`."));
                }
                candidate[pos].status = status.to_string();
                if status == "pending" {
                    candidate[pos].owner_id = None;
                }
            }
            if let Some(owner) = input.get("owner_id").and_then(Value::as_str) {
                candidate[pos].owner_id = (!owner.trim().is_empty()).then(|| owner.to_string());
            }
            if let Some(subj) = input
                .get("subject")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                candidate[pos].subject = subj.chars().take(200).collect();
            }
            if let Some(deps) = input.get("blocked_by").and_then(Value::as_array) {
                let list: Vec<String> = deps
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
                for dep in &list {
                    if !candidate
                        .iter()
                        .any(|t| &t.id == dep && t.status != "deleted")
                    {
                        return Err(format!("Unknown or deleted blocker `{dep}`."));
                    }
                }
                candidate[pos].blocked_by = list;
            }
            if has_task_cycle(&candidate) {
                return Err("Task update rejected: `blocked_by` edges would introduce a dependency cycle.".into());
            }
            candidate[pos].revision += 1;
            let target_id = candidate[pos].id.clone();
            st.team.tasks = candidate;
            let views = enrich_team_tasks(&st.team.tasks);
            Ok(views
                .into_iter()
                .find(|v| v["id"] == target_id)
                .unwrap_or(Value::Null))
        })
    }
}

fn has_task_cycle(tasks: &[TeamTask]) -> bool {
    let by_id: HashMap<&str, &TeamTask> = tasks
        .iter()
        .filter(|t| t.status != "deleted")
        .map(|t| (t.id.as_str(), t))
        .collect();
    let mut state: HashMap<&str, u8> = HashMap::new();
    fn dfs<'a>(
        node: &'a str,
        by_id: &HashMap<&'a str, &'a TeamTask>,
        state: &mut HashMap<&'a str, u8>,
    ) -> bool {
        match state.get(node).copied() {
            Some(1) => return true,
            Some(2) => return false,
            _ => {}
        }
        state.insert(node, 1);
        if let Some(task) = by_id.get(node) {
            for dep in &task.blocked_by {
                if by_id.contains_key(dep.as_str()) && dfs(dep.as_str(), by_id, state) {
                    return true;
                }
            }
        }
        state.insert(node, 2);
        false
    }
    for &id in by_id.keys() {
        if dfs(id, &by_id, &mut state) {
            return true;
        }
    }
    false
}

fn scopes_overlap(a: &str, b: &str) -> bool {
    a == b
        || a.starts_with(&format!("{b}/"))
        || b.starts_with(&format!("{a}/"))
}

pub fn enrich_team_tasks(tasks: &[TeamTask]) -> Vec<Value> {
    let completed: HashSet<&str> = tasks
        .iter()
        .filter(|t| t.status == "completed")
        .map(|t| t.id.as_str())
        .collect();
    tasks
        .iter()
        .filter(|t| t.status != "deleted")
        .map(|t| {
            let ready = t.status == "pending"
                && t.blocked_by
                    .iter()
                    .all(|dep| completed.contains(dep.as_str()));
            let mut warnings = Vec::new();
            if t.status == "in_progress" {
                for other in tasks {
                    if other.id != t.id && other.status == "in_progress" {
                        for s1 in &t.write_scopes {
                            for s2 in &other.write_scopes {
                                if scopes_overlap(s1, s2) {
                                    warnings.push(format!(
                                        "Scope `{s1}` overlaps with in-progress {} (`{s2}`)",
                                        other.id
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            json!({
                "id": t.id,
                "revision": t.revision,
                "subject": t.subject,
                "description": t.description,
                "status": t.status,
                "ownerId": t.owner_id,
                "blockedBy": t.blocked_by,
                "writeScopes": t.write_scopes,
                "ready": ready,
                "writeScopeWarnings": warnings,
            })
        })
        .collect()
}

pub fn collaboration_tool_schemas(team_enabled: bool) -> Vec<Value> {
    let mut list = vec![
        json!({
            "type": "function",
            "function": {
                "name": TOOL_TODO_WRITE,
                "description": "Replace this conversation's live task checklist (`pending`, `in_progress`, `completed`) so the user can see step-by-step progress.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": { "type": "string" },
                                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                                },
                                "required": ["content", "status"]
                            }
                        }
                    },
                    "required": ["todos"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_EXIT_PLAN_MODE,
                "description": "Submit a complete Markdown implementation plan (starting with `# `) for user approval when Plan Mode is active.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "plan": { "type": "string", "description": "Complete Markdown plan starting with `# `." }
                    },
                    "required": ["plan"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_ASK_USER,
                "description": "Ask the user 1-4 structured choice questions when a decision genuinely belongs to the user. Supports optional `timeout_seconds` so unanswered questions remain open for late reply while you continue independent work.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "questions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "header": { "type": "string" },
                                    "question": { "type": "string" },
                                    "detail": { "type": "string" },
                                    "multi_select": { "type": "boolean" },
                                    "options": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": { "type": "string" },
                                                "description": { "type": "string" }
                                            },
                                            "required": ["label"]
                                        }
                                    }
                                },
                                "required": ["question"]
                            }
                        },
                        "timeout_seconds": { "type": "integer", "description": "Optional foreground wait in seconds (1-300). Omit to wait until answered." }
                    },
                    "required": ["questions"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_PRESENT,
                "description": "Declare final deliverable files created or updated during this turn so the user gets direct file cards at the end of the turn.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "files": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "path": { "type": "string" },
                                    "description": { "type": "string" }
                                },
                                "required": ["path"]
                            }
                        }
                    },
                    "required": ["files"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_GET_GOAL,
                "description": "Read the current persistent session goal, revision, phase, and round budget.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_CREATE_GOAL,
                "description": "Create an autonomous multi-round completion objective for this conversation.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "objective": { "type": "string" },
                        "max_goal_rounds": { "type": "integer", "description": "Max continuation rounds (default 25, max 256)." }
                    },
                    "required": ["objective"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_UPDATE_GOAL,
                "description": "Compare-and-set update on the current session goal (`complete`, `block`, `pause`, `edit`).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["complete", "block", "pause", "edit"] },
                        "revision": { "type": "integer" },
                        "objective": { "type": "string" },
                        "max_goal_rounds": { "type": "integer" },
                        "code": { "type": "string", "description": "Blocker code when action is `block`." },
                        "message": { "type": "string", "description": "Explanation when action is `block`." }
                    },
                    "required": ["action"],
                    "additionalProperties": false
                }
            }
        }),
    ];

    if team_enabled {
        list.extend([
            json!({
                "type": "function",
                "function": {
                    "name": TOOL_TEAM_CREATE,
                    "description": "Create a task on the shared Team task DAG with optional `blocked_by` task IDs and advisory `write_scopes` path prefixes.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "subject": { "type": "string" },
                            "description": { "type": "string" },
                            "blocked_by": { "type": "array", "items": { "type": "string" } },
                            "write_scopes": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["subject"],
                        "additionalProperties": false
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": TOOL_TEAM_LIST,
                    "description": "List all non-deleted tasks on the shared Team task board, including readiness and write-scope overlap warnings.",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": [],
                        "additionalProperties": false
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": TOOL_TEAM_UPDATE,
                    "description": "Compare-and-set update on a shared Team task (`pending`, `in_progress`, `completed`, `deleted`, `owner_id`, `blocked_by`).",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "revision": { "type": "integer" },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed", "deleted"] },
                            "owner_id": { "type": "string" },
                            "subject": { "type": "string" },
                            "blocked_by": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["id"],
                        "additionalProperties": false
                    }
                }
            }),
        ]);
    }
    list
}

/// Canonicalizes JSON by deep-sorting object keys so `{"a":1,"b":2}` and `{"b":2,"a":1}`
/// match identically in consecutive repeat-call tracking.
pub fn canonicalize_json(val: &Value) -> Value {
    match val {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), canonicalize_json(v)))
                .collect();
            let mut out = serde_json::Map::new();
            for (k, v) in sorted {
                out.insert(k, v);
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize_json).collect()),
        other => other.clone(),
    }
}

/// Loop-hygiene guard inspired by `@deepseek-ai/dsh-repeat-tool-reminder`.
/// Bookkeeping tools (`TodoWrite`, `GetGoal`, `JobList`, `TeamTaskList`) are transparent so
/// `Grep X -> TodoWrite -> Grep X` still counts as consecutive repeats!
#[derive(Default)]
pub struct RepeatToolGuard {
    last_signature: Option<String>,
    last_tool_name: String,
    last_preview: String,
    streak: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RepeatVerdict {
    None,
    Nudge(String),
    StopLoop(String),
}

impl RepeatToolGuard {
    pub fn is_transparent(tool_name: &str) -> bool {
        matches!(
            tool_name,
            TOOL_TODO_WRITE | TOOL_GET_GOAL | "JobList" | TOOL_TEAM_LIST
        )
    }

    pub fn observe(&mut self, tool_name: &str, raw_args: &str) -> RepeatVerdict {
        if Self::is_transparent(tool_name) {
            return RepeatVerdict::None;
        }
        let canonical_args = serde_json::from_str::<Value>(raw_args)
            .map(|v| canonicalize_json(&v).to_string())
            .unwrap_or_else(|_| raw_args.trim().to_string());
        let sig = format!("{tool_name}|{canonical_args}");
        if self.last_signature.as_deref() == Some(sig.as_str()) {
            self.streak += 1;
        } else {
            self.last_signature = Some(sig);
            self.last_tool_name = tool_name.to_string();
            self.last_preview = canonical_args.chars().take(320).collect();
            self.streak = 1;
        }

        match self.streak {
            3 => RepeatVerdict::Nudge(format!(
                "Loop-hygiene notice: `{}` has been called 3 consecutive times with identical arguments. Verify whether workspace state changed or choose a different approach.",
                self.last_tool_name
            )),
            5 => RepeatVerdict::Nudge(format!(
                "Loop-hygiene warning: `{}` has been called 5 consecutive times with identical arguments (`{}`). Stop repeating this exact call; inspect why it is not progressing or report the blocker.",
                self.last_tool_name, self.last_preview
            )),
            n if n >= 8 => RepeatVerdict::StopLoop(format!(
                "Stopped runaway tool loop after {n} identical `{}` invocations.",
                self.last_tool_name
            )),
            _ => RepeatVerdict::None,
        }
    }

    pub fn reset_on_mutation(&mut self) {
        self.last_signature = None;
        self.streak = 0;
    }
}

#[allow(dead_code)]
fn _keep_mutex_import(_: &Mutex<()>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_guard_treats_todo_write_as_transparent_and_canonicalizes_json_keys() {
        let mut guard = RepeatToolGuard::default();
        assert_eq!(guard.observe("Grep", r#"{"query":"foo","path":"src"}"#), RepeatVerdict::None);
        // Transparent bookkeeping call in between!
        assert_eq!(guard.observe("TodoWrite", r#"{"todos":[]}"#), RepeatVerdict::None);
        // Reordered keys still match!
        assert_eq!(guard.observe("Grep", r#"{"path":"src","query":"foo"}"#), RepeatVerdict::None);
        assert!(matches!(
            guard.observe("Grep", r#"{"query":"foo","path":"src"}"#),
            RepeatVerdict::Nudge(_)
        ));
    }

    #[test]
    fn goal_disarms_across_reload_and_team_dag_rejects_cycles() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SessionCollaborationStore::new(tmp.path().to_path_buf());
        store
            .manage_goal("s1", "create", &json!({"objective": "Ship tests", "max_goal_rounds": 3}))
            .unwrap();
        assert!(store.is_goal_armed("s1"));
        assert!(store.claim_next_goal_round("s1").is_some());

        // Simulate app restart on same data_dir: durable phase stays `active`, but `armed` is false!
        let reloaded = SessionCollaborationStore::new(tmp.path().to_path_buf());
        assert_eq!(reloaded.get("s1").goal.unwrap().phase, "active");
        assert!(!reloaded.is_goal_armed("s1"));
        assert!(reloaded.claim_next_goal_round("s1").is_none());

        // Team DAG cycle check:
        let t1 = reloaded
            .team_create_task("s1", &json!({"subject": "First", "write_scopes": ["crates/a"]}))
            .unwrap();
        let t2 = reloaded
            .team_create_task(
                "s1",
                &json!({"subject": "Second", "blocked_by": [t1["id"]], "write_scopes": ["crates/a/sub"]}),
            )
            .unwrap();
        assert_eq!(t2["ready"], false);
        let cycle = reloaded.team_update_task(
            "s1",
            &json!({"id": t1["id"], "blocked_by": [t2["id"]]}),
        );
        assert!(cycle.is_err());
    }
}
