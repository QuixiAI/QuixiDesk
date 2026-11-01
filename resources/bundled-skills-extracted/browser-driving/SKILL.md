---
name: browser-driving
description: Drive a real installed browser (Chrome, Edge, Brave over CDP; Firefox over WebDriver BiDi) on macOS, Windows or Linux when the in-app browser or plain HTTP is not enough, and verify fetch results before reporting them.
license: MIT
---

# Driving a real browser

Original QuixiDesk task instructions, adapted from the browser-driving skill by volkdude85 (MIT).

Use this when a task needs a page that QuixiDesk's in-app browser tools or a plain HTTP fetch
cannot load correctly: pages that need a full desktop browser engine, a specific browser, or a
session the user signs in to for this task. Most failures in this area are a transport mistake or a
misread result, not a blocked site.

## Choose the lightest transport that works

```
JSON API, feed, file download     -> HTTP (curl)
Static HTML                       -> HTTP (curl)
Needs JavaScript to render        -> in-app browser tools first, then a real browser
Needs a signed-in session         -> real browser + an automation profile, with the user's consent
Specific engine required          -> real browser (Chromium family over CDP, Firefox over BiDi)
```

Prefer the in-app browser tools when they are available: they are isolated from the user's own
browsing and need no setup. Reach for an installed browser only when they are not enough.

## Consent and isolation come first

Driving a browser gives you whatever that browser can reach.

- **Never attach to the user's everyday browser or profile.** It holds their email, banking and other
  signed-in sessions. Work in a dedicated automation profile directory.
- **Ask before signing in anywhere.** If a task needs an account, tell the user which site and why,
  and let them sign in to the automation profile themselves. Do not type their credentials.
- **Do not defeat access controls.** If a site shows a CAPTCHA, a bot challenge, a paywall or a login
  wall you were not given access to, stop and tell the user. Do not spoof fingerprints, rotate
  identities, or retry until it gives in.
- **Respect rate limits.** On `429` or `503`, back off; honour `Retry-After`. Do not parallelise
  requests against one site to go faster.
- **Close what you open.** A debugging port lets any local process control that browser. Quit the
  automation browser when the task is done.

## Chromium family (Chrome, Edge, Brave, Chromium) over CDP

Launch a separate instance with its own profile directory and a debugging port. Chrome 136 and later
ignore `--remote-debugging-port` on the default profile, so a dedicated `--user-data-dir` is required
anyway. Pick a port nothing else is using; 9222 is the common default and is often already taken.

First check that the port is free (no answer means free):

```sh
curl -s http://127.0.0.1:9333/json/version
```

macOS (zsh/bash):

```sh
PROFILE="$HOME/Library/Caches/QuixiDesk/browser-profile"
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --remote-debugging-port=9333 --user-data-dir="$PROFILE" \
  --no-first-run --no-default-browser-check about:blank &
# Brave: "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"
# Edge:  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"
```

Windows (PowerShell):

```powershell
$profileDir = "$env:LOCALAPPDATA\QuixiDesk\browser-profile"
$browser = @(
  "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
  "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
  "$env:ProgramFiles\BraveSoftware\Brave-Browser\Application\brave.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
Start-Process $browser -ArgumentList "--remote-debugging-port=9333",
  "--user-data-dir=`"$profileDir`"", "--no-first-run", "--no-default-browser-check", "about:blank"
```

Edge ships with Windows, so it is always available there. In Windows PowerShell 5.1, `curl` is an
alias for `Invoke-WebRequest`; call `curl.exe` to get real curl.

Linux:

```sh
BROWSER=$(command -v google-chrome || command -v chromium || command -v brave-browser || command -v brave)
case "$BROWSER" in
  # Snap browsers (Ubuntu's chromium) cannot write hidden directories such as ~/.cache;
  # they fail with "Failed To Create Data Directory" and run without a usable profile.
  /snap/bin/*) PROFILE="$HOME/snap/$(basename "$BROWSER")/common/quixidesk-browser-profile" ;;
  *)           PROFILE="${XDG_CACHE_HOME:-$HOME/.cache}/quixidesk/browser-profile" ;;
esac
mkdir -p "$PROFILE"
"$BROWSER" --remote-debugging-port=9333 --user-data-dir="$PROFILE" \
  --no-first-run --no-default-browser-check about:blank &
```

Confirm the profile is really in use before relying on it: a few seconds after launch the
directory should contain a `Local State` file. If it stays empty, the browser could not write there
(sandboxed packages such as snaps and Flatpaks restrict where they can write) and nothing you sign in
to will be kept.

The HTTP endpoints cover most needs (use `curl.exe` on Windows):

```sh
curl -s http://127.0.0.1:9333/json/version                          # browser and WebSocket URL
curl -s http://127.0.0.1:9333/json/list                             # open tabs
curl -s -X PUT "http://127.0.0.1:9333/json/new?https://example.com" # open a tab (PUT is required)
```

To read or interact with a page, attach to a tab's `webSocketDebuggerUrl`. Node 22 and later have a
built-in `WebSocket`, so no package is needed:

```js
// node cdp.mjs <webSocketDebuggerUrl>
const ws = new WebSocket(process.argv[2]);
let id = 0;
const call = (method, params = {}) => new Promise((resolve) => {
  const mine = ++id;
  ws.addEventListener("message", function onMessage(event) {
    const msg = JSON.parse(event.data);
    if (msg.id === mine) { ws.removeEventListener("message", onMessage); resolve(msg); }
  });
  ws.send(JSON.stringify({ id: mine, method, params }));
});
ws.addEventListener("open", async () => {
  const r = await call("Runtime.evaluate", { expression: "document.title", returnByValue: true });
  console.log(r.result?.result?.value);
  ws.close();
});
```

In Python, use `websocket-client` (`python3 -c "import websocket"` to check; ask before installing
packages).

Mistakes that cost real time:

- **Relaunching with the same `--user-data-dir` changes nothing.** The running instance takes the
  request and keeps its old flags. Quit it first, or use a different profile directory.
- **A port that answers is not necessarily yours.** Check `/json/version` and the open tabs before
  acting, so you do not drive someone else's session.
- **Window placement:** on Linux Wayland sessions, X11 tools such as `xdotool` cannot see native
  Wayland windows. Add `--ozone-platform=x11` if you need X11 window geometry.

## Firefox over WebDriver BiDi

Firefox removed its CDP support in version 129; automate it with WebDriver BiDi instead. Do not try
to attach to Firefox over CDP.

Selenium 4.6 and later download a matching `geckodriver` themselves (Selenium Manager), so no driver
path is needed on any OS:

```python
from selenium import webdriver
from selenium.webdriver.firefox.options import Options

opts = Options()
opts.enable_bidi = True
opts.add_argument("-profile")
opts.add_argument("/path/to/automation-profile")   # a dedicated profile, never the user's default
driver = webdriver.Firefox(options=opts)
driver.get("https://example.com")
print(driver.title)
driver.quit()
```

Check for Selenium first (`python3 -c "import selenium"`; on Windows `py -c "import selenium"`) and
ask before installing it. Create the profile directory first; on Windows use a path under
`%LOCALAPPDATA%`. Ubuntu's Firefox is also a snap, so the same rule applies there: use a directory
under `~/snap/firefox/common/`, not a hidden one such as `~/.cache`.

## HTTP: when it is right, and how to tell it worked

curl is the right tool for APIs, feeds and downloads. Two rules prevent the classic false negative:

- **Check the status code.** A fetch that discards the status reports a `429` or a challenge page as
  "no results". Capture the code and treat anything but a 2xx as a failure to explain, not an
  empty answer.
- **Give downloads a floor.** Without one, a stalled transfer looks like a slow success.

```sh
curl -sS -L -o page.html -w '%{http_code} %{size_download}\n' 'https://example.com/'
curl -L --retry 3 --speed-limit 2048 --speed-time 45 -o file.bin 'https://example.com/file.bin'
```

## Verify before reporting

"The site has nothing" must be distinguishable from "my transport failed". Before reporting a result,
state the status code, the number of bytes received, and a short excerpt of what the page actually
said. If you cannot, say the fetch is unverified rather than presenting a guess as a finding.
