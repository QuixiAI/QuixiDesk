/* Copyright (c) 2026 QuixiAI. MIT license. Main-frame native transport. */
(() => {
  "use strict";
  const origin = "__QUIXI_APP_ORIGIN__";
  const listeners = (window.__QUIXI_LISTENERS__ = new Map());
  const api = async (path, body, method) => {
    const response = await fetch(origin + path, {
      method: method || (body === undefined ? "GET" : "POST"),
      headers: body === undefined ? {} : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const text = await response.text();
    let value;
    try {
      value = text ? JSON.parse(text) : null;
    } catch (_) {
      value = null;
    }
    if (!response.ok)
      throw new Error(
        typeof value?.error === "string"
          ? value.error
          : text || `Request failed (${response.status})`,
      );
    return value;
  };
  const on = (channel, callback) => {
    if (!listeners.has(channel)) listeners.set(channel, new Set());
    listeners.get(channel).add(callback);
    return () => listeners.get(channel)?.delete(callback);
  };
  window.__quixi_eipc_emit__ = (channel, payload) => {
    for (const callback of listeners.get(channel) || []) {
      try {
        callback(null, payload);
      } catch (error) {
        console.error(error);
      }
    }
    window.dispatchEvent(
      new CustomEvent("quixi:event", { detail: { channel, payload } }),
    );
  };
  const rpc = async (service, member, args = [], namespace = "quixi.web") => {
    const result = await api("/__eipc__/invoke", {
      channel: `$eipc_message$_6d3e7708-9b4b-4d8a-9274-060f783c110b_$_${namespace}_$_${service}_$_${member}`,
      args,
    });
    if (result?.error)
      throw new Error(
        typeof result.error === "string"
          ? result.error
          : JSON.stringify(result.error),
      );
    return result?.result;
  };
  const copy = async (text) => {
    const result = await api("/__eipc__/clipboard", {
      action: "write",
      text: String(text),
    });
    if (!result?.ok) throw new Error("Clipboard copy failed");
  };
  window.quixi = Object.freeze({ api, rpc, on, copy, origin });
  document.addEventListener("mousedown", (event) => {
    const target = event.target;
    if (
      event.button !== 0 ||
      !target.closest?.(".topbar,.brand") ||
      target.closest("button,a,input,textarea,select")
    )
      return;
    api("/__eipc__/window-action", {
      action: event.detail === 2 ? "toggle_maximize" : "drag",
    }).catch(() => {});
  });
  function controls() {
    const activeDialog = Array.from(
      document.querySelectorAll("dialog[open]"),
    ).at(-1);
    return Array.from(
      document.querySelectorAll(
        'button,a,input,textarea,select,[role="textbox"],summary',
      ),
    )
      .filter((el) => {
        if (activeDialog && !activeDialog.contains(el)) return false;
        const rect = el.getBoundingClientRect();
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          getComputedStyle(el).visibility !== "hidden"
        );
      })
      .map((el, i) => {
        el.dataset.quixiAxIdx = i + 1;
        const r = el.getBoundingClientRect();
        return {
          index: i + 1,
          tag: el.tagName.toLowerCase(),
          role: el.getAttribute("role") || el.tagName.toLowerCase(),
          text:
            el.getAttribute("aria-label") ||
            el.innerText ||
            el.placeholder ||
            "",
          placeholder: el.placeholder || "",
          x: r.x,
          y: r.y,
          width: r.width,
          height: r.height,
          centerX: r.x + r.width / 2,
          centerY: r.y + r.height / 2,
        };
      });
  }
  window.__quixi_automation__ = {
    getState() {
      return {
        ok: true,
        title: document.title,
        url: location.href,
        pathname: location.pathname,
        width: innerWidth,
        height: innerHeight,
        elements: controls(),
        elementsCount: controls().length,
        textSample: (document.body?.innerText || "")
          .replace(/\s+/g, " ")
          .slice(0, 1600),
      };
    },
    performAction(payload = {}) {
      const action = payload.action || "click";
      if (action === "eval" || action === "evaluate") {
        try {
          return {
            ok: true,
            action: "eval",
            result: (0, eval)(
              payload.script || payload.expression || payload.code,
            ),
          };
        } catch (error) {
          return { ok: false, error: String(error) };
        }
      }
      if (action === "navigate") {
        window.quixiNavigate?.(payload.path || "/new");
        return { ok: true };
      }
      controls();
      let el = payload.selector
        ? document.querySelector(payload.selector)
        : document.querySelector(
            `[data-quixi-ax-idx="${payload.element_index || payload.index || 0}"]`,
          );
      if (!el && (payload.query || payload.text)) {
        const q = String(payload.query || payload.text).toLowerCase();
        const row = controls().find((r) => r.text.toLowerCase().includes(q));
        if (row)
          el = document.querySelector(`[data-quixi-ax-idx="${row.index}"]`);
      }
      if (!el && typeof payload.x === "number")
        el = document.elementFromPoint(payload.x, payload.y);
      if (action === "type") {
        el ||= document.activeElement;
        const setter = Object.getOwnPropertyDescriptor(
          el instanceof HTMLTextAreaElement
            ? HTMLTextAreaElement.prototype
            : HTMLInputElement.prototype,
          "value",
        )?.set;
        if (!setter) return { ok: false, error: "No text input" };
        setter.call(el, String(payload.text || ""));
        el.dispatchEvent(new Event("input", { bubbles: true }));
      } else if (action === "key") {
        el ||= document.activeElement;
        const key = payload.key === "Return" ? "Enter" : payload.key;
        el.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
        el.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true }));
      } else if (el) el.click();
      else return { ok: false, error: "No matching control" };
      return { ok: true };
    },
  };
})();
