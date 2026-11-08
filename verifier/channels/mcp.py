"""MCP channel: SONE's own MCP server (streamable HTTP, JSON-RPC), stdlib only."""
import json
import time
import urllib.request

_NO_PROXY = urllib.request.build_opener(urllib.request.ProxyHandler({}))


class McpError(RuntimeError):
    pass


class Mcp:
    def __init__(self, url: str):
        self.url, self.session, self.n = url, None, 0

    def _post(self, msg: dict, timeout: float = 15):
        headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        if self.session:
            headers["Mcp-Session-Id"] = self.session
        req = urllib.request.Request(self.url, json.dumps(msg).encode(), headers)
        with _NO_PROXY.open(req, timeout=timeout) as r:
            self.session = r.headers.get("Mcp-Session-Id", self.session)
            body = r.read().decode()
        if not body.strip():
            return None
        if body.lstrip().startswith("{"):
            return json.loads(body)
        for line in body.splitlines():
            if line.startswith("data:") and line[5:].strip():
                obj = json.loads(line[5:])
                if "id" in obj:
                    return obj
        return None

    def _rpc(self, method: str, params: dict):
        self.n += 1
        return self._post({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params})

    def connect(self) -> "Mcp":
        self._rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                 "clientInfo": {"name": "sone-verifier", "version": "0"}})
        self._post({"jsonrpc": "2.0", "method": "notifications/initialized"})
        return self

    def call(self, tool: str, args: dict | None = None):
        """Call a tool; return its text content parsed as JSON when possible."""
        r = self._rpc("tools/call", {"name": tool, "arguments": args or {}})
        if r is None or "error" in r:
            raise McpError(f"{tool}: {r and r.get('error')}")
        res = r["result"]
        texts = [c.get("text", "") for c in res.get("content", []) if c.get("type") == "text"]
        out = texts[0] if len(texts) == 1 else "\n".join(texts)
        try:
            out = json.loads(out)
        except (ValueError, TypeError):
            pass
        return {"error": out} if res.get("isError") else out

    def tools(self) -> list[str]:
        r = self._rpc("tools/list", {})
        return sorted(t["name"] for t in r["result"]["tools"])


def wait_ready(url: str, timeout: float = 40) -> Mcp:
    deadline, last = time.time() + timeout, None
    while time.time() < deadline:
        try:
            return Mcp(url).connect()
        except Exception as e:  # noqa: BLE001 - startup races are expected
            last = e
            time.sleep(0.3)
    raise TimeoutError(f"MCP not ready at {url}: {last}")
