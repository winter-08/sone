#!/usr/bin/env python3
"""Animated album cover check for the Rune build (cover_video.rs): runs harness/vcover_drive.py
in the sandbox with the fake TIDAL's animated cover on (SPIKE_VIDEO_COVER=1) and reports.

    vcover_check.py --bin rune-app/target/debug/sone-rune-wrapped [--out DIR] [--gpu]

Run inside `nix-shell verifier/harness/shell.nix` (and under `flock /tmp/sone-sandbox.lock`).
Needs fixtures/videocover (harness/make_fixtures.sh). The display is 1920x1200 so the docked
drawer has its cover column. "Moves" means two screenshots 0.4 s apart differ inside the
region; the cover is the only thing there that animates (the player bar is left out).
"""
import argparse
import json
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ROOT)
import verify  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", default=os.path.join(ROOT, "runs", time.strftime("%Y%m%d-%H%M%S") + "-vcover"))
    ap.add_argument("--gpu", action="store_true")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    env = {**os.environ, "SPIKE_SEED": json.dumps(verify.BASE_PROFILE), "SPIKE_OUTPUT": "1920x1200", "SPIKE_VIDEO_COVER": "1"}
    env.pop("SPIKE_GPU", None)
    if a.gpu:
        env["SPIKE_GPU"] = "1"
    cmd = [os.path.join(ROOT, "harness", "sandbox.sh"), os.path.join(a.out, "run"), os.path.abspath(a.bin), "--",
           "python3", "-I", os.path.join(ROOT, "harness", "vcover_drive.py"), a.out]
    p = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=400)
    path = os.path.join(a.out, "drive.json")
    if not os.path.exists(path):
        print("no result; sandbox said:", (p.stdout + p.stderr)[-2000:])
        return 1
    d = json.load(open(path))
    steps = {s["label"]: s for s in d["steps"]}
    checks = []

    def check(name, ok, detail=""):
        checks.append((name, bool(ok), detail))

    # (the fixture's square crosses the cover in 4 s: even the album page's 180 pt cover changes
    # well over MOVED pixels in 0.4 s; a still cover changes none, or a stray few at an edge)
    MOVED, STRAY = 300, 100

    def moves(label, region):
        m = steps.get(label, {}).get("moved", {}).get(region)
        return m is not None and all(x is not None and x[1] >= MOVED for x in m)

    def still(label, region):
        m = steps.get(label, {}).get("moved", {}).get(region)
        return m is not None and all(x is None or x[1] < STRAY for x in m)

    def box(label, region):
        m = steps.get(label, {}).get("moved", {}).get(region) or []
        return next((x[0] for x in m if x), None)

    def bus(label):
        return steps.get(label, {}).get("cover_bus_threads")

    def logged(label, needle):
        return any(needle in l for l in steps.get(label, {}).get("log", []))

    check("album page: the cover animates", moves("album page", "page"), f"changed box {box('album page', 'page')}, {bus('album page')} pipeline(s)")
    check("album page: the 640 file, at the drawn size", logged("album page", "of 640"), "; ".join(l for l in steps.get("album page", {}).get("log", []) if "start" in l))
    check("toggle off: the cover stops and its pipeline goes", still("toggle off", "page") and logged("toggle off", "stop page") and bus("toggle off") == 0,
          f"{bus('toggle off')} pipeline(s)")
    check("toggle on: it plays again", moves("toggle on", "page") and bus("toggle on") == 1)
    check("drawer: its cover animates beside the page's (two pipelines)", moves("drawer open", "drawer") and moves("drawer open", "page") and bus("drawer open") == 2,
          f"drawer box {box('drawer open', 'drawer')}, {bus('drawer open')} pipeline(s)")
    check("narrower window: the drawer's cover is rescaled in place and keeps moving", moves("window narrower", "drawer") and logged("window narrower", "resize now"),
          "; ".join(l for l in steps.get("window narrower", {}).get("log", []) if "resize" in l or "start" in l))
    check("full player: the cover animates; the page cover stops under it", moves("full player", "centre") and logged("full player", "stop page") and bus("full player") == 1,
          "; ".join(steps.get("full player", {}).get("log", [])))
    check("full player: drawn large, so the 1280 file", logged("full player", "of 1280"))
    check("unfocused: the pipeline pauses and the still shows", still("full player unfocused", "centre") and logged("full player unfocused", "pause now"))
    check("refocused: it moves again", moves("full player refocused", "centre") and logged("full player refocused", "resume now"))
    check("drawer closed: one pipeline (the page's)", bus("drawer closed") == 1 and logged("drawer closed", "stop now"))
    check("page left: no pipeline", bus("home") == 0 and logged("home", "stop page"))
    q = d.get("quit") or {}
    check("quits in well under a second, no TLS abort", q.get("seconds") is not None and q["seconds"] < 1.0 and not q.get("tls_abort"),
          f"{q.get('seconds')} s")
    for name, ok, detail in checks:
        print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  ({detail})" if detail else ""))
    for e in d["errors"]:
        print("error:", e)
    json.dump({"checks": [{"check": n, "ok": ok, "detail": det} for n, ok, det in checks], **d}, open(os.path.join(a.out, "report.json"), "w"), indent=1)
    print(f"\n({a.out})")
    return 0 if all(ok for _, ok, _ in checks) and not d["errors"] else 1


if __name__ == "__main__":
    sys.exit(main())
