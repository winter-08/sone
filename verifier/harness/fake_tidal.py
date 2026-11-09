"""mitmproxy addon: a fake TIDAL that never touches the real internet.

Every request the app makes through the proxy is logged to $SPIKE_LOG (JSONL), then answered
locally. Nothing is forwarded upstream: unknown requests get a logged 404, so a missing fixture
shows up in the log instead of silently reaching a real server.

    mitmdump -s fake_tidal.py --set fixtures=/path/to/fixtures

Besides the built-in handlers below, a synthetic catalog (search, album/artist/playlist/explore/mix
pages, home and activity feeds, lyrics, credits) is served from fixtures/catalog/: routes.json maps
(method, host, path regex[, exact query parameter]) to a JSON file and status there; first match
wins. The device code sign-in is stateful and lives here, not in routes.json.
See fixtures/catalog/README.md.
"""
import base64
import json
import os
import re
import time

from mitmproxy import ctx, http

USER_ID = 424242
FIXTURE_HOST = "fixtures.sone.test"
COVER = "2a9d8f00-e9c4-46a0-b1c2-d3e4f5a6b7c8"
TRACKS: dict = {}  # track id -> (file, rate, bits, quality, title, duration), from tracks.json
CATALOG: list = []  # [(method, host, compiled path regex, query param or None, file path, status)]
DEVICE_POLLS: dict = {}  # device code issued by /device_authorization -> token polls so far
DEVICE_SEQ = [0]


def load(loader):
    loader.add_option("fixtures", str, "", "directory with FLAC fixtures")


def configure(updated):
    if "fixtures" in updated and ctx.options.fixtures:
        spec = json.load(open(os.path.join(ctx.options.fixtures, "tracks.json")))
        TRACKS.clear()
        for k, v in spec.items():
            if k.isdigit():
                TRACKS[int(k)] = (v["file"], v["rate"], v["bits"], v["quality"], v["title"], v["duration"])
        cat_dir = os.path.join(ctx.options.fixtures, "catalog")
        CATALOG.clear()
        if os.path.isfile(os.path.join(cat_dir, "routes.json")):
            for r in json.load(open(os.path.join(cat_dir, "routes.json"))):
                CATALOG.append((r.get("method", "GET").upper(), r["host"], re.compile(r["path_regex"]),
                                r.get("query_contains"), os.path.join(cat_dir, r["file"]),
                                int(r.get("status", 200))))


def serve_catalog(flow: http.HTTPFlow, host: str, path: str, method: str) -> bool:
    """First routes.json entry matching method, host, path (no query) and, if given, one exact
    `key=value` query parameter (a value without '=' is matched as a substring of the query).
    The response status is the entry's `status` (default 200)."""
    query = flow.request.path.partition("?")[2]
    params = set(query.split("&"))
    for m, h, rx, qc, f, st in CATALOG:
        if m != method or h != host or not rx.search(path):
            continue
        if qc and not (qc in params if "=" in qc else qc in query):
            continue
        respond(flow, st, open(f, "rb").read())
        return True
    return False


def track_json(tid: int) -> dict:
    """One track, consistent with the synthetic catalog (catalog/README.md): 1001-1004 are on
    album 77 "Fixture Album" by artist 7, 1005-1007 on album 78 "Queue Suite" by artist 8."""
    f, sr, bd, q, title, dur = TRACKS.get(tid, TRACKS[1001])
    if tid >= 1005:
        artist, album, number = {"id": 8, "name": "Mock Ensemble", "type": "MAIN"}, {"id": 78, "title": "Queue Suite", "cover": COVER}, tid - 1004
    else:
        artist, album, number = {"id": 7, "name": "Fixture Artist", "type": "MAIN"}, {"id": 77, "title": "Fixture Album", "cover": COVER}, tid - 1000
    return {
        "id": tid, "title": title, "duration": dur, "trackNumber": number,
        "volumeNumber": 1, "audioQuality": q, "explicit": False, "streamReady": True,
        "allowStreaming": True,
        "artist": artist,
        "artists": [artist],
        "album": album,
        "mediaMetadata": {"tags": ["LOSSLESS"]},
        "mixes": {"TRACK_MIX": f"000fixture0track0mix00{tid}"},
    }


def page(items):
    return {"limit": 50, "offset": 0, "totalNumberOfItems": len(items), "items": items}


def respond(flow: http.HTTPFlow, status: int, body, ctype="application/json"):
    raw = body if isinstance(body, bytes) else json.dumps(body).encode()
    flow.response = http.Response.make(status, raw, {"Content-Type": ctype})


def serve_file(flow: http.HTTPFlow, p: str, ctype: str):
    data = open(p, "rb").read()
    rng = flow.request.headers.get("Range")
    if rng and (m := re.fullmatch(r"bytes=(\d+)-(\d*)", rng)):
        a, b = int(m.group(1)), int(m.group(2) or len(data) - 1)
        flow.response = http.Response.make(206, data[a:b + 1], {
            "Content-Type": ctype, "Accept-Ranges": "bytes",
            "Content-Range": f"bytes {a}-{b}/{len(data)}"})
    else:
        flow.response = http.Response.make(200, data, {"Content-Type": ctype,
                                                       "Accept-Ranges": "bytes"})


def route(flow: http.HTTPFlow) -> bool:
    host, path, method = flow.request.pretty_host, flow.request.path.split("?")[0], flow.request.method

    if host == "auth.tidal.com":
        # Device code sign-in (catalog/README.md, "Sign-in"): every /device_authorization issues
        # a fresh code; the first token poll for it answers 400 authorization_pending (as if
        # the user hadn't approved yet), later polls hand out the tokens. Other grants
        # (refresh_token, authorization_code, unknown device codes) get tokens straight away.
        if path.endswith("/device_authorization"):
            DEVICE_SEQ[0] += 1
            code = f"fake-device-{DEVICE_SEQ[0]}"
            DEVICE_POLLS[code] = 0
            respond(flow, 200, {"deviceCode": code, "userCode": "FAKE1", "verificationUri":
                                "link.tidal.com", "verificationUriComplete": "link.tidal.com/FAKE1",
                                "expiresIn": 300, "interval": 1})
            return True
        if path.endswith("/token"):
            form = flow.request.urlencoded_form
            code = form.get("device_code", "")
            if form.get("grant_type", "").endswith("device_code") and code in DEVICE_POLLS:
                DEVICE_POLLS[code] += 1
                if DEVICE_POLLS[code] < 2:
                    respond(flow, 400, {"status": 400, "error": "authorization_pending", "sub_status": 1002,
                                        "error_description": "Device Authorization code is not authorized yet"})
                    return True
            respond(flow, 200, {"access_token": "fake-access", "refresh_token": "fake-refresh",
                                "expires_in": 86400, "token_type": "Bearer", "user_id": USER_ID,
                                "user": {"userId": USER_ID, "countryCode": "US", "username": "fixture"}})
            return True

    if host == "api.tidal.com":
        if path == "/v1/sessions":
            respond(flow, 200, {"sessionId": "fake-session", "userId": USER_ID, "countryCode": "US"})
            return True
        if m := re.fullmatch(r"/v1/tracks/(\d+)/playbackinfopostpaywall", path):
            tid = int(m.group(1))
            f, sr, bd, q, _, _ = TRACKS.get(tid, TRACKS[1001])
            if f.endswith(".mpd"):
                mime = "application/dash+xml"
                manifest = open(os.path.join(ctx.options.fixtures, f), "rb").read()
            else:
                mime = "application/vnd.tidal.bts"
                manifest = json.dumps({"mimeType": "audio/flac", "codecs": "flac",
                                       "encryptionType": "NONE",
                                       "urls": [f"https://{FIXTURE_HOST}/{f}"]}).encode()
            respond(flow, 200, {"trackId": tid, "assetPresentation": "FULL", "audioMode": "STEREO",
                                "audioQuality": q, "bitDepth": bd, "sampleRate": sr,
                                "manifestMimeType": mime,
                                "manifest": base64.b64encode(manifest).decode(),
                                "trackReplayGain": 0.0, "trackPeakAmplitude": 1.0,
                                "albumReplayGain": 0.0, "albumPeakAmplitude": 1.0})
            return True
        # Music videos (catalog/README.md, "Videos"): every video plays fixtures/video, an HLS
        # master playlist with 360p and 720p variants, as TIDAL's "emu" manifest points at one.
        if m := re.fullmatch(r"/v1/videos/(\d+)/playbackinfopostpaywall", path):
            manifest = json.dumps({"mimeType": "application/vnd.apple.mpegurl",
                                   "urls": [f"https://{FIXTURE_HOST}/video/master.m3u8"]}).encode()
            respond(flow, 200, {"videoId": int(m.group(1)), "streamType": "ON_DEMAND",
                                "assetPresentation": "FULL", "videoQuality": "HIGH",
                                "manifestMimeType": "application/vnd.tidal.emu",
                                "manifest": base64.b64encode(manifest).decode()})
            return True
        if m := re.fullmatch(r"/v1/tracks/(\d+)", path):
            respond(flow, 200, track_json(int(m.group(1))))
            return True
        if re.fullmatch(rf"/v1/users/{USER_ID}", path):
            respond(flow, 200, {"id": USER_ID, "username": "fixture", "firstName": "Fixture",
                                "lastName": "User", "countryCode": "US"})
            return True
        if path.endswith("/subscription"):
            respond(flow, 200, {"status": "ACTIVE", "highestSoundQuality": "HI_RES_LOSSLESS",
                                "subscription": {"type": "HIFI"}})
            return True
        if "/favorites/ids" in path:
            respond(flow, 200, {"TRACK": [], "ALBUM": [], "ARTIST": [], "PLAYLIST": [], "VIDEO": []})
            return True
        # Catalog before the catch-all empty-page stub, so e.g. /users/{id}/playlists can be served.
        if serve_catalog(flow, host, path, method):
            return True
        if "/users/" in path or "/favorites" in path:
            respond(flow, 200, page([]))
            return True

    if host == FIXTURE_HOST:
        root = os.path.realpath(ctx.options.fixtures)
        p = os.path.realpath(os.path.join(root, path.lstrip("/")))
        if p.startswith(root + os.sep) and os.path.isfile(p):
            if flow.request.method == "OPTIONS":
                # CORS preflight from the web view's hls.js (TIDAL's CDN allows any origin).
                flow.response = http.Response.make(204, b"", {"Access-Control-Allow-Origin": "*",
                                                             "Access-Control-Allow-Headers": "*",
                                                             "Access-Control-Allow-Methods": "GET, HEAD, OPTIONS"})
                return True
            ctype = {".mp4": "audio/mp4", ".m3u8": "application/vnd.apple.mpegurl", ".ts": "video/mp2t"}
            serve_file(flow, p, ctype.get(os.path.splitext(p)[1], "audio/flac"))
            if p.endswith((".m3u8", ".ts")):
                flow.response.headers["Access-Control-Allow-Origin"] = "*"
            return True

    if host == "resources.tidal.com" and path.startswith("/images/"):
        serve_file(flow, os.path.join(ctx.options.fixtures, "cover.jpg"), "image/jpeg")
        return True

    if host == "api.github.com":
        respond(flow, 200, {"tag_name": "v0.0.1", "html_url": "https://github.com/lullabyX/sone"})
        return True
    if host == "ec.tidal.com":
        respond(flow, 200, {})
        return True
    return serve_catalog(flow, host, path, method)


def request(flow: http.HTTPFlow):
    handled = route(flow)
    if not handled:
        respond(flow, 404, {"status": 404, "userMessage": "spike: no fixture"})
    with open(os.environ.get("SPIKE_LOG", "flows.jsonl"), "a") as f:
        f.write(json.dumps({
            "t": time.time(), "method": flow.request.method, "host": flow.request.pretty_host,
            "path": flow.request.path, "status": flow.response.status_code, "handled": handled,
            "ua": flow.request.headers.get("User-Agent", "")[:60],
        }) + "\n")
