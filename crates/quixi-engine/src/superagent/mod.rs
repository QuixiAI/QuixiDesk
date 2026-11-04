//! `dsh-superagent` Cordis bundle integration: registers `SuperagentPlugin`,
//! the `dsh-superagent-router` and `dsh-superagent-worker` presets, the `noWait` guard,
//! and all Main Session (`orchestrator_*`) and Executor Session tools!

pub mod domain;
pub mod engine;

pub use domain::*;
pub use engine::{SuperagentService, SUPERAGENT_SERVICE};

use crate::cordis_host::seams::{
    AgentPresetSpec, PromptSectionSpec, SectionOrder, ToolActivationCtx, ToolExecutorKind,
    ToolRegistration, AGENT_PRESETS_SERVICE, SYSTEM_PROMPT_SERVICE, TOOLS_SERVICE,
};
use quixi_cordis::{
    BundleManifest, Context, CordisError, Plugin, PluginRowSpec, ScopeKey, ToolFilter,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SuperagentPlugin {
    pub data_dir: PathBuf,
}

impl Plugin for SuperagentPlugin {
    fn name(&self) -> &'static str {
        "dsh-superagent"
    }
    fn inject(&self) -> &'static [&'static str] {
        &["tools", "systemPrompt", "agentPresets"]
    }
    fn provide(&self) -> &'static [&'static str] {
        &["superagent"]
    }
    fn apply(&self, ctx: &Context, _cfg: &Value) -> Result<(), CordisError> {
        let svc = Arc::new(SuperagentService::open(&self.data_dir));
        let tools = ctx.get(TOOLS_SERVICE)?;
        let prompt = ctx.get(SYSTEM_PROMPT_SERVICE)?;
        let presets = ctx.get(AGENT_PRESETS_SERVICE)?;

        // 1. Register `dsh-superagent-router` preset (Main Orchestrator Session + `noWait` guard!):
        let router_ctx = ctx.scoped(ScopeKey::preset("dsh-superagent-router"));
        tools.restrict(
            &router_ctx,
            ToolFilter::allow_only([
                "Read",
                "ListDirectory",
                "Bash",
                "JobList",
                "JobOutput",
                "JobKill",
                "ListSkills",
                "LoadSkill",
            ]),
        )?;
        // `noWait` monotonic guard (`007-architecture.md:512`):
        tools.guard(
            &router_ctx,
            Arc::new(|tool_name, args, _act| {
                if tool_name == "Bash" {
                    let cmd = args["command"].as_str().unwrap_or("").trim();
                    if cmd.starts_with("sleep ") || cmd.contains("; sleep ") || cmd.contains("&& sleep ") {
                        return Some("Refused by `dsh-superagent` router `noWait` guard: never wait with `sleep` in the main Session; reports and candidates arrive automatically.".into());
                    }
                    if args["timeout_seconds"].as_u64().unwrap_or(5) > 30 {
                        return Some("Refused by `dsh-superagent` router `noWait` guard: shell timeout_seconds in main Session must be <= 30.".into());
                    }
                }
                if tool_name == "JobOutput" && args["wait_seconds"].as_u64().unwrap_or(0) > 5 {
                    return Some("Refused by `dsh-superagent` router `noWait` guard: JobOutput wait_seconds in main Session must be <= 5.".into());
                }
                None
            }),
        )?;

        let svc_for_router_prompt = svc.clone();
        prompt.register_section(
            &router_ctx,
            PromptSectionSpec {
                name: "superagent:router-complete".into(),
                order: 0,
                text: crate::cordis_host::seams::TextContribution::Dynamic(Arc::new(move |_| {
                    let st = svc_for_router_prompt.state.read();
                    let env_lines = vec![format!(
                        "DSH_SUPERAGENT_GITHUB_REPOSITORY={}",
                        st.github.repository
                    )];
                    st.prompts
                        .compose_role_prompt("orchestrator", &st.policy, &env_lines)
                })),
                interpolate: false,
                complete: true,
            },
        )?;

        presets.register(
            ctx,
            AgentPresetSpec {
                id: "dsh-superagent-router".into(),
                title: "Superagent Main Session".into(),
                badge: "Orchestrator".into(),
                description: "Dedicated preset for dsh-superagent's Main Orchestrator Session: assesses Triggers, routes Tickets, enforces `noWait`, and sends Channel replies.".into(),
                order: 91,
                builtin: true,
                requires_coding_tools: true,
                executor: ToolExecutorKind::Native,
                guide_markdown: "### Superagent Main Session (`dsh-superagent-router`)\nRoutes incoming Triggers into Tickets using `orchestrator_route` and assesses proactive candidates with `orchestrator_eval` / `orchestrator_eval_decide`.".into(),
                composition_yaml: "preset: dsh-superagent-router\ncompletePrompt: true\nguards:\n  - noWait\n".into(),
            },
        )?;

        // 2. Register `dsh-superagent-worker` preset (Executor Sessions `worker-<ticket_id>`):
        let worker_ctx = ctx.scoped(ScopeKey::preset("dsh-superagent-worker"));
        tools.restrict(
            &worker_ctx,
            ToolFilter::deny_names([
                "AskUserQuestion",
                "Present",
                "ExitPlanMode",
                "PluginManager",
                "ProposeAction",
                "CreateGoal",
                "UpdateGoal",
            ]),
        )?;
        let svc_for_worker_prompt = svc.clone();
        prompt.register_section(
            &worker_ctx,
            PromptSectionSpec {
                name: "superagent:worker-guidance".into(),
                order: SectionOrder::DEPLOYMENT_PERSONA_SUFFIX,
                text: crate::cordis_host::seams::TextContribution::Dynamic(Arc::new(move |inp| {
                    let st = svc_for_worker_prompt.state.read();
                    let ticket_opt = st
                        .tickets
                        .values()
                        .find(|t| t.worker_session_id == inp.session_id);
                    let mut env_lines = vec![format!(
                        "DSH_SUPERAGENT_GITHUB_REPOSITORY={}",
                        st.github.repository
                    )];
                    if let Some(t) = ticket_opt {
                        env_lines.push(format!("DSH_SUPERAGENT_TICKET={}", t.id));
                        env_lines.push(format!("DSH_SUPERAGENT_NAME={}", t.short_name));
                        env_lines.push(format!("DSH_SUPERAGENT_DIR={}", t.directory));
                    }
                    st.prompts
                        .compose_role_prompt("worker", &st.policy, &env_lines)
                })),
                interpolate: false,
                complete: false,
            },
        )?;

        presets.register(
            ctx,
            AgentPresetSpec {
                id: "dsh-superagent-worker".into(),
                title: "Superagent Executor".into(),
                badge: "Worker".into(),
                description: "Dedicated preset for dsh-superagent Executor Sessions (`worker-<ticket>`): unattended coding tools + `orchestrator_ack` / `orchestrator_report`.".into(),
                order: 90,
                builtin: true,
                requires_coding_tools: true,
                executor: ToolExecutorKind::Native,
                guide_markdown: "### Superagent Executor (`dsh-superagent-worker`)\nExecutes one authorized Ticket in an isolated work directory and reports outcomes via `orchestrator_report`.".into(),
                composition_yaml: "preset: dsh-superagent-worker\ndenyTools:\n  - AskUserQuestion\n  - Present\n  - ExitPlanMode\n  - PluginManager\n".into(),
            },
        )?;

        // 3. Register `orchestrator_*` tools admitted by preset role!
        register_superagent_tools(ctx, &tools, svc.clone())?;

        ctx.provide(SUPERAGENT_SERVICE, svc)
    }
}

fn register_superagent_tools(
    ctx: &Context,
    tools: &crate::cordis_host::seams::ToolsService,
    svc: Arc<SuperagentService>,
) -> Result<(), CordisError> {
    let is_router: crate::cordis_host::seams::ToolWhenPredicate =
        Arc::new(|act: &ToolActivationCtx| act.preset_id == "dsh-superagent-router");
    let is_worker: crate::cordis_host::seams::ToolWhenPredicate =
        Arc::new(|act: &ToolActivationCtx| act.preset_id == "dsh-superagent-worker");
    let is_either: crate::cordis_host::seams::ToolWhenPredicate = Arc::new(|act: &ToolActivationCtx| {
        matches!(
            act.preset_id.as_str(),
            "dsh-superagent-router" | "dsh-superagent-worker"
        )
    });

    // Shared `orchestrator_read` (automatically confines workers to their own ticket!):
    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_read".into(),
                description: "Read one page of a Superagent resource (`tickets`, `triggers`, `threads`, `decisions`, `ticket/<id>`, `thread/<key>`, `trigger/<id>`, `settings`, `prompts`).".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "resource": { "type": "string" },
                        "offset": { "type": "integer" },
                        "limit": { "type": "integer" }
                    },
                    "required": ["resource"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| true,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_either),
                custom_handler: Some(Arc::new(move |_, args, exec_ctx| {
                    let res_name = args["resource"].as_str().unwrap_or("");
                    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
                    let limit = args["limit"].as_u64().unwrap_or(20) as usize;
                    let worker_scope = {
                        let st = s.state.read();
                        st.tickets
                            .values()
                            .find(|t| t.worker_session_id == exec_ctx.session_id)
                            .map(|t| t.id.clone())
                    };
                    match s.read_resource(res_name, offset, limit, worker_scope.as_deref()) {
                        Ok(v) => (v.to_string(), false),
                        Err(e) => (e, true),
                    }
                })),
            },
        )?;
    }

    // Main Session (`router`) tools:
    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_route".into(),
                description: "Route 1-20 candidate triggers in order (`accept`, `attach`, `stop`, or `ignore`).".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "decisions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "kind": { "type": "string", "enum": ["accept", "attach", "stop", "ignore"] },
                                    "trigger_id": { "type": "string" },
                                    "ticket_id": { "type": "string" },
                                    "title": { "type": "string" },
                                    "worktree_name": { "type": "string" },
                                    "acknowledgement": { "type": "string" },
                                    "note": { "type": "string" },
                                    "reason": { "type": "string" }
                                },
                                "required": ["kind", "trigger_id"]
                            }
                        }
                    },
                    "required": ["decisions"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| false,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_router.clone()),
                custom_handler: Some(Arc::new(move |_, args, _| {
                    let items = args["decisions"].as_array().cloned().unwrap_or_default();
                    let out = s.route_decisions(&items);
                    let is_err = out.get("error").is_some();
                    (out.to_string(), is_err)
                })),
            },
        )?;
    }

    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_eval".into(),
                description: "Load bounded thread window, related tickets, and active policy rules for a proactive candidate.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": { "trigger_id": { "type": "string" } },
                    "required": ["trigger_id"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| true,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_router.clone()),
                custom_handler: Some(Arc::new(move |_, args, _| {
                    match s.eval_context(args["trigger_id"].as_str().unwrap_or("")) {
                        Ok(v) => (v.to_string(), false),
                        Err(e) => (e, true),
                    }
                })),
            },
        )?;
    }

    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_eval_decide".into(),
                description: "Record the decision of a proactive candidate assessment (`accept` vs `ignore`, respecting `observe` mode).".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "trigger_id": { "type": "string" },
                        "accept": { "type": "boolean" },
                        "matched_rules": { "type": "array", "items": { "type": "string" } },
                        "title": { "type": "string" },
                        "worktree_name": { "type": "string" },
                        "acknowledgement": { "type": "string" },
                        "reason": { "type": "string" }
                    },
                    "required": ["trigger_id", "accept", "reason"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| false,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_router.clone()),
                custom_handler: Some(Arc::new(move |_, args, _| {
                    match s.eval_decide(args["trigger_id"].as_str().unwrap_or(""), args) {
                        Ok(v) => (v.to_string(), false),
                        Err(e) => (e, true),
                    }
                })),
            },
        )?;
    }

    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_ticket_retry".into(),
                description: "Start a fresh assignment on a blocked, failed, or undelivered Ticket.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": { "ticket_id": { "type": "string" } },
                    "required": ["ticket_id"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| false,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_router.clone()),
                custom_handler: Some(Arc::new(move |_, args, _| {
                    match s.retry_ticket(args["ticket_id"].as_str().unwrap_or("")) {
                        Ok(v) => (v.to_string(), false),
                        Err(e) => (e, true),
                    }
                })),
            },
        )?;
    }

    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_ticket_note".into(),
                description: "Send a context-only note (<= 100 words, grants no authority) to a Ticket's worker.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "ticket_id": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["ticket_id", "text"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| false,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_router.clone()),
                custom_handler: Some(Arc::new(move |_, args, _| {
                    let tid = args["ticket_id"].as_str().unwrap_or("");
                    let txt = args["text"].as_str().unwrap_or("").trim();
                    if word_count(txt) > IM_WORD_LIMIT {
                        return (format!("Note exceeds {IM_WORD_LIMIT} words."), true);
                    }
                    s.mutate(|st| {
                        if let Some(t) = st.tickets.get_mut(tid) {
                            t.inputs.push(TicketInput {
                                id: format!("inp_{}", &uuid::Uuid::new_v4().to_string().replace('-', "")[..10]),
                                kind: "note".into(),
                                trigger_id: None,
                                from_person: false,
                                text: txt.to_owned(),
                                delivered: false,
                                read: false,
                                at: now_iso(),
                            });
                            (json!({ "noted": true, "ticket_id": tid }).to_string(), false)
                        } else {
                            (format!("Unknown ticket `{tid}`"), true)
                        }
                    })
                })),
            },
        )?;
    }

    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_reply".into(),
                description: "Queue an outgoing Channel reply (`answer`, `result`, or `attention`).".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "ticket_id": { "type": "string" },
                        "trigger_id": { "type": "string" },
                        "purpose": { "type": "string", "enum": ["answer", "result", "attention"] },
                        "human_action": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["purpose", "text"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| false,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_router),
                custom_handler: Some(Arc::new(move |_, args, _| {
                    let txt = args["text"].as_str().unwrap_or("").trim();
                    let purpose = args["purpose"].as_str().unwrap_or("answer");
                    let tid = args["ticket_id"].as_str().map(str::to_owned);
                    let id = format!("out_{}", &uuid::Uuid::new_v4().to_string().replace('-', "")[..10]);
                    s.mutate(|st| {
                        st.outbox.insert(
                            id.clone(),
                            OutboxReceipt {
                                id: id.clone(),
                                ticket_id: tid,
                                source: "github".into(),
                                channel: "github:reply".into(),
                                purpose: purpose.to_owned(),
                                text: txt.to_owned(),
                                status: "sent".into(),
                                attempts: 1,
                                last_error: None,
                                dropped_reason: None,
                                at: now_iso(),
                            },
                        );
                    });
                    (json!({ "status": "sent", "id": id }).to_string(), false)
                })),
            },
        )?;
    }

    // Worker (`dsh-superagent-worker`) tools:
    {
        let s = svc.clone();
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_report".into(),
                description: "Report Ticket execution state (`progress`, `ready`, `blocked`, `failed`, or `dropped`), optional `human_action` question, and optional `pr_number`.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "state": { "type": "string", "enum": ["progress", "ready", "blocked", "failed", "dropped"] },
                        "summary": { "type": "string" },
                        "human_action": { "type": "string" },
                        "pr_number": { "type": "integer" }
                    },
                    "required": ["state", "summary"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| false,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_worker.clone()),
                custom_handler: Some(Arc::new(move |_, args, exec_ctx| {
                    let tid_opt = {
                        let st = s.state.read();
                        st.tickets
                            .values()
                            .find(|t| t.worker_session_id == exec_ctx.session_id)
                            .map(|t| t.id.clone())
                    };
                    let Some(tid) = tid_opt else {
                        return ("orchestrator_report requires an active Superagent worker session.".into(), true);
                    };
                    match s.report_ticket(&tid, args) {
                        Ok(v) => (v.to_string(), false),
                        Err(e) => (e, true),
                    }
                })),
            },
        )?;
    }

    {
        let s = svc;
        tools.register(
            ctx,
            ToolRegistration {
                name: "orchestrator_ack".into(),
                description: "Acknowledge a human request concretely once (<= 100 words) before substantial work.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "trigger_id": { "type": "string" },
                        "text": { "type": "string" }
                    },
                    "required": ["trigger_id", "text"],
                    "additionalProperties": false
                }),
                is_concurrency_safe: |_| false,
                requires_workspace: false,
                timeout_ms: None,
                owner_plugin: "dsh-superagent".into(),
                when: Some(is_worker),
                custom_handler: Some(Arc::new(move |_, args, _| {
                    let txt = args["text"].as_str().unwrap_or("").trim();
                    if word_count(txt) > IM_WORD_LIMIT {
                        return (format!("Acknowledgement exceeds {IM_WORD_LIMIT} words."), true);
                    }
                    let trg_id = args["trigger_id"].as_str().unwrap_or("");
                    s.mutate(|st| {
                        if let Some(d) = st.dispositions.get_mut(trg_id) {
                            d.acked_by.push("worker".into());
                        }
                    });
                    (json!({ "trigger_id": trg_id, "acknowledgement": "sent" }).to_string(), false)
                })),
            },
        )?;
    }

    Ok(())
}

pub fn superagent_bundle_manifest() -> BundleManifest {
    BundleManifest {
        id: "dsh-superagent".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        title: "Superagent Proactive Repository Orchestrator".into(),
        description: "Proactive coding agent (`dsh-superagent`): Trigger -> Gate/Eval -> Mandate -> Ticket -> isolated Worker Executor Sessions, GitHub watch, and 6-tab supervision panel.".into(),
        category: "Orchestration".into(),
        official: true,
        experimental: true,
        listed_by_default: true,
        default_enabled: true,
        locked: false,
        source: Some("built-in".into()),
        readme_markdown: Some(
            "# dsh-superagent\n\n\
            **Take as little human attention as possible, and give as much value as possible.**\n\n\
            Listens to GitHub (and simulated/manual Triggers), evaluates candidates under a versioned Policy (`off`, `observe`, `auto`), grants scoped Mandates, runs isolated `worker-<ticket>` Executor Sessions, and surfaces only what needs a person in the 6-tab **Superagent** sidebar panel."
                .into(),
        ),
        client_script: None,
        declarative: Value::Null,
        rows: vec![PluginRowSpec {
            id: "dsh-superagent".into(),
            plugin: "dsh-superagent".into(),
            title: "Superagent Domain Engine, Presets & Panel".into(),
            description: "Mounts `superagent` service, `dsh-superagent-router` & `dsh-superagent-worker` presets, `orchestrator_*` tools, and Sidebar Superagent panel.".into(),
            disabled: false,
            locked: false,
            isolate: BTreeMap::new(),
            config: json!({}),
            config_schema: None,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superagent_lifecycle_gate_observe_vs_auto_and_ticket_stages_work() {
        let tmp = tempfile::tempdir().unwrap();
        let svc = SuperagentService::open(tmp.path());

        // 1. Add allowed sender "alice" on GitHub:
        svc.save_settings_block(
            "policy",
            &json!({
                "acceptMode": "observe",
                "evalTimeoutMs": 180000,
                "rules": [{
                    "id": "ci-fix",
                    "name": "Fix CI regression",
                    "instruction": "Investigate default-branch CI failure.",
                    "enabled": true
                }],
                "sources": {
                    "github": {
                        "allowed": [{ "id": "alice", "name": "Alice" }],
                        "bots": ["dependabot[bot]"],
                        "admission": {}
                    }
                }
            }),
            "test",
        )
        .unwrap();

        // 2. Explicit mention from allowed sender `alice` -> queued in Eval -> `orchestrator_route` accepts & opens Ticket!
        let r1 = svc.receive_trigger(Trigger {
            id: "gh-issue-10".into(),
            source: "github".into(),
            scope: "issues".into(),
            actor: "alice".into(),
            is_bot: false,
            addressed: "mention".into(),
            threads: vec!["github:10".into()],
            title: "#10 Fix flaky test".into(),
            text: "@agent please fix the flaky timer spec.".into(),
            url: Some("https://github.com/org/repo/issues/10".into()),
            at: String::new(),
            meta: json!({}),
        });
        assert_eq!(r1["disposition"], "stored");

        let routed = svc.route_decisions(&[json!({
            "kind": "accept",
            "trigger_id": "gh-issue-10",
            "title": "Fix flaky timer spec",
            "worktree_name": "fix-flaky-timer",
            "acknowledgement": "Investigating the flaky timer test and preparing a fix PR."
        })]);
        let ticket_id = routed["results"][0]["ticket"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(routed["results"][0]["ticket"]["stage"], "working/queued");

        // 3. Worker reports `blocked` with `human_action` -> stage becomes `person/answer`!
        svc.report_ticket(
            &ticket_id,
            &json!({
                "state": "progress",
                "summary": "Reproduced timer race; need confirmation on retry cap.",
                "human_action": "Should timer retries cap at 3 or 5?"
            }),
        )
        .unwrap();
        let snap1 = svc.panel_snapshot();
        assert_eq!(snap1["attentionCount"], 1);
        assert_eq!(snap1["overview"]["person"][0]["stage"], "person/answer");

        // 4. Proactive bot trigger in `observe` mode -> records `would_accept: true` WITHOUT creating a Ticket!
        svc.receive_trigger(Trigger {
            id: "gh-bot-1".into(),
            source: "github".into(),
            scope: "pulls".into(),
            actor: "dependabot[bot]".into(),
            is_bot: true,
            addressed: "none".into(),
            threads: vec!["github:11".into()],
            title: "#11 Bump deps".into(),
            text: "CI failed on bump".into(),
            url: None,
            at: String::new(),
            meta: json!({}),
        });
        let obs_dec = svc
            .eval_decide(
                "gh-bot-1",
                &json!({
                    "accept": true,
                    "matched_rules": ["ci-fix"],
                    "reason": "Matches CI fix rule"
                }),
            )
            .unwrap();
        assert_eq!(obs_dec["state"], "ignore");
        assert_eq!(obs_dec["would_accept"], true);
    }
}
