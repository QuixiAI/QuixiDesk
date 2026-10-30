#!/usr/bin/env python3
"""HTTP fixture: execute a mutation, then expire its session before replying."""
import argparse
import http.server
import json
import threading
import uuid
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument('--port-file', required=True)
parser.add_argument('--closed-file')
args = parser.parse_args()
lock = threading.Lock()
sessions = set()
mutations = 0
class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *_): pass
    def response(self, status, value=None, headers=None):
        body = b'' if value is None else json.dumps(value).encode()
        self.send_response(status)
        self.send_header('Content-Type','application/json')
        self.send_header('Content-Length',str(len(body)))
        for key,value in (headers or {}).items(): self.send_header(key,value)
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self): self.response(405)
    def do_DELETE(self):
        if args.closed_file: Path(args.closed_file).write_text('closed')
        self.response(200)
    def do_POST(self):
        global mutations
        req = json.loads(self.rfile.read(int(self.headers.get('Content-Length','0'))))
        method = req.get('method')
        sid = self.headers.get('Mcp-Session-Id')
        headers = {}
        if method == 'initialize':
            sid = str(uuid.uuid4())
            with lock: sessions.add(sid)
            headers['Mcp-Session-Id'] = sid
            result = {'protocolVersion':'2025-11-25','capabilities':{'tools':{}},'serverInfo':{'name':'worker-fixture','version':'1'}}
        elif sid not in sessions: self.response(404,{'error':'expired'}); return
        elif method == 'notifications/initialized': self.response(202); return
        elif method == 'tools/list': result = {'tools':[{'name':name,'description':name,'inputSchema':{'type':'object','properties':{}}} for name in ['mutate','status']]}
        elif method == 'tools/call':
            if req['params']['name'] == 'mutate':
                with lock:
                    mutations += 1
                    sessions.discard(sid)
                self.response(404,{'error':'expired after mutation'})
                return
            result = {'content':[{'type':'text','text':str(mutations)}],'structuredContent':{'mutations':mutations,'session':sid}}
        else: result = {}
        self.response(200,{'jsonrpc':'2.0','id':req['id'],'result':result},headers)
server = http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
Path(args.port_file).write_text(str(server.server_port))
try: server.serve_forever()
except KeyboardInterrupt: pass
finally: server.server_close()
