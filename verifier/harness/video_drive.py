#!/usr/bin/env python3
"""Drive the Rune build's music video player inside the sandbox (see video_check.py).

    video_drive.py OUT_DIR

Two plays of the fixture video (fixtures/video, 6 s at 25 fps): the first runs to its end
(CPU is sampled while it plays, and the player must close by itself afterwards); the second
minimises to the bar and back, switches quality, pauses and resumes from the player, and closes
with Escape. Writes OUT_DIR/drive.json with what each step saw, and screenshots.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from channels import atspi  # noqa: E402

OUT = sys.argv[1]
BUS = ["busctl", "--user"]
NAME = "org.mpris.MediaPlayer2.io.github.lullabyX.sone"
result = {"steps": [], "errors": []}


def mpris():
    def prop(p, js=False):
        cmd = BUS + (["--json=short"] if js else []) + ["get-property", NAME, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", p]
        return subprocess.run(cmd, capture_output=True, text=True).stdout.strip()
    status = prop("PlaybackStatus").split('"')[1] if '"' in prop("PlaybackStatus") else None
    try:
        md = json.loads(prop("Metadata", True))["data"]
        title = md.get("xesam:title", {}).get("data")
    except Exception:  # noqa: BLE001
        title = None
    return {"status": status, "title": title}


def has(name):
    return any(c.endswith(": " + name) for c in atspi.controls())


def step(label, **seen):
    seen.update(mpris())
    result["steps"].append({"label": label, "t": round(time.time(), 3), **seen})


def shot(name):
    subprocess.run(["grim", os.path.join(OUT, f"{name}.png")])


def wait_status(want, timeout=10.0):
    end = time.time() + timeout
    while time.time() < end:
        if mpris()["status"] == want:
            return True
        time.sleep(0.1)
    return False


def cpu_seconds(pid):
    with open(f"/proc/{pid}/stat") as f:
        parts = f.read().rsplit(")", 1)[1].split()
    return (int(parts[11]) + int(parts[12])) / os.sysconf("SC_CLK_TCK")


def threads(pid):
    out = {}
    for t in os.listdir(f"/proc/{pid}/task"):
        try:
            with open(f"/proc/{pid}/task/{t}/stat") as f:
                head, rest = f.read().rsplit(")", 1)
            parts = rest.split()
            out[t] = (head.split("(", 1)[1], (int(parts[11]) + int(parts[12])) / os.sysconf("SC_CLK_TCK"))
        except OSError:
            pass
    return out


def rss_mb(pid):
    with open(f"/proc/{pid}/status") as f:
        return next(int(l.split()[1]) // 1024 for l in f if l.startswith("VmRSS:"))


def open_videos_tab():
    atspi.focus("Search", role="entry")
    time.sleep(0.5)
    atspi.type_text("fixture video")
    time.sleep(1.0)
    atspi.press("View all results")
    time.sleep(2.0)
    atspi.press("Videos", role="button")
    time.sleep(1.0)


def play():
    t0 = time.time()
    atspi.press("Fixture Video", role="heading")
    ok = wait_status("Playing", 15)
    return ok, round(time.time() - t0, 2)


def close_test():
    """Close the video while a segment download is in flight (the fake server answers .ts late),
    three ways: Escape, starting another video, and an error-free quality switch."""
    atspi.wait_ready(60)
    pid = int(os.environ["SONE_PID"])
    alive = lambda: os.path.exists(f"/proc/{pid}") and open(f"/proc/{pid}/stat").read().split(")")[1].split()[0] != "Z"  # noqa: E731
    open_videos_tab()
    for label in ("escape", "replay", "quality"):
        atspi.press("Fixture Video", role="heading")
        time.sleep(1.2)  # the playlists are in; the first segment is still on its way
        if label == "escape":
            atspi.key("Escape")
        elif label == "replay":
            atspi.press("Minimize video")
            time.sleep(0.3)
            atspi.press("Fixture Video", role="heading")
        else:
            atspi.press("Quality", role="button")
            time.sleep(0.3)
            atspi.press("Low")
        time.sleep(2.0)
        step(f"close mid-download: {label}", alive=alive())
        if not alive():
            return
        atspi.key("Escape")
        time.sleep(1.0)
    time.sleep(3.0)
    step("close mid-download: settled", alive=alive())


try:
    if os.environ.get("SPIKE_VIDEO_DRIVE") == "close":
        close_test()
        raise SystemExit
    atspi.wait_ready(60)
    pid = int(os.environ["SONE_PID"])
    rss_idle = rss_mb(pid)
    open_videos_tab()
    step("videos tab", card=has("Fixture Video"))
    shot("1-videos-tab")

    # Play 1: to the end, sampling CPU on the way.
    ok, secs = play()
    step("play 1", playing=ok, seconds_to_playing=secs, player=has("Minimize video"))
    time.sleep(1.0)
    c0, w0, t0 = cpu_seconds(pid), time.time(), threads(pid)
    time.sleep(3.0)
    wall = time.time() - w0
    cpu = (cpu_seconds(pid) - c0) / wall * 100
    t1 = threads(pid)
    by = {}
    for k, (name, c) in t1.items():
        by[name] = by.get(name, 0) + (c - t0.get(k, (name, c))[1]) / wall * 100
    result["threads"] = sorted(((round(v, 1), n) for n, v in by.items() if v > 0.2), reverse=True)
    shot("2-playing")
    step("cpu while playing", cpu_percent=round(cpu, 1), rss_mb=rss_mb(pid), rss_idle_mb=rss_idle)
    time.sleep(4.0)
    step("after the end", player=has("Minimize video"))
    shot("3-after-end")

    # Play 2: minimise/expand, quality, pause/resume, Escape.
    ok, secs = play()
    step("play 2", playing=ok, seconds_to_playing=secs)
    # Minimise first: the fixture is 6 s long, and an ended video closes its player.
    atspi.press("Minimize video")
    time.sleep(0.6)
    step("minimised", player=has("Minimize video"), expand=has("Expand video"))
    shot("4-minimised")
    atspi.press("Expand video")
    time.sleep(0.6)
    step("expanded", player=has("Minimize video"))
    time.sleep(0.8)
    atspi.press("Quality", role="button")
    time.sleep(0.3)
    atspi.press("Medium")
    time.sleep(1.5)
    step("quality medium")
    atspi.press("Pause", role="button")
    step("paused from the player", paused=wait_status("Paused", 3))
    atspi.press("Play", role="button")
    step("resumed from the player", playing=wait_status("Playing", 5))
    atspi.key("Escape")
    time.sleep(1.0)
    step("escape", player=has("Minimize video"))
    time.sleep(2.0)
    step("after closing", rss_mb=rss_mb(pid))
except SystemExit:
    pass
except Exception as e:  # noqa: BLE001
    result["errors"].append(f"{type(e).__name__}: {e}")
json.dump(result, open(os.path.join(OUT, "drive.json"), "w"), indent=1)
