import{x as e}from"./shared-12-Dro5Rhl0.js";import{Server as t}from"./c2a6d07ee-B94IjrMM.js";var n="ui/resourceUri",r="ui://mcp-apps-example/hello-world.html",i="ui://mcp-apps-example/counter.html",a="ui://mcp-apps-example/sticky-notes.html",o=function(e){return e.MCP_APPS_HELLO="mcp_apps_hello",e.MCP_APPS_COUNTER="mcp_apps_counter",e.MCP_APPS_STICKY_NOTES="mcp_apps_sticky_notes",e}(o||{}),s=`
    const app = (() => {
      let nextId = 1;

      return {
        sendRequest({ method, params }) {
          const id = nextId++;
          return new Promise((resolve, reject) => {
            const listener = (event) => {
              const data = event.data;
              if (
                event.source !== window.parent ||
                !data || data.jsonrpc !== "2.0" || data.id !== id
              ) {
                return;
              }
              window.removeEventListener("message", listener);
              if (data.error) {
                const err = new Error(data.error.message || "Request failed");
                err.code = data.error.code;
                reject(err);
              } else {
                resolve(data.result);
              }
            };
            window.addEventListener("message", listener);
            window.parent.postMessage({ jsonrpc: "2.0", id, method, params }, "*");
          });
        },
        // ui/initialize + ui/notifications/initialized; resolves with the
        // initialize result (hostContext, hostCapabilities, ...).
        async initialize(appInfo) {
          const result = await this.sendRequest({
            method: "ui/initialize",
            params: {
              protocolVersion: "2025-11-21",
              appInfo,
              appCapabilities: {},
            },
          });
          this.sendNotification({ method: "ui/notifications/initialized", params: {} });
          return result;
        },
        // Vendored persistence (anthropic/set-view-state). Resolves with the
        // host's revision, or null when this host does not support it.
        async setViewState(state) {
          try {
            const { revision } = await this.sendRequest({
              method: "anthropic/set-view-state",
              params: { state },
            });
            return revision;
          } catch (err) {
            if (err && err.code === -32601) {
              return null;
            }
            throw err;
          }
        },
        sendNotification({ method, params }) {
          window.parent.postMessage({ jsonrpc: "2.0", method, params }, "*");
        },
        onNotification(method, handler) {
          window.addEventListener("message", function listener(event) {
            // Only the host (the sandbox's parent) speaks this protocol.
            if (event.source !== window.parent) {
              return;
            }
            if (event.data?.method === method) {
              handler(event.data.params);
            }
          });
        },
        setupAutoResize() {
          new ResizeObserver(() => {
            const { body, documentElement: html } = document;
            const htmlStyle = getComputedStyle(html);

            // getBoundingClientRect returns fractional values (unlike scrollHeight)
            const rect = body.getBoundingClientRect();

            const width = Math.ceil(rect.width);
            const height = Math.ceil(
              rect.height +
              (parseFloat(htmlStyle.borderTop) || 0) +
              (parseFloat(htmlStyle.borderBottom) || 0)
            );

            this.sendNotification({
              method: "ui/notifications/size-changed",
              params: { width, height }
            });
          }).observe(document.body);
        },
      };
    })();
`;async function c(){let s=new t({name:"MCP Apps Example",version:"1.0.0"},{capabilities:{tools:{},resources:{}},jsonSchemaValidator:await e()});return s.setRequestHandler("tools/list",()=>({tools:[{name:o.MCP_APPS_HELLO,description:"Demonstrates MCP Apps - returns an interactive UI that greets the user",inputSchema:{type:"object",properties:{name:{type:"string",description:"Name to greet"}}},_meta:{[n]:r}},{name:o.MCP_APPS_COUNTER,description:"Demonstrates MCP Apps with an interactive counter UI. The user can increment/decrement the counter.",inputSchema:{type:"object",properties:{initialValue:{type:"number",description:"Initial counter value",default:0}}},_meta:{[n]:i}},{name:o.MCP_APPS_STICKY_NOTES,description:"Demonstrates MCP Apps view-state persistence: a board of sticky notes the user edits in place. When the host offers persistence the notes survive page reloads; the view itself says whether they were stored.",inputSchema:{type:"object",properties:{title:{type:"string",description:"Board title",default:"Sticky notes"}}},_meta:{[n]:a}}]})),s.setRequestHandler("tools/call",e=>{let{name:t,arguments:n}=e.params;if(t===o.MCP_APPS_HELLO){let e=n?.name;return{content:[{type:"text",text:`Greeting: ${e?`Hello, ${e}!`:"Hello, World!"}. If your client supports MCP Apps, an interactive UI should have rendered.`}]}}return t===o.MCP_APPS_COUNTER?{content:[{type:"text",text:`Counter initialized to ${n?.initialValue??0}. If your client supports MCP Apps, an interactive counter UI should have rendered.`}]}:t===o.MCP_APPS_STICKY_NOTES?{content:[{type:"text",text:"Sticky notes board rendered. The user edits the notes directly in the view; their content is not visible to you."}]}:{content:[{type:"text",text:`Unknown tool: ${t}`}],isError:!0}}),s.setRequestHandler("resources/list",()=>({resources:[{uri:r,name:"Hello World MCP App",description:"Interactive greeting UI for the mcp_apps_hello tool",mimeType:"text/html;profile=mcp-app"},{uri:i,name:"Counter MCP App",description:"Interactive counter UI for the mcp_apps_counter tool",mimeType:"text/html;profile=mcp-app"},{uri:a,name:"Sticky Notes MCP App",description:"Persistent sticky-notes board for the mcp_apps_sticky_notes tool",mimeType:"text/html;profile=mcp-app"}]})),s.setRequestHandler("resources/read",e=>{let{uri:t}=e.params;if(t===r)return{contents:[{uri:r,mimeType:"text/html;profile=mcp-app",text:l(),_meta:{ui:{csp:{resourceDomains:["https://unpkg.com"],connectDomains:["https://api.anthropic.com","https://bad-bunny.com"]},permissions:{clipboardWrite:{},microphone:{}}}}}]};if(t===i)return{contents:[{uri:i,mimeType:"text/html;profile=mcp-app",text:u()}]};if(t===a)return{contents:[{uri:a,mimeType:"text/html;profile=mcp-app",text:d()}]};throw Error(`Unknown resource: ${t}`)}),s}function l(){return`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Hello World MCP App</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      padding: 1rem;
      background: transparent;
    }
    .container {
      max-width: 400px;
    }
    h1 {
      font-size: 1.25rem;
      margin-bottom: 0.75rem;
      color: #1a1a1a;
    }
    .greeting {
      font-size: 1.5rem;
      padding: 1rem;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      color: white;
      border-radius: 8px;
      text-align: center;
      margin-bottom: 1rem;
    }
    .input-row {
      display: flex;
      gap: 0.5rem;
      margin-bottom: 1rem;
    }
    input {
      flex: 1;
      padding: 0.5rem 0.75rem;
      border: 1px solid #ddd;
      border-radius: 6px;
      font-size: 0.875rem;
    }
    input:focus {
      outline: none;
      border-color: #667eea;
    }
    button {
      padding: 0.5rem 1rem;
      background: #667eea;
      color: white;
      border: none;
      border-radius: 6px;
      cursor: pointer;
      font-size: 0.875rem;
    }
    button:hover {
      background: #5a67d8;
    }
    .info {
      font-size: 0.75rem;
      color: #666;
    }
    .tool-input {
      margin-top: 1rem;
      padding: 0.75rem;
      background: #f5f5f5;
      border-radius: 6px;
      font-size: 0.75rem;
    }
    .tool-input pre {
      white-space: pre-wrap;
      word-break: break-all;
    }
  </style>
</head>
<body>
  <div class="container" id="app">
    <h1>Hello World MCP App</h1>
    <div class="greeting" id="greeting">Hello, World!</div>
    <div class="input-row">
      <input type="text" id="nameInput" placeholder="Enter a name..." />
      <button id="greetBtn">Greet</button>
    </div>
    <p class="info">Type a name and click Greet to update the greeting.</p>
    <div class="tool-input" id="toolInputDisplay" style="display: none;">
      <strong>Tool Input:</strong>
      <pre id="toolInputContent"></pre>
    </div>
  </div>

  <script type="module">
${s}
    const IMPLEMENTATION = { name: "Hello World App", version: "1.0.0" };

    // State
    let currentName = "World";

    // DOM elements
    const greetingEl = document.getElementById("greeting");
    const nameInput = document.getElementById("nameInput");
    const greetBtn = document.getElementById("greetBtn");
    const toolInputDisplay = document.getElementById("toolInputDisplay");
    const toolInputContent = document.getElementById("toolInputContent");

    // Update greeting
    function updateGreeting(name) {
      currentName = name || "World";
      greetingEl.textContent = "Hello, " + currentName + "!";
    }

    // Handle button click
    greetBtn.addEventListener("click", () => {
      updateGreeting(nameInput.value);
      nameInput.value = "";
    });

    // Handle enter key
    nameInput.addEventListener("keypress", (e) => {
      if (e.key === "Enter") {
        updateGreeting(nameInput.value);
        nameInput.value = "";
      }
    });

    // MCP App connection
    let nextRequestId = 1;

    async function connectToHost() {
      try {
        // Listen for messages from parent
        window.addEventListener("message", (event) => {
          try {
            const data = event.data;
            if (event.source === window.parent && data && data.jsonrpc === "2.0") {
              // Handle initialize response - send initialized notification to complete handshake
              if (data.id === 1 && data.result) {
                const initializedNotification = {
                  jsonrpc: "2.0",
                  method: "ui/notifications/initialized",
                  params: {}
                };
                window.parent.postMessage(initializedNotification, "*");
              }
            }
          } catch (err) {
            console.error("[HelloWorldApp] Error handling message:", err);
          }
        });

        // Register notification handlers
        app.onNotification("ui/notifications/tool-input", (params) => {
          const args = params?.arguments || {};
          toolInputDisplay.style.display = "block";
          toolInputContent.textContent = JSON.stringify(args, null, 2);
          if (args.name) {
            updateGreeting(args.name);
          }
        });

        app.onNotification("ui/notifications/tool-result", (params) => {
          console.log("[HelloWorldApp] Tool result:", params);
        });

        app.onNotification("ui/notifications/host-context-changed", (params) => {
          console.log("[HelloWorldApp] Host context changed:", params);
        });

        // Send initialize request
        const initRequest = {
          jsonrpc: "2.0",
          id: nextRequestId++,
          method: "ui/initialize",
          params: {
            protocolVersion: "2025-11-21",
            appInfo: IMPLEMENTATION,
            appCapabilities: {}
          }
        };
        window.parent.postMessage(initRequest, "*");
        app.setupAutoResize();
      } catch (err) {
        console.error("[HelloWorldApp] Failed to connect:", err);
      }
    }

    // Connect when page loads
    connectToHost();
  <\/script>
</body>
</html>`}function u(){return`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Counter MCP App</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      padding: 1rem;
      background: transparent;
    }
    .container {
      max-width: 300px;
      text-align: center;
    }
    h1 {
      font-size: 1.25rem;
      margin-bottom: 1rem;
      color: #1a1a1a;
    }
    .counter-display {
      font-size: 3rem;
      font-weight: bold;
      padding: 1.5rem;
      background: linear-gradient(135deg, #11998e 0%, #38ef7d 100%);
      color: white;
      border-radius: 12px;
      margin-bottom: 1rem;
      min-width: 150px;
      display: inline-block;
    }
    .buttons {
      display: flex;
      gap: 0.5rem;
      justify-content: center;
      margin-bottom: 1rem;
    }
    button {
      width: 50px;
      height: 50px;
      font-size: 1.5rem;
      border: none;
      border-radius: 50%;
      cursor: pointer;
      transition: transform 0.1s;
    }
    button:hover {
      transform: scale(1.1);
    }
    button:active {
      transform: scale(0.95);
    }
    .decrement {
      background: #ff6b6b;
      color: white;
    }
    .increment {
      background: #4ecdc4;
      color: white;
    }
    .reset {
      background: #95a5a6;
      color: white;
      font-size: 0.75rem;
      width: 60px;
      height: 30px;
      border-radius: 15px;
    }
    .info {
      font-size: 0.75rem;
      color: #666;
      margin-top: 0.5rem;
    }
  </style>
</head>
<body>
  <div class="container">
    <h1>Counter</h1>
    <div class="counter-display" id="counter">0</div>
    <div class="buttons">
      <button class="decrement" id="decrementBtn">-</button>
      <button class="increment" id="incrementBtn">+</button>
    </div>
    <button class="reset" id="resetBtn">Reset</button>
    <p class="info">Click the buttons to change the counter value.</p>
    <p class="info" id="persistence">Persistence: checking\u2026</p>
  </div>

  <script type="module">
${s}
    const IMPLEMENTATION = { name: "Counter App", version: "1.0.0" };

    // State
    let count = 0;
    let initialValue = 0;
    // The restore can arrive in the initialize result and again in the first
    // host-context-changed; snapshotSeen applies it once.
    let persistenceSupported = null;
    let snapshotSeen = false;
    let restored = false;
    // Clicks before the restore arrives are kept as a change to apply on top of the stored count.
    let changedBeforeRestore = false;
    let deltaBeforeRestore = 0;
    let resetBeforeRestore = false;

    // DOM elements
    const counterEl = document.getElementById("counter");
    const incrementBtn = document.getElementById("incrementBtn");
    const decrementBtn = document.getElementById("decrementBtn");
    const resetBtn = document.getElementById("resetBtn");
    const persistenceEl = document.getElementById("persistence");

    // Accepts the host's restore snapshot from either the initialize result
    // or a later host-context-changed (whichever carries it first).
    function applyViewState(hostContext) {
      if (!hostContext || !("anthropic/viewState" in hostContext) || snapshotSeen) {
        return;
      }
      const snapshot = hostContext["anthropic/viewState"];
      persistenceSupported = true;
      snapshotSeen = true;
      const storedCount =
        snapshot && snapshot.state && typeof snapshot.state.count === "number"
          ? snapshot.state.count
          : null;
      if (changedBeforeRestore) {
        restored = true;
        const base = resetBeforeRestore || storedCount === null ? initialValue : storedCount;
        count = base + deltaBeforeRestore;
        updateDisplay();
        updateWidgetContext();
        persistCount();
      } else if (storedCount !== null) {
        restored = true;
        count = storedCount;
        updateDisplay();
        updateWidgetContext();
        persistenceEl.textContent =
          "Persistence: restored count " + count + " (revision " + snapshot.revision + ")";
      } else {
        persistenceEl.textContent = "Persistence: supported, nothing stored yet";
      }
    }

    function persistCount() {
      if (persistenceSupported === false) {
        return;
      }
      if (!snapshotSeen) {
        changedBeforeRestore = true;
        persistenceEl.textContent =
          "Persistence: not saved yet: the host hasn\u2019t sent the saved count (still loading, or saving is off)";
        return;
      }
      const sent = count;
      app.setViewState({ count: sent }).then((revision) => {
        if (revision === null) {
          persistenceSupported = false;
          persistenceEl.textContent = "Persistence: not supported by this host";
          return;
        }
        persistenceEl.textContent = "Persistence: saved count " + sent + " (revision " + revision + ")";
      }).catch((err) => {
        console.error("[CounterApp] Save failed:", err);
        persistenceEl.textContent = "Persistence: the host did not save the count";
      });
    }

    // Update display
    function updateDisplay() {
      counterEl.textContent = count;
      // Add color animation based on value
      if (count > 0) {
        counterEl.style.background = "linear-gradient(135deg, #11998e 0%, #38ef7d 100%)";
      } else if (count < 0) {
        counterEl.style.background = "linear-gradient(135deg, #ff6b6b 0%, #ee5a5a 100%)";
      } else {
        counterEl.style.background = "linear-gradient(135deg, #667eea 0%, #764ba2 100%)";
      }
    }

    // Update widget context with current counter value
    function updateWidgetContext() {
      app.sendRequest({
        method: "ui/update-model-context",
        params: {
          content: [
            {
              type: "text",
              text: "Counter is at " + count,
            },
          ],
        },
      }).catch((err) => {
        console.error("[CounterApp] Failed to update widget context:", err);
      });
    }

    // Event listeners
    incrementBtn.addEventListener("click", () => {
      count++;
      if (!snapshotSeen) {
        deltaBeforeRestore++;
      }
      updateDisplay();
      updateWidgetContext();
      persistCount();
    });

    decrementBtn.addEventListener("click", () => {
      count--;
      if (!snapshotSeen) {
        deltaBeforeRestore--;
      }
      updateDisplay();
      updateWidgetContext();
      persistCount();
    });

    resetBtn.addEventListener("click", () => {
      count = initialValue;
      if (!snapshotSeen) {
        resetBeforeRestore = true;
        deltaBeforeRestore = 0;
      }
      updateDisplay();
      updateWidgetContext();
      persistCount();
    });

    async function connectToHost() {
      try {
        // Register notification handlers
        app.onNotification("ui/notifications/tool-input", (params) => {
          const args = params?.arguments || {};
          if (typeof args.initialValue === "number") {
            initialValue = args.initialValue;
            // A restored count wins over the tool's initial value; clicks made before either are kept.
            if (!restored) {
              count = initialValue + deltaBeforeRestore;
              updateDisplay();
            }
          }
        });

        app.onNotification("ui/notifications/tool-result", (params) => {
          console.log("[CounterApp] Tool result:", params);
        });

        app.onNotification("ui/notifications/host-context-changed", (params) => {
          applyViewState(params);
        });

        const result = await app.initialize(IMPLEMENTATION);
        applyViewState(result && result.hostContext);
        if (!snapshotSeen) {
          persistenceEl.textContent = "Persistence: no saved count from the host yet (still loading, or not offered)";
        }
        app.setupAutoResize();
      } catch (err) {
        console.error("[CounterApp] Failed to connect:", err);
      }
    }

    // Initialize
    updateDisplay();
    connectToHost();
  <\/script>
</body>
</html>`}function d(){return`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Sticky Notes MCP App</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      padding: 1rem;
      background: transparent;
      color: #1a1a1a;
    }
    h1 { font-size: 1.1rem; margin-bottom: 0.75rem; }
    .board { display: flex; gap: 0.75rem; flex-wrap: wrap; }
    .note {
      width: 180px;
      border-radius: 10px;
      padding: 0.6rem;
      box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
      display: flex;
      flex-direction: column;
      gap: 0.4rem;
    }
    .note textarea {
      width: 100%;
      height: 90px;
      border: none;
      background: rgba(255, 255, 255, 0.45);
      border-radius: 6px;
      padding: 0.4rem;
      font: inherit;
      font-size: 0.85rem;
      resize: none;
    }
    .note select { font: inherit; font-size: 0.8rem; border-radius: 6px; padding: 0.15rem; }
    .yellow { background: #fff3a3; }
    .pink { background: #ffc6d9; }
    .green { background: #c8f5c1; }
    .blue { background: #c4e1ff; }
    .status { font-size: 0.75rem; color: #555; margin-top: 0.75rem; }
    .status strong { color: #1a1a1a; }
  </style>
</head>
<body>
  <h1 id="title">Sticky notes</h1>
  <div class="board" id="board"></div>
  <p class="status" id="restore">Restore: waiting for the host\u2026</p>
  <p class="status" id="save">Save: nothing changed yet</p>

  <script type="module">
${s}
    const IMPLEMENTATION = { name: "Sticky Notes App", version: "1.0.0" };
    const COLORS = ["yellow", "pink", "green", "blue"];
    const NOTE_COUNT = 3;

    const boardEl = document.getElementById("board");
    const titleEl = document.getElementById("title");
    const restoreEl = document.getElementById("restore");
    const saveEl = document.getElementById("save");

    let notes = Array.from({ length: NOTE_COUNT }, (_, i) => ({
      text: "",
      color: COLORS[i % COLORS.length],
    }));
    let restored = false;
    let saveTimer = null;
    // Fields edited before the restore arrives ("index:text", "index:color") keep the edit; nothing saves until then.
    const editedBeforeRestore = new Set();

    function render() {
      // Keep focus (and a note's caret) when a late restore redraws the board, only while this frame has focus.
      const active = document.hasFocus() ? document.activeElement : null;
      const tag = active && (active.tagName === "TEXTAREA" || active.tagName === "SELECT") ? active.tagName.toLowerCase() : null;
      const focusIndex = tag ? [...boardEl.querySelectorAll(tag)].indexOf(active) : -1;
      const selection = tag === "textarea" && focusIndex >= 0 ? [active.selectionStart, active.selectionEnd] : null;
      boardEl.textContent = "";
      notes.forEach((note, i) => {
        const wrap = document.createElement("div");
        wrap.className = "note " + note.color;
        const textarea = document.createElement("textarea");
        textarea.value = note.text;
        textarea.placeholder = "Note " + (i + 1);
        textarea.setAttribute("aria-label", "Note " + (i + 1) + " text");
        textarea.addEventListener("input", () => {
          notes[i].text = textarea.value;
          scheduleSave(i + ":text");
        });
        const select = document.createElement("select");
        select.setAttribute("aria-label", "Note " + (i + 1) + " colour");
        for (const color of COLORS) {
          const option = document.createElement("option");
          option.value = color;
          option.textContent = color;
          option.selected = color === note.color;
          select.appendChild(option);
        }
        select.addEventListener("change", () => {
          notes[i].color = select.value;
          wrap.className = "note " + select.value;
          scheduleSave(i + ":color");
        });
        wrap.appendChild(textarea);
        wrap.appendChild(select);
        boardEl.appendChild(wrap);
      });
      if (focusIndex >= 0) {
        const el = boardEl.querySelectorAll(tag)[focusIndex];
        el.focus();
        if (selection) {
          el.setSelectionRange(selection[0], selection[1]);
        }
      }
    }

    function scheduleSave(field) {
      if (!restored) {
        editedBeforeRestore.add(field);
        saveEl.textContent = "Save: not saved yet: the host hasn\u2019t sent the saved notes (still loading, or saving is off)";
        return;
      }
      saveEl.textContent = "Save: pending\u2026";
      clearTimeout(saveTimer);
      saveTimer = setTimeout(save, 300);
    }

    // A save still waiting on the debounce goes out now, before the page goes away.
    window.addEventListener("pagehide", () => {
      if (saveTimer !== null) {
        clearTimeout(saveTimer);
        save();
      }
    });

    // The revision comes from the host: rendered with textContent, never as HTML.
    function setStatus(el, parts) {
      el.textContent = "";
      for (const part of parts) {
        if (typeof part === "string") {
          el.appendChild(document.createTextNode(part));
        } else {
          const strong = document.createElement("strong");
          strong.textContent = String(part.strong);
          el.appendChild(strong);
        }
      }
    }

    function save() {
      saveTimer = null;
      app.setViewState({ notes }).then((revision) => {
        // A newer edit is already waiting to be saved: this reply is out of date.
        if (saveTimer !== null) {
          return;
        }
        if (revision === null) {
          setStatus(saveEl, ["Save: this host does not support view-state persistence"]);
        } else {
          setStatus(saveEl, ["Save: stored as ", { strong: "revision " + revision }]);
        }
      }).catch((err) => {
        console.error("[StickyNotesApp] Save failed:", err);
        saveEl.textContent = "Save: the host did not save the notes";
      });
    }

    function applyViewState(hostContext) {
      if (!hostContext || !("anthropic/viewState" in hostContext) || restored) {
        return;
      }
      restored = true;
      const snapshot = hostContext["anthropic/viewState"];
      const stored = snapshot && snapshot.state && Array.isArray(snapshot.state.notes)
        ? snapshot.state.notes
        : null;
      if (stored) {
        notes = notes.map((note, i) => ({
          text:
            editedBeforeRestore.has(i + ":text") || typeof stored[i]?.text !== "string"
              ? note.text
              : stored[i].text,
          color:
            editedBeforeRestore.has(i + ":color") || !COLORS.includes(stored[i]?.color)
              ? note.color
              : stored[i].color,
        }));
        render();
        const kept = [...new Set([...editedBeforeRestore].map((field) => Number(field.split(":")[0]) + 1))].sort();
        setStatus(restoreEl, [
          "Restore: ",
          { strong: "revision " + snapshot.revision },
          kept.length > 0
            ? " (kept your edits to note" + (kept.length > 1 ? "s " : " ") + kept.join(", ") + ")"
            : "",
        ]);
      } else {
        restoreEl.textContent = "Restore: persistence available, nothing stored yet";
      }
      if (editedBeforeRestore.size > 0) {
        save();
      }
    }

    async function connectToHost() {
      app.onNotification("ui/notifications/tool-input", (params) => {
        const title = params?.arguments?.title;
        if (typeof title === "string" && title.trim()) {
          titleEl.textContent = title;
        }
      });
      app.onNotification("ui/notifications/host-context-changed", (params) => {
        applyViewState(params);
      });
      const result = await app.initialize(IMPLEMENTATION);
      applyViewState(result && result.hostContext);
      if (!restored) {
        restoreEl.textContent = "Restore: the host offered no saved notes (still loading, or saving is off)";
      }
      app.setupAutoResize();
    }

    render();
    connectToHost().catch((err) => {
      console.error("[StickyNotesApp] Failed to connect:", err);
    });
  <\/script>
</body>
</html>`}export{c as getMcpAppsExampleServerInstance};import"./c28eafaad-iTqpbj_J.js";import"./cd1efad72-DHI651qt.js";import"./c6805d2eb-vXZUyulN.js";import"./c615e65e2-BI9bDY3u.js";