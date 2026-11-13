#!/usr/bin/env python3
"""A plain xdg-shell window for the headless compositor: a solid grey toplevel (app_id
"bench-cover") that takes keyboard focus when it maps. bench/states_driver.py uses it to
unfocus the app (floating, small, beside it) or to cover it (fullscreen over it).

Prints "mapped" once its first buffer is committed, then serves the connection (configure,
ping) until stdin closes.
"""
import mmap
import os
import sys
import tempfile
import threading

XML = os.path.join(os.environ["SPIKE_WAYLAND_PROTOCOLS"], "stable", "xdg-shell", "xdg-shell.xml")
from pywayland.scanner import Protocol  # noqa: E402

gen = tempfile.mkdtemp(prefix="cover-")
proto = Protocol.parse_file(XML)
proto.output(gen, {"wl_seat": "wayland", "wl_output": "wayland", "wl_surface": "wayland",
                   **{i.name: proto.name for i in proto.interface}})
mod = os.path.join(gen, proto.name + ".py")
src = open(mod).read().replace("from .wayland import", "from pywayland.protocol.wayland import")
open(mod, "w").write(src)
sys.path.insert(0, gen)
from pywayland.client import Display  # noqa: E402
from pywayland.protocol.wayland import WlCompositor, WlShm  # noqa: E402
from xdg_shell import XdgWmBase  # noqa: E402

display = Display()
display.connect()
g = {}


def on_global(registry, name, interface, version):
    if interface == "wl_compositor":
        g["comp"] = registry.bind(name, WlCompositor, 4)
    elif interface == "wl_shm":
        g["shm"] = registry.bind(name, WlShm, 1)
    elif interface == "xdg_wm_base":
        g["wm"] = registry.bind(name, XdgWmBase, 1)


registry = display.get_registry()
registry.dispatcher["global"] = on_global
display.roundtrip()
g["wm"].dispatcher["ping"] = lambda wm, serial: wm.pong(serial)
surface = g["comp"].create_surface()
xs = g["wm"].get_xdg_surface(surface)
top = xs.get_toplevel()
top.set_app_id("bench-cover")
top.set_title("bench cover")
size = {"w": 0, "h": 0}
state = {"buf": None, "mapped": False, "keep": []}


def draw(w: int, h: int):
    w, h = max(w, 64), max(h, 64)
    stride, n = w * 4, w * h * 4
    fd = os.memfd_create("cover")
    os.ftruncate(fd, n)
    mm = mmap.mmap(fd, n)
    mm.write(b"\x40\x40\x40\xff" * (w * h))
    pool = g["shm"].create_pool(fd, n)
    buf = pool.create_buffer(0, w, h, stride, WlShm.format.xrgb8888.value)
    pool.destroy()
    os.close(fd)
    state["keep"] = [mm, buf]  # alive until the next configure replaces them
    surface.attach(buf, 0, 0)
    surface.damage(0, 0, w, h)
    surface.commit()


def on_top_configure(t, w, h, states):
    size["w"], size["h"] = w, h


def on_configure(x, serial):
    x.ack_configure(serial)
    draw(size["w"] or 400, size["h"] or 300)
    if not state["mapped"]:
        state["mapped"] = True
        display.flush()
        print("mapped", flush=True)  # committed; the driver confirms the map in sway's tree


top.dispatcher["configure"] = on_top_configure
xs.dispatcher["configure"] = on_configure
surface.commit()
display.flush()


def watch_stdin():
    sys.stdin.read()  # until EOF
    os._exit(0)


threading.Thread(target=watch_stdin, daemon=True).start()
while display.dispatch(block=True) != -1:
    pass
os._exit(0)
