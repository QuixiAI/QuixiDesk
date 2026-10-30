#!/usr/bin/env python3
"""Deterministic MCP peer exercising genuine server-to-client elicitation RPCs."""
import json
import sys

pending = {}
sequence = 0
calls = 0


def send(value):
    print(json.dumps(dict(jsonrpc="2.0", **value)), flush=True)


def result(request_id, value):
    send(dict(id=request_id, result=value))


def tool_result(request_id, value):
    result(request_id, {"content": [{"type": "text", "text": json.dumps(value)}], "structuredContent": value})


for line in sys.stdin:
    try:
        message = json.loads(line)
        method = message.get("method")
        request_id = message.get("id")
        if method is None and request_id in pending:
            parent = pending.pop(request_id)
            tool_result(parent, {"response": message.get("result"), "error": message.get("error")})
        elif method == "initialize":
            result(request_id, {"protocolVersion": "2025-11-25", "capabilities": {"tools": {}}, "serverInfo": {"name": "interaction-fixture", "version": "1"}})
        elif method == "tools/list":
            result(request_id, {"tools": [{"name": name, "description": name, "inputSchema": {"type": "object", "properties": {}}} for name in ["ask_form", "ask_constraints", "unsupported_form", "ask_url", "secret_form", "unsafe_url", "complete_url", "url_required", "count_calls"]]})
        elif method == "tools/call":
            calls += 1
            name = message["params"]["name"]
            if name == "complete_url":
                send(dict(method="notifications/elicitation/complete", params={"elicitationId": "fixture-auth"}))
                tool_result(request_id, {"completed": True})
            elif name == "count_calls":
                tool_result(request_id, {"calls": calls})
            elif name == "url_required":
                send(dict(id=request_id, error={"code": -32042, "message": "Additional consent is needed", "data": {"elicitations": [{"mode": "url", "message": "Connect the fixture", "url": "https://example.com/fixture", "elicitationId": "fixture-auth"}]}}))
            else:
                sequence += 1
                elicit_id = "elicit-" + str(sequence)
                pending[elicit_id] = request_id
                if name in ("ask_url", "unsafe_url"):
                    params = {"mode": "url", "message": "Connect the fixture", "url": "https://example.com/fixture" if name == "ask_url" else "javascript:alert(1)", "elicitationId": "fixture-auth"}
                else:
                    field = "password" if name == "secret_form" else "nickname"
                    params = {"mode": "form", "message": "Choose a display name", "requestedSchema": {"type": "object", "properties": {field: {"type": "string", "minLength": 3, "maxLength": 20}}, "required": [field]}}
                if name == "ask_constraints":
                    params["requestedSchema"] = {"type": "object", "properties": {
                        "code": {"type": "string", "pattern": "^[A-Z]{3}$"},
                        "choices": {"type": "array", "minItems": 1, "maxItems": 2, "uniqueItems": True, "items": {"type": "string", "enum": ["one", "two"]}},
                        "count": {"type": "integer", "minimum": 2, "maximum": 8, "multipleOf": 2}
                    }, "required": ["code", "choices", "count"], "additionalProperties": False}
                elif name == "unsupported_form":
                    params["requestedSchema"]["allOf"] = [{"properties": {"nickname": {"const": "only-this"}}}]
                send(dict(id=elicit_id, method="elicitation/create", params=params))
        elif request_id is not None:
            send(dict(id=request_id, error={"code": -32601, "message": "Method not found"}))
    except Exception as error:
        print(str(error), file=sys.stderr, flush=True)
