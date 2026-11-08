"""Network channel: the fake TIDAL's request log (JSONL, one line per request).

Steps take a mark before acting and read what arrived since, so each request is attributed to
the step that caused it. Normalization drops what legitimately varies between runs or builds.
"""
import json
import os
from urllib.parse import parse_qsl, urlsplit

# Query parameters that carry no behaviour (client fingerprinting, cache busting).
VOLATILE_QUERY = {"deviceType", "locale", "_", "t", "ts"}
# Credential-like values (embedded API keys, signatures, tokens): compared by presence only,
# and never written into goldens.
SECRET_QUERY = {"api_key", "api_sig", "token", "sk", "client_secret", "client_id"}
# Hosts whose traffic is not compared step by step: artwork is fetched by different code paths
# in the two builds, and the update check fires on a timer.
UNCOMPARED_HOSTS = {"resources.tidal.com", "api.github.com"}


class FlowLog:
    def __init__(self, path: str):
        self.path = path

    def mark(self) -> int:
        return os.path.getsize(self.path) if os.path.exists(self.path) else 0

    def since(self, mark: int) -> list[dict]:
        if not os.path.exists(self.path):
            return []
        with open(self.path, "rb") as f:
            f.seek(mark)
            return [json.loads(l) for l in f.read().decode().splitlines() if l.strip()]


def normalize(flow: dict) -> str | None:
    """'METHOD host/path?sorted&query' for compared traffic, else None."""
    if flow["host"] in UNCOMPARED_HOSTS:
        return None
    u = urlsplit(flow["path"])
    q = sorted((k, "<redacted>" if k in SECRET_QUERY else v)
               for k, v in parse_qsl(u.query, keep_blank_values=True) if k not in VOLATILE_QUERY)
    qs = "&".join(f"{k}={v}" for k, v in q)
    return f"{flow['method']} {flow['host']}{u.path}{'?' + qs if qs else ''}"


def calls(flows: list[dict]) -> list[str]:
    """Sorted, de-duplicated request set. Retries and Range re-fetches collapse to one entry,
    since how often a stream is re-requested is an implementation detail."""
    return sorted({n for f in flows if (n := normalize(f))})
