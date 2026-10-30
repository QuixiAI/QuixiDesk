#!/usr/bin/env python3
"""Deterministic, dependency-free MCP peer for QuixiDesk integration tests.

This process never opens files requested by tools or executes arbitrary commands.
All logs and marker files must be explicitly supplied by the test harness.
"""
import argparse
import http.server
import json
import os
import queue
import sys
import threading
import time
import urllib.parse
import uuid
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--transport', choices=['stdio', 'http', 'sse'], default='stdio')
parser.add_argument('--scenario', default='normal')
parser.add_argument('--log')
parser.add_argument('--pid-file')
parser.add_argument('--port-file')
parser.add_argument('--port', type=int, default=0)
parser.add_argument('--token', default='fixture-token')
args = parser.parse_args()
log_lock = threading.Lock()
stdout_lock = threading.Lock()

def log(event, **fields):
    if args.log:
        with log_lock, open(args.log, 'a', encoding='utf-8') as f:
            f.write(json.dumps(dict(event=event, pid=os.getpid(), at=time.monotonic(), **fields)) + '\n')

if args.pid_file:
    Path(args.pid_file).write_text(str(os.getpid()), encoding='utf-8')
log('started', transport=args.transport, scenario=args.scenario)
print('QuixiDesk MCP fixture diagnostic: startup complete', file=sys.stderr, flush=True)

def tool(name, description, properties=None, required=None, output=None):
    value = dict(name=name, description=description,
                 inputSchema=dict(type='object', properties=properties or {}, required=required or [], additionalProperties=False),
                 annotations=dict(readOnlyHint=name not in ['crash', 'mutate_catalog'], destructiveHint=False),
                 _meta={'fixture/metadata': 'preserve-me'})
    if output:
        value['outputSchema'] = output
    return value

BASE_TOOLS = [
    tool('echo', 'Echo a string using the discovered argument schema.', {'text': {'type': 'string', 'minLength': 1}}, ['text']),
    tool('counter', 'Return an incrementing counter and this server process identity.', output={
        'type': 'object', 'properties': {'count': {'type': 'integer'}, 'pid': {'type': 'integer'}},
        'required': ['count', 'pid'], 'additionalProperties': False}),
    tool('mutate_catalog', 'Add one tool and notify the client that the catalog changed.'),
    tool('delay', 'Delay until cancelled or the timeout expires.', {'seconds': {'type': 'number', 'minimum': 0}}, []),
    tool('crash', 'Exit this fixture process immediately.'),
    tool('fail', 'Return an explicit tool error result.'),
    tool('request_error', 'Return a JSON-RPC error.'),
]

class Peer:
    def __init__(self):
        self.id = str(uuid.uuid4())
        self.initialized = False
        self.initialize_replied = False
        self.count = 0
        self.mutated = False
        self.cancelled = set()
        self.events = queue.Queue()
        self.lock = threading.Lock()

    def result(self, req, value):
        if args.scenario == 'modern':
            value = dict(value, resultType='complete', ttlMs=0, cacheScope='private')
        return {'jsonrpc': '2.0', 'id': req['id'], 'result': value}

    def error(self, req, code, message):
        return {'jsonrpc': '2.0', 'id': req.get('id'), 'error': {'code': code, 'message': message}}

    def notify(self, method, params=None):
        value = dict(jsonrpc='2.0', method=method)
        if params is not None:
            value['params'] = params
        self.events.put(value)

    def receive(self, req):
        method = req.get('method', '')
        log('request', method=method, requestId=req.get('id'), initialized=self.initialized,
            initializeReplied=self.initialize_replied, params=req.get('params', {}), session=self.id)
        if method == 'notifications/cancelled':
            self.cancelled.add(req.get('params', {}).get('requestId'))
            log('cancelled', requestId=req.get('params', {}).get('requestId'))
            return None
        if method == 'notifications/initialized':
            if not self.initialize_replied:
                log('order_violation', method=method)
            self.initialized = self.initialize_replied
            return None
        if 'id' not in req:
            return None
        if method == 'server/discover':
            if args.scenario == 'modern_rejection':
                return self.error(req, -32021, 'Required client capability is missing')
            if args.scenario != 'modern':
                return self.error(req, -32601, 'Server uses initialize lifecycle')
            self.initialized = True
            return self.result(req, {'supportedVersions': ['2026-07-28'],
                'capabilities': {'tools': {}, 'resources': {}, 'prompts': {}},
                '_meta': {'io.modelcontextprotocol/serverInfo': {'name': 'modern-fixture', 'version': '1.0.0'}}})
        if method == 'initialize':
            if args.scenario == 'modern':
                return self.error(req, -32601, 'Modern server does not support initialize')
            if args.scenario == 'initialize_timeout':
                time.sleep(15)
            if args.scenario == 'invalid_initialize':
                return self.result(req, {'serverInfo': {'name': 'invalid'}})
            protocol = req.get('params', {}).get('protocolVersion', '2025-11-25')
            if args.scenario == 'unsupported_version':
                protocol = '1900-01-01'
            caps = {'tools': {'listChanged': True}, 'resources': {'listChanged': True}, 'prompts': {'listChanged': True}}
            if args.scenario == 'missing_tools_capability':
                caps.pop('tools')
            self.initialize_replied = True
            return self.result(req, {'protocolVersion': protocol, 'capabilities': caps,
                'serverInfo': {'name': 'quixi-fixture', 'version': '1.0.0'},
                'instructions': 'Fixture instructions: use discovered schemas; never invent tools.'})
        if not self.initialized:
            log('order_violation', method=method)
            return self.error(req, -32002, 'Initialization must finish before requests')
        if method == 'ping':
            return self.result(req, {})
        if method == 'tools/list':
            if args.scenario == 'discovery_timeout':
                time.sleep(15)
            if args.scenario == 'missing_tools_capability':
                return self.error(req, -32601, 'Tools capability not advertised')
            if args.scenario == 'discovery_error' or (args.scenario == 'refresh_error' and self.mutated):
                return self.error(req, -32099, 'Deliberate discovery failure')
            if args.scenario == 'zero_tools':
                return self.result(req, {'tools': []})
            tools = list(BASE_TOOLS)
            if self.mutated:
                tools.append(tool('newly_available', 'A tool added after the initial connection.'))
            if args.scenario == 'invalid_schema':
                tools[0] = tool('bad_schema', 'Invalid schema', {'x': {'type': 'not-a-json-schema-type'}})
            if args.scenario == 'external_schema':
                tools[0]['inputSchema'] = {'$ref': 'https://example.invalid/schema.json'}
            if args.scenario == 'complex_schema':
                tools[0]['inputSchema'] = {
                    'type': 'object',
                    '$defs': {'message': {'type': 'string', 'pattern': '^allowed-[0-9]+$'}},
                    'properties': {'text': {'$ref': '#/$defs/message'}},
                    'required': ['text'], 'additionalProperties': False}
            if args.scenario == 'large_catalog':
                tools = [tool('item_%03d' % i, 'Page fixture',
                    {'text': {'type': 'string', 'description': 'x' * (70000 if i == 0 else 5000)}})
                    for i in range(45)]
            cursor = req.get('params', {}).get('cursor')
            if args.scenario in ['paginated', 'cursor_loop', 'duplicate_tools']:
                if not cursor:
                    return self.result(req, {'tools': tools[:1], 'nextCursor': 'page-2'})
                if args.scenario == 'cursor_loop':
                    return self.result(req, {'tools': tools[1:2], 'nextCursor': 'page-2'})
                if args.scenario == 'duplicate_tools':
                    return self.result(req, {'tools': tools[:1]})
                return self.result(req, {'tools': tools[1:]})
            return self.result(req, {'tools': tools})
        if method == 'resources/list':
            if args.scenario == 'resources_error':
                return self.error(req, -32099, 'Deliberate resource discovery failure')
            if args.scenario == 'resource_prompt_pages':
                if not req.get('params', {}).get('cursor'):
                    return self.result(req, {'resources': [{'uri': 'fixture://first', 'name': 'First resource'}], 'nextCursor': 'second'})
            return self.result(req, {'resources': [{'uri': 'fixture://readme', 'name': 'Fixture README', 'mimeType': 'text/plain'}]})
        if method == 'resources/templates/list':
            return self.result(req, {'resourceTemplates': []})
        if method == 'resources/read':
            return self.result(req, {'contents': [{'uri': req.get('params', {}).get('uri', 'fixture://readme'), 'text': 'Fixture resource', 'mimeType': 'text/plain'}]})
        if method == 'prompts/list':
            if args.scenario == 'resource_prompt_pages':
                if not req.get('params', {}).get('cursor'):
                    return self.result(req, {'prompts': [{'name': 'first_prompt'}], 'nextCursor': 'second'})
            return self.result(req, {'prompts': [{'name': 'fixture_prompt', 'description': 'Fixture prompt'}]})
        if method == 'prompts/get':
            return self.result(req, {'messages': [{'role': 'user', 'content': {'type': 'text', 'text': 'Fixture prompt body'}}]})
        if method != 'tools/call':
            return self.error(req, -32601, 'Unknown protocol method: ' + method)
        params = req.get('params', {})
        name = params.get('name', '')
        inputs = params.get('arguments', {})
        if name == 'request_error':
            return self.error(req, -32042, 'Deliberate JSON-RPC failure')
        if name == 'crash':
            log('crash')
            os._exit(17)
        if name == 'delay':
            end = time.monotonic() + inputs.get('seconds', 5)
            while time.monotonic() < end:
                if req['id'] in self.cancelled:
                    return self.error(req, -32800, 'Request cancelled')
                time.sleep(.01)
        if name == 'fail':
            return self.result(req, {'content': [{'type': 'text', 'text': 'Deliberate tool failure; no action was performed.'}], 'isError': True})
        if name == 'counter':
            with self.lock:
                self.count += 1
                value = {'count': self.count, 'pid': os.getpid()}
            if args.scenario == 'invalid_output':
                value['count'] = 'wrong type'
            return self.result(req, {'content': [{'type': 'text', 'text': json.dumps(value)}], 'structuredContent': value, 'isError': False})
        if name == 'mutate_catalog':
            self.mutated = True
            self.notify('notifications/tools/list_changed')
            return self.result(req, {'content': [{'type': 'text', 'text': 'Catalog changed.'}], 'isError': False})
        if name == 'echo':
            if not isinstance(inputs.get('text'), str) or not inputs['text']:
                return self.error(req, -32602, 'text must be a nonempty string')
            text = inputs['text']
        elif name == 'newly_available' and self.mutated:
            text = 'Fresh catalog tool executed.'
        elif name == 'delay':
            text = 'Delay completed.'
        else:
            return self.error(req, -32602, 'Unknown tool: ' + name)
        return self.result(req, {'content': [{'type': 'text', 'text': text}], 'isError': False})


def stdio():
    peer = Peer()
    def emit(value):
        if value is not None:
            with stdout_lock:
                print(json.dumps(value), flush=True)
    def run(req):
        emit(peer.receive(req))
    def notifications():
        while True:
            emit(peer.events.get())
    threading.Thread(target=notifications, daemon=True).start()
    if args.scenario == 'malformed_stdout':
        print('THIS IS NOT JSON-RPC', flush=True)
    for line in sys.stdin:
        try:
            req = json.loads(line)
        except ValueError:
            log('invalid_client_json')
            continue
        # Long-running tools must not block cancellation/other requests.
        if (req.get('method') == 'tools/call' and req.get('params', {}).get('name') == 'delay') or args.scenario in ['initialize_timeout', 'discovery_timeout']:
            threading.Thread(target=run, args=(req,), daemon=True).start()
        else:
            run(req)
    log('stdin_closed')

sessions = {}
sessions_lock = threading.Lock()
class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *_):
        pass
    def response(self, code, body=b'', mime='application/json', headers=None):
        self.send_response(code)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(body)))
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        if body:
            self.wfile.write(body)
            self.wfile.flush()
    def unauthorized(self):
        if args.scenario == 'http_401' and self.headers.get('Authorization') != 'Bearer ' + args.token:
            self.response(401, b'{"error":"authorization_required"}', headers={'WWW-Authenticate': 'Bearer realm="fixture"'})
            return True
        return False
    def peer(self):
        sid = self.headers.get('Mcp-Session-Id') or urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query).get('sessionId', [None])[0]
        return sessions.get(sid)
    def do_POST(self):
        if self.unauthorized():
            return
        if args.scenario == 'sse_post_401':
            self.response(401, headers={'WWW-Authenticate': 'Bearer realm="fixture"'})
            return
        try:
            req = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
        except ValueError:
            self.response(400)
            return
        peer = self.peer()
        if peer is None and req.get('method') == 'initialize':
            peer = Peer()
            with sessions_lock:
                sessions[peer.id] = peer
        if peer is None:
            self.response(404, b'{"error":"session_not_found"}')
            return
        if args.transport == 'sse':
            if 'id' not in req:
                peer.receive(req)
                self.response(202)
                return
            self.response(202)
            def run():
                value = peer.receive(req)
                if value is not None:
                    peer.events.put(value)
            threading.Thread(target=run, daemon=True).start()
        else:
            value = peer.receive(req)
            if value is None:
                self.response(202)
            else:
                self.response(200, json.dumps(value).encode(), headers={'Mcp-Session-Id': peer.id})
    def do_GET(self):
        if self.unauthorized():
            return
        peer = self.peer()
        if args.transport == 'sse' and urllib.parse.urlsplit(self.path).path == '/sse':
            peer = Peer()
            with sessions_lock:
                sessions[peer.id] = peer
        if peer is None:
            self.response(404)
            return
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Cache-Control', 'no-cache')
        self.end_headers()
        try:
            if args.transport == 'sse':
                endpoint = '/messages?sessionId=' + peer.id
                if args.scenario == 'sse_cross_origin':
                    endpoint = 'http://127.0.0.1:1/messages'
                self.wfile.write(('event: endpoint\ndata: ' + endpoint + '\n\n').encode())
                self.wfile.flush()
            while True:
                try:
                    event = peer.events.get(timeout=.3)
                    self.wfile.write(('event: message\ndata: ' + json.dumps(event) + '\n\n').encode())
                except queue.Empty:
                    self.wfile.write(b': keepalive\n\n')
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, OSError):
            pass
    def do_DELETE(self):
        peer = self.peer()
        if peer is not None:
            with sessions_lock:
                sessions.pop(peer.id, None)
        self.response(200)

if args.transport == 'stdio':
    stdio()
else:
    server = http.server.ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    server.daemon_threads = True
    if args.port_file:
        Path(args.port_file).write_text(str(server.server_port), encoding='utf-8')
    log('http_listening', port=server.server_port)
    server.serve_forever()
