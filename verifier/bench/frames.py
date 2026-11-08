#!/usr/bin/env python3
"""Frame pacing probe, inside a sandbox whose app runs with WAYLAND_DEBUG=client (see bench.py).

Counts wl_surface.commit requests on the app's busiest surface (its main window) from the
WAYLAND_DEBUG log, with libwayland's millisecond timestamps, so it measures presented frames
for any toolkit. Writes a JSON result:
  - idle_fps / playing_fps: frames per second on Home, idle and while a track plays
  - playing_album: on the playing track's album, whose row animates its "playing" bars
  - scroll: a scripted wheel scroll on Home (pointer over the content, 16 ticks down then 16
    up, one tick per 50 ms): frames per second while scrolling, the median and worst gap
    between frames, and frames that took longer than two refresh periods ("long frames")
  - modal: open the Shortcuts dialog, close it 0.6 s later: frames and gaps over 1.4 s
"""
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from channels import atspi, mcp  # noqa: E402

LOG = os.path.join(os.environ["RUN"], "logs", "sone.log")
MCP_URL = "http://127.0.0.1:5577/verifier/mcp"
HZ = float(os.environ.get("SPIKE_REFRESH", "60"))
# libwayland prints "[HH:MM:SS.uuuuuu]  -> wl_surface#20.commit()" (local wall clock)
LINE = re.compile(r"^\[(\d+):(\d+):(\d+\.\d+)\]\s+(?:\{[^}]*\}\s+)?-> (wl_surface[#@]\d+)\.commit\(")


def commits() -> dict[str, list[float]]:
    by = {}
    with open(LOG, errors="ignore") as f:
        for line in f:
            m = LINE.match(line)
            if m:
                ms = ((int(m.group(1)) * 60 + int(m.group(2))) * 60 + float(m.group(3))) * 1000
                by.setdefault(m.group(4), []).append(ms)
    return by


def main_surface_times(t0: float, t1: float) -> list[float]:
    """Commit times (ms of the day) of the surface that committed most in [t0, t1]."""
    best = []
    for ts in commits().values():
        sel = [t for t in ts if t0 <= t <= t1]
        if len(sel) > len(best):
            best = sel
    return best


def wl_now() -> float:
    """libwayland's timestamp clock: local time of day, in ms."""
    t = time.time()
    lt = time.localtime(t)
    return ((lt.tm_hour * 60 + lt.tm_min) * 60 + lt.tm_sec + t % 1) * 1000.0


def window(fn, secs: float) -> list[float]:
    t0 = wl_now()
    fn()
    rest = secs - (wl_now() - t0) / 1000
    if rest > 0:
        time.sleep(rest)
    time.sleep(0.3)  # let the log flush
    return main_surface_times(t0, t0 + secs * 1000)


def stats(ts: list[float], secs: float) -> dict:
    gaps = sorted(b - a for a, b in zip(ts, ts[1:]))
    period = 1000.0 / HZ
    return {
        "fps": round(len(ts) / secs, 1),
        "median_gap_ms": round(gaps[len(gaps) // 2], 2) if gaps else None,
        "worst_gap_ms": round(gaps[-1], 1) if gaps else None,
        "long_frames": sum(1 for g in gaps if g > 2 * period),
    }


def swaymsg(*a):
    subprocess.run(["swaymsg", *a], capture_output=True)


def wait_control(name: str, timeout: float = 60.0):
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        try:
            if name in atspi.controls():
                return
        except Exception:  # noqa: BLE001
            pass
        time.sleep(0.05)
    raise TimeoutError(name)


def main():
    out = {"refresh_hz": HZ}
    mcp.wait_ready(MCP_URL)
    atspi.wait_ready()
    wait_control("heading: Fixture Albums")
    time.sleep(4)
    out["idle_fps"] = stats(window(lambda: None, 4), 4)["fps"]

    # pointer over the main content (right of the sidebar, below the header)
    swaymsg("seat", "-", "cursor", "set", "800", "500")
    pointer = subprocess.Popen(["python3", "-I", os.path.join(os.path.dirname(os.path.abspath(__file__)), "wheel.py")],
                               stdin=subprocess.PIPE, text=True, bufsize=1)
    time.sleep(0.5)

    def scroll():
        for dy in (1,) * 16 + (-1,) * 16:
            pointer.stdin.write(f"{dy}\n")
            time.sleep(0.05)
    secs = 32 * 0.05 + 0.4
    out["scroll"] = stats(window(scroll, secs), secs)
    time.sleep(1)

    atspi.press("Account")  # the account menu, then time the Shortcuts dialog opening and closing
    time.sleep(1)

    def modal():
        atspi.press("Shortcuts")
        time.sleep(0.6)
        atspi.key("Escape")
    out["modal"] = stats(window(modal, 1.4), 1.4)
    time.sleep(1)

    m = mcp.Mcp(MCP_URL).connect()
    m.call("play_tracks", {"track_ids": [1001, 1002], "action": "play_now"})
    time.sleep(2)
    out["playing_fps"] = stats(window(lambda: None, 5), 5)["fps"]
    atspi.press("Fixture Album", "heading")  # the playing row's animated bars
    wait_control("button: Favorite album")
    time.sleep(2)
    out["playing_album"] = stats(window(lambda: None, 4), 4)
    m.call("pause")
    json.dump(out, open(sys.argv[1], "w"), indent=1)


if __name__ == "__main__":
    main()
