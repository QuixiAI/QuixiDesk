/* Copyright (c) 2026 QuixiAI. MIT license.
 * Spatiotemporal Client Plugin Kernel & Typed Slot Registry (`cordis-runtime.js`).
 * Mirrors `crates/quixi-cordis` and `dsh-client-ui-slots` in lightweight browser JS.
 */
(() => {
  "use strict";

  class SlotRegistry {
    constructor(onChange) {
      this._slots = new Map();
      this._seq = 0;
      this._onChange = onChange;
      // Pre-declare core application extension slots:
      this.declare("sidebar.nav", "list", "root");
      this.declare("main.page", "keyed", "root");
      this.declare("composer.controls", "list", "root");
      this.declare("composer.hero", "list", "root");
      this.declare("composer.footer", "list", "root");
      this.declare("session.heading.extras", "list", "root");
      this.declare("conversation.views", "list", "root");
      this.declare("chat.turn-tail", "list", "root");
      this.declare("session.menu.actions", "list", "root");
      this.declare("sidebar.right.tab", "list", "root");
      this.declare("settings.sections", "list", "root");
      this.declare("plugins.row.config", "keyed", "ui-plugin-manager");
    }

    declare(name, kind = "list", ownerPlugin = "root") {
      if (this._slots.has(name)) {
        const existing = this._slots.get(name);
        if (existing.kind !== kind) {
          throw new Error(`Slot "${name}" already declared with kind "${existing.kind}"`);
        }
        return () => {};
      }
      this._slots.set(name, {
        name,
        kind, // "single" | "list" | "keyed" | "chain"
        ownerPlugin,
        entries: [],
      });
      this._notify();
      return () => {
        this._slots.delete(name);
        this._notify();
      };
    }

    register(slotName, entry, ownerPlugin = "anonymous") {
      let slot = this._slots.get(slotName);
      if (!slot) {
        slot = { name: slotName, kind: entry.key ? "keyed" : "list", ownerPlugin: "dynamic", entries: [] };
        this._slots.set(slotName, slot);
      }
      if (slot.kind === "single" && slot.entries.length > 0) {
        throw new Error(`Single slot "${slotName}" already has an occupant`);
      }
      if (slot.kind === "chain" && typeof entry.select !== "function") {
        throw new Error(`Chain slot "${slotName}" requires a select(props) function`);
      }
      const id = ++this._seq;
      const record = {
        id,
        ownerPlugin,
        order: Number.isFinite(entry.order) ? entry.order : 100,
        priority: Number.isFinite(entry.priority) ? entry.priority : 100,
        key: entry.key ?? null,
        label: entry.label ?? null,
        select: entry.select ?? null,
        render: entry.render,
        meta: entry.meta ?? {},
      };
      slot.entries.push(record);
      slot.entries.sort((a, b) => (a.order - b.order) || (a.priority - b.priority) || (a.id - b.id));
      this._notify();
      return () => {
        const current = this._slots.get(slotName);
        if (!current) return;
        const idx = current.entries.findIndex((r) => r.id === id);
        if (idx >= 0) {
          current.entries.splice(idx, 1);
          this._notify();
        }
      };
    }

    entries(slotName) {
      return (this._slots.get(slotName)?.entries || []).slice();
    }

    renderList(slotName, props = {}) {
      const nodes = [];
      for (const rec of this.entries(slotName)) {
        try {
          const out = rec.render(props, rec);
          if (Array.isArray(out)) {
            for (const item of out) if (item) nodes.push(item);
          } else if (out) {
            nodes.push(out);
          }
        } catch (err) {
          console.warn(`[QuixiSlots] render failed in ${slotName} (${rec.ownerPlugin}):`, err);
        }
      }
      return nodes;
    }

    renderKeyed(slotName, key, props = {}) {
      const rec = this.entries(slotName).find((r) => r.key === key);
      if (!rec) return null;
      try {
        return rec.render(props, rec);
      } catch (err) {
        console.warn(`[QuixiSlots] keyed render failed in ${slotName}[${key}]:`, err);
        return null;
      }
    }

    renderChain(slotName, props = {}, fallback = null) {
      for (const rec of this.entries(slotName)) {
        const matched = rec.select ? rec.select(props) : null;
        if (matched !== null && matched !== undefined && matched !== false) {
          return rec.render({ ...props, matched }, rec);
        }
      }
      return typeof fallback === "function" ? fallback(props) : fallback;
    }

    inspect() {
      const slots = [];
      for (const s of this._slots.values()) {
        slots.push({
          name: s.name,
          kind: s.kind,
          ownerPlugin: s.ownerPlugin,
          occupantCount: s.entries.length,
          occupants: s.entries.map((e) => ({
            ownerPlugin: e.ownerPlugin,
            key: e.key,
            label: e.label,
            order: e.order,
          })),
        });
      }
      return slots;
    }

    _notify() {
      if (typeof this._onChange === "function") this._onChange();
    }
  }

  class ClientCordisRuntime {
    constructor() {
      this._fibers = new Map();
      this._services = new Map();
      this._events = new Map();
      this._waterfalls = new Map();
      this._hostSnapshot = null;
      this._activeHostRows = new Set();
      this._dynamicBundleIds = new Set();
      this._appBridge = null;
      this._rerenderScheduled = false;
      this.slots = new SlotRegistry(() => this.requestRerender());
    }

    attachAppBridge(bridge) {
      this._appBridge = bridge;
      this.quiesce();
      this._syncClientManifestToHost();
    }

    app() {
      return this._appBridge;
    }

    hostSnapshot() {
      return this._hostSnapshot;
    }

    isHostRowActive(rowId) {
      if (!this._hostSnapshot) return true;
      return this._activeHostRows.has(rowId);
    }

    requestRerender() {
      if (this._rerenderScheduled || !this._appBridge?.onSlotsChanged) return;
      this._rerenderScheduled = true;
      queueMicrotask(() => {
        this._rerenderScheduled = false;
        try {
          this._appBridge?.onSlotsChanged();
        } catch (err) {
          console.warn("[QuixiCordis] UI refresh diagnostic:", err);
        }
      });
    }

    definePlugin(spec) {
      if (!spec || typeof spec.id !== "string" || !spec.id.trim()) {
        throw new Error("QuixiCordis.definePlugin requires a non-empty `id`");
      }
      const id = spec.id.trim();
      if (this._fibers.has(id)) {
        this.undefinePlugin(id);
      }
      const fiber = {
        id,
        title: spec.title || id,
        hostRowId: spec.hostRowId || null,
        inject: Array.isArray(spec.inject) ? spec.inject.slice() : [],
        provide: Array.isArray(spec.provide) ? spec.provide.slice() : [],
        setup: spec.setup,
        disabled: Boolean(spec.disabled),
        state: "inactive", // "inactive" | "active" | "unloading" | "failed"
        error: null,
        committed: new Map(),
        disposeStack: [],
        config: spec.config || {},
      };
      this._fibers.set(id, fiber);
      this.quiesce();
      return () => this.undefinePlugin(id);
    }

    undefinePlugin(id) {
      const f = this._fibers.get(id);
      if (!f) return;
      f.retired = true;
      this.quiesce();
      this._fibers.delete(id);
    }

    _targetFor(fiber) {
      if (fiber.retired || fiber.disabled) return null;
      if (fiber.hostRowId && this._hostSnapshot && !this._activeHostRows.has(fiber.hostRowId)) {
        return null;
      }
      const resolved = new Map();
      for (const svc of fiber.inject) {
        const provider = this._services.get(svc);
        if (!provider) return null;
        const pFiber = this._fibers.get(provider.ownerId);
        if (!pFiber || pFiber.state !== "active" || pFiber.retired || pFiber.disabled) {
          return null;
        }
        resolved.set(svc, provider.ownerId);
      }
      return resolved;
    }

    _sameCommitted(a, b) {
      if (!a || !b || a.size !== b.size) return false;
      for (const [k, v] of a.entries()) {
        if (b.get(k) !== v) return false;
      }
      return true;
    }

    _isReliedUpon(id) {
      for (const other of this._fibers.values()) {
        if (other.id !== id && (other.state === "active" || other.state === "unloading")) {
          for (const providerId of other.committed.values()) {
            if (providerId === id) return true;
          }
        }
      }
      return false;
    }

    _makeContext(fiber) {
      const rt = this;
      return {
        id: fiber.id,
        app: () => rt._appBridge,
        slots: rt.slots,
        effect(label, fn) {
          const inv = fn();
          if (typeof inv === "function") {
            fiber.disposeStack.push({ label: String(label || "effect"), inverse: inv });
          }
          return inv;
        },
        provide(name, value) {
          if (!fiber.provide.includes(name)) {
            throw new Error(`Undeclared client provision "${name}" in "${fiber.id}"`);
          }
          rt._services.set(name, { ownerId: fiber.id, value });
          fiber.disposeStack.push({
            label: `provide(${name})`,
            inverse: () => {
              if (rt._services.get(name)?.ownerId === fiber.id) {
                rt._services.delete(name);
              }
            },
          });
        },
        get(name) {
          if (!fiber.inject.includes(name) && !fiber.provide.includes(name)) {
            throw new Error(`Undeclared client service access "${name}" from "${fiber.id}"`);
          }
          return rt._services.get(name)?.value;
        },
        slot(slotName, entry) {
          const off = rt.slots.register(slotName, entry, fiber.id);
          fiber.disposeStack.push({ label: `slot(${slotName})`, inverse: off });
          return off;
        },
        on(event, fn) {
          let list = rt._events.get(event);
          if (!list) {
            list = [];
            rt._events.set(event, list);
          }
          const rec = { ownerId: fiber.id, fn };
          list.push(rec);
          const off = () => {
            const cur = rt._events.get(event);
            if (!cur) return;
            const i = cur.indexOf(rec);
            if (i >= 0) cur.splice(i, 1);
          };
          fiber.disposeStack.push({ label: `on(${event})`, inverse: off });
          return off;
        },
        onWaterfall(event, priority, fn) {
          let list = rt._waterfalls.get(event);
          if (!list) {
            list = [];
            rt._waterfalls.set(event, list);
          }
          const rec = { ownerId: fiber.id, priority: priority ?? 100, fn };
          list.push(rec);
          list.sort((a, b) => a.priority - b.priority);
          const off = () => {
            const cur = rt._waterfalls.get(event);
            if (!cur) return;
            const i = cur.indexOf(rec);
            if (i >= 0) cur.splice(i, 1);
          };
          fiber.disposeStack.push({ label: `waterfall(${event})`, inverse: off });
          return off;
        },
        registerPersonalizedSurface(spec) {
          const placement = spec.placement || "sidebar-right";
          const pageKey = `custom:${spec.id}`;
          const renderBody = (ui) => {
            const { el, plainBody } = ui;
            return el(
              "section",
              { class: "cordis-personalized-surface" },
              el("header", { class: "cordis-surface-head" },
                el("span", { class: "cordis-badge experimental" }, "Personalized Plugin"),
                el("h2", {}, spec.title || spec.id),
                spec.brief ? el("p", { class: "hint" }, spec.brief) : null,
              ),
              plainBody ? plainBody(spec.details || "") : el("pre", {}, spec.details || ""),
            );
          };
          if (placement === "page" || placement === "sidebar-left") {
            const offNav = rt.slots.register(
              "sidebar.nav",
              {
                order: 85,
                label: spec.title || spec.id,
                render: ({ navigationButton, state, navigate }) =>
                  navigationButton(
                    spec.title || spec.id,
                    "spark",
                    () => navigate(pageKey),
                    state.page === pageKey ? "active" : "",
                  ),
              },
              fiber.id,
            );
            const offPage = rt.slots.register(
              "main.page",
              {
                key: pageKey,
                label: spec.title || spec.id,
                render: (ui) => renderBody(ui),
              },
              fiber.id,
            );
            fiber.disposeStack.push({
              label: `personalized(${pageKey})`,
              inverse: () => {
                offPage();
                offNav();
              },
            });
          } else {
            const offTab = rt.slots.register(
              "sidebar.right.tab",
              {
                key: spec.id,
                label: spec.title || spec.id,
                order: 50,
                render: (ui) => renderBody(ui),
              },
              fiber.id,
            );
            fiber.disposeStack.push({ label: `personalized-tab(${spec.id})`, inverse: offTab });
          }
        },
      };
    }

    quiesce() {
      let anyChanged = false;
      for (let step = 0; step < 512; step++) {
        // 1. L-Leave: active fiber whose target changed -> unloading
        let progressed = false;
        for (const f of this._fibers.values()) {
          if (f.state === "active") {
            const target = this._targetFor(f);
            if (!this._sameCommitted(target, f.committed)) {
              f.state = "unloading";
              progressed = true;
              anyChanged = true;
              break;
            }
          }
        }
        if (progressed) continue;

        // 2. L-Unload (!relied): unwind LIFO disposeStack
        for (const f of this._fibers.values()) {
          if (f.state === "unloading" && !this._isReliedUpon(f.id)) {
            while (f.disposeStack.length > 0) {
              const item = f.disposeStack.pop();
              try {
                item.inverse();
              } catch (err) {
                console.warn(`[QuixiCordis] inverse error in ${f.id} (${item.label}):`, err);
              }
            }
            f.committed.clear();
            f.state = "inactive";
            progressed = true;
            anyChanged = true;
            break;
          }
        }
        if (progressed) continue;

        // 3. L-Begin -> L-Finish: inactive fiber whose target is satisfied
        for (const f of this._fibers.values()) {
          if (f.state === "inactive") {
            const target = this._targetFor(f);
            if (target !== null) {
              f.committed = target;
              const ctx = this._makeContext(f);
              try {
                if (typeof f.setup === "function") {
                  f.setup(ctx, f.config);
                }
                f.state = "active";
                f.error = null;
              } catch (err) {
                while (f.disposeStack.length > 0) {
                  try { f.disposeStack.pop().inverse(); } catch {}
                }
                f.committed.clear();
                f.state = "failed";
                f.error = err?.message || String(err);
              }
              progressed = true;
              anyChanged = true;
              break;
            }
          }
        }
        if (!progressed) break;
      }
      if (anyChanged) this.requestRerender();
    }

    emit(event, payload) {
      for (const rec of this._events.get(event) || []) {
        try { rec.fn(payload); } catch (e) { console.warn(e); }
      }
    }

    waterfall(event, initial, terminal) {
      const list = (this._waterfalls.get(event) || []).slice();
      const dispatchAt = (idx, val) => {
        if (idx >= list.length) return typeof terminal === "function" ? terminal(val) : val;
        return list[idx].fn(val, (nextVal) => dispatchAt(idx + 1, nextVal));
      };
      return dispatchAt(0, initial);
    }

    syncFromHostSnapshot(snap) {
      if (!snap || typeof snap !== "object") return;
      this._hostSnapshot = snap;
      this._activeHostRows = new Set(Array.isArray(snap.activeRowIds) ? snap.activeRowIds : []);

      // Reconcile dynamic user/Creator client bundles:
      const incomingIds = new Set();
      for (const b of snap.activeClientBundles || []) {
        if (!b || !b.id) continue;
        const dynId = `user-client:${b.id}`;
        incomingIds.add(dynId);
        if (!this._fibers.has(dynId)) {
          if (typeof b.clientScript === "string" && b.clientScript.trim()) {
            try {
              const fn = new Function(b.clientScript);
              fn();
              // Also track by bundle id if script registered under `b.id` directly:
              this._dynamicBundleIds.add(b.id);
              incomingIds.add(b.id);
            } catch (err) {
              console.warn(`[QuixiCordis] dynamic client bundle "${b.id}" failed:`, err);
            }
          } else if (b.declarative?.placement) {
            this.definePlugin({
              id: dynId,
              title: b.title || b.id,
              setup(ctx) {
                ctx.registerPersonalizedSurface({
                  id: b.id,
                  title: b.title || b.id,
                  brief: "",
                  details: JSON.stringify(b.declarative, null, 2),
                  placement: b.declarative.placement,
                });
              },
            });
            this._dynamicBundleIds.add(dynId);
          }
        }
      }
      for (const oldId of Array.from(this._dynamicBundleIds)) {
        const stillActive =
          incomingIds.has(oldId) ||
          (snap.activeClientBundles || []).some((b) => b.id === oldId || `user-client:${b.id}` === oldId);
        if (!stillActive) {
          this.undefinePlugin(oldId);
          this._dynamicBundleIds.delete(oldId);
        }
      }

      this.quiesce();
      this._syncClientManifestToHost();
      this.emit("cordis/host-snapshot", snap);
    }

    inspectClient() {
      const fibers = [];
      for (const f of this._fibers.values()) {
        fibers.push({
          id: f.id,
          title: f.title,
          hostRowId: f.hostRowId,
          state: f.state,
          error: f.error,
          inject: f.inject,
          provide: f.provide,
          effectCount: f.disposeStack.length,
        });
      }
      return {
        fibers,
        services: Array.from(this._services.keys()),
        slots: this.slots.inspect(),
      };
    }

    _syncClientManifestToHost() {
      const rpc = this._appBridge?.rpc;
      if (typeof rpc !== "function") return;
      Promise.resolve(rpc("CordisPlugins", "reportClientManifest", [this.inspectClient()])).catch(() => {});
    }
  }

  const runtime = new ClientCordisRuntime();
  window.QuixiCordis = runtime;
  window.QuixiSlots = runtime.slots;
})();
