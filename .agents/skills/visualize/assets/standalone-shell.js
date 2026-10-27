(() => {
  const frame = document.getElementById("codex-visualization");
  if (!(frame instanceof HTMLIFrameElement)) {
    return;
  }
  const isShare = document.documentElement.classList.contains(
    "visualization-share-root",
  );
  let viewportHeight = globalThis.innerHeight;
  let sizeFingerprint = null;
  let sizeWriteTimeout;
  let lastSize = null;
  let pendingSize = null;
  const storageKey = `codex:visualization-widget-state-v2:${JSON.stringify([
    globalThis.location.pathname,
    globalThis.location.search,
  ])}`;
  const sizeStorageKey = `${storageKey}:share-size-v1`;
  const parseState = (serializedState) => {
    if (
      typeof serializedState !== "string" ||
      new TextEncoder().encode(serializedState).length > 16 * 1024
    ) {
      throw new TypeError(
        "Widget state must be a JSON object of at most 16 KiB",
      );
    }
    /** @type {unknown} */
    const state = JSON.parse(serializedState);
    if (
      state == null ||
      typeof state !== "object" ||
      Array.isArray(state) ||
      ("imageIds" in state &&
        (!Array.isArray(state.imageIds) || state.imageIds.length > 0))
    ) {
      throw new TypeError("Widget state must be a JSON object");
    }
    const storedState = { modelContent: null, privateContent: null, ...state };
    const canonicalState = JSON.stringify(
      storedState,
      (_key, /** @type {unknown} */ value) => {
        if (typeof value === "number" && !Number.isFinite(value)) {
          throw new TypeError("Widget state must contain finite numbers");
        }
        return value;
      },
    );
    if (new TextEncoder().encode(canonicalState).length > 16 * 1024) {
      throw new TypeError(
        "Widget state must be a JSON object of at most 16 KiB",
      );
    }
    return storedState;
  };
  let storage = null;
  let state = null;
  try {
    const localStorage = globalThis.localStorage;
    const probeKey = `${storageKey}:probe`;
    localStorage.setItem(probeKey, "1");
    localStorage.removeItem(probeKey);
    storage = localStorage;
    const savedState = storage.getItem(storageKey);
    if (savedState != null) {
      try {
        state = parseState(savedState);
      } catch {
        // Invalid saved state does not disable future writes.
      }
    }
  } catch {
    storage = null;
  }
  const persistSize = () => {
    globalThis.clearTimeout(sizeWriteTimeout);
    if (pendingSize == null) {
      return;
    }
    try {
      storage?.setItem(sizeStorageKey, pendingSize);
    } catch {
      // A denied or full cache must not affect the visualization.
    }
    pendingSize = null;
  };
  if (isShare) {
    globalThis.addEventListener("pagehide", persistSize);
  }
  const initialGlobals = {
    widgetState: state,
    statePersistence: storage == null ? "none" : "local",
  };

  const channel = new MessageChannel();
  channel.port1.addEventListener("message", (event) => {
    const data = event.data;
    if (typeof data !== "object" || data == null) {
      return;
    }
    if (data.type === "widget-state-write") {
      if (!Number.isSafeInteger(data.id) || data.id <= 0) {
        return;
      }
      let ok = false;
      try {
        if (storage != null) {
          const state = parseState(data.state);
          storage.setItem(storageKey, JSON.stringify(state));
          ok = true;
        }
      } catch {
        // Storage denial, quota limits, and invalid state return a failed receipt.
      }
      channel.port1.postMessage({
        type: "widget-state-result",
        id: data.id,
        ok,
      });
      return;
    }
    if (data.type === "height") {
      const height = data.height;
      if (
        typeof height !== "number" ||
        !Number.isFinite(height) ||
        height < 0
      ) {
        return;
      }
      if (height <= 10_000) {
        const width = isShare && storage != null ? frame.clientWidth : null;
        frame.style.height = Math.ceil(height) + "px";
        if (isShare && storage != null) {
          const size = JSON.stringify({
            fingerprint: sizeFingerprint,
            width,
            viewportHeight: globalThis.innerHeight,
            dark: globalThis.matchMedia("(prefers-color-scheme: dark)").matches,
            scale: globalThis.devicePixelRatio,
            height: Math.ceil(height),
          });
          if (size !== lastSize) {
            lastSize = size;
            pendingSize = size;
            globalThis.clearTimeout(sizeWriteTimeout);
            sizeWriteTimeout = globalThis.setTimeout(persistSize, 100);
          }
        }
      }
      if (isShare) {
        // Keep unusually tall content accessible without accepting an unbounded height.
        if (height > 10_000 && Number.parseFloat(frame.style.height) === 0) {
          frame.style.removeProperty("height");
        }
        frame.scrolling = height > 10_000 ? "auto" : "no";
        document.documentElement.classList.add("visualization-share-ready");
      }
      return;
    }
    if (data.type === "scroll-to") {
      const top = data.top;
      if (
        typeof top === "number" &&
        Number.isFinite(top) &&
        top >= 0 &&
        globalThis.navigator.userActivation?.isActive === true
      ) {
        globalThis.scrollTo({
          top: Math.ceil(
            globalThis.scrollY + frame.getBoundingClientRect().top + top,
          ),
        });
      }
      return;
    }
    if (data.type === "open-external") {
      const rawHref = data.href;
      if (typeof rawHref !== "string") {
        return;
      }
      let href;
      try {
        const url = new URL(rawHref, document.baseURI);
        if (url.protocol !== "https:") {
          return;
        }
        href = url.href;
      } catch {
        return;
      }
      if (globalThis.navigator.userActivation?.isActive === true) {
        const anchor = document.createElement("a");
        anchor.href = href;
        anchor.rel = "noopener noreferrer";
        anchor.target = "_blank";
        document.body.appendChild(anchor);
        anchor.click();
        anchor.remove();
      }
      return;
    }
    if (data.type === "follow-up") {
      const prompt = data.prompt;
      if (
        typeof prompt !== "string" ||
        globalThis.navigator.userActivation?.isActive !== true
      ) {
        return;
      }
      const title = data.title;
      globalThis.prompt(typeof title === "string" ? title : prompt, prompt);
    }
  });
  channel.port1.start();
  const srcdoc = frame.dataset.srcdoc;
  if (srcdoc == null) {
    return;
  }
  globalThis.addEventListener("resize", () => {
    const height = Number.parseFloat(frame.style.height);
    if (!isShare && (!Number.isFinite(height) || height <= viewportHeight)) {
      frame.style.removeProperty("height");
    }
    viewportHeight = globalThis.innerHeight;
    channel.port1.postMessage({ type: "measure" });
  });
  frame.addEventListener(
    "load",
    () => {
      frame.contentWindow?.postMessage(
        { type: "codex-visualization-initialize" },
        "*",
        [channel.port2],
      );
    },
    { once: true },
  );
  // Match the trusted bootstrap element, not the same text in an authored title.
  const frameHtml = srcdoc.replace(
    '<script type="application/json" id="codex-visualization-widget-state">__CODEX_VISUALIZATION_WIDGET_STATE__</script>',
    () =>
      '<script type="application/json" id="codex-visualization-widget-state">' +
      JSON.stringify(initialGlobals).replaceAll("<", "\\u003c") +
      "</script>",
  );
  if (isShare && storage != null) {
    // Fingerprint the rendered document, including restored widget state, without
    // storing its content. This is a layout cache, not a security identity.
    let hash = 5381;
    for (let index = 0; index < frameHtml.length; index += 1) {
      hash = (hash * 33 + frameHtml.charCodeAt(index)) % 4_294_967_296;
    }
    sizeFingerprint = hash.toString(36);
    try {
      const size = JSON.parse(storage.getItem(sizeStorageKey) ?? "null");
      if (
        size?.fingerprint === sizeFingerprint &&
        size.width === frame.clientWidth &&
        size.viewportHeight === globalThis.innerHeight &&
        size.dark ===
          globalThis.matchMedia("(prefers-color-scheme: dark)").matches &&
        size.scale === globalThis.devicePixelRatio &&
        typeof size.height === "number" &&
        Number.isFinite(size.height) &&
        size.height >= 0 &&
        size.height <= 10_000
      ) {
        frame.style.height = size.height + "px";
      }
    } catch {
      // Ignore missing or invalid cache entries and measure the live frame.
    }
  }
  frame.srcdoc = frameHtml;
  delete frame.dataset.srcdoc;
})();
