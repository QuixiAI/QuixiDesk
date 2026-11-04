//! Hierarchical scope chains and scoped contribution layers (`dsh-scope` realization).
//!
//! Scopes form an explicit chain: `Global -> Preset -> Session -> Subagent`.
//! Nearer scopes shadow farther named entries while intersecting restrictions,
//! and every mutation returns a revertible disposer that prunes empty layers.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScopeKey {
    Preset {
        preset_id: String,
    },
    Session {
        session_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preset_id: Option<String>,
    },
    Subagent {
        session_id: String,
        agent_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preset_id: Option<String>,
    },
}

impl ScopeKey {
    pub fn preset(preset_id: impl Into<String>) -> Self {
        Self::Preset {
            preset_id: preset_id.into(),
        }
    }

    pub fn session(session_id: impl Into<String>, preset_id: Option<String>) -> Self {
        Self::Session {
            session_id: session_id.into(),
            preset_id,
        }
    }

    pub fn subagent(
        session_id: impl Into<String>,
        agent_id: impl Into<String>,
        preset_id: Option<String>,
    ) -> Self {
        Self::Subagent {
            session_id: session_id.into(),
            agent_id: agent_id.into(),
            preset_id,
        }
    }

    /// Returns the chain of scopes from farthest (after Global) to nearest (self).
    pub fn chain(&self) -> Vec<ScopeKey> {
        match self {
            ScopeKey::Preset { preset_id } => vec![ScopeKey::Preset {
                preset_id: preset_id.clone(),
            }],
            ScopeKey::Session {
                session_id,
                preset_id,
            } => {
                let mut out = Vec::with_capacity(2);
                if let Some(pid) = preset_id {
                    out.push(ScopeKey::Preset {
                        preset_id: pid.clone(),
                    });
                }
                out.push(ScopeKey::Session {
                    session_id: session_id.clone(),
                    preset_id: preset_id.clone(),
                });
                out
            }
            ScopeKey::Subagent {
                session_id,
                agent_id,
                preset_id,
            } => {
                let mut out = Vec::with_capacity(3);
                if let Some(pid) = preset_id {
                    out.push(ScopeKey::Preset {
                        preset_id: pid.clone(),
                    });
                }
                out.push(ScopeKey::Session {
                    session_id: session_id.clone(),
                    preset_id: preset_id.clone(),
                });
                out.push(ScopeKey::Subagent {
                    session_id: session_id.clone(),
                    agent_id: agent_id.clone(),
                    preset_id: preset_id.clone(),
                });
                out
            }
        }
    }

    /// True if a listener registered on `listener_scope` applies when dispatching to `target_scope`.
    pub fn matches_dispatch(listener_scope: Option<&ScopeKey>, target_scope: Option<&ScopeKey>) -> bool {
        match (listener_scope, target_scope) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(ls), Some(ts)) => ts.chain().iter().any(|item| item == ls),
        }
    }
}

pub trait ScopeLayer: Default + Send + Sync + 'static {
    fn is_empty(&self) -> bool;
}

#[derive(Debug)]
pub struct NamedEntries<V> {
    entries: BTreeMap<String, (u64, V)>,
    seq: u64,
}

impl<V> Default for NamedEntries<V> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            seq: 0,
        }
    }
}

impl<V: Clone> NamedEntries<V> {
    pub fn insert(&mut self, name: impl Into<String>, value: V) -> Result<u64, String> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(format!("entry \"{name}\" is already registered in this scope"));
        }
        self.seq += 1;
        let token = self.seq;
        self.entries.insert(name, (token, value));
        Ok(token)
    }

    pub fn remove_matching(&mut self, name: &str, token: u64) -> bool {
        if self.entries.get(name).is_some_and(|(t, _)| *t == token) {
            self.entries.remove(name);
            true
        } else {
            false
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&V> {
        self.entries.get(name).map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, (_, v))| (k.as_str(), v))
    }
}

#[derive(Debug)]
pub struct AnonymousEntries<V> {
    entries: Vec<(u64, V)>,
    seq: u64,
}

impl<V> Default for AnonymousEntries<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            seq: 0,
        }
    }
}

impl<V: Clone> AnonymousEntries<V> {
    pub fn append(&mut self, value: V) -> u64 {
        self.seq += 1;
        let token = self.seq;
        self.entries.push((token, value));
        token
    }

    pub fn remove(&mut self, token: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(t, _)| *t != token);
        self.entries.len() != before
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }
}

struct ScopedLayersInner<L: ScopeLayer> {
    global: L,
    scoped: HashMap<ScopeKey, L>,
}

/// Thread-safe hierarchical layer table with automatic empty-layer pruning and change callbacks.
pub struct ScopedLayers<L: ScopeLayer> {
    inner: Arc<RwLock<ScopedLayersInner<L>>>,
    generation: Arc<AtomicU64>,
    on_change: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl<L: ScopeLayer> Clone for ScopedLayers<L> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            generation: self.generation.clone(),
            on_change: self.on_change.clone(),
        }
    }
}

impl<L: ScopeLayer> ScopedLayers<L> {
    pub fn new(on_change: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ScopedLayersInner {
                global: L::default(),
                scoped: HashMap::new(),
            })),
            generation: Arc::new(AtomicU64::new(0)),
            on_change,
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn notify(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(ref cb) = self.on_change {
            cb();
        }
    }

    /// Apply a forward mutation on `scope` and obtain a `Box<dyn FnOnce() + Send>` inverse.
    #[allow(clippy::type_complexity)]
    pub fn mutate_revertible<F, R>(
        &self,
        scope: Option<ScopeKey>,
        apply: F,
    ) -> Result<(R, Box<dyn FnOnce() + Send>), String>
    where
        F: FnOnce(&mut L) -> Result<(R, Box<dyn FnOnce(&mut L) -> bool + Send>), String>,
    {
        let (out, revert_fn) = {
            let mut guard = self.inner.write();
            let layer = match &scope {
                None => &mut guard.global,
                Some(key) => guard.scoped.entry(key.clone()).or_default(),
            };
            apply(layer)?
        };
        self.notify();

        let layers = self.clone();
        let scope_for_dispose = scope;
        let inverse = Box::new(move || {
            let changed = {
                let mut guard = layers.inner.write();
                match &scope_for_dispose {
                    None => revert_fn(&mut guard.global),
                    Some(key) => {
                        let mut changed = false;
                        let mut prune = false;
                        if let Some(layer) = guard.scoped.get_mut(key) {
                            changed = revert_fn(layer);
                            prune = layer.is_empty();
                        }
                        if prune {
                            guard.scoped.remove(key);
                        }
                        changed
                    }
                }
            };
            if changed {
                layers.notify();
            }
        });

        Ok((out, inverse))
    }

    /// Read global + scope-chain layers in farthest-to-nearest order.
    pub fn with_chain<R>(&self, scope: Option<&ScopeKey>, f: impl FnOnce(&L, &[&L]) -> R) -> R {
        let guard = self.inner.read();
        let chain_keys = scope.map(ScopeKey::chain).unwrap_or_default();
        let refs: Vec<&L> = chain_keys
            .iter()
            .filter_map(|k| guard.scoped.get(k))
            .collect();
        f(&guard.global, &refs)
    }

    /// Merge named entries from Global -> farthest -> nearest so nearer scopes shadow globals.
    pub fn merge_named<V: Clone>(
        &self,
        scope: Option<&ScopeKey>,
        select: impl Fn(&L) -> &NamedEntries<V>,
    ) -> BTreeMap<String, V> {
        self.with_chain(scope, |global, chain| {
            let mut merged = BTreeMap::new();
            for (k, v) in select(global).iter() {
                merged.insert(k.to_owned(), v.clone());
            }
            for layer in chain {
                for (k, v) in select(layer).iter() {
                    merged.insert(k.to_owned(), v.clone());
                }
            }
            merged
        })
    }

    /// Drop an entire scope explicitly (e.g. when a session or subagent terminates).
    pub fn clear_scope(&self, scope: &ScopeKey) {
        let removed = self.inner.write().scoped.remove(scope).is_some();
        if removed {
            self.notify();
        }
    }
}
