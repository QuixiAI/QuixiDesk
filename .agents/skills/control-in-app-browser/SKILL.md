---
name: control-in-app-browser
description: "Control the in-app Browser for opening, navigating, inspecting visible or interactive page state, clicking, typing, screenshots, and local web testing. It can have existing signed-in sessions. For semantic operations on linked resources, prefer a purpose-built connector, API, or CLI when available."
---

# Browser
## Stop: choose the right surface before any browser action
Explicit browser intent wins: if the user names the in-app browser or Chrome, or asks to open, show, or navigate to a page; inspect its visual or interactive state; or interact with its UI, continue with Browser and do not substitute a connector.

Otherwise, treat a URL or open browser tab as context, not browser intent. Earlier Browser use does not make later semantic work browser-first. Before each semantic operation on a linked resource, you MUST query available and deferred tools for an applicable connector, API, or CLI. Reading these instructions or scanning visible tools does not count. Do not use Browser for that operation until the query is complete. Use the non-browser tool when available. If it handles the current operation, continue the larger workflow without Browser for that operation. Use Browser when no such tool exists, the tool cannot access the resource or lacks a required capability, or UI work remains; use available browser context before asking the user to repeat it.

Use this skill for browser automation tasks such as inspecting pages, navigating, testing local apps, clicking, typing, taking screenshots, and reading visible page state.

If this plugin is listed as available in the session, treat that as mandatory reading before browser work. Open and follow this skill before saying that Browser is unavailable and before falling back to standalone Playwright or Computer Use.

Do not skip this skill just because Computer Use MCP tool calls are directly visible or appear easier to invoke. The presence of Computer Use tools is not evidence that Computer Use is the preferred browser surface.

## Setup Documentation
Use `await agent.documentation.get("<name>")` when one of these setup topics applies:
- `bootstrap-troubleshooting`: read when browser setup succeeds but discovery or selection fails
- `chrome-troubleshooting`: read when Chromium browser extension setup, installation, or communication fails

## Bootstrap
These setup details are internal. User-facing progress updates should be less technical in nature. Never mention `Node REPL`, `node_repl`, `REPL`, JavaScript sessions, module exports, reading documentation, or loading instructions unless a user is asking for that exact information. If setup or recovery is needed, describe it naturally as connecting to the browser or retrying the browser connection.

The `browser-client` module is the core entry point for browser use, and is available under `scripts/browser-client.mjs` in this plugin's root directory. ALWAYS import it using an absolute path. IMPORTANT: If this path cannot be found, stop and report that this plugin is missing `scripts/browser-client.mjs`. NEVER use the built in `browser-client` library.

Run browser setup code through the Node REPL `js` tool. In this environment the callable tool id typically appears as `mcp__node_repl__js`. If it is not already available, use tool discovery for `node_repl js` without setting a result limit. You need the `js` execution tool: `js_reset` only clears state, and `js_add_node_module_dir` only changes package resolution. Do not call either helper while trying to expose `js`. If `js` is still not available, search again for `node_repl js` with `limit: 10`.

CODE MODE REQUIREMENT: When you call the Node REPL `js` tool from the code-mode `exec` tool, the outer `exec` script containing the initial `documentation()` call MUST begin with `// @exec: {"max_output_tokens": 20000}`. This is a first-line pragma for the outer `exec` call, not an argument to the nested `js` tool. Keep the exact `nodeRepl.write(await <browser>.documentation());` call shown in the applicable selection scenario below and forward its complete result.

Initialize the runtime once. Use `const` for stable handles and `let` for changing values; reassign instead of redeclaring. Never use `globalThis`.

```js
const { setupBrowserRuntime } = await import("<plugin root>/scripts/browser-client.mjs");
const agent = await setupBrowserRuntime();
```

Once a browser connection is established, reuse its existing browser binding across later turns and do not reread these instructions. Once you have read a browser's complete documentation, do not read it again unless you select a different browser.

Bind tabs directly from the selected browser, for example `const tab = await browser.tabs.new()`. If a later turn reports that a tab is missing, stale, closed, or not part of the current browser session, discard that tab binding and obtain or create a fresh tab from the existing browser binding. An empty `browser.tabs.list()` result is normal after tab cleanup and does not invalidate the browser binding. Never call `agent.browsers.get*` to recover a tab; only an explicit browser-disconnected error invalidates the binding.

## Browser selection
Keep browser actions in the browser hosting the current chat unless the user explicitly names another browser.

The scenarios below are for the initial browser selection only. Before calling any `agent.browsers.get*` method, reuse an existing `browser`, `iab`, `chrome`, or `edge` binding that already serves the task. A new user turn does not invalidate a browser binding or require another selection or documentation call.

Select the initial browser with exactly one of these scenarios, in the order
shown. An explicit request for the in-app browser, Chrome, or Edge always wins
over URL selection. Never call `getForUrl()` when the user names a browser.
An explicit browser request is a hard constraint: use only that browser and
never fall back to another browser surface. If its exact selector is
unavailable, report that browser as unavailable instead of calling
`getDefault()`, `getForUrl()`, or `get("extension")`.

App-provided in-app-browser context is ambient UI state, not a user instruction to select or switch browsers. Only the text of the user's request can explicitly choose a browser.

Do not inspect browser cookies, local storage, profiles, passwords, or session stores. Browser discovery must remain read-only.

When authentication blocks requested browser navigation, do not replace it with web search, a search engine, another site, or another source merely to bypass sign-in.

### The user explicitly requests a browser
A plugin mention in the user's request explicitly names its browser.
`[@Browser](plugin://browser@openai-bundled)` names the in-app browser.
Browser plugin mentions whose URL contains `browserFamily=chrome` or
`browserFamily=edge` name Chrome or Edge respectively.
`[@Chrome](plugin://chrome@openai-bundled)`,
`[@chrome-internal](plugin://chrome-internal@openai-bundled)`, and
`[@chrome-dev](plugin://chrome-dev@openai-bundled)` name Chrome. Follow the
corresponding explicit-browser scenario below.

The in-app browser is available only when the Browser skill is listed for the session. If the user explicitly requests the in-app browser and it is available, use a distinct persistent binding and immediately read its complete documentation:

```js
const iab = await agent.browsers.get("iab");
nodeRepl.write(await iab.documentation());
```

If the user explicitly requests the in-app browser but it is unavailable, report that instead of substituting another browser.

Chrome or Edge is available only when a Browser or Chrome skill is listed for the session and `agent.browsers.get("chrome")` or
`agent.browsers.get("edge")` succeeds. The browser family is a stable selector;
do not list browsers first or pass an opaque browser ID for an explicit family.

For Chrome, use a separate persistent binding and immediately read its complete
documentation:

```js
const chrome = await agent.browsers.get("chrome");
nodeRepl.write(await chrome.documentation());
```

For Edge, use its own persistent binding and immediately read its complete
documentation:

```js
const edge = await agent.browsers.get("edge");
nodeRepl.write(await edge.documentation());
```

If the user explicitly requests Chrome or Edge but that family is unavailable,
tell them that browser needs the ChatGPT browser extension and direct them to
**Settings → Computer use** to install it. Do not substitute another browser.

An explicit browser choice remains in force for the task. If authentication blocks the task in an explicitly selected browser, your next response must explicitly ask the user to sign in in that browser and tell you when it is ready, unless that browser's documentation provides a supported authentication flow to try first. Merely reporting that sign-in is required is not sufficient. Do not switch to another browser unless the user asks or approves the switch.

### The user explicitly requests an external browser without naming a family
When the user says to use their external browser, browser extension, or a
similar external-browser surface without naming Chrome or Edge, select the
first connected extension instance directly. Do not call
`agent.browsers.list()` first:

```js
const browser = await agent.browsers.get("extension");
nodeRepl.write(await browser.documentation());
```

If no extension instance is available, tell the user that their external
browser needs the ChatGPT browser extension and direct them to
**Settings → Computer use** to install it. Do not substitute the in-app
browser.

### The task requires browser interaction, the user does not specify a browser, and the task has a target URL
When the user supplies a URL or the intended URL can be reasonably inferred from the request, replace the example below with that URL and let browser-client choose the browser best suited to it. Do not call `agent.browsers.list()` first:

```js
const browser = await agent.browsers.getForUrl("https://example.com/");
nodeRepl.write(await browser.documentation());
```

### The user specifies neither a browser nor a target URL
Use the runtime default, which prefers the in-app browser when it is available and otherwise uses Chrome. Do not list browsers first:

```js
const browser = await agent.browsers.getDefault();
nodeRepl.write(await browser.documentation());
```

## After setup
If setup succeeds but browser discovery or selection fails, read `await agent.documentation.get("bootstrap-troubleshooting")` before resetting the JavaScript session or trying another browser-control mechanism.

If the failure is specific to Chrome extension setup, installation, or communication, read `await agent.documentation.get("chrome-troubleshooting")` before retrying or taking another recovery action.

When the user did not explicitly choose a browser, a browser selected by the runtime is not a user constraint. Do not switch browsers based only on an assumption about authentication. If navigation shows that the selected browser lacks the required authentication, select another available browser before asking the user to sign in. You may select it without resetting the Node session. Preserve existing `iab`, `chrome`, `edge`, and `browser` bindings when they are still useful. Existing tabs remain bound to the browser that created them. After selecting a different browser, obtain a tab from that browser before continuing and read its complete documentation.

The ability to interact directly with browsers is exposed through the `browser-client` runtime via the `agent.browsers.*` API. Before trying to interact with a selected browser for the first time, you MUST emit and read the complete documentation returned by its `documentation()` call in one go. For the initial documentation read, run the exact direct `nodeRepl.write(await <browser>.documentation());` call shown in the applicable scenario above. Do not assign the documentation to a variable, inspect its length, slice it, truncate it, summarize it, or emit only an excerpt. Do not proactively split the documentation into pages or chunks. Only if the tool output itself explicitly reports that it was truncated may you emit and read smaller chunks until you have read the documentation in its entirety.

Only the Node REPL `js` tool (`mcp__node_repl__js`) can be used to control the selected browser. Do not use external MCP browser-control tools, separate browser automation servers, or other browser skills for this surface. References to Playwright mean the documented `tab.playwright` API.

<!-- BROWSER_SKILL_EOF: This is the complete Browser skill. Do not request additional lines. -->
