#!/usr/bin/env python3
"""Benchmark two SONE builds in the verifier's sandbox, one run at a time.

    bench.py --ref REFERENCE_BIN --cand CANDIDATE_BIN [--runs 5] [--display 1400x900]

Both builds get the same hermetic environment: the fake TIDAL, a seeded logged-in profile,
virtual audio (bit-perfect into the capture device), a headless compositor with software
rendering (WebKit's software path, Rune on lavapipe). Runs alternate between builds and never
overlap. Medians are reported (with min-max). Results go to verifier/bench/results/.
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


def one(binary: str, rundir: str, display: str) -> dict:
    out = os.path.join(rundir, "bench.json")
    env = {**os.environ, "SPIKE_SEED": json.dumps(BASE_PROFILE), "SPIKE_OUTPUT": display}
    cmd = [os.path.join(ROOT, "harness", "sandbox.sh"), rundir, binary, "--",
           "python3", "-I", os.path.join(ROOT, "bench", "driver.py"), out]
    subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=300)
    if not os.path.exists(out):
        raise RuntimeError(f"run failed: see {rundir}/logs")
    return json.load(open(out))


def flat(d: dict, prefix: str = "") -> dict:
    o = {}
    for k, v in d.items():
        if isinstance(v, dict):
            o.update(flat(v, f"{prefix}{k}."))
        else:
            o[f"{prefix}{k}"] = v
    return o


def size_info(binary: str) -> dict:
    real = os.path.realpath(binary)
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
    ("latency.open_album_s", "Open album (s)"),
    ("latency.open_artist_s", "Open artist (s)"),
    ("latency.search_results_s", "Search results (s)"),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", required=True)
    ap.add_argument("--cand", required=True)
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--display", default="1400x900")
    ap.add_argument("--out", default=os.path.join(ROOT, "runs", "bench-" + time.strftime("%Y%m%d-%H%M%S")))
    a = ap.parse_args()
    builds = {"SONE (Tauri/WebKit)": os.path.abspath(a.ref), "SONE on Rune": os.path.abspath(a.cand)}
    results = {k: [] for k in builds}
    for i in range(a.runs):
        for name, b in builds.items():  # alternate, so drift hits both builds equally
            rd = os.path.join(a.out, name.split()[-1].strip("()").lower().replace("/", "-"), f"run{i}")
            try:
                results[name].append(flat(one(b, rd, a.display)))
                print(f"run {i + 1}/{a.runs} {name}: ok", flush=True)
            except Exception as e:  # noqa: BLE001
                print(f"run {i + 1}/{a.runs} {name}: FAILED ({e})", flush=True)
    names = list(builds)
    lines = [f"| Metric | {names[0]} | {names[1]} | Rune vs WebKit |", "|---|---|---|---|"]
    summary = {}
    for key, label in ROWS:
        cells, meds = [], []
        for n in names:
            vals = [r[key] for r in results[n] if key in r]
            if not vals:
                cells.append("–")
                meds.append(None)
                continue
            med = statistics.median(vals)
            meds.append(med)
            cells.append(f"{med:g} ({min(vals):g}–{max(vals):g})")
            summary.setdefault(n, {})[key] = {"median": med, "min": min(vals), "max": max(vals), "n": len(vals)}
        rel = f"{meds[1] / meds[0]:.2f}×" if all(m not in (None, 0) for m in meds) else "–"
        lines.append(f"| {label} | {cells[0]} | {cells[1]} | {rel} |")
    sizes = {n: size_info(b) for n, b in builds.items()}
    lines.append(f"| Main binary (MB) | {sizes[names[0]]['binary_mb']} | {sizes[names[1]]['binary_mb']} | |")
    table = "\n".join(lines)
    print("\n" + table)
    os.makedirs(os.path.join(ROOT, "bench", "results"), exist_ok=True)
    stamp = time.strftime("%Y-%m-%d")
    json.dump({"runs": results, "summary": summary, "sizes": sizes, "display": a.display, "n": a.runs},
              open(os.path.join(ROOT, "bench", "results", f"{stamp}.json"), "w"), indent=1)
    open(os.path.join(ROOT, "bench", "results", f"{stamp}.md"), "w").write(
        f"# Benchmark {stamp}\n\n{a.runs} runs per build, alternating; median (min–max). Display {a.display}, "
        "software rendering, fake TIDAL, virtual audio.\n\n" + table + "\n")


if __name__ == "__main__":
    main()
