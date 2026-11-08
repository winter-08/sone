#!/usr/bin/env python3
"""Tray channel: a stand-in StatusNotifierWatcher, plus a reader for the app's tray menu.

    tray.py watcher          serve org.kde.StatusNotifierWatcher on the session bus (blocks)
    tray.py menu             print every registered item's dbusmenu (id, label, enabled)
    tray.py click LABEL      send a "clicked" event to the first menu item with that label
"""
import sys

import dbus
import dbus.service

WATCHER = "org.kde.StatusNotifierWatcher"
WPATH = "/StatusNotifierWatcher"
PROPS = "org.freedesktop.DBus.Properties"


class Watcher(dbus.service.Object):
    def __init__(self, bus):
        self.items, self.bus = [], bus
        super().__init__(dbus.service.BusName(WATCHER, bus), WPATH)

    @dbus.service.method(WATCHER, in_signature="s", sender_keyword="sender")
    def RegisterStatusNotifierItem(self, service, sender=None):
        # Items pass either a bus name or an object path on their own connection.
        item = f"{sender}{service}" if service.startswith("/") else service
        if item not in self.items:
            self.items.append(item)
            print(f"registered item {item} (sender {sender})", flush=True)
            self.StatusNotifierItemRegistered(item)

    @dbus.service.method(WATCHER, in_signature="s")
    def RegisterStatusNotifierHost(self, service):
        pass

    @dbus.service.signal(WATCHER, signature="s")
    def StatusNotifierItemRegistered(self, item):
        pass

    @dbus.service.signal(WATCHER, signature="")
    def StatusNotifierHostRegistered(self):
        pass

    def _props(self):
        return {"RegisteredStatusNotifierItems": dbus.Array(self.items, signature="s"),
                "IsStatusNotifierHostRegistered": dbus.Boolean(True),
                "ProtocolVersion": dbus.Int32(0)}

    @dbus.service.method(PROPS, in_signature="ss", out_signature="v")
    def Get(self, iface, prop):
        return self._props()[prop]

    @dbus.service.method(PROPS, in_signature="s", out_signature="a{sv}")
    def GetAll(self, iface):
        return self._props()


def items(bus):
    w = bus.get_object(WATCHER, WPATH)
    for item in w.Get(WATCHER, "RegisteredStatusNotifierItems", dbus_interface=PROPS):
        name, _, path = str(item).partition("/")
        yield name, "/" + (path or "StatusNotifierItem")


def menus(bus):
    for name, path in items(bus):
        sni = bus.get_object(name, path)
        menu_path = sni.Get("org.kde.StatusNotifierItem", "Menu", dbus_interface=PROPS)
        title = sni.Get("org.kde.StatusNotifierItem", "Title", dbus_interface=PROPS)
        menu = dbus.Interface(bus.get_object(name, menu_path), "com.canonical.dbusmenu")
        _, layout = menu.GetLayout(0, -1, dbus.Array([], signature="s"))
        yield title, menu, layout


def flatten(layout, depth=0):
    mid, props, children = layout
    yield depth, int(mid), str(props.get("label", "")), bool(props.get("enabled", True)), \
        str(props.get("type", "standard"))
    for c in children:
        yield from flatten(c, depth + 1)


def menu_labels(bus=None) -> list[str]:
    bus = bus or dbus.SessionBus()
    out = []
    for _, _, layout in menus(bus):
        for _, _, label, enabled, typ in flatten(layout):
            if typ != "separator" and label:
                out.append(label if enabled else f"{label} (disabled)")
    return out


def click(label: str, bus=None) -> str:
    bus = bus or dbus.SessionBus()
    want = label.replace("_", "").lower()
    for _, menu, layout in menus(bus):
        for _, mid, lab, _, _ in flatten(layout):
            if lab.replace("_", "").lower() == want:
                menu.Event(mid, "clicked", dbus.String("", variant_level=1), dbus.UInt32(0))
                return lab
    raise LookupError(f"no tray menu item {label!r}")


def main():
    cmd = sys.argv[1]
    if cmd == "watcher":
        from dbus.mainloop.glib import DBusGMainLoop
        from gi.repository import GLib
        DBusGMainLoop(set_as_default=True)
        Watcher(dbus.SessionBus())
        print("watcher up", flush=True)
        GLib.MainLoop().run()
    bus = dbus.SessionBus()
    if cmd == "menu":
        for title, _, layout in menus(bus):
            print(f"# item: {title}")
            for depth, mid, label, enabled, typ in flatten(layout):
                if typ == "separator":
                    label = "---"
                print(f"{'  ' * depth}[{mid}] {label}{'' if enabled else ' (disabled)'}")
    elif cmd == "click":
        want = sys.argv[2].replace("_", "").lower()
        for _, menu, layout in menus(bus):
            for _, mid, label, _, _ in flatten(layout):
                if label.replace("_", "").lower() == want:
                    menu.Event(mid, "clicked", dbus.String("", variant_level=1), dbus.UInt32(0))
                    print(f"clicked [{mid}] {label}")
                    return
        raise SystemExit(f"no tray menu item {sys.argv[2]!r}")


if __name__ == "__main__":
    main()
