# States benchmark 2026-11-14-095043 (sanity-tip)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 3 min 45 s.

Builds: `tip` = `rune-app/target/stage-tip/release/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1'}

## Software rendering (pixman sway, lavapipe)

| State | took | fps tip | views/s tip | polls/s tip | CPU % tip | PSS MB tip |
|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 1/1 | 0.1 | 0.1 | 5.1 | 0.6 | 304 |
| Player closed, playing | 1/1 | 1.0 | 1.0 | 5.0 | 6.7 | 312 |
| Player open, playing | 1/1 | 1.0 | 1.0 | 5.0 | 27.5 | 373 |
| Player open, paused | 1/1 | 0.0 | 0.0 | 5.0 | 0.1 | 373 |
| Player open, playing, window unfocused (visible) | 1/1 | 1.0 | 1.0 | 5.0 | 27.7 | 371 |
| Playing, window covered by a fullscreen window | 1/1 | 0.0 | 0.0 | 4.0 | 0.3 | 362 |
| Playing, window on a hidden workspace | 1/1 | 0.0 | 0.0 | 4.0 | 0.3 | 374 |
| Playing, window in the scratchpad (hidden) | 1/1 | 0.0 | 0.0 | 4.0 | 0.3 | 362 |
| Big list open, playing (row shows playing bars) | 1/1 | 62.4 | 1.0 | 62.4 | 246.3 | 395 |
| Big list, wheel-scrolling, playing | 1/1 | 62.5 | 1.0 | 62.4 | 261.0 | 396 |
| Big list open, paused | 1/1 | 0.0 | 0.0 | 5.0 | 0.0 | 391 |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | 1.0 |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 62.4 |
| Big list, wheel-scrolling, playing | 62.5 |
| Big list open, paused | 0.0 |

**Unchanged frames/s**

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | 0.0 |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 |
| Big list open, paused | 0.0 |

**Skipped frames/s**

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | 0.0 |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 |
| Big list open, paused | 0.0 |

**View runs/s**

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | 1.0 |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 1.0 |
| Big list, wheel-scrolling, playing | 1.0 |
| Big list open, paused | 0.0 |

**Polls/s**

| State | tip |
|---|---|
| Idle on Home, nothing played | 5.1 |
| Player closed, playing | 5.0 |
| Player open, playing | 5.0 |
| Player open, paused | 5.0 |
| Player open, playing, window unfocused (visible) | 5.0 |
| Playing, window covered by a fullscreen window | 4.0 |
| Playing, window on a hidden workspace | 4.0 |
| Playing, window in the scratchpad (hidden) | 4.0 |
| Big list open, playing (row shows playing bars) | 62.4 |
| Big list, wheel-scrolling, playing | 62.4 |
| Big list open, paused | 5.0 |

**Regions/s (perf line, newer builds only)**

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | 1.0 |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 1.0 |
| Big list, wheel-scrolling, playing | 1.0 |
| Big list open, paused | 0.0 |

**CPU, % of a core (perf line)**

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.6 |
| Player closed, playing | 6.7 |
| Player open, playing | 27.5 |
| Player open, paused | 0.1 |
| Player open, playing, window unfocused (visible) | 27.7 |
| Playing, window covered by a fullscreen window | 0.3 |
| Playing, window on a hidden workspace | 0.3 |
| Playing, window in the scratchpad (hidden) | 0.3 |
| Big list open, playing (row shows playing bars) | 246.3 |
| Big list, wheel-scrolling, playing | 261.0 |
| Big list open, paused | 0.0 |

**Compositor CPU, % of a core**

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | 0.0 |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.2 |
| Big list, wheel-scrolling, playing | 0.4 |
| Big list open, paused | 0.0 |

**RSS MB (perf line)**

| State | tip |
|---|---|
| Idle on Home, nothing played | 319 |
| Player closed, playing | 329 |
| Player open, playing | 390 |
| Player open, paused | 390 |
| Player open, playing, window unfocused (visible) | 390 |
| Playing, window covered by a fullscreen window | 379 |
| Playing, window on a hidden workspace | 391 |
| Playing, window in the scratchpad (hidden) | 379 |
| Big list open, playing (row shows playing bars) | 412 |
| Big list, wheel-scrolling, playing | 414 |
| Big list open, paused | 415 |

**PSS MB (smaps_rollup)**

| State | tip |
|---|---|
| Idle on Home, nothing played | 304 |
| Player closed, playing | 312 |
| Player open, playing | 373 |
| Player open, paused | 373 |
| Player open, playing, window unfocused (visible) | 371 |
| Playing, window covered by a fullscreen window | 362 |
| Playing, window on a hidden workspace | 374 |
| Playing, window in the scratchpad (hidden) | 362 |
| Big list open, playing (row shows playing bars) | 395 |
| Big list, wheel-scrolling, playing | 396 |
| Big list open, paused | 391 |

**Machine busy % during window (all 24 cores)**

| State | tip |
|---|---|
| Idle on Home, nothing played | 1.6 |
| Player closed, playing | 2.0 |
| Player open, playing | 3.0 |
| Player open, paused | 2.1 |
| Player open, playing, window unfocused (visible) | 2.8 |
| Playing, window covered by a fullscreen window | 1.8 |
| Playing, window on a hidden workspace | 1.7 |
| Playing, window in the scratchpad (hidden) | 1.9 |
| Big list open, playing (row shows playing bars) | 12.4 |
| Big list, wheel-scrolling, playing | 12.9 |
| Big list open, paused | 1.8 |

**perf.drawn** (perf line field)

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.5 |
| Player closed, playing | 5 |
| Player open, playing | 5 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | 5 |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 312 |
| Big list, wheel-scrolling, playing | 312.5 |
| Big list open, paused | 0 |

**perf.drawn_per_s** (perf line field)

| State | tip |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1 |
| Player open, playing | 1 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | 1 |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 62.4 |
| Big list, wheel-scrolling, playing | 62.5 |
| Big list open, paused | 0 |

**perf.skipped** (perf line field)

| State | tip |
|---|---|
| Idle on Home, nothing played | 0 |
| Player closed, playing | 0 |
| Player open, playing | 0 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | 0 |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 0 |
| Big list, wheel-scrolling, playing | 0 |
| Big list open, paused | 0 |

**perf.unchanged** (perf line field)

| State | tip |
|---|---|
| Idle on Home, nothing played | 0 |
| Player closed, playing | 0 |
| Player open, playing | 0 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | 0 |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 0 |
| Big list, wheel-scrolling, playing | 0 |
| Big list open, paused | 0 |

**Busiest threads (median % of a core)**

| State | tip |
|---|---|
| Idle on Home, nothing played | – |
| Player closed, playing | llvmpipe-# 6 |
| Player open, playing | llvmpipe-# 26.8 |
| Player open, paused | – |
| Player open, playing, window unfocused (visible) | llvmpipe-# 27.2, sone-rune 0.5 |
| Playing, window covered by a fullscreen window | – |
| Playing, window on a hidden workspace | – |
| Playing, window in the scratchpad (hidden) | – |
| Big list open, playing (row shows playing bars) | llvmpipe-# 232.9, sone-rune 13.2 |
| Big list, wheel-scrolling, playing | llvmpipe-# 241.3, sone-rune 19.4 |
| Big list open, paused | – |

</details>

| Timing (s) | tip |
|---|---|
| Launch → window mapped (first frame) | 2.987 |
| Launch → MCP answers | 1.436 |
| Launch → Home button on AT-SPI | 3.081 |
| Launch → Home content on AT-SPI | 3.133 |
| Close request → process exited | 2.369 |
| Big list: press → its heading on AT-SPI | 0.209 |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- tip run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 416 MB

Sessions (machine load, 1-min loadavg at start → end):

- tip: 09:50:43 0.8→1.7 (225 s)

