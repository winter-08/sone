#!/usr/bin/env python3
"""Generate verifier/fixtures/catalog/*.json (synthetic catalog for the fake TIDAL)."""
import json
import os
import sys

ROOT = sys.argv[1]  # repo root
FIX = os.path.join(ROOT, "verifier", "fixtures")
OUT = os.path.join(FIX, "catalog")
os.makedirs(OUT, exist_ok=True)
TRACKS = {int(k): v for k, v in json.load(open(os.path.join(FIX, "tracks.json"))).items() if k.isdigit()}

COVER = "2a9d8f00-e9c4-46a0-b1c2-d3e4f5a6b7c8"
USER_ID = 424242
P1 = "11111111-2222-3333-4444-555555555555"
P2 = "66666666-7777-8888-9999-aaaaaaaaaaaa"
TS = "2024-05-01T12:00:00.000+0000"

ARTISTS = {
    7: {"id": 7, "name": "Fixture Artist", "picture": COVER, "selectedAlbumCoverFallback": COVER,
        "popularity": 50, "url": "http://www.tidal.com/artist/7", "artistTypes": ["ARTIST"],
        "artistRoles": [{"category": "Artist", "categoryId": -1}], "mixes": {}},
    8: {"id": 8, "name": "Mock Ensemble", "picture": COVER, "selectedAlbumCoverFallback": COVER,
        "popularity": 40, "url": "http://www.tidal.com/artist/8", "artistTypes": ["ARTIST"],
        "artistRoles": [{"category": "Artist", "categoryId": -1}], "mixes": {}},
}
ALBUM_DEFS = {
    77: {"title": "Fixture Album", "artist": 7, "tracks": [1001, 1002, 1003, 1004], "type": "ALBUM",
         "releaseDate": "2024-01-05", "quality": "HI_RES_LOSSLESS"},
    78: {"title": "Queue Suite", "artist": 8, "tracks": [1005, 1006, 1007], "type": "EP",
         "releaseDate": "2024-03-15", "quality": "LOSSLESS"},
}
TRACK_ALBUM = {t: a for a, d in ALBUM_DEFS.items() for t in d["tracks"]}


def artist_ref(aid, typ="MAIN"):
    return {"id": aid, "name": ARTISTS[aid]["name"], "type": typ, "picture": COVER}


def track(tid):
    v = TRACKS[tid]
    aid = TRACK_ALBUM[tid]
    al = ALBUM_DEFS[aid]
    return {
        "id": tid, "title": v["title"], "duration": v["duration"], "version": None,
        "trackNumber": al["tracks"].index(tid) + 1, "volumeNumber": 1,
        "audioQuality": v["quality"], "audioModes": ["STEREO"], "explicit": False, "ai": False,
        "popularity": 30, "isrc": f"QZFIX24{tid:05d}", "copyright": "(P) 2024 Fixture Records",
        "url": f"http://www.tidal.com/track/{tid}",
        "streamReady": True, "allowStreaming": True, "premiumStreamingOnly": False,
        "streamStartDate": "2024-01-01T00:00:00.000+0000",
        "replayGain": 0.0, "peak": 1.0,
        "artist": artist_ref(al["artist"]), "artists": [artist_ref(al["artist"])],
        "album": {"id": aid, "title": al["title"], "cover": COVER, "vibrantColor": "#4a6fa5",
                  "videoCover": None, "releaseDate": al["releaseDate"]},
        "mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"] if v["quality"] == "HI_RES_LOSSLESS" else ["LOSSLESS"]},
        "mixes": {"TRACK_MIX": f"000fixture0track0mix00{tid}"},
    }


def album(aid):
    d = ALBUM_DEFS[aid]
    return {
        "id": aid, "title": d["title"], "version": None, "cover": COVER, "vibrantColor": "#4a6fa5",
        "videoCover": None, "artist": artist_ref(d["artist"]), "artists": [artist_ref(d["artist"])],
        "numberOfTracks": len(d["tracks"]), "numberOfVideos": 0, "numberOfVolumes": 1,
        "duration": sum(TRACKS[t]["duration"] for t in d["tracks"]),
        "releaseDate": d["releaseDate"], "upc": f"0000000000{aid:03d}", "type": d["type"],
        "copyright": "(P) 2024 Fixture Records", "explicit": False, "ai": False, "popularity": 30,
        "url": f"http://www.tidal.com/album/{aid}", "audioQuality": d["quality"],
        "streamReady": True, "allowStreaming": True,
        "streamStartDate": "2024-01-01T00:00:00.000+0000", "audioModes": ["STEREO"],
        "mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"] if d["quality"] == "HI_RES_LOSSLESS" else ["LOSSLESS"]},
    }


PLAYLIST_DEFS = {
    P1: {"title": "Fixture Mixtape", "description": "A user playlist of fixture tracks.",
         "tracks": [1001, 1002, 1005, 1006], "type": "USER",
         "creator": {"id": USER_ID, "name": "fixture"}},
    P2: {"title": "Synthetic Selections", "description": "Editorial picks from the synthetic catalog.",
         "tracks": [1003, 1004, 1007], "type": "EDITORIAL",
         "creator": {"id": 0, "name": "TIDAL"}},
}


def playlist(uuid):
    d = PLAYLIST_DEFS[uuid]
    return {
        "uuid": uuid, "title": d["title"], "description": d["description"],
        "image": COVER, "squareImage": COVER, "numberOfTracks": len(d["tracks"]),
        "numberOfVideos": 0, "creator": d["creator"], "type": d["type"],
        "duration": sum(TRACKS[t]["duration"] for t in d["tracks"]), "popularity": 10,
        "publicPlaylist": True, "url": f"http://www.tidal.com/playlist/{uuid}",
        "created": "2024-04-01T12:00:00.000+0000", "lastUpdated": TS, "lastItemAddedAt": TS,
        "promotedArtists": [], "sharingLevel": "PUBLIC", "status": "READY",
        "contentBehavior": "UNRESTRICTED", "curators": [],
    }


def page(items, limit=50):
    return {"limit": limit, "offset": 0, "totalNumberOfItems": len(items), "items": items}


def write(name, obj):
    with open(os.path.join(OUT, name), "w") as f:
        json.dump(obj, f, indent=1)
        f.write("\n")


ALL_TRACKS = sorted(TRACKS)
routes = []


def route(path_regex, file, obj, host="api.tidal.com", query_contains=None, method="GET", status=None):
    write(file, obj)
    r = {"method": method, "host": host, "path_regex": path_regex, "file": file}
    if query_contains:
        r["query_contains"] = query_contains
    if status is not None:
        r["status"] = status
    routes.append(r)


# ---- search (every query returns the same result set) + suggestions
top_hits = [{"type": "ARTISTS", "value": ARTISTS[7]},
            {"type": "ALBUMS", "value": album(77)},
            {"type": "TRACKS", "value": track(1001)},
            {"type": "PLAYLISTS", "value": playlist(P1)}]
route(r"^/v2/search/?$", "search.json", {
    "artists": page([ARTISTS[7], ARTISTS[8]]),
    "albums": page([album(77), album(78)]),
    "tracks": page([track(t) for t in ALL_TRACKS]),
    "playlists": page([playlist(P1), playlist(P2)]),
    "videos": page([]),
    "topHits": top_hits,
})
# v1 fallback shares the file
routes.append({"method": "GET", "host": "api.tidal.com", "path_regex": r"^/v1/search/?$", "file": "search.json"})
route(r"^/v2/suggestions/?$", "suggestions.json", {
    "history": [],
    "suggestions": [{"query": "fixture", "highlights": []},
                    {"query": "fixture album", "highlights": []},
                    {"query": "fixture artist", "highlights": []}],
    "directHits": top_hits,
})

# ---- albums: v1 page (what the album view uses) + v1 detail/tracks (MCP, play buttons)
for aid, d in ALBUM_DEFS.items():
    other = 78 if aid == 77 else 77
    route(r"^/v1/pages/album$", f"album_page_{aid}.json", {
        "selfLink": None, "id": f"album-{aid}", "title": d["title"],
        "rows": [
            {"modules": [{"type": "ALBUM_HEADER", "title": "", "album": album(aid),
                          "credits": [], "review": {"source": None, "text": None}}]},
            {"modules": [{"type": "ALBUM_ITEMS", "title": "",
                          "pagedList": {"limit": 50, "offset": 0,
                                        "totalNumberOfItems": len(d["tracks"]),
                                        "items": [{"item": track(t), "type": "track"} for t in d["tracks"]]}}]},
            {"modules": [{"type": "ALBUM_LIST", "title": "Related Albums",
                          "pagedList": page([album(other)])}]},
        ],
    }, query_contains=f"albumId={aid}")
    route(rf"^/v1/albums/{aid}$", f"album_{aid}.json", album(aid))
    route(rf"^/v1/albums/{aid}/(tracks|items)$", f"album_{aid}_tracks.json",
          page([track(t) for t in d["tracks"]]))


# ---- artists: v2 page (what the artist view uses) + v1 detail/toptracks/albums/bio
def v2(typ, obj):
    return {"type": typ, "data": obj}


for aid in (7, 8):
    other = 8 if aid == 7 else 7
    own_albums = [a for a, d in ALBUM_DEFS.items() if d["artist"] == aid]
    own_tracks = [t for a in own_albums for t in ALBUM_DEFS[a]["tracks"]]
    own_playlists = [P1] if aid == 7 else [P2]
    name = ARTISTS[aid]["name"]
    route(rf"^/v2/artist/{aid}$", f"artist_page_{aid}.json", {
        "item": {"type": "ARTIST", "data": ARTISTS[aid]},
        "header": {"biography": {"text": f"{name} is a synthetic artist that exists only in the SONE test fixtures.",
                                 "source": "Fixture Wiki"},
                   "followersAmount": 1234 if aid == 7 else 567},
        "items": [
            {"type": "HORIZONTAL_LIST", "moduleId": "ARTIST_TOP_TRACKS", "title": "Top tracks",
             "items": [v2("TRACK", track(t)) for t in own_tracks]},
            {"type": "HORIZONTAL_LIST", "moduleId": "ARTIST_ALBUMS", "title": "Albums",
             "items": [v2("ALBUM", album(a)) for a in own_albums]},
            {"type": "HORIZONTAL_LIST", "moduleId": "ARTIST_PLAYLISTS", "title": f"Playlists with {name}",
             "items": [v2("PLAYLIST", playlist(p)) for p in own_playlists]},
            {"type": "HORIZONTAL_LIST", "moduleId": "ARTIST_SIMILAR_ARTISTS", "title": "Fans also like",
             "items": [v2("ARTIST", ARTISTS[other])]},
        ],
    })
    route(rf"^/v1/artists/{aid}$", f"artist_{aid}.json", ARTISTS[aid])
    route(rf"^/v1/artists/{aid}/toptracks$", f"artist_{aid}_toptracks.json",
          page([track(t) for t in own_tracks]))
    route(rf"^/v1/artists/{aid}/albums$", f"artist_{aid}_albums.json",
          page([album(a) for a in own_albums]))
    route(rf"^/v1/artists/{aid}/bio$", f"artist_{aid}_bio.json",
          {"source": "Fixture Wiki", "lastUpdated": TS,
           "text": f"{name} is a synthetic artist that exists only in the SONE test fixtures."})

# ---- playlists
for uuid, d in PLAYLIST_DEFS.items():
    short = "p1" if uuid == P1 else "p2"
    route(rf"^/v1/playlists/{uuid}$", f"playlist_{short}.json", playlist(uuid))
    route(rf"^/v1/playlists/{uuid}/(items|tracks)$", f"playlist_{short}_items.json",
          page([{"item": track(t), "type": "track", "cut": None} for t in d["tracks"]], limit=50))
    route(rf"^/v1/playlists/{uuid}/recommendations/items$", f"playlist_{short}_recommendations.json",
          page([]))


# user's own playlists: v2 folders (sidebar / library) + v1 list (MCP get_user_playlists)
def folder_item(uuid):
    p = playlist(uuid)
    p["creator"] = {**p["creator"], "picture": None, "type": "USER" if p["type"] == "USER" else "TIDAL"}
    p.update({"source": "DEFAULT", "customImageUrl": None, "trn": f"trn:playlist:{uuid}",
              "itemType": "PLAYLIST"})
    return {"trn": f"trn:playlist:{uuid}", "itemType": "PLAYLIST", "addedAt": TS,
            "lastModifiedAt": TS, "name": p["title"], "parent": None, "data": p}


folders = {"lastModifiedAt": TS, "items": [folder_item(P1)], "totalNumberOfItems": 1, "cursor": None}
route(r"^/v2/my-collection/playlists/folders$", "playlist_folders.json", folders)
route(r"^/v2/my-collection/playlists/folders/flattened$", "playlist_folders.json", folders)
route(rf"^/v1/users/{USER_ID}/playlists$", "user_playlists.json", page([playlist(P1)]))
route(rf"^/v1/users/{USER_ID}/playlistsAndFavoritePlaylists$", "user_playlists_and_favorites.json",
      page([{"type": "USER_CREATED", "created": TS, "playlist": playlist(P1)}]))


# ---- home (v2 feed; every tab slug gets the same feed)
home = {
    "uuid": "fixture-home",
    "page": {"cursor": None},
    "header": {"vibes": {"items": [{"name": "For You", "type": "STATIC"}]}},
    "items": [
        {"type": "SHORTCUT_LIST", "moduleId": "SHORTCUTS", "title": "Shortcuts",
         "items": [v2("PLAYLIST", playlist(P1)), v2("ALBUM", album(77)), v2("ALBUM", album(78)),
                   v2("ARTIST", ARTISTS[7]), v2("PLAYLIST", playlist(P2)), v2("ARTIST", ARTISTS[8])]},
        {"type": "HORIZONTAL_LIST", "moduleId": "FIXTURE_ALBUMS", "title": "Fixture Albums",
         "items": [v2("ALBUM", album(77)), v2("ALBUM", album(78))]},
        {"type": "HORIZONTAL_LIST", "moduleId": "FIXTURE_PLAYLISTS", "title": "Fixture Playlists",
         "items": [v2("PLAYLIST", playlist(P1)), v2("PLAYLIST", playlist(P2))]},
        {"type": "HORIZONTAL_LIST", "moduleId": "FIXTURE_ARTISTS", "title": "Fixture Artists",
         "items": [v2("ARTIST", ARTISTS[7]), v2("ARTIST", ARTISTS[8])]},
        {"type": "TRACK_LIST", "moduleId": "FIXTURE_TRACKS", "title": "Fixture Tracks",
         "items": [v2("TRACK", track(t)) for t in ALL_TRACKS]},
    ],
}
route(r"^/v2/home/feed/[A-Za-z_]+$", "home_feed.json", home)

# ---- mixes: v1 pages/mix (MIX_HEADER + TRACK_LIST). Only the feed's history mix exists;
# catalog tracks carry no `mixes.TRACK_MIX`, so there is no track radio.
def image_url(size):
    return f"https://resources.tidal.com/images/{COVER.replace('-', '/')}/{size}x{size}.jpg"


MIX_IMAGES = {"SMALL": {"width": 320, "height": 320, "url": image_url(320)},
              "MEDIUM": {"width": 640, "height": 640, "url": image_url(640)},
              "LARGE": {"width": 1280, "height": 1280, "url": image_url(1280)}}
HISTORY_MIX_ID = "000fixture0history0mix0000001"
HISTORY_MIX = {"id": HISTORY_MIX_ID, "mixType": "HISTORY_MONTHLY_MIX", "title": "April 2024",
               "subTitle": "Fixture Artist, Mock Ensemble and more", "shortSubtitle": "Fixture Artist",
               "images": MIX_IMAGES, "detailImages": MIX_IMAGES, "sharingImages": None,
               "contentBehavior": "UNRESTRICTED", "master": False, "isStableId": True,
               "titleColor": "#ffffff", "updated": TS}
HISTORY_MIX_TRACKS = [1001, 1005, 1002, 1006]
route(r"^/v1/pages/mix$", "mix_history.json", {
    "selfLink": None, "id": f"mix-{HISTORY_MIX_ID}", "title": HISTORY_MIX["title"],
    "rows": [
        {"modules": [{"type": "MIX_HEADER", "title": "", "mix": HISTORY_MIX}]},
        {"modules": [{"type": "TRACK_LIST", "title": "",
                      "pagedList": page([track(t) for t in HISTORY_MIX_TRACKS])}]},
    ],
}, query_contains=f"mixId={HISTORY_MIX_ID}")

# ---- activity feed (Feed page; the sidebar Feed bell shows the unseen dot from the same call)
# Dates are fixed, so the page groups everything under "Older".
route(r"^/v2/feed/activities$", "feed_activities.json", {
    "activities": [
        {"seen": False, "followableActivity": {"activityType": "NEW_HISTORY_MIX",
                                               "occurredAt": "2024-05-01T00:00:00.000Z",
                                               "historyMix": HISTORY_MIX}},
        {"seen": False, "followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                                               "occurredAt": "2024-03-15T00:00:00.000Z",
                                               "album": album(78)}},
        {"seen": True, "followableActivity": {"activityType": "NEW_ALBUM_RELEASE",
                                              "occurredAt": "2024-01-05T00:00:00.000Z",
                                              "album": album(77)}},
    ],
    "stats": {"totalNotSeenActivities": 2},
})
# Opening the Feed page marks everything seen (PUT; any 2xx will do)
route(r"^/v2/feed/activities/seen$", "feed_seen.json", {}, method="PUT")

# ---- explore (v1 pages/explore: PAGE_LINKS_CLOUD pill rows + a PAGE_LINKS icon grid)
# Every link opens a v1 page served below; content pages show catalog items.
def link(title, name):
    return {"title": title, "icon": None, "imageId": None, "apiPath": f"pages/{name}"}


EXPLORE_CONTENT = {  # page name -> (title, playlists, albums, tracks, artists)
    "genre_fixture_pop": ("Fixture Pop", [P1], [77], [1001, 1002, 1003, 1004], [7]),
    "genre_synthetic_jazz": ("Synthetic Jazz", [P2], [78], [1005, 1006, 1007], [8]),
    "mood_focus": ("Focus", [P2, P1], [77], [1003, 1004], [7, 8]),
    "mood_late_night": ("Late Night", [P1], [78], [1005, 1006], [8]),
    "m_2020s": ("2020s", [P1, P2], [77, 78], ALL_TRACKS, [7, 8]),
    "explore_new_music": ("New", [P1], [78, 77], [1005, 1006, 1007], [8, 7]),
    "explore_top": ("Top", [P1, P2], [77, 78], [1001, 1005, 1002], [7, 8]),
    "hires": ("HiRes", [P2], [77], [1002, 1003, 1004], [7]),
}
EXPLORE_GROUPS = [  # (title, view-all page, [page names])
    ("Genres", "genre_page", ["genre_fixture_pop", "genre_synthetic_jazz"]),
    ("Moods & Activities", "moods_page", ["mood_focus", "mood_late_night"]),
    ("Decades", "decades_page", ["m_2020s"]),
]
EXPLORE_ICONS = ["explore_new_music", "explore_top", "hires"]


def links(names):
    return [link(EXPLORE_CONTENT[n][0], n) for n in names]


def v1_module(typ, title, items, **extra):
    return {"modules": [{"type": typ, "title": title, "pagedList": page(items), **extra}]}


route(r"^/v1/pages/explore$", "explore.json", {
    "selfLink": None, "id": "explore", "title": "Explore",
    "rows": [v1_module("PAGE_LINKS_CLOUD", t, links(names),
                       showMore={"title": "View all", "apiPath": f"pages/{va}"})
             for t, va, names in EXPLORE_GROUPS]
            + [v1_module("PAGE_LINKS", "Shortcuts", links(EXPLORE_ICONS))],
})
for t, va, names in EXPLORE_GROUPS:  # "View all": a nav-only page (text grid of links)
    route(rf"^/v1/pages/{va}$", f"explore_{va}.json", {
        "selfLink": None, "id": va, "title": t,
        "rows": [v1_module("PAGE_LINKS_CLOUD", "", links(names))]})
for name, (t, pls, als, trs, ars) in EXPLORE_CONTENT.items():
    route(rf"^/v1/pages/{name}$", f"explore_{name}.json", {
        "selfLink": None, "id": name, "title": t,
        "rows": [v1_module("PLAYLIST_LIST", f"{t} Playlists", [playlist(p) for p in pls]),
                 v1_module("ALBUM_LIST", f"{t} Albums", [album(a) for a in als]),
                 v1_module("TRACK_LIST", f"{t} Tracks", [track(x) for x in trs]),
                 v1_module("ARTIST_LIST", f"{t} Artists", [ARTISTS[a] for a in ars])]})

# ---- lyrics: 1001 and 1005 have synced lyrics; every other track has none, which TIDAL
# says with a 404 (subStatus 2001). Credits for every track (drawer "Credits" tab).
LYRICS = {
    1001: ["Fixture line one", "Fixture line two", "Fixture line three", "Fixture line four"],
    1005: ["Queue A opens", "Queue A carries on", "Queue A winds down"],
}
for tid in ALL_TRACKS:
    if tid in LYRICS:
        lines = LYRICS[tid]
        step = TRACKS[tid]["duration"] / (len(lines) + 1)
        stamps = [f"[{int(step * (i + 0.5)) // 60:02d}:{step * (i + 0.5) % 60:05.2f}]" for i in range(len(lines))]
        route(rf"^/v1/tracks/{tid}/lyrics$", f"lyrics_{tid}.json", {
            "trackId": tid, "lyricsProvider": "Fixture Lyrics", "providerCommontrackId": f"fx{tid}",
            "providerLyricsId": f"fxl{tid}", "isRightToLeft": False,
            "lyrics": "\n".join(lines),
            "subtitles": "\n".join(f"{s} {l}" for s, l in zip(stamps, lines)),
        })
    a = ALBUM_DEFS[TRACK_ALBUM[tid]]["artist"]
    route(rf"^/v1/tracks/{tid}/credits$", f"credits_{tid}.json", [
        {"type": "Main Artist", "contributors": [{"name": ARTISTS[a]["name"], "id": a}]},
        {"type": "Composer", "contributors": [{"name": "Fixture Composer"}]},
        {"type": "Producer", "contributors": [{"name": "Synthetic Producer"}]},
    ])
route(r"^/v1/tracks/\d+/lyrics$", "lyrics_none.json",
      {"status": 404, "subStatus": 2001, "userMessage": "Lyrics not available"}, status=404)

# ---- MusicBrainz ISRC lookup (scrobble enrichment, fires once a catalog track has played a
# while): the synthetic ISRCs are unknown there, which MusicBrainz answers with a 404.
route(r"^/ws/2/isrc/[^/]+$", "musicbrainz_isrc_none.json",
      {"error": "Not Found", "help": "For usage, please see: https://musicbrainz.org/development/mmd"},
      host="musicbrainz.org", status=404)

# ---- track radio (the drawer's Suggested tracks tab loads mixes.TRACK_MIX): every track's radio
# is the same fixture mix, matched by the id prefix in the mixId query.
TRACK_MIX = {"id": "000fixture0track0mix00", "mixType": "TRACK_MIX", "title": "Track Radio",
             "subTitle": "Fixture Artist, Mock Ensemble", "shortSubtitle": "Fixture Artist",
             "images": MIX_IMAGES, "detailImages": MIX_IMAGES, "sharingImages": None,
             "contentBehavior": "UNRESTRICTED", "master": False, "isStableId": True,
             "titleColor": "#ffffff", "updated": TS}
route(r"^/v1/pages/mix$", "mix_track_radio.json", {
    "selfLink": None, "id": "mix-track-radio", "title": TRACK_MIX["title"],
    "rows": [
        {"modules": [{"type": "MIX_HEADER", "title": "", "mix": TRACK_MIX}]},
        {"modules": [{"type": "TRACK_LIST", "title": "",
                      "pagedList": page([track(t) for t in (1003, 1004, 1007, 1006)])}]},
    ],
}, query_contains="track0mix")

# ---- ListenBrainz: token validation for Settings > Scrobbling > Connect
route(r"^/1/validate-token$", "listenbrainz_validate.json",
      {"code": 200, "message": "Token valid.", "valid": True, "user_name": "fixture"},
      host="api.listenbrainz.org")

with open(os.path.join(OUT, "routes.json"), "w") as f:
    json.dump(routes, f, indent=1)
    f.write("\n")
print(f"{len(routes)} routes, {len(os.listdir(OUT))} files")
