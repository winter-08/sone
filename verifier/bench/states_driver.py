#!/usr/bin/env python3
"""One states run, inside a sandbox (see bench/states.py). Writes a JSON result to argv[1].

The app runs with SONE_PERF=1, so it prints one "[perf] ..." line every 5 s (rune-app/src/perf.rs).
The driver tails sone.log and, at every perf line, snapshots /proc (the app's and the
compositor's CPU ticks, per-thread ticks, smaps_rollup, machine-wide /proc/stat and loadavg)
and, on the GPU, nvidia-smi's per-process memory. A state is entered right after a perf line;
the interval that contains the transition (and any that started less than SETTLE s after it)
is discarded, and the next KEEP intervals are kept. Nothing touches the app (no AT-SPI, no
MCP) while a kept interval runs, except the wheel in the scrolling state.

Each state records what it expected the perf line's flags to say (open/closed,
playing/paused, focused/unfocused) and what the compositor said (window visible or not);
"took" is false, with the reason, when they disagree.
"""
import json
import os
import re
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
from channels import atspi, mcp  # noqa: E402

PID = int(os.environ["SONE_PID"])
RUN = os.environ["RUN"]
LOG = os.path.join(RUN, "logs", "sone.log")
MCP_URL = "http://127.0.0.1:5577/verifier/mcp"
TICK = os.sysconf("SC_CLK_TCK")
PAGE = os.sysconf("SC_PAGE_SIZE")
KEEP = int(os.environ.get("SPIKE_BENCH_KEEP", "2"))
SETTLE = float(os.environ.get("SPIKE_BENCH_SETTLE", "1.0"))
BIG = int(os.environ.get("SPIKE_BENCH_BIG", "0") or 0)
BIG_BASE = 300000
GPU = bool(os.environ.get("SPIKE_GPU"))
ALL_STATES = ["idle_home", "closed_playing", "open_playing", "open_paused", "open_unfocused",
              "covered_playing", "hidden_workspace", "hidden_scratchpad",
              "biglist_playing", "biglist_scroll", "biglist_paused"]
# Not in the default list (give them with --states):
#   biglist_down, biglist_up, biglist_down2: the big list, paused, wheeled to the very bottom
#     (every page loaded), back to the top, and to the bottom again; memory after each.
#   region_home, region_playlist, region_drawer: playing on Home, on the fixture playlist, and
#     there with the now-playing drawer open; with RUNE_REGION_MISS=1 in the build's env the
#     app prints "rune-miss KEY WHY" for every region that builds again, counted per state.
MISS = re.compile(r"^rune-miss (.+?) (version|state \S+ v\d+|layout .+|caret .+|selection)$")
STATES = [s for s in os.environ.get("SPIKE_BENCH_STATES", ",".join(ALL_STATES)).split(",") if s]


def boottime() -> float:
    return time.clock_gettime(time.CLOCK_BOOTTIME)


# ---------------------------------------------------------------- perf lines

NUM = re.compile(r"([A-Za-z_][\w.-]*)[ =](-?\d+(?:\.\d+)?)\s*(/s|%|MB|ms|s\b)?")


def parse_perf(line: str) -> dict:
    """'[perf] open playing focused fx=on | frames 4.2/s (drawn 21, unchanged 0, skipped 0) ...'
    -> {"flags": [...], "tags": {"fx": "on"}, "values": {"frames": 4.2, "drawn": 21, ...}}.
    Tolerant: any extra word in the head is a flag, any extra `k=v` a tag, any `name number[unit]`
    after the first '|' a value (parenthesised counts too), so fields added later just appear."""
    body = line.split("[perf]", 1)[1]
    head, _, rest = body.partition("|")
    flags, tags = [], {}
    for tok in head.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            tags[k] = v
        else:
            flags.append(tok)
    values, units = {}, {}
    for name, num, unit in NUM.findall(rest):
        if name in values:  # keep the first occurrence of a repeated name
            continue
        values[name] = float(num)
        units[name] = (unit or "").strip()
    return {"flags": flags, "tags": tags, "values": values, "units": units}


def proc_tree(pid: int) -> list[int]:
    out, todo = [], [pid]
    while todo:
        p = todo.pop()
        out.append(p)
        try:
            for t in os.listdir(f"/proc/{p}/task"):
                with open(f"/proc/{p}/task/{t}/children") as f:
                    todo += [int(c) for c in f.read().split()]
        except OSError:
            pass
    return out


def stat_fields(path: str) -> list[str]:
    with open(path) as f:
        s = f.read()
    return [s[s.index("(") + 1:s.rindex(")")]] + s[s.rindex(")") + 2:].split()


def ticks(pid: int) -> int:
    try:
        f = stat_fields(f"/proc/{pid}/stat")
        return int(f[12]) + int(f[13])  # utime + stime (all threads)
    except (OSError, ValueError, IndexError):
        return 0


def thread_ticks(pid: int) -> dict:
    """comm with digits folded ("llvmpipe-3" -> "llvmpipe-#") -> ticks summed."""
    out = {}
    try:
        tids = os.listdir(f"/proc/{pid}/task")
    except OSError:
        return out
    for t in tids:
        try:
            f = stat_fields(f"/proc/{pid}/task/{t}/stat")
        except OSError:
            continue
        name = re.sub(r"\d+", "#", f[0])
        out[name] = out.get(name, 0) + int(f[12]) + int(f[13])
    return out


def smaps(pid: int) -> dict:
    o = {}
    try:
        with open(f"/proc/{pid}/smaps_rollup") as f:
            for line in f:
                k, _, v = line.partition(":")
                if k in ("Rss", "Pss", "Anonymous"):
                    o[k.lower() + "_mb"] = round(int(v.split()[0]) / 1024, 1)
    except OSError:
        pass
    return o


def machine() -> dict:
    with open("/proc/stat") as f:
        cpu = [int(x) for x in f.readline().split()[1:]]
    idle = cpu[3] + cpu[4]
    with open("/proc/loadavg") as f:
        la = f.read().split()
    return {"busy": sum(cpu) - idle, "total": sum(cpu), "load1": float(la[0]), "load5": float(la[1]),
            "running": int(la[3].split("/")[0])}


def gpu_mem(pids: dict) -> dict:
    """{label: MiB} from `nvidia-smi -q -d PIDS` (graphics and compute), for the given pids."""
    try:
        out = subprocess.run(["nvidia-smi", "-q", "-d", "PIDS"], capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.TimeoutExpired):
        return {"error": "nvidia-smi unavailable"}
    found, pid = {}, None
    for line in out.splitlines():
        k, _, v = line.partition(":")
        k, v = k.strip(), v.strip()
        if k == "Process ID":
            pid = int(v)
        elif k == "Used GPU Memory" and pid is not None:
            for label, p in pids.items():
                if p == pid:
                    found[label] = float(v.split()[0]) if v.split()[0].replace(".", "").isdigit() else v
            pid = None
    return {label: found.get(label) for label in pids}


def app_root() -> int:
    for p in proc_tree(PID):
        try:
            if os.path.basename(os.readlink(f"/proc/{p}/exe")) in ("sone-rune", ".sone-wrapped", "sone"):
                return p
        except OSError:
            pass
    return PID


def sway_pid() -> int | None:
    for p in proc_tree(os.getppid()):
        try:
            if os.path.basename(os.readlink(f"/proc/{p}/exe")) == "sway":
                return p
        except OSError:
            pass
    return None


RUN_OBJ = None
RSS_LIMIT_MB = int(os.environ.get("SPIKE_BENCH_RSS_LIMIT_MB", "3000"))


class Guard:
    """Kills the app if its RSS passes RSS_LIMIT_MB. (The baseline, given a 3000-track Loved
    Tracks page, grew by ~1.3 GB/s until killed by hand at 32 GB; this machine's desktop shares
    the RAM.) Checked every 0.1 s from /proc/PID/statm."""

    def __init__(self, pid: int):
        self.pid, self.tripped, self.peak = pid, None, 0.0
        threading.Thread(target=self._run, daemon=True).start()

    def _run(self):
        while True:
            try:
                rss_mb = int(open(f"/proc/{self.pid}/statm").read().split()[1]) * PAGE / 2**20
            except (OSError, ValueError, IndexError):
                return
            self.peak = max(self.peak, rss_mb)
            if rss_mb > RSS_LIMIT_MB:
                self.tripped = {"rss_mb": round(rss_mb), "t": boottime(), "phase": getattr(RUN_OBJ, "phase", "startup")}
                try:
                    os.kill(self.pid, 9)
                except OSError:
                    pass
                print(f"guard: app RSS {rss_mb:.0f} MB > {RSS_LIMIT_MB} MB, killed it", flush=True)
                return
            time.sleep(0.1)


class Tail:
    """Follows sone.log; at every perf line, snapshots /proc."""

    def __init__(self, root: int, sway: int | None):
        self.root, self.sway = root, sway
        self.lines = []  # dicts: t (boottime), parsed, snap
        self.vis = []  # (boottime, line): the app's own "[rune] main window on/off screen" log lines
        self.miss = []  # (boottime, key, why): RUNE_REGION_MISS lines
        self.stale = []  # (boottime, line): debug builds' "rune: region ... was reused, but ..." warnings
        self.cv = threading.Condition()
        self.log_bytes = 0
        threading.Thread(target=self._run, daemon=True).start()

    def snap(self) -> dict:
        s = {"t": boottime(), "app_ticks": ticks(self.root), "threads": thread_ticks(self.root),
             "mem": smaps(self.root), "machine": machine(), "log_bytes": self.log_bytes}
        if self.sway:
            s["sway_ticks"] = ticks(self.sway)
        return s

    def _run(self):
        while not os.path.exists(LOG):
            time.sleep(0.05)
        with open(LOG, "rb") as f:
            buf = b""
            while True:
                chunk = f.read()
                if not chunk:
                    time.sleep(0.03)
                    continue
                self.log_bytes += len(chunk)
                buf += chunk
                *done, buf = buf.split(b"\n")
                for raw in done:
                    line = raw.decode(errors="replace")
                    # the stderr copy only (the log::info copy carries a level prefix)
                    if "main window" in line and ("on screen" in line or "off screen" in line):
                        self.vis.append((boottime(), line[-120:]))
                    if line.startswith("rune-miss "):
                        mm = MISS.match(line)
                        self.miss.append((boottime(), *(mm.groups() if mm else (line[10:], "?"))))
                    elif "rune: region" in line:
                        self.stale.append((boottime(), line[:400]))
                    if line.startswith("[perf]"):
                        rec = {"t": boottime(), "line": line, "p": parse_perf(line), "snap": self.snap()}
                        with self.cv:
                            self.lines.append(rec)
                            self.cv.notify_all()

    def next_line(self, after: int, timeout: float = 20.0) -> dict:
        """Wait for perf line number `after` (0-based) to exist; return it."""
        deadline = time.monotonic() + timeout
        with self.cv:
            while len(self.lines) <= after:
                left = deadline - time.monotonic()
                if left <= 0:
                    raise TimeoutError("no perf line (is SONE_PERF=1 set, and is the app alive?)")
                self.cv.wait(left)
            return self.lines[after]

    def count(self) -> int:
        with self.cv:
            return len(self.lines)


def fav_requests() -> list:
    """The Loved Tracks requests the app made (offset, limit) and rows the fake served, from
    the fake TIDAL's flow log, so row counts can be compared between builds."""
    out = []
    try:
        for line in open(os.path.join(RUN, "logs", "flows.jsonl")):
            f = json.loads(line)
            if "/favorites/tracks" in f["path"]:
                from urllib.parse import parse_qs, urlsplit
                q = parse_qs(urlsplit(f["path"]).query)
                off, lim = int(q.get("offset", ["0"])[0]), int(q.get("limit", ["0"])[0])
                served = max(0, BIG - off) if os.environ.get("SPIKE_BENCH_BIG_LIMIT") != "honor" else max(0, min(lim, BIG - off))
                out.append({"offset": off, "limit": lim, "rows_served": served, "status": f["status"]})
    except OSError:
        pass
    return out


# ---------------------------------------------------------------- compositor

def swaymsg(*a) -> str:
    p = subprocess.run(["swaymsg", *a], capture_output=True, text=True, timeout=10)
    return p.stdout


def sway_tree() -> dict:
    return json.loads(swaymsg("-t", "get_tree"))


def find_views(node: dict, out=None, ws=None, output=None, scratch=False) -> list[dict]:
    out = [] if out is None else out
    t = node.get("type")
    if t == "output":
        output = node.get("name")
        scratch = output == "__i3"
    if t == "workspace":
        ws = node.get("name")
    if node.get("pid") and t in ("con", "floating_con"):
        out.append({"pid": node["pid"], "app_id": node.get("app_id"), "visible": node.get("visible"),
                    "focused": node.get("focused"), "fullscreen": node.get("fullscreen_mode"),
                    "workspace": ws, "output": output, "scratchpad": scratch,
                    "inhibit_idle": node.get("inhibit_idle"),
                    "rect": node.get("rect")})
    for c in node.get("nodes", []) + node.get("floating_nodes", []):
        find_views(c, out, ws, output, scratch)
    return out


def app_view(root: int) -> dict | None:
    for v in find_views(sway_tree()):
        if v["pid"] in (root, PID):
            return v
    return None


class Cover:
    """bench/cover.py: a grey toplevel; sway rules decide where it goes."""

    def __init__(self):
        self.p = subprocess.Popen(["python3", "-I", os.path.join(HERE, "cover.py")], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True)
        if self.p.stdout.readline().strip() != "mapped":
            raise RuntimeError("cover window failed to map")
        for _ in range(50):
            if any(v["app_id"] == "bench-cover" for v in find_views(sway_tree())):
                return
            time.sleep(0.05)
        raise RuntimeError("cover window never appeared in sway's tree")

    def close(self):
        self.p.stdin.close()
        self.p.wait(5)


# ---------------------------------------------------------------- app helpers

def wait_control(name: str, timeout: float = 30.0, poll: float = 0.05) -> float:
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        try:
            if name in atspi.controls():
                return time.monotonic() - t0
        except Exception:  # noqa: BLE001 - the tree changes under us
            pass
        time.sleep(poll)
    raise TimeoutError(name)


def find_first(pred, max_nodes: int = 4000):
    """Breadth-first over the app's AT-SPI tree, stopping at the first node pred(role, name)
    accepts; at most max_nodes nodes (a big list is thousands of nodes deep in the tree)."""
    root = atspi.app()
    if root is None:
        return None
    queue, seen = [root], 0
    while queue and seen < max_nodes:
        n = queue.pop(0)
        seen += 1
        try:
            role, name = n.getRoleName(), (n.name or "").strip()
            if pred(role, name):
                return n
            queue += [n.getChildAtIndex(i) for i in range(min(n.childCount, 200))]
        except Exception:  # noqa: BLE001
            continue
    return None


class Wheel:
    """bench/wheel.py, fed by a thread: down `span` notches, up `span`, one every `every` s."""

    def __init__(self, span: int = 40, every: float = 0.05):
        swaymsg("seat", "-", "cursor", "set", "800", "620")
        self.p = subprocess.Popen(["python3", "-I", os.path.join(HERE, "wheel.py")], stdin=subprocess.PIPE,
                                  text=True, bufsize=1)
        time.sleep(0.5)
        self.stop = threading.Event()
        self.notches = 0
        self.span, self.every = span, every
        threading.Thread(target=self._run, daemon=True).start()

    def _run(self):
        while not self.stop.is_set():
            for d in (1,) * self.span + (-1,) * self.span:
                if self.stop.is_set():
                    break
                try:
                    self.p.stdin.write(f"{d}\n")
                except (BrokenPipeError, ValueError):
                    return
                self.notches += 1
                time.sleep(self.every)

    def close(self):
        self.stop.set()
        time.sleep(0.1)
        self.p.stdin.close()
        self.p.wait(5)


# ---------------------------------------------------------------- the run

class Run:
    def __init__(self):
        self.out = {"keep": KEEP, "settle_s": SETTLE, "big": BIG, "gpu": GPU, "states": {}, "errors": []}
        self.t_launch = None
        self.root = PID
        self.sway = None
        self.tail = None
        self.m = None
        self.cover = None
        self.wheel = None
        self.last_sweep = {}

    # -- startup: launch -> first map (sway), MCP up, Home button exposed, Home content
    def startup(self):
        f = stat_fields(f"/proc/{PID}/stat")
        self.t_launch = int(f[20]) / TICK  # field 22, starttime  # boottime seconds at exec
        first_map = {}
        tree_before = None
        try:
            sub = subprocess.Popen(["swaymsg", "-t", "subscribe", "-m", '["window"]'], stdout=subprocess.PIPE, text=True)

            def watch():
                for line in sub.stdout:
                    try:
                        ev = json.loads(line)
                    except ValueError:
                        continue
                    c = ev.get("container", {})
                    if ev.get("change") == "new" and c.get("pid") in proc_tree(PID) and "t" not in first_map:
                        first_map["t"] = boottime()
            threading.Thread(target=watch, daemon=True).start()
            time.sleep(0.15)  # the subscription is live
            tree_before = [v for v in find_views(sway_tree()) if v["pid"] in proc_tree(PID)]
        except Exception as e:  # noqa: BLE001
            self.out["errors"].append(f"sway subscribe: {e}")
        su = {}
        while True:
            try:
                self.m = mcp.Mcp(MCP_URL).connect()
                break
            except Exception:  # noqa: BLE001
                if boottime() - self.t_launch > 60:
                    raise
                time.sleep(0.02)
        su["backend_s"] = round(boottime() - self.t_launch, 3)
        while atspi.app() is None:
            if boottime() - self.t_launch > 60:
                raise TimeoutError("the app never appeared on the accessibility bus")
            time.sleep(0.02)
        wait_control("button: Home", 60)
        su["ui_exposed_s"] = round(boottime() - self.t_launch, 3)
        wait_control("heading: Fixture Albums", 60)
        su["home_content_s"] = round(boottime() - self.t_launch, 3)
        if "t" in first_map:
            su["first_frame_s"] = round(first_map["t"] - self.t_launch, 3)
        else:
            su["first_frame_s"] = None
            su["first_frame_note"] = ("window was already mapped when the driver subscribed" if tree_before
                                      else "no window 'new' event seen")
        su["first_frame_method"] = "sway IPC window::new (the toplevel maps on its first buffer commit)"
        self.out["startup"] = su
        self.root = app_root()
        self.sway = sway_pid()
        self.tail = Tail(self.root, self.sway)
        # The headless seat has no keyboard (WLR_LIBINPUT_NO_DEVICES=1), so no client ever gets
        # keyboard focus or loses it and the perf line's focus flag never changes. A real desktop
        # has one: create the virtual keyboard now (it presses nothing until asked).
        self.kbd = subprocess.Popen(["python3", "-I", os.path.join(HERE, "..", "channels", "keyboard.py")],
                                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        if self.kbd.stdout.readline().strip() != "ready":
            self.out["errors"].append("virtual keyboard did not start")
        time.sleep(0.3)
        self.out["seat_after_keyboard"] = json.loads(swaymsg("-t", "get_seats"))[0].get("capabilities")
        try:
            self.m.call("set_repeat", {"mode": "all"})
        except Exception as e:  # noqa: BLE001
            self.out["errors"].append(f"set_repeat: {e}")

    # -- one state: enter right after a perf line, skip the transition, keep KEEP intervals
    def state(self, name: str, enter, expect: dict, check=None, leave=None):
        n0 = self.tail.count()
        self.tail.next_line(n0)  # a fresh line has just arrived
        rec = {"expect": expect, "notes": []}
        t_change = boottime()
        try:
            extra = enter() or {}
        except Exception as e:  # noqa: BLE001
            rec["took"] = False
            rec["notes"].append(f"entering failed: {e!r}")
            self.out["states"][name] = rec
            return
        t_ready = boottime()
        self.phase = name
        rec.update(extra)
        rec["enter_s"] = round(t_ready - t_change, 3)
        # first interval that starts SETTLE s after the change finished
        i = self.tail.count()
        while True:
            ln = self.tail.next_line(i, timeout=30)
            if ln["t"] - 5.0 >= t_ready + SETTLE:
                break
            i += 1
        base_idx = i - 1  # its start is the previous line's snapshot
        kept = [self.tail.next_line(i + k, timeout=30) for k in range(KEEP)]
        base = self.tail.lines[base_idx]
        if check:
            try:
                rec.update(check() or {})
            except Exception as e:  # noqa: BLE001
                rec["notes"].append(f"check failed: {e!r}")
        if GPU:
            rec["gpu_mem_mib"] = gpu_mem({"app": self.root, "sway": self.sway})
        if leave:
            try:
                leave()
            except Exception as e:  # noqa: BLE001
                rec["notes"].append(f"leaving failed: {e!r}")
        rec.update(self.summarise(base, kept))
        rec["visibility_log"] = [ln for t, ln in self.tail.vis if t_change - 0.5 <= t <= boottime()]
        # did it take?
        bad = []
        for k in kept:
            fl = k["p"]["flags"]
            for want in expect.get("flags", []):
                if want not in fl:
                    bad.append(f"perf line says {' '.join(fl)}, expected {want}")
        if expect.get("focused") is not None and rec.get("sway_view", {}).get("focused") != expect["focused"]:
            bad.append(f"sway says focused={rec.get('sway_view', {}).get('focused')}, expected {expect['focused']}")
        if expect.get("visible") is not None and rec.get("sway_view", {}).get("visible") != expect["visible"]:
            bad.append(f"sway says visible={rec.get('sway_view', {}).get('visible')}, expected {expect['visible']}")
        rec["notes"] += sorted(set(bad))
        rec["took"] = not bad and not any(n.startswith(("entering failed", "check failed")) for n in rec["notes"])
        self.out["states"][name] = rec

    def summarise(self, base: dict, kept: list) -> dict:
        a, b = base["snap"], kept[-1]["snap"]
        secs = b["t"] - a["t"]
        o = {"window_s": round(secs, 2), "perf_lines": [k["line"] for k in kept]}
        # perf values: means over the kept intervals; parenthesised counts also as rates
        vals = {}
        for k in kept:
            for name, v in k["p"]["values"].items():
                vals.setdefault(name, []).append(v)
        perf = {n: round(sum(v) / len(v), 3) for n, v in vals.items()}
        units = kept[-1]["p"]["units"]
        for n, u in units.items():
            if u == "" and n in vals:  # a bare count over the 5 s interval
                perf[n + "_per_s"] = round(sum(vals[n]) / (5.0 * len(kept)), 3)
        o["perf"] = perf
        o["flags"] = kept[-1]["p"]["flags"]
        o["tags"] = kept[-1]["p"]["tags"]
        ext = {"cpu_pct": round((b["app_ticks"] - a["app_ticks"]) / TICK / secs * 100, 1)}
        if "sway_ticks" in a and "sway_ticks" in b:
            ext["sway_cpu_pct"] = round((b["sway_ticks"] - a["sway_ticks"]) / TICK / secs * 100, 1)
        th = {n: round((b["threads"].get(n, 0) - a["threads"].get(n, 0)) / TICK / secs * 100, 1) for n in b["threads"]}
        ext["threads_pct"] = dict(sorted(((n, v) for n, v in th.items() if v >= 0.5), key=lambda x: -x[1])[:8])
        ext.update(b["mem"])
        ma, mb = a["machine"], b["machine"]
        ext["machine_busy_pct"] = round((mb["busy"] - ma["busy"]) / max(1, mb["total"] - ma["total"]) * 100, 1)
        ext["load1_start"], ext["load1_end"] = ma["load1"], mb["load1"]
        ext["log_bytes_per_s"] = round((b["log_bytes"] - a["log_bytes"]) / secs)
        o["ext"] = ext
        # region rebuilds (RUNE_REGION_MISS) inside the window, per second, by region key
        misses = [(k, w) for t, k, w in self.tail.miss if a["t"] <= t <= b["t"]]
        if self.tail.miss:
            by = {}
            for k, w in misses:
                e = by.setdefault(k, {"n": 0, "why": {}})
                e["n"] += 1
                w0 = re.sub(r" v\d+$", "", w)
                e["why"][w0] = e["why"].get(w0, 0) + 1
            o["region_miss_per_s"] = {k: {"per_s": round(e["n"] / secs, 2), "why": e["why"]}
                                      for k, e in sorted(by.items(), key=lambda kv: -kv[1]["n"])}
            o["region_miss_total_per_s"] = round(len(misses) / secs, 2)
        return o

    def view_check(self):
        return {"sway_view": app_view(self.root)}

    # -- the states
    def run(self):
        self.guard = Guard(PID)
        self.startup()
        m = self.m
        play_ids = [BIG_BASE + 1, BIG_BASE + 2] if BIG else [1001, 1002]
        self.out["play_ids"] = play_ids
        S = {}

        S["idle_home"] = (lambda: time.sleep(0.1), {"flags": ["closed", "paused", "focused"], "visible": True}, self.view_check, None)

        def play():
            m.call("play_tracks", {"track_ids": play_ids, "action": "play_now"})
            time.sleep(1.0)
        S["closed_playing"] = (play, {"flags": ["closed", "playing", "focused"], "visible": True}, self.view_check, None)

        def ensure_playing():
            m.call("resume")  # a no-op when already playing

        def open_player():
            if "button: Open full-screen player" not in atspi.controls():
                play()
            atspi.press("Open full-screen player")
        S["open_playing"] = (open_player, {"flags": ["open", "playing", "focused"], "visible": True}, self.view_check, None)
        S["open_paused"] = (lambda: m.call("pause"), {"flags": ["open", "paused", "focused"], "visible": True}, self.view_check, None)

        def unfocus():
            if "button: Open full-screen player" in atspi.controls():
                atspi.press("Open full-screen player")
            m.call("resume")
            # a second (empty) output: focusing it takes keyboard focus from the app while the
            # app's workspace stays on screen, uncovered
            outs = json.loads(swaymsg("-t", "get_outputs"))
            if not any(o["name"] == "HEADLESS-2" for o in outs):
                swaymsg("create_output")
                time.sleep(0.3)
            swaymsg("focus", "output", "HEADLESS-2")
            time.sleep(0.3)

        def refocus():
            swaymsg("focus", "output", "HEADLESS-1")
            v = app_view(self.root)
            if v:
                swaymsg(f"[pid={v['pid']}]", "focus")
        # The focus is judged by sway's tree, not the perf line's flag: from the parity work on the
        # app's perf focus handler sits on an element that is no longer the view's root (the shell
        # frame wraps it), so Rune never calls it and the flag stays "focused" (rune-app main.rs).
        S["open_unfocused"] = (unfocus, {"flags": ["open", "playing"], "focused": False, "visible": True}, self.view_check, refocus)

        def unfocus_closed():  # diagnostic (not in the default list): the same, player closed
            close_player()
            m.call("resume")
            outs = json.loads(swaymsg("-t", "get_outputs"))
            if not any(o["name"] == "HEADLESS-2" for o in outs):
                swaymsg("create_output")
                time.sleep(0.3)
            swaymsg("focus", "output", "HEADLESS-2")
            time.sleep(0.3)
        S["closed_unfocused"] = (unfocus_closed, {"flags": ["closed", "playing"], "focused": False, "visible": True}, self.view_check, refocus)

        def close_player():
            time.sleep(0.3)
            if "button: Open full-screen player" not in atspi.controls():
                atspi.key("Escape")
                time.sleep(0.8)

        def cover():
            close_player()
            swaymsg("for_window [app_id=bench-cover] fullscreen enable")
            self.cover = Cover()
            time.sleep(0.3)

        def uncover():
            if self.cover:
                self.cover.close()
                self.cover = None
            refocus()

        def cover_check():
            d = self.view_check()
            views = find_views(sway_tree())
            cv = d["cover_view"] = next((v for v in views if v["app_id"] == "bench-cover"), None)
            if not (cv and cv["fullscreen"] and cv["focused"] and cv["workspace"] == (d["sway_view"] or {}).get("workspace")):
                raise RuntimeError(f"cover is not fullscreen and focused on the app's workspace: {cv}")
            return d
        # Sway marks the covered app visible=false. The focus flag is not checked: the baseline's
        # event loop stops (polls 0/s) when it gets no frame callbacks, so its focus flag can
        # stay stale at "focused"; sway's tree says the cover holds the focus.
        S["covered_playing"] = (cover, {"flags": ["closed", "playing"], "visible": False}, cover_check, uncover)

        def hide_ws():
            close_player()
            swaymsg("focus", "output", "HEADLESS-1")
            swaymsg("workspace", "9")
            time.sleep(0.3)
        S["hidden_workspace"] = (hide_ws, {"flags": ["closed", "playing"], "visible": False}, self.view_check,
                                 lambda: (swaymsg("workspace", "1"), refocus()))

        def hide_scratch():
            close_player()
            swaymsg(f"[pid={self.root}]", "move", "scratchpad")
            time.sleep(0.3)

        def unscratch():
            swaymsg(f"[pid={self.root}]", "scratchpad", "show")
            swaymsg(f"[pid={self.root}]", "floating", "disable")
            refocus()
        S["hidden_scratchpad"] = (hide_scratch, {"flags": ["closed", "playing"], "visible": False}, self.view_check, unscratch)

        def to_biglist():
            close_player()
            play()  # (re)start the two long tracks, so the state is "playing" even when run alone
            t0 = time.monotonic()
            atspi.press("Loved Tracks Collection")
            hit = None
            while time.monotonic() - t0 < 60:
                hit = find_first(lambda r, n: r == "heading" and n == "Loved Tracks")
                if hit is not None:
                    break
                time.sleep(0.02)
            d = {"nav_heading_s": round(time.monotonic() - t0, 3) if hit is not None else None}
            # the first row's buttons (rows themselves are presentation; their hearts are not
            # named per track, so look for the count text the header shows)
            t1 = time.monotonic()
            cnt = find_first(lambda r, n: n.endswith(" tracks") and n.split()[0].replace(",", "").isdigit())
            d["rows_total_text"] = cnt.name if cnt is not None else None
            d["count_lookup_s"] = round(time.monotonic() - t1, 3)
            return d

        def big_check():
            d = self.view_check()
            return d
        S["biglist_playing"] = (to_biglist, {"flags": ["closed", "playing", "focused"], "visible": True}, big_check, None)

        def shot(tag: str) -> str | None:
            """A grim capture of part of the track list (below the header and first rows)."""
            import hashlib
            f = os.path.join(RUN, f"scroll-{tag}.png")
            subprocess.run(["grim", "-g", "420,520 900x330", f], capture_output=True, timeout=10)
            return hashlib.sha1(open(f, "rb").read()).hexdigest()[:12] if os.path.exists(f) else None

        def scroll():
            self.scroll_before = shot("before")
            self.wheel = Wheel()

        def scroll_check():
            d = self.view_check()
            h = [shot(f"during{i}") for i in range(3) if not time.sleep(0.4)]
            d["scroll_shots"] = {"before": self.scroll_before, "during": h}
            # the list moved iff captures during the scroll differ from before it and each other
            d["scroll_moved"] = bool(self.scroll_before) and len({self.scroll_before, *h}) >= 3
            if not d["scroll_moved"]:
                raise RuntimeError(f"the list did not visibly move under the wheel: {d['scroll_shots']}")
            return d

        def unscroll():
            if self.wheel:
                d = self.wheel.notches
                self.wheel.close()
                self.wheel = None
                self.out.setdefault("wheel_notches", d)
        S["biglist_scroll"] = (scroll, {"flags": ["closed", "playing", "focused"], "visible": True}, scroll_check, unscroll)
        S["biglist_paused"] = (lambda: m.call("pause"), {"flags": ["closed", "paused", "focused"], "visible": True}, self.view_check, None)

        # ---- stress: wheel the big list to the bottom (every page in), the top, the bottom again
        def rows_requested() -> int:
            fr = fav_requests()
            return max((x["offset"] + x["rows_served"] for x in fr), default=0)

        def sweep(direction: int, need_all: bool, max_s: float = 300.0) -> dict:
            if find_first(lambda r, n: r == "heading" and n == "Loved Tracks") is None:
                to_biglist()
            m.call("pause")
            swaymsg("seat", "-", "cursor", "set", "800", "620")
            w = subprocess.Popen(["python3", "-I", os.path.join(HERE, "wheel.py")], stdin=subprocess.PIPE, text=True, bufsize=1)
            time.sleep(0.5)
            t0, notches, still, last = time.monotonic(), 0, 0, None
            try:
                while time.monotonic() - t0 < max_s:
                    for _ in range(20):
                        w.stdin.write(f"{direction}\n")
                        notches += 1
                        time.sleep(0.005)
                    time.sleep(0.15)
                    if need_all and rows_requested() < BIG:
                        still, last = 0, None
                        continue
                    h = shot("sweep")
                    still = still + 1 if h is not None and h == last else 0
                    last = h
                    if still >= 3:
                        break
            finally:
                w.stdin.close()
                w.wait(5)
            self.last_sweep = {"sweep_s": round(time.monotonic() - t0, 1), "sweep_notches": notches, "sweep_end_reached": still >= 3,
                               "rows_requested": rows_requested(), "fav_requests_n": len(fav_requests())}
            return self.last_sweep

        def sweep_check():
            d = self.view_check()
            d["rss_peak_mb_so_far"] = round(self.guard.peak)
            if not self.last_sweep.get("sweep_end_reached"):
                raise RuntimeError("the sweep never reached the end of the list")
            return d
        exp_paused = {"flags": ["closed", "paused", "focused"], "visible": True}
        S["biglist_down"] = (lambda: sweep(1, True), exp_paused, sweep_check, None)
        S["biglist_up"] = (lambda: sweep(-1, False), exp_paused, sweep_check, None)
        S["biglist_down2"] = (lambda: sweep(1, False), exp_paused, sweep_check, None)

        # ---- regions: what builds again per position tick, on Home, a playlist, with the drawer
        def reg_home():
            close_player()
            atspi.press("Home")
            play()
        S["region_home"] = (reg_home, {"flags": ["closed", "playing", "focused"], "visible": True}, self.view_check, None)

        def reg_playlist():
            atspi.press("Fixture Mixtape Fixture Mixtape You · 4 Tracks")
            time.sleep(2.0)
            ensure_playing()
        S["region_playlist"] = (reg_playlist, {"flags": ["closed", "playing", "focused"], "visible": True}, self.view_check, None)

        def reg_drawer():
            atspi.press("Play queue")
            time.sleep(1.5)
            ensure_playing()
        S["region_drawer"] = (reg_drawer, {"flags": ["closed", "playing", "focused"], "visible": True}, self.view_check, None)

        for name in STATES:
            if name not in S:
                self.out["errors"].append(f"unknown state {name}")
                continue
            if name.startswith("biglist") and not BIG:
                self.out["errors"].append(f"{name}: skipped (SPIKE_BENCH_BIG unset)")
                continue
            enter, expect, check, leave = S[name]
            print(f"state {name}", flush=True)
            self.phase = name + " (entering)"
            self.state(name, enter, expect, check, leave)
            if name.startswith("biglist"):
                self.out["fav_requests"] = fav_requests()
        self.out["perf_all"] = [{"t": round(r["t"] - self.t_launch, 2), "line": r["line"]} for r in self.tail.lines]
        self.out["visibility_log_all"] = [{"t": round(t - self.t_launch, 2), "line": ln} for t, ln in self.tail.vis]
        self.out["region_stale_warnings"] = [{"t": round(t - self.t_launch, 2), "line": ln} for t, ln in self.tail.stale]
        self.out["region_miss_lines"] = len(self.tail.miss)
        self.out["rss_peak_mb"] = round(self.guard.peak)
        self.quit()

    # -- quit: the compositor asks the window to close (xdg_toplevel.close, as the title bar's
    # close button or a desktop's "close window" does); time until the process has exited
    def quit(self):
        q = {"method": "swaymsg kill (xdg_toplevel.close)"}
        t0 = boottime()
        swaymsg(f"[pid={self.root}]", "kill")
        exited = False
        while boottime() - t0 < 20:
            try:
                state = stat_fields(f"/proc/{self.root}/stat")[1]
                if state in ("Z", "X"):
                    exited = True
                    break
            except OSError:
                exited = True
                break
            time.sleep(0.005)
        q["exit_s"] = round(boottime() - t0, 3) if exited else None
        q["exited"] = exited
        self.out["quit"] = q


def main():
    global RUN_OBJ
    r = RUN_OBJ = Run()
    try:
        r.run()
    except Exception as e:  # noqa: BLE001
        import traceback
        r.out["errors"].append("fatal: " + "".join(traceback.format_exception(e))[-1500:])
    g = getattr(r, "guard", None)
    if g and g.tripped:
        r.out["guard"] = {"killed_at_rss_mb": g.tripped["rss_mb"], "limit_mb": RSS_LIMIT_MB, "phase": g.tripped["phase"],
                          "t_since_launch_s": round(g.tripped["t"] - (r.t_launch or 0), 1)}
        r.out["errors"].append(f"app killed by the RSS guard at {g.tripped['rss_mb']} MB")
    if r.tail is not None:
        r.out.setdefault("perf_all", [{"t": round(x["t"] - (r.t_launch or 0), 2), "line": x["line"]} for x in r.tail.lines])
    json.dump(r.out, open(sys.argv[1], "w"), indent=1)


if __name__ == "__main__":
    main()
