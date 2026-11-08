#!/usr/bin/env python3
"""One benchmark run, inside a sandbox (see bench.py). Writes a JSON result.

Measures the app's whole process tree (WebKit's web and network processes count for the
reference), from /proc only:
  - startup: launch -> backend up (MCP answers), UI exposed (AT-SPI), Home content shown
  - memory: RSS and PSS of the tree, idle on Home, after browsing, while playing
  - cpu: CPU seconds used by the tree over a 10 s window: idle, while playing (on the search
    results page), and while playing on the playing track's album (its animated "playing" bars)
  - latency: press -> the next page's content is exposed (album, artist, search results);
    the pages come from bench/targets.py (fixtures, or a real catalogue)
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from channels import atspi, mcp  # noqa: E402
import targets  # noqa: E402

PID = int(os.environ["SONE_PID"])
TICK = os.sysconf("SC_CLK_TCK")
MCP_URL = "http://127.0.0.1:5577/verifier/mcp"


def tree(pid: int) -> list[int]:
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


def app_root() -> int:
    """The app's top process: below the strace/bwrap/wrapper shell layers, if any."""
    procs = tree(PID)
    for p in procs:
        try:
            exe = os.readlink(f"/proc/{p}/exe")
        except OSError:
            continue
        name = os.path.basename(exe)
        if name in ("sone-rune", ".sone-wrapped", "sone"):
            return p
    return PID


def mem(root: int) -> dict:
    rss = pss = 0
    procs = tree(root)
    for p in procs:
        try:
            with open(f"/proc/{p}/smaps_rollup") as f:
                for line in f:
                    if line.startswith("Rss:"):
                        rss += int(line.split()[1])
                    elif line.startswith("Pss:"):
                        pss += int(line.split()[1])
        except OSError:
            pass
    return {"rss_mb": round(rss / 1024, 1), "pss_mb": round(pss / 1024, 1), "processes": len(procs)}


def cpu_seconds(root: int) -> float:
    total = 0
    for p in tree(root):
        try:
            with open(f"/proc/{p}/stat") as f:
                fields = f.read().rsplit(")", 1)[1].split()
            total += int(fields[11]) + int(fields[12])  # utime + stime
        except OSError:
            pass
    return total / TICK


def started_ago(pid: int) -> float:
    with open(f"/proc/{pid}/stat") as f:
        start_ticks = int(f.read().rsplit(")", 1)[1].split()[19])
    with open("/proc/uptime") as f:
        up = float(f.read().split()[0])
    return up - start_ticks / TICK


def wait_control(name: str, timeout: float = 30.0, poll: float = 0.05) -> float:
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        try:
            if name in atspi.controls():
                return time.monotonic() - t0
        except Exception:  # noqa: BLE001 - tree changing under us
            pass
        time.sleep(poll)
    raise TimeoutError(name)


def cpu_window(root: int, secs: float = 10.0) -> float:
    a = cpu_seconds(root)
    time.sleep(secs)
    return round((cpu_seconds(root) - a) / secs * 100, 1)  # % of one core


def main():
    out = {}
    launch_age = started_ago(PID)  # how long ago the sandbox launched the app
    t_launch = time.monotonic() - launch_age
    while True:
        try:
            mcp.Mcp(MCP_URL).connect()
            break
        except Exception:  # noqa: BLE001
            time.sleep(0.02)
    out["startup_backend_s"] = round(time.monotonic() - t_launch, 2)
    t0 = time.monotonic()
    while atspi.app() is None:
        if time.monotonic() - t0 > 60:
            raise TimeoutError("the app never appeared on the accessibility bus")
        time.sleep(0.02)
    wait_control("button: Home", 60)
    out["startup_ui_s"] = round(time.monotonic() - t_launch, 2)
    targets.wait_home(set(), 60)
    out["startup_home_s"] = round(time.monotonic() - t_launch, 2)
    root = app_root()

    time.sleep(5)
    out["idle_home"] = mem(root)
    out["idle_cpu_pct"] = cpu_window(root)

    m = mcp.Mcp(MCP_URL).connect()
    t = targets.discover(m)
    lat = {}
    if targets.REAL:
        # from Home: search, then open the album from the results (we stay on its page)
        lat["search_results_s"] = round(targets.search(t["search"], t["album"]), 3)
        lat["open_album_s"] = round(targets.press_album(t), 3)
    else:
        lat["open_album_s"] = round(targets.open_album(t), 3)
        lat["open_artist_s"] = round(targets.open_artist(t), 3)
        lat["search_results_s"] = round(targets.search(t["search"]), 3)
    out["latency"] = lat
    time.sleep(3)
    out["after_browsing"] = mem(root)

    m.call("play_tracks", {"track_ids": t["tracks"], "action": "play_now"})
    time.sleep(2)
    if targets.REAL:
        # on the album page already: its playing row's bars, then Home
        out["playing_album_cpu_pct"] = cpu_window(root)
        atspi.press("Home")
        targets.wait_home(set(), 30)
        time.sleep(2)
        out["playing_cpu_pct"] = cpu_window(root)
        out["playing"] = mem(root)
    else:
        out["playing_cpu_pct"] = cpu_window(root)
        out["playing"] = mem(root)
        # the playing track's album: its row shows the animated "playing" bars
        targets.open_album(t)
        time.sleep(2)
        out["playing_album_cpu_pct"] = cpu_window(root)
    m.call("pause")
    json.dump(out, open(sys.argv[1], "w"), indent=1)


if __name__ == "__main__":
    main()
