#!/usr/bin/env python3
"""Benchmark two SONE builds in the verifier's sandbox, one run at a time.

    bench.py --ref REFERENCE_BIN --cand CANDIDATE_BIN [--runs 5] [--display 1400x900]
             [--gpu] [--refresh 120] [--build "LABEL=BIN[,VAR=VALUE...]" ...]

Every build gets the same hermetic environment: the fake TIDAL, a seeded logged-in profile,
virtual audio (bit-perfect into the capture device), a headless compositor at --refresh Hz.
Without --gpu everything renders in software (WebKit's software path, Rune on lavapipe); with
--gpu the compositor, WebKit and Rune use the machine's GPU. --build adds a build, optionally
with environment variables (e.g. a reference with WEBKIT_DISABLE_DMABUF_RENDERER=0). The last
build is the candidate; the ratio column compares it with the first. Each run has two passes per
build: bench/driver.py (memory, CPU, latency) and bench/frames.py (frame pacing, with
WAYLAND_DEBUG on, so it doesn't share a pass with the CPU numbers). Runs alternate between
builds and never overlap. Medians are reported (with min-max). Results go to
verifier/bench/results/.
Run inside `nix-shell verifier/harness/shell.nix`.
"""
import argparse
import json
import os
import statistics
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ROOT)
from verify import BASE_PROFILE  # noqa: E402


def wrapper(rundir: str, binary: str, env: dict) -> str:
    """A launcher that sets the build's environment, then execs it."""
    os.makedirs(rundir, exist_ok=True)
    path = os.path.join(rundir, "launch.sh")
    lines = ["#!/bin/sh"] + [f"export {k}='{v}'" for k, v in env.items()] + [f'exec "{binary}" "$@"']
    open(path, "w").write("\n".join(lines) + "\n")
    os.chmod(path, 0o755)
    return path


def one(binary: str, env: dict, rundir: str, a, script: str) -> dict:
    out = os.path.join(rundir, "bench.json")
    senv = {**os.environ, "SPIKE_SEED": json.dumps(BASE_PROFILE), "SPIKE_OUTPUT": a.display,
            "SPIKE_REFRESH": str(a.refresh)}
    if a.gpu:
        senv["SPIKE_GPU"] = "1"
    if script == "frames.py":
        env = {**env, "WAYLAND_DEBUG": "client"}
    launch = wrapper(rundir + "-launch", binary, env)
    cmd = [os.path.join(ROOT, "harness", "sandbox.sh"), rundir, launch, "--",
           "python3", "-I", os.path.join(ROOT, "bench", script), out]
    p = subprocess.run(cmd, env=senv, capture_output=True, text=True, timeout=300)
    os.makedirs(rundir, exist_ok=True)
    open(os.path.join(rundir, "driver.log"), "w").write(p.stdout + p.stderr)
    if not os.path.exists(out):
        raise RuntimeError(f"run failed: see {rundir}/driver.log")
    return json.load(open(out))


def flat(d: dict, prefix: str = "") -> dict:
    o = {}
    for k, v in d.items():
        if isinstance(v, dict):
            o.update(flat(v, f"{prefix}{k}."))
        else:
            o[f"{prefix}{k}"] = v
    return o


def real_binary(path: str) -> str:
    """Follow wrapper scripts (nix's makeWrapper, rune-app/wrap.sh) to the ELF they exec."""
    import re
    real = os.path.realpath(path)
    for _ in range(4):
        # nix binary wrappers: a small ELF next to the real program, ".NAME-wrapped"
        sib = os.path.join(os.path.dirname(real), "." + os.path.basename(real).lstrip(".") + "-wrapped")
        if os.path.exists(sib) and os.path.getsize(real) < 1_000_000:
            real = os.path.realpath(sib)
            continue
        with open(real, "rb") as f:
            if f.read(4) == b"ELF":
                return real
        txt = open(real, errors="ignore").read()
        m = re.findall(r'exec (?:-a "\S+" )?"?([^"\s]+)"?', txt)
        if not m:
            return real
        real = os.path.realpath(m[-1])
    return real


def size_info(binary: str) -> dict:
    real = real_binary(binary)
    info = {"binary_mb": round(os.path.getsize(real) / 1e6, 1)}
    if real.startswith("/nix/store/"):
        pkg = "/".join(real.split("/")[:4])
        p = subprocess.run(["nix", "path-info", "-S", pkg], capture_output=True, text=True)
        if p.returncode == 0:
            info["closure_mb"] = round(int(p.stdout.split()[-1]) / 1e6, 1)
    return info


ROWS = [
    ("startup_backend_s", "Startup: backend ready (s)"),
    ("startup_ui_s", "Startup: UI on screen (s)"),
    ("startup_home_s", "Startup: Home content shown (s)"),
    ("idle_home.rss_mb", "Memory idle on Home: RSS (MB)"),
    ("idle_home.pss_mb", "Memory idle on Home: PSS (MB)"),
    ("idle_home.processes", "Processes"),
    ("after_browsing.pss_mb", "Memory after browsing: PSS (MB)"),
    ("playing.pss_mb", "Memory while playing: PSS (MB)"),
    ("idle_cpu_pct", "CPU idle on Home (% of a core)"),
    ("playing_cpu_pct", "CPU while playing (% of a core)"),
    ("playing_album_cpu_pct", "CPU while playing, bars animating (% of a core)"),
    ("latency.open_album_s", "Open album (s)"),
    ("latency.open_artist_s", "Open artist (s)"),
    ("latency.search_results_s", "Search results (s)"),
    ("frames.idle_fps", "Frames/s idle"),
    ("frames.playing_fps", "Frames/s while playing"),
    ("frames.playing_album.fps", "Frames/s while playing, bars animating"),
    ("frames.playing_album.worst_gap_ms", "Bars animating: worst frame gap (ms)"),
    ("frames.scroll.fps", "Wheel scroll: frames/s"),
    ("frames.scroll.median_gap_ms", "Wheel scroll: median frame gap (ms)"),
    ("frames.scroll.worst_gap_ms", "Wheel scroll: worst frame gap (ms)"),
    ("frames.modal.fps", "Dialog open/close: frames/s"),
    ("frames.modal.worst_gap_ms", "Dialog open/close: worst frame gap (ms)"),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref")
    ap.add_argument("--cand")
    ap.add_argument("--build", action="append", default=[], help='"LABEL=BIN[,VAR=VALUE...]"')
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--display", default="1400x900")
    ap.add_argument("--refresh", type=int, default=60)
    ap.add_argument("--gpu", action="store_true")
    ap.add_argument("--out", default=os.path.join(ROOT, "runs", "bench-" + time.strftime("%Y%m%d-%H%M%S")))
    a = ap.parse_args()
    builds = {}  # label -> (binary, env)
    if a.ref:
        builds["SONE (Tauri/WebKit)"] = (os.path.abspath(a.ref), {})
    for spec in a.build:
        label, rest = spec.split("=", 1)
        binary, *kv = rest.split(",")
        builds[label] = (os.path.abspath(binary), dict(x.split("=", 1) for x in kv))
    if a.cand:
        builds["SONE on Rune"] = (os.path.abspath(a.cand), {})
    names = list(builds)
    results = {k: [] for k in builds}
    for i in range(a.runs):
        for n, (b, env) in builds.items():  # alternate, so drift hits every build equally
            slug = "".join(c if c.isalnum() else "-" for c in n.lower()).strip("-")
            rd = os.path.join(a.out, slug, f"run{i}")
            try:
                r = flat(one(b, env, rd, a, "driver.py"))
                r.update(flat(one(b, env, rd + "-frames", a, "frames.py"), "frames."))
                results[n].append(r)
                print(f"run {i + 1}/{a.runs} {n}: ok", flush=True)
            except Exception as e:  # noqa: BLE001
                print(f"run {i + 1}/{a.runs} {n}: FAILED ({e})", flush=True)
    lines = ["| Metric | " + " | ".join(names) + f" | {names[-1]} vs {names[0]} |",
             "|---" * (len(names) + 2) + "|"]
    summary = {}
    for key, label in ROWS:
        cells, meds = [], []
        for n in names:
            vals = [r[key] for r in results[n] if r.get(key) is not None]
            if not vals:
                cells.append("–")
                meds.append(None)
                continue
            med = statistics.median(vals)
            meds.append(med)
            cells.append(f"{med:g} ({min(vals):g}–{max(vals):g})")
            summary.setdefault(n, {})[key] = {"median": med, "min": min(vals), "max": max(vals), "n": len(vals)}
        rel = f"{meds[-1] / meds[0]:.2f}×" if meds[0] not in (None, 0) and meds[-1] is not None else "–"
        lines.append(f"| {label} | " + " | ".join(cells) + f" | {rel} |")
    sizes = {n: size_info(b) for n, (b, _) in builds.items()}
    lines.append("| Main binary (MB) | " + " | ".join(str(sizes[n]["binary_mb"]) for n in names) + " | |")
    table = "\n".join(lines)
    print("\n" + table)
    os.makedirs(os.path.join(ROOT, "bench", "results"), exist_ok=True)
    mode = f"{'gpu' if a.gpu else 'software'}-{a.refresh}hz"
    stamp = time.strftime("%Y-%m-%d") + "-" + mode
    json.dump({"runs": results, "summary": summary, "sizes": sizes, "display": a.display, "n": a.runs,
               "gpu": a.gpu, "refresh": a.refresh, "builds": {n: {"bin": b, "env": e} for n, (b, e) in builds.items()}},
              open(os.path.join(ROOT, "bench", "results", f"{stamp}.json"), "w"), indent=1)
    render = "the machine's GPU" if a.gpu else "software rendering"
    open(os.path.join(ROOT, "bench", "results", f"{stamp}.md"), "w").write(
        f"# Benchmark {stamp}\n\n{a.runs} runs per build, alternating; median (min–max). Display {a.display} "
        f"at {a.refresh} Hz, {render}, fake TIDAL, virtual audio.\n\n" + table + "\n")


if __name__ == "__main__":
    main()
