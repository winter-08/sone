#!/usr/bin/env python3
"""How long loading and navigation take when TIDAL answers slowly (fake_tidal.py's
SPIKE_LATENCY_MS), and how many requests each action makes.

    latency.py run --bin BIN [--bin BIN ...] [--latency-ms 300] [--slow REGEX=MS,...] [--out FILE]
    latency.py drive MODE OUT.json        (inside the sandbox; run starts it)

Modes, one launch each:
  seq       launch -> Home content; then, one after another with Home in between: a playlist
            (header, first tracks), an album, an artist, Loved Tracks, a search, a Home tab
  play      launch -> play pressed at once (MCP play_tracks) -> MPRIS Playing
  playlist  launch -> the sidebar playlist clicked as soon as it shows (during the start-up burst)
  album     launch -> an album on Home clicked as soon as it shows
  search    launch -> a search typed as soon as the field shows
Times are seconds, measured by polling the accessibility tree (a walk takes about 50 ms);
"launch" is the app process's start. Requests are counted per endpoint (ids folded) from the fake
TIDAL's log, from the action until the network has been quiet for a second.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)

PLAYLIST_BTN = "Fixture Mixtape Fixture Mixtape You · 4 Tracks"
API_HOSTS = {"api.tidal.com", "auth.tidal.com", "openapi.tidal.com"}


# ---------------------------------------------------------------- inside the sandbox

def proc_start() -> float:
    """Wall-clock start of the app process (SONE_PID: the wrapper, which execs the app)."""
    pid = os.environ["SONE_PID"]
    ticks = int(open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[19])
    uptime = float(open("/proc/uptime").read().split()[0])
    return time.time() - uptime + ticks / os.sysconf("SC_CLK_TCK")


def drive(mode: str, out_path: str):
    from channels import atspi, mcp, mpris, net
    run = os.environ["RUN"]
    flows = net.FlowLog(os.path.join(run, "logs", "flows.jsonl"))
    t_launch = proc_start()
    res = {"mode": mode, "times": {}, "requests": {}, "errors": []}

    def snap():
        a = atspi.app()
        return [(r, n) for _, r, n, _ in atspi.walk(a)] if a is not None else []

    def has(s, role, name):
        return any(r == role and n == name for r, n in s)

    def count(s, role, name):
        return sum(1 for r, n in s if r == role and n == name)

    def starts(s, role, prefix):
        return any(r == role and n.startswith(prefix) for r, n in s)

    def wait(preds: dict, t0: float, timeout: float = 20.0, tag: str = "") -> dict:
        got = {}
        deadline = time.time() + timeout
        while len(got) < len(preds) and time.time() < deadline:
            s = snap()
            now = time.time()
            for k, p in preds.items():
                if k not in got and p(s):
                    got[k] = round(now - t0, 3)
            time.sleep(0.02)
        for k in preds:
            if k not in got:
                got[k] = None
                res["errors"].append(f"{tag}{k}: not seen in {timeout}s")
                res.setdefault("dumps", {})[f"{tag}{k}"] = sorted({f"{r}: {n}" for r, n in snap() if n})
        return got

    def quiet(mark: int, idle: float = 1.0, cap: float = 15.0) -> list:
        last, last_change, deadline = flows.mark(), time.time(), time.time() + cap
        while time.time() < deadline and time.time() - last_change < idle:
            time.sleep(0.1)
            m = flows.mark()
            if m != last:
                last, last_change = m, time.time()
        return flows.since(mark)

    def endpoint(f):
        path = f["path"].split("?")[0]
        path = re.sub(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", "{uuid}", path)
        path = re.sub(r"/\d+(?=/|$)", "/{id}", path)
        q = dict(p.split("=", 1) for p in f["path"].partition("?")[2].split("&") if "=" in p)
        if "/users/{id}" in path and re.search(r"/users/0(/|\?|$)", f["path"]):
            path = path.replace("/users/{id}", "/users/0")
        extra = f" limit={q['limit']}" if "limit" in q and "favorites" in path else ""
        return f"{f['method']} {path}{extra}"

    def record(tag: str, fl: list, t0: float):
        api = [f for f in fl if f["host"] in API_HOSTS]
        counts = {}
        for f in api:
            counts[endpoint(f)] = counts.get(endpoint(f), 0) + 1
        res["requests"][tag] = {"total": len(api), "by_endpoint": dict(sorted(counts.items())),
                                "last_done": round(max((f["t"] for f in api), default=t0) - t0, 3),
                                "timeline": [[round(f.get("t0", f["t"]) - t0, 3), round(f["t"] - t0, 3), endpoint(f)] for f in api]}

    home_content = lambda s: has(s, "heading", "Fixture Albums")

    def go_home():
        atspi.press("Home", "button")
        wait({"home": home_content}, time.time(), tag="back home: ")
        quiet(flows.mark())

    def action(tag, press, preds, t_ref=None):
        """Press, then time each predicate from the press; record the requests it caused."""
        mark = flows.mark()
        t0 = time.time()
        try:
            press()
        except Exception as e:  # noqa: BLE001
            res["errors"].append(f"{tag}: {type(e).__name__}: {e}")
            return
        got = wait(preds, t0, tag=f"{tag}: ")
        res["times"][tag] = got
        if t_ref is not None:
            res["times"][tag + " (from launch)"] = {k: (round(v + t0 - t_ref, 3) if v is not None else None) for k, v in got.items()}
        record(tag, quiet(mark), t0)

    atspi.wait_ready(60)

    if mode == "seq":
        res["times"]["launch"] = wait({"home content": home_content,
                                       "sidebar playlist": lambda s: has(s, "button", PLAYLIST_BTN)}, t_launch, timeout=40, tag="launch: ")
        record("launch", quiet(0, idle=2.0, cap=40), t_launch)
        base = snap()
        action("playlist", lambda: atspi.press(PLAYLIST_BTN), {
            "header": lambda s: has(s, "heading", "Fixture Mixtape") and not has(s, "heading", "Fixture Albums"),
            "first tracks": lambda s: has(s, "entry", "Filter playlist on title, artist or album") and has(s, "label", "Queue B")})
        go_home()
        base = snap()
        action("album", lambda: atspi.press("Fixture Album", "heading"), {
            "header": lambda s: has(s, "button", "Share album"),
            "first tracks": lambda s: has(s, "button", "Share album") and has(s, "label", "Fixture 1004")})
        go_home()
        action("artist", lambda: atspi.press("Mock Ensemble", "heading"), {
            "header": lambda s: has(s, "button", "Follow"),
            "top tracks": lambda s: has(s, "heading", "Top tracks")})
        go_home()
        base = snap()
        action("loved tracks", lambda: atspi.press("Loved Tracks Collection"), {
            "header": lambda s: has(s, "heading", "Loved Tracks"),
            "content": lambda s: has(s, "heading", "Loved Tracks") and (has(s, "label", "No loved tracks yet") or count(s, "button", "Remove from favorites") > count(base, "button", "Remove from favorites"))})
        go_home()

        def search():
            atspi.focus("Search", "entry")
            time.sleep(0.2)
            atspi.type_text("fixture")
            atspi.key("Return")
        action("search", search, {"results": lambda s: has(s, "button", "All Results") or has(s, "tab", "All Results")})
        go_home()
        s = snap()
        if any(n == "Staff Picks" for _, n in s):
            action("home tab", lambda: atspi.press("Staff Picks"), {"content": lambda s: has(s, "heading", "Staff Picks Albums")})
        else:
            res["times"]["home tab"] = {"content": "no tabs"}
    elif mode == "play":
        m = mcp.wait_ready(os.environ.get("MCP_URL", "http://127.0.0.1:5577/verifier/mcp"))
        pl = mpris.wait_ready()
        mark = flows.mark()
        t0 = time.time()
        m.call("play_tracks", {"track_ids": [1005], "action": "play_now"})
        playing = None
        while time.time() - t0 < 30:
            if pl.snapshot().get("status") == "Playing":
                playing = round(time.time() - t0, 3)
                break
            time.sleep(0.02)
        res["times"]["play at once"] = {"pressed (from launch)": round(t0 - t_launch, 3), "playing": playing,
                                        "playing (from launch)": round(t0 - t_launch + playing, 3) if playing else None}
        record("play at once", quiet(mark, idle=2.0, cap=30), t0)
    elif mode == "playlist":
        wait({"button": lambda s: has(s, "button", PLAYLIST_BTN)}, t_launch, timeout=40)
        action("playlist during burst", lambda: atspi.press(PLAYLIST_BTN), {
            "header": lambda s: has(s, "heading", "Fixture Mixtape") and not has(s, "heading", "Fixture Albums"),
            "first tracks": lambda s: has(s, "entry", "Filter playlist on title, artist or album") and has(s, "label", "Queue B")}, t_ref=t_launch)
    elif mode == "album":
        wait({"card": lambda s: has(s, "heading", "Fixture Album")}, t_launch, timeout=40)
        action("album during burst", lambda: atspi.press("Fixture Album", "heading"), {
            "header": lambda s: has(s, "button", "Share album"),
            "first tracks": lambda s: has(s, "button", "Share album") and has(s, "label", "Fixture 1004")}, t_ref=t_launch)
    elif mode == "search":
        wait({"field": lambda s: has(s, "entry", "Search")}, t_launch, timeout=40)

        def search():
            atspi.focus("Search", "entry")
            time.sleep(0.2)
            atspi.type_text("fixture")
            atspi.key("Return")
        action("search during burst", search, {"results": lambda s: has(s, "button", "All Results") or has(s, "tab", "All Results")}, t_ref=t_launch)
    json.dump(res, open(out_path, "w"), indent=1)


# ---------------------------------------------------------------- outside: launch and tabulate

BASE_PROFILE = {
    "auth_tokens": {"access_token": "fake-access", "refresh_token": "fake-refresh",
                    "expires_in": 86400, "token_type": "Bearer", "user_id": 424242},
    "mcp_enabled": True, "mcp_port": 5577, "mcp_token": "verifier",
    "proxy": {"enabled": True, "proxy_type": "http", "host": "127.0.0.1", "port": 18080},
    "exclusive_mode": True, "exclusive_device": "sone_capture", "bit_perfect": True,
}


def run(args):
    out = {"latency_ms": args.latency_ms, "slow": args.slow, "builds": {}}
    for b in args.bin:
        label = args.label[len(out["builds"])] if args.label and len(args.label) > len(out["builds"]) else b
        out["builds"][label] = {}
        for mode in args.modes.split(","):
            rundir = tempfile.mkdtemp(prefix=f"lat-{mode}-", dir=args.rundir)
            trace = os.path.join(rundir, "drive.json")
            env = {**os.environ, "SPIKE_SEED": json.dumps(BASE_PROFILE), "SPIKE_OUTPUT": args.display,
                   "SPIKE_LATENCY_MS": str(args.latency_ms), "SPIKE_LATENCY_SLOW": args.slow or "",
                   "SPIKE_HOME_TABS": "1", "MCP_URL": "http://127.0.0.1:5577/verifier/mcp"}
            cmd = [os.path.join(ROOT, "harness", "sandbox.sh"), os.path.join(rundir, "run"), b, "--",
                   "python3", "-I", os.path.abspath(__file__), "drive", mode, trace]
            p = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=300)
            if os.path.exists(trace):
                out["builds"][label][mode] = json.load(open(trace))
            else:
                out["builds"][label][mode] = {"errors": [(p.stdout + p.stderr)[-1500:]]}
            print(f"{label} {mode}: {json.dumps(out['builds'][label][mode].get('times'))}", flush=True)
    json.dump(out, open(args.out, "w"), indent=1)
    print(table(out))


def table(out) -> str:
    builds = list(out["builds"])
    rows = {}
    reqs = {}
    for b in builds:
        for mode, r in out["builds"][b].items():
            for tag, ts in (r.get("times") or {}).items():
                for k, v in ts.items():
                    rows.setdefault(f"{tag}: {k}", {})[b] = v
            for tag, rq in (r.get("requests") or {}).items():
                reqs.setdefault(tag, {})[b] = rq
    lines = ["| step | " + " | ".join(builds) + " |", "|---|" + "---|" * len(builds)]
    for k, per in rows.items():
        lines.append(f"| {k} | " + " | ".join(str(per.get(b, "")) for b in builds) + " |")
    lines += ["", "| requests | " + " | ".join(builds) + " |", "|---|" + "---|" * len(builds)]
    for tag, per in reqs.items():
        lines.append(f"| {tag} | " + " | ".join(f"{per[b]['total']} (done {per[b]['last_done']} s)" if b in per else "" for b in builds) + " |")
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--bin", action="append", required=True)
    r.add_argument("--label", action="append")
    r.add_argument("--latency-ms", type=int, default=300)
    r.add_argument("--slow", default="")
    r.add_argument("--modes", default="seq,play,playlist,album,search")
    r.add_argument("--display", default="1400x2000")
    r.add_argument("--rundir", default=tempfile.gettempdir())
    r.add_argument("--out", default="latency.json")
    d = sub.add_parser("drive")
    d.add_argument("mode")
    d.add_argument("out")
    t = sub.add_parser("table")
    t.add_argument("json")
    a = ap.parse_args()
    if a.cmd == "run":
        run(a)
    elif a.cmd == "drive":
        drive(a.mode, a.out)
    else:
        print(table(json.load(open(a.json))))


if __name__ == "__main__":
    main()
