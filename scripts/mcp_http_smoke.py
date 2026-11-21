#!/usr/bin/env python3
"""Smoke-test a Streamable HTTP MCP endpoint."""
from __future__ import annotations

import json
import os
import re
import sys
import urllib.error
import urllib.request

URL = os.environ.get("MCP_URL", "http://127.0.0.1:8080/mcp")
HEALTH_URL = os.environ.get("MCP_HEALTH_URL", URL.removesuffix("/mcp") + "/health")


def request(url: str, payload: dict, session_id: str | None = None) -> tuple[dict | None, str | None]:
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
    }
    if session_id:
        headers["Mcp-Session-Id"] = session_id
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode(),
        headers=headers,
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=45) as response:
        body = response.read().decode()
        returned_session = response.headers.get("Mcp-Session-Id")
        if not body.strip():
            return None, returned_session
        if response.headers.get_content_type() == "application/json":
            return json.loads(body), returned_session
        data = "".join(re.findall(r"^data: (.*)$", body, re.M))
        return json.loads(data), returned_session


def main() -> int:
    try:
        with urllib.request.urlopen(HEALTH_URL, timeout=15) as response:
            health = json.load(response)
        print(f"Health OK: {health['status']} ({health['transport']})")

        initialized, session_id = request(
            URL,
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "mtr-http-smoke", "version": "1.0"},
                },
            },
        )
        assert initialized is not None
        info = initialized["result"]["serverInfo"]
        print(f"Initialized: {info['name']} {info['version']}")

        request(
            URL,
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            session_id,
        )

        listed, _ = request(URL, {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}, session_id)
        assert listed is not None
        names = [tool["name"] for tool in listed["result"]["tools"]]
        print(f"Tools exposed ({len(names)}): {', '.join(names)}")
        if len(names) < 15:
            raise AssertionError(f"expected at least 15 tools, got {len(names)}")

        called, _ = request(
            URL,
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "mtr_ferries", "arguments": {}},
            },
            session_id,
        )
        assert called is not None
        if called["result"].get("isError"):
            raise AssertionError("mtr_ferries returned a tool error")
        ferries = json.loads(called["result"]["content"][0]["text"])
        print(f"mtr_ferries OK: {len(ferries['operators'])} operators, {ferries['totalRouteCount']} routes")
        print("HTTP MCP smoke test passed.")
        return 0
    except (urllib.error.URLError, TimeoutError, KeyError, AssertionError) as error:
        print(f"HTTP MCP smoke test failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
