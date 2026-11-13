#!/usr/bin/env python3
"""Drive Rune builds of SONE through a fixed list of states and compare their SONE_PERF numbers.

    bench/states.sh [--build] LABEL=BIN[,VAR=VALUE...] ... [--runs 3] [--modes software,gpu]
                    [--keep 2] [--big 3000] [--states a,b,...] [--tag NAME]

(states.sh enters the verifier's nix shell, pinned; this file can also be run inside that shell
directly.) Every build runs in the verifier sandbox (harness/sandbox.sh: fake TIDAL, seeded
signed-in profile, private PipeWire with a null sink, headless sway, no network), with
SONE_PERF=1, one session per (run, mode, build), sessions strictly one after another, builds
alternating within each run and modes alternating within each run, so drift and machine load hit
every build alike. bench/states_driver.py walks each session through the states (its docstring
says how intervals are chosen) and writes one JSON; this file collects them, takes medians with
min-max over the runs, and writes verifier/bench/results/states-<date>-<tag>.{json,md}.

Modes: "software" = sway on pixman, Rune on lavapipe (SONE_SOFTWARE_VULKAN=1); "gpu" =
SPIKE_GPU=1 (sway's Vulkan renderer and Rune on the machine's GPU).

The big list: the fake TIDAL's Loved Tracks is made --big tracks long (SPIKE_BENCH_BIG, see
harness/fake_tidal.py), each a 600 s silent FLAC generated here into fixtures/bench/ if missing.
Every "playing" state plays two of those tracks with repeat on, so position ticks are steady (no
track restarts every few seconds, which the 10 s fixtures cause; see HANDOFF.md).
"""
import argparse
import json
import os
import statistics
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
from verify import BASE_PROFILE  # noqa: E402

LONG = os.path.join(ROOT, "fixtures", "bench", "long_44k16.flac")

STATE_LABELS = {
    "idle_home": "Idle on Home, nothing played",
    "closed_playing": "Player closed, playing",
    "open_playing": "Player open, playing",
    "open_paused": "Player open, paused",
    "open_unfocused": "Player open, playing, window unfocused (visible)",
    "covered_playing": "Playing, window covered by a fullscreen window",
    "hidden_workspace": "Playing, window on a hidden workspace",
    "hidden_scratchpad": "Playing, window in the scratchpad (hidden)",
    "biglist_playing": "Big list open, playing (row shows playing bars)",
    "biglist_scroll": "Big list, wheel-scrolling, playing",
    "biglist_paused": "Big list open, paused",
}

# (key in a state's record, label, number format)
METRICS = [
    ("perf.frames", "Frames/s (drawn)", "{:.1f}"),
    ("perf.unchanged_per_s", "Unchanged frames/s", "{:.1f}"),
    ("perf.skipped_per_s", "Skipped frames/s", "{:.1f}"),
    ("perf.views", "View runs/s", "{:.1f}"),
    ("perf.polls", "Polls/s", "{:.1f}"),
    ("perf.regions", "Regions/s (perf line, newer builds only)", "{:.1f}"),
    ("perf.cpu", "CPU, % of a core (perf line)", "{:.1f}"),
    ("ext.sway_cpu_pct", "Compositor CPU, % of a core", "{:.1f}"),
    ("perf.rss", "RSS MB (perf line)", "{:.0f}"),
    ("ext.pss_mb", "PSS MB (smaps_rollup)", "{:.0f}"),
    ("gpu_mem_mib.app", "GPU memory MiB (nvidia-smi, app)", "{:.0f}"),
    ("ext.machine_busy_pct", "Machine busy % during window (all 24 cores)", "{:.1f}"),
]


def wrapper(path: str, binary: str, env: dict) -> str:
    lines = ["#!/bin/sh"] + [f"export {k}='{v}'" for k, v in env.items()] + [f'exec "{binary}" "$@"']
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w").write("\n".join(lines) + "\n")
    os.chmod(path, 0o755)
    return path


def ensure_long_fixture():
    if os.path.exists(LONG):
        return
    os.makedirs(os.path.dirname(LONG), exist_ok=True)
    tmp = LONG + ".tmp.flac"
    subprocess.run(["sox", "-n", "-r", "44100", "-b", "16", "-c", "2", tmp, "trim", "0", "600"], check=True)
    os.rename(tmp, LONG)


def loadavg() -> list[float]:
    return [float(x) for x in open("/proc/loadavg").read().split()[:3]]


def one(label: str, binary: str, env: dict, mode: str, rundir: str, a) -> dict:
    out = os.path.join(rundir, "states.json")
    senv = {**os.environ, "SPIKE_OUTPUT": a.display, "SPIKE_REFRESH": str(a.refresh),
            "SPIKE_SEED": json.dumps(BASE_PROFILE), "SPIKE_BENCH_BIG": str(a.big),
            "SPIKE_BENCH_KEEP": str(a.keep), "SPIKE_BENCH_SETTLE": str(a.settle)}
    if a.states:
        senv["SPIKE_BENCH_STATES"] = a.states
    senv.pop("SPIKE_BENCH_BIG_LIMIT", None)
    if a.big_limit == "honor":
        senv["SPIKE_BENCH_BIG_LIMIT"] = "honor"
    senv["SPIKE_BENCH_RSS_LIMIT_MB"] = str(a.rss_limit)
    senv.pop("SPIKE_GPU", None)
    if mode == "gpu":
        senv["SPIKE_GPU"] = "1"
    launch = wrapper(rundir + "-launch/launch.sh", binary, {"SONE_PERF": "1", **env})
    cmd = [os.path.join(ROOT, "harness", "sandbox.sh"), rundir, launch, "--",
           "python3", "-I", os.path.join(ROOT, "bench", "states_driver.py"), out]
    la0, t0 = loadavg(), time.time()
    try:
        p = subprocess.run(cmd, env=senv, capture_output=True, text=True, timeout=a.timeout)
        log = p.stdout + p.stderr
    except subprocess.TimeoutExpired as e:
        log = f"timeout after {a.timeout}s\n" + str(e.stdout or "") + str(e.stderr or "")
    os.makedirs(rundir, exist_ok=True)
    open(os.path.join(rundir, "driver.log"), "w").write(log)
    if not os.path.exists(out):
        raise RuntimeError(f"no result: see {rundir}/driver.log")
    r = json.load(open(out))
    r["session"] = {"wall_s": round(time.time() - t0, 1), "loadavg_start": la0, "loadavg_end": loadavg(),
                    "rundir": rundir, "started": time.strftime("%H:%M:%S", time.localtime(t0))}
    json.dump(r, open(out, "w"), indent=1)  # so --collect can rebuild the tables later
    return r


def collect(outdir: str, builds: dict, modes: list) -> tuple[dict, list]:
    """Results of sessions already on disk (a pass that was cut short): outdir/MODE/SLUG/runN."""
    results = {m: {b: [] for b in builds} for m in modes}
    missing = []
    for m in modes:
        for label in builds:
            slug = "".join(c if c.isalnum() else "-" for c in label.lower()).strip("-")
            d = os.path.join(outdir, m, slug)
            runs = sorted((x for x in os.listdir(d) if x.startswith("run") and x[3:].isdigit()), key=lambda x: int(x[3:])) if os.path.isdir(d) else []
            for rn in runs:
                f = os.path.join(d, rn, "states.json")
                if not os.path.exists(f):
                    missing.append(f"{m} {label} {rn}: no states.json (session cut short)")
                    continue
                r = json.load(open(f))
                r.setdefault("session", {"wall_s": 0, "loadavg_start": [float("nan")] * 3, "loadavg_end": [float("nan")] * 3,
                                         "rundir": os.path.join(d, rn),
                                         "started": time.strftime("%H:%M:%S", time.localtime(os.path.getmtime(f)))})
                results[m][label].append(r)
    return results, missing


def get(d: dict, dotted: str):
    for k in dotted.split("."):
        if not isinstance(d, dict) or k not in d:
            return None
        d = d[k]
    return d if isinstance(d, (int, float)) else None


def agg(vals: list) -> dict | None:
    vals = [v for v in vals if v is not None]
    if not vals:
        return None
    return {"median": statistics.median(vals), "min": min(vals), "max": max(vals), "n": len(vals)}


def cell(s: dict | None, fmt: str, took: str = "") -> str:
    if s is None:
        return "–"
    if s["n"] == 1:
        return fmt.format(s["median"]) + took
    return f"{fmt.format(s['median'])} ({fmt.format(s['min'])}–{fmt.format(s['max'])})" + took


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("builds", nargs="*", help="LABEL=BIN[,VAR=VALUE...]")
    ap.add_argument("--build", action="append", default=[])
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--modes", default="software,gpu")
    ap.add_argument("--gpu", action="store_true", help="shorthand for --modes gpu")
    ap.add_argument("--software", action="store_true", help="shorthand for --modes software")
    ap.add_argument("--keep", type=int, default=2, help="5 s perf intervals kept per state")
    ap.add_argument("--settle", type=float, default=1.0)
    ap.add_argument("--big", type=int, default=3000)
    ap.add_argument("--big-limit", choices=["all", "honor"], default="honor",
                    help="all: the Loved Tracks request gets every row from its offset on; "
                         "honor (default): only `limit` rows, as TIDAL does (the baseline asks for 100)")
    ap.add_argument("--rss-limit", type=int, default=3000, help="MB; the driver kills the app above it")
    ap.add_argument("--states", default="", help="comma list (default: all, see states_driver.py)")
    ap.add_argument("--display", default="1400x900")
    ap.add_argument("--refresh", type=int, default=60)
    ap.add_argument("--timeout", type=int, default=900)
    ap.add_argument("--tag", default="")
    ap.add_argument("--out", default=None)
    ap.add_argument("--collect", metavar="DIR", help="run nothing: rebuild the tables from the sessions saved in "
                    "DIR (verifier/runs/states-...), e.g. after a pass was cut short; give the same builds and modes")
    a = ap.parse_args()
    modes = ["gpu"] if a.gpu else ["software"] if a.software else [m for m in a.modes.split(",") if m]
    builds = {}
    for spec in a.builds + a.build:
        label, rest = spec.split("=", 1)
        binary, *kv = rest.split(",")
        if not os.path.exists(binary) and not a.collect:
            sys.exit(f"{label}: {binary} does not exist")
        builds[label] = (os.path.abspath(binary), dict(x.split("=", 1) for x in kv))
    if not builds:
        sys.exit("no builds given")
    stamp = time.strftime("%Y-%m-%d-%H%M%S")
    outdir = a.out or os.path.join(ROOT, "runs", f"states-{stamp}")
    results = {m: {b: [] for b in builds} for m in modes}
    failures = []
    t_start = time.time()
    if a.collect:
        outdir = os.path.abspath(a.collect)
        stamp = os.path.basename(outdir.rstrip("/")).removeprefix("states-")
        results, failures = collect(outdir, builds, modes)
        a.runs = 0
    else:
        ensure_long_fixture()
    for i in range(a.runs):
        for mode in modes:
            order = list(builds) if i % 2 == 0 else list(reversed(builds))  # alternate who goes first
            for label in order:
                binary, env = builds[label]
                slug = "".join(c if c.isalnum() else "-" for c in label.lower()).strip("-")
                rd = os.path.join(outdir, mode, slug, f"run{i}")
                try:
                    r = one(label, binary, env, mode, rd, a)
                    results[mode][label].append(r)
                    bad = [s for s, v in r["states"].items() if not v.get("took")]
                    print(f"run {i + 1}/{a.runs} {mode} {label}: {r['session']['wall_s']} s, load "
                          f"{r['session']['loadavg_start'][0]:.1f}->{r['session']['loadavg_end'][0]:.1f}"
                          + (f", did not take: {', '.join(bad)}" if bad else "")
                          + (f", errors: {r['errors']}" if r.get("errors") else ""), flush=True)
                except Exception as e:  # noqa: BLE001
                    failures.append(f"run {i + 1} {mode} {label}: {e}")
                    print(f"run {i + 1}/{a.runs} {mode} {label}: FAILED ({e})", flush=True)
    total_s = round(time.time() - t_start)

    # ---- aggregate
    names = list(builds)
    state_names = []
    for m in modes:
        for b in names:
            for r in results[m][b]:
                for s in r["states"]:
                    if s not in state_names:
                        state_names.append(s)
    summary = {}
    for m in modes:
        for b in names:
            rs = results[m][b]
            sm = summary.setdefault(m, {}).setdefault(b, {"states": {}})
            for s in state_names:
                recs = [r["states"][s] for r in rs if s in r["states"]]
                took = [x for x in recs if x.get("took")]
                st = {"took": len(took), "runs": len(recs),
                      "notes": sorted({n for x in recs for n in x.get("notes", [])})}
                # numbers only from runs where the state took
                for key, _, _ in METRICS:
                    st[key] = agg([get(x, key) for x in took])
                # any other perf field a later build adds
                extra = sorted({k for x in took for k in x.get("perf", {})} - {k.split(".", 1)[1] for k, _, _ in METRICS if k.startswith("perf.")})
                for k in extra:
                    st["perf." + k] = agg([get(x, "perf." + k) for x in took])
                st["threads_pct"] = {}
                for x in took:
                    for tn, v in x.get("ext", {}).get("threads_pct", {}).items():
                        st["threads_pct"].setdefault(tn, []).append(v)
                st["threads_pct"] = {tn: statistics.median(v) for tn, v in sorted(st["threads_pct"].items(), key=lambda kv: -statistics.median(kv[1]))[:6]}
                for k in ("nav_heading_s", "count_lookup_s", "enter_s"):
                    st[k] = agg([x.get(k) for x in recs if x.get(k) is not None])
                sm["states"][s] = st
            for k in ("backend_s", "ui_exposed_s", "home_content_s", "first_frame_s"):
                sm["startup." + k] = agg([r.get("startup", {}).get(k) for r in rs])
            sm["quit.exit_s"] = agg([r.get("quit", {}).get("exit_s") for r in rs])
            sm["quit.exited"] = sum(1 for r in rs if r.get("quit", {}).get("exited"))
            sm["runs"] = len(rs)

    # ---- markdown
    L = [f"# States benchmark {stamp}" + (f" ({a.tag})" if a.tag else ""), ""]
    L.append(f"{max((len(results[m][n]) for m in modes for n in names), default=0)} runs per build and mode" + (f" (re-collected from {outdir})" if a.collect else "") + ", sessions one at a time, builds alternating. Median (min–max) "
             f"over the runs where the state took. Each state: the first 5 s perf interval after the change "
             f"is dropped, then {a.keep} interval(s) kept ({5 * a.keep} s). Display {a.display} at {a.refresh} Hz, "
             f"fake TIDAL, virtual audio, Loved Tracks = {a.big} tracks ({'served `limit` rows per request, as TIDAL' if a.big_limit == 'honor' else 'every row in one response'}). Total wall time {total_s // 60} min {total_s % 60} s.")
    L.append("")
    L.append("Builds: " + "; ".join(f"`{n}` = `{os.path.relpath(b, os.path.dirname(ROOT))}`" + (f" {e}" if e else "") for n, (b, e) in builds.items()))
    L.append("")
    for m in modes:
        L.append(f"## {'Software rendering (pixman sway, lavapipe)' if m == 'software' else 'GPU'}")
        L.append("")
        # one compact table: state rows, a column per build per headline metric
        heads = [("perf.frames", "fps"), ("perf.views", "views/s"), ("perf.polls", "polls/s"),
                 ("perf.cpu", "CPU %"), ("ext.pss_mb", "PSS MB")]
        L.append("| State | took | " + " | ".join(f"{h} {n}" for _, h in heads for n in names) + " |")
        L.append("|---|---|" + "---|" * (len(heads) * len(names)))
        for s in state_names:
            tk = " ".join(f"{summary[m][n]['states'].get(s, {}).get('took', 0)}/{summary[m][n]['states'].get(s, {}).get('runs', 0)}" for n in names)
            row = [STATE_LABELS.get(s, s), tk]
            for key, _ in heads:
                fmt = next(f for k, _, f in METRICS if k == key)
                for n in names:
                    row.append(cell(summary[m][n]["states"].get(s, {}).get(key), fmt))
            L.append("| " + " | ".join(row) + " |")
        L.append("")
        L.append("<details><summary>All metrics</summary>")
        L.append("")
        for key, label, fmt in METRICS:
            if all(summary[m][n]["states"].get(s, {}).get(key) is None for n in names for s in state_names):
                continue
            L.append(f"**{label}**")
            L.append("")
            L.append("| State | " + " | ".join(names) + " |")
            L.append("|---|" + "---|" * len(names))
            for s in state_names:
                L.append(f"| {STATE_LABELS.get(s, s)} | " + " | ".join(cell(summary[m][n]["states"].get(s, {}).get(key), fmt) for n in names) + " |")
            L.append("")
        extra_keys = sorted({k for n in names for s in state_names for k in summary[m][n]["states"].get(s, {})
                             if k.startswith("perf.") and k not in {x for x, _, _ in METRICS}})
        for key in extra_keys:
            L.append(f"**{key}** (perf line field)")
            L.append("")
            L.append("| State | " + " | ".join(names) + " |")
            L.append("|---|" + "---|" * len(names))
            for s in state_names:
                L.append(f"| {STATE_LABELS.get(s, s)} | " + " | ".join(cell(summary[m][n]["states"].get(s, {}).get(key), "{:g}") for n in names) + " |")
            L.append("")
        L.append("**Busiest threads (median % of a core)**")
        L.append("")
        L.append("| State | " + " | ".join(names) + " |")
        L.append("|---|" + "---|" * len(names))
        for s in state_names:
            L.append(f"| {STATE_LABELS.get(s, s)} | " + " | ".join(", ".join(f"{t} {v:g}" for t, v in summary[m][n]["states"].get(s, {}).get("threads_pct", {}).items()) or "–" for n in names) + " |")
        L.append("")
        L.append("</details>")
        L.append("")
        L.append("| Timing (s) | " + " | ".join(names) + " |")
        L.append("|---|" + "---|" * len(names))
        for k, label in [("startup.first_frame_s", "Launch → window mapped (first frame)"),
                         ("startup.backend_s", "Launch → MCP answers"),
                         ("startup.ui_exposed_s", "Launch → Home button on AT-SPI"),
                         ("startup.home_content_s", "Launch → Home content on AT-SPI"),
                         ("quit.exit_s", "Close request → process exited")]:
            L.append(f"| {label} | " + " | ".join(cell(summary[m][n].get(k), "{:.3f}") for n in names) + " |")
        L.append("| Big list: press → its heading on AT-SPI | " + " | ".join(cell(summary[m][n]["states"].get("biglist_playing", {}).get("nav_heading_s"), "{:.3f}") for n in names) + " |")
        L.append("")
        notes = [(n, s, summary[m][n]["states"][s]) for n in names for s in state_names
                 if s in summary[m][n]["states"] and summary[m][n]["states"][s]["took"] < summary[m][n]["states"][s]["runs"]]
        if notes:
            L.append("States that did not take in some runs (their numbers are left out):")
            L.append("")
            for n, s, st in notes:
                L.append(f"- {n} / {s}: {st['took']}/{st['runs']}: " + "; ".join(st["notes"][:4]))
            L.append("")
        L.append("Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:")
        L.append("")
        for n in names:
            for i, r in enumerate(results[m][n]):
                fr = r.get("fav_requests") or []
                vis = r.get("visibility_log_all") or []
                g = r.get("guard")
                L.append(f"- {n} run{i}: requests " + (", ".join(f"{x['offset']}+{x['limit']}→{x['rows_served']}" for x in fr) or "none")
                         + f"; rows served {sum(x['rows_served'] for x in fr)}"
                         + f"; on/off-screen log lines {len(vis)}"
                         + (f"; KILLED by guard at {g['killed_at_rss_mb']} MB during {g.get('phase')}" if g else "")
                         + (f"; errors {len(r.get('errors', []))}" if r.get("errors") else ""))
        L.append("")
        L.append("Sessions (machine load, 1-min loadavg at start → end):")
        L.append("")
        for n in names:
            L.append(f"- {n}: " + ", ".join(f"{r['session']['started']} {r['session']['loadavg_start'][0]:.1f}→{r['session']['loadavg_end'][0]:.1f} ({r['session']['wall_s']:.0f} s)" for r in results[m][n]))
        L.append("")
    if failures:
        L.append("Failed sessions: " + "; ".join(failures))
    md = "\n".join(L) + "\n"
    print("\n" + md)
    os.makedirs(os.path.join(ROOT, "bench", "results"), exist_ok=True)
    base = os.path.join(ROOT, "bench", "results", f"states-{stamp}" + (f"-{a.tag}" if a.tag else ""))
    json.dump({"args": vars(a), "builds": {n: {"bin": b, "env": e} for n, (b, e) in builds.items()},
               "summary": summary, "runs": results, "failures": failures, "total_s": total_s},
              open(base + ".json", "w"), indent=1)
    open(base + ".md", "w").write(md)
    print(f"wrote {base}.json and .md; sessions in {outdir}")


if __name__ == "__main__":
    main()
