#!/usr/bin/env python3
"""Execute one scenario against a running SONE, from inside its sandbox. Writes a trace.

    runner.py SCENARIO.toml TRACE.json

Every step does one action through a channel (MCP, MPRIS, tray, AT-SPI), lets the app settle,
then records what every channel shows. Nothing here knows which build it is driving.

Step keys (exactly one action per step):
    mcp     = { tool = "play_tracks", args = { ... } }
    mpris   = { call = "Next" }  |  { call = "Seek", args = [2000000] }  |  { set = "Shuffle", value = true }
    tray    = "Next Track"
    ui      = { press = "Explore", role = "button" }
    wait    = 2.5
    wait_for = { "mpris.track.title" = "Queue B", timeout = 12 }
Optional per step: label, settle (seconds, default 1.0), observe (true, false, or a list of
state paths such as ["mpris.shuffle", "net"]).
Scenario keys: name, description, covers, tracks (fixtures the audio channel looks for),
audio (false to skip audio comparison), profile (settings.json overrides),
quarantine (reason; record/check skip the scenario and say why), views (true: also snapshot the
visible controls after every step, for per-view L3 comparison and scoring), settings (list of
settings.json keys to record after every step).
UI step forms: ui = { press = NAME [, role] } | ui = { type = TEXT, into = NAME [, role] };
key = "Return" presses one key.
"""
import json
import os
import sys
import time
import tomllib
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from channels import atspi, mcp, mpris, net, tray  # noqa: E402
from harness import seed_profile  # noqa: E402

MCP_URL = os.environ.get("MCP_URL", "http://127.0.0.1:5577/verifier/mcp")
RUN = os.environ["RUN"]


def dig(obj, path: str):
    for part in path.split("."):
        if isinstance(obj, dict):
            obj = obj.get(part)
        elif isinstance(obj, list) and part.isdigit():
            obj = obj[int(part)] if int(part) < len(obj) else None
        else:
            return None
    return obj


class Session:
    def __init__(self, view_mode: bool = False, settings_keys=()):
        self.view_mode = view_mode
        self.settings_keys = list(settings_keys)
        self.mcp = mcp.wait_ready(MCP_URL)
        self.mpris = mpris.wait_ready()
        atspi.wait_ready()
        self.flows = net.FlowLog(os.path.join(RUN, "logs", "flows.jsonl"))

    def state(self) -> dict:
        s = {"mpris": self.mpris.snapshot()}
        np_ = self.mcp.call("get_now_playing")
        s["now_playing"] = np_.get("nowPlaying") if isinstance(np_, dict) else np_
        q = self.mcp.call("get_queue", {"limit": 50})
        s["queue"] = [t.get("id") for t in q.get("queue", [])] if isinstance(q, dict) else q
        if self.view_mode:
            # What's on screen: the named controls, headings and landmarks (L3 per view).
            s["controls"] = atspi.controls()
        if self.settings_keys:
            # What the app saved: chosen settings.json keys (decrypted with the profile key).
            try:
                saved = seed_profile.dump(Path(os.environ["XDG_CONFIG_HOME"]) / "sone")
                s["settings"] = {k: saved.get(k) for k in self.settings_keys}
            except Exception as e:  # noqa: BLE001
                s["settings"] = {"error": str(e)[:80]}
        return s

    def act(self, step: dict):
        if "mcp" in step:
            return self.mcp.call(step["mcp"]["tool"], step["mcp"].get("args", {}))
        if "mpris" in step:
            m = step["mpris"]
            if "set" in m:
                self.mpris.set(m["set"], m["value"])
                return None
            self.mpris.call(m["call"], *m.get("args", []))
            return None
        if "tray" in step:
            return tray.click(step["tray"])
        if "ui" in step:
            u = step["ui"]
            if "press" in u:
                return atspi.press(u["press"], u.get("role"), u.get("nth", 0))
            if "type" in u:  # focus a named field, then type real keystrokes
                atspi.focus(u["into"], u.get("role"), u.get("nth", 0))
                time.sleep(0.2)
                atspi.type_text(u["type"])
                return None
            raise ValueError(f"unknown ui action: {u}")
        if "key" in step:
            atspi.key(step["key"])
            return None
        if "wait" in step:
            time.sleep(float(step["wait"]))
            return None
        if "wait_for" in step:
            cond = dict(step["wait_for"])
            timeout = float(cond.pop("timeout", 15))
            deadline = time.time() + timeout
            while True:
                st = self.state()
                unmet = {k: dig(st, k) for k, v in cond.items() if dig(st, k) != v}
                if not unmet:
                    return {"met": True}
                if time.time() > deadline:
                    return {"met": False, "last": unmet}
                time.sleep(0.25)
        raise ValueError(f"step has no action: {step}")


def main():
    scenario = tomllib.load(open(sys.argv[1], "rb"))
    out = {"scenario": scenario["name"], "steps": [], "errors": []}
    try:
        s = Session(view_mode=bool(scenario.get("views", False)), settings_keys=scenario.get("settings", []))
    except Exception as e:  # noqa: BLE001
        out["errors"].append(f"startup: {e}")
        json.dump(out, open(sys.argv[2], "w"), indent=1)
        return 1
    # Startup loads (library, favorites, home) arrive at unpredictable times. Wait for the
    # network to go quiet so they aren't attributed to step 0.
    quiet, deadline = float(scenario.get("startup_quiet", 2.0)), time.time() + 30
    last, last_change = s.flows.mark(), time.time()
    while time.time() < deadline and time.time() - last_change < quiet:
        time.sleep(0.2)
        m = s.flows.mark()
        if m != last:
            last, last_change = m, time.time()
    out["initial"] = {"state": s.state(), "tray": tray.menu_labels(), "controls": atspi.controls()}
    for i, step in enumerate(scenario.get("step", [])):
        mark = s.flows.mark()
        rec = {"i": i, "label": step.get("label", ""),
               "action": {k: v for k, v in step.items() if k not in ("label", "settle", "observe")}}
        try:
            rec["result"] = s.act(step)
        except Exception as e:  # noqa: BLE001 - a failing action is a finding, not a crash
            rec["error"] = f"{type(e).__name__}: {e}"
        if "wait" not in step and "wait_for" not in step:
            time.sleep(float(step.get("settle", 1.0)))
        # Let requests the step caused land before cutting its slice of the network log, so a
        # fetch that starts just as a wait_for is satisfied isn't attributed to the next step.
        quiet_deadline, last, last_change = time.time() + 2.0, s.flows.mark(), time.time()
        while time.time() < quiet_deadline and time.time() - last_change < 0.4:
            time.sleep(0.1)
            m2 = s.flows.mark()
            if m2 != last:
                last, last_change = m2, time.time()
        observe = step.get("observe", True)
        if observe:
            try:
                st = s.state()
                # `observe = ["mpris.shuffle", ...]` keeps only those paths (for steps whose
                # other state is legitimately random, e.g. a shuffled queue)
                rec["state"] = st if observe is True else {p: dig(st, p) for p in observe}
            except Exception as e:  # noqa: BLE001
                rec["state_error"] = str(e)
            if observe is True or "net" in observe:
                rec["net"] = net.calls(s.flows.since(mark))
        out["steps"].append(rec)
    opened = os.path.join(RUN, "logs", "opened.txt")
    out["final"] = {"tray": tray.menu_labels(), "controls": atspi.controls(),
                    "unnamed_controls": atspi.unnamed_controls(),
                    # URLs the app tried to open in a browser (xdg-open is stubbed in the sandbox)
                    "opened": open(opened).read().split() if os.path.exists(opened) else []}
    json.dump(out, open(sys.argv[2], "w"), indent=1, default=str)
    return 0


if __name__ == "__main__":
    sys.exit(main())
