#!/usr/bin/env python3
"""A persistent virtual keyboard (zwp_virtual_keyboard_v1) for the headless compositor.

Reads one key per line on stdin, an XKB keysym name ("Return", "Escape", "a") or a chord
("ctrl+a", "ctrl+shift+Tab"), presses it and answers "ok" once the compositor has it.

Why not wtype: wtype uploads a minimal keymap of just the keys it types and expresses
modifiers only as a modifier mask. Unpaced, its chords reach no app in the headless sway (the
device is gone before the events are delivered); paced, Rune selects all but WebKitGTK ignores
Ctrl+A. This keyboard uploads the full default (us) keymap compiled by libxkbcommon, presses the
real modifier keys (Control_L ...) as well as setting the mask, and stays alive for the run, the
way a physical keyboard behaves.
"""
import ctypes
import os
import sys
import tempfile
import time

XML = """<?xml version="1.0" encoding="UTF-8"?>
<protocol name="virtual_keyboard_unstable_v1">
  <interface name="zwp_virtual_keyboard_v1" version="1">
    <request name="keymap">
      <arg name="format" type="uint"/>
      <arg name="fd" type="fd"/>
      <arg name="size" type="uint"/>
    </request>
    <request name="key">
      <arg name="time" type="uint"/>
      <arg name="key" type="uint"/>
      <arg name="state" type="uint"/>
    </request>
    <request name="modifiers">
      <arg name="mods_depressed" type="uint"/>
      <arg name="mods_latched" type="uint"/>
      <arg name="mods_locked" type="uint"/>
      <arg name="group" type="uint"/>
    </request>
    <request name="destroy" type="destructor"/>
  </interface>
  <interface name="zwp_virtual_keyboard_manager_v1" version="1">
    <request name="create_virtual_keyboard">
      <arg name="seat" type="object" interface="wl_seat"/>
      <arg name="id" type="new_id" interface="zwp_virtual_keyboard_v1"/>
    </request>
  </interface>
</protocol>
"""

# ---- the keymap: libxkbcommon's default rules/model/layout (us), as the compositor would build
xkb = ctypes.CDLL(os.environ.get("SPIKE_XKBCOMMON", "libxkbcommon.so.0"))
xkb.xkb_context_new.restype = ctypes.c_void_p
xkb.xkb_keymap_new_from_names.restype = ctypes.c_void_p
xkb.xkb_keymap_new_from_names.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
xkb.xkb_keymap_get_as_string.restype = ctypes.c_void_p  # char*, freed by us (leaked: one per run)
xkb.xkb_keymap_get_as_string.argtypes = [ctypes.c_void_p, ctypes.c_int]
xkb.xkb_keymap_min_keycode.argtypes = xkb.xkb_keymap_max_keycode.argtypes = [ctypes.c_void_p]
xkb.xkb_keymap_key_get_syms_by_level.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32,
                                                 ctypes.c_uint32, ctypes.POINTER(ctypes.POINTER(ctypes.c_uint32))]
xkb.xkb_keymap_mod_get_index.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
xkb.xkb_keysym_from_name.restype = ctypes.c_uint32
xkb.xkb_keysym_from_name.argtypes = [ctypes.c_char_p, ctypes.c_int]

ctx = xkb.xkb_context_new(0)
keymap = xkb.xkb_keymap_new_from_names(ctx, None, 0)
if not keymap:
    sys.exit("keyboard.py: libxkbcommon couldn't compile the default keymap")
KEYMAP_TEXT = ctypes.string_at(xkb.xkb_keymap_get_as_string(keymap, 1))

# keysym -> (evdev code, level) over the first layout's levels 0 (plain) and 1 (shifted)
CODES: dict[int, tuple[int, int]] = {}
for kc in range(xkb.xkb_keymap_min_keycode(keymap), xkb.xkb_keymap_max_keycode(keymap) + 1):
    for level in (0, 1):
        syms = ctypes.POINTER(ctypes.c_uint32)()
        for i in range(xkb.xkb_keymap_key_get_syms_by_level(keymap, kc, 0, level, ctypes.byref(syms))):
            CODES.setdefault(syms[i], (kc - 8, level))


def code(name: str) -> tuple[int, int]:
    sym = xkb.xkb_keysym_from_name(name.encode(), 0)
    if sym == 0 or sym not in CODES:
        raise KeyError(f"no key for keysym {name!r} in the default keymap")
    return CODES[sym]


def mask(mod: str) -> int:
    i = xkb.xkb_keymap_mod_get_index(keymap, mod.encode())
    if i < 0 or i >= 32:
        raise KeyError(mod)
    return 1 << i


MODS = {  # chord prefix -> (key pressed, XKB modifier)
    "ctrl": ("Control_L", "Control"), "control": ("Control_L", "Control"),
    "shift": ("Shift_L", "Shift"), "alt": ("Alt_L", "Mod1"), "super": ("Super_L", "Mod4"),
    "logo": ("Super_L", "Mod4"),
}

# ---- bindings, generated from the XML above with pywayland's scanner (as bench/wheel.py does)
from pywayland.scanner import Protocol  # noqa: E402

gen = tempfile.mkdtemp(prefix="vkbd-")
xml_path = os.path.join(gen, "virtual-keyboard-unstable-v1.xml")
open(xml_path, "w").write(XML)
proto = Protocol.parse_file(xml_path)
proto.output(gen, {"wl_seat": "wayland", **{i.name: proto.name for i in proto.interface}})
mod_path = os.path.join(gen, proto.name + ".py")
src = open(mod_path).read().replace("from .wayland import", "from pywayland.protocol.wayland import")
open(mod_path, "w").write(src)
sys.path.insert(0, gen)
from pywayland.client import Display  # noqa: E402
from pywayland.protocol.wayland import WlSeat  # noqa: E402
from virtual_keyboard_unstable_v1 import ZwpVirtualKeyboardManagerV1  # noqa: E402

display = Display()
display.connect()
found = {}


def on_global(registry, name, interface, version):
    if interface == "wl_seat":
        found["seat"] = registry.bind(name, WlSeat, 1)
    elif interface == "zwp_virtual_keyboard_manager_v1":
        found["mgr"] = registry.bind(name, ZwpVirtualKeyboardManagerV1, 1)


registry = display.get_registry()
registry.dispatcher["global"] = on_global
display.roundtrip()
kbd = found["mgr"].create_virtual_keyboard(found["seat"])

fd = os.memfd_create("keymap")
os.write(fd, KEYMAP_TEXT + b"\0")
kbd.keymap(1, fd, len(KEYMAP_TEXT) + 1)  # WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1
display.roundtrip()
os.close(fd)

PAUSE = 0.03  # between events, so each lands in its own client dispatch as a human's would


def ms() -> int:
    return int(time.monotonic() * 1000) & 0xFFFFFFFF


def send(ev):
    ev()
    display.flush()
    time.sleep(PAUSE)


def chord(spec: str) -> None:
    *mods, k = spec.split("+")
    held = 0
    pressed = []
    for m in mods:
        keyname, modname = MODS[m.lower()]
        c, _ = code(keyname)
        held |= mask(modname)
        send(lambda c=c: kbd.key(ms(), c, 1))
        pressed.append(c)
        send(lambda h=held: kbd.modifiers(h, 0, 0, 0))
    c, level = code(k)
    if level == 1 and not held & mask("Shift"):  # a shifted symbol ("A", "?") on its own
        s, _ = code("Shift_L")
        held |= mask("Shift")
        send(lambda: kbd.key(ms(), s, 1))
        pressed.append(s)
        send(lambda h=held: kbd.modifiers(h, 0, 0, 0))
    send(lambda: kbd.key(ms(), c, 1))
    send(lambda: kbd.key(ms(), c, 0))
    for p in reversed(pressed):
        send(lambda p=p: kbd.key(ms(), p, 0))
    send(lambda: kbd.modifiers(0, 0, 0, 0))
    display.roundtrip()


time.sleep(0.15)  # let clients take the new keymap before the first key
print("ready", flush=True)
for line in sys.stdin:
    spec = line.strip()
    if not spec:
        continue
    try:
        chord(spec)
        print("ok", flush=True)
    except Exception as e:  # noqa: BLE001 - report, keep the keyboard alive
        print(f"error {e}", flush=True)
os._exit(0)  # skip pywayland teardown (it can crash at interpreter exit)
