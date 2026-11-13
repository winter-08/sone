# Synthetic catalog for the fake TIDAL

Static JSON responses that let Home, Search, Album, Artist, Playlist, Explore and Feed views,
the lyrics/credits panels and the mix page render real content in the hermetic sandbox. `harness/fake_tidal.py` loads `routes.json` at startup and
consults it after its built-in handlers (auth, sessions, `/v1/tracks/{id}`, playbackinfo,
favorites stubs) and before the catch-all empty-page stub for `/users/` and `/favorites` paths
and the 404 fallback. Everything is invented. Covers are all `2a9d8f00-e9c4-46a0-b1c2-d3e4f5a6b7c8`.

## Search

**Every query returns the same result set**: `/v2/search` (and the `/v1/search` fallback) for
any `query=` serves `search.json`, and `/v2/suggestions/` serves `suggestions.json`. Scenarios
search for `fixture`.

- Top hits: Fixture Artist (artist 7), Fixture Album (album 77), Fixture 1001 (track), Fixture Mixtape (playlist)
- Tracks: 1001-1007, all of them
- Albums: 77, 78
- Artists: 7, 8
- Playlists: Fixture Mixtape, Synthetic Selections
- Videos: none, except for a query containing `video` (e.g. `fixture video`), which serves
  `search_video.json`: the same results plus video 9001 (see "Videos")
- Suggestions: "fixture", "fixture album", "fixture artist", plus the same four direct hits

## Catalog

| Kind | Id | Name | Contents |
|---|---|---|---|
| Artist | 7 | Fixture Artist | album 77; 1.2K fans; "Fans also like" Mock Ensemble |
| Artist | 8 | Mock Ensemble | album 78; "Fans also like" Fixture Artist |
| Album | 77 | Fixture Album (ALBUM, 2024-01-05, HI_RES_LOSSLESS) by Fixture Artist | 1001, 1002, 1003, 1004 |
| Album | 78 | Queue Suite (EP, 2024-03-15, LOSSLESS) by Mock Ensemble | 1005, 1006, 1007 |
| Playlist | `11111111-2222-3333-4444-555555555555` | Fixture Mixtape (USER, owned by user 424242 "fixture") | 1001, 1002, 1005, 1006 |
| Playlist | `66666666-7777-8888-9999-aaaaaaaaaaaa` | Synthetic Selections (EDITORIAL, creator TIDAL id 0) | 1003, 1004, 1007 |
| Mix | `000fixture0history0mix0000001` | April 2024 (HISTORY_MONTHLY_MIX, "Fixture Artist, Mock Ensemble and more") | 1001, 1005, 1002, 1006 |

Lyrics: 1001 (4 lines) and 1005 (3 lines) have synced lyrics (`subtitles` in LRC form, lines
spread evenly over the track, provider "Fixture Lyrics"). 1002-1004, 1006 and 1007 have none,
which TIDAL says with a 404 (`subStatus` 2001, "Lyrics not available"): the fake answers that
404 on purpose, so it shows as `"status": 404, "handled": true` in flows.jsonl and as an
`[api_get_body] ... 404` ERROR line in sone.log, and the panel shows "No lyrics available for
this track". Credits: every track has Main Artist (its album artist), Composer "Fixture
Composer" and Producer "Synthetic Producer".

No catalog track carries `mixes.TRACK_MIX`, so there is no track radio and the drawer's
"Suggested tracks" tab stays empty. (Adding it would change every track object, and with it
existing goldens.)

### Explore

`pages/explore` has three pill rows and an icon grid; every link is a page below.

| Row | Links (title -> page) | "View all" |
|---|---|---|
| Genres | Fixture Pop -> `genre_fixture_pop`, Synthetic Jazz -> `genre_synthetic_jazz` | `genre_page` |
| Moods & Activities | Focus -> `mood_focus`, Late Night -> `mood_late_night` | `moods_page` |
| Decades | 2020s -> `m_2020s` | `decades_page` |
| icon grid (PAGE_LINKS "Shortcuts", title not shown) | New -> `explore_new_music`, Top -> `explore_top`, HiRes -> `hires` | - |

"View all" pages hold one untitled PAGE_LINKS_CLOUD row with the same links (the app shows them
as a text grid). Each content page has four rows titled "{page title} Playlists / Albums /
Tracks / Artists" (PLAYLIST_LIST, ALBUM_LIST, TRACK_LIST, ARTIST_LIST) drawn from the catalog
above; see `EXPLORE_CONTENT` in gen.py for which items each page holds.

### Feed

`/v2/feed/activities` has three activities, all dated in 2024, so the page groups them under
"Older" whatever the date: NEW_HISTORY_MIX "April 2024" (unseen, 2024-05-01), NEW_ALBUM_RELEASE
Queue Suite (unseen, 2024-03-15) and Fixture Album (seen, 2024-01-05).
`stats.totalNotSeenActivities` is 2, so the sidebar Feed bell shows its unread dot until the
Feed page is opened (that sends `PUT /v2/feed/activities/seen`, answered 200 `{}`).

Track titles and durations come from `../tracks.json` (Fixture 1001-1004, Queue A/B/C).

The built-in `/v1/tracks/{id}` handler in `fake_tidal.py` agrees with this: 1001-1004 report
album 77 by artist 7, and 1005-1007 report album 78 "Queue Suite" by artist 8 "Mock Ensemble".
(All goldens were re-recorded after this was made consistent.)

## Views and the endpoints behind them

| View | Requests (all GET, host api.tidal.com) | File(s) |
|---|---|---|
| Home | `/v2/home/feed/{slug}` (any slug; one tab "For You" = STATIC) | `home_feed.json`: shortcuts grid, "Fixture Albums", "Fixture Playlists", "Fixture Artists", "Fixture Tracks" |
| Feed page / sidebar Feed bell dot | `/v2/feed/activities`; `PUT /v2/feed/activities/seen` on opening the page | `feed_activities.json`, `feed_seen.json` |
| Mix page (feed mix row, its Play button) | `/v1/pages/mix?mixId=000fixture0history0mix0000001` | `mix_history.json` (MIX_HEADER + TRACK_LIST) |
| Explore | `/v1/pages/explore` | `explore.json` |
| Explore sub-pages | `/v1/pages/{name}` for the pages in the Explore table | `explore_{name}.json` |
| Lyrics tab (player bar "Lyrics") | `/v1/tracks/{id}/lyrics` | `lyrics_1001.json`, `lyrics_1005.json`; every other id gets `lyrics_none.json` with status 404 |
| Credits tab | `/v1/tracks/{id}/credits` (plus `/v1/artists/{id}/bio`) | `credits_{id}.json` |
| Scrobble enrichment (after a catalog track plays a while) | `GET musicbrainz.org/ws/2/isrc/{isrc}` | `musicbrainz_isrc_none.json`, status 404 (synthetic ISRCs are unknown) |
| Sidebar / library playlists | `/v2/my-collection/playlists/folders`, `.../folders/flattened` | `playlist_folders.json` (Fixture Mixtape) |
| User playlists (MCP `get_user_playlists`) | `/v1/users/424242/playlists`, `/v1/users/424242/playlistsAndFavoritePlaylists` | `user_playlists*.json` |
| Search bar dropdown | `/v2/suggestions/` | `suggestions.json` |
| Search page | `/v2/search`, `/v1/search` | `search.json` |
| Album page | `/v1/pages/album?albumId=77` or `78` | `album_page_77.json`, `album_page_78.json` |
| Album detail/tracks (MCP, play buttons) | `/v1/albums/{id}`, `/v1/albums/{id}/tracks` | `album_{id}.json`, `album_{id}_tracks.json` |
| Artist page | `/v2/artist/7`, `/v2/artist/8` | `artist_page_{id}.json`: Top tracks, Albums, Playlists with ..., Fans also like |
| Artist v1 extras | `/v1/artists/{id}`, `/toptracks`, `/albums`, `/bio` | `artist_{id}*.json` |
| Playlist page | `/v1/playlists/{uuid}`, `/v1/playlists/{uuid}/items`, `/v1/playlists/{uuid}/recommendations/items` (empty) | `playlist_p1*.json`, `playlist_p2*.json` |

## routes.json

This is a list of `{"method", "host", "path_regex", "file"}` entries, plus an optional
`"query_contains"` and an optional `"status"` (default 200). The first match wins. `path_regex` is searched against the URL path without
its query. `query_contains` set to `"key=value"` must match one query parameter exactly, for
example `albumId=77`. A value without `=` is matched as a substring of the raw query. `file` is
relative to this directory, and the response is `application/json` with the entry's status.

Not served: artist "view all" pages (`/v2/artist/.../view-all`). The artist sections carry no
`viewAll` link, so the UI never asks for them. Also not served: videos, track radio
(`mixes.TRACK_MIX`) and mixes other than the feed's history mix.

## Not in the catalog: settings, account, profile

Nothing else is needed for these. Opening the Account menu, the Settings sheet (all nine tabs)
and the profile page ("Fixture User" in the Account menu) makes no request beyond what start-up
already makes: `/v1/users/424242` (built into fake_tidal.py: Fixture User, username "fixture",
no `artistId`, so the profile page is the minimal one) and `/v1/sessions`.

## Sign-in (logged-out start)

Start with a profile that has no `auth_tokens` (verify.py's BASE_PROFILE without that key). The
login screen's "Login Code" tab, then "Get Login Code", runs the device code flow, which
fake_tidal.py answers with state (not routes.json):

1. `POST auth.tidal.com/v1/oauth2/device_authorization` returns a fresh `deviceCode`
   (`fake-device-1`, `-2`, ... per call), `userCode` "FAKE1",
   `verificationUri` "link.tidal.com", `interval` 1 (the app polls every 5 s anyway).
2. The first `POST /v1/oauth2/token` for that code answers 400 `authorization_pending`, as if
   the user had not approved yet; the app keeps showing the code and polling.
3. The next poll gets the tokens (`access_token` "fake-access", `user_id` 424242). Any other
   grant (refresh_token, PKCE authorization_code, an unknown device code) gets them at once.
4. The app then calls `/v1/sessions` and loads the library and Home as on a logged-in start;
   it lands on Home about 10 s after "Get Login Code".

The app also opens `https://link.tidal.com/FAKE1` in the default browser. In the sandbox that
launches whatever browser is on PATH, which cannot reach anything (the sandbox network is loopback only).
The embedded PKCE webview ("Sign in with TIDAL") is not supported.

## Videos

| Id | Title | Artist | Stream |
|---|---|---|---|
| 9001 | Fixture Video (Music Video, 6 s, MP4_720P) | Fixture Artist (7) | `fixtures/video/master.m3u8` |

`/v1/videos/9001` serves `video_9001.json`. Every video's `playbackinfopostpaywall` (built into
`fake_tidal.py`) answers TIDAL's "emu" manifest, base64 JSON whose one URL is
`https://fixtures.sone.test/video/master.m3u8`: an HLS master playlist with 360p and 720p
variants, each three MPEG-TS segments (2 s, 2 s, 1.96 s) of H.264 (SMPTE bars with the frame
number, 25 fps, a keyframe every second) and AAC (a 440 Hz tone, 48 kHz stereo), made with
GStreamer's `hlssink2`. The fixture host adds `Access-Control-Allow-Origin: *` to playlists and
segments, as TIDAL's CDN does. The React build's web view still can't play it inside the
sandbox ("Unable to play this video", without a request), so there's no golden for video;
`verifier/video_check.py` checks the Rune player on its own.

## Playlist edits

`POST /v1/playlists/{uuid}/items` (built into `fake_tidal.py`) answers `{"addedItemIds": [...]}`
as TIDAL does: with `onDupes=SKIP` it leaves out ids the playlist's catalog items, or an earlier
add in the same session, already hold; with `onDupes=ADD` it adds every id asked for. Fixture
Mixtape holds 1001, 1002, 1005 and 1006, so adding 1003 adds it and adding 1001 adds nothing.
The playlist pages don't change after an add.

With `SPIKE_PLAYLIST_VIDEO=1` in the environment, Fixture Mixtape's items also end with video
9001 (wrapper `type` "video"), for a playlist that holds a music video. Unset (the default),
nothing changes.
