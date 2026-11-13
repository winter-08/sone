# SONE playback / queue behavioural spec (for the pure-Rust UI port)

Source of truth: the React frontend in `src/` as of this writing. All references are
`file:line` relative to the repo root (`/home/winter/dev/sone`). Abbreviations:

- `UPA` = `src/hooks/usePlaybackActions.ts`
- `AI`  = `src/components/AppInitializer.tsx`
- `GP`  = `src/hooks/useGaplessPrefetch.ts`
- `MCPB`= `src/hooks/useMcpBridge.ts`
- `PS`  = `src/hooks/usePlaySource.ts`
- `POS` = `src/lib/playbackPosition.ts`
- `AT`  = `src/atoms/playback.ts`
- `BE`  = `src-tauri/src/commands/playback.rs`

"invoke X {args}" means a Tauri command call; in the Rust port it becomes a direct call
into the kept backend code (same semantics, same argument values). Argument names
below are the camelCase names the frontend sends (Tauri `rename_all = "camelCase"`).

Toast strings are quoted exactly; reproduce them (or equivalent UI notices).

---------------------------------------------------------------------------------------

## 0. Architecture notes that matter for a faithful port

1. **Single logical player.** In React, `usePlaybackActions()` is instantiated by many
   components, and several pieces of "global" state are actually *per hook instance*
   refs: `playGenerationRef`, `autoplayIdsRef`, `playNextLockRef`, `lastPlayInvokeRef`
   (UPA:225-228). Only `pendingResumeTimer` (UPA:186) and the `userPausedAtom` /
   `consecutiveFailCountAtom` atoms are truly global. The intent everywhere is "one
   player"; the Rust port should use **one** instance of each (one generation counter,
   one play-next lock, one re-entry timestamp, one autoplay-id set). This differs from
   React only in edge cases where two different components raced (e.g. AppInitializer's
   `track-advanced` handler deletes from *its own* `autoplayIdsRef`, AI:946, which is
   not the instance whose `playNext` populated it). A single set is the correct fix.
2. **Event ordering.** JS is single-threaded; every handler runs to its first `await`
   atomically. Rust handlers that mutate state must hold the state lock across the
   same synchronous segments (between awaits) to preserve the guards described below.
3. **"Subscribers".** Many side effects are driven by `store.sub(atom, cb)`, which fires
   whenever the atom's value changes identity. Every `set` of a new array/object fires.
   Treat each as "on change of X, do Y" (an observer/derived-effect layer).
4. **Video items.** Queue items can be music videos (`itemType === "video"`). They are
   played in an HTML `<video>` element, never through the audio backend. If the Rust UI
   does not support video, treat video items as: skipped by gapless (already), and
   either unsupported or handled by a separate player. All video branches are noted
   but described briefly.

---------------------------------------------------------------------------------------

## 1. State model

### 1.1 Track object
`Track` (src/types.ts:75-127). Fields used by playback logic:
`id: number`, `title`, `version?` (display title = `"title (version)"` if version,
src/types.ts:128-134), `artist?: {id,name,...}`, `artists?: [{id,name,type?/artistType?}]`,
`album?: {id,title,cover?,releaseDate?}`, `duration` (seconds), `trackNumber?`,
`volumeNumber?`, `isrc?`, `explicit?`, `ai?`, `streamReady?`, `allowStreaming?`,
`streamStartDate?`, `mixes?: {TRACK_MIX?}`, `itemType?: "track"|"video"`, `imageId?`.

Runtime-only fields attached by the queue logic:
- `_qid: string` — per-queue-instance identity, format `"q<N>"` from a global
  monotonically increasing counter (src/lib/qid.ts:3-16). `stampQid` always assigns a
  fresh qid; `ensureQid` keeps an existing one. Duplicate track ids in a queue are
  distinguished **only** by `_qid`.
- `_source?: ManualTrackSource` (`{type,id,name,image?,subtitle?,mixType?}`,
  src/types.ts:136-148) — only on manual-queue items; "playing from" context to switch
  to when this item starts.
- `_playingFrom`, `_contextFrom` — snapshots of `playbackSource`/`contextSource` stamped
  onto the track when it becomes current (UPA:396-397, 639-640, 1412-1413). Used by
  `playPrevious` to restore context. **Stripped on persistence** (AI:157-169), so they
  do not survive a restart.

**normalizeTrack** (UPA:101-107): shallow copy; if `artist` missing and `artists[0]`
exists, set `artist = artists[0]`. Applied to every track entering a queue / becoming
current.

### 1.2 Playback atoms (AT)

| Atom | Type / default | Persisted? | Meaning |
|---|---|---|---|
| `isPlayingAtom` (AT:70) | bool, false | no | UI "playing" flag |
| `currentTrackAtom` (AT:71) | Track\|null | snapshot | the current item |
| `volumeAtom` (AT:72) | f64, **1.0** | localStorage `sone.volume.v1` | linear amplitude 0..1 |
| `queueAtom` (AT:75) | Track[] | snapshot | **context queue** (upcoming from the current collection) |
| `manualQueueAtom` (AT:86) | Track[] | snapshot | **manual "Next up" queue** (user-added); always plays before `queueAtom` |
| `historyAtom` (AT:76) | Track[] | snapshot (capped 500) | previously played, oldest first, newest last |
| `originalQueueAtom` (AT:87) | Track[]\|null | snapshot | un-shuffled order of the context queue while shuffle is on; `null` when shuffle off / not tracked |
| `playbackSourceAtom` (AT:88) | PlaybackSource\|null | snapshot | "Playing from" container. `{type,id,name,image?,subtitle?,mixType?,tracks: QueuedTrack[]}` (src/types.ts:668-676). `tracks` = the full collection (used for repeat-all rebuild and previous-fallback) |
| `contextSourceAtom` (AT:89) | PlaybackSource\|null | snapshot | stashed collection source while a manual item with its own `_source` is playing; restored when the manual queue drains |
| `queueEpochAtom` (AT:91) | int, 0 | no | bumped whenever the context queue is replaced wholesale (cancels background page loaders) |
| `streamInfoAtom` (AT:77) | StreamInfo\|null | no | result of `play_tidal_track`/`set_next_track` for current track |
| `preMuteVolumeAtom` (AT:78) | f64, 0 | no | volume before mute shortcut |
| `autoplayAtom` (AT:79) | bool, **false** | localStorage `sone.autoplay.v1` | radio continuation at end of queue |
| `useTrackGainAtom` (AT:82) | bool, **true** | no | replay-gain mode sent to backend: true = track gain (mixed/shuffled), false = album gain (album in order) |
| `repeatAtom` (AT:84) | int, **0** | localStorage `sone.repeat.v1` | 0 off, 1 repeat-all, 2 repeat-one |
| `shuffleAtom` (AT:85) | bool, false | localStorage `sone.shuffle.v1` | shuffle mode |
| `allowExplicitAtom` (AT:93) | bool, true | `sone.allowExplicit.v1` | content filter |
| `allowAiAtom` (AT:99) | bool, true | `sone.allowAi.v1` | content filter |
| `exclusiveModeAtom`, `bitPerfectAtom`, `gaplessAtom`(default true), `maxQualityAtom`("HI_RES_LOSSLESS"), `exclusiveDeviceAtom`, `volumeNormalizationAtom` (AT:103-111) | — | backend-authoritative, hydrated at login via `get_exclusive_mode`, `get_bit_perfect`, `get_gapless`, `get_max_quality`, `get_exclusive_device`, `get_volume_normalization` (AI:326-343) | audio settings |
| `bitPerfectPreviousStateAtom` (AT:118) | `{volume, volumeNormalization}`\|null | `sone.bitPerfect.previousState.v1` | saved to restore when bit-perfect is turned off |
| `consecutiveFailCountAtom` (AT:128) | int 0 | no | skip-loop failure counter |
| `userPausedAtom` (AT:133) | bool false | no | explicit user pause intent (global) |

Queue display order (what the user sees as "upcoming", what MCP `get_queue` sees,
what `removeFromQueue(index)`/`playFromQueue(index)` index into) is
**`[...manualQueue, ...queue]`** (UPA:922-953, 1700-1711; MCPB:202).

### 1.3 Content filter
`contentBlockReason(item, prefs)` (src/lib/contentFilter.ts:10-18): `"ai"` if
`!allowAi && item.ai === true`, else `"explicit"` if `!allowExplicit && item.explicit === true`,
else null. `blockedMessage`: `"AI content is turned off in Settings"` /
`"Explicit content is turned off in Settings"`.

### 1.4 Unavailability (metadata pre-check)
`isTrackUnavailable(t)` (src/lib/trackAvailability.ts:35-45): false for videos; true if
`streamReady === false` or `allowStreaming === false` or `streamStartDate` parses to a
time in the future. Undefined fields = available.

### 1.5 Persistence of the queue snapshot (AI:571-816)

Snapshot JSON (`PlaybackSnapshot`, src/types.ts:678+):
```json
{ "currentTrack": Track|null, "queue": Track[], "history": Track[] (last 500),
  "manualQueue": Track[], "originalQueue": Track[]|null,
  "playbackSource": {...,"tracks":Track[]}|null, "contextSource": {...}|null }
```
Tracks are serialized with `_qid`/`_source` kept, `_playingFrom`/`_contextFrom` removed.
`isPlaying`, `streamInfo`, position, `useTrackGain` are **not** persisted.

**Write path** (AI:715-793): subscribers on `currentTrack, queue, history, manualQueue,
originalQueue, playbackSource, contextSource` mark dirty; flush coalesced to one per
microtask (i.e., once per synchronous burst of mutations). Each flush:
1. writes JSON to localStorage key **`sone.playback-state.v1`** immediately;
2. (re)starts a **2000 ms debounce**; on expiry `invoke save_playback_queue {snapshotJson}`
   (BE:404-411 → `queue.json` in the state dir).
On teardown, a pending (not yet written) JSON is flushed with `save_playback_queue`
immediately (AI:808-814). Persistence subscriptions are installed only **after** restore
finishes (AI:795-797).

Rust port: keep `queue.json` via `save_playback_queue`-equivalent with the same 2 s
debounce + flush on exit. The localStorage copy is only a fallback; a Rust port may use
any secondary store or drop it.

**Restore path** (AI:583-713), at app start (not gated on auth):
1. `invoke load_playback_queue` → if a string, parse and restore; else (or on error)
   fall back to localStorage `sone.playback-state.v1`.
2. For each list: keep only entries with numeric `id`; strip runtime fields; drop
   content-blocked tracks (current prefs). `originalQueue`, `manualQueue`, sources'
   `tracks` get `ensureQid`. History capped to the last 500.
3. Set atoms: currentTrack only if valid and not blocked; queue/history/manualQueue only
   if the field was an array; originalQueue = restored or `null`; sources if present.
4. Advance the global qid counter past every restored `q<N>` (src/lib/qid.ts:18-26).
5. Nothing auto-plays; `isPlaying` stays false. Position is not restored; pressing play
   on a restored track replays it from 0 (see §6.2: `is_track_finished` is true when no
   URI is loaded).

`resetPlaybackForContentChange` (src/lib/playbackReset.ts:17-28), called when the user
turns explicit/AI content **off** while something is playing/queued
(src/components/settings/PlaybackTab.tsx:54-58): `stop_track`, currentTrack=null,
isPlaying=false, queue=[], manualQueue=[], originalQueue=null, history=[], both sources
null, epoch+1.

### 1.6 Other persisted things
- Volume: frontend key `sone.volume.v1` (default 1.0). Backend `set_volume` also writes
  `settings.volume` (BE:356-360).
- Backend `play_tidal_track` writes `settings.last_track_id` (BE:233-237).

---------------------------------------------------------------------------------------

## 2. Starting playback

### 2.1 The core primitive: `playTrack(track, opts)` (UPA:302-488)

opts: `chosenByUser` (default **true**), `skipHistoryPush`, `suppressUnplayableToast`,
`retryOnRateLimit`, `contextFirst`. Returns `PlayResult`:
`{ok:true}` or `{ok:false, reason: "network"|"unplayable"|"transient"|"rate-limited"|"blocked"|"filtered"}`.

Steps, in order:
1. **Content filter**: if blocked → toast `blockedMessage(reason)` (info), return
   `filtered`. Nothing else changes.
2. **Re-entry guard**: if `now - lastPlayInvoke < 250 ms` → return `transient` silently
   (UPA:327-331, `PLAY_REENTRY_GUARD_MS = 250`, UPA:180). Else `lastPlayInvoke = now`.
3. `cancelPendingResume()` — cancels any scheduled rate-limit retry (UPA:332).
4. If `track.itemType === "video"`: generation += 1; userPaused=false; push previous
   current to history (unless `skipHistoryPush`; cap 500); stamp `_playingFrom/_contextFrom`;
   set current; `startVideoSession(...)` (src/lib/videoSession.ts: sets isPlaying=false,
   `stop_track`, fetches `get_video_metadata {videoId}` + `get_video_stream_info
   {videoId, videoQuality:"HIGH"}`). Return ok / transient on failure. **End.**
5. If a video is currently active, clear the video state (mutual exclusion, UPA:369-374).
6. `userPaused = false`; `generation = ++playGeneration`.
7. `stamped = ensureQid(normalizeTrack(track))`; preload cover 640 and 1280 (UI only).
8. Save `previousTrack = current`, `previousHistory = history` for rollback.
9. **Eager UI update**: if previousTrack exists and not `skipHistoryPush`, append it to
   history (cap: keep last **500**, `MAX_HISTORY_TRACKS`, UPA:179). Stamp
   `stamped._playingFrom = playbackSource`, `stamped._contextFrom = contextSource`.
   `markPlaybackLoading(true)` (§8.4). Set `currentTrack = stamped`.
10. `invokePlayWithRetry(stamped.id, useTrackGain)` (UPA:198-219):
    `invoke play_tidal_track {trackId, useTrackGain}`. If the error message is exactly
    `"device_busy"` and attempts remain: on the first retry set isPlaying=false and toast
    `"Preparing exclusive audio…"` (info); sleep **500 ms**; retry. Up to
    **10 retries** (11 attempts, ~5 s) (UPA:177-178). Other errors throw immediately.
11. On success: if `generation != playGeneration` → return `transient` (superseded, do
    not touch state). Else `streamInfo = info`, `isPlaying = true`,
    `consecutiveFailCount = 0`, `markPlaybackLoading(false)`, fire-and-forget
    `invoke notify_track_started {payload}` (§2.6), return ok.
12. On error: if superseded → `transient`. Else **rollback** current := previousTrack,
    history := previousHistory; isPlaying=false; `markPlaybackLoading(false)`. Classify
    (in this order):
    - ProxyBlocked (`{kind:"ProxyBlocked", message:{reason}}`) → toast reason (error),
      return `blocked`.
    - Network (`kind === "Network"`) → `checkNetworkError` (dispatches a global
      "network-error" toast at most once per **30 s**: `"Network error — check your
      internet connection"` or `"... check your proxy settings"` if proxy enabled,
      src/api/tidal.ts:103-119, AI:414-427) → return `network`.
    - Rate-limited (Api status 429) → if `retryOnRateLimit`: `invoke stop_track`
      (fire-and-forget) and `scheduleRateLimitResume(error, track, {contextFirst})`;
      else `scheduleRateLimitResume(error)`. Return `rate-limited`.
    - Unplayable (Api 404/410/451, or 401 whose body JSON `subStatus` ∈
      {4005,4010,4030,4031,4032,4034,4035}; src/lib/trackAvailability.ts:50-80) → toast
      `"Track unavailable"` (info) unless `suppressUnplayableToast`; return `unplayable`.
    - Anything else → "playback-error" toast with `extractPlaybackError(error)`
      (UPA:135-151: proxy reason, else string `message`, else `"Playback failed"`),
      return `transient`.

Errors from the backend are `SoneError` serialized as `{kind, message}`; for `Api`,
`message = {status, body}` (src/lib/errorUtils.ts:42-81, src-tauri/src/error.rs:5-10).
In Rust, match on the `SoneError` variant directly.

**Quality selection** is entirely backend-side (BE:27-86): tiers
`["HI_RES_LOSSLESS","HI_RES","LOSSLESS","HIGH"]` filtered to ≤ the `max_quality`
ceiling, Hi-Res tiers dropped without a client secret; try each until success; network,
429 and terminal-unplayable errors abort the cascade immediately. The frontend never
passes a quality. Replay-gain selection uses `useTrackGain` (BE:129-148).

### 2.2 `scheduleRateLimitResume(error, retryTrack?, retryOpts?)` (UPA:247-288)
- `wait = (retryAfterSecs(error) ?? 5) + 1` seconds, where `retryAfterSecs` reads
  `retryAfterSecs` (number) from the JSON in `message.body`.
- Toast `"Too many requests — resuming in ${wait}s"` (info).
- Capture `gen = playGeneration`; cancel any previous pending resume; arm a single
  global timer for `wait * 1000` ms. On fire:
  - abort if `gen != playGeneration`, or `userPaused`, or a video is current;
  - no `retryTrack` → `playNext()` (non-explicit);
  - with `retryTrack`: abort if play-next lock is held; else take the lock,
    `playTrack(retryTrack, {chosenByUser:true, retryOnRateLimit:true, contextFirst})`,
    release lock; if result was `unplayable` **and** `contextFirst` →
    `playNext({explicit:true, contextFirst:true})`.
- `cancelPendingResume()` is called by: every `playTrack` (step 3), `clearQueue`,
  `mpris:stop` (AI:1155).

### 2.3 Queue-building primitives

**`setQueueTracks(tracks, options)`** (UPA:801-852):
- `options.reorder` (drag-and-drop): `stamped = tracks.map(ensureQid∘normalize)`;
  manualQueue = first `manualCount`, queue = rest. No epoch bump, no other change. Return.
- Otherwise: epoch += 1; drop content-blocked; `useTrackGain = !albumMode`;
  `originalQueue = null`; if `clearManual` → manualQueue = []; `contextSource = null`;
  `playbackSource = source ? {type,id,name,image,subtitle,mixType, tracks: stampQids(allTracks.map(normalize))} : null`;
  `queue = stampQids(eligible.map(normalize))` (fresh qids).
  Note: unavailable tracks are **not** filtered here (the skip-loop handles them).
  Shuffle atom is not touched.

**`setShuffledQueue(tracks, options)`** (UPA:1655-1698): epoch += 1; drop blocked;
stamp fresh qids; if `clearManual` → manualQueue=[]; `contextSource=null`;
`originalQueue = shuffleAtom ? stamped : null` (i.e. the un-shuffled order is kept only
if global shuffle is already on — it does **not** turn shuffle on); `queue =
fisherYates(stamped)`; `useTrackGain = !albumMode`; `playbackSource` as above.

**Fisher–Yates** (UPA:168-175): standard, i from n-1 down to 1, `j = floor(rand*(i+1))`.

**`playSingle(track)`** (UPA:854-865): if blocked → toast, `filtered`. Else
`setQueueTracks([])` (no source, **manual queue kept**, context emptied, playbackSource
null, useTrackGain=true, epoch+1) then `playTrack(track, {retryOnRateLimit:true})`.
Used by deep links / `mpris:open-uri` `tidal://track/<id>` (AI:557-560:
`get_track {trackId}` then `playSingle`).

**`playFromSource(track, allTracks, options)`** (UPA:1747-1792) — "clicked a track in a
collection":
1. If `track` blocked → toast, return (queue untouched).
2. `eligible = allTracks minus blocked`; `idx` = index of first `t.id === track.id`.
3. `rest = idx>=0 ? [eligible[idx+1..], eligible[0..idx)] (wrap-around) : eligible without track.id`.
4. If shuffle on → `setShuffledQueue(rest, options)` else `setQueueTracks(rest, options)`.
   **No `clearManual`** → the manual queue survives a track click.
5. `consecutiveFailCount = 0`; `playTrack(track, {retryOnRateLimit:true, contextFirst:true})`;
   if `unplayable` → `playNext({explicit:true, contextFirst:true})`.

**`playAllFromSource(allTracks, options)`** (UPA:1794-1844) — "Play"/"Shuffle" button,
MCP `play_now`:
1. `eligible = allTracks` minus unavailable minus blocked. If empty → return **false**
   (nothing changed).
2. `consecutiveFailCount = 0`.
3. `clearManual = source.type ∉ {"artist","artist-tracks"}` (UPA:182) — so no-source
   (MCP play_now), album, playlist, mix, favorites **clear the manual queue**; artist
   sources keep it.
4. If `options.shuffle || shuffleAtom`: `first = eligible[floor(rand*len)]`,
   `setShuffledQueue(rest, ...)`. (`options.shuffle` alone does **not** enable shuffle
   mode; then `originalQueue = null`.) Else `first = eligible[0]`,
   `setQueueTracks(rest, ...)`.
5. `playTrack(first, {retryOnRateLimit:true, contextFirst:true})`; if `unplayable` →
   `playNext({explicit:true, contextFirst:true})`. Return true.

**`addToQueue(track, source?)`** (UPA:777-785): ignore if blocked; append
`stampQid(normalize(track))` (+ `_source` if given) to **manualQueue**.
**`playNextInQueue(track, source?)`** (UPA:787-795): same but **prepend** to manualQueue.
Neither starts playback, neither filters unavailable.

**`appendToQueue(newTracks, {shuffle?, source?})`** (UPA:867-920) — background paging:
- drop blocked; if none → return. Stamp fresh qids.
- If `source` owner given: if `contextSource` matches `{type,id}` append to its
  `tracks`; else if `playbackSource` matches append there. (Without owner: append to
  playbackSource if non-null.)
- If shuffle mode on or `options.shuffle`: if shuffle mode on and originalQueue non-null,
  append the new tracks to originalQueue (in order). Insert each new track into queue at
  `idx = queue.empty ? 0 : 1 + floor(rand * queue.length)` (never before the head).
- Else append to the end of queue. No epoch bump.

**`removeFromQueue(index)`** (UPA:922-953): index into `[...manual, ...queue]`. Manual →
remove from manual only. Context → remove from queue and remove the same `_qid` from
originalQueue.

**`playFromQueue(index)`** (UPA:1700-1745): bounds-check; if unavailable → toast
`"Track unavailable"`, return; if blocked → toast, return; `consecutiveFailCount=0`;
remove the item (manual: from manual; context: from queue and originalQueue by `_qid`);
`playTrack(track, {retryOnRateLimit:true})`. Note: does **not** apply a manual item's
`_source` (only `playNext` does) and does not remove the items before it.

**`clearQueue()`** (UPA:1846-1854): cancelPendingResume; epoch+1; queue=[],
manualQueue=[], originalQueue=null, playbackSource=null, contextSource=null.
**Does not stop** the current track or touch history/current.

### 2.4 Playing a whole source with paging: `usePlaySource(src, {shuffle?, startAt?})` (PS:257-401)
`PlayableSource` = `{meta:{type,id,name,image?,subtitle?,mixType?}, loaded: Track[],
fetchPage?: (offset)=>{items,hasMore}, nextOffset?, hasMore?, include?, dedupe?, albumMode?}`
(src/lib/trackSources.ts:19-40).
1. If `startAt` blocked → toast, return false.
2. `request = ++latestRequest` (global); remember `epochAtRequest`, `shuffleModeAtStart`.
3. `take(items)`: if `dedupe`, skip ids already seen; apply `include`; collect; count
   playable (available and not blocked).
4. Take `loaded`. `offset = nextOffset ?? loaded.length`; `hasMore = fetchPage && (hasMore ?? true)`.
5. Unless `startAt` is already in the collected tracks: fetch pages while `hasMore` and
   playable count < **50** (`START_BATCH`). After each page: `hasMore = page.hasMore &&
   page.items.length > 0 && pages < 200 (MAX_PAGES) && (!dedupe || newIds > 0)`.
   Abort (return false) if a newer request started or the epoch changed. A fetch error
   stops paging (`failed = true`).
6. If superseded → false. If no tracks → toast `"Couldn't load tracks"` (error) only if
   failed; return false.
7. `source = {...meta, allTracks: tracks}`. With `startAt`:
   `noWrap = hasMore && idx > 0 && !shuffleAtom`; `playFromSource(startAt, noWrap ?
   tracks.slice(idx) : tracks, {source, albumMode})`. Without: `started =
   playAllFromSource(tracks, {source, albumMode, shuffle})`; if false → toast the first
   content-block reason found, else `"No playable tracks"` (info); return false.
8. Background: `ourEpoch = epochAtRequest + 1`; while `hasMore` and epoch still equals
   `ourEpoch`: fetch next page, drop unavailable, `appendToQueue(playable, {shuffle:
   opts.shuffle && !shuffleModeAtStart, source:{type,id}})`. Errors end silently.
9. Return true.

Page sizes (src/lib/trackSources.ts:42-45): artist top tracks 50
(`get_artist_top_tracks_all {artistId, offset, limit}`; `hasMore = items.length >= limit`),
playlist 100 (`get_playlist_tracks_page {playlistId, offset, limit, order, orderDirection}`;
`hasMore = offset + items.length < totalNumberOfItems`), favorites 100, favorite videos 50.
Album: `get_album_page {albumId}` (`page.tracks`, complete, `albumMode: true`).
Mix: `get_mix_items {mixId}` (complete).

UI entry points (for reference): album/playlist/mix/artist/favorites pages call
`playSource(src, {startAt})` on row click, `playSource(src)` on Play, `playSource(src,
{shuffle:true})` on Shuffle. Home sections call `playFromSource` directly. Context menus
call `addToQueue`/`playNextInQueue` with a `_source` tag. Card play
(`useMediaPlay`, src/hooks/useMediaPlay.ts:21-58) has its own **250 ms** double-click
guard, plays videos via `playSingle`, otherwise `playableFromMedia` → `playSource`;
on error toast `"Failed to play"`.

### 2.5 MCP-originated playback (MCPB:218-347)
- **`mcp:play-tracks` `{trackIds:number[], action}`** (MCPB:221-251): fetch every id in
  parallel with `invoke get_track {trackId}` (src/api/tidal.ts:863-865); keep fulfilled
  results **in original order**, drop failures; if none → return silently.
  - `"play_now"` → `playAllFromSource(tracks)` with **no options**: no source
    (playbackSource=null), clears the manual queue, `useTrackGain=true`, random first
    track if shuffle mode is on. If every track is unavailable/blocked it returns false
    **silently** (no toast).
  - `"queue"` → `addToQueue(t)` for each in order (append to manual, no `_source`).
  - `"play_next"` → iterate **reversed**, `playNextInQueue(t)` → tracks end up at the
    front of the manual queue in their original order.
  - queue/play_next never start playback even if idle.
- **`mcp:play-source` `{sourceType, id: string}`** (MCPB:253-266): build a
  `PlayableSource` via `playableForSource` (MCPB:54-131), then `playSource(playable)`:
  - `playlist`: `get_playlist_details {playlistId}` for name/image (on failure name
    `"Playlist"`), `loaded: []`, playlist pager.
  - `album`: `get_album_page {albumId: Number(id)}`, `loaded = page.tracks`, name =
    album title, image = cover, `albumMode: true`.
  - `artist`: `get_artist_detail {artistId}` (fallback name `"Artist"`), subtitle
    `"Top tracks"`, artist pager, `dedupe: true`.
  - `mix`: `get_mix_items {mixId}`; name `title ?? "Mix"`, image, mixType.
- **`mcp:shuffle-source`** (MCPB:268-286): same source building; if null return;
  `prevShuffle = shuffleAtom`; **write `shuffleAtom = true` directly** (no
  `toggleShuffle`, so the existing queue is not permuted and no originalQueue is saved
  from it); `playSource(playable)`; if it returned false restore `shuffleAtom =
  prevShuffle`. Because shuffle is now on, `playAllFromSource` picks a random first track
  and `setShuffledQueue` keeps `originalQueue`; background pages are inserted randomly
  and appended to originalQueue.

### 2.6 `notify_track_started` payload (UPA:112-132)
```
{ artist: artists.map(name).join(", ") || artist.name || "Unknown Artist",
  artistPrimary: first artist with (artistType ?? type) == "MAIN", else artists[0], else artist.name, else "Unknown Artist",
  title: track.title,             // raw title, no version
  album: album.title || null, albumArtist: null,
  durationSecs: duration || 0, trackNumber: trackNumber || null,
  chosenByUser, isrc: isrc || null, trackId: id || null,
  sourceType: playbackSource?.type ?? null, sourceId: playbackSource ? String(id) : null }
```
`chosenByUser` per call site: `playTrack` default **true**; manual-queue drain
`!!explicit`; context drain `!isAutoplayTrack` (so normal auto-advance through a
collection reports **true**); repeat-all rebuild true; autoplay radio first track
**false**; repeat-one replay **false**; resume-after-finished replay true;
`playPrevious` true; gapless `advanceToTrack` **false**.

---------------------------------------------------------------------------------------

## 3. Advancing

### 3.1 Backend event `track-finished` (no payload) (AI:985-993)
Emitted on terminal EOS only (errors emit `audio-error`). Handler:
`streamInfo = null; playNext()` (non-explicit). Backend separately scrobbles
(src-tauri/src/lib.rs:793-804).

### 3.2 `playNext({explicit?, contextFirst?})` (UPA:955-1340)
1. If play-next lock held → **return (drop the call)**. Take lock. `userPaused = false`.
   Lock is released in `finally`.
2. **Repeat-one** (`repeat === 2`) and **not explicit**: if a current track exists:
   - video: rewind the element to 0 and play (or start a video session). Return.
   - audio: in-place replay. `markPlaybackLoading(true)`; `invokePlayWithRetry(current.id,
     useTrackGain)`; on success `streamInfo = info`, `isPlaying = true`, `notifySeek(0)`,
     notify_track_started (chosenByUser **false**). **No history push, currentTrack
     unchanged, queue untouched, no `stop_track` first.** On error:
     `markPlaybackLoading(false)`, isPlaying=false, then network → checkNetworkError;
     429 → `scheduleRateLimitResume(error)` (resume = playNext → lands here again);
     unplayable → toast `"Track unavailable"`; else playback-error toast. Return.
   - If no current track, fall through to normal advance.
3. `invoke stop_track` (errors ignored) — always, before advancing (UPA:1041).
4. If `explicit` → `consecutiveFailCount = 0`.
5. `recordUnplayableAndCheckCap()`: count += 1; if count ≥ **3**
   (`MAX_CONSECUTIVE_PLAY_FAILS`, UPA:97) → toast `"Multiple tracks failed to play —
   stopped"` (error), count = 0, isPlaying=false, **stop (return)**. Else toast
   `"Track unavailable — skipping"` (info) at most once per playNext call.
6. `skipManual = contextFirst && queue has some track that is available and not blocked`.
7. **Manual drain** (unless skipManual), while manualQueue non-empty, head = `next`:
   - blocked → pop it, continue (no counter);
   - unavailable → pop, `recordUnplayable…` (stop if cap), continue;
   - pop it. If it has `_source`: save prev playbackSource/contextSource; if
     `contextSource` is null, stash `contextSource = playbackSource`; set
     `playbackSource = {..._source, tracks: []}`.
   - reset re-entry guard (`lastPlayInvoke = 0`); `playTrack(next, {chosenByUser:
     !!explicit, suppressUnplayableToast:true})`.
   - ok → return. `unplayable` → roll back the source change, record (stop if cap),
     continue. Any other failure (network/blocked/transient/rate-limited/filtered) →
     roll back source change, **re-insert `next` at the head of manualQueue**, return.
8. **Restore stashed context**: if `contextSource` non-null → `playbackSource =
   contextSource; contextSource = null`.
9. **Context drain**, while queue non-empty, head = `next`, `isAutoplay =
   autoplayIds.has(next.id)`:
   - blocked → remove id from autoplayIds, pop, remove `_qid` from originalQueue, continue;
   - unavailable → same + `recordUnplayable…`, continue;
   - pop, remove id from autoplayIds, remove `_qid` from originalQueue (remember old
     `orig`); reset re-entry guard; `playTrack(next, {chosenByUser: !isAutoplay,
     suppressUnplayableToast:true})`.
   - ok → return. `unplayable` → record, continue. Other failure → re-insert `next` at
     queue head, restore originalQueue to `orig`, return.
10. If `skipManual` and manualQueue non-empty → release lock and
    `playNext({explicit:true})` (manual queue is used after the collection is exhausted),
    return.
11. **End of queue**:
    - **Repeat-all** (`repeat === 1`) (UPA:1227-1283): `repeatSource = contextSource ??
      playbackSource`; if it has non-empty `tracks` → `raw = source.tracks` (the whole
      collection, including the just-finished track) and `hasSource = true`; else
      `raw = [...history, current?]`. Filter blocked and unavailable; stamp fresh qids
      → `all`. If `all` non-empty: `ordered = shuffle ? fisherYates(all) : all`;
      `first = ordered[0]`; `queue = ordered[1..]`; `originalQueue = shuffle ?
      all.filter(qid != first.qid) : null`; if `!hasSource` → `history = []`;
      `playTrack(first, hasSource ? default : {skipHistoryPush:true})`. If unplayable →
      release lock, `playNext()` (non-explicit). If rate-limited → `requeueHead(first)`
      (UPA:292-300: prepend to queue and, if non-null, originalQueue). If `all` empty →
      isPlaying=false.
      Note: after `playSingle`/MCP `play_now` (playbackSource null) repeat-all replays
      the entire history (up to 500 tracks) + current, then clears history.
    - **Autoplay** (`autoplayAtom`) (UPA:1284-1331): requires current track. `historyIds
      = history ids ∪ {current.id}`. If `current.mixes.TRACK_MIX` is missing →
      **return without setting isPlaying=false** (quirk: stays "playing" though the
      pipeline was stopped in step 3). Else `getMixItems(TRACK_MIX)` (`invoke
      get_mix_items {mixId}`, cached): `fresh = tracks not in historyIds, not blocked,
      not unavailable`. If non-empty: `[next, ...rest] = fresh`; `autoplayIds =
      set(rest ids)` (replaced); epoch+1; `queue = stampQids(rest)`; `useTrackGain =
      true`; **playbackSource is not changed**; `playTrack(next, {chosenByUser:false})`;
      unplayable → release lock, `playNext()`; rate-limited → `requeueHead(next)`;
      return. On fetch error: network → checkNetworkError; 429 →
      `scheduleRateLimitResume(error)`. Fall through: isPlaying=false.
    - **Otherwise**: isPlaying=false. currentTrack stays as the last track; the
      pipeline is stopped (step 3). Pressing play then replays that track from 0.

Consequence: an **explicit skip at the end of the queue** (repeat off, autoplay off)
stops playback (stop_track) and sets isPlaying=false.

### 3.3 Gapless prefetch (GP:31-192, src/lib/gaplessPredict.ts)

Maintains the backend's "next track" slot so the backend can switch without a gap and
emit `track-advanced` instead of `track-finished`.

State: `pendingNext: {trackId, qid, track, streamInfo} | null` (shared with the
`track-advanced` handler), `gen`, `inFlight`, `queued`, `failed: {trackId, qid, until}`,
module-level `cachedSupported`.

`refresh()` (GP:50-141):
1. `gen = ++genCounter`. If `cachedSupported` unknown → `invoke get_gapless_supported`
   (false on error), cached for the process lifetime.
2. `enabled = supported && gaplessAtom && !exclusiveMode && !bitPerfect && no current
   video && currentTrack != null`. **Not** gated on isPlaying (a slot armed while paused
   stays armed).
3. If not enabled → `clearSlot()` (pendingNext = null; `invoke clear_next_track`, errors
   ignored); return.
4. `next = pickGaplessNext(...)` (src/lib/gaplessPredict.ts:209-224):
   null if `repeat === 2`; `head = manualQueue[0] ?? queue[0]`; null if no head, head
   unavailable, head content-blocked, head is a video, or head has `_source` whose `id
   !== playbackSource?.id` (loose identity: compares `id` only). Else head. If null →
   clearSlot, return.
5. `qid = next._qid ?? String(next.id)`.
6. Dedup: if `pendingNext.trackId == next.id && pendingNext.qid == qid` → return.
7. Failure memo: if `failed` matches `(trackId, qid)` and `now < until` → return.
8. If in flight → `queued = true`, return (coalesce).
9. `inFlight = true`; `invoke set_next_track {trackId: next.id, qid, useTrackGain}`
   (BE:242-263). On success: if `gen` superseded → return (no record); else
   `failed = null`, `pendingNext = {trackId, qid, track: next, streamInfo: info}`. On
   error: if superseded return; else `failed = {trackId, qid, until: now + 10 000 ms}`
   (`FAILURE_MEMO_MS`). Finally: `inFlight = false`; if `queued` → `queued = false`,
   `refresh()`.

Triggers (GP:171-191):
- **Debounced 250 ms** (each trigger restarts the timer): changes of manualQueue,
  queue, currentTrack (also clears `failed`), repeat, shuffle, autoplay.
- **Immediate**: changes of exclusiveMode, bitPerfect, gapless.
- Once on mount.
- Proxy settings saved (window event `sone:proxy-saved`): `pendingNext = null`, then
  immediate refresh (backend detached the prerolled branch; force re-arm).

Backend notes: `set_next_track` dedups by `track_id` (only updates stored qid)
(src-tauri/src/audio.rs:3507-3519) and is a no-op unless gapless+normal mode.
`play_tidal_track` (PlayUrl) and `stop_track` **discard** the backend slot
(audio.rs ~2175-2181, 2939-2946) but the frontend's `pendingNext` is not cleared. If
after a play the predicted next is unchanged (same trackId and qid, e.g.
`playFromQueue(k>0)` with an untouched head), refresh dedups and does not re-arm, so
that one boundary falls back to `track-finished → playNext` (audible but correct).
A port may replicate this or clear `pendingNext` whenever it calls
`play_tidal_track`/`stop_track` (strictly better; no other behaviour depends on it).

### 3.4 Backend event `track-advanced` `{trackId, qid, replayGain, peakAmplitude}` (AI:902-978)
Emitted by the audio thread when it switched gaplessly to the armed next track
(src-tauri/src/audio.rs:3642-3650). **Never gated on the play-next lock**; always
reconciles ("audio reality wins").
1. `pending = pendingNext; pendingNext = null`.
2. `qidOf(t) = t._qid ?? String(t.id)`.
3. If manualQueue contains `qidOf == qid` → remove exactly that one item from
   manualQueue (originalQueue untouched).
4. Else: find in queue by qid; if not found, fall back to the **first** item with
   `id == trackId`. If found: remove it; remove items with `qidOf == qid` from
   originalQueue (if non-null). In both cases (found or not) remove `trackId` from
   autoplayIds.
5. If `pending && pending.trackId == trackId` → `advanceToTrack(pending.track,
   pending.streamInfo)`. Else (stash miss): in parallel `invoke get_stream_info
   {trackId, useTrackGain}` (pure resolve, BE:274-283; must **not** use set_next_track)
   and `invoke get_track {trackId}`, then `advanceToTrack(track, info)`; on failure log
   and give up.

Note: the manual item's `_source` is not applied here (pickGaplessNext only arms items
whose `_source.id` equals the current source id, or untagged ones).

**`advanceToTrack(track, info)`** (UPA:623-662): generation += 1 (aborts any in-flight
playTrack); `stamped = ensureQid(normalize(track))`; push previous current to history
(cap 500; always — no skipHistoryPush); stamp `_playingFrom/_contextFrom`; current =
stamped; streamInfo = info; **isPlaying = !userPaused**; consecutiveFailCount = 0;
`markPlaybackLoading(false)`; notify_track_started (chosenByUser **false**). No
`stop_track`/`play_tidal_track`. The prefetch hook then re-arms the following track
(current/queue changed).

### 3.5 `audio-error` event `{kind, message?}` (AI:1023-1049)
isPlaying=false. If `kind` is `"device_disconnected"` or `"playback_error"` →
streamInfo=null. Toast: `"device_busy"` → `"Audio device is busy — close other apps
using it"`; otherwise message truncated to 80 chars + `"…"`, or `"Playback error"`.
No queue change, no auto-advance.

Other informational events: `audio-resampled {from,to}` → toast `"DAC doesn't support
<from> — resampling to <to>"` (rates formatted `X kHz` as `${r/1000}kHz` if ≥1000 else
`${r}Hz`); `audio-bit-depth-changed {from,to}` → `"DAC doesn't support <from> — playing
as <to> (lossless)"`; `signal-path-changed` → store payload; `scrobble-auth-error
<provider>` → `"<Last.fm|ListenBrainz|Libre.fm> session expired — reconnect in
Scrobbling settings"` (AI:1054-1113).

---------------------------------------------------------------------------------------

## 4. Next / previous

### 4.1 Next
All user/remote "next" actions call `playNext({explicit: true})`: player-bar button,
keyboard shortcut, `tray:next-track` (tray menu, MPRIS Next, global media key),
`mcp:skip-next`, miniplayer. Explicit means: repeat-one does **not** replay (it
advances), the fail counter is reset, and manual items report `chosenByUser = true`.
Automatic advance (`track-finished`, rate-limit resume, video `ended`) uses
`playNext()` (non-explicit). Behaviour at queue end: §3.2 step 11 (repeat-all loops,
autoplay radio, else stop).

### 4.2 Previous: `playPrevious()` (UPA:1344-1629)
Callers: player-bar, shortcut, `tray:prev-track` (tray, MPRIS Previous, media key),
`mcp:skip-previous`, miniplayer.
1. If play-next lock held → return. Take lock (released in finally).
2. `pos = getInterpolatedPosition()` (§8.4). If **`pos > 3`** seconds (strict) →
   `seekTo(0)`, return. (Applies regardless of history.)
3. `consecutiveFailCount = 0`; `invoke stop_track` (ignored errors); clear any active video.
4. **History non-empty**: `prev = history.pop()`.
   - Snapshot current, queue, originalQueue, manualQueue, playbackSource, contextSource
     for rollback.
   - `history = history minus last`.
   - If current exists: prepend it to **manualQueue**, tagged `_source = {type,id,name,
     image,subtitle,mixType}` of the current playbackSource (untagged if none). It keeps
     its `_qid`. (So "next" after "previous" returns to it and restores its context via
     the manual-drain `_source` logic.)
   - If `prev._playingFrom !== undefined`: `playbackSource = prev._playingFrom`,
     `contextSource = prev._contextFrom ?? null`. (Lost after restart because runtime
     fields are stripped.)
   - Re-stamp `prev._playingFrom/_contextFrom` from the now-current sources.
   - `markPlaybackLoading(true)`; current = prev. **Current is not pushed to history.**
   - Video → start video session, return.
   - `invokePlayWithRetry(prev.id, useTrackGain)` → streamInfo, isPlaying=true,
     `markPlaybackLoading(false)`, notify_track_started (true).
   - On error: restore **all** snapshotted state incl. history; isPlaying=false;
     `markPlaybackLoading(false)`; network → checkNetworkError; 429 → toast `"Too many
     requests — try again in ${retryAfterSecs ?? 5}s"` (no auto-retry); unplayable →
     `"Track unavailable"`; else playback-error toast. Playback remains stopped.
5. **History empty** — source fallback: if playbackSource and current exist:
   `idx = source.tracks.findIndex(id == current.id)`; `p = idx-1`, walk back while
   `source.tracks[p]` is blocked or unavailable. If `idx > 0 && p >= 0`:
   - `prev = stampQid(source.tracks[p])` (fresh qid).
   - If manualQueue non-empty → prepend current (tagged with current source) to manual.
     Else prepend current to **queue**, and if originalQueue non-null insert current
     before the first originalQueue element whose position in `source.tracks` is greater
     than current's (append if none; if current not found in source, prepend).
   - `markPlaybackLoading(true)`; current = prev (no history push, no `_playingFrom`
     stamping). Video → session.
   - Play as above; on error restore current, queue, originalQueue, manualQueue (not
     sources/history); isPlaying=false; network/unplayable/other toasts (no special 429
     message here).
   - Else (first playable track of the source) → `seekTo(0)`.
   Otherwise (no source) → if current, `seekTo(0)`.
   **Quirk**: in these `seekTo(0)` fallbacks `stop_track` was already issued in step 3,
   so the seek targets a stopped pipeline; isPlaying is not changed (UI may still say
   playing while audio is stopped). Faithful port: reproduce, or (recommended) skip the
   `stop_track` in that path. Flag to product owner.

---------------------------------------------------------------------------------------

## 5. Shuffle and repeat

### 5.1 `toggleShuffle()` (UPA:1631-1653)
Callers: player-bar, maximized player, shortcut, `mcp:toggle-shuffle`,
`mpris:set-shuffle` (only if the requested value differs), miniplayer.
- **Turning on**: `originalQueue = queue` (same array, current order);
  `queue = fisherYates(queue)`; `shuffle = true`. Manual queue and current track are
  untouched (current keeps playing; only upcoming context items are permuted).
- **Turning off**: if originalQueue non-null: `queue = originalQueue filtered to `_qid`s
  still present in queue` (restores original relative order of the remaining items;
  items appended to queue but missing from originalQueue would be dropped — appends
  always add to originalQueue while shuffled, so in practice none). `originalQueue =
  null`; `shuffle = false`. If originalQueue was null the queue order is left as is.
- Shuffle mode also affects: `playFromSource`/`playAllFromSource` (random first +
  shuffled queue + originalQueue saved), `appendToQueue` (random insertion), repeat-all
  rebuild (reshuffled each loop).
- Changing shuffle triggers a debounced gapless refresh and `update_mpris_shuffle`.

### 5.2 Repeat (`repeatAtom`: 0 off, 1 all, 2 one)
Cycling (player bar, maximized player, shortcut, miniplayer): `(repeat + 1) % 3`
(src/components/PlayerBar.tsx:605, AI:1376-1378). Set directly by `mcp:set-repeat`
(`off→0, all→1, one→2`, unknown ignored) and `mpris:set-loop-status <u8>` (written
as-is; backend maps None/Playlist/Track → 0/1/2, src-tauri/src/mpris.rs:156-163).
- **Off**: at track end advance; at queue end autoplay (if on) or stop (§3.2).
- **All**: at track end advance normally (and gaplessly); only when both queues are
  exhausted, rebuild from source/history (§3.2 step 11). Explicit skip behaves the same.
- **One**: on `track-finished` (non-explicit `playNext`) replay the current track in
  place from 0 (§3.2 step 2). Gapless is disabled (pickGaplessNext returns null →
  slot cleared), so the boundary always goes through `track-finished`. Explicit skip
  (next button etc.) advances to the next item as if repeat were off at that moment;
  at the end of the queue with repeat-one, step 11's repeat-all branch is not taken
  (mode ≠ 1), so autoplay or stop applies.
- Previous is unaffected by repeat.

---------------------------------------------------------------------------------------

## 6. Pause / resume / seek / stop / volume

### 6.1 `pauseTrack()` (UPA:492-505)
Video current → pause the video element, return. Else `userPaused = true`;
`invoke pause_track`; on success isPlaying=false (on error log only, isPlaying unchanged).

### 6.2 `resumeTrack()` (UPA:507-586)
`userPaused = false`. Video current → play element. If current is a video with no live
session → start session. Else, if no current → return.
`invoke is_track_finished` (true when EOS reached **or no URI loaded**, i.e. after
stop/end-of-queue/restart; audio.rs:3129-3133).
- Finished → `markPlaybackLoading(true)`; `invokePlayWithRetry(current.id, useTrackGain)`;
  streamInfo=info; `notifySeek(0)`; notify_track_started (true). Note: **no history push,
  current unchanged**.
- Not finished → `invoke resume_track`.
- Then isPlaying=true.
- On error: isPlaying=false; `markPlaybackLoading(false)`; network → checkNetworkError;
  429 → toast `"Too many requests — try again in ${s ?? 5}s"`; unplayable →
  `"Track unavailable"`; else playback-error toast.

### 6.3 `togglePlayPause()` (UPA:588-601)
Decide from (video element paused state | videoPlaying | isPlayingAtom); playing →
pause, else resume. Callers: play button, shortcut, `tray:toggle-play` (tray, MPRIS
PlayPause, global media key), miniplayer.

### 6.4 `seekTo(secs)` (UPA:768-775)
`invoke seek_track {positionSecs}`; on success `notifySeek(secs)` (§8.4). No clamping.
Callers: progress bar, lyrics click, `mcp:seek` (u32 seconds), `mpris:set-position`
(clamped ≥ 0), miniplayer. `mpris:seek` is relative (AI:1282-1289): `cur = invoke
get_playback_position`; `new = max(0, cur + offset)`; `invoke seek_track`; `notifySeek`
(not clamped to duration; errors swallowed). Backend `seek_track` itself notifies
MPRIS `Seeked` and Discord (BE:376-393).

### 6.5 Stop
There is no UI "stop" button. `mpris:stop` (AI:1154-1158): `cancelPendingResume()`;
`invoke stop_track`; isPlaying=false. currentTrack and queues are kept. Backend
`stop_track` also clears MPRIS/Discord now-playing (BE:328-336).

### 6.6 Volume: `setVolume(level)` (UPA:664-682)
- `bitPerfect && !videoCurrent` → **ignore entirely** (atom not changed).
- Set `volumeAtom = level` (persisted `sone.volume.v1`).
- If `bitPerfect` (only reachable while a video is current) → return without invoking
  (video element volume only).
- `invoke set_volume {level}` (backend applies, notifies MPRIS volume, persists
  `settings.volume`).
Callers / derived values: slider (0..1, wheel step **0.05** rounded to 2 decimals;
mute button sets 0 or 1; disabled while bit-perfect and no video,
src/components/VolumeSlider.tsx:23-73); shortcuts volume up/down **±0.1** clamped
(AI:1355-1356); mute shortcut: if vol>0 save `preMuteVolume = vol`, set 0, else set
`preMuteVolume || 0.5` (AI:1357-1365); `mpris:set-volume` clamped to [0,1]
(AI:1290-1292); `mcp:set-volume {level}` (server already clamps to [0,1]).

**Startup volume sync** (AI:872-893): once at mount, `bitPerfect = invoke get_bit_perfect`
(fallback atom), and if not bit-perfect `invoke set_volume {level: persisted volume}`.

**Bit-perfect toggle** `setBitPerfect(enabled)` (UPA:715-766), no-op if unchanged:
- On: save `{volume, volumeNormalization}` to `sone.bitPerfect.previousState.v1`;
  `rampVolume(volume → 1.0)`; volumeNormalization=false + `invoke
  set_volume_normalization {enabled:false}`; bitPerfect=true + `invoke set_bit_perfect
  {enabled:true}`.
- Off: bitPerfect=false + `invoke set_bit_perfect {enabled:false}`; if saved state:
  `rampVolume(current → saved.volume)`, restore normalization (+invoke), clear saved.
- `rampVolume(from,to, 300 ms, 12 steps)` (UPA:696-713): skip if |Δ| < 1e-4; for i=1..12
  set atom and `invoke set_volume` with `from+(to-from)*i/12`, sleeping 25 ms between
  steps.
Shortcut `toggleBitPerfect` also enables exclusive mode if off (and picks the first
`list_audio_devices` entry if no device set); toasts `"Bit-perfect on — takes effect
next track"` / `"... off ..."`. `toggleExclusive` turning off also disables bit-perfect
(AI:1383-1432).

`setVolumeNormalization(enabled)` (UPA:684-694): set atom, `invoke
set_volume_normalization {enabled}`.

---------------------------------------------------------------------------------------

## 7. Event handling (backend → frontend)

### 7.1 MCP events (payloads from src-tauri/src/mcp/events.rs; emitted by
src-tauri/src/mcp/tools/*.rs). MCP tools only emit; they never touch playback directly.

| Event | Payload (camelCase JSON) | MCP tool | Frontend action (MCPB) |
|---|---|---|---|
| `mcp:play-tracks` | `{trackIds: u64[], action: "play_now"\|"queue"\|"play_next"}` (validated non-empty / valid action server-side) | `play_tracks` | §2.5 (MCPB:221-251) |
| `mcp:play-source` | `{sourceType: "playlist"\|"album"\|"artist"\|"mix", id: string}` | `play_source` | §2.5 (MCPB:253-266) |
| `mcp:shuffle-source` | same as play-source | `shuffle_source` | §2.5 (MCPB:268-286) |
| `mcp:pause` | `null` | `pause` | `pauseTrack()` |
| `mcp:resume` | `null` | `resume` | `resumeTrack()` |
| `mcp:skip-next` | `null` | `skip_next` | `playNext({explicit:true})` |
| `mcp:skip-previous` | `null` | `skip_previous` | `playPrevious()` |
| `mcp:seek` | `{positionSeconds: u32}` | `seek_to` | `seekTo(positionSeconds)` |
| `mcp:set-volume` | `{level: f32}` (clamped 0..1 server-side) | `set_volume` | `setVolume(level)` (ignored under bit-perfect) |
| `mcp:clear-queue` | `null` | `clear_queue` | `clearQueue()` (does not stop current) |
| `mcp:remove-from-queue` | `{trackId: u64}` | `remove_from_queue` | first index with `id == trackId` in `[...manual, ...queue]` → `removeFromQueue(index)`; no-op if absent (MCPB:331-339) |
| `mcp:set-repeat` | `{mode: "off"\|"all"\|"one"}` | `set_repeat` | `repeat = {off:0,all:1,one:2}[mode]` |
| `mcp:toggle-shuffle` | `null` | `toggle_shuffle` | `toggleShuffle()` |
| `mcp:playlist-created` | full `Playlist` object | `create_playlist` | add to sidebar playlists (dedupe by uuid, prepend), add to root folder list, invalidate `user-playlists` cache, refresh root playlists after 3000 ms (MCPB:399-426) |
| `mcp:playlist-updated` | `{uuid, title: string\|null, description: string\|null}` | `update_playlist` | patch title/description where non-null; invalidate cache (MCPB:428-460) |
| `mcp:playlist-deleted` | `{uuid}` | `delete_playlist` | remove; mark deleted; invalidate caches `user-playlists`, `playlist:<uuid>`, `playlist-page:<uuid>` (MCPB:462-473) |
| `mcp:playlist-tracks-changed` | `{uuid, delta: i32}` | `create_playlist` (when created with tracks), `add_to_playlist`, `remove_track_from_playlist` | `numberOfTracks = max(0, n + delta)`; invalidate; refresh after 3000 ms (MCPB:475-500) |
| `mcp:favorite-changed` | `{kind: "track"\|"album"\|"artist", id: u64, action: "add"\|"remove"}` | `favorite_track`, `favorite_album`, `favorite_artist` | update favorite id sets / caches; album/artist add fetches detail for optimistic lists (MCPB:502-571). Track favourites also feed MPRIS `userRating` (§8.1). |

None of the MCP handlers is debounced; errors are logged and swallowed.

### 7.2 MPRIS events (src-tauri/src/mpris.rs:108-193 → AI:1132-1332)

| D-Bus action | Event emitted | Payload | Frontend action |
|---|---|---|---|
| PlayPause | `tray:toggle-play` | `null` | `togglePlayPause()` |
| Play | `mpris:play` | `null` | if not playing (video: videoPlaying; else isPlaying) → `resumeTrack()` |
| Pause | `mpris:pause` | `null` | if playing → `pauseTrack()` |
| Stop | `mpris:stop` | `null` | cancelPendingResume; `stop_track`; isPlaying=false |
| Next | `tray:next-track` | `null` | `playNext({explicit:true})` |
| Previous | `tray:prev-track` | `null` | `playPrevious()` |
| Seek(offset) | `mpris:seek` | `f64` seconds (relative) | §6.4 |
| SetPosition | `mpris:set-position` | `f64` seconds (absolute) | `seekTo(max(0, s))` |
| Volume set | `mpris:set-volume` | `f64` | `setVolume(clamp(v,0,1))` |
| Shuffle set | `mpris:set-shuffle` | `bool` | if differs from shuffleAtom → `toggleShuffle()` |
| LoopStatus set | `mpris:set-loop-status` | `u8` 0/1/2 | `repeat = payload` |
| OpenUri | `mpris:open-uri` | `string` | deep link: if not authenticated queue it; else `tidal://track/<id>` → `get_track` + `playSingle`; `tidal://album|artist|playlist|mix/<id>` → navigate. Same-URL dedupe vs the last handled deep link (AI:545-562, src/lib/tidalUrl.ts) |
| Fullscreen set | `mpris:set-fullscreen` | `bool` | maximized-player UI flag |
| Raise / Quit | (backend-only) | — | restore window / exit |

### 7.3 Tray / global media keys
`tray:toggle-play`, `tray:next-track`, `tray:prev-track` (no payload), emitted by the
tray menu (src-tauri/src/tray.rs:115-131), MPRIS (above) and global media-key
shortcuts (src-tauri/src/lib.rs:~902-908). Actions: `togglePlayPause()`,
`playNext({explicit:true})`, `playPrevious()` (AI:1133-1141).

### 7.4 Miniplayer (secondary window; may be dropped in the port)
`miniplayer-command {action, value?}` (src/hooks/useMiniplayerEmitter.ts:213-350):
`toggle-play`, `play-next` (explicit), `play-previous`, `toggle-favorite`,
`toggle-shuffle`, `cycle-repeat` ((r+1)%3), `set-volume value`, `seek value`, plus
navigation actions.

---------------------------------------------------------------------------------------

## 8. Outputs pushed to the backend

### 8.1 `update_mpris_metadata {metadata}` (AI:1193-1237)
Fires on mount and on every change of `currentTrack`, `streamInfo`, or favourite track
ids. Skipped if currentTrack is null or a video is current. Payload (BE:420-443):
```
trackId: track.id
title: getTrackDisplayTitle(track)                  // "title (version)"
artist: getTrackArtistDiscordDisplay(track)         // see below
album: track.album?.title || ""
artUrl: getTidalImageUrl(album.cover, 320)          // https://resources.tidal.com/images/<uuid with / instead of ->/320x320.jpg, "" if no cover, passthrough if it starts with http
durationSecs: track.duration
url: "tidal://track/<id>"
qualityText: formatStreamQuality(streamInfo)        // "<bitDepth>-BIT <kHz>KHZ <CODEC>", parts omitted when absent; kHz = sampleRate/1000, 1 decimal if not integral; "" if streamInfo null
albumArtist: track.artist?.name || null
trackNumber: track.trackNumber ?? null
discNumber: track.volumeNumber ?? null
contentCreated: track.album?.releaseDate || null
userRating: favourite ? 1.0 : 0.0
```
`getTrackArtistDiscordDisplay` (src/utils/itemHelpers.ts:451-483): no artists →
`artist.name || "Unknown Artist"`; one → its name; else split by `artistType ===
"MAIN"` (note: only `artistType`, not the runtime `type` field, so usually no MAIN is
found → comma-join of all names); with MAIN: `"<mains, comma>"` or `"<mains> ft.
<featured joined ', ' with ' & ' before the last>"`.
Immediately after each metadata push it also sends `update_mpris_playback_status
{isPlaying}` **without** `positionSecs` (→ Discord receives position 0.0, BE:496-500).
Because playTrack sets `currentTrack` before the new `streamInfo`, a track start
produces a push with the previous track's quality text, then another once streamInfo
arrives; `track-finished` nulls streamInfo → one more push with empty quality.

### 8.2 `update_mpris_playback_status {isPlaying, positionSecs}` (AI:1239-1259)
On mount and on every change of `isPlaying`, `videoPlaying`, or current video.
`isPlaying = video ? videoPlaying : isPlayingAtom`; `positionSecs =
getInterpolatedPosition()` (§8.4). No periodic push — the backend MPRIS task polls
`get_position` itself every 1 s while Playing (src-tauri/src/mpris.rs:200-225).

### 8.3 `update_mpris_shuffle {enabled}` / `update_mpris_loop_status {mode: 0|1|2}` /
`update_mpris_fullscreen {fullscreen}` (AI:1261-1342)
Each pushed once on mount and on every change of the respective atom.

Also `update_tray_tooltip {text}` on mount and every currentTrack change: `"<display
title> — <artists comma-joined>"` or `"Sone"` (AI:1172-1188).

### 8.4 Position: interpolator + `get_playback_position` polling (POS)
Module singleton, initialised once (AI:1505-1508).
- State: `lastKnownPosition`, `lastFetchTime`, `playing`, `trackGeneration`,
  `trackResetTime`, `loadingTrack`.
- `getInterpolatedPosition()` (POS:84-88): if `!playing || loadingTrack` →
  `lastKnownPosition`; else `lastKnownPosition + (now - lastFetchTime)/1000`.
- **Polling**: while isPlaying, `invoke get_playback_position` immediately and then
  every **2000 ms** (POS:63-75). Response discarded if the track changed meanwhile
  (generation check). **Settle window**: within **3000 ms** after a track change, a
  poll value greater than `interpolated + 2 s` is discarded (gapless concat transient)
  (POS:26-29, 49-54). Otherwise anchor `lastKnownPosition = pos, lastFetchTime = now`.
  Errors keep the last value.
- On isPlaying false→true: `lastFetchTime = now`, playing=true, restart polling. On
  true→false: freeze `lastKnownPosition = interpolated`, `lastFetchTime = now`,
  playing=false, stop polling (POS:159-178).
- On currentTrack change: generation++, position=0, `lastFetchTime = now`,
  `trackResetTime = now`, cancel pending seek correction (POS:181-194).
- `notifySeek(t)` (POS:95-121): position=t, `lastFetchTime = now`, `loadingTrack = false`,
  `trackResetTime = 0` (closes settle window); broadcast "playback-seeked" (UI); cancel
  previous correction; schedule one correction poll after **300 ms**.
- `markPlaybackLoading(b)` (POS:132-137): `loadingTrack = b`; when false also
  `lastFetchTime = now`. Set true before an explicit play's currentTrack change and
  false when the backend confirms start (or on failure). Gapless advances never set it
  true.
Consumers: progress UI (polls the interpolator every 500 ms,
src/hooks/useProgressScrub.ts:38), `playPrevious` 3 s threshold, MPRIS status push,
overlay (below), miniplayer (1 s heartbeat).

### 8.5 `mcp_publish_state {nowPlaying, queue}` (MCPB:189-216, BE commands/mcp.rs:35-49)
Sent on mount and whenever any of `currentTrack`, `isPlaying`, `queue`, `manualQueue`
changes (React effect deps; position changes alone never trigger it).
```
nowPlaying = currentTrack ? {
  trackId: id ?? null, title: title ?? "" (raw, no version),
  artist: artists.map(name).join(", ") || artist.name || "Unknown Artist",
  album: album?.title ?? null, durationSeconds: duration ?? 0 (u32 in backend),
  positionSeconds: positionRef, isPlaying } : null
queue = [...manualQueue, ...queue].slice(0, 50).map(t => {id, title: title ?? "", artist: <same artist rule>})
```
**Why `positionSeconds` is always 0**: `positionRef` is only updated by a window
`CustomEvent` named `"sone:playback-position"` (MCPB:176-187), and **nothing in the
codebase dispatches that event** (the interpolator dispatches `"playback-seeked"`
instead, POS:107-109). So `positionRef` stays at its initial 0 forever. Additionally,
even if it were updated, publishes happen only on track/play-state/queue changes, so
the value would be stale. Recommended in the port: publish
`round(getInterpolatedPosition())` and either publish periodically while playing or
have `get_now_playing` compute position on demand.

Backend merge semantics: `nowPlaying: null` is treated as "no update" (`if let
Some(np)`), so after the current track becomes null (content reset, or never set)
`get_now_playing` keeps returning the **last** published track. `queue` is always sent
and replaces the stored list. MCP `get_queue` returns up to `limit` (default 20, max 50)
of this 50-item snapshot with `totalSnapshot` = its length (src-tauri/src/mcp/tools/state.rs).

### 8.6 Overlay (OBS-style overlay; src/hooks/useOverlayBridge.ts:60-115)
When the overlay is enabled (`overlay_get_connection_info`): `invoke
overlay_publish_state {track: {title, artist, album, coverUrl (640), isPlaying,
positionSeconds, durationSeconds, quality} | null}` on change of currentTrack /
isPlaying / streamInfo / enable, every **1000 ms** while playing, and on every seek.
Position = interpolated.

### 8.7 Other outputs
`notify_track_started {payload}` (§2.6). `save_playback_queue {snapshotJson}` (§1.5).
`set_next_track` / `clear_next_track` (§3.3).

---------------------------------------------------------------------------------------

## 9. Other rules a faithful re-implementation must reproduce

### 9.1 Timers / debounces / thresholds (summary)
| Value | Where | Meaning |
|---|---|---|
| 250 ms | UPA:180, 327-331 | playTrack re-entry guard (returns `transient`); reset to 0 by playNext before each drain attempt |
| 250 ms | src/hooks/useMediaPlay.ts:5 | card-play double-click guard |
| 500 ms × 10 | UPA:177-178 | device_busy retry for play_tidal_track |
| 3 failures | UPA:97 | skip-loop cap |
| 500 | UPA:179, AI:146 | history cap (keep newest) |
| `(retryAfterSecs ?? 5) + 1` s | UPA:255 | rate-limit auto-resume delay |
| 3 s (strict >) | UPA:1349 | previous: restart vs go back |
| 250 ms debounce | GP:144-147 | gapless refresh on queue/current/repeat/shuffle/autoplay changes |
| 10 000 ms | GP:29 | gapless set_next_track failure memo |
| 2000 ms | POS:67 | get_playback_position poll while playing |
| 300 ms | POS:118-121 | post-seek correction poll |
| 3000 ms / 2 s | POS:26-29 | settle window / ahead tolerance after track change |
| 2000 ms debounce | AI:767-775 | save_playback_queue |
| 30 s | src/api/tidal.ts:103 | network-error toast cooldown |
| 300 ms / 12 steps | UPA:697 | volume ramp for bit-perfect toggle |
| 50 / 200 | PS:240-241 | start batch of playable tracks / max pages |
| 3000 ms | MCPB:424, 497 | sidebar refresh after MCP playlist mutations |

### 9.2 Generation counter (supersession)
`playGeneration` is incremented by every audio `playTrack` (before the invoke), video
`playTrack`, and `advanceToTrack`. A `play_tidal_track` result that returns after the
generation moved on is ignored entirely (no state writes, no rollback). Rate-limit
resume timers capture the generation and abort if it changed.

### 9.3 Play-next lock
Shared by `playNext`, `playPrevious` and the rate-limit retry-with-track path. A second
call while held is **dropped** (not queued). The only internal re-entries release the
lock first (repeat-all/autoplay unplayable → `playNext()`, contextFirst manual fallback
→ `playNext({explicit:true})`). `track-advanced` ignores the lock. Note: user-initiated
`playTrack` paths (playFromSource, playFromQueue, playSingle) do not take the lock.

### 9.4 Error classification → behaviour matrix
| Reason | In playNext drains | User-initiated play | Repeat-one replay | Previous | Resume |
|---|---|---|---|---|---|
| unplayable | skip, count toward cap, toast once "Track unavailable — skipping" | toast "Track unavailable"; with `contextFirst` → `playNext({explicit,contextFirst})` | toast | toast, rollback | toast |
| rate-limited | re-insert at head, schedule auto `playNext()` | `stop_track`, schedule retry of the same track | schedule `playNext()` (replays) | toast "try again in Ns", rollback | toast |
| network | re-insert, stop advancing, network toast | rollback, network toast | network toast | rollback | network toast |
| blocked (proxy) | re-insert, stop, toast reason | rollback, toast reason | playback-error toast w/ reason | playback-error toast | playback-error toast |
| other | re-insert, stop, "playback-error" toast | rollback, toast | toast | toast | toast |
In all failure cases isPlaying becomes false.

### 9.5 Rollback semantics
`playTrack` failure restores only current + history (queues were already mutated by
the caller; playNext re-inserts the track itself on non-unplayable failures). Previous
restores its full snapshot. Superseded (generation-changed) results never roll back.

### 9.6 Replay-gain mode (`useTrackGain`)
Set false only for album sources played in order via `albumMode` (album page / MCP
album source) — `setQueueTracks`/`setShuffledQueue` set `useTrackGain = !albumMode`
(so a shuffled album still uses album gain if `albumMode` was passed). Set true by any
other queue replacement, `playSingle`, and autoplay radio. Passed to
`play_tidal_track`, `set_next_track`, `get_stream_info`.

### 9.7 `userPaused` intent
Set true by `pauseTrack`; false by `resumeTrack`, `playTrack`, `playNext`. Used by
`advanceToTrack` (gapless advance while paused keeps isPlaying false) and to abort
rate-limit resumes.

### 9.8 Queue epoch
Bumped by `setQueueTracks` (non-reorder), `setShuffledQueue`, `clearQueue`, autoplay
radio queue replacement, content reset. Background paging (`usePlaySource`) stops when
the epoch is no longer `epochAtRequest + 1`; a pending start-batch fetch aborts when it
changes at all. Also a newer `playSource` call (global `latestRequest`) cancels an older
one's start phase.

### 9.9 De-duplication summary
- qids distinguish duplicate track ids everywhere (remove/reconcile by `_qid`).
- `track-advanced` reconciles by qid, falling back to first matching track id in the
  context queue only.
- `mcp:remove-from-queue` removes the first occurrence by track id.
- Gapless `set_next_track` skipped when (trackId, qid) already armed; backend also
  dedups by track id.
- Autoplay radio excludes ids already in history or current.
- Artist top-track paging dedups by id (`dedupe: true`); other sources keep repeats.
- Deep links dedup identical consecutive URLs.

### 9.10 Things that are NOT done (so do not add them silently)
- No crossfade; no position restore across restarts; no automatic retry for network
  errors; no clamping of seek to duration; queue/play_next MCP actions don't start
  playback; clearQueue doesn't stop playback; end-of-queue leaves the last track as
  current; MCP `get_now_playing` position is always 0 (bug, see §8.5).

---------------------------------------------------------------------------------------

## 10. How the Rust port realises the parts added later (rune-app)

These were missing from the first slice; the React behaviour above is the contract, and the
notes say where the port differs on purpose.

- **Content filter (§1.3).** `content.rs` holds `contentBlockReason` / `blockedMessage` /
  `isTrackUnavailable` (the future `streamStartDate` included). The two switches live with the
  controller's other preferences (`rune-ui.json`: `allow_explicit`, `allow_ai`, both default
  true) and are set with `Cmd::SetAllowExplicit` / `Cmd::SetAllowAi`. Turning one off runs
  `resetPlaybackForContentChange` at once; the React settings screen asks for confirmation
  first, so the settings UI must confirm before sending. Every place §2-§4 names a blocked check
  applies it: playTrack (toast, `Filtered`), set queue (blocked dropped), playAllFromSource /
  playFromSource (with usePlaySource's refusal toast), add / play next, both drains, repeat-all,
  previous's source fallback, gapless prediction, playFromQueue, restore.
- **Autoplay radio (§3.2 step 11).** `Player::autoplay_radio`, on `prefs.autoplay`
  (`Cmd::SetAutoplay`). One deliberate difference: a current track without a TRACK_MIX stops
  cleanly (isPlaying false) instead of React's early return that leaves the bar saying "playing"
  over a stopped pipeline.
- **Queue panel edits.** By queue item: `Cmd::PlayQueued(qid)` (playFromQueue),
  `Cmd::RemoveQueued(qid)` (removeFromQueue), `Cmd::MoveQueued { qid, to }` (setQueueTracks
  `reorder`, the manual/context boundary moving as React's drop handler moves it), and
  `Cmd::PlayHistory(qid)` (a History row: playTrack with retryOnRateLimit). Addressing by qid
  instead of index keeps a press meant for one row from landing on another after the queue moved.
  The Suggested tab's row press is `Cmd::QueueTrack(track)` (addToQueue tagged with its album
  source) and its cover `Cmd::PlayTrack(track)`.
- **MCP `play_source` / `shuffle_source` (§2.4-2.5).** `sources.rs` builds the playable source
  (playableForSource) and loads the start batch (50 playable, 200 pages at most, artist pages
  deduplicated) off the actor; the controller then plays it with playAllFromSource and appends
  later pages (appendToQueue) while the queue epoch is the one the source started. The epoch is
  a shared counter bumped wherever §9.8 says; a newer source request supersedes one still
  loading. shuffle_source sets shuffle mode directly and puts it back if nothing starts.
