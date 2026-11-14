#!/usr/bin/env python3
"""Drive the Rune build's animated album covers inside the sandbox (see vcover_check.py).

    vcover_drive.py OUT_DIR

Needs SPIKE_VIDEO_COVER=1 (fake_tidal.py: album 77 has a videoCover) and a display wide enough
for the docked drawer's cover column (1920x1200). Steps: the album page; the toggle off and on
again in Settings; playing the album with the drawer open; the full-screen player; closing it,
the drawer, the page. At each step: screenshots 0.4 s apart, compared by region (does the cover
move?), the `[cover]` log lines since the step began, and how many `cover-bus` threads (one per
pipeline) the app has. Writes OUT_DIR/drive.json.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from channels import atspi  # noqa: E402

OUT = sys.argv[1]
RUN = os.environ["RUN"]
LOG = os.path.join(RUN, "logs", "sone.log")
result = {"steps": [], "errors": []}


def app_pid() -> int:
    """The app process (the wrapper execs it, or starts it as a child)."""
    todo, seen = [int(os.environ["SONE_PID"])], []
    while todo:
        p = todo.pop()
        seen.append(p)
        try:
            if open(f"/proc/{p}/comm").read().strip().startswith("sone-rune"):
                return p
            for t in os.listdir(f"/proc/{p}/task"):
                todo += [int(c) for c in open(f"/proc/{p}/task/{t}/children").read().split()]
        except OSError:
            pass
    return seen[0]


def threads(name: str) -> int:
    pid = app_pid()
    n = 0
    for t in os.listdir(f"/proc/{pid}/task"):
        try:
            if open(f"/proc/{pid}/task/{t}/comm").read().strip() == name:
                n += 1
        except OSError:
            pass
    return n


def log_lines(since: int) -> list[str]:
    with open(LOG, errors="ignore") as f:
        lines = f.read().splitlines()
    return [l[l.index("[cover]"):] for l in lines[since:] if "[cover]" in l]


def log_len() -> int:
    with open(LOG, errors="ignore") as f:
        return len(f.read().splitlines())


def wait_log(needle: str, since: int, timeout: float = 20.0) -> bool:
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        if any(needle in l for l in log_lines(since)):
            return True
        time.sleep(0.1)
    return False


def shot(tag: str) -> tuple[int, int, bytes]:
    path = os.path.join(OUT, f"{tag}.ppm")
    subprocess.run(["grim", "-t", "ppm", path], check=True, timeout=10)
    data = open(path, "rb").read()
    # P6\n<w> <h>\n255\n<rgb>
    parts = data.split(b"\n", 3)
    w, h = (int(x) for x in parts[1].split())
    return w, h, parts[3]


def diff_box(a, b, region=None):
    """Bounding box [x0, y0, x1, y1] of the pixels that differ between two shots (inside
    `region`, [x0, y0, x1, y1]), and how many differ; None when none do."""
    w, h, pa = a
    _, _, pb = b
    x0, y0, x1, y1 = region or (0, 0, w, h)
    box, count = None, 0
    for y in range(y0, y1):
        ra, rb = pa[(y * w + x0) * 3:(y * w + x1) * 3], pb[(y * w + x0) * 3:(y * w + x1) * 3]
        if ra == rb:
            continue
        xs = [i // 3 for i in range(0, len(ra), 3) if ra[i:i + 3] != rb[i:i + 3]]
        count += len(xs)
        lo, hi = x0 + xs[0], x0 + xs[-1] + 1
        box = [lo, y, hi, y + 1] if box is None else [min(box[0], lo), box[1], max(box[2], hi), y + 1]
    return (box, count) if box else None


def extents(name: str, role: str | None = None):
    _, _, node = atspi.find(name, role)
    e = node.queryComponent().getExtents(0)
    return [e.x, e.y, e.x + e.width, e.y + e.height]


def step(label: str, since: int, regions: dict, frames: int = 3, gap: float = 0.4, **extra):
    shots = []
    tag = f"{len(result['steps']) + 1:02d}-{label.replace(' ', '_')}"
    for i in range(frames):
        if i:
            time.sleep(gap)
        shots.append(shot(f"{tag}-{i}"))
    subprocess.run(["grim", os.path.join(OUT, f"{tag}.png")], timeout=10)
    moved = {}
    for rname, reg in regions.items():
        moved[rname] = [diff_box(shots[i], shots[i + 1], reg) for i in range(len(shots) - 1)]
    rec = {"label": label, "moved": moved, "cover_bus_threads": threads("cover-bus"),
           "log": log_lines(since), **extra}
    result["steps"].append(rec)
    print(json.dumps(rec), flush=True)
    return rec


def main():
    atspi.wait_ready(60)
    t0 = time.monotonic()
    while "heading: Fixture Albums" not in atspi.controls():
        if time.monotonic() - t0 > 60:
            raise TimeoutError("Home")
        time.sleep(0.2)
    w, h, _ = shot("00-home")
    page_region = [300, 0, w, h - 110]  # right of the sidebar, above the player bar

    # -- the album page
    n = log_len()
    atspi.press("Fixture Album", "heading")
    atspi.find("Favorite album", "button", timeout=20)
    first = wait_log("first frame page", n)
    time.sleep(0.5)
    step("album page", n, {"page": page_region}, first_frame=first)

    # -- Settings: the toggle off, then on again
    n = log_len()
    atspi.press("Account")
    time.sleep(0.8)
    atspi.press("Settings")
    atspi.find("Animated album covers", timeout=10)
    atspi.press("Animated album covers")
    time.sleep(0.6)
    atspi.key("Escape")
    time.sleep(1.5)
    step("toggle off", n, {"page": page_region})
    n = log_len()
    atspi.press("Account")
    time.sleep(0.8)
    atspi.press("Settings")
    atspi.press("Animated album covers")
    time.sleep(0.6)
    atspi.key("Escape")
    first = wait_log("first frame page", n)
    time.sleep(0.5)
    step("toggle on", n, {"page": page_region}, first_frame=first)

    # -- play the album, open the drawer: two covers, two pipelines
    n = log_len()
    atspi.press("Play", "button")
    time.sleep(2.0)
    atspi.press("Play queue")
    atspi.find("Now playing panel", timeout=10)
    first = wait_log("first frame now", n)
    time.sleep(0.8)
    try:
        dx = extents("Now playing panel")[0]
    except Exception as e:  # noqa: BLE001
        result["errors"].append(f"drawer extents: {e!r}")
        dx = w - 820
    step("drawer open", n, {"page": [300, 0, dx, h - 110], "drawer": [dx, 0, w, h - 110]}, first_frame=first, drawer_x=dx)

    # -- the full-screen player: the page cover stops (covered), the shared one goes large
    n = log_len()
    atspi.press("Open full-screen player")
    time.sleep(1.5)
    first = wait_log("first frame now", n)
    time.sleep(0.5)
    step("full player", n, {"centre": [w // 4, 0, w * 3 // 4, h - 160]}, first_frame=first)

    # -- unfocused (a second output takes focus): the cover holds its frame
    n = log_len()
    outs = json.loads(subprocess.run(["swaymsg", "-t", "get_outputs"], capture_output=True, text=True).stdout)
    if not any(o["name"] == "HEADLESS-2" for o in outs):
        subprocess.run(["swaymsg", "create_output"], capture_output=True)
    subprocess.run(["swaymsg", "focus", "output", "HEADLESS-2"], capture_output=True)
    time.sleep(1.0)
    step("full player unfocused", n, {"centre": [w // 4, 0, w * 3 // 4, h - 160]})
    n = log_len()
    subprocess.run(["swaymsg", "focus", "output", "HEADLESS-1"], capture_output=True)
    pid = app_pid()
    subprocess.run(["swaymsg", f"[pid={pid}]", "focus"], capture_output=True)
    time.sleep(1.0)
    step("full player refocused", n, {"centre": [w // 4, 0, w * 3 // 4, h - 160]})

    # -- close the player, then the drawer, then leave the page
    n = log_len()
    atspi.key("Escape")
    time.sleep(1.5)
    step("full player closed", n, {"page": [300, 0, dx, h - 110], "drawer": [dx, 0, w, h - 110]})
    n = log_len()
    atspi.press("Close now playing panel")
    time.sleep(1.0)
    step("drawer closed", n, {"page": page_region})
    n = log_len()
    atspi.press("Home")
    time.sleep(1.5)
    step("home", n, {"page": page_region})

    # -- quit: how long, and how it ended
    t_quit = time.monotonic()
    subprocess.run(["swaymsg", f"[pid={pid}]", "kill"], capture_output=True)
    gone = None
    while time.monotonic() - t_quit < 10:
        if not os.path.exists(f"/proc/{pid}"):
            gone = round(time.monotonic() - t_quit, 3)
            break
        time.sleep(0.02)
    tail = open(LOG, errors="ignore").read()
    result["quit"] = {"seconds": gone, "tls_abort": "Thread Local Storage" in tail, "panicked": "panicked" in tail}


try:
    main()
except Exception as e:  # noqa: BLE001
    import traceback
    result["errors"].append(traceback.format_exc())
json.dump(result, open(os.path.join(OUT, "drive.json"), "w"), indent=1)
