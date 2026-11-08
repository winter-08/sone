"""MPRIS channel: org.mpris.MediaPlayer2 on the sandbox session bus."""
import time

import dbus

PLAYER = "org.mpris.MediaPlayer2.Player"
PROPS = "org.freedesktop.DBus.Properties"
PATH = "/org/mpris/MediaPlayer2"


def _plain(v):
    if isinstance(v, (dbus.Boolean,)):
        return bool(v)
    if isinstance(v, (dbus.Int16, dbus.Int32, dbus.Int64, dbus.UInt16, dbus.UInt32, dbus.UInt64, dbus.Byte)):
        return int(v)
    if isinstance(v, dbus.Double):
        return float(v)
    if isinstance(v, (dbus.String, dbus.ObjectPath)):
        return str(v)
    if isinstance(v, dbus.Array):
        return [_plain(x) for x in v]
    if isinstance(v, dbus.Dictionary):
        return {str(k): _plain(x) for k, x in v.items()}
    return v


class Mpris:
    def __init__(self, bus=None, name=None):
        self.bus = bus or dbus.SessionBus()
        self.name = name or self._find()
        self.obj = self.bus.get_object(self.name, PATH)

    def _find(self):
        names = [n for n in self.bus.list_names() if str(n).startswith("org.mpris.MediaPlayer2.")]
        if not names:
            raise LookupError("no MPRIS player on the bus")
        return str(names[0])

    def get(self, prop: str, iface: str = PLAYER):
        return _plain(self.obj.Get(iface, prop, dbus_interface=PROPS))

    def set(self, prop: str, value, iface: str = PLAYER):
        if isinstance(value, bool):
            value = dbus.Boolean(value)
        elif isinstance(value, float):
            value = dbus.Double(value)
        elif isinstance(value, str):
            value = dbus.String(value)
        self.obj.Set(iface, prop, value, dbus_interface=PROPS)

    def call(self, method: str, *args):
        conv = []
        for a in args:
            if isinstance(a, int):
                conv.append(dbus.Int64(a))
            elif isinstance(a, str) and a.startswith("/"):
                conv.append(dbus.ObjectPath(a))
            else:
                conv.append(a)
        getattr(dbus.Interface(self.obj, PLAYER), method)(*conv)

    def snapshot(self) -> dict:
        """The player state the verifier compares. Position is in seconds."""
        props = _plain(self.obj.GetAll(PLAYER, dbus_interface=PROPS))
        md = props.get("Metadata", {})
        return {
            "status": props.get("PlaybackStatus"),
            "loop": props.get("LoopStatus"),
            "shuffle": props.get("Shuffle"),
            "volume": round(float(props.get("Volume", 0.0)), 3),
            "position_s": round(int(props.get("Position", 0)) / 1e6, 2),
            "track": {
                "title": md.get("xesam:title"),
                "artist": md.get("xesam:artist"),
                "album": md.get("xesam:album"),
                "length_s": round(int(md.get("mpris:length", 0)) / 1e6, 1),
            } if md else None,
        }


def wait_ready(timeout: float = 30) -> Mpris:
    deadline, last = time.time() + timeout, None
    while time.time() < deadline:
        try:
            m = Mpris()
            m.snapshot()
            return m
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(0.3)
    raise TimeoutError(f"MPRIS not ready: {last}")
