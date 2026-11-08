"""Accessibility channel: SONE's AT-SPI tree (WebKitGTK today, AccessKit in a Rune build)."""
import time

INTERACTIVE = {"button", "push button", "toggle button", "entry", "link", "menu item", "slider",
               "check box", "radio button", "combo box", "tab", "list item"}
NOISE = {"section", "panel", "filler", "paragraph"}


def apps(name_hint: str = "sone") -> list:
    """Every accessible application of the app under test. A second window (the miniplayer)
    is a second frame in WebKit's application, but may be its own application elsewhere."""
    import pyatspi
    desktop = pyatspi.Registry.getDesktop(0)
    out = []
    for i in range(desktop.childCount):
        a = desktop.getChildAtIndex(i)
        if a and a.name and name_hint in a.name.lower():
            out.append(a)
    return out


class _All:
    """A stand-in root whose children are all of the app's accessible applications."""
    def __init__(self, roots):
        self.roots = roots
        self.childCount = len(roots)
        self.name = ""
        self.parent = None

    def getChildAtIndex(self, i):
        return self.roots[i]

    def getRoleName(self):
        return "apps"

    def getState(self):
        raise RuntimeError("virtual root")

    def queryAction(self):
        raise NotImplementedError


def app(name_hint: str = "sone"):
    roots = apps(name_hint)
    if not roots:
        return None
    return roots[0] if len(roots) == 1 else _All(roots)


def walk(node, depth=0, max_depth=40, out=None):
    out = [] if out is None else out
    if node is None or depth > max_depth:
        return out
    try:
        out.append((depth, node.getRoleName(), (node.name or "").strip(), node))
        for i in range(min(node.childCount, 500)):
            walk(node.getChildAtIndex(i), depth + 1, max_depth, out)
    except Exception:  # noqa: BLE001 - nodes go stale while the UI changes
        pass
    return out


def controls(root=None) -> list[str]:
    """The semantic inventory of what's on screen: sorted 'role: name' for named controls,
    landmarks and headings. This is what L3 compares between builds."""
    root = root or app()
    seen = set()
    for _, role, name, _ in walk(root):
        if name and (role in INTERACTIVE or role in ("heading", "landmark")):
            seen.add(f"{role}: {name}")
    return sorted(seen)


def unnamed_controls(root=None) -> int:
    return sum(1 for _, role, name, _ in walk(root or app()) if role in INTERACTIVE and not name)


def _actionable(node, max_up: int = 4):
    """The node itself if it has an action, else the nearest ancestor that does (a card whose
    only accessible part is its heading is clicked through the card)."""
    for _ in range(max_up + 1):
        try:
            if node.queryAction().nActions > 0:
                return node
        except Exception:  # noqa: BLE001 - no Action interface
            pass
        node = node.parent
        if node is None:
            break
    raise LookupError("no actionable element at or above the control")


def press(name: str, role: str | None = None, nth: int = 0) -> str:
    r, n, node = find(name, role, nth=nth)
    act = _actionable(node).queryAction()
    names = [act.getName(i) for i in range(act.nActions)]
    for pref in ("press", "click", "activate", "jump"):
        if pref in names:
            act.doAction(names.index(pref))
            return f"{r}: {n}"
    act.doAction(0)
    return f"{r}: {n}"


def find(name: str, role: str | None = None, timeout: float = 5.0, nth: int = 0):
    """The `nth` (0-based, tree order) control with this accessible name (case-insensitive),
    waiting for it to appear. `nth` disambiguates repeated names, e.g. the sidebar's "Albums"
    tab (0) and the search page's "Albums" tab (1)."""
    deadline = time.time() + timeout
    while True:
        hits = [(r, n, node) for _, r, n, node in walk(app())
                if node is not None and n.lower() == name.lower() and (role is None or r == role)]
        if len(hits) > nth:
            return hits[nth]
        if time.time() > deadline:
            raise LookupError(f"no control named {name!r}" + (f" with role {role!r}" if role else ""))
        time.sleep(0.25)


def focus(name: str, role: str | None = None, nth: int = 0) -> str:
    """Give a control keyboard focus (then keystrokes go to it): AT-SPI grabFocus, falling back
    to its default action (a click), which focuses text fields in both WebKit and AccessKit."""
    r, n, node = find(name, role, nth=nth)
    try:
        if node.queryComponent().grabFocus():
            return f"{r}: {n}"
    except Exception:  # noqa: BLE001
        pass
    act = node.queryAction()
    act.doAction(0)
    return f"{r}: {n}"


def type_text(text: str) -> None:
    """Real keystrokes through the compositor's virtual keyboard, so both toolkits see typing."""
    import subprocess
    # A fresh virtual keyboard's first key can arrive before the client has its keymap and be
    # dropped; a modifier tap plus a short pause absorbs that.
    subprocess.run(["wtype", "-k", "Shift_L", "-s", "150", "--", text], check=True, timeout=10)


def key(name: str) -> None:
    """One key press by XKB name, e.g. Return, Escape, Tab."""
    import subprocess
    subprocess.run(["wtype", "-k", name], check=True, timeout=10)


def wait_ready(timeout: float = 30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        a = app()
        if a is not None and any(r == "document web" or r == "button" for _, r, _, _ in walk(a, max_depth=12)):
            return a
        time.sleep(0.5)
    raise TimeoutError("SONE never appeared on the accessibility bus")
