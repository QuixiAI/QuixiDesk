//! `quixi-cordis`: Spatiotemporal plugin kernel for QuixiDesk.
//!
//! Realizes the calculus and runtime of *A Programming Paradigm for Spatiotemporal Composability*
//! (`arXiv:2608.25512`):
//! - **Temporal composability**: Revertible effects (`Context::effect`) accumulated as a LIFO
//!   twisted monoid (`DisposeStack`, Theorem 68 *Recovery Exactness*).
//! - **Spatial composability**: Reactive coeffects (`ServiceKey<T>`, `inject`, `provide`),
//!   guarded dependent draining (`!relied` on `L-Unload`, Theorem 70 *Ordering*),
//!   step-boundary target coherence (`L-Divert`, Theorem 71 *Resolution Coherence*),
//!   realm isolation (`Context::isolate`), and capability monoid attenuation (`Context::intercept`).
//! - **Confluent declarative reconciliation**: Live profile/bundle updates (`CordisRuntime::reconcile`,
//!   Theorem 73 *Progress* and Theorem 80 *Confluence*).

pub mod coeffect;
pub mod events;
pub mod loader;
pub mod scope;

pub use coeffect::{
    AccessLevel, CapabilityMonoid, InterceptionKey, RealmId, ServiceKey, ToolFilter,
    FS_ACCESS_INTERCEPTION, SHELL_ACCESS_INTERCEPTION, TOOL_FILTER_INTERCEPTION,
};
pub use events::{EventBus, EventDispatchMode, EventSpecInfo, NextWaterfall};
pub use loader::{
    BundleManifest, PluginRowSpec, ProfilePatchFile, ProfileRowPatch, ProfileSelectionFile,
    ProfileStore, ResolvedLoaderRow,
};
pub use scope::{AnonymousEntries, NamedEntries, ScopeKey, ScopeLayer, ScopedLayers};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CordisError {
    #[error("undeclared coeffect access to \"{service}\" from fiber \"{fiber}\" (declare it in `inject`)")]
    UndeclaredAccess { fiber: String, service: String },
    #[error("inactive coeffect access to \"{service}\" from fiber \"{fiber}\"")]
    InactiveAccess { fiber: String, service: String },
    #[error("undeclared coeffect provision \"{service}\" from fiber \"{fiber}\" (declare it in `provide`)")]
    UndeclaredProvision { fiber: String, service: String },
    #[error("duplicate coeffect provision for \"{service}\" in realm {realm:?} (already claimed by \"{owner}\")")]
    ProvisionConflict {
        service: String,
        realm: RealmId,
        owner: String,
    },
    #[error("service \"{service}\" type mismatch")]
    TypeMismatch { service: String },
    #[error("unknown plugin factory \"{plugin}\"")]
    UnknownPlugin { plugin: String },
    #[error("dependency cycle detected involving service \"{service}\"")]
    DependencyCycle { service: String },
    #[error("{0}")]
    SetupFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FiberUid(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", content = "error", rename_all = "kebab-case")]
pub enum FiberState {
    Inactive,
    Loading,
    Active,
    Unloading,
    Failed(String),
}

impl FiberState {
    pub fn is_installed(&self) -> bool {
        matches!(
            self,
            FiberState::Loading | FiberState::Active | FiberState::Unloading
        )
    }

    pub fn phase_name(&self) -> &'static str {
        match self {
            FiberState::Inactive => "inactive",
            FiberState::Loading => "loading",
            FiberState::Active => "active",
            FiberState::Unloading => "unloading",
            FiberState::Failed(_) => "failed",
        }
    }
}

/// One state-dependent inverse closure recorded in a fiber's LIFO accumulator.
pub type Dispose = Box<dyn FnOnce(&Context) + Send>;

pub fn noop_dispose() -> Dispose {
    Box::new(|_| {})
}

struct RecordedDispose {
    label: String,
    inverse: Dispose,
}

#[derive(Default)]
struct DisposeStack {
    entries: Vec<RecordedDispose>,
}

impl DisposeStack {
    fn push(&mut self, label: impl Into<String>, inverse: Dispose) {
        self.entries.push(RecordedDispose {
            label: label.into(),
            inverse,
        });
    }

    fn labels(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.label.clone()).collect()
    }

    /// Unwinds accumulated inverses in strict LIFO order (`Definition 18 & Theorem 68`).
    fn unwind_lifo(&mut self, ctx: &Context) {
        while let Some(entry) = self.entries.pop() {
            (entry.inverse)(ctx);
        }
    }
}

/// Multi-step setup step for `L-Iter` / `L-Divert` target coherence (`Theorem 71`).
pub type SetupStep = Box<dyn FnOnce(&Context, &Value) -> Result<(), CordisError> + Send>;

/// Trait implemented by every host plugin in QuixiDesk.
pub trait Plugin: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn inject(&self) -> &'static [&'static str] {
        &[]
    }

    fn provide(&self) -> &'static [&'static str] {
        &[]
    }

    /// Single-step activation body (used unless `setup_steps` is overridden).
    fn apply(&self, _ctx: &Context, _config: &Value) -> Result<(), CordisError> {
        Ok(())
    }

    /// Optional multi-step iterator (`Section 3.1.3 & Algorithm 3`). Between steps, the
    /// runtime re-verifies `target(fiber) == committed(fiber)` (`L-Divert`), unwinding
    /// partial effects if a dependency turns mid-setup.
    fn setup_steps(self: Arc<Self>) -> Vec<SetupStep> {
        vec![Box::new(move |ctx, cfg| self.apply(ctx, cfg))]
    }
}

/// Serializable inspection snapshot of a mounted Fiber.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiberSnapshot {
    pub uid: u64,
    pub id: String,
    pub bundle_id: String,
    pub plugin: String,
    pub title: String,
    pub description: String,
    pub parent_uid: Option<u64>,
    pub disabled: bool,
    pub locked: bool,
    pub state: FiberState,
    pub inject: Vec<String>,
    pub provide: Vec<String>,
    pub isolate: BTreeMap<String, RealmId>,
    pub committed_providers: BTreeMap<String, u64>,
    pub active_provisions: Vec<String>,
    pub effect_labels: Vec<String>,
    pub config: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<Value>,
}

struct FiberEntry {
    uid: FiberUid,
    id: String,
    bundle_id: String,
    plugin_name: String,
    title: String,
    description: String,
    plugin: Arc<dyn Plugin>,
    parent: Option<FiberUid>,
    inject: BTreeSet<String>,
    provide: BTreeSet<String>,
    isolate: BTreeMap<String, RealmId>,
    disabled: bool,
    locked: bool,
    retired: bool,
    config: Value,
    config_digest: String,
    config_schema: Option<Value>,
    state: FiberState,
    /// `omega_n`: pinned provider UIDs resolved at `L-Begin`, kept constant until `L-Unload` finishes.
    committed: BTreeMap<String, FiberUid>,
    /// `sigma_n`: services currently provided by this fiber, keyed by `(service_name, realm)`.
    provisions: HashMap<(String, RealmId), Arc<dyn Any + Send + Sync>>,
    /// Pending setup steps while in `FiberState::Loading`.
    pending_steps: std::collections::VecDeque<SetupStep>,
    /// LIFO inverse accumulator `g_n`.
    dispose_stack: DisposeStack,
}

impl FiberEntry {
    fn resolve_realm(&self, key: &str) -> RealmId {
        self.isolate.get(key).cloned().unwrap_or(RealmId::Shared)
    }

    fn snapshot(&self) -> FiberSnapshot {
        FiberSnapshot {
            uid: self.uid.0,
            id: self.id.clone(),
            bundle_id: self.bundle_id.clone(),
            plugin: self.plugin_name.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            parent_uid: self.parent.map(|u| u.0),
            disabled: self.disabled,
            locked: self.locked,
            state: self.state.clone(),
            inject: self.inject.iter().cloned().collect(),
            provide: self.provide.iter().cloned().collect(),
            isolate: self.isolate.clone(),
            committed_providers: self
                .committed
                .iter()
                .map(|(k, v)| (k.clone(), v.0))
                .collect(),
            active_provisions: {
                let mut v: Vec<String> = self.provisions.keys().map(|(k, _)| k.clone()).collect();
                v.sort();
                v
            },
            effect_labels: self.dispose_stack.labels(),
            config: self.config.clone(),
            config_schema: self.config_schema.clone(),
        }
    }
}

#[derive(Default)]
struct RuntimeRegistry {
    fibers: BTreeMap<FiberUid, FiberEntry>,
    id_to_uid: HashMap<String, FiberUid>,
    factories: HashMap<String, Arc<dyn Plugin>>,
    service_descriptions: BTreeMap<String, String>,
    client_manifest: Value,
}

pub struct CordisRuntime {
    registry: parking_lot::Mutex<RuntimeRegistry>,
    events: EventBus,
    next_uid: AtomicU64,
    reconcile_lock: parking_lot::Mutex<()>,
}

/// First-class execution `Context` mediating all effects and coeffects for a Fiber or Root caller.
#[derive(Clone)]
pub struct Context {
    runtime: Weak<CordisRuntime>,
    fiber_uid: Option<FiberUid>,
    scope: Option<ScopeKey>,
    realm_overrides: BTreeMap<String, RealmId>,
    interceptions: BTreeMap<String, Arc<dyn Any + Send + Sync>>,
}

impl Context {
    pub fn runtime(&self) -> Option<Arc<CordisRuntime>> {
        self.runtime.upgrade()
    }

    pub fn fiber_uid(&self) -> Option<FiberUid> {
        self.fiber_uid
    }

    pub fn scope(&self) -> Option<&ScopeKey> {
        self.scope.as_ref()
    }

    /// Derive a scoped view (`Global -> Preset -> Session -> Subagent`).
    pub fn scoped(&self, scope: ScopeKey) -> Self {
        let mut next = self.clone();
        next.scope = Some(scope);
        next
    }

    /// Derive a realm-isolated view for `service_name` (`Definition 25`).
    pub fn isolate(&self, service_name: impl Into<String>, realm: RealmId) -> Self {
        let mut next = self.clone();
        next.realm_overrides.insert(service_name.into(), realm);
        next
    }

    /// Attach associative monoid capability metadata (`Definition 26`), combining right-biased
    /// with any outer interception already carried on `key`.
    pub fn intercept<M: CapabilityMonoid>(&self, key: InterceptionKey<M>, meta: M) -> Self {
        let combined = self.read_interception(key).combine(&meta);
        let mut next = self.clone();
        next.interceptions
            .insert(key.name.to_owned(), Arc::new(combined));
        next
    }

    /// Read effective monoid metadata for `key` (`identity()` when unintercepted).
    pub fn read_interception<M: CapabilityMonoid>(&self, key: InterceptionKey<M>) -> M {
        self.interceptions
            .get(key.name)
            .and_then(|any| any.downcast_ref::<M>())
            .cloned()
            .unwrap_or_else(M::identity)
    }

    pub fn effective_realm(&self, service_name: &str) -> RealmId {
        if let Some(r) = self.realm_overrides.get(service_name) {
            return r.clone();
        }
        if let (Some(rt), Some(uid)) = (self.runtime(), self.fiber_uid) {
            let guard = rt.registry.lock();
            if let Some(f) = guard.fibers.get(&uid) {
                return f.resolve_realm(service_name);
            }
        }
        RealmId::Shared
    }

    /// Primitive revertible effect (`Section 3.1.1`): runs `f(self)` and records its inverse
    /// in the owning Fiber's LIFO accumulator. On the root context, executes `f` immediately.
    pub fn effect<R>(
        &self,
        label: impl Into<String>,
        f: impl FnOnce(&Context) -> Result<(R, Dispose), CordisError>,
    ) -> Result<R, CordisError> {
        let (out, inverse) = f(self)?;
        if let (Some(rt), Some(uid)) = (self.runtime(), self.fiber_uid) {
            let mut guard = rt.registry.lock();
            if let Some(fiber) = guard.fibers.get_mut(&uid) {
                fiber.dispose_stack.push(label, inverse);
            }
        }
        Ok(out)
    }

    /// Provide a typed coeffect service (`Section 3.2.1`), automatically retracted on unload.
    pub fn provide<T: Send + Sync + 'static>(
        &self,
        key: ServiceKey<T>,
        value: Arc<T>,
    ) -> Result<(), CordisError> {
        let Some(rt) = self.runtime() else {
            return Err(CordisError::SetupFailed("runtime dropped".into()));
        };
        let Some(uid) = self.fiber_uid else {
            return Err(CordisError::SetupFailed(
                "provide() requires a mounted plugin fiber context".into(),
            ));
        };
        let realm = self.effective_realm(key.name);
        {
            let mut guard = rt.registry.lock();
            guard
                .service_descriptions
                .entry(key.name.to_owned())
                .or_insert_with(|| key.description.to_owned());
            let fiber_id = guard
                .fibers
                .get(&uid)
                .map(|f| f.id.clone())
                .unwrap_or_else(|| format!("fiber:{}", uid.0));
            let declared = guard
                .fibers
                .get(&uid)
                .is_some_and(|f| f.provide.contains(key.name));
            if !declared {
                return Err(CordisError::UndeclaredProvision {
                    fiber: fiber_id,
                    service: key.name.to_owned(),
                });
            }
            for (other_uid, other) in &guard.fibers {
                if *other_uid != uid
                    && !other.retired
                    && other.provide.contains(key.name)
                    && other.resolve_realm(key.name) == realm
                    && other.provisions.contains_key(&(key.name.to_owned(), realm.clone()))
                {
                    return Err(CordisError::ProvisionConflict {
                        service: key.name.to_owned(),
                        realm,
                        owner: other.id.clone(),
                    });
                }
            }
            if let Some(fiber) = guard.fibers.get_mut(&uid) {
                fiber
                    .provisions
                    .insert((key.name.to_owned(), realm.clone()), value);
                let key_name = key.name.to_owned();
                let realm_clone = realm.clone();
                fiber.dispose_stack.push(
                    format!("provide({})", key.name),
                    Box::new(move |dispose_ctx| {
                        if let Some(rt) = dispose_ctx.runtime() {
                            if let Some(f) = rt.registry.lock().fibers.get_mut(&uid) {
                                f.provisions.remove(&(key_name, realm_clone));
                            }
                        }
                    }),
                );
            }
        }
        Ok(())
    }

    /// Confinement-checked coeffect resolution (`Algorithm 6`):
    /// - Inside a Fiber, verifies `key` is in `inject` (or `provide`) and reads via `fiber.committed`
    ///   so teardown (`Unloading`) continues to see its pinned provider until dependent drain completes!
    /// - On the root administrative context (`fiber_uid == None`), resolves the active provider in `realm`.
    pub fn get<T: Send + Sync + 'static>(&self, key: ServiceKey<T>) -> Result<Arc<T>, CordisError> {
        let Some(rt) = self.runtime() else {
            return Err(CordisError::InactiveAccess {
                fiber: "detached".into(),
                service: key.name.to_owned(),
            });
        };
        let realm = self.effective_realm(key.name);
        let guard = rt.registry.lock();
        let any_arc: Arc<dyn Any + Send + Sync> = match self.fiber_uid {
            Some(uid) => {
                let Some(caller) = guard.fibers.get(&uid) else {
                    return Err(CordisError::InactiveAccess {
                        fiber: format!("fiber:{}", uid.0),
                        service: key.name.to_owned(),
                    });
                };
                if !caller.state.is_installed() {
                    return Err(CordisError::InactiveAccess {
                        fiber: caller.id.clone(),
                        service: key.name.to_owned(),
                    });
                }
                if caller.provide.contains(key.name) {
                    if let Some(own) = caller.provisions.get(&(key.name.to_owned(), realm.clone()))
                    {
                        own.clone()
                    } else {
                        return Err(CordisError::InactiveAccess {
                            fiber: caller.id.clone(),
                            service: key.name.to_owned(),
                        });
                    }
                } else if caller.inject.contains(key.name) {
                    let Some(&provider_uid) = caller.committed.get(key.name) else {
                        return Err(CordisError::InactiveAccess {
                            fiber: caller.id.clone(),
                            service: key.name.to_owned(),
                        });
                    };
                    let Some(provider) = guard.fibers.get(&provider_uid) else {
                        return Err(CordisError::InactiveAccess {
                            fiber: caller.id.clone(),
                            service: key.name.to_owned(),
                        });
                    };
                    let provider_realm = provider.resolve_realm(key.name);
                    let Some(found) = provider
                        .provisions
                        .get(&(key.name.to_owned(), provider_realm))
                    else {
                        return Err(CordisError::InactiveAccess {
                            fiber: caller.id.clone(),
                            service: key.name.to_owned(),
                        });
                    };
                    found.clone()
                } else {
                    return Err(CordisError::UndeclaredAccess {
                        fiber: caller.id.clone(),
                        service: key.name.to_owned(),
                    });
                }
            }
            None => {
                let found = guard.fibers.values().find_map(|f| {
                    if f.state == FiberState::Active && !f.retired {
                        f.provisions.get(&(key.name.to_owned(), realm.clone())).cloned()
                    } else {
                        None
                    }
                });
                let Some(found) = found else {
                    return Err(CordisError::InactiveAccess {
                        fiber: "root".into(),
                        service: key.name.to_owned(),
                    });
                };
                found
            }
        };

        Arc::downcast::<T>(any_arc).map_err(|_| CordisError::TypeMismatch {
            service: key.name.to_owned(),
        })
    }

    pub fn try_get<T: Send + Sync + 'static>(&self, key: ServiceKey<T>) -> Option<Arc<T>> {
        self.get(key).ok()
    }

    fn owner_label(&self) -> String {
        if let (Some(rt), Some(uid)) = (self.runtime(), self.fiber_uid) {
            if let Some(f) = rt.registry.lock().fibers.get(&uid) {
                return f.id.clone();
            }
        }
        "root".to_string()
    }

    /// Revertibly register a synchronous broadcast listener (`@mode emit`).
    pub fn on_emit(
        &self,
        event: impl Into<String>,
        priority: i32,
        cb: impl Fn(&Value) + Send + Sync + 'static,
    ) -> Result<(), CordisError> {
        let event_name = event.into();
        let label = format!("on_emit({event_name})");
        let scope = self.scope.clone();
        let owner = self.owner_label();
        self.effect(label, move |ctx| {
            let rt = ctx
                .runtime()
                .ok_or_else(|| CordisError::SetupFailed("runtime dropped".into()))?;
            let off = rt.events.register_emit(event_name, scope, priority, owner, cb);
            Ok(((), Box::new(move |_| off())))
        })
    }

    /// Revertibly register a parallel fan-out listener (`@mode parallel`).
    pub fn on_parallel(
        &self,
        event: impl Into<String>,
        priority: i32,
        cb: impl Fn(&Value) -> Value + Send + Sync + 'static,
    ) -> Result<(), CordisError> {
        let event_name = event.into();
        let label = format!("on_parallel({event_name})");
        let scope = self.scope.clone();
        let owner = self.owner_label();
        self.effect(label, move |ctx| {
            let rt = ctx
                .runtime()
                .ok_or_else(|| CordisError::SetupFailed("runtime dropped".into()))?;
            let off = rt
                .events
                .register_parallel(event_name, scope, priority, owner, cb);
            Ok(((), Box::new(move |_| off())))
        })
    }

    /// Revertibly register an ordered serial pipeline listener (`@mode serial`).
    pub fn on_serial(
        &self,
        event: impl Into<String>,
        priority: i32,
        cb: impl Fn(&mut Value) + Send + Sync + 'static,
    ) -> Result<(), CordisError> {
        let event_name = event.into();
        let label = format!("on_serial({event_name})");
        let scope = self.scope.clone();
        let owner = self.owner_label();
        self.effect(label, move |ctx| {
            let rt = ctx
                .runtime()
                .ok_or_else(|| CordisError::SetupFailed("runtime dropped".into()))?;
            let off = rt
                .events
                .register_serial(event_name, scope, priority, owner, cb);
            Ok(((), Box::new(move |_| off())))
        })
    }

    /// Revertibly register a short-circuiting decision listener (`@mode bail`).
    pub fn on_bail(
        &self,
        event: impl Into<String>,
        priority: i32,
        cb: impl Fn(&Value) -> Option<Value> + Send + Sync + 'static,
    ) -> Result<(), CordisError> {
        let event_name = event.into();
        let label = format!("on_bail({event_name})");
        let scope = self.scope.clone();
        let owner = self.owner_label();
        self.effect(label, move |ctx| {
            let rt = ctx
                .runtime()
                .ok_or_else(|| CordisError::SetupFailed("runtime dropped".into()))?;
            let off = rt
                .events
                .register_bail(event_name, scope, priority, owner, cb);
            Ok(((), Box::new(move |_| off())))
        })
    }

    /// Revertibly register an around-middleware waterfall listener (`@mode waterfall`).
    pub fn on_waterfall(
        &self,
        event: impl Into<String>,
        priority: i32,
        cb: impl Fn(Value, NextWaterfall<'_>) -> Value + Send + Sync + 'static,
    ) -> Result<(), CordisError> {
        let event_name = event.into();
        let label = format!("on_waterfall({event_name})");
        let scope = self.scope.clone();
        let owner = self.owner_label();
        self.effect(label, move |ctx| {
            let rt = ctx
                .runtime()
                .ok_or_else(|| CordisError::SetupFailed("runtime dropped".into()))?;
            let off = rt
                .events
                .register_waterfall(event_name, scope, priority, owner, cb);
            Ok(((), Box::new(move |_| off())))
        })
    }

    pub fn emit(&self, event: &str, payload: &Value) {
        if let Some(rt) = self.runtime() {
            rt.events.emit(event, self.scope.as_ref(), payload);
        }
    }

    pub fn parallel(&self, event: &str, payload: &Value) -> Vec<Value> {
        self.runtime()
            .map(|rt| rt.events.parallel(event, self.scope.as_ref(), payload))
            .unwrap_or_default()
    }

    pub fn serial(&self, event: &str, payload: &mut Value) {
        if let Some(rt) = self.runtime() {
            rt.events.serial(event, self.scope.as_ref(), payload);
        }
    }

    pub fn bail(&self, event: &str, payload: &Value) -> Option<Value> {
        self.runtime()
            .and_then(|rt| rt.events.bail(event, self.scope.as_ref(), payload))
    }

    pub fn waterfall<T>(&self, event: &str, initial: Value, terminal: T) -> Value
    where
        T: FnMut(Value) -> Value,
    {
        match self.runtime() {
            Some(rt) => rt.events.waterfall(event, self.scope.as_ref(), initial, terminal),
            None => {
                let mut t = terminal;
                t(initial)
            }
        }
    }

    /// Revertibly instantiate a child Fiber (`Definition 52`): inherits isolated realms from `self`
    /// and automatically retires when the parent Fiber unloads!
    pub fn mount_child(
        &self,
        id: impl Into<String>,
        plugin: Arc<dyn Plugin>,
        config: Value,
    ) -> Result<FiberUid, CordisError> {
        let id = id.into();
        let parent_uid = self.fiber_uid;
        let inherited_isolate = self.realm_overrides.clone();
        let label = format!("mount_child({id})");
        self.effect(label, move |ctx| {
            let rt = ctx
                .runtime()
                .ok_or_else(|| CordisError::SetupFailed("runtime dropped".into()))?;
            let child_uid = rt.insert_fiber_internal(
                id.clone(),
                "child".into(),
                id,
                String::new(),
                plugin,
                parent_uid,
                inherited_isolate,
                false,
                false,
                config,
                None,
            )?;
            Ok((
                child_uid,
                Box::new(move |dispose_ctx| {
                    if let Some(rt) = dispose_ctx.runtime() {
                        rt.retire_fiber_internal(child_uid);
                    }
                }),
            ))
        })
    }
}

impl CordisRuntime {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            registry: parking_lot::Mutex::new(RuntimeRegistry::default()),
            events: EventBus::new(),
            next_uid: AtomicU64::new(0),
            reconcile_lock: parking_lot::Mutex::new(()),
        })
    }

    pub fn root_context(self: &Arc<Self>) -> Context {
        Context {
            runtime: Arc::downgrade(self),
            fiber_uid: None,
            scope: None,
            realm_overrides: BTreeMap::new(),
            interceptions: BTreeMap::new(),
        }
    }

    pub fn fiber_context(self: &Arc<Self>, uid: FiberUid) -> Context {
        let isolate = self
            .registry
            .lock()
            .fibers
            .get(&uid)
            .map(|f| f.isolate.clone())
            .unwrap_or_default();
        Context {
            runtime: Arc::downgrade(self),
            fiber_uid: Some(uid),
            scope: None,
            realm_overrides: isolate,
            interceptions: BTreeMap::new(),
        }
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    pub fn register_factory(&self, plugin: Arc<dyn Plugin>) {
        self.registry
            .lock()
            .factories
            .insert(plugin.name().to_owned(), plugin);
    }

    pub fn has_factory(&self, name: &str) -> bool {
        self.registry.lock().factories.contains_key(name)
    }

    pub fn set_client_manifest(&self, manifest: Value) {
        self.registry.lock().client_manifest = manifest;
    }

    pub fn client_manifest(&self) -> Value {
        self.registry.lock().client_manifest.clone()
    }

    fn alloc_uid(&self) -> FiberUid {
        FiberUid(self.next_uid.fetch_add(1, Ordering::SeqCst) + 1)
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_fiber_internal(
        self: &Arc<Self>,
        id: String,
        bundle_id: String,
        title: String,
        description: String,
        plugin: Arc<dyn Plugin>,
        parent: Option<FiberUid>,
        isolate: BTreeMap<String, RealmId>,
        disabled: bool,
        locked: bool,
        config: Value,
        config_schema: Option<Value>,
    ) -> Result<FiberUid, CordisError> {
        let uid = self.alloc_uid();
        let inject: BTreeSet<String> = plugin.inject().iter().map(|s| (*s).to_owned()).collect();
        let provide: BTreeSet<String> = plugin.provide().iter().map(|s| (*s).to_owned()).collect();
        let config_digest = serde_json::to_string(&json!({
            "plugin": plugin.name(),
            "isolate": isolate,
            "config": config,
        }))
        .unwrap_or_default();

        {
            let mut guard = self.registry.lock();
            for p_key in &provide {
                let realm = isolate.get(p_key).cloned().unwrap_or(RealmId::Shared);
                for other in guard.fibers.values() {
                    if !other.retired
                        && !other.disabled
                        && other.provide.contains(p_key)
                        && other.resolve_realm(p_key) == realm
                    {
                        return Err(CordisError::ProvisionConflict {
                            service: p_key.clone(),
                            realm,
                            owner: other.id.clone(),
                        });
                    }
                }
            }
            guard.id_to_uid.insert(id.clone(), uid);
            guard.fibers.insert(
                uid,
                FiberEntry {
                    uid,
                    id,
                    bundle_id,
                    plugin_name: plugin.name().to_owned(),
                    title,
                    description,
                    plugin,
                    parent,
                    inject,
                    provide,
                    isolate,
                    disabled,
                    locked,
                    retired: false,
                    config,
                    config_digest,
                    config_schema,
                    state: FiberState::Inactive,
                    committed: BTreeMap::new(),
                    provisions: HashMap::new(),
                    pending_steps: std::collections::VecDeque::new(),
                    dispose_stack: DisposeStack::default(),
                },
            );
        }
        Ok(uid)
    }

    fn retire_fiber_internal(&self, uid: FiberUid) {
        if let Some(f) = self.registry.lock().fibers.get_mut(&uid) {
            f.retired = true;
        }
    }

    /// Mount a standalone plugin directly (used in unit tests and programmatic bootstrap)
    /// and drive the lifecycle state machine to quiescence.
    pub fn mount(
        self: &Arc<Self>,
        id: impl Into<String>,
        plugin: Arc<dyn Plugin>,
        config: Value,
    ) -> Result<FiberUid, CordisError> {
        let _guard = self.reconcile_lock.lock();
        let id = id.into();
        self.register_factory(plugin.clone());
        let uid = self.insert_fiber_internal(
            id.clone(),
            "direct".into(),
            id,
            String::new(),
            plugin,
            None,
            BTreeMap::new(),
            false,
            false,
            config,
            None,
        )?;
        self.quiesce();
        Ok(uid)
    }

    /// Mount with explicit realm isolation map (`ctx.isolate` declarative form).
    pub fn mount_isolated(
        self: &Arc<Self>,
        id: impl Into<String>,
        plugin: Arc<dyn Plugin>,
        isolate: BTreeMap<String, RealmId>,
        config: Value,
    ) -> Result<FiberUid, CordisError> {
        let _guard = self.reconcile_lock.lock();
        let id = id.into();
        self.register_factory(plugin.clone());
        let uid = self.insert_fiber_internal(
            id.clone(),
            "direct".into(),
            id,
            String::new(),
            plugin,
            None,
            isolate,
            false,
            false,
            config,
            None,
        )?;
        self.quiesce();
        Ok(uid)
    }

    /// Retire/unmount a fiber by entry ID and drive dependent draining + LIFO recovery to quiescence.
    pub fn unmount(self: &Arc<Self>, id: &str) -> bool {
        let _guard = self.reconcile_lock.lock();
        let uid_opt = self.registry.lock().id_to_uid.get(id).copied();
        let Some(uid) = uid_opt else {
            return false;
        };
        self.retire_fiber_internal(uid);
        self.quiesce();
        true
    }

    /// Reconcile declarative profile rows against the live Fiber registry (`Section 5.2`).
    ///
    /// - Unchanged rows stay untouched (zero flicker, zero session disruption).
    /// - Disabled rows transition `Active -> Unloading`, drain dependents first, and unwind LIFO.
    /// - Reconfigured rows retire the old Fiber episode cleanly and mount the new revision.
    /// - Newly enabled rows insert and activate in dependency order, converging confluently (`Theorem 80`).
    pub fn reconcile(self: &Arc<Self>, desired: &[ResolvedLoaderRow]) -> Result<(), CordisError> {
        let _guard = self.reconcile_lock.lock();
        let desired_ids: BTreeSet<&str> = desired.iter().map(|r| r.id.as_str()).collect();

        // Phase A: Retire removed or config-changed top-level rows, and mark newly disabled ones.
        let mut needs_reinsert: Vec<&ResolvedLoaderRow> = Vec::new();
        {
            let mut guard = self.registry.lock();
            let existing_root_ids: Vec<(String, FiberUid)> = guard
                .id_to_uid
                .iter()
                .filter_map(|(id, uid)| {
                    let is_root = guard
                        .fibers
                        .get(uid)
                        .is_some_and(|f| f.parent.is_none() && f.bundle_id != "direct");
                    is_root.then(|| (id.clone(), *uid))
                })
                .collect();

            for (existing_id, uid) in existing_root_ids {
                if !desired_ids.contains(existing_id.as_str()) {
                    if let Some(f) = guard.fibers.get_mut(&uid) {
                        f.retired = true;
                    }
                }
            }

            for row in desired {
                if let Some(&uid) = guard.id_to_uid.get(&row.id) {
                    let replace = guard.fibers.get(&uid).is_some_and(|f| {
                        f.retired
                            || f.config_digest != row.config_digest()
                            || (matches!(f.state, FiberState::Failed(_)) && !row.disabled)
                    });
                    if replace {
                        if let Some(f) = guard.fibers.get_mut(&uid) {
                            f.retired = true;
                        }
                        needs_reinsert.push(row);
                    } else if let Some(f) = guard.fibers.get_mut(&uid) {
                        f.disabled = row.disabled;
                        f.locked = row.locked;
                        f.title = row.title.clone();
                        f.description = row.description.clone();
                        f.config_schema = row.config_schema.clone();
                    }
                } else {
                    needs_reinsert.push(row);
                }
            }
        }

        // Drain retired/disabled fibers before inserting replacements that claim the same provision keys.
        self.quiesce();

        // Phase B: Insert new or replaced rows.
        for row in needs_reinsert {
            let plugin = {
                let guard = self.registry.lock();
                guard.factories.get(&row.plugin).cloned()
            };
            let Some(plugin) = plugin else {
                return Err(CordisError::UnknownPlugin {
                    plugin: row.plugin.clone(),
                });
            };
            self.insert_fiber_internal(
                row.id.clone(),
                row.bundle_id.clone(),
                row.title.clone(),
                row.description.clone(),
                plugin,
                None,
                row.isolate.clone(),
                row.disabled,
                row.locked,
                row.config.clone(),
                row.config_schema.clone(),
            )?;
        }

        // Phase C: Drive full graph to quiescence (`Theorem 73 & Theorem 80`).
        self.quiesce();
        self.events.emit(
            "cordis/reconciled",
            None,
            &json!({ "fibers": self.inspect_fibers().len() }),
        );
        Ok(())
    }

    /// Computes `target_n(gamma)` (`Definition 53`):
    /// Returns `Some(committed_view)` iff `!retired && !disabled`, `parent` is `Active` (or `None`),
    /// and every `k` in `inject` has an `Active` provider in `(k, rho_n(k))`.
    fn compute_target(registry: &RuntimeRegistry, uid: FiberUid) -> Option<BTreeMap<String, FiberUid>> {
        let fiber = registry.fibers.get(&uid)?;
        if fiber.retired || fiber.disabled {
            return None;
        }
        if let Some(p_uid) = fiber.parent {
            let parent_active = registry
                .fibers
                .get(&p_uid)
                .is_some_and(|p| p.state == FiberState::Active && !p.retired && !p.disabled);
            if !parent_active {
                return None;
            }
        }
        let mut resolved = BTreeMap::new();
        for req_key in &fiber.inject {
            let realm = fiber.resolve_realm(req_key);
            let provider_uid = registry.fibers.values().find_map(|cand| {
                if cand.uid != uid
                    && cand.state == FiberState::Active
                    && !cand.retired
                    && !cand.disabled
                    && cand
                        .provisions
                        .contains_key(&(req_key.clone(), realm.clone()))
                {
                    Some(cand.uid)
                } else {
                    None
                }
            })?;
            resolved.insert(req_key.clone(), provider_uid);
        }
        Some(resolved)
    }

    /// Computes `relied_n(gamma)` (`Definition 53 & Theorem 70`):
    /// True iff any other installed fiber `m` currently pins `n` in `m.committed`,
    /// or has `m.parent == Some(n)` while installed.
    fn is_relied_upon(registry: &RuntimeRegistry, uid: FiberUid) -> bool {
        registry.fibers.values().any(|other| {
            other.uid != uid
                && other.state.is_installed()
                && (other.parent == Some(uid) || other.committed.values().any(|&p| p == uid))
        })
    }

    /// Inertial lifecycle fixed-point loop (`Section 4.2 Table 1 & Section 5.1.3`):
    /// Repeatedly applies `L-Leave`, `L-Divert`, `L-Unload (!relied)`, `L-Begin`, `L-Iter`, `L-Finish`,
    /// and `O-Remove` until `quiet(gamma)` holds (`Theorem 73 Progress`).
    pub fn quiesce(self: &Arc<Self>) {
        // Safety bound against pathological user-plugin self-spawning loops (`Section 4.3.4`).
        const MAX_LIFECYCLE_STEPS: usize = 8_192;

        for _ in 0..MAX_LIFECYCLE_STEPS {
            // 1. L-Leave: Active fiber whose target != Some(committed) transitions immediately to Unloading
            //    so its dependents see it leave Active and begin draining FIRST!
            let leave_uid = {
                let guard = self.registry.lock();
                guard.fibers.keys().copied().find(|&uid| {
                    let f = &guard.fibers[&uid];
                    f.state == FiberState::Active
                        && Self::compute_target(&guard, uid).as_ref() != Some(&f.committed)
                })
            };
            if let Some(uid) = leave_uid {
                if let Some(f) = self.registry.lock().fibers.get_mut(&uid) {
                    f.state = FiberState::Unloading;
                }
                continue;
            }

            // 2. L-Divert (inertial): Loading fiber whose target != Some(committed) transitions to Unloading.
            let divert_uid = {
                let guard = self.registry.lock();
                guard.fibers.keys().copied().find(|&uid| {
                    let f = &guard.fibers[&uid];
                    f.state == FiberState::Loading
                        && Self::compute_target(&guard, uid).as_ref() != Some(&f.committed)
                })
            };
            if let Some(uid) = divert_uid {
                if let Some(f) = self.registry.lock().fibers.get_mut(&uid) {
                    f.pending_steps.clear();
                    f.state = FiberState::Unloading;
                }
                continue;
            }

            // 3. L-Unload: Unloading fiber with `!relied(uid)` unwinds its LIFO DisposeStack!
            let unload_uid = {
                let guard = self.registry.lock();
                guard.fibers.keys().copied().find(|&uid| {
                    let f = &guard.fibers[&uid];
                    f.state == FiberState::Unloading && !Self::is_relied_upon(&guard, uid)
                })
            };
            if let Some(uid) = unload_uid {
                let mut taken_stack = {
                    let mut guard = self.registry.lock();
                    guard
                        .fibers
                        .get_mut(&uid)
                        .map(|f| std::mem::take(&mut f.dispose_stack))
                        .unwrap_or_default()
                };
                let ctx = self.fiber_context(uid);
                taken_stack.unwind_lifo(&ctx);
                {
                    let mut guard = self.registry.lock();
                    if let Some(f) = guard.fibers.get_mut(&uid) {
                        f.provisions.clear();
                        f.committed.clear();
                        f.state = FiberState::Inactive;
                    }
                }
                continue;
            }

            // 4. O-Remove: Retired + Inactive fiber with no children is garbage-collected from registry.
            let remove_uid = {
                let guard = self.registry.lock();
                guard.fibers.keys().copied().find(|&uid| {
                    let f = &guard.fibers[&uid];
                    f.retired
                        && matches!(f.state, FiberState::Inactive | FiberState::Failed(_))
                        && !guard.fibers.values().any(|other| other.parent == Some(uid))
                })
            };
            if let Some(uid) = remove_uid {
                let mut guard = self.registry.lock();
                if let Some(removed) = guard.fibers.remove(&uid) {
                    if guard.id_to_uid.get(&removed.id) == Some(&uid) {
                        guard.id_to_uid.remove(&removed.id);
                    }
                }
                continue;
            }

            // 5. L-Iter / L-Finish: Loading fiber whose target == Some(committed) advances one setup step.
            let loading_step = {
                let mut guard = self.registry.lock();
                let uid_opt = guard.fibers.keys().copied().find(|&uid| {
                    let f = &guard.fibers[&uid];
                    f.state == FiberState::Loading
                        && Self::compute_target(&guard, uid).as_ref() == Some(&f.committed)
                });
                uid_opt.map(|uid| {
                    let f = guard.fibers.get_mut(&uid).unwrap();
                    let next_step = f.pending_steps.pop_front();
                    let cfg = f.config.clone();
                    (uid, next_step, cfg)
                })
            };
            if let Some((uid, step_opt, cfg)) = loading_step {
                match step_opt {
                    Some(step) => {
                        let ctx = self.fiber_context(uid);
                        if let Err(err) = step(&ctx, &cfg) {
                            // Section 4.4 Failure: unwind accumulated inverses and record Failed outcome!
                            let mut taken_stack = {
                                let mut guard = self.registry.lock();
                                if let Some(f) = guard.fibers.get_mut(&uid) {
                                    f.pending_steps.clear();
                                    f.state = FiberState::Unloading;
                                    std::mem::take(&mut f.dispose_stack)
                                } else {
                                    DisposeStack::default()
                                }
                            };
                            taken_stack.unwind_lifo(&ctx);
                            let mut guard = self.registry.lock();
                            if let Some(f) = guard.fibers.get_mut(&uid) {
                                f.provisions.clear();
                                f.committed.clear();
                                f.state = FiberState::Failed(err.to_string());
                            }
                        }
                    }
                    None => {
                        // L-Finish: all setup steps succeeded!
                        if let Some(f) = self.registry.lock().fibers.get_mut(&uid) {
                            f.state = FiberState::Active;
                        }
                    }
                }
                continue;
            }

            // 6. L-Begin: Inactive (non-Failed) fiber whose target == Some(omega) enters Loading!
            let begin_candidate = {
                let guard = self.registry.lock();
                guard.fibers.keys().copied().find_map(|uid| {
                    let f = &guard.fibers[&uid];
                    if f.state != FiberState::Inactive {
                        return None;
                    }
                    let omega = Self::compute_target(&guard, uid)?;
                    Some((uid, omega, f.plugin.clone()))
                })
            };
            if let Some((uid, omega, plugin)) = begin_candidate {
                let steps: std::collections::VecDeque<SetupStep> =
                    plugin.setup_steps().into_iter().collect();
                if let Some(f) = self.registry.lock().fibers.get_mut(&uid) {
                    f.committed = omega;
                    f.pending_steps = steps;
                    f.state = FiberState::Loading;
                }
                continue;
            }

            // Quiet state reached!
            break;
        }
    }

    pub fn inspect_fibers(&self) -> Vec<FiberSnapshot> {
        self.registry
            .lock()
            .fibers
            .values()
            .filter(|f| !f.retired)
            .map(FiberEntry::snapshot)
            .collect()
    }

    pub fn inspect_summary(&self) -> Value {
        let guard = self.registry.lock();
        let fibers: Vec<FiberSnapshot> = guard
            .fibers
            .values()
            .filter(|f| !f.retired)
            .map(FiberEntry::snapshot)
            .collect();
        let mut active_services: Vec<Value> = Vec::new();
        for f in guard
            .fibers
            .values()
            .filter(|f| f.state == FiberState::Active && !f.retired)
        {
            for (svc, realm) in f.provisions.keys() {
                active_services.push(json!({
                    "service": svc,
                    "realm": realm,
                    "providerId": f.id,
                    "providerPlugin": f.plugin_name,
                    "description": guard.service_descriptions.get(svc).cloned().unwrap_or_default(),
                }));
            }
        }
        let registered_factories: Vec<String> = {
            let mut names: Vec<String> = guard.factories.keys().cloned().collect();
            names.sort();
            names
        };
        json!({
            "fibers": fibers,
            "services": active_services,
            "events": self.events.inspect_events(),
            "registeredFactories": registered_factories,
            "client": guard.client_manifest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    const DB_KEY: ServiceKey<String> = ServiceKey::new("db", "Database connection string");
    const CACHE_KEY: ServiceKey<String> = ServiceKey::new("cache", "Cache backend");

    struct DbProvider {
        log: Arc<Mutex<Vec<String>>>,
        conn: String,
    }

    impl Plugin for DbProvider {
        fn name(&self) -> &'static str {
            "db-provider"
        }
        fn provide(&self) -> &'static [&'static str] {
            &["db"]
        }
        fn apply(&self, ctx: &Context, _cfg: &Value) -> Result<(), CordisError> {
            self.log.lock().push(format!("db:up:{}", self.conn));
            ctx.provide(DB_KEY, Arc::new(self.conn.clone()))?;
            let log = self.log.clone();
            let conn = self.conn.clone();
            ctx.effect("db-cleanup", move |_| {
                Ok((
                    (),
                    Box::new(move |_| {
                        log.lock().push(format!("db:down:{conn}"));
                    }),
                ))
            })
        }
    }

    struct CacheBridge {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for CacheBridge {
        fn name(&self) -> &'static str {
            "cache-bridge"
        }
        fn inject(&self) -> &'static [&'static str] {
            &["db"]
        }
        fn provide(&self) -> &'static [&'static str] {
            &["cache"]
        }
        fn apply(&self, ctx: &Context, _cfg: &Value) -> Result<(), CordisError> {
            let db = ctx.get(DB_KEY)?;
            self.log.lock().push(format!("cache:up(on {db})"));
            ctx.provide(CACHE_KEY, Arc::new(format!("cached({db})")))?;
            let log = self.log.clone();
            ctx.effect("cache-drain-against-db", move |_| {
                Ok((
                    (),
                    Box::new(move |teardown_ctx| {
                        // Theorem 70: During CacheBridge teardown, DbProvider MUST still be readable
                        // via CacheBridge's committed view `omega`!
                        let live_db = teardown_ctx
                            .get(DB_KEY)
                            .expect("provider must remain readable until dependent finishes unloading");
                        log.lock().push(format!("cache:down(flushed to {live_db})"));
                    }),
                ))
            })
        }
    }

    struct ApiConsumer {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Plugin for ApiConsumer {
        fn name(&self) -> &'static str {
            "api-consumer"
        }
        fn inject(&self) -> &'static [&'static str] {
            &["cache"]
        }
        fn apply(&self, ctx: &Context, _cfg: &Value) -> Result<(), CordisError> {
            let cache = ctx.get(CACHE_KEY)?;
            self.log.lock().push(format!("api:up({cache})"));
            let log = self.log.clone();
            ctx.effect("api-stop", move |_| {
                Ok((
                    (),
                    Box::new(move |teardown_ctx| {
                        let live_cache = teardown_ctx
                            .get(CACHE_KEY)
                            .expect("cache must remain live while api-consumer unloads");
                        log.lock().push(format!("api:down({live_cache})"));
                    }),
                ))
            })
        }
    }

    #[test]
    fn theorem_68_and_70_lifo_recovery_and_dependent_drain_ordering() {
        let rt = CordisRuntime::new();
        let log = Arc::new(Mutex::new(Vec::new()));

        // Mount in REVERSE dependency order (consumer -> middle -> provider) to prove reactive activation!
        rt.mount("api", Arc::new(ApiConsumer { log: log.clone() }), json!({}))
            .unwrap();
        rt.mount("cache", Arc::new(CacheBridge { log: log.clone() }), json!({}))
            .unwrap();
        assert!(log.lock().is_empty(), "consumers stay Inactive until provider arrives");

        rt.mount(
            "db",
            Arc::new(DbProvider {
                log: log.clone(),
                conn: "pg://main".into(),
            }),
            json!({}),
        )
        .unwrap();

        assert_eq!(
            *log.lock(),
            vec![
                "db:up:pg://main",
                "cache:up(on pg://main)",
                "api:up(cached(pg://main))",
            ]
        );

        // Now unmount the ROOT provider `db` while `cache` and `api` are still mounted!
        // Guarded `L-Unload (!relied)` MUST drain `api` first, then `cache` (reading `db`), and `db` last!
        log.lock().clear();
        assert!(rt.unmount("db"));
        assert_eq!(
            *log.lock(),
            vec![
                "api:down(cached(pg://main))",
                "cache:down(flushed to pg://main)",
                "db:down:pg://main",
            ]
        );
    }

    #[test]
    fn algorithm_6_enforces_access_confinement_and_failure_isolation() {
        struct SneakyPlugin;
        impl Plugin for SneakyPlugin {
            fn name(&self) -> &'static str {
                "sneaky"
            }
            // Deliberately omits `inject = &["db"]`!
            fn apply(&self, ctx: &Context, _cfg: &Value) -> Result<(), CordisError> {
                let _ = ctx.get(DB_KEY)?;
                Ok(())
            }
        }

        let rt = CordisRuntime::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        rt.mount(
            "db",
            Arc::new(DbProvider {
                log,
                conn: "sqlite://mem".into(),
            }),
            json!({}),
        )
        .unwrap();
        rt.mount("sneaky", Arc::new(SneakyPlugin), json!({}))
            .unwrap();

        let fibers = rt.inspect_fibers();
        let sneaky = fibers.iter().find(|f| f.id == "sneaky").unwrap();
        assert!(
            matches!(&sneaky.state, FiberState::Failed(msg) if msg.contains("undeclared coeffect access")),
            "expected UndeclaredAccess failure, got {:?}",
            sneaky.state
        );
        // Sibling `db` fiber remains completely healthy!
        let db = fibers.iter().find(|f| f.id == "db").unwrap();
        assert_eq!(db.state, FiberState::Active);
    }

    #[test]
    fn realm_isolation_and_capability_monoid_interception_work() {
        let rt = CordisRuntime::new();
        let log = Arc::new(Mutex::new(Vec::new()));

        let mut worktree_realm = BTreeMap::new();
        worktree_realm.insert("db".into(), RealmId::named("worktree-a"));

        rt.mount(
            "db-shared",
            Arc::new(DbProvider {
                log: log.clone(),
                conn: "local-fs".into(),
            }),
            json!({}),
        )
        .unwrap();
        rt.mount_isolated(
            "db-worktree",
            Arc::new(DbProvider {
                log: log.clone(),
                conn: "worktree-fs".into(),
            }),
            worktree_realm.clone(),
            json!({}),
        )
        .unwrap();

        let root = rt.root_context();
        assert_eq!(*root.get(DB_KEY).unwrap(), "local-fs");
        let isolated = root.isolate("db", RealmId::named("worktree-a"));
        assert_eq!(*isolated.get(DB_KEY).unwrap(), "worktree-fs");

        // Monoid attenuation: WorkspaceWrite (+) ReadOnly == ReadOnly; ToolFilter intersects allow & unions deny.
        let session_ctx = root
            .intercept(FS_ACCESS_INTERCEPTION, AccessLevel::WorkspaceWrite)
            .intercept(
                TOOL_FILTER_INTERCEPTION,
                ToolFilter::allow_only(["Read", "Edit", "Bash"]),
            );
        let subagent_ctx = session_ctx
            .intercept(FS_ACCESS_INTERCEPTION, AccessLevel::ReadOnly)
            .intercept(TOOL_FILTER_INTERCEPTION, ToolFilter::deny_names(["Edit", "Bash"]));

        assert_eq!(
            session_ctx.read_interception(FS_ACCESS_INTERCEPTION),
            AccessLevel::WorkspaceWrite
        );
        assert_eq!(
            subagent_ctx.read_interception(FS_ACCESS_INTERCEPTION),
            AccessLevel::ReadOnly
        );
        let filter = subagent_ctx.read_interception(TOOL_FILTER_INTERCEPTION);
        assert!(filter.admits("Read"));
        assert!(!filter.admits("Edit"));
        assert!(!filter.admits("Write"));
    }

    #[test]
    fn theorem_80_confluent_profile_reconciliation_and_scoped_waterfalls() {
        let rt = CordisRuntime::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        rt.register_factory(Arc::new(DbProvider {
            log: log.clone(),
            conn: "v1".into(),
        }));
        rt.register_factory(Arc::new(CacheBridge { log: log.clone() }));
        rt.register_factory(Arc::new(ApiConsumer { log: log.clone() }));

        let make_row = |id: &str, plugin: &str, disabled: bool, cfg: Value| ResolvedLoaderRow {
            bundle_id: "core".into(),
            id: id.into(),
            plugin: plugin.into(),
            title: id.into(),
            description: String::new(),
            disabled,
            locked: false,
            isolate: BTreeMap::new(),
            config: cfg,
            config_schema: None,
        };

        // Sequence 1: enable all -> disable middle `cache` -> re-enable `cache`.
        rt.reconcile(&[
            make_row("db", "db-provider", false, json!({"rev": 1})),
            make_row("cache", "cache-bridge", false, json!({})),
            make_row("api", "api-consumer", false, json!({})),
        ])
        .unwrap();
        assert_eq!(
            rt.inspect_fibers()
                .iter()
                .filter(|f| f.state == FiberState::Active)
                .count(),
            3
        );

        // Disable `cache`: `api` loses its dependency and unloads cleanly, while `db` stays Active!
        rt.reconcile(&[
            make_row("db", "db-provider", false, json!({"rev": 1})),
            make_row("cache", "cache-bridge", true, json!({})),
            make_row("api", "api-consumer", false, json!({})),
        ])
        .unwrap();
        let snap = rt.inspect_fibers();
        assert_eq!(snap.iter().find(|f| f.id == "db").unwrap().state, FiberState::Active);
        assert_eq!(
            snap.iter().find(|f| f.id == "cache").unwrap().state,
            FiberState::Inactive
        );
        assert_eq!(
            snap.iter().find(|f| f.id == "api").unwrap().state,
            FiberState::Inactive
        );

        // Re-enable `cache`: converges back to the exact canonical 3-Active state!
        rt.reconcile(&[
            make_row("db", "db-provider", false, json!({"rev": 1})),
            make_row("cache", "cache-bridge", false, json!({})),
            make_row("api", "api-consumer", false, json!({})),
        ])
        .unwrap();
        assert_eq!(
            rt.inspect_fibers()
                .iter()
                .filter(|f| f.state == FiberState::Active)
                .count(),
            3
        );

        // Verify scoped waterfall middleware ordering & revertible cleanup:
        let root = rt.root_context();
        let sess_a = root.scoped(ScopeKey::session("s-a", Some("standard".into())));
        let sess_b = root.scoped(ScopeKey::session("s-b", Some("minimal".into())));

        root.on_waterfall("tools/pre-execute", 10, |mut v, next| {
            v["trace"].as_array_mut().unwrap().push(json!("global-outer"));
            let mut out = next(v);
            out["trace"].as_array_mut().unwrap().push(json!("global-after"));
            out
        })
        .unwrap();
        sess_a
            .on_waterfall("tools/pre-execute", 20, |mut v, next| {
                v["trace"].as_array_mut().unwrap().push(json!("sess-a-only"));
                next(v)
            })
            .unwrap();

        let res_a = sess_a.waterfall("tools/pre-execute", json!({"trace": []}), |mut v| {
            v["trace"].as_array_mut().unwrap().push(json!("terminal"));
            v
        });
        assert_eq!(
            res_a["trace"],
            json!(["global-outer", "sess-a-only", "terminal", "global-after"])
        );

        let res_b = sess_b.waterfall("tools/pre-execute", json!({"trace": []}), |mut v| {
            v["trace"].as_array_mut().unwrap().push(json!("terminal"));
            v
        });
        assert_eq!(
            res_b["trace"],
            json!(["global-outer", "terminal", "global-after"])
        );
    }
}
