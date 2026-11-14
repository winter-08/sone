# States benchmark 2026-11-14-094713 (regions-tip-debug)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 300 tracks (served `limit` rows per request, as TIDAL). Total wall time 3 min 27 s.

Builds: `tip-debug` = `rune-app/target/stage-tip/debug/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1', 'RUNE_REGION_MISS': '1', 'RUST_BACKTRACE': '1'}

## GPU

| State | took | fps tip-debug | views/s tip-debug | polls/s tip-debug | CPU % tip-debug | PSS MB tip-debug |
|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 1/1 | 0.1 | 0.1 | 5.1 | 0.3 | 182 |
| Player closed, playing | 1/1 | 1.0 | 1.0 | 5.0 | 1.3 | 206 |
| Player open, playing | 1/1 | 1.0 | 1.0 | 5.0 | 1.4 | 218 |
| Player open, paused | 1/1 | 0.0 | 0.0 | 5.0 | 0.2 | 220 |
| Playing on Home (region check) | 1/1 | 1.0 | 1.0 | 5.0 | 1.3 | 223 |
| Playing on a playlist page (region check) | 1/1 | 1.0 | 1.0 | 5.0 | 0.9 | 228 |
| Playing on a playlist, now-playing drawer open (region check) | 1/1 | 62.1 | 1.0 | 62.0 | 6.8 | 228 |
| Big list open, playing (row shows playing bars) | 1/1 | 62.1 | 1.1 | 62.1 | 9.8 | 233 |
| Big list, wheel-scrolling, playing | 1/1 | 62.0 | 2.8 | 62.0 | 17.2 | 237 |
| Big list open, paused | 1/1 | 0.0 | 0.0 | 5.0 | 0.2 | 237 |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Playing on Home (region check) | 1.0 |
| Playing on a playlist page (region check) | 1.0 |
| Playing on a playlist, now-playing drawer open (region check) | 62.1 |
| Big list open, playing (row shows playing bars) | 62.1 |
| Big list, wheel-scrolling, playing | 62.0 |
| Big list open, paused | 0.0 |

**Unchanged frames/s**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Playing on Home (region check) | 0.0 |
| Playing on a playlist page (region check) | 0.0 |
| Playing on a playlist, now-playing drawer open (region check) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 |
| Big list open, paused | 0.0 |

**Skipped frames/s**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Playing on Home (region check) | 0.0 |
| Playing on a playlist page (region check) | 0.0 |
| Playing on a playlist, now-playing drawer open (region check) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 |
| Big list open, paused | 0.0 |

**View runs/s**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Playing on Home (region check) | 1.0 |
| Playing on a playlist page (region check) | 1.0 |
| Playing on a playlist, now-playing drawer open (region check) | 1.0 |
| Big list open, playing (row shows playing bars) | 1.1 |
| Big list, wheel-scrolling, playing | 2.8 |
| Big list open, paused | 0.0 |

**Polls/s**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 5.1 |
| Player closed, playing | 5.0 |
| Player open, playing | 5.0 |
| Player open, paused | 5.0 |
| Playing on Home (region check) | 5.0 |
| Playing on a playlist page (region check) | 5.0 |
| Playing on a playlist, now-playing drawer open (region check) | 62.0 |
| Big list open, playing (row shows playing bars) | 62.1 |
| Big list, wheel-scrolling, playing | 62.0 |
| Big list open, paused | 5.0 |

**Regions/s (perf line, newer builds only)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Playing on Home (region check) | 1.0 |
| Playing on a playlist page (region check) | 1.0 |
| Playing on a playlist, now-playing drawer open (region check) | 1.0 |
| Big list open, playing (row shows playing bars) | 1.1 |
| Big list, wheel-scrolling, playing | 0.9 |
| Big list open, paused | 0.0 |

**CPU, % of a core (perf line)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.3 |
| Player closed, playing | 1.3 |
| Player open, playing | 1.4 |
| Player open, paused | 0.2 |
| Playing on Home (region check) | 1.3 |
| Playing on a playlist page (region check) | 0.9 |
| Playing on a playlist, now-playing drawer open (region check) | 6.8 |
| Big list open, playing (row shows playing bars) | 9.8 |
| Big list, wheel-scrolling, playing | 17.2 |
| Big list open, paused | 0.2 |

**Compositor CPU, % of a core**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.1 |
| Player open, playing | 0.2 |
| Player open, paused | 0.1 |
| Playing on Home (region check) | 0.1 |
| Playing on a playlist page (region check) | 0.1 |
| Playing on a playlist, now-playing drawer open (region check) | 1.0 |
| Big list open, playing (row shows playing bars) | 1.0 |
| Big list, wheel-scrolling, playing | 1.2 |
| Big list open, paused | 0.1 |

**RSS MB (perf line)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 292 |
| Player closed, playing | 319 |
| Player open, playing | 331 |
| Player open, paused | 335 |
| Playing on Home (region check) | 338 |
| Playing on a playlist page (region check) | 343 |
| Playing on a playlist, now-playing drawer open (region check) | 343 |
| Big list open, playing (row shows playing bars) | 348 |
| Big list, wheel-scrolling, playing | 352 |
| Big list open, paused | 352 |

**PSS MB (smaps_rollup)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 182 |
| Player closed, playing | 206 |
| Player open, playing | 218 |
| Player open, paused | 220 |
| Playing on Home (region check) | 223 |
| Playing on a playlist page (region check) | 228 |
| Playing on a playlist, now-playing drawer open (region check) | 228 |
| Big list open, playing (row shows playing bars) | 233 |
| Big list, wheel-scrolling, playing | 237 |
| Big list open, paused | 237 |

**GPU memory MiB (nvidia-smi, app)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 96 |
| Player closed, playing | 96 |
| Player open, playing | 160 |
| Player open, paused | 160 |
| Playing on Home (region check) | 160 |
| Playing on a playlist page (region check) | 160 |
| Playing on a playlist, now-playing drawer open (region check) | 160 |
| Big list open, playing (row shows playing bars) | 160 |
| Big list, wheel-scrolling, playing | 160 |
| Big list open, paused | 160 |

**Machine busy % during window (all 24 cores)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 2.1 |
| Player closed, playing | 1.7 |
| Player open, playing | 1.8 |
| Player open, paused | 1.8 |
| Playing on Home (region check) | 1.7 |
| Playing on a playlist page (region check) | 1.7 |
| Playing on a playlist, now-playing drawer open (region check) | 2.4 |
| Big list open, playing (row shows playing bars) | 2.2 |
| Big list, wheel-scrolling, playing | 2.4 |
| Big list open, paused | 1.7 |

**perf.drawn** (perf line field)

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.5 |
| Player closed, playing | 5 |
| Player open, playing | 5 |
| Player open, paused | 0 |
| Playing on Home (region check) | 5 |
| Playing on a playlist page (region check) | 5 |
| Playing on a playlist, now-playing drawer open (region check) | 310.5 |
| Big list open, playing (row shows playing bars) | 310.5 |
| Big list, wheel-scrolling, playing | 310 |
| Big list open, paused | 0 |

**perf.drawn_per_s** (perf line field)

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1 |
| Player open, playing | 1 |
| Player open, paused | 0 |
| Playing on Home (region check) | 1 |
| Playing on a playlist page (region check) | 1 |
| Playing on a playlist, now-playing drawer open (region check) | 62.1 |
| Big list open, playing (row shows playing bars) | 62.1 |
| Big list, wheel-scrolling, playing | 62 |
| Big list open, paused | 0 |

**perf.skipped** (perf line field)

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0 |
| Player closed, playing | 0 |
| Player open, playing | 0 |
| Player open, paused | 0 |
| Playing on Home (region check) | 0 |
| Playing on a playlist page (region check) | 0 |
| Playing on a playlist, now-playing drawer open (region check) | 0 |
| Big list open, playing (row shows playing bars) | 0 |
| Big list, wheel-scrolling, playing | 0 |
| Big list open, paused | 0 |

**perf.unchanged** (perf line field)

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | 0 |
| Player closed, playing | 0 |
| Player open, playing | 0 |
| Player open, paused | 0 |
| Playing on Home (region check) | 0 |
| Playing on a playlist page (region check) | 0 |
| Playing on a playlist, now-playing drawer open (region check) | 0 |
| Big list open, playing (row shows playing bars) | 0 |
| Big list, wheel-scrolling, playing | 0 |
| Big list open, paused | 0 |

**Busiest threads (median % of a core)**

| State | tip-debug |
|---|---|
| Idle on Home, nothing played | – |
| Player closed, playing | sone-rune 0.9 |
| Player open, playing | sone-rune 0.9 |
| Player open, paused | – |
| Playing on Home (region check) | sone-rune 0.8 |
| Playing on a playlist page (region check) | sone-rune 0.6 |
| Playing on a playlist, now-playing drawer open (region check) | sone-rune 6.5 |
| Big list open, playing (row shows playing bars) | sone-rune 9.6 |
| Big list, wheel-scrolling, playing | sone-rune 16.9 |
| Big list open, paused | – |

</details>

| Timing (s) | tip-debug |
|---|---|
| Launch → window mapped (first frame) | 2.351 |
| Launch → MCP answers | 1.419 |
| Launch → Home button on AT-SPI | 2.447 |
| Launch → Home content on AT-SPI | 2.499 |
| Close request → process exited | 3.723 |
| Big list: press → its heading on AT-SPI | 0.203 |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- tip-debug run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 353 MB

Region rebuilds (RUNE_REGION_MISS) per second in the kept window, by region (first stale read):

- tip-debug run0 idle_home: total 0.0/s: none
- tip-debug run0 closed_playing: total 1.0/s: `nowbar` 1.0/s (state 13 ×10)
- tip-debug run0 open_playing: total 1.0/s: `nowbar` 1.0/s (state 13 ×10)
- tip-debug run0 open_paused: total 0.0/s: none
- tip-debug run0 region_home: total 1.0/s: `nowbar` 1.0/s (state 13 ×10)
- tip-debug run0 region_playlist: total 1.0/s: `nowbar` 1.0/s (state 13 ×10)
- tip-debug run0 region_drawer: total 1.0/s: `nowbar` 1.0/s (state 13 ×10)
- tip-debug run0 biglist_playing: total 1.1/s: `nowbar` 1.1/s (state 13 ×11)
- tip-debug run0 biglist_scroll: total 0.9/s: `nowbar` 0.9/s (state 13 ×9)
- tip-debug run0 biglist_paused: total 0.0/s: none

Sessions (machine load, 1-min loadavg at start → end):

- tip-debug: 09:47:13 8.9→0.8 (207 s)

