#!/usr/bin/env python3
"""SONE port verifier: run scenarios against a build in hermetic sandboxes.

    verify.py record --bin REFERENCE_BIN [SCENARIO ...]    record goldens from the reference
    verify.py check  --bin BUILD_BIN [SCENARIO ...]        compare a build against the goldens
    verify.py run    --bin BUILD_BIN SCENARIO              run once, print the trace summary

Options: --jobs N (parallel sandboxes, default 4), --runs N (repeat each scenario; any run
that differs fails it), --out DIR (default verifier/runs/<timestamp>).
Run inside `nix-shell verifier/harness/shell.nix`. Scenarios are names in verifier/scenarios/.
"""
import argparse
import concurrent.futures as cf
import json
import os
import subprocess
import sys
import time
import tomllib

ROOT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ROOT)
from channels import audio  # noqa: E402
from harness import compare as compare_mod  # noqa: E402
from harness.compare import compare, score as score_trace  # noqa: E402
compare_mod.ALLOWED_EXTRA |= compare_mod.load_waivers(os.path.join(ROOT, "waivers.toml"))

MCP_TOKEN = "verifier"
BASE_PROFILE = {
    # A logged-in user: tokens are fake, the fake TIDAL accepts anything.
    "auth_tokens": {"access_token": "fake-access", "refresh_token": "fake-refresh",
                    "expires_in": 86400, "token_type": "Bearer", "user_id": 424242},
    "mcp_enabled": True, "mcp_port": 5577, "mcp_token": MCP_TOKEN,
    "proxy": {"enabled": True, "proxy_type": "http", "host": "127.0.0.1", "port": 18080},
    # Bit-perfect exclusive output into the capture device, so audio is comparable.
    "exclusive_mode": True, "exclusive_device": "sone_capture", "bit_perfect": True,
}


def scenario_paths(names):
    d = os.path.join(ROOT, "scenarios")
    if not names:
        return sorted(os.path.join(d, f) for f in os.listdir(d) if f.endswith(".toml"))
    return [n if n.endswith(".toml") and os.path.exists(n) else os.path.join(d, f"{n}.toml") for n in names]


def run_one(path: str, binary: str, rundir: str, override: dict | None = None, timeout: int = 240, display: str = "1400x900") -> dict:
    sc = tomllib.load(open(path, "rb"))
    profile = {**BASE_PROFILE, **sc.get("profile", {}), **(override or {})}
    if sc.get("logged_out"):
        profile.pop("auth_tokens", None)
    env = {**os.environ, "SPIKE_SEED": json.dumps(profile), "SPIKE_OUTPUT": display,
           "MCP_URL": f"http://127.0.0.1:5577/{MCP_TOKEN}/mcp"}
    trace_path = os.path.join(rundir, "trace.json")
    cmd = [os.path.join(ROOT, "harness", "sandbox.sh"), rundir, binary, "--",
           "python3", "-I", os.path.join(ROOT, "harness", "runner.py"), path, trace_path]
    t0 = time.time()
    try:
        p = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=timeout)
        tail = (p.stdout + p.stderr)[-2000:]
    except subprocess.TimeoutExpired as e:
        tail = f"timeout after {timeout}s"
    if os.path.exists(trace_path):
        trace = json.load(open(trace_path))
    else:
        trace = {"scenario": sc["name"], "steps": [], "errors": [f"no trace; sandbox said: {tail}"]}
    if sc.get("audio", True):
        idx = audio.Index(os.path.join(ROOT, "fixtures"), sc.get("tracks"))
        trace["audio"] = audio.analyze(os.path.join(rundir, "audio.raw"), idx)
    else:
        trace["audio"] = {}
    trace["_meta"] = {"seconds": round(time.time() - t0, 1), "rundir": rundir}
    json.dump(trace, open(trace_path, "w"), indent=1)
    return trace


def unmet(trace: dict) -> list[str]:
    bad = [f"step {s['i']}: {s['error']}" for s in trace["steps"] if s.get("error")]
    bad += [f"step {s['i']}: wait_for not met {s['result'].get('last')}" for s in trace["steps"]
            if isinstance(s.get("result"), dict) and s["result"].get("met") is False]
    return trace.get("errors", []) + bad


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["record", "check", "run", "score"])
    ap.add_argument("--save", help="score: store the scores as verifier/scores/NAME.json")
    ap.add_argument("--baseline", help="score: compare against verifier/scores/NAME.json")
    ap.add_argument("scenarios", nargs="*")
    ap.add_argument("--bin", required=True)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--out", default=os.path.join(ROOT, "runs", time.strftime("%Y%m%d-%H%M%S")))
    ap.add_argument("--display", default="1400x900", help="headless display size WxH (goldens are recorded at 1400x900; Rune builds need a tall one, e.g. 1400x2000, because AccessKit hides off-screen content)")
    ap.add_argument("--ignore", default="", help="comma-separated trace keys to leave out of the comparison, e.g. controls,unnamed_controls (L3) for a partial port")
    ap.add_argument("--profile", default="{}", help="JSON merged over every scenario profile (experiments, mutation checks)")
    a = ap.parse_args()
    paths = []
    for p in scenario_paths(a.scenarios):
        q = tomllib.load(open(p, "rb")).get("quarantine")
        if q and a.mode != "run":
            print(f"QUARANTINED   {os.path.basename(p)[:-5]}: {q}")
        else:
            paths.append(p)
    os.makedirs(a.out, exist_ok=True)
    jobs = [(p, r) for p in paths for r in range(a.runs)]
    results: dict[str, list[dict]] = {}
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(run_one, p, os.path.abspath(a.bin),
                          os.path.join(a.out, os.path.basename(p)[:-5], f"run{r}"),
                          json.loads(a.profile), 240, a.display): (p, r) for p, r in jobs}
        for f in cf.as_completed(futs):
            p, r = futs[f]
            results.setdefault(p, []).append(f.result())

    failed, report, missing_count, extra_count = 0, {}, {}, {}
    for p in paths:
        name = os.path.basename(p)[:-5]
        traces = results[p]
        golden_path = os.path.join(ROOT, "goldens", f"{name}.json")
        if a.mode == "record":
            problems = sum((unmet(t) for t in traces), [])
            # a golden must be reproducible: every run of the reference must agree
            problems += [f"run{i} differs from run0: {d}" for i, t in enumerate(traces[1:], 1)
                         for d in compare(traces[0], t)]
            if problems:
                failed += 1
                print(f"NOT RECORDED  {name}")
                for d in problems[:15]:
                    print(f"    {d}")
            else:
                g = {k: v for k, v in traces[0].items() if k != "_meta"}
                json.dump(g, open(golden_path, "w"), indent=1)
                print(f"RECORDED      {name}  ({traces[0]['_meta']['seconds']}s)  audio: "
                      + ("; ".join(audio.summary(g["audio"])) if g["audio"] else "not compared"))
            report[name] = problems
        elif a.mode == "check":
            if not os.path.exists(golden_path):
                print(f"NO GOLDEN     {name}")
                failed += 1
                continue
            golden = json.load(open(golden_path))
            diffs = [f"run{i}: {d}" if len(traces) > 1 else d
                     for i, t in enumerate(traces) for d in compare(golden, t, set(filter(None, a.ignore.split(","))))]
            print(f"{'PASS' if not diffs else 'FAIL':13} {name}"
                  + "".join(f"\n    {d}" for d in diffs[:25]) + (f"\n    … {len(diffs) - 25} more" if len(diffs) > 25 else ""))
            failed += bool(diffs)
            report[name] = diffs
        elif a.mode == "score":
            if not os.path.exists(golden_path):
                print(f"NO GOLDEN     {name}")
                continue
            golden = json.load(open(golden_path))
            runs = [score_trace(golden, t) for t in traces]
            # the worst run counts: a climb must hold on every run
            report[name] = min(runs, key=lambda s: s["total"])
            for t in traces:
                for missing in _missing_controls(golden, t):
                    missing_count[missing] = missing_count.get(missing, 0) + 1
                for extra in _missing_controls(t, golden):
                    if extra not in compare_mod.ALLOWED_EXTRA:
                        extra_count[extra] = extra_count.get(extra, 0) + 1
        else:
            t = traces[0]
            print(json.dumps({k: v for k, v in t.items() if k not in ("initial", "final")}, indent=1)[:6000])
            print("audio:", audio.summary(t["audio"]))
            print("problems:", unmet(t))
    json.dump(report, open(os.path.join(a.out, "report.json"), "w"), indent=1)
    if a.mode == "score":
        print_scores(report, missing_count, a.baseline, a.save, extra_count)
        return 0
    if a.mode != "run":
        print(f"\n{len(paths) - failed}/{len(paths)} {'recorded' if a.mode == 'record' else 'passed'}  ({a.out})")
    return 1 if failed else 0


def _missing_controls(golden: dict, trace: dict) -> list[str]:
    """Reference controls absent from the trace, at every point the golden snapshotted them."""
    out = []
    points = [((g.get("state") or {}).get("controls"), ((t.get("state") or {}) if t else {}).get("controls"))
              for g, t in zip(golden.get("steps", []), trace.get("steps", []) + [None] * len(golden.get("steps", [])))]
    points += [((golden.get(p) or {}).get("controls"), (trace.get(p) or {}).get("controls")) for p in ("initial", "final")]
    for gc, tc in points:
        if gc is not None:
            out += sorted(set(gc) - set(tc or []))
    return out


def print_scores(report: dict, missing: dict, baseline: str | None, save: str | None, extra: dict | None = None):
    """The hill-climbing readout: per-scenario scores, the overall number, deltas against a
    saved baseline, and the reference controls most often missing (what to build next)."""
    sdir = os.path.join(ROOT, "scores")
    base = json.load(open(os.path.join(sdir, f"{baseline}.json"))) if baseline else {}
    cols = ("total", "controls_recall", "controls_precision", "net", "behaviour", "audio")
    print(f"{'scenario':26}" + "".join(f"{c[:10]:>11}" for c in cols))
    for name, s in sorted(report.items()):
        cells = []
        for c in cols:
            v = s.get(c)
            if v is None:
                cells.append(f"{'-':>11}")
                continue
            b = (base.get("scenarios", {}).get(name) or {}).get(c)
            delta = f"{v - b:+.2f}" if isinstance(b, (int, float)) and abs(v - b) >= 0.005 else ""
            cells.append(f"{v:>6.2f}{delta:>5}")
        print(f"{name:26}" + "".join(cells) + (f"  ({s['error'][:60]})" if s.get("error") else ""))
    overall = round(sum(s["total"] for s in report.values()) / len(report), 3) if report else 0.0
    b_overall = base.get("overall")
    print(f"\noverall {overall:.3f}" + (f"  (baseline {b_overall:.3f}, {overall - b_overall:+.3f})" if isinstance(b_overall, (int, float)) else ""))
    if missing:
        print("\nmost-missed reference controls (build these next):")
        for k, n in sorted(missing.items(), key=lambda kv: (-kv[1], kv[0]))[:25]:
            print(f"  {n:4}  {k}")
    if extra:
        print("\nmost-common extra controls (the reference has no such control there):")
        for k, n in sorted(extra.items(), key=lambda kv: (-kv[1], kv[0]))[:15]:
            print(f"  {n:4}  {k}")
    if save:
        os.makedirs(sdir, exist_ok=True)
        json.dump({"overall": overall, "scenarios": report, "missing": missing}, open(os.path.join(sdir, f"{save}.json"), "w"), indent=1)
        print(f"\nsaved verifier/scores/{save}.json")


if __name__ == "__main__":
    sys.exit(main())
