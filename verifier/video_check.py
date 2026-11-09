#!/usr/bin/env python3
"""Music video check for the Rune build: plays the fixture video in the sandbox on the GPU at
120 Hz and reports behaviour, frame pacing and CPU.

    video_check.py --bin rune-app/target/release/sone-rune-wrapped [--out DIR] [--refresh 120]

Run inside `nix-shell verifier/harness/shell.nix`. There is no golden: the React build's web
view can't play HLS inside the sandbox (it shows "Unable to play this video" without making a
request), so this checks the Rune player against what React's VideoPlayer does instead.

Pacing comes from the app's frame trace (SPIKE_VIDEO_TRACE): when each decoded frame was handed
to the window, its timestamp, and when Rune submitted a draw of it. The video's audio goes to a
silent sink here (SPIKE_VIDEO_NO_AUDIO) so the pipeline runs on the system clock; the sandbox's
virtual sound card runs a few percent slow, which would read as drift.
"""
import argparse
import collections
import json
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ROOT)
import verify  # noqa: E402


def sessions(trace_path):
    """Per video session (frame ids restart at 1): {id: (push_ms, pts_ms)} and first draws."""
    out, cur = [], None
    for line in open(trace_path):
        kind, i, t, pts = line.split()
        i, t, pts = int(i), float(t), float(pts)
        if kind == "push":
            if cur is None or i == 1:
                cur = {"push": {}, "draw": {}}
                out.append(cur)
            cur["push"][i] = (t, pts)
        elif cur is not None:
            cur["draw"].setdefault(i, t)
    return out


def pacing(s, hz):
    ids = sorted(i for i in s["push"] if i in s["draw"])
    push = s["push"]
    # Stretches of continuous playback (a pause or seek leaves a gap).
    pairs = [(a, b) for a, b in zip(ids, ids[1:]) if b == a + 1 and push[b][0] - push[a][0] < 100]
    wall = sum(push[b][0] - push[a][0] for a, b in pairs)
    media = sum(push[b][1] - push[a][1] for a, b in pairs)
    # The first frame swaps the poster for the picture (a layout change), so it's left out.
    lat = sorted(s["draw"][i] - push[i][0] for i in ids if i > 1)
    jitter = sorted(abs((push[b][0] - push[a][0]) - (push[b][1] - push[a][1])) for a, b in pairs)
    frame_ms = (media / len(pairs)) if pairs else 0
    return {
        "frames": len(push),
        "drawn": len(ids),
        "fps": round(1000 / frame_ms, 2) if frame_ms else None,
        "clock_ratio": round(wall / media, 4) if media else None,
        "release_jitter_ms_p95": round(jitter[int(len(jitter) * 0.95)], 2) if jitter else None,
        "to_draw_ms_median": round(lat[len(lat) // 2], 2) if lat else None,
        "to_draw_ms_max": round(lat[-1], 2) if lat else None,
        # A frame released on time and drawn within a refresh presents at the next vblank.
        "late_draws": sum(1 for x in lat if x > 1000 / hz + 0.5),  # 0.5 ms of timer slack
        "refreshes_per_frame": round(hz * frame_ms / 1000, 2) if frame_ms else None,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", default=os.path.join(ROOT, "runs", time.strftime("%Y%m%d-%H%M%S") + "-video"))
    ap.add_argument("--refresh", type=int, default=120)
    ap.add_argument("--close-test", action="store_true",
                    help="only close videos while a segment download is in flight (it must not crash)")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    trace = os.path.join(a.out, "frames.txt")
    env = {**os.environ, "SPIKE_SEED": json.dumps(verify.BASE_PROFILE), "SPIKE_OUTPUT": "1400x900", "SPIKE_GPU": "1",
           "SPIKE_REFRESH": str(a.refresh), "SPIKE_VIDEO_TRACE": trace, "SPIKE_VIDEO_NO_AUDIO": "1"}
    if a.close_test:
        env.update(SPIKE_SLOW_TS="4", SPIKE_VIDEO_DRIVE="close")
    cmd = [os.path.join(ROOT, "harness", "sandbox.sh"), os.path.join(a.out, "run"), os.path.abspath(a.bin), "--",
           "python3", "-I", os.path.join(ROOT, "harness", "video_drive.py"), a.out]
    p = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=300)
    drive_path = os.path.join(a.out, "drive.json")
    if not os.path.exists(drive_path):
        print("no result; sandbox said:", (p.stdout + p.stderr)[-1500:])
        return 1
    drive = json.load(open(drive_path))
    steps = {s["label"]: s for s in drive["steps"]}
    if a.close_test:
        ok = True
        for label in ("escape", "replay", "quality", "settled"):
            alive = steps.get(f"close mid-download: {label}", {}).get("alive")
            ok &= bool(alive)
            print(f"{'PASS' if alive else 'FAIL'}  survives closing mid-download: {label}")
        for e in drive["errors"]:
            print("error:", e)
        print(f"\n({a.out})")
        return 0 if ok and not drive["errors"] else 1
    log = open(os.path.join(a.out, "run", "logs", "sone.log"), errors="ignore").read()
    decoders = sorted({l.split("decoder: ")[1].strip() for l in log.splitlines() if "[video] decoder: " in l})

    checks = []
    def check(name, ok, detail=""):
        checks.append((name, bool(ok), detail))
    g = lambda label, k: steps.get(label, {}).get(k)  # noqa: E731
    check("videos tab lists the video", g("videos tab", "card"))
    check("video plays; MPRIS says Playing with its title", g("play 1", "playing") and g("play 1", "title") == "Fixture Video",
          f"{g('play 1', 'seconds_to_playing')} s to Playing")
    check("player opens over the app", g("play 1", "player"))
    check("player closes at the end", g("after the end", "player") is False)
    check("plays again after the end", g("play 2", "playing"))
    check("pause and play from the player", g("paused from the player", "paused") and g("resumed from the player", "playing"))
    check("minimise to the bar, expand again", g("minimised", "player") is False and g("minimised", "expand") and g("expanded", "player"))
    check("Escape closes the video", g("escape", "player") is False and g("escape", "status") != "Playing")

    ss = sessions(trace) if os.path.exists(trace) else []
    # The quality switch opens a third session; it must carry on where the second stopped.
    if len(ss) >= 3:
        last = max(pts for _, pts in ss[1]["push"].values())
        first = min(pts for _, pts in ss[2]["push"].values())
        check("quality switch carries on from the same moment", g("quality medium", "status") == "Playing" and abs(first - last) < 200,
              f"stopped at {last - 62000:.0f} ms, resumed at {first - 62000:.0f} ms")
    else:
        check("quality switch carries on from the same moment", False, f"{len(ss)} sessions")
    pace = pacing(ss[0], a.refresh) if ss else {}
    if pace:
        check("every frame drawn", pace["drawn"] == pace["frames"], f"{pace['drawn']}/{pace['frames']}")
        check("frames released on the clock", abs(pace["clock_ratio"] - 1) < 0.01 and pace["release_jitter_ms_p95"] < 4,
              f"wall/media {pace['clock_ratio']}, p95 jitter {pace['release_jitter_ms_p95']} ms")
        check("each frame drawn within one refresh", pace["late_draws"] == 0,
              f"median {pace['to_draw_ms_median']} ms, max {pace['to_draw_ms_max']} ms")

    report = {"checks": [{"check": n, "ok": ok, "detail": d} for n, ok, d in checks], "pacing": pace,
              "cpu_percent_while_playing": g("cpu while playing", "cpu_percent"),
              "rss_mb": {"before": g("cpu while playing", "rss_idle_mb"), "playing": g("cpu while playing", "rss_mb"), "after_close": g("after closing", "rss_mb")}, "decoders": decoders,
              "errors": drive["errors"]}
    json.dump(report, open(os.path.join(a.out, "report.json"), "w"), indent=1)
    for n, ok, d in checks:
        print(f"{'PASS' if ok else 'FAIL'}  {n}" + (f"  ({d})" if d else ""))
    print(f"\npacing: {json.dumps(pace)}")
    print(f"cpu while playing: {report['cpu_percent_while_playing']}%   decoder: {', '.join(decoders) or 'unknown'}")
    print(f"memory (MB): {report['rss_mb']}")
    print("cpu by thread (%):", drive.get("threads"))
    for e in drive["errors"]:
        print("error:", e)
    print(f"\n({a.out})")
    return 0 if all(ok for _, ok, _ in checks) and not drive["errors"] else 1


if __name__ == "__main__":
    sys.exit(main())
