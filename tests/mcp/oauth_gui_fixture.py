#!/usr/bin/env python3
"""Loopback-only OAuth + MCP fixture for native-app sign-in testing.

Run: python3 tests/mcp/oauth_gui_fixture.py --port-file /tmp/quixi-oauth-port
Configure an HTTP MCP server at http://127.0.0.1:<port>/mcp. Sign-in opens
an explicit Approve/Deny page; this fixture never opens a browser itself.
Credentials are disposable random in-memory values and are never logged.
"""
import argparse
import base64
import hashlib
import html
import http.server
import json
import secrets
import threading
import urllib.parse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--port", type=int, default=0)
parser.add_argument("--port-file")
parser.add_argument("--log")
args = parser.parse_args()
lock = threading.RLock()
clients = {}
consents = {}
codes = {}
tokens = set()
refresh_tokens = set()
sessions = set()


def log(event, **data):
    if args.log:
        with lock, open(args.log, "a", encoding="utf-8") as stream:
            stream.write(json.dumps(dict(event=event, **data)) + "\n")


def scalar(params, key):
    values = params.get(key, [])
    return values[0] if len(values) == 1 else ""


def callback_allowed(uri):
    parsed = urllib.parse.urlsplit(uri)
    return parsed.scheme == "http" and parsed.hostname == "127.0.0.1" and parsed.port and not parsed.username and not parsed.password and not parsed.fragment


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def respond(self, code, data=None, headers=None, mime="application/json"):
        body = b"" if data is None else data.encode() if isinstance(data, str) else json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Connection", "close")
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(body)

    def authorized(self):
        with lock:
            return self.headers.get("Authorization", "").removeprefix("Bearer ") in tokens

    def challenge(self):
        log("mcp_auth_required")
        self.respond(401, {"error": "Sign in to the local fixture"}, {"WWW-Authenticate": 'Bearer resource_metadata="' + BASE + '/.well-known/oauth-protected-resource/mcp", scope="tools:read"'})

    def do_GET(self):
        url = urllib.parse.urlsplit(self.path)
        if url.path.startswith("/.well-known/oauth-protected-resource"):
            self.respond(200, {"resource": BASE + "/mcp", "authorization_servers": [BASE], "scopes_supported": ["tools:read"]})
        elif url.path.startswith("/.well-known/oauth-authorization-server") or url.path.startswith("/.well-known/openid-configuration"):
            self.respond(200, {"issuer": BASE, "authorization_endpoint": BASE + "/authorize", "token_endpoint": BASE + "/token", "registration_endpoint": BASE + "/register", "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"], "code_challenge_methods_supported": ["S256"], "token_endpoint_auth_methods_supported": ["none"], "authorization_response_iss_parameter_supported": True})
        elif url.path == "/authorize":
            params = urllib.parse.parse_qs(url.query)
            client = scalar(params, "client_id")
            redirect = scalar(params, "redirect_uri")
            with lock:
                valid = client in clients and redirect in clients[client]
            if not valid or not callback_allowed(redirect) or scalar(params, "code_challenge_method") != "S256" or not scalar(params, "state") or not scalar(params, "code_challenge") or scalar(params, "resource") != BASE + "/mcp":
                self.respond(400, {"error": "Invalid authorization request"})
                return
            ticket = secrets.token_urlsafe(24)
            with lock:
                consents[ticket] = params
            log("consent_presented")
            page = '<!doctype html><html><head><meta name="viewport" content="width=device-width"><title>QuixiDesk test sign-in</title><style>body{font:18px system-ui;background:#161b22;color:#e7edf4;max-width:600px;margin:12vh auto;padding:30px}main{padding:32px;border:1px solid #45505d;border-radius:20px}button{font:inherit;padding:12px 24px;margin:10px 10px 0 0;border:0;border-radius:8px;background:#8ad5bf;color:#132521}button[value=deny]{background:#3d4550;color:#fff}</style></head><body><main><h1>Connect the local test server</h1><p>Allow QuixiDesk to use the fixture echo tool? This test requires no account, password, or external service.</p><p>Server: <strong>' + html.escape(BASE) + '</strong></p><form method="post" action="/authorize"><input type="hidden" name="ticket" value="' + html.escape(ticket) + '"><button name="decision" value="approve">Approve connection</button><button name="decision" value="deny">Deny</button></form></main></body></html>'
            self.respond(200, page, mime="text/html; charset=utf-8")
        elif url.path == "/mcp":
            if not self.authorized():
                self.challenge()
            else:
                self.respond(405, {"error": "No event stream is needed for this fixture"})
        else:
            self.respond(404, {"error": "Not found"})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        if length > 128 * 1024:
            self.respond(413)
            return
        raw = self.rfile.read(length)
        path = urllib.parse.urlsplit(self.path).path
        try:
            if path == "/register":
                data = json.loads(raw)
                redirects = data.get("redirect_uris", [])
                if not redirects or any(not callback_allowed(uri) for uri in redirects):
                    self.respond(400, {"error": "invalid_redirect_uri"})
                    return
                client = "fixture-" + secrets.token_hex(8)
                with lock:
                    clients[client] = redirects
                log("client_registered")
                self.respond(201, {"client_id": client, "redirect_uris": redirects, "token_endpoint_auth_method": "none", "grant_types": ["authorization_code", "refresh_token"], "response_types": ["code"]})
            elif path == "/authorize":
                params = urllib.parse.parse_qs(raw.decode())
                with lock:
                    original = consents.pop(scalar(params, "ticket"), None)
                if original is None:
                    self.respond(400, {"error": "Consent expired or was already answered"})
                    return
                response = {"state": scalar(original, "state"), "iss": BASE}
                if scalar(params, "decision") == "approve":
                    code = secrets.token_urlsafe(24)
                    with lock:
                        codes[code] = original
                    response["code"] = code
                    log("consent_approved")
                else:
                    response["error"] = "access_denied"
                    log("consent_denied")
                redirect = scalar(original, "redirect_uri")
                self.respond(303, headers={"Location": redirect + ("&" if "?" in redirect else "?") + urllib.parse.urlencode(response)})
            elif path == "/token":
                params = urllib.parse.parse_qs(raw.decode())
                grant = scalar(params, "grant_type")
                valid = False
                with lock:
                    if grant == "authorization_code":
                        original = codes.pop(scalar(params, "code"), None)
                        challenge = base64.urlsafe_b64encode(hashlib.sha256(scalar(params, "code_verifier").encode()).digest()).rstrip(b"=").decode()
                        valid = original is not None and secrets.compare_digest(challenge, scalar(original, "code_challenge")) and scalar(params, "client_id") == scalar(original, "client_id") and scalar(params, "redirect_uri") == scalar(original, "redirect_uri")
                    elif grant == "refresh_token":
                        previous = scalar(params, "refresh_token")
                        valid = previous in refresh_tokens
                        refresh_tokens.discard(previous)
                    if valid:
                        access, refresh = secrets.token_urlsafe(24), secrets.token_urlsafe(24)
                        tokens.add(access)
                        refresh_tokens.add(refresh)
                if not valid:
                    self.respond(400, {"error": "invalid_grant"})
                    return
                log("token_issued", grant=grant)
                self.respond(200, {"access_token": access, "refresh_token": refresh, "token_type": "Bearer", "expires_in": 3600, "scope": "tools:read"})
            elif path == "/mcp":
                if not self.authorized():
                    self.challenge()
                    return
                request = json.loads(raw)
                method = request.get("method")
                log("mcp_request", method=method)
                headers = {}
                if "id" not in request:
                    self.respond(202)
                    return
                if method == "initialize":
                    sid = secrets.token_urlsafe(12)
                    with lock:
                        sessions.add(sid)
                    headers["Mcp-Session-Id"] = sid
                    result = {"protocolVersion": "2025-11-25", "capabilities": {"tools": {}}, "serverInfo": {"name": "local-oauth-fixture", "version": "1"}}
                elif method == "tools/list":
                    result = {"tools": [{"name": "echo", "description": "Echo harmless text after real native sign-in", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}}]}
                elif method == "tools/call":
                    result = {"content": [{"type": "text", "text": request.get("params", {}).get("arguments", {}).get("text", "")}]}
                elif method == "ping":
                    result = {}
                else:
                    self.respond(200, {"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32601, "message": "Method not found"}})
                    return
                self.respond(200, {"jsonrpc": "2.0", "id": request["id"], "result": result}, headers)
            else:
                self.respond(404)
        except (ValueError, TypeError, KeyError):
            self.respond(400, {"error": "Invalid request"})

    def do_DELETE(self):
        self.respond(200 if self.authorized() else 401)


server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
BASE = "http://127.0.0.1:" + str(server.server_port)
if args.port_file:
    Path(args.port_file).write_text(str(server.server_port), encoding="utf-8")
print(json.dumps({"url": BASE + "/mcp", "transport": "http"}), flush=True)
log("started", url=BASE + "/mcp")
try:
    server.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    server.server_close()
