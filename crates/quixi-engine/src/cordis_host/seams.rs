//! Core Cordis capability seams (`SystemPromptService`, `ToolsService`, `AgentPresetRegistry`).
//!
//! Realizes the 3-role Capability Seam Triad (`Service Definition -> Provider -> Consumer`)
//! across `dsh-system-prompt`, `dsh-tools`, and `dsh-agent-preset-registry`.

use parking_lot::RwLock;
use quixi_cordis::{
    AnonymousEntries, Context, CordisError, NamedEntries, ScopeKey, ScopeLayer,
    ScopedLayers, ServiceKey, ToolFilter, TOOL_FILTER_INTERCEPTION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const SYSTEM_PROMPT_SERVICE: ServiceKey<SystemPromptService> =
    ServiceKey::new("systemPrompt", "Ordered prompt sections, variables, and cache-safe runtime context");
pub const TOOLS_SERVICE: ServiceKey<ToolsService> =
    ServiceKey::new("tools", "Scoped tool registry, step admission, guards, and native/PTC executors");
pub const AGENT_PRESETS_SERVICE: ServiceKey<AgentPresetRegistry> =
    ServiceKey::new("agentPresets", "Declarative per-session agent preset compositions");

/// Central section placement orders (`dsh-system-prompt` `SECTION_ORDERS`).
pub struct SectionOrder;
impl SectionOrder {
    pub const HARNESS_IDENTITY: i32 = -1000;
    pub const DEPLOYMENT_PERSONA_PREFIX: i32 = 0;
    pub const SAFETY_POLICY: i32 = 200;
    pub const PLAN_POLICY: i32 = 500;
    pub const TEAM_POLICY: i32 = 600;
    pub const PTC_ONLY: i32 = 800;
    pub const TOOL_BASH: i32 = 1000;
    pub const TOOL_FS: i32 = 1100;
    pub const TOOL_SEARCH: i32 = 1400;
    pub const TOOL_JOBS: i32 = 1600;
    pub const TOOL_TERMINAL: i32 = 1700;
    pub const TOOL_WEB: i32 = 2000;
    pub const TOOL_LSP: i32 = 2200;
    pub const TOOL_ORCHESTRATION: i32 = 2600;
    pub const TOOL_COLLABORATION: i32 = 2900;
    pub const TOOL_CORDIS: i32 = 3050;
    pub const MCP_SERVERS: i32 = 3100;
    pub const TOOLS_SDK: i32 = 5000;
    pub const PROJECT_INSTRUCTIONS: i32 = 7000;
    pub const WORKSPACE_INSTRUCTIONS: i32 = 7500;
    pub const ARTIFACT_GUIDANCE: i32 = 8000;
    pub const SKILLS_LIBRARY: i32 = 8500;
    pub const DELIVERABLE_FILE_REFERENCES: i32 = 9000;
    pub const DEPLOYMENT_PERSONA_SUFFIX: i32 = 10200;
}

/// Central runtime-context orders (`dsh-system-prompt` `CONTEXT_ORDERS`).
pub struct ContextOrder;
impl ContextOrder {
    pub const WORKING_DIRECTORY: i32 = 100;
    pub const SANDBOX_POLICY: i32 = 110;
    pub const APPROVAL_POLICY: i32 = 115;
    pub const SUBAGENT_DELEGATION: i32 = 120;
    pub const TIME_CONTEXT: i32 = 150;
}

#[derive(Clone)]
pub enum TextContribution {
    Static(String),
    Dynamic(Arc<dyn Fn(&PromptAssembleInput) -> String + Send + Sync>),
}

impl TextContribution {
    pub fn resolve(&self, input: &PromptAssembleInput) -> String {
        match self {
            Self::Static(s) => s.clone(),
            Self::Dynamic(f) => f(input),
        }
    }
}

#[derive(Clone)]
pub struct PromptSectionSpec {
    pub name: String,
    pub order: i32,
    pub text: TextContribution,
    pub interpolate: bool,
    pub complete: bool,
}

#[derive(Clone)]
pub struct PromptContextSpec {
    pub name: String,
    pub order: i32,
    pub text: TextContribution,
    pub interpolate: bool,
    pub required: bool,
}

#[derive(Clone)]
pub struct PromptAssembleInput {
    pub scope: Option<ScopeKey>,
    pub session_id: String,
    pub preset_id: String,
    pub model: String,
    pub provider: String,
    pub cwd: String,
    pub permission_mode: String,
    pub plan_active: bool,
    pub team_enabled: bool,
    pub tools_enabled: bool,
    pub project_instructions: String,
    pub workspace_instructions: String,
    pub mcp_capabilities: String,
    pub artifact_guidance: String,
    pub skills_catalog: String,
    pub sdk_declarations: String,
}

#[derive(Default)]
pub struct PromptLayer {
    pub sections: NamedEntries<PromptSectionSpec>,
    pub contexts: NamedEntries<PromptContextSpec>,
    pub variables: NamedEntries<Arc<dyn Fn(&PromptAssembleInput) -> String + Send + Sync>>,
    pub suppress_runtime_context: AnonymousEntries<bool>,
}

impl ScopeLayer for PromptLayer {
    fn is_empty(&self) -> bool {
        self.sections.is_empty()
            && self.contexts.is_empty()
            && self.variables.is_empty()
            && self.suppress_runtime_context.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembledPromptOutput {
    pub system_prompt: String,
    pub runtime_context_snapshot: String,
    pub section_names: Vec<String>,
}

pub struct SystemPromptService {
    layers: ScopedLayers<PromptLayer>,
}

impl Default for SystemPromptService {
    fn default() -> Self {
        Self::new(None)
    }
}

impl SystemPromptService {
    pub fn new(on_change: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
        Self {
            layers: ScopedLayers::new(on_change),
        }
    }

    pub fn register_section(&self, ctx: &Context, spec: PromptSectionSpec) -> Result<(), CordisError> {
        let scope = ctx.scope().cloned();
        let label = format!("systemPrompt.section({})", spec.name);
        let layers = self.layers.clone();
        ctx.effect(label, move |_| {
            let name = spec.name.clone();
            let (_, inv) = layers
                .mutate_revertible(scope, move |layer| {
                    let tok = layer.sections.insert(name.clone(), spec)?;
                    Ok((
                        (),
                        Box::new(move |l: &mut PromptLayer| l.sections.remove_matching(&name, tok))
                            as Box<dyn FnOnce(&mut PromptLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    pub fn register_context(&self, ctx: &Context, spec: PromptContextSpec) -> Result<(), CordisError> {
        let scope = ctx.scope().cloned();
        let label = format!("systemPrompt.context({})", spec.name);
        let layers = self.layers.clone();
        ctx.effect(label, move |_| {
            let name = spec.name.clone();
            let (_, inv) = layers
                .mutate_revertible(scope, move |layer| {
                    let tok = layer.contexts.insert(name.clone(), spec)?;
                    Ok((
                        (),
                        Box::new(move |l: &mut PromptLayer| l.contexts.remove_matching(&name, tok))
                            as Box<dyn FnOnce(&mut PromptLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    pub fn register_variable(
        &self,
        ctx: &Context,
        name: impl Into<String>,
        provider: impl Fn(&PromptAssembleInput) -> String + Send + Sync + 'static,
    ) -> Result<(), CordisError> {
        let name = name.into();
        let scope = ctx.scope().cloned();
        let label = format!("systemPrompt.variable({name})");
        let layers = self.layers.clone();
        let arc_fn: Arc<dyn Fn(&PromptAssembleInput) -> String + Send + Sync> = Arc::new(provider);
        ctx.effect(label, move |_| {
            let key = name.clone();
            let (_, inv) = layers
                .mutate_revertible(scope, move |layer| {
                    let tok = layer.variables.insert(key.clone(), arc_fn)?;
                    Ok((
                        (),
                        Box::new(move |l: &mut PromptLayer| l.variables.remove_matching(&key, tok))
                            as Box<dyn FnOnce(&mut PromptLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    pub fn suppress_runtime_context(&self, ctx: &Context) -> Result<(), CordisError> {
        let scope = ctx.scope().cloned();
        let layers = self.layers.clone();
        ctx.effect("systemPrompt.suppressRuntimeContext()", move |_| {
            let (_, inv) = layers
                .mutate_revertible(scope, move |layer| {
                    let tok = layer.suppress_runtime_context.append(true);
                    Ok((
                        (),
                        Box::new(move |l: &mut PromptLayer| l.suppress_runtime_context.remove(tok))
                            as Box<dyn FnOnce(&mut PromptLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    pub fn interpolate_strict(text: &str, vars: &BTreeMap<String, String>) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(open) = rest.find("{{") {
            out.push_str(&rest[..open]);
            let after_open = &rest[open + 2..];
            if let Some(close) = after_open.find("}}") {
                let candidate = &after_open[..close];
                let valid = !candidate.is_empty()
                    && candidate
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_lowercase())
                    && candidate
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                if valid {
                    if let Some(val) = vars.get(candidate) {
                        out.push_str(val);
                    } else {
                        out.push_str("{{");
                        out.push_str(candidate);
                        out.push_str("}}");
                    }
                    rest = &after_open[close + 2..];
                    continue;
                }
            }
            out.push_str("{{");
            rest = after_open;
        }
        out.push_str(rest);
        out
    }

    pub fn assemble(&self, ctx: &Context, input: &PromptAssembleInput) -> AssembledPromptOutput {
        let scope = input.scope.as_ref().or_else(|| ctx.scope());
        let suppressed = self.layers.with_chain(scope, |global, chain| {
            !global.suppress_runtime_context.is_empty()
                || chain.iter().any(|l| !l.suppress_runtime_context.is_empty())
        });

        let var_fns = self.layers.merge_named(scope, |l| &l.variables);
        let mut vars = BTreeMap::new();
        vars.insert("model".into(), input.model.clone());
        vars.insert("provider".into(), input.provider.clone());
        vars.insert("os".into(), std::env::consts::OS.to_owned());
        vars.insert(
            "shell".into(),
            quixi_workspace::ShellHostEngine::posix_bash_shell(),
        );
        for (k, f) in var_fns {
            vars.insert(k, f(input));
        }

        let section_map = self.layers.merge_named(scope, |l| &l.sections);
        let mut section_specs: Vec<PromptSectionSpec> = section_map.into_values().collect();
        section_specs.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));

        let complete_spec = section_specs.iter().find(|s| s.complete).cloned();

        let mut rendered_sections: Vec<(String, String)> = Vec::new();
        if let Some(comp) = complete_spec {
            let raw = comp.text.resolve(input);
            let txt = if comp.interpolate {
                Self::interpolate_strict(&raw, &vars)
            } else {
                raw
            };
            if !txt.trim().is_empty() {
                rendered_sections.push((comp.name, txt));
            }
        } else {
            for spec in &section_specs {
                let raw = spec.text.resolve(input);
                if raw.trim().is_empty() {
                    continue;
                }
                let txt = if spec.interpolate {
                    Self::interpolate_strict(&raw, &vars)
                } else {
                    raw
                };
                if !txt.trim().is_empty() {
                    rendered_sections.push((spec.name.clone(), txt));
                }
            }
        }

        let context_map = self.layers.merge_named(scope, |l| &l.contexts);
        let mut context_specs: Vec<PromptContextSpec> = context_map
            .into_values()
            .filter(|c| !suppressed || c.required)
            .collect();
        context_specs.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));

        let mut rendered_contexts = Vec::new();
        for cspec in &context_specs {
            let raw = cspec.text.resolve(input);
            if raw.trim().is_empty() {
                continue;
            }
            let txt = if cspec.interpolate {
                Self::interpolate_strict(&raw, &vars)
            } else {
                raw
            };
            if !txt.trim().is_empty() {
                rendered_contexts.push(txt);
            }
        }

        let initial_payload = json!({
            "sections": rendered_sections.iter().map(|(n, t)| json!({"name": n, "text": t})).collect::<Vec<_>>(),
            "contexts": rendered_contexts,
        });
        let transformed = ctx.waterfall("system-prompt/assemble", initial_payload, |v| v);

        let final_sections: Vec<(String, String)> = transformed["sections"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let n = item["name"].as_str()?.to_owned();
                        let t = item["text"].as_str()?.to_owned();
                        (!t.trim().is_empty()).then_some((n, t))
                    })
                    .collect()
            })
            .unwrap_or(rendered_sections);

        let section_names = final_sections.iter().map(|(n, _)| n.clone()).collect();
        let system_prompt = final_sections
            .into_iter()
            .map(|(_, t)| t)
            .collect::<Vec<_>>()
            .join("\n\n");

        let final_ctx_parts: Vec<String> = transformed["contexts"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .filter(|s| !s.trim().is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let runtime_context_snapshot = if final_ctx_parts.is_empty() {
            String::new()
        } else {
            format!(
                "[Runtime context update — supersedes earlier runtime snapshots]\n{}",
                final_ctx_parts.join("\n")
            )
        };

        AssembledPromptOutput {
            system_prompt,
            runtime_context_snapshot,
            section_names,
        }
    }
}

/// Step-time context passed to `ToolDefinition.when` predicates (`dsh-tools`).
#[derive(Debug, Clone)]
pub struct ToolActivationCtx {
    pub session_id: String,
    pub preset_id: String,
    pub model: String,
    pub cwd: String,
    pub permission_mode: String,
    pub plan_active: bool,
    pub team_enabled: bool,
    pub artifact_enabled: bool,
    pub web_search_ready: bool,
    pub creator_skills_loaded: bool,
}

#[derive(Clone)]
pub struct ToolExecContext<'a> {
    pub session_id: &'a str,
    pub call_id: &'a str,
    pub cwd: &'a str,
    pub permission_mode: &'a str,
    pub resolved_model: &'a str,
}

pub type CustomToolHandler =
    Arc<dyn Fn(&str, &Value, &ToolExecContext<'_>) -> (String, bool) + Send + Sync>;
pub type ToolWhenPredicate = Arc<dyn Fn(&ToolActivationCtx) -> bool + Send + Sync>;
pub type MonotonicToolGuard =
    Arc<dyn Fn(&str, &Value, &ToolActivationCtx) -> Option<String> + Send + Sync>;

#[derive(Clone)]
pub struct ToolRegistration {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub is_concurrency_safe: fn(&Value) -> bool,
    pub requires_workspace: bool,
    pub timeout_ms: Option<u64>,
    pub owner_plugin: String,
    pub when: Option<ToolWhenPredicate>,
    pub custom_handler: Option<CustomToolHandler>,
}

impl ToolRegistration {
    pub fn to_openai_schema(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolExecutorKind {
    #[default]
    Native,
    PtcRunCode,
}

#[derive(Default)]
pub struct ToolLayer {
    pub tools: NamedEntries<ToolRegistration>,
    pub filters: AnonymousEntries<ToolFilter>,
    pub guards: AnonymousEntries<MonotonicToolGuard>,
    pub executor: Option<ToolExecutorKind>,
}

impl ScopeLayer for ToolLayer {
    fn is_empty(&self) -> bool {
        self.tools.is_empty()
            && self.filters.is_empty()
            && self.guards.is_empty()
            && self.executor.is_none()
    }
}

#[derive(Clone)]
pub struct ResolvedStepTools {
    pub executor: ToolExecutorKind,
    /// Underlying admitted capability tools (even in PTC mode, where they become `tools.<Name>()`).
    pub capabilities: BTreeMap<String, ToolRegistration>,
    /// Model-visible OpenAI function schemas for this step (`RunCode` alone when in `PtcRunCode` mode!).
    pub model_schemas: Vec<Value>,
    /// Generated TypeScript SDK declaration block when `executor == PtcRunCode`.
    pub sdk_declarations: String,
}

pub struct ToolsService {
    layers: ScopedLayers<ToolLayer>,
}

impl Default for ToolsService {
    fn default() -> Self {
        Self::new(None)
    }
}

pub const RUN_CODE_TOOL_NAME: &str = "RunCode";

impl ToolsService {
    pub fn new(on_change: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
        Self {
            layers: ScopedLayers::new(on_change),
        }
    }

    pub fn register(&self, ctx: &Context, reg: ToolRegistration) -> Result<(), CordisError> {
        if reg.name == RUN_CODE_TOOL_NAME {
            return Err(CordisError::SetupFailed(format!(
                "tool name \"{RUN_CODE_TOOL_NAME}\" is reserved for the PTC executor transport"
            )));
        }
        let scope = ctx.scope().cloned();
        let label = format!("tools.register({})", reg.name);
        let layers = self.layers.clone();
        ctx.effect(label, move |_| {
            let name = reg.name.clone();
            let (_, inv) = layers
                .mutate_revertible(scope, move |layer| {
                    let tok = layer.tools.insert(name.clone(), reg)?;
                    Ok((
                        (),
                        Box::new(move |l: &mut ToolLayer| l.tools.remove_matching(&name, tok))
                            as Box<dyn FnOnce(&mut ToolLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    pub fn restrict(&self, ctx: &Context, filter: ToolFilter) -> Result<(), CordisError> {
        let Some(scope) = ctx.scope().cloned() else {
            return Err(CordisError::SetupFailed(
                "tools.restrict() requires a scoped context (Preset, Session, or Subagent)".into(),
            ));
        };
        let layers = self.layers.clone();
        ctx.effect("tools.restrict()", move |_| {
            let (_, inv) = layers
                .mutate_revertible(Some(scope), move |layer| {
                    let tok = layer.filters.append(filter);
                    Ok((
                        (),
                        Box::new(move |l: &mut ToolLayer| l.filters.remove(tok))
                            as Box<dyn FnOnce(&mut ToolLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    pub fn guard(&self, ctx: &Context, guard_fn: MonotonicToolGuard) -> Result<(), CordisError> {
        let scope = ctx.scope().cloned();
        let layers = self.layers.clone();
        ctx.effect("tools.guard()", move |_| {
            let (_, inv) = layers
                .mutate_revertible(scope, move |layer| {
                    let tok = layer.guards.append(guard_fn);
                    Ok((
                        (),
                        Box::new(move |l: &mut ToolLayer| l.guards.remove(tok))
                            as Box<dyn FnOnce(&mut ToolLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    pub fn set_executor(&self, ctx: &Context, kind: ToolExecutorKind) -> Result<(), CordisError> {
        let scope = ctx.scope().cloned();
        let layers = self.layers.clone();
        ctx.effect(format!("tools.setExecutor({kind:?})"), move |_| {
            let (_, inv) = layers
                .mutate_revertible(scope, move |layer| {
                    if layer.executor.is_some() {
                        return Err("Only one tool executor may be registered per scope".into());
                    }
                    layer.executor = Some(kind);
                    Ok((
                        (),
                        Box::new(move |l: &mut ToolLayer| {
                            l.executor = None;
                            true
                        }) as Box<dyn FnOnce(&mut ToolLayer) -> bool + Send>,
                    ))
                })
                .map_err(CordisError::SetupFailed)?;
            Ok(((), Box::new(move |_: &Context| inv())))
        })
    }

    /// Evaluates monotonic guards in Global -> Scope order (`dsh-tools` `checkGuards`).
    pub fn check_guards(
        &self,
        scope: Option<&ScopeKey>,
        tool_name: &str,
        args: &Value,
        act: &ToolActivationCtx,
    ) -> Option<String> {
        self.layers.with_chain(scope, |global, chain| {
            for g in global.guards.values() {
                if let Some(reason) = g(tool_name, args, act) {
                    return Some(reason);
                }
            }
            for layer in chain {
                for g in layer.guards.values() {
                    if let Some(reason) = g(tool_name, args, act) {
                        return Some(reason);
                    }
                }
            }
            None
        })
    }

    /// Resolves step-admitted tools, evaluating `when` predicates, scoped restrictions,
    /// `Context::intercept(TOOL_FILTER_INTERCEPTION)`, and Native vs. PTC `RunCode` projection!
    pub fn resolve_step(&self, ctx: &Context, act: &ToolActivationCtx) -> ResolvedStepTools {
        let scope = ctx.scope();
        let intercepted_filter = ctx.read_interception(TOOL_FILTER_INTERCEPTION);

        let (executor, merged) = self.layers.with_chain(scope, |global, chain| {
            let mut exec = global.executor.unwrap_or(ToolExecutorKind::Native);
            for l in chain {
                if let Some(k) = l.executor {
                    exec = k;
                }
            }

            let mut inherited = BTreeMap::new();
            for (k, v) in global.tools.iter() {
                inherited.insert(k.to_owned(), v.clone());
            }

            let mut visible = BTreeMap::new();
            if chain.is_empty() {
                for (name, reg) in inherited {
                    if global.filters.values().all(|f| f.admits(&name))
                        && intercepted_filter.admits(&name)
                    {
                        visible.insert(name, reg);
                    }
                }
            } else {
                let nearest_idx = chain.len() - 1;
                for (idx, layer) in chain.iter().enumerate() {
                    if idx < nearest_idx {
                        for (k, v) in layer.tools.iter() {
                            inherited.insert(k.to_owned(), v.clone());
                        }
                    }
                }
                for (name, reg) in inherited {
                    let admitted_by_chain = global.filters.values().all(|f| f.admits(&name))
                        && chain
                            .iter()
                            .all(|layer| layer.filters.values().all(|f| f.admits(&name)))
                        && intercepted_filter.admits(&name);
                    if admitted_by_chain {
                        visible.insert(name, reg);
                    }
                }
                // Viewing scope's own local registrations shadow inherited ones and are exempt from ancestor filters.
                for (k, v) in chain[nearest_idx].tools.iter() {
                    if intercepted_filter.admits(k) {
                        visible.insert(k.to_owned(), v.clone());
                    }
                }
            }
            (exec, visible)
        });

        let has_workspace = !act.cwd.trim().is_empty();
        let mut capabilities = BTreeMap::new();
        for (name, reg) in merged {
            if reg.requires_workspace && !has_workspace {
                continue;
            }
            if let Some(ref pred) = reg.when {
                if !pred(act) {
                    continue;
                }
            }
            capabilities.insert(name, reg);
        }

        match executor {
            ToolExecutorKind::Native => {
                let model_schemas = capabilities
                    .values()
                    .map(ToolRegistration::to_openai_schema)
                    .collect();
                ResolvedStepTools {
                    executor,
                    capabilities,
                    model_schemas,
                    sdk_declarations: String::new(),
                }
            }
            ToolExecutorKind::PtcRunCode => {
                let sdk_declarations = render_ptc_sdk(&capabilities);
                let run_code_schema = json!({
                    "type": "function",
                    "function": {
                        "name": RUN_CODE_TOOL_NAME,
                        "description": "Execute an async JavaScript/TypeScript program with direct programmatic bindings `await tools.<ToolName>(args)` over all admitted workspace, search, shell, web, and orchestration tools. Prefer batching multi-file reads, searches, and structured transformations in a single `RunCode` script.",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "description": {
                                    "type": "string",
                                    "description": "Concise 3-8 word summary of what this script does."
                                },
                                "code": {
                                    "type": "string",
                                    "description": "Async JS/TS function body (`await` and `return` supported; `console.log` captured). Call capabilities via `await tools.<Name>(args)`."
                                },
                                "timeout_seconds": {
                                    "type": "integer",
                                    "description": "Execution timeout in seconds (1-600, default 120)."
                                }
                            },
                            "required": ["description", "code"],
                            "additionalProperties": false
                        }
                    }
                });
                ResolvedStepTools {
                    executor,
                    capabilities,
                    model_schemas: vec![run_code_schema],
                    sdk_declarations,
                }
            }
        }
    }

    pub fn inspect_registered_tools(&self) -> Vec<Value> {
        self.layers.with_chain(None, |global, _| {
            global
                .tools
                .iter()
                .map(|(name, reg)| {
                    json!({
                        "name": name,
                        "description": reg.description,
                        "ownerPlugin": reg.owner_plugin,
                        "concurrencySafe": (reg.is_concurrency_safe)(&json!({})),
                        "requiresWorkspace": reg.requires_workspace,
                        "timeoutMs": reg.timeout_ms,
                        "conditional": reg.when.is_some(),
                    })
                })
                .collect()
        })
    }
}

fn render_ptc_sdk(capabilities: &BTreeMap<String, ToolRegistration>) -> String {
    let mut out = String::from(
        "Programmatic Tool Calling (`RunCode`) SDK — invoke tools ONLY via `RunCode` using `await tools.<Name>(args)`:\n```ts\ndeclare const tools: {\n",
    );
    for (name, reg) in capabilities {
        let props = reg
            .parameters
            .get("properties")
            .and_then(Value::as_object)
            .map(|m| {
                let req: BTreeSet<&str> = reg
                    .parameters
                    .get("required")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                m.iter()
                    .map(|(k, v)| {
                        let ty = match v.get("type").and_then(Value::as_str) {
                            Some("integer" | "number") => "number",
                            Some("boolean") => "boolean",
                            Some("array") => "unknown[]",
                            Some("object") => "Record<string, unknown>",
                            _ => "string",
                        };
                        let opt = if req.contains(k.as_str()) { "" } else { "?" };
                        format!("{k}{opt}: {ty}")
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        let short_desc: String = reg.description.chars().take(120).collect();
        out.push_str(&format!(
            "  /** {short_desc} */\n  {name}(args: {{ {props} }}): Promise<string>;\n"
        ));
    }
    out.push_str("};\n```");
    out
}

/// Declarative Agent Preset descriptor (`dsh-agent-preset-registry`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPresetSpec {
    pub id: String,
    pub title: String,
    pub badge: String,
    pub description: String,
    pub order: i32,
    pub builtin: bool,
    pub requires_coding_tools: bool,
    pub executor: ToolExecutorKind,
    pub guide_markdown: String,
    pub composition_yaml: String,
}

#[derive(Default)]
pub struct AgentPresetRegistry {
    presets: RwLock<BTreeMap<String, AgentPresetSpec>>,
}

impl AgentPresetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, ctx: &Context, spec: AgentPresetSpec) -> Result<(), CordisError> {
        let id = spec.id.clone();
        let label = format!("agentPresets.register({id})");
        self.presets.write().insert(id.clone(), spec);
        let reg_ptr = self as *const Self as usize;
        // Safe: registry lives inside Arc<AgentPresetRegistry> provided by the owning fiber;
        // dependent fibers drain before AgentPresetRegistry unloads (`Theorem 70`).
        ctx.effect(label, move |_| {
            Ok((
                (),
                Box::new(move |_: &Context| unsafe {
                    let reg = &*(reg_ptr as *const AgentPresetRegistry);
                    reg.presets.write().remove(&id);
                }),
            ))
        })
    }

    pub fn list(&self, default_id: &str) -> Vec<Value> {
        let mut items: Vec<AgentPresetSpec> = self.presets.read().values().cloned().collect();
        items.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        items
            .into_iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "title": p.title,
                    "badge": p.badge,
                    "description": p.description,
                    "order": p.order,
                    "builtin": p.builtin,
                    "requiresCodingTools": p.requires_coding_tools,
                    "executor": p.executor,
                    "isDefault": p.id == default_id,
                    "guideMarkdown": p.guide_markdown,
                })
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<AgentPresetSpec> {
        self.presets.read().get(id).cloned()
    }

    pub fn read_yaml(&self, id: &str) -> Option<String> {
        self.presets.read().get(id).map(|p| p.composition_yaml.clone())
    }
}
