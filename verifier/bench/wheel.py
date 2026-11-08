#!/usr/bin/env python3
"""A persistent virtual mouse wheel (wlr-virtual-pointer) for the headless compositor.

Reads one integer per line on stdin: +1 scrolls one wheel notch down, -1 one notch up. The
pointer stays alive for the whole run, so the app keeps one wl_pointer and sees every notch
(one-shot tools create and destroy a device per call, and the axis events get lost).
"""
import os
import sys
import tempfile
import time

XML = os.path.join(os.environ["SPIKE_WLR_PROTOCOLS"], "unstable", "wlr-virtual-pointer-unstable-v1.xml")
# Generate the bindings with the scanner's API (its command line wants pkg-config), pointing
# their core-protocol imports at pywayland's own module.
from pywayland.scanner import Protocol  # noqa: E402

gen = tempfile.mkdtemp(prefix="wheel-")
proto = Protocol.parse_file(XML)
proto.output(gen, {"wl_seat": "wayland", "wl_output": "wayland", **{i.name: proto.name for i in proto.interface}})
mod = os.path.join(gen, proto.name + ".py")
src = open(mod).read().replace("from .wayland import", "from pywayland.protocol.wayland import")
open(mod, "w").write(src)
sys.path.insert(0, gen)
from pywayland.client import Display  # noqa: E402
from pywayland.protocol.wayland import WlSeat  # noqa: E402
from wlr_virtual_pointer_unstable_v1 import ZwlrVirtualPointerManagerV1  # noqa: E402

AXIS_VERTICAL, SOURCE_WHEEL, NOTCH = 0, 0, 15.0

display = Display()
display.connect()
found = {}


def on_global(registry, name, interface, version):
    if interface == "wl_seat":
        found["seat"] = registry.bind(name, WlSeat, 1)
    elif interface == "zwlr_virtual_pointer_manager_v1":
        found["mgr"] = registry.bind(name, ZwlrVirtualPointerManagerV1, 1)


registry = display.get_registry()
registry.dispatcher["global"] = on_global
display.roundtrip()
ptr = found["mgr"].create_virtual_pointer(found["seat"])


def ms() -> int:
    return int(time.monotonic() * 1000) & 0xFFFFFFFF


ptr.motion(ms(), 1.0, 0.0)  # enter the surface under the cursor
ptr.frame()
display.flush()
for line in sys.stdin:
    try:
        n = int(line)
    except ValueError:
        continue
    t = ms()
    ptr.axis_source(SOURCE_WHEEL)
    ptr.axis_discrete(t, AXIS_VERTICAL, NOTCH * n, n)
    ptr.frame()
    display.flush()
os._exit(0)  # skip pywayland teardown (it can crash at interpreter exit)
