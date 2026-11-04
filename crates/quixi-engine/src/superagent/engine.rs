//! Stateful `SuperagentService` implementing `dsh-superagent`'s Trigger -> Eval -> Ticket ->
//! Assignment -> Outbox domain engine, resource reader, GitHub watcher, and 6-tab panel views.

use super::domain::*;
use parking_lot::RwLock;
use quixi_cordis::ServiceKey;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

pub const SUPERAGENT_SERVICE: ServiceKey<SuperagentService> =
    ServiceKey::new("superagent", "Proactive repository orchestrator (`dsh-superagent`)");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshotRecord {
    pub revision: u64,
    pub block: String,
    pub origin: String,
    pub policy_version: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuperagentPersistedState {
    pub schema_version: u32,
    pub paused: bool,
    pub settings_revision: u64,
    pub prompt_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_session_id: Option<String>,
    pub policy: PolicySettings,
    pub github: GithubSourceConfig,
    pub feishu: FeishuSourceConfig,
    pub executor: ExecutorSettings,
    pub prompts: PromptConfig,
    #[serde(default)]
    pub prompt_history: Vec<PromptVersionRecord>,
    #[serde(default)]
    pub settings_history: Vec<SettingsSnapshotRecord>,
    #[serde(default)]
    pub triggers: BTreeMap<String, Trigger>,
    #[serde(default)]
    pub dispositions: BTreeMap<String, Disposition>,
    #[serde(default)]
    pub threads: BTreeMap<String, ThreadRecord>,
    #[serde(default)]
    pub decisions: BTreeMap<String, DecisionRecord>,
    #[serde(default)]
    pub tickets: BTreeMap<String, Ticket>,
    #[serde(default)]
    pub outbox: BTreeMap<String, OutboxReceipt>,
    #[serde(default)]
    pub seen_github_ids: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_github_error: Option<String>,
}

impl Default for SuperagentPersistedState {
    fn default() -> Self {
        let prompts = PromptConfig::default();
        Self {
            schema_version: 11,
            paused: false,
            settings_revision: 1,
            prompt_revision: 1,
            master_session_id: None,
            policy: PolicySettings::default(),
            github: GithubSourceConfig::default(),
            feishu: FeishuSourceConfig::default(),
            executor: ExecutorSettings::default(),
            prompt_history: vec![PromptVersionRecord {
                revision: 1,
                origin: "default".into(),
                at: now_iso(),
                config: prompts.clone(),
            }],
            prompts,
            settings_history: Vec::new(),
            triggers: BTreeMap::new(),
            dispositions: BTreeMap::new(),
            threads: BTreeMap::new(),
            decisions: BTreeMap::new(),
            tickets: BTreeMap::new(),
            outbox: BTreeMap::new(),
            seen_github_ids: BTreeSet::new(),
            last_github_error: None,
        }
    }
}

pub struct SuperagentService {
    pub state_dir: PathBuf,
    pub state: RwLock<SuperagentPersistedState>,
}

impl SuperagentService {
    pub fn open(data_dir: &Path) -> Self {
        let state_dir = data_dir.join("superagent");
        let _ = fs::create_dir_all(state_dir.join("work"));
        let state_path = state_dir.join("orchestrator.json");
        let mut loaded: SuperagentPersistedState = fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if loaded.executor.work_root.trim().is_empty() {
            loaded.executor.work_root = state_dir.join("work").to_string_lossy().into_owned();
        }
        Self {
            state_dir,
            state: RwLock::new(loaded),
        }
    }

    pub fn save_locked(&self, st: &SuperagentPersistedState) {
        let path = self.state_dir.join("orchestrator.json");
        if let Ok(raw) = serde_json::to_string_pretty(st) {
            let _ = fs::write(path, raw);
        }
    }

    pub fn mutate<R>(&self, f: impl FnOnce(&mut SuperagentPersistedState) -> R) -> R {
        let mut guard = self.state.write();
        let out = f(&mut guard);
        self.save_locked(&guard);
        out
    }

    /// Intake a Trigger (`Engine.receive` in `007-architecture.md`):
    /// - Deduplicates by `trigger.id`.
    /// - Fans out to subscribed Tickets (resuming ended non-stopped Tickets on human messages).
    /// - Unless a human follow-up is owned by a subscribed Ticket, queues on primary Thread for Eval.
    pub fn receive_trigger(&self, mut trigger: Trigger) -> Value {
        if trigger.id.trim().is_empty() {
            trigger.id = format!("trg_{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
        }
        if trigger.at.trim().is_empty() {
            trigger.at = now_iso();
        }
        if trigger.threads.is_empty() {
            trigger.threads.push(format!("{}:general", trigger.source));
        }

        self.mutate(|st| {
            if st.triggers.contains_key(&trigger.id) {
                return json!({ "disposition": "duplicate", "triggerId": trigger.id });
            }
            // Drop self-messages from configured GitHub agent account:
            if trigger.source == "github"
                && !st.github.agent_account.trim().is_empty()
                && trigger
                    .actor
                    .eq_ignore_ascii_case(st.github.agent_account.trim())
            {
                return json!({ "disposition": "filtered", "reason": "self_message" });
            }

            let primary_thread = trigger.threads[0].clone();
            let mut disp = Disposition::default();
            let is_human = !trigger.is_bot && !trigger.actor.ends_with("[bot]");

            for th_key in &trigger.threads {
                let th = st.threads.entry(th_key.clone()).or_insert_with(|| ThreadRecord {
                    key: th_key.clone(),
                    source: trigger.source.clone(),
                    title: if trigger.title.trim().is_empty() {
                        th_key.clone()
                    } else {
                        trigger.title.clone()
                    },
                    url: trigger.url.clone(),
                    subscriptions: Vec::new(),
                    context_only: Vec::new(),
                    queued_triggers: Vec::new(),
                    running_eval: None,
                    expired_count: 0,
                    decision_ids: Vec::new(),
                    updated_at: trigger.at.clone(),
                    meta: json!({}),
                });
                th.updated_at = trigger.at.clone();
            }

            // Fan-out to subscribed Tickets:
            let subscribed_ticket_ids: Vec<(String, bool)> = trigger
                .threads
                .iter()
                .filter_map(|tk| st.threads.get(tk))
                .flat_map(|th| {
                    th.subscriptions
                        .iter()
                        .map(move |tid| (tid.clone(), th.context_only.contains(tid)))
                })
                .collect();

            let mut owned_by_ticket = false;
            for (tid, is_context_only) in subscribed_ticket_ids {
                let policy_clone = st.policy.clone();
                let paused = st.paused;
                if let Some(ticket) = st.tickets.get_mut(&tid) {
                    let stopped_by_person = ticket
                        .ended
                        .as_ref()
                        .is_some_and(|e| e.outcome == "stopped");
                    if stopped_by_person {
                        continue;
                    }
                    if ticket.ended.is_some() {
                        // Ended tickets only wake on human messages on non-context threads if Mandate still holds!
                        if !is_human || is_context_only || !policy_clone.revalidate(&ticket.mandate)
                        {
                            continue;
                        }
                        if !paused {
                            ticket.ended = None;
                        }
                    }
                    let inp_id = format!("inp_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]);
                    ticket.inputs.push(TicketInput {
                        id: inp_id,
                        kind: "trigger".into(),
                        trigger_id: Some(trigger.id.clone()),
                        from_person: is_human,
                        text: format!("[{} by {}] {}", trigger.source, trigger.actor, trigger.text),
                        delivered: false,
                        read: false,
                        at: trigger.at.clone(),
                    });
                    ticket.updated_at = trigger.at.clone();
                    if !disp.delivered_tickets.contains(&tid) {
                        disp.delivered_tickets.push(tid.clone());
                    }
                    if is_human && !is_context_only {
                        owned_by_ticket = true;
                    }
                }
            }

            // If not owned as a human follow-up by an existing Ticket, run Gate & queue for Eval:
            if !owned_by_ticket {
                let has_own_ticket = st
                    .threads
                    .get(&primary_thread)
                    .is_some_and(|th| !th.subscriptions.is_empty());
                match st.policy.gate(&trigger, has_own_ticket) {
                    Some((strength, basis)) => {
                        if let Some(th) = st.threads.get_mut(&primary_thread) {
                            th.queued_triggers.push(trigger.id.clone());
                            if !st.paused && th.running_eval.is_none() {
                                let eval_id = format!(
                                    "eval_{}",
                                    &Uuid::new_v4().to_string().replace('-', "")[..10]
                                );
                                let batch = std::mem::take(&mut th.queued_triggers);
                                disp.eval_id = Some(eval_id.clone());
                                th.running_eval = Some(RunningEval {
                                    id: eval_id,
                                    trigger_ids: batch,
                                    primary_trigger_id: trigger.id.clone(),
                                    strength,
                                    basis,
                                    policy_version: st.policy.policy_version(),
                                    started_at_ms: chrono::Utc::now().timestamp_millis(),
                                    delivered_at_ms: Some(chrono::Utc::now().timestamp_millis()),
                                });
                            }
                        }
                    }
                    None => {
                        disp.filtered = Some("not_admitted_by_policy_gate".into());
                    }
                }
            }

            let tid = trigger.id.clone();
            let status = if disp.filtered.is_some() {
                "filtered"
            } else {
                "stored"
            };
            st.triggers.insert(tid.clone(), trigger);
            st.dispositions.insert(tid.clone(), disp.clone());
            json!({
                "disposition": status,
                "triggerId": tid,
                "deliveredTickets": disp.delivered_tickets,
                "evalId": disp.eval_id,
            })
        })
    }

    /// Executes `orchestrator_route` batch decisions (`accept`, `attach`, `stop`, `ignore`).
    pub fn route_decisions(&self, items: &[Value]) -> Value {
        if items.is_empty() || items.len() > 20 {
            return json!({ "error": "orchestrator_route requires 1..=20 decision items." });
        }
        self.mutate(|st| {
            let mut results = Vec::with_capacity(items.len());
            for item in items {
                let kind = item["kind"].as_str().unwrap_or("");
                let trigger_id = item["trigger_id"].as_str().unwrap_or("");
                let Some(trig) = st.triggers.get(trigger_id).cloned() else {
                    results.push(json!({
                        "trigger_id": trigger_id,
                        "kind": kind,
                        "refused": format!("Unknown trigger `{trigger_id}`"),
                    }));
                    continue;
                };
                let primary_th = trig
                    .threads
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "github:general".into());
                let pver = st.policy.policy_version();

                match kind {
                    "accept" => {
                        let title = item["title"].as_str().unwrap_or("").trim();
                        let wt_name = item["worktree_name"].as_str().unwrap_or("").trim();
                        let ack = item["acknowledgement"].as_str().unwrap_or("").trim();
                        let note = item["note"].as_str().map(str::trim).filter(|s| !s.is_empty());
                        if title.is_empty() || wt_name.is_empty() || ack.is_empty() {
                            results.push(json!({
                                "trigger_id": trigger_id,
                                "kind": "accept",
                                "refused": "accept requires non-empty `title`, `worktree_name`, and `acknowledgement`",
                            }));
                            continue;
                        }
                        if word_count(ack) > IM_WORD_LIMIT {
                            results.push(json!({
                                "trigger_id": trigger_id,
                                "kind": "accept",
                                "refused": format!("acknowledgement exceeds {IM_WORD_LIMIT} words"),
                            }));
                            continue;
                        }
                        if let Some(n) = note {
                            if word_count(n) > ROUTING_NOTE_WORD_LIMIT {
                                results.push(json!({
                                    "trigger_id": trigger_id,
                                    "kind": "accept",
                                    "refused": format!("routing note exceeds {ROUTING_NOTE_WORD_LIMIT} words"),
                                }));
                                continue;
                            }
                        }

                        // Direct accept requires explicit strength (`002-authorization.md:89`):
                        let eval_strength = st
                            .threads
                            .get(&primary_th)
                            .and_then(|t| t.running_eval.as_ref())
                            .map(|e| (e.strength, e.basis.clone()))
                            .or_else(|| st.policy.gate(&trig, false));
                        let Some((Strength::Explicit, basis)) = eval_strength else {
                            results.push(json!({
                                "trigger_id": trigger_id,
                                "kind": "accept",
                                "refused": "Direct accept requires an explicit request; use `orchestrator_eval_decide` for inferred candidates",
                            }));
                            continue;
                        };

                        let ticket = create_ticket_in_state(
                            st,
                            &trig,
                            &primary_th,
                            title,
                            wt_name,
                            ack,
                            note,
                            Strength::Explicit,
                            basis,
                            Vec::new(),
                            "Explicit request accepted",
                        );
                        results.push(json!({
                            "trigger_id": trigger_id,
                            "kind": "accept",
                            "decision": "accept",
                            "reason": "Explicit request accepted",
                            "ticket": ticket_card(&ticket),
                        }));
                    }
                    "attach" => {
                        let ticket_id = item["ticket_id"].as_str().unwrap_or("");
                        let note = item["note"].as_str().map(str::trim).filter(|s| !s.is_empty());
                        let Some(ticket) = st.tickets.get_mut(ticket_id) else {
                            results.push(json!({
                                "trigger_id": trigger_id,
                                "kind": "attach",
                                "refused": format!("Unknown ticket `{ticket_id}`"),
                            }));
                            continue;
                        };
                        if ticket.ended.as_ref().is_some_and(|e| e.outcome == "stopped") {
                            results.push(json!({
                                "trigger_id": trigger_id,
                                "kind": "attach",
                                "refused": "Cannot attach to a ticket stopped by a person",
                            }));
                            continue;
                        }
                        if ticket.ended.is_some() && !st.paused {
                            ticket.ended = None;
                        }
                        ticket.inputs.push(TicketInput {
                            id: format!("inp_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]),
                            kind: "trigger".into(),
                            trigger_id: Some(trig.id.clone()),
                            from_person: !trig.is_bot,
                            text: match note {
                                Some(n) => format!("[Attached note: {n}]\n{}", trig.text),
                                None => trig.text.clone(),
                            },
                            delivered: false,
                            read: false,
                            at: now_iso(),
                        });
                        let card = ticket_card(ticket);
                        if let Some(th) = st.threads.get_mut(&primary_th) {
                            if !th.subscriptions.contains(&ticket_id.to_owned()) {
                                th.subscriptions.push(ticket_id.to_owned());
                            }
                            th.running_eval = None;
                        }
                        results.push(json!({
                            "trigger_id": trigger_id,
                            "kind": "attach",
                            "reason": "Attached trigger and subscribed ticket to thread",
                            "ticket": card,
                        }));
                    }
                    "stop" => {
                        let ticket_id = item["ticket_id"].as_str().unwrap_or("");
                        let reason = item["reason"]
                            .as_str()
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or("Stopped on human request");
                        let Some(ticket) = st.tickets.get_mut(ticket_id) else {
                            results.push(json!({
                                "trigger_id": trigger_id,
                                "kind": "stop",
                                "refused": format!("Unknown ticket `{ticket_id}`"),
                            }));
                            continue;
                        };
                        ticket.mandate.withdrawn = true;
                        ticket.ended = Some(TicketEnd {
                            outcome: "stopped".into(),
                            by: trig.actor.clone(),
                            reason: reason.to_owned(),
                            at: now_iso(),
                        });
                        let card = ticket_card(ticket);
                        record_decision_in_state(
                            st,
                            &primary_th,
                            vec![trig.id.clone()],
                            "stop",
                            false,
                            false,
                            reason,
                            Vec::new(),
                            Some(ticket_id.to_owned()),
                            &pver,
                        );
                        results.push(json!({
                            "trigger_id": trigger_id,
                            "kind": "stop",
                            "decision": "stop",
                            "reason": reason,
                            "ticket": card,
                        }));
                    }
                    "ignore" => {
                        let reason = item["reason"].as_str().unwrap_or("").trim();
                        if reason.is_empty() {
                            results.push(json!({
                                "trigger_id": trigger_id,
                                "kind": "ignore",
                                "refused": "ignore requires a non-empty `reason`",
                            }));
                            continue;
                        }
                        let is_inferred = st
                            .threads
                            .get(&primary_th)
                            .and_then(|t| t.running_eval.as_ref())
                            .is_some_and(|e| e.strength == Strength::Inferred);
                        record_decision_in_state(
                            st,
                            &primary_th,
                            vec![trig.id.clone()],
                            "ignore",
                            false,
                            is_inferred,
                            reason,
                            Vec::new(),
                            None,
                            &pver,
                        );
                        results.push(json!({
                            "trigger_id": trigger_id,
                            "kind": "ignore",
                            "decision": "ignore",
                            "reason": reason,
                        }));
                    }
                    other => results.push(json!({
                        "trigger_id": trigger_id,
                        "kind": other,
                        "refused": format!("Unknown route kind `{other}`"),
                    })),
                }
            }
            json!({ "results": results })
        })
    }

    /// Proactive assessment context (`orchestrator_eval`).
    pub fn eval_context(&self, trigger_id: &str) -> Result<Value, String> {
        let st = self.state.read();
        let trig = st
            .triggers
            .get(trigger_id)
            .ok_or_else(|| format!("Unknown trigger `{trigger_id}`"))?;
        let th_key = trig
            .threads
            .first()
            .cloned()
            .unwrap_or_else(|| "github:general".into());
        let related_tickets: Vec<Value> = st
            .tickets
            .values()
            .filter(|t| t.origin_thread == th_key || t.channels.contains(&th_key))
            .take(8)
            .map(ticket_card)
            .collect();
        let thread_msgs: Vec<Value> = st
            .triggers
            .values()
            .filter(|t| t.threads.contains(&th_key))
            .rev()
            .take(11)
            .map(|t| {
                json!({
                    "id": t.id,
                    "from": t.actor,
                    "at": t.at,
                    "text": t.text.chars().take(1200).collect::<String>(),
                })
            })
            .collect();
        Ok(json!({
            "trigger": trig,
            "thread": th_key,
            "policy_version": st.policy.policy_version(),
            "accept_mode": st.policy.accept_mode,
            "enabled_rules": st.policy.rules.iter().filter(|r| r.enabled).collect::<Vec<_>>(),
            "related_tickets": related_tickets,
            "tickets_total": related_tickets.len(),
            "window": thread_msgs,
        }))
    }

    /// Proactive assessment decision (`orchestrator_eval_decide`, `002-authorization.md:95-106`):
    /// In `observe` mode, a positive assessment records `"ignore"` with `would_accept: true`!
    pub fn eval_decide(&self, trigger_id: &str, payload: &Value) -> Result<Value, String> {
        self.mutate(|st| {
            let trig = st
                .triggers
                .get(trigger_id)
                .cloned()
                .ok_or_else(|| format!("Unknown trigger `{trigger_id}`"))?;
            let primary_th = trig
                .threads
                .first()
                .cloned()
                .unwrap_or_else(|| "github:general".into());
            let matched: Vec<String> = payload["matched_rules"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let reason = payload["reason"]
                .as_str()
                .unwrap_or("Proactive assessment completed")
                .trim();
            let wants_accept = payload["accept"].as_bool().unwrap_or(!matched.is_empty());
            let pver = st.policy.policy_version();

            if !wants_accept || matched.is_empty() {
                record_decision_in_state(
                    st,
                    &primary_th,
                    vec![trig.id.clone()],
                    "ignore",
                    false,
                    false,
                    reason,
                    matched,
                    None,
                    &pver,
                );
                return Ok(json!({ "state": "ignore", "reason": reason }));
            }

            if st.policy.accept_mode != AcceptMode::Auto {
                // `observe` mode: record would_accept without opening a Ticket (`000-philosophy.md`!)
                record_decision_in_state(
                    st,
                    &primary_th,
                    vec![trig.id.clone()],
                    "ignore",
                    true,
                    false,
                    &format!("[would accept in auto mode] {reason}"),
                    matched,
                    None,
                    &pver,
                );
                return Ok(json!({
                    "state": "ignore",
                    "would_accept": true,
                    "reason": format!("Observe mode recorded would-accept: {reason}"),
                }));
            }

            let title = payload["title"].as_str().unwrap_or(&trig.title).trim();
            let wt_name = payload["worktree_name"]
                .as_str()
                .unwrap_or("proactive-task")
                .trim();
            let ack = payload["acknowledgement"]
                .as_str()
                .unwrap_or("Investigating this issue under proactive repository policy.");
            let basis = MandateBasis {
                source: trig.source.clone(),
                scope: trig.scope.clone(),
                sender: trig.actor.clone(),
                via: "admission".into(),
            };
            let ticket = create_ticket_in_state(
                st,
                &trig,
                &primary_th,
                if title.is_empty() { "Proactive task" } else { title },
                wt_name,
                ack,
                None,
                Strength::Inferred,
                basis,
                matched,
                reason,
            );
            Ok(json!({
                "state": "accept",
                "reason": reason,
                "ticket_id": ticket.id,
                "ticket": ticket_card(&ticket),
            }))
        })
    }

    /// Worker `orchestrator_report` (`progress | ready | blocked | failed | dropped`).
    pub fn report_ticket(&self, ticket_id: &str, args: &Value) -> Result<Value, String> {
        let state_str = args["state"].as_str().unwrap_or("progress");
        if !matches!(
            state_str,
            "progress" | "ready" | "blocked" | "failed" | "dropped"
        ) {
            return Err(format!("Invalid report state `{state_str}`"));
        }
        let summary = args["summary"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("orchestrator_report requires non-empty `summary`")?;
        let pr_number = args.get("pr_number").and_then(Value::as_u64);
        let has_question_key = args.as_object().is_some_and(|o| o.contains_key("human_action"));
        let question_opt = args
            .get("human_action")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        self.mutate(|st| {
            let ticket = st
                .tickets
                .get_mut(ticket_id)
                .ok_or_else(|| format!("Unknown ticket `{ticket_id}`"))?;
            if ticket.ended.is_some() {
                return Err("Ticket already ended; do not resume it".into());
            }
            let now = now_iso();
            if let Some(pr) = pr_number {
                let pr_thread = format!("github:{pr}");
                if !ticket.follows.iter().any(|f| f.thread == pr_thread) {
                    ticket.follows.push(FollowedThread {
                        thread: pr_thread,
                        own: true,
                        pr_number: Some(pr),
                        closed_outcome: None,
                    });
                }
            }
            if has_question_key {
                ticket.attention = question_opt.clone();
            }
            // Mark delivered inputs as read on report:
            for inp in &mut ticket.inputs {
                if inp.delivered {
                    inp.read = true;
                }
            }
            let report = AssignmentReport {
                state: state_str.to_owned(),
                summary: summary.to_owned(),
                question: question_opt,
                pr_number,
                at: now.clone(),
            };
            if let Some(asg) = ticket.assignments.last_mut() {
                asg.reports.push(report);
                if matches!(state_str, "ready" | "dropped") {
                    asg.status = "ended".into();
                    asg.ended_at = Some(now.clone());
                }
            }
            if state_str == "dropped" {
                ticket.ended = Some(TicketEnd {
                    outcome: "dropped".into(),
                    by: "worker".into(),
                    reason: summary.to_owned(),
                    at: now.clone(),
                });
            } else if state_str == "ready" && ticket.follows.is_empty() {
                ticket.ended = Some(TicketEnd {
                    outcome: "completed".into(),
                    by: "worker".into(),
                    reason: summary.to_owned(),
                    at: now.clone(),
                });
            }
            ticket.updated_at = now;
            Ok(json!({
                "reported": true,
                "ticket": ticket_card(ticket),
            }))
        })
    }

    pub fn stop_ticket_by_person(&self, ticket_id: &str, reason: Option<&str>) -> Result<Value, String> {
        self.mutate(|st| {
            let ticket = st
                .tickets
                .get_mut(ticket_id)
                .ok_or_else(|| format!("Unknown ticket `{ticket_id}`"))?;
            ticket.mandate.withdrawn = true;
            let now = now_iso();
            if let Some(asg) = ticket.assignments.last_mut() {
                asg.status = "ended".into();
                asg.ended_at = Some(now.clone());
            }
            ticket.ended = Some(TicketEnd {
                outcome: "stopped".into(),
                by: "operator".into(),
                reason: reason.unwrap_or("Stopped from Superagent panel").to_owned(),
                at: now.clone(),
            });
            ticket.updated_at = now;
            Ok(ticket_card(ticket))
        })
    }

    pub fn retry_ticket(&self, ticket_id: &str) -> Result<Value, String> {
        self.mutate(|st| {
            let ticket = st
                .tickets
                .get_mut(ticket_id)
                .ok_or_else(|| format!("Unknown ticket `{ticket_id}`"))?;
            if !ticket.can_retry() {
                return Err(format!(
                    "Ticket `{ticket_id}` is in stage `{}` and does not need retry",
                    ticket.stage().code()
                ));
            }
            let now = now_iso();
            if let Some(asg) = ticket.assignments.last_mut() {
                asg.status = "ended".into();
                asg.ended_at = Some(now.clone());
            }
            ticket.delivery_error = None;
            ticket.assignments.push(Assignment {
                id: format!("asg_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]),
                session_id: ticket.worker_session_id.clone(),
                directory: ticket.directory.clone(),
                status: "pending".into(),
                started_at: now.clone(),
                ended_at: None,
                reports: Vec::new(),
            });
            ticket.updated_at = now;
            Ok(ticket_card(ticket))
        })
    }

    /// Scoped `orchestrator_read` resource reader (`007-architecture.md:550-578`).
    /// When `worker_ticket_scope` is `Some(tid)`, strictly confines reads to that worker's own Ticket!
    pub fn read_resource(
        &self,
        resource: &str,
        offset: usize,
        limit: usize,
        worker_ticket_scope: Option<&str>,
    ) -> Result<Value, String> {
        let st = self.state.read();
        let lim = limit.clamp(1, 50);
        let res = resource.trim().trim_start_matches('/');

        if let Some(own_tid) = worker_ticket_scope {
            if matches!(res, "tickets" | "decisions" | "settings") || res.starts_with("decision/") {
                return Err("Executor sessions may only read their own ticket resources.".into());
            }
            if let Some(rest) = res.strip_prefix("ticket/") {
                let target_tid = rest.split('/').next().unwrap_or("");
                if target_tid != own_tid {
                    return Err(format!(
                        "Access confined: worker `{own_tid}` cannot read ticket `{target_tid}`."
                    ));
                }
            }
        }

        match res {
            "tickets" => {
                let mut items: Vec<Value> = st.tickets.values().rev().map(ticket_card).collect();
                let next = (offset + lim < items.len()).then_some(offset + lim);
                items = items.into_iter().skip(offset).take(lim).collect();
                Ok(json!({ "items": items, "next_offset": next }))
            }
            "triggers" => {
                let mut items: Vec<&Trigger> = st.triggers.values().rev().collect();
                let next = (offset + lim < items.len()).then_some(offset + lim);
                items = items.into_iter().skip(offset).take(lim).collect();
                Ok(json!({ "items": items, "next_offset": next }))
            }
            "threads" => {
                let mut items: Vec<&ThreadRecord> = st.threads.values().rev().collect();
                let next = (offset + lim < items.len()).then_some(offset + lim);
                items = items.into_iter().skip(offset).take(lim).collect();
                Ok(json!({ "items": items, "next_offset": next }))
            }
            "decisions" => {
                let mut items: Vec<&DecisionRecord> = st.decisions.values().rev().collect();
                let next = (offset + lim < items.len()).then_some(offset + lim);
                items = items.into_iter().skip(offset).take(lim).collect();
                Ok(json!({ "items": items, "next_offset": next }))
            }
            "settings" => Ok(json!({
                "revision": st.settings_revision,
                "paused": st.paused,
                "policyVersion": st.policy.policy_version(),
                "policy": st.policy,
                "github": st.github,
                "feishu": st.feishu,
                "executor": st.executor,
            })),
            "prompts" | "prompts/current" => Ok(json!({
                "revision": st.prompt_revision,
                "config": st.prompts,
                "orchestratorPreview": st.prompts.compose_role_prompt("orchestrator", &st.policy, &[]),
                "workerPreview": st.prompts.compose_role_prompt("worker", &st.policy, &[]),
            })),
            "prompts/defaults" => Ok(json!(PromptConfig::default())),
            _ if res.starts_with("ticket/") => {
                let tid = res.trim_start_matches("ticket/").split('/').next().unwrap_or("");
                let ticket = st
                    .tickets
                    .get(tid)
                    .ok_or_else(|| format!("Unknown ticket `{tid}`"))?;
                Ok(json!(ticket))
            }
            _ if res.starts_with("thread/") => {
                let key = res.trim_start_matches("thread/");
                let th = st
                    .threads
                    .get(key)
                    .ok_or_else(|| format!("Unknown thread `{key}`"))?;
                Ok(json!(th))
            }
            _ if res.starts_with("trigger/") => {
                let id = res.trim_start_matches("trigger/");
                let tr = st
                    .triggers
                    .get(id)
                    .ok_or_else(|| format!("Unknown trigger `{id}`"))?;
                Ok(json!(tr))
            }
            _ if res.starts_with("decision/") => {
                let id = res.trim_start_matches("decision/");
                let dec = st
                    .decisions
                    .get(id)
                    .ok_or_else(|| format!("Unknown decision `{id}`"))?;
                Ok(json!(dec))
            }
            other => Err(format!("Unknown superagent resource `{other}`")),
        }
    }

    /// Polls configured GitHub repository via scrubbed `gh api` (`dsh-superagent/github-watch`).
    pub fn poll_github_now(&self) -> Value {
        let cfg = self.state.read().github.clone();
        if !cfg.enabled || cfg.repository.trim().is_empty() {
            return json!({ "polled": false, "reason": "GitHub source is disabled or repository is unset." });
        }
        let repo = cfg.repository.trim();
        let mut cmd = Command::new("gh");
        cmd.args([
            "api",
            &format!("repos/{repo}/issues?state=open&per_page=15"),
        ])
        .env("GH_PROMPT_DISABLED", "1");

        let out = match cmd.output() {
            Ok(o) => o,
            Err(e) => {
                let msg = format!("Failed to run `gh api`: {e}");
                self.mutate(|st| st.last_github_error = Some(msg.clone()));
                return json!({ "polled": false, "error": msg });
            }
        };
        if !out.status.success() {
            let msg = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            self.mutate(|st| st.last_github_error = Some(msg.clone()));
            return json!({ "polled": false, "error": msg });
        }

        let items: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let mut ingested = 0usize;
        for item in items {
            let num = item["number"].as_u64().unwrap_or(0);
            if num == 0 {
                continue;
            }
            let is_pr = item.get("pull_request").is_some();
            if is_pr && !cfg.receive_all_pulls {
                // Still ingest if body mentions agent_account
                let body = item["body"].as_str().unwrap_or("");
                if cfg.agent_account.is_empty() || !body.contains(&format!("@{}", cfg.agent_account)) {
                    continue;
                }
            }
            if !is_pr && !cfg.receive_all_issues {
                let body = item["body"].as_str().unwrap_or("");
                if cfg.agent_account.is_empty() || !body.contains(&format!("@{}", cfg.agent_account)) {
                    continue;
                }
            }
            let gh_id = format!("gh:{repo}#{num}:{}", item["updated_at"].as_str().unwrap_or(""));
            if self.state.read().seen_github_ids.contains(&gh_id) {
                continue;
            }
            self.mutate(|st| {
                st.seen_github_ids.insert(gh_id.clone());
                st.last_github_error = None;
            });
            let body_text = item["body"].as_str().unwrap_or("");
            let mentioned = !cfg.agent_account.is_empty()
                && body_text.contains(&format!("@{}", cfg.agent_account));
            let actor = item["user"]["login"].as_str().unwrap_or("unknown").to_owned();
            let is_bot = item["user"]["type"].as_str() == Some("Bot") || actor.ends_with("[bot]");
            let _ = self.receive_trigger(Trigger {
                id: gh_id,
                source: "github".into(),
                scope: if is_pr { "pulls".into() } else { "issues".into() },
                actor,
                is_bot,
                addressed: if mentioned { "mention".into() } else { "none".into() },
                threads: vec![format!("github:{num}")],
                title: format!("#{num} {}", item["title"].as_str().unwrap_or("")),
                text: body_text.to_owned(),
                url: item["html_url"].as_str().map(str::to_owned),
                at: now_iso(),
                meta: json!({ "number": num, "isPullRequest": is_pr }),
            });
            ingested += 1;
        }
        json!({ "polled": true, "ingested": ingested })
    }

    /// Complete snapshot for the 6-tab **Superagent** UI panel (`Overview`, `Source`, `Trigger`, `Eval`, `Execution`, `Settings`).
    pub fn panel_snapshot(&self) -> Value {
        let st = self.state.read();
        let tickets_all: Vec<Value> = st.tickets.values().rev().map(|t| {
            let mut c = ticket_card(t);
            c["canRetry"] = json!(t.can_retry());
            c["mandate"] = json!(t.mandate);
            c["inputsCount"] = json!(t.inputs.len());
            c["assignments"] = json!(t.assignments);
            c["follows"] = json!(t.follows);
            c
        }).collect();

        let person_tickets: Vec<Value> = st
            .tickets
            .values()
            .filter(|t| t.stage().needs_person())
            .map(|t| {
                let mut c = ticket_card(t);
                c["canRetry"] = json!(t.can_retry());
                c
            })
            .collect();

        let running_workers = st
            .tickets
            .values()
            .filter(|t| t.stage() == TicketStage::Working)
            .count();
        let queued_workers = st
            .tickets
            .values()
            .filter(|t| t.stage() == TicketStage::WorkingQueued)
            .count();
        let idle_workers = st
            .tickets
            .values()
            .filter(|t| t.stage() == TicketStage::Idle)
            .count();

        let mut problems = Vec::new();
        if st.paused {
            problems.push(json!({
                "kind": "executor-paused",
                "title": "Processing is paused",
                "detail": "New Evals, Ticket launches, and outgoing Channel replies are held until resumed.",
            }));
        }
        if !st.github.enabled && !st.feishu.enabled {
            problems.push(json!({
                "kind": "no-source",
                "title": "No external Source enabled yet",
                "detail": "Configure GitHub repository watch in Settings -> Sources, or use Submit Trigger to test locally.",
            }));
        } else if st.github.enabled
            && st
                .policy
                .sources
                .get("github")
                .is_none_or(|p| p.allowed.is_empty())
        {
            problems.push(json!({
                "kind": "no-allowed",
                "title": "Allowed sender list is empty",
                "detail": "Add at least one GitHub username to Policy -> Allowed Senders so explicit requests can be accepted.",
            }));
        }
        if let Some(ref gh_err) = st.last_github_error {
            problems.push(json!({
                "kind": "source-error",
                "title": "GitHub Source poll error",
                "detail": gh_err,
            }));
        }
        for dec in st.decisions.values() {
            if dec.kind == "unavailable" && !dec.dismissed {
                problems.push(json!({
                    "kind": "eval-unavailable",
                    "decisionId": dec.id,
                    "thread": dec.thread,
                    "title": format!("Unavailable Eval on {}", dec.thread),
                    "detail": dec.reason,
                }));
            }
        }

        let filtered_count = st
            .dispositions
            .values()
            .filter(|d| d.filtered.is_some())
            .count();
        let accepted_count = st.decisions.values().filter(|d| d.kind == "accept").count();
        let dropped_replies = st.outbox.values().filter(|r| r.status == "dropped").count();

        json!({
            "paused": st.paused,
            "masterId": st.master_session_id,
            "policyVersion": st.policy.policy_version(),
            "settingsRevision": st.settings_revision,
            "promptRevision": st.prompt_revision,
            "attentionCount": person_tickets.len(),
            "overview": {
                "person": person_tickets,
                "problems": problems,
                "flow": {
                    "received": st.triggers.len(),
                    "filtered": filtered_count,
                    "judged": st.decisions.len(),
                    "accepted": accepted_count,
                    "dropped": dropped_replies,
                },
                "executor": {
                    "running": running_workers,
                    "maxWorkers": st.executor.max_workers,
                    "queued": queued_workers,
                    "idle": idle_workers,
                }
            },
            "sources": {
                "github": st.github,
                "feishu": st.feishu,
                "lastGithubError": st.last_github_error,
            },
            "threads": st.threads.values().rev().cloned().collect::<Vec<_>>(),
            "triggers": st.triggers.values().rev().take(100).map(|t| {
                let disp = st.dispositions.get(&t.id);
                json!({
                    "trigger": t,
                    "disposition": disp,
                })
            }).collect::<Vec<_>>(),
            "decisions": st.decisions.values().rev().cloned().collect::<Vec<_>>(),
            "tickets": tickets_all,
            "settings": {
                "policy": st.policy,
                "github": st.github,
                "feishu": st.feishu,
                "executor": st.executor,
                "history": st.settings_history,
            },
            "prompts": {
                "revision": st.prompt_revision,
                "config": st.prompts,
                "history": st.prompt_history,
                "orchestratorFull": st.prompts.compose_role_prompt("orchestrator", &st.policy, &[]),
                "workerFull": st.prompts.compose_role_prompt("worker", &st.policy, &[]),
            }
        })
    }

    pub fn search(&self, query: &str) -> Value {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return json!({ "tickets": [], "threads": [], "items": [] });
        }
        let st = self.state.read();
        let tickets: Vec<Value> = st
            .tickets
            .values()
            .filter(|t| {
                t.title.to_lowercase().contains(&q)
                    || t.id.to_lowercase().contains(&q)
                    || t.short_name.to_lowercase().contains(&q)
                    || t.origin_thread.to_lowercase().contains(&q)
            })
            .take(20)
            .map(ticket_card)
            .collect();
        let threads: Vec<Value> = st
            .threads
            .values()
            .filter(|th| th.key.to_lowercase().contains(&q) || th.title.to_lowercase().contains(&q))
            .take(20)
            .map(|th| json!({ "key": th.key, "title": th.title }))
            .collect();
        let items: Vec<Value> = st
            .triggers
            .values()
            .filter(|tr| {
                tr.text.to_lowercase().contains(&q)
                    || tr.title.to_lowercase().contains(&q)
                    || tr.id.to_lowercase().contains(&q)
                    || tr.actor.to_lowercase().contains(&q)
            })
            .take(20)
            .map(|tr| {
                json!({
                    "id": tr.id,
                    "source": tr.source,
                    "actor": tr.actor,
                    "preview": tr.text.chars().take(140).collect::<String>(),
                    "threads": tr.threads,
                    "at": tr.at,
                })
            })
            .collect();
        json!({ "tickets": tickets, "threads": threads, "items": items })
    }

    pub fn save_settings_block(&self, block: &str, value: &Value, origin: &str) -> Result<Value, String> {
        self.mutate(|st| {
            match block {
                "pause" => {
                    st.paused = value["paused"].as_bool().unwrap_or(false);
                }
                "policy" => {
                    st.policy = serde_json::from_value(value.clone())
                        .map_err(|e| format!("Invalid policy block: {e}"))?;
                }
                "github" | "source:github" => {
                    st.github = serde_json::from_value(value.clone())
                        .map_err(|e| format!("Invalid GitHub source config: {e}"))?;
                }
                "feishu" | "source:feishu" => {
                    st.feishu = serde_json::from_value(value.clone())
                        .map_err(|e| format!("Invalid Feishu source config: {e}"))?;
                }
                "executor" => {
                    st.executor = serde_json::from_value(value.clone())
                        .map_err(|e| format!("Invalid executor config: {e}"))?;
                }
                other => return Err(format!("Unknown settings block `{other}`")),
            }
            st.settings_revision += 1;
            let pver = st.policy.policy_version();
            st.settings_history.push(SettingsSnapshotRecord {
                revision: st.settings_revision,
                block: block.to_owned(),
                origin: origin.to_owned(),
                policy_version: pver.clone(),
                at: now_iso(),
            });
            Ok(json!({
                "ok": true,
                "revision": st.settings_revision,
                "policyVersion": pver,
                "paused": st.paused,
            }))
        })
    }

    pub fn save_prompts(&self, cfg: PromptConfig, origin: &str) -> Value {
        self.mutate(|st| {
            st.prompt_revision += 1;
            let rev = st.prompt_revision;
            st.prompts = cfg.clone();
            st.prompt_history.push(PromptVersionRecord {
                revision: rev,
                origin: origin.to_owned(),
                at: now_iso(),
                config: cfg,
            });
            json!({ "ok": true, "revision": rev })
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn create_ticket_in_state(
    st: &mut SuperagentPersistedState,
    trig: &Trigger,
    primary_th: &str,
    title: &str,
    wt_name: &str,
    ack: &str,
    note: Option<&str>,
    strength: Strength,
    basis: MandateBasis,
    matched_rules: Vec<String>,
    reason: &str,
) -> Ticket {
    let tid = format!("tkt_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]);
    let safe_short: String = wt_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(40)
        .collect();
    let short_name = if safe_short.is_empty() {
        tid.clone()
    } else {
        safe_short
    };
    let dir = Path::new(&st.executor.work_root)
        .join(&short_name)
        .to_string_lossy()
        .into_owned();
    let _ = fs::create_dir_all(&dir);
    let worker_sid = format!("local_worker_{tid}");
    let pver = st.policy.policy_version();
    let now = now_iso();

    let asg = Assignment {
        id: format!("asg_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]),
        session_id: worker_sid.clone(),
        directory: dir.clone(),
        status: "pending".into(),
        started_at: now.clone(),
        ended_at: None,
        reports: Vec::new(),
    };

    let ticket = Ticket {
        id: tid.clone(),
        short_name,
        title: title.chars().take(200).collect(),
        origin_trigger_id: trig.id.clone(),
        origin_thread: primary_th.to_owned(),
        mandate: Mandate {
            policy_version: pver.clone(),
            strength,
            scope: title.to_owned(),
            matched_rules: matched_rules.clone(),
            basis,
            withdrawn: false,
        },
        routing_note: note.map(str::to_owned),
        directory: dir,
        worker_session_id: worker_sid,
        channels: vec![primary_th.to_owned()],
        follows: Vec::new(),
        inputs: vec![TicketInput {
            id: format!("inp_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]),
            kind: "trigger".into(),
            trigger_id: Some(trig.id.clone()),
            from_person: !trig.is_bot,
            text: trig.text.clone(),
            delivered: false,
            read: false,
            at: now.clone(),
        }],
        assignments: vec![asg],
        attention: None,
        delivery_error: None,
        ended: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    // Queue acknowledgement in Outbox (`001-domain.md`):
    let out_id = format!("out_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]);
    st.outbox.insert(
        out_id.clone(),
        OutboxReceipt {
            id: out_id,
            ticket_id: Some(tid.clone()),
            source: trig.source.clone(),
            channel: primary_th.to_owned(),
            purpose: "acknowledgement".into(),
            text: ack.to_owned(),
            status: "sent".into(),
            attempts: 1,
            last_error: None,
            dropped_reason: None,
            at: now,
        },
    );

    if let Some(disp) = st.dispositions.get_mut(&trig.id) {
        disp.acked_by.push(tid.clone());
    }
    if let Some(th) = st.threads.get_mut(primary_th) {
        if !th.subscriptions.contains(&tid) {
            th.subscriptions.push(tid.clone());
        }
    }

    record_decision_in_state(
        st,
        primary_th,
        vec![trig.id.clone()],
        "accept",
        false,
        false,
        reason,
        matched_rules,
        Some(tid.clone()),
        &pver,
    );
    st.tickets.insert(tid, ticket.clone());
    ticket
}

#[allow(clippy::too_many_arguments)]
fn record_decision_in_state(
    st: &mut SuperagentPersistedState,
    thread_key: &str,
    trigger_ids: Vec<String>,
    kind: &str,
    would_accept: bool,
    ignored_from_card: bool,
    reason: &str,
    matched_rules: Vec<String>,
    ticket_id: Option<String>,
    policy_version: &str,
) {
    let dec_id = format!("dec_{}", &Uuid::new_v4().to_string().replace('-', "")[..10]);
    let rec = DecisionRecord {
        id: dec_id.clone(),
        thread: thread_key.to_owned(),
        trigger_ids: trigger_ids.clone(),
        kind: kind.to_owned(),
        would_accept,
        ignored_from_card,
        reason: reason.to_owned(),
        matched_rules,
        ticket_id,
        policy_version: policy_version.to_owned(),
        dismissed: false,
        at: now_iso(),
    };
    for tid in &trigger_ids {
        if let Some(d) = st.dispositions.get_mut(tid) {
            d.decision_id = Some(dec_id.clone());
        }
    }
    if let Some(th) = st.threads.get_mut(thread_key) {
        th.running_eval = None;
        th.decision_ids.push(dec_id.clone());
    }
    st.decisions.insert(dec_id, rec);
}
