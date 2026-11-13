"""Compare a trace against a golden. Returns human-readable differences; empty means a match.

The rules encode what is allowed to differ between two correct builds (or two runs of one):
timing jitter on positions, the lossless container chosen for bit-perfect output, the exact
frame a skip lands on. Everything else must match exactly.
"""

TOLERANCE = {"position_s": 1.5, "positionSeconds": 1.5, "volume": 0.02}
AUDIO_EDGE_S = 1.0      # a partial play may start/stop this much earlier or later
AUDIO_GAP_S = 0.25      # silence between tracks may differ by this much (gapless matters)
IGNORED = {"last"}      # diagnostic payloads (e.g. why a wait_for timed out)
ALLOWED_EXTRA: set[str] = set()  # waived extra controls, set by the CLI from waivers.toml
# Controls that only show while a modal overlay is open (waivers.toml [modal_overlays]): the
# candidate's modals make the page behind them inert, which takes it out of the accessibility
# tree, where the reference's DOM keeps it. With one of these in a snapshot, controls missing
# from it are waived; extras still count.
MODAL_MARKERS: set[str] = set()


def modal_open(controls) -> bool:
    return any(str(c) in MODAL_MARKERS for c in controls)


def diff_values(g, a, path, out):
    if isinstance(g, dict) and isinstance(a, dict):
        for k in sorted(set(g) | set(a)):
            if k in IGNORED:
                continue
            if k not in a:
                out.append(f"{path}.{k}: missing (expected {short(g[k])})")
            elif k not in g:
                if a[k] in ([], None, {}):
                    continue  # a field newer than the golden, with nothing in it
                out.append(f"{path}.{k}: unexpected {short(a[k])}")
            else:
                diff_values(g[k], a[k], f"{path}.{k}", out)
        return
    if isinstance(g, list) and isinstance(a, list) and path.endswith((".net", ".controls")):
        missing, extra = sorted(set(map(str, g)) - set(map(str, a))), sorted(set(map(str, a)) - set(map(str, g)))
        if path.endswith(".controls"):
            extra = [e for e in extra if e not in ALLOWED_EXTRA]
            if modal_open(a):
                missing = []
        for m in missing:
            out.append(f"{path}: missing {m}")
        for e in extra:
            out.append(f"{path}: extra {e}")
        return
    if isinstance(g, list) and isinstance(a, list):
        if len(g) != len(a):
            out.append(f"{path}: expected {short(g)}, got {short(a)}")
            return
        for i, (x, y) in enumerate(zip(g, a)):
            diff_values(x, y, f"{path}[{i}]", out)
        return
    key = path.rsplit(".", 1)[-1]
    if key == "unnamed_controls" and isinstance(g, int) and isinstance(a, int):
        if a > g + 2:  # fewer is an improvement; popovers make the count wobble by one or two
            out.append(f"{path}: {a} unnamed controls, the reference has {g}")
        return
    if key in TOLERANCE and isinstance(g, (int, float)) and isinstance(a, (int, float)):
        if abs(g - a) > TOLERANCE[key]:
            out.append(f"{path}: expected {g} ±{TOLERANCE[key]}, got {a}")
        return
    if g != a:
        out.append(f"{path}: expected {short(g)}, got {short(a)}")


def diff_audio(g: dict, a: dict, out):
    if not g:
        return  # scenario opted out of audio comparison
    gs, as_ = g.get("segments", []), a.get("segments", [])
    seq = lambda segs: [(s["track"], "full" if s["full"] else "partial") for s in segs]
    if seq(gs) != seq(as_):
        out.append(f"audio: played {seq(as_)}, expected {seq(gs)}")
        return
    for i, (x, y) in enumerate(zip(gs, as_)):
        r = x["rate"]
        if x["rate"] != y["rate"]:
            out.append(f"audio[{i}] track {x['track']}: rate {y['rate']}, expected {x['rate']} (resampled?)")
        if not x["full"]:
            for edge in ("start_frame", "end_frame"):
                if abs(x[edge] - y[edge]) > AUDIO_EDGE_S * r:
                    out.append(f"audio[{i}] track {x['track']}: {edge} {y[edge] / r:.2f}s, expected {x[edge] / r:.2f}s ±{AUDIO_EDGE_S}")
        gb, ab = x.get("silence_before_frames"), y.get("silence_before_frames")
        if gb is not None and ab is not None and abs(gb - ab) > AUDIO_GAP_S * r:
            out.append(f"audio[{i}] track {x['track']}: {ab / r:.3f}s silence before, expected {gb / r:.3f}s ±{AUDIO_GAP_S}")
    if not g.get("unmatched_nonsilent_bytes") and a.get("unmatched_nonsilent_bytes"):
        out.append(f"audio: {a['unmatched_nonsilent_bytes']} bytes of non-silent audio match no fixture (not bit-perfect)")


def strip_ignored(snap, ignore) -> dict:
    """An initial/final snapshot without the ignored keys: at its top level and in its `state`,
    which has the same shape as a step's state (initial.state.controls is the start-up
    inventory), so `ignore` means the same thing there as it does for steps."""
    out = {k: v for k, v in (snap or {}).items() if k not in ignore}
    if isinstance(out.get("state"), dict):
        out["state"] = {k: v for k, v in out["state"].items() if k not in ignore}
    return out


def compare(golden: dict, trace: dict, ignore: set[str] = frozenset()) -> list[str]:
    """`ignore` drops whole parts of the comparison by key, e.g. {"controls", "unnamed_controls"}
    to judge behaviour (L2) without the accessibility inventory (L3) for a partial port."""
    out = []
    if trace.get("errors"):
        out += [f"run error: {e}" for e in trace["errors"]]
        return out
    gsteps, tsteps = golden.get("steps", []), trace.get("steps", [])
    if len(gsteps) != len(tsteps):
        out.append(f"steps: expected {len(gsteps)}, got {len(tsteps)}")
    for g, t in zip(gsteps, tsteps):
        p = f"step {g['i']}" + (f" ({g['label']})" if g.get("label") else "")
        for key in ("error", "result", "state", "state_error", "net"):
            if key in g or key in t:
                gv, tv = g.get(key), t.get(key)
                if key == "state" and ignore:
                    gv = {k: v for k, v in (gv or {}).items() if k not in ignore}
                    tv = {k: v for k, v in (tv or {}).items() if k not in ignore}
                diff_values(gv, tv, f"{p}.{key}", out)
    for part in ("initial", "final"):
        diff_values(strip_ignored(golden.get(part), ignore), strip_ignored(trace.get(part), ignore), part, out)
    diff_audio(golden.get("audio", {}), trace.get("audio", {}), out)
    return out


def short(v, n=140):
    s = repr(v)
    return s if len(s) <= n else s[:n] + "…"


def _prf(golden: set, got: set) -> tuple[float, float]:
    got = {x for x in got if x in golden or x not in ALLOWED_EXTRA}
    if modal_open(got):
        golden = golden & got  # the page behind an inert modal is waived (MODAL_MARKERS)
    if not golden and not got:
        return 1.0, 1.0
    inter = len(golden & got)
    recall = inter / len(golden) if golden else 1.0
    precision = inter / len(got) if got else (1.0 if not golden else 0.0)
    return recall, precision


def score(golden: dict, trace: dict) -> dict:
    """How close a trace is to its golden, as numbers to climb (1.0 = identical).

    controls: recall/precision of the reference's visible named controls, averaged over every
      point the golden snapshotted them (per step in view scenarios, plus initial/final). Recall
      is "how much of the reference UI is there"; precision is "how much extra/renamed UI".
    net: F1 of the API calls per step against the reference's (missing and extra requests both cost).
    behaviour: share of steps whose non-UI state (MPRIS, MCP, queue, results, errors) matches.
    audio: 1 if the audio comparison passes, else 0 (when the scenario compares audio).
    total: mean of the parts present.
    """
    if trace.get("errors"):
        return {"total": 0.0, "error": trace["errors"][0]}
    ctl_r, ctl_p, net_r, beh = [], [], [], []
    gsteps, tsteps = golden.get("steps", []), trace.get("steps", [])
    for i, g in enumerate(gsteps):
        t = tsteps[i] if i < len(tsteps) else {}
        gs, ts = g.get("state") or {}, t.get("state") or {}
        if "controls" in gs:
            r, p = _prf(set(gs["controls"]), set(ts.get("controls") or []))
            ctl_r.append(r)
            ctl_p.append(p)
        if g.get("net") or t.get("net"):
            # requests both missed and extra count: F1 of the step's request set
            r, p = _prf(set(g.get("net") or []), set(t.get("net") or []))
            net_r.append(0.0 if r + p == 0 else 2 * r * p / (r + p))
        d: list[str] = []
        for key in ("error", "result", "state_error"):
            if key in g or key in t:
                diff_values(g.get(key), t.get(key), "x", d)
        diff_values({k: v for k, v in gs.items() if k != "controls"},
                    {k: v for k, v in ts.items() if k != "controls"}, "x", d)
        beh.append(0.0 if d else 1.0)
    for part in ("initial", "final"):
        gc, tc = (golden.get(part) or {}).get("controls"), (trace.get(part) or {}).get("controls")
        if gc is not None:
            r, p = _prf(set(gc), set(tc or []))
            ctl_r.append(r)
            ctl_p.append(p)
    mean = lambda xs: round(sum(xs) / len(xs), 3) if xs else None
    out = {"controls_recall": mean(ctl_r), "controls_precision": mean(ctl_p), "net": mean(net_r), "behaviour": mean(beh)}
    if golden.get("audio"):
        a: list[str] = []
        diff_audio(golden["audio"], trace.get("audio", {}), a)
        out["audio"] = 0.0 if a else 1.0
    parts = [v for v in out.values() if v is not None]
    out["total"] = round(sum(parts) / len(parts), 3) if parts else 0.0
    return out


def load_waivers(path: str) -> set[str]:
    """Allowed extra controls from verifier/waivers.toml (see that file for the reasons); also
    fills MODAL_MARKERS from its [modal_overlays]."""
    import tomllib
    try:
        w = tomllib.load(open(path, "rb"))
    except FileNotFoundError:
        return set()
    MODAL_MARKERS.update(w.get("modal_overlays", {}).get("markers", []))
    return set(w.get("extra_controls", {}).get("allow", []))
