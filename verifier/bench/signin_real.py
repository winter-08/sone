#!/usr/bin/env python3
"""Sign a real TIDAL account in once, for real-account benchmarks (sandbox with SPIKE_NET=1).

Starts SONE's device-code sign-in and prints the verification link the app tried to open (the
sandbox's xdg-open records it); the account owner approves it in their own browser. Once the
app has stored tokens, copies the profile (sone.key + encrypted settings.json) to
$SPIKE_SAVE_PROFILE for SPIKE_PROFILE_DIR. Never prints tokens.
"""
import os
import shutil
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "harness"))
from channels import atspi  # noqa: E402
from seed_profile import dump  # noqa: E402
from pathlib import Path  # noqa: E402

RUN = os.environ["RUN"]
CFG = Path(os.environ["XDG_CONFIG_HOME"]) / "sone"
SAVE = os.environ["SPIKE_SAVE_PROFILE"]


def signed_in() -> bool:
    try:
        return bool(dump(CFG).get("auth_tokens"))
    except Exception:  # noqa: BLE001 - settings being rewritten
        return False


atspi.wait_ready(60)
time.sleep(2)
atspi.press("Login Code")
time.sleep(1.5)
atspi.press("Get Login Code")
opened = os.path.join(RUN, "logs", "opened.txt")
t0 = time.monotonic()
while not os.path.exists(opened) and time.monotonic() - t0 < 30:
    time.sleep(0.5)
print("SIGN-IN LINK:", open(opened).read().strip().splitlines()[-1], flush=True)
t0 = time.monotonic()
while not signed_in():
    if time.monotonic() - t0 > 900:
        print("TIMED OUT waiting for approval", flush=True)
        sys.exit(1)
    time.sleep(2)
time.sleep(5)  # let the app finish writing its settings
os.makedirs(SAVE, mode=0o700, exist_ok=True)
for f in ("sone.key", "settings.json"):
    shutil.copy2(CFG / f, os.path.join(SAVE, f))
    os.chmod(os.path.join(SAVE, f), 0o600)
print("SIGNED IN: profile saved", flush=True)
