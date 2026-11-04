//! Scoped Cordis event bus supporting all 5 dispatch modes:
//! `emit`, `parallel`, `serial`, `bail`, and `waterfall` (around-middleware).

use crate::scope::ScopeKey;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventDispatchMode {
    Emit,
    Parallel,
    Serial,
    Bail,
    Waterfall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSpecInfo {
    pub name: String,
    pub mode: EventDispatchMode,
    pub listener_count: usize,
    pub owners: Vec<String>,
}

type EmitFn = Arc<dyn Fn(&Value) + Send + Sync>;
type ParallelFn = Arc<dyn Fn(&Value) -> Value + Send + Sync>;
type SerialFn = Arc<dyn Fn(&mut Value) + Send + Sync>;
type BailFn = Arc<dyn Fn(&Value) -> Option<Value> + Send + Sync>;
pub type NextWaterfall<'a> = &'a mut dyn FnMut(Value) -> Value;
type WaterfallFn = Arc<dyn Fn(Value, NextWaterfall<'_>) -> Value + Send + Sync>;

struct ListenerRecord<F> {
    id: u64,
    priority: i32,
    scope: Option<ScopeKey>,
    owner_plugin: String,
    callback: F,
}

#[derive(Default)]
struct EventBusInner {
    emit: HashMap<String, Vec<ListenerRecord<EmitFn>>>,
    parallel: HashMap<String, Vec<ListenerRecord<ParallelFn>>>,
    serial: HashMap<String, Vec<ListenerRecord<SerialFn>>>,
    bail: HashMap<String, Vec<ListenerRecord<BailFn>>>,
    waterfall: HashMap<String, Vec<ListenerRecord<WaterfallFn>>>,
}

#[derive(Clone, Default)]
pub struct EventBus {
    inner: Arc<RwLock<EventBusInner>>,
    next_id: Arc<AtomicU64>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn register_emit(
        &self,
        event: impl Into<String>,
        scope: Option<ScopeKey>,
        priority: i32,
        owner_plugin: impl Into<String>,
        cb: impl Fn(&Value) + Send + Sync + 'static,
    ) -> Box<dyn FnOnce() + Send> {
        let event = event.into();
        let id = self.alloc_id();
        {
            let mut guard = self.inner.write();
            let list = guard.emit.entry(event.clone()).or_default();
            list.push(ListenerRecord {
                id,
                priority,
                scope,
                owner_plugin: owner_plugin.into(),
                callback: Arc::new(cb),
            });
            list.sort_by_key(|r| (r.priority, r.id));
        }
        let bus = self.clone();
        Box::new(move || {
            let mut guard = bus.inner.write();
            if let Some(list) = guard.emit.get_mut(&event) {
                list.retain(|r| r.id != id);
                if list.is_empty() {
                    guard.emit.remove(&event);
                }
            }
        })
    }

    pub fn emit(&self, event: &str, scope: Option<&ScopeKey>, payload: &Value) {
        let callbacks: Vec<EmitFn> = {
            let guard = self.inner.read();
            guard
                .emit
                .get(event)
                .map(|list| {
                    list.iter()
                        .filter(|r| ScopeKey::matches_dispatch(r.scope.as_ref(), scope))
                        .map(|r| r.callback.clone())
                        .collect()
                })
                .unwrap_or_default()
        };
        for cb in callbacks {
            cb(payload);
        }
    }

    pub fn register_parallel(
        &self,
        event: impl Into<String>,
        scope: Option<ScopeKey>,
        priority: i32,
        owner_plugin: impl Into<String>,
        cb: impl Fn(&Value) -> Value + Send + Sync + 'static,
    ) -> Box<dyn FnOnce() + Send> {
        let event = event.into();
        let id = self.alloc_id();
        {
            let mut guard = self.inner.write();
            let list = guard.parallel.entry(event.clone()).or_default();
            list.push(ListenerRecord {
                id,
                priority,
                scope,
                owner_plugin: owner_plugin.into(),
                callback: Arc::new(cb),
            });
            list.sort_by_key(|r| (r.priority, r.id));
        }
        let bus = self.clone();
        Box::new(move || {
            let mut guard = bus.inner.write();
            if let Some(list) = guard.parallel.get_mut(&event) {
                list.retain(|r| r.id != id);
                if list.is_empty() {
                    guard.parallel.remove(&event);
                }
            }
        })
    }

    pub fn parallel(&self, event: &str, scope: Option<&ScopeKey>, payload: &Value) -> Vec<Value> {
        let callbacks: Vec<ParallelFn> = {
            let guard = self.inner.read();
            guard
                .parallel
                .get(event)
                .map(|list| {
                    list.iter()
                        .filter(|r| ScopeKey::matches_dispatch(r.scope.as_ref(), scope))
                        .map(|r| r.callback.clone())
                        .collect()
                })
                .unwrap_or_default()
        };
        if callbacks.len() <= 1 {
            return callbacks.into_iter().map(|cb| cb(payload)).collect();
        }
        std::thread::scope(|s| {
            let handles: Vec<_> = callbacks
                .into_iter()
                .map(|cb| s.spawn(move || cb(payload)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or(Value::Null))
                .collect()
        })
    }

    pub fn register_serial(
        &self,
        event: impl Into<String>,
        scope: Option<ScopeKey>,
        priority: i32,
        owner_plugin: impl Into<String>,
        cb: impl Fn(&mut Value) + Send + Sync + 'static,
    ) -> Box<dyn FnOnce() + Send> {
        let event = event.into();
        let id = self.alloc_id();
        {
            let mut guard = self.inner.write();
            let list = guard.serial.entry(event.clone()).or_default();
            list.push(ListenerRecord {
                id,
                priority,
                scope,
                owner_plugin: owner_plugin.into(),
                callback: Arc::new(cb),
            });
            list.sort_by_key(|r| (r.priority, r.id));
        }
        let bus = self.clone();
        Box::new(move || {
            let mut guard = bus.inner.write();
            if let Some(list) = guard.serial.get_mut(&event) {
                list.retain(|r| r.id != id);
                if list.is_empty() {
                    guard.serial.remove(&event);
                }
            }
        })
    }

    pub fn serial(&self, event: &str, scope: Option<&ScopeKey>, payload: &mut Value) {
        let callbacks: Vec<SerialFn> = {
            let guard = self.inner.read();
            guard
                .serial
                .get(event)
                .map(|list| {
                    list.iter()
                        .filter(|r| ScopeKey::matches_dispatch(r.scope.as_ref(), scope))
                        .map(|r| r.callback.clone())
                        .collect()
                })
                .unwrap_or_default()
        };
        for cb in callbacks {
            cb(payload);
        }
    }

    pub fn register_bail(
        &self,
        event: impl Into<String>,
        scope: Option<ScopeKey>,
        priority: i32,
        owner_plugin: impl Into<String>,
        cb: impl Fn(&Value) -> Option<Value> + Send + Sync + 'static,
    ) -> Box<dyn FnOnce() + Send> {
        let event = event.into();
        let id = self.alloc_id();
        {
            let mut guard = self.inner.write();
            let list = guard.bail.entry(event.clone()).or_default();
            list.push(ListenerRecord {
                id,
                priority,
                scope,
                owner_plugin: owner_plugin.into(),
                callback: Arc::new(cb),
            });
            list.sort_by_key(|r| (r.priority, r.id));
        }
        let bus = self.clone();
        Box::new(move || {
            let mut guard = bus.inner.write();
            if let Some(list) = guard.bail.get_mut(&event) {
                list.retain(|r| r.id != id);
                if list.is_empty() {
                    guard.bail.remove(&event);
                }
            }
        })
    }

    pub fn bail(&self, event: &str, scope: Option<&ScopeKey>, payload: &Value) -> Option<Value> {
        let callbacks: Vec<BailFn> = {
            let guard = self.inner.read();
            guard
                .bail
                .get(event)
                .map(|list| {
                    list.iter()
                        .filter(|r| ScopeKey::matches_dispatch(r.scope.as_ref(), scope))
                        .map(|r| r.callback.clone())
                        .collect()
                })
                .unwrap_or_default()
        };
        for cb in callbacks {
            if let Some(res) = cb(payload) {
                return Some(res);
            }
        }
        None
    }

    pub fn register_waterfall(
        &self,
        event: impl Into<String>,
        scope: Option<ScopeKey>,
        priority: i32,
        owner_plugin: impl Into<String>,
        cb: impl Fn(Value, NextWaterfall<'_>) -> Value + Send + Sync + 'static,
    ) -> Box<dyn FnOnce() + Send> {
        let event = event.into();
        let id = self.alloc_id();
        {
            let mut guard = self.inner.write();
            let list = guard.waterfall.entry(event.clone()).or_default();
            list.push(ListenerRecord {
                id,
                priority,
                scope,
                owner_plugin: owner_plugin.into(),
                callback: Arc::new(cb),
            });
            list.sort_by_key(|r| (r.priority, r.id));
        }
        let bus = self.clone();
        Box::new(move || {
            let mut guard = bus.inner.write();
            if let Some(list) = guard.waterfall.get_mut(&event) {
                list.retain(|r| r.id != id);
                if list.is_empty() {
                    guard.waterfall.remove(&event);
                }
            }
        })
    }

    pub fn waterfall<T>(
        &self,
        event: &str,
        scope: Option<&ScopeKey>,
        initial: Value,
        mut terminal: T,
    ) -> Value
    where
        T: FnMut(Value) -> Value,
    {
        let callbacks: Vec<WaterfallFn> = {
            let guard = self.inner.read();
            guard
                .waterfall
                .get(event)
                .map(|list| {
                    list.iter()
                        .filter(|r| ScopeKey::matches_dispatch(r.scope.as_ref(), scope))
                        .map(|r| r.callback.clone())
                        .collect()
                })
                .unwrap_or_default()
        };

        fn dispatch_at<T: FnMut(Value) -> Value>(
            idx: usize,
            cbs: &[WaterfallFn],
            val: Value,
            terminal: &mut T,
        ) -> Value {
            if idx >= cbs.len() {
                return terminal(val);
            }
            let current = &cbs[idx];
            let mut next_fn = |next_val: Value| dispatch_at(idx + 1, cbs, next_val, terminal);
            current(val, &mut next_fn)
        }

        dispatch_at(0, &callbacks, initial, &mut terminal)
    }

    pub fn inspect_events(&self) -> Vec<EventSpecInfo> {
        let guard = self.inner.read();
        let mut out = BTreeMap::new();
        for (k, v) in &guard.emit {
            out.insert(
                format!("emit:{k}"),
                EventSpecInfo {
                    name: k.clone(),
                    mode: EventDispatchMode::Emit,
                    listener_count: v.len(),
                    owners: v.iter().map(|r| r.owner_plugin.clone()).collect(),
                },
            );
        }
        for (k, v) in &guard.parallel {
            out.insert(
                format!("parallel:{k}"),
                EventSpecInfo {
                    name: k.clone(),
                    mode: EventDispatchMode::Parallel,
                    listener_count: v.len(),
                    owners: v.iter().map(|r| r.owner_plugin.clone()).collect(),
                },
            );
        }
        for (k, v) in &guard.serial {
            out.insert(
                format!("serial:{k}"),
                EventSpecInfo {
                    name: k.clone(),
                    mode: EventDispatchMode::Serial,
                    listener_count: v.len(),
                    owners: v.iter().map(|r| r.owner_plugin.clone()).collect(),
                },
            );
        }
        for (k, v) in &guard.bail {
            out.insert(
                format!("bail:{k}"),
                EventSpecInfo {
                    name: k.clone(),
                    mode: EventDispatchMode::Bail,
                    listener_count: v.len(),
                    owners: v.iter().map(|r| r.owner_plugin.clone()).collect(),
                },
            );
        }
        for (k, v) in &guard.waterfall {
            out.insert(
                format!("waterfall:{k}"),
                EventSpecInfo {
                    name: k.clone(),
                    mode: EventDispatchMode::Waterfall,
                    listener_count: v.len(),
                    owners: v.iter().map(|r| r.owner_plugin.clone()).collect(),
                },
            );
        }
        out.into_values().collect()
    }
}
