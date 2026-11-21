#!/usr/bin/env python3
"""Minimal MCP stdio handshake and tool-call smoke test."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = os.environ.get("MTR_MCP_BIN", str(ROOT / "target" / "release" / "mtr-hk-mcp"))


class Client:
    def __init__(self) -> None:
        self.process = subprocess.Popen(
            [BIN],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        self.next_id = 1

    def send(self, method: str, params: dict | None = None) -> dict:
        request_id = self.next_id
        self.next_id += 1
        payload = {"jsonrpc": "2.0", "id": request_id, "method": method}
        if params is not None:
            payload["params"] = params
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(payload) + "\n")
        self.process.stdin.flush()

        assert self.process.stdout is not None
        while True:
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError("MCP server closed stdout")
            line = line.strip()
            if not line:
                continue
            message = json.loads(line)
            if message.get("id") == request_id:
                if "error" in message:
                    raise RuntimeError(message["error"])
                return message.get("result", {})

    def notify(self, method: str, params: dict | None = None) -> None:
        payload = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            payload["params"] = params
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(payload) + "\n")
        self.process.stdin.flush()

    def close(self) -> None:
        if self.process.stdin:
            self.process.stdin.close()
        self.process.terminate()
        self.process.wait(timeout=5)


def tool_text(result: dict) -> dict:
    content = result.get("content", [])
    text = next((item.get("text") for item in content if item.get("type") == "text"), None)
    if text is None:
        raise AssertionError("tool returned no text content")
    return json.loads(text)


def main() -> int:
    if not Path(BIN).is_file():
        print(f"Binary not found: {BIN}. Run cargo build --release.", file=sys.stderr)
        return 1

    client = Client()
    try:
        initialized = client.send(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "mtr-hk-smoke", "version": "1.0"},
            },
        )
        print(f"Initialized: {initialized['serverInfo']['name']} {initialized['serverInfo']['version']}")
        client.notify("notifications/initialized")

        listed = client.send("tools/list", {})
        names = [tool["name"] for tool in listed["tools"]]
        print(f"Tools exposed ({len(names)}): {', '.join(names)}")
        if len(names) < 15:
            raise AssertionError(f"expected at least 15 tools, got {len(names)}")
        for required in ("mtr_status", "mtr_ferries", "mtr_ferry_eta"):
            if required not in names:
                raise AssertionError(f"missing tool {required}")

        status = client.send("tools/call", {"name": "mtr_status", "arguments": {}})
        status_data = tool_text(status)
        print(f"mtr_status OK: {len(status_data['lines'])} lines, overall {status_data['overall']}")

        ferries = client.send("tools/call", {"name": "mtr_ferries", "arguments": {}})
        ferry_data = tool_text(ferries)
        if len(ferry_data["operators"]) < 16:
            raise AssertionError("expected 16 ferry operators")
        print(f"mtr_ferries OK: {ferry_data['totalRouteCount']} routes")

        eta = client.send(
            "tools/call",
            {"name": "mtr_ferry_eta", "arguments": {"operator": "hkkf", "route": "1"}},
        )
        eta_data = tool_text(eta)
        if not eta_data.get("live"):
            raise AssertionError("expected live HKKF ETA")
        print(f"mtr_ferry_eta OK: {len(eta_data.get('etas', []))} sailing(s)")

        invalid = client.send(
            "tools/call",
            {
                "name": "mtr_plan_journey",
                "arguments": {"from": "Not A Station", "to": "Admiralty"},
            },
        )
        if not invalid.get("isError"):
            raise AssertionError("invalid station should produce a tool error")
        print("invalid input returns a tool error OK")
        print("MCP smoke test passed.")
        return 0
    finally:
        client.close()


if __name__ == "__main__":
    raise SystemExit(main())
