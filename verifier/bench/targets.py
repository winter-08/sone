"""What the benchmark drivers open: the fake TIDAL's fixtures, or a real catalogue.

With SPIKE_BENCH_REAL=1 (a signed-in profile, SPIKE_NET=1) the album comes from an MCP search
for SPIKE_BENCH_ALBUM, its artist from the album, the played tracks from the album's track list;
the album is reached through the search box from Home, as a user would. (No artist page: a
real "Daft Punk" is also an album title, and Rune's album page doesn't link its artist yet.)
Without it, the fixture names.
"""
import os
import time

from channels import atspi

REAL = bool(os.environ.get("SPIKE_BENCH_REAL"))
ALBUM_QUERY = os.environ.get("SPIKE_BENCH_ALBUM", "Random Access Memories")


def wait_control(name: str, timeout: float = 30.0, poll: float = 0.05) -> float:
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        try:
            if name in atspi.controls():
                return time.monotonic() - t0
        except Exception:  # noqa: BLE001 - tree changing under us
            pass
        time.sleep(poll)
    raise TimeoutError(name)


def headings() -> set[str]:
    try:
        return {c for c in atspi.controls() if c.startswith("heading: ")}
    except Exception:  # noqa: BLE001
        return set()


def wait_home(before: set[str], timeout: float = 60.0) -> None:
    """Home's content is on screen: the fixture's section, or any section heading (a real
    Home's sections vary by account; `before` lists headings that aren't content, if any)."""
    if not REAL:
        wait_control("heading: Fixture Albums", timeout)
        return
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        if headings() - CHROME - before:
            return
        time.sleep(0.05)
    raise TimeoutError("Home content")


# headings that are app chrome, not Home content
CHROME = {"heading: Your Library"}


def discover(m) -> dict:
    if not REAL:
        return {"album": "Fixture Album", "artist": "Mock Ensemble", "search": "fixture", "tracks": [1001, 1002]}
    res = m.call("search_tracks", {"query": ALBUM_QUERY, "limit": 10})
    albums = res["albums"]
    album = next((a for a in albums if a["title"].lower() == ALBUM_QUERY.lower()), albums[0])
    tracks = m.call("get_album_tracks", {"album_id": album["id"]})["tracks"]
    return {"album": album["title"], "artist": album["artist"].split(",")[0].strip(),
            "search": album["title"], "tracks": [t["id"] for t in tracks[:2]]}


def search(query: str, until: str | None = None) -> float:
    """Type a query and open all results; seconds from the press until the results are shown:
    the results heading, or (`until`) a result's heading (a real catalogue's page may title
    itself before its results arrive)."""
    atspi.focus("Search", "entry")
    time.sleep(0.2)
    atspi.key("ctrl+a")  # typing replaces an earlier query (select-all works in both apps)
    time.sleep(0.1)
    atspi.type_text(query)
    time.sleep(0.8)  # the suggestions for this query, not the last one (debounced)
    atspi.press("View all results")
    return wait_control(f"heading: {until}" if until else f'heading: Results for "{query}"')


def press_album(t: dict) -> float:
    """On a page showing the album's card: open it. Seconds until its page is up (Shuffle is
    there whether or not the account has favourited the album)."""
    if REAL:
        # The Albums tab: on "All" the first card with the album's title can be a playlist of
        # the same name, and the two apps order their sections differently. (nth 0 is the
        # library's Albums filter in the sidebar.)
        atspi.press("Albums", nth=1)
        time.sleep(1.5)
        wait_control(f"heading: {t['album']}")
    atspi.press(t["album"], "heading")
    return wait_control("button: Shuffle" if REAL else "button: Favorite album")


def open_album(t: dict) -> float:
    """From Home (or, for fixtures, anywhere): to the album's page."""
    if REAL:
        search(t["album"], t["album"])
    elif "heading: Fixture Albums" not in headings():
        atspi.press("Home")
        wait_control("heading: Fixture Albums")
    return press_album(t)


def open_artist(t: dict) -> float:
    if "heading: Fixture Albums" not in headings():
        atspi.press("Home")
        wait_control("heading: Fixture Albums")
    atspi.press(t["artist"], "heading")
    return wait_control("button: Follow")
