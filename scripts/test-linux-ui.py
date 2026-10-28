#!/usr/bin/env python3
"""Automated Linux/WebKit smoke checks; uses only a fresh synthetic profile."""

import json, os, subprocess, time, urllib.request, threading, tempfile, pathlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import argparse

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--root", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parent.parent
)
parser.add_argument("--output", type=pathlib.Path)
parser.add_argument("--port", type=int, default=19460)
args = parser.parse_args()
root = args.root.resolve()
out = args.output or pathlib.Path(tempfile.mkdtemp(prefix="quixi-linux-ui-"))
out.mkdir(parents=True, exist_ok=True)
results = []
counter = 0


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        global counter
        counter += 1
        page = "second" if self.path.startswith("/next") else "first"
        body = (
            """<!doctype html><html><head><meta charset="utf-8"><title>Independent browser fixture</title></head><body><h1>"""
            + page
            + '''</h1><a id="next" href="/next">Next page</a><script>
  (async()=>{let parentBlocked=false,apiBlocked=false;try{parent.document.body.innerText}catch(e){parentBlocked=true}
  try{const r=await fetch('app://localhost/api/bootstrap');apiBlocked=!r.ok}catch(e){apiBlocked=true}
  parent.postMessage({testProbe:true,parentBlocked,apiBlocked,nativeBridgeAbsent:!window.quixi,page:"'''
            + page
            + """",load:"""
            + str(counter)
            + """},'*')})();
  addEventListener('message',e=>{if(e.data?.testNavigate)document.getElementById('next').click()});</script></body></html>"""
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(body)


server = ThreadingHTTPServer(("127.0.0.1", 0), H)
threading.Thread(target=server.serve_forever, daemon=True).start()
base = "http://127.0.0.1:" + str(args.port)
fixture_url = "http://127.0.0.1:" + str(server.server_address[1])
# Fail before launch if another bridge owns the chosen port.
import socket

with socket.socket() as probe:
    probe.bind(("127.0.0.1", args.port))


def request(path, body=None):
    raw = None if body is None else json.dumps(body).encode()
    return json.load(
        urllib.request.urlopen(
            urllib.request.Request(
                base + path, data=raw, headers={"Content-Type": "application/json"}
            ),
            timeout=8,
        )
    )


def ev(js):
    r = request("/action", {"action": "eval", "script": js})
    if not r.get("ok"):
        raise Exception(r)
    return r.get("result")


def wait(js):
    for _ in range(100):
        try:
            r = ev(js)
            if r:
                return r
        except Exception:
            pass
        time.sleep(0.1)
    raise Exception("Timed out: " + js)


def check(name, expr):
    assert expr, name
    results.append({"name": name, "pass": True})
    print("PASS", name, flush=True)


def click(label):
    ev(
        "(()=>{const b=[...document.querySelectorAll('button')].find(b=>b.getAttribute('aria-label')==="
        + json.dumps(label)
        + "||b.textContent.trim()==="
        + json.dumps(label)
        + ");if(!b)throw Error('missing button');b.click();return true})()"
    )


env = os.environ.copy()
env.update(
    QUIXI_TEST_PROFILE=str(out / "profile"),
    QUIXI_AUTOMATION_PORT=str(args.port),
    QUIXI_RESOURCES_DIR=str(root / "resources"),
    GDK_BACKEND="x11",
)
log = open(out / "app.log", "w")
proc = subprocess.Popen(
    [
        "xvfb-run",
        "-a",
        "dbus-run-session",
        "--",
        str(root / "target/release/QuixiDesk"),
    ],
    env=env,
    stdout=log,
    stderr=log,
    start_new_session=True,
)
try:
    wait("document.querySelectorAll('button').length>10")
    check(
        "original native UI loads", ev("document.body.innerText.includes('QuixiDesk')")
    )
    for origin in ["null", "https://example.com", fixture_url]:
        for route, body in [
            ("/state", None),
            ("/action", b'{"action":"eval","script":"window.__untrustedRan=true"}'),
        ]:
            req = urllib.request.Request(
                base + route,
                data=body,
                headers={"Origin": origin, "Content-Type": "application/json"},
            )
            try:
                urllib.request.urlopen(req, timeout=5)
                raise AssertionError("web origin was allowed")
            except urllib.error.HTTPError as error:
                check("automation " + route + " rejects " + origin, error.code == 403)
    check("rejected automation cannot execute", ev("window.__untrustedRan!==true"))
    ev(
        "window.__frameProbes=[];addEventListener('message',e=>{if(e.data?.testProbe)window.__frameProbes.push(e.data)});true"
    )
    click("Toggle QuixiDesk Browser")
    wait("!!document.getElementById('browser-frame')")
    ev(
        "(()=>{const i=document.querySelector('[aria-label=\"Browser address\"]');i.value="
        + json.dumps(fixture_url)
        + ";i.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));return true})()"
    )
    p = wait("window.__frameProbes.find(p=>p.page==='first')")
    check("remote page cannot read parent DOM", p["parentBlocked"])
    check("remote page cannot read application API", p["apiBlocked"])
    check("native bridge is absent from remote frame", p["nativeBridgeAbsent"])
    check(
        "browser frame has no same-origin privileges",
        "allow-same-origin"
        not in ev("document.getElementById('browser-frame').getAttribute('sandbox')"),
    )
    ev(
        "document.getElementById('browser-frame').contentWindow.postMessage({testNavigate:true},'*');true"
    )
    wait("window.__frameProbes.find(p=>p.page==='second')")
    check(
        "page link updates native URL",
        ev(
            "document.querySelector('[aria-label=\"Browser address\"]').value.endsWith('/next')"
        ),
    )
    before = ev("window.__frameProbes.length")
    click("Reload browser")
    wait("window.__frameProbes.length>" + str(before))
    check(
        "reload fetches a new page snapshot",
        ev("window.__frameProbes.at(-1).load") > p["load"],
    )
    before = ev("window.__frameProbes.length")
    click("Browser back")
    wait("window.__frameProbes.length>" + str(before))
    check(
        "back restores first page and address",
        ev(
            "window.__frameProbes.at(-1).page==='first'&&!document.querySelector('[aria-label=\"Browser address\"]').value.endsWith('/next')"
        ),
    )
    before = ev("window.__frameProbes.length")
    click("Browser forward")
    wait("window.__frameProbes.length>" + str(before))
    check(
        "forward restores second page",
        ev("window.__frameProbes.at(-1).page==='second'"),
    )
    check(
        "exactly one browser panel remains",
        ev("document.querySelectorAll('.browser-panel').length===1"),
    )
    click("Close browser")
    for label, marker in [
        ("Projects", "Group related work"),
        ("Artifacts", "Artifacts"),
        ("Schedules", "Schedules"),
        ("Tools & skills", "Tools & skills"),
        ("Settings", "Settings"),
    ]:
        click(label)
        wait("document.body.innerText.includes(" + json.dumps(marker) + ")")
        check(label + " page renders in native WebKit", True)
    print(json.dumps(results, indent=2))
finally:
    (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    server.shutdown()
    import signal

    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
    print("Evidence:", out, flush=True)
