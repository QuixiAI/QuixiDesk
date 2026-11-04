//! Typed coeffect keys, realm isolation (`isolate`), and capability monoid interception (`intercept`).
//!
//! Realizes Section 3.2 and Section 4.4 of *A Programming Paradigm for Spatiotemporal Composability*.

use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::collections::BTreeSet;
use std::fmt;
use std::marker::PhantomData;

/// Typed service coeffect key. Equivalent to a declaration-merged `Context` property in TypeScript Cordis.
pub struct ServiceKey<T: Send + Sync + 'static> {
    pub name: &'static str,
    pub description: &'static str,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Send + Sync + 'static> Copy for ServiceKey<T> {}
impl<T: Send + Sync + 'static> Clone for ServiceKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> fmt::Debug for ServiceKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceKey")
            .field("name", &self.name)
            .field("type_id", &TypeId::of::<T>())
            .finish()
    }
}

impl<T: Send + Sync + 'static> ServiceKey<T> {
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            _marker: PhantomData,
        }
    }

    pub fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

/// Canonical coeffect realm identifier (`Definition 24 & Section 4.4 Isolation`).
///
/// By default, every key `k` resolves on the shared diagonal realm `RealmId::Shared`.
/// Calling `ctx.isolate(key, RealmId::Named(...))` rebinds `(k, rho(k))` for the sub-context
/// and its descendants so multiple providers of `fs`, `shell`, or `sandbox` coexist cleanly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "name", rename_all = "kebab-case")]
pub enum RealmId {
    #[default]
    Shared,
    Named(String),
}

impl RealmId {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }
}

/// Associative monoid $(\mathcal{M}_k, \oplus_k, \epsilon_k)$ for capability interception (`Definition 26`).
///
/// Outer contexts attach metadata `m1`, inner contexts attach `m2`, and consumers observe `m1.combine(&m2)`
/// so enclosing scopes can attenuate permissions (e.g. `ReadWrite` -> `ReadOnly`) without provider awareness.
pub trait CapabilityMonoid: Clone + Send + Sync + 'static {
    fn identity() -> Self;
    fn combine(&self, inner: &Self) -> Self;
}

/// Typed interception key paired with a `CapabilityMonoid`.
pub struct InterceptionKey<M: CapabilityMonoid> {
    pub name: &'static str,
    _marker: PhantomData<fn() -> M>,
}

impl<M: CapabilityMonoid> Copy for InterceptionKey<M> {}
impl<M: CapabilityMonoid> Clone for InterceptionKey<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: CapabilityMonoid> InterceptionKey<M> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _marker: PhantomData,
        }
    }
}

/// Built-in lattice monoid for filesystem/workspace access attenuation:
/// `Full` -> `WorkspaceWrite` -> `ReadOnly` -> `Denied` (meet operation: min privilege wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessLevel {
    Denied = 0,
    ReadOnly = 1,
    WorkspaceWrite = 2,
    Full = 3,
}

impl CapabilityMonoid for AccessLevel {
    fn identity() -> Self {
        Self::Full
    }

    fn combine(&self, inner: &Self) -> Self {
        (*self).min(*inner)
    }
}

/// Built-in monoid for tool visibility attenuation across scopes (`allow` intersection, `deny` union).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<BTreeSet<String>>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub deny: BTreeSet<String>,
}

impl ToolFilter {
    pub fn allow_only<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            allow: Some(names.into_iter().map(Into::into).collect()),
            deny: BTreeSet::new(),
        }
    }

    pub fn deny_names<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            allow: None,
            deny: names.into_iter().map(Into::into).collect(),
        }
    }

    pub fn admits(&self, tool_name: &str) -> bool {
        if self.deny.contains(tool_name) {
            return false;
        }
        match &self.allow {
            Some(allowed) => allowed.contains(tool_name),
            None => true,
        }
    }
}

impl CapabilityMonoid for ToolFilter {
    fn identity() -> Self {
        Self::default()
    }

    fn combine(&self, inner: &Self) -> Self {
        let allow = match (&self.allow, &inner.allow) {
            (None, None) => None,
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b.clone()),
            (Some(a), Some(b)) => Some(a.intersection(b).cloned().collect()),
        };
        let deny = self.deny.union(&inner.deny).cloned().collect();
        Self { allow, deny }
    }
}

pub const FS_ACCESS_INTERCEPTION: InterceptionKey<AccessLevel> =
    InterceptionKey::new("fs:access-level");
pub const SHELL_ACCESS_INTERCEPTION: InterceptionKey<AccessLevel> =
    InterceptionKey::new("shell:access-level");
pub const TOOL_FILTER_INTERCEPTION: InterceptionKey<ToolFilter> =
    InterceptionKey::new("tools:filter");
