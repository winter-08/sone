# States benchmark 2026-11-14-011145 (trial-parity)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 3 min 46 s.

Builds: `parity` = `rune-app/target/stage-parity/release/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1'}

## Software rendering (pixman sway, lavapipe)

| State | took | fps parity | views/s parity | polls/s parity | CPU % parity | PSS MB parity |
|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 1/1 | 0.1 | 0.1 | 5.1 | 0.6 | 309 |
| Player closed, playing | 1/1 | 1.0 | 1.0 | 5.0 | 6.8 | 308 |
| Player open, playing | 1/1 | 1.0 | 1.0 | 5.0 | 27.9 | 370 |
| Player open, paused | 1/1 | 0.0 | 0.0 | 5.0 | 0.1 | 369 |
| Player open, playing, window unfocused (visible) | 0/1 | – | – | – | – | – |
| Playing, window covered by a fullscreen window | 1/1 | 0.0 | 0.0 | 4.0 | 0.8 | 358 |
| Playing, window on a hidden workspace | 1/1 | 0.0 | 0.0 | 4.0 | 0.4 | 370 |
| Playing, window in the scratchpad (hidden) | 1/1 | 0.0 | 0.0 | 4.0 | 0.3 | 358 |
| Big list open, playing (row shows playing bars) | 1/1 | 62.4 | 1.4 | 62.4 | 249.6 | 398 |
| Big list, wheel-scrolling, playing | 1/1 | 62.5 | 1.0 | 62.4 | 264.9 | 397 |
| Big list open, paused | 1/1 | 0.0 | 0.0 | 5.0 | 0.0 | 392 |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 62.4 |
| Big list, wheel-scrolling, playing | 62.5 |
| Big list open, paused | 0.0 |

**Unchanged frames/s**

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 |
| Big list open, paused | 0.0 |

**Skipped frames/s**

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 |
| Big list open, paused | 0.0 |

**View runs/s**

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 1.4 |
| Big list, wheel-scrolling, playing | 1.0 |
| Big list open, paused | 0.0 |

**Polls/s**

| State | parity |
|---|---|
| Idle on Home, nothing played | 5.1 |
| Player closed, playing | 5.0 |
| Player open, playing | 5.0 |
| Player open, paused | 5.0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 4.0 |
| Playing, window on a hidden workspace | 4.0 |
| Playing, window in the scratchpad (hidden) | 4.0 |
| Big list open, playing (row shows playing bars) | 62.4 |
| Big list, wheel-scrolling, playing | 62.4 |
| Big list open, paused | 5.0 |

**Regions/s (perf line, newer builds only)**

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 1.0 |
| Player open, playing | 1.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 1.4 |
| Big list, wheel-scrolling, playing | 1.0 |
| Big list open, paused | 0.0 |

**CPU, % of a core (perf line)**

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.6 |
| Player closed, playing | 6.8 |
| Player open, playing | 27.9 |
| Player open, paused | 0.1 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0.8 |
| Playing, window on a hidden workspace | 0.4 |
| Playing, window in the scratchpad (hidden) | 0.3 |
| Big list open, playing (row shows playing bars) | 249.6 |
| Big list, wheel-scrolling, playing | 264.9 |
| Big list open, paused | 0.0 |

**Compositor CPU, % of a core**

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.0 |
| Player closed, playing | 0.0 |
| Player open, playing | 0.0 |
| Player open, paused | 0.0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0.0 |
| Playing, window on a hidden workspace | 0.0 |
| Playing, window in the scratchpad (hidden) | 0.0 |
| Big list open, playing (row shows playing bars) | 0.4 |
| Big list, wheel-scrolling, playing | 0.5 |
| Big list open, paused | 0.0 |

**RSS MB (perf line)**

| State | parity |
|---|---|
| Idle on Home, nothing played | 328 |
| Player closed, playing | 334 |
| Player open, playing | 395 |
| Player open, paused | 395 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 383 |
| Playing, window on a hidden workspace | 396 |
| Playing, window in the scratchpad (hidden) | 384 |
| Big list open, playing (row shows playing bars) | 423 |
| Big list, wheel-scrolling, playing | 426 |
| Big list open, paused | 426 |

**PSS MB (smaps_rollup)**

| State | parity |
|---|---|
| Idle on Home, nothing played | 309 |
| Player closed, playing | 308 |
| Player open, playing | 370 |
| Player open, paused | 369 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 358 |
| Playing, window on a hidden workspace | 370 |
| Playing, window in the scratchpad (hidden) | 358 |
| Big list open, playing (row shows playing bars) | 398 |
| Big list, wheel-scrolling, playing | 397 |
| Big list open, paused | 392 |

**Machine busy % during window (all 24 cores)**

| State | parity |
|---|---|
| Idle on Home, nothing played | 4.4 |
| Player closed, playing | 2.7 |
| Player open, playing | 8.2 |
| Player open, paused | 62.7 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 99.5 |
| Playing, window on a hidden workspace | 13.2 |
| Playing, window in the scratchpad (hidden) | 5.4 |
| Big list open, playing (row shows playing bars) | 13.0 |
| Big list, wheel-scrolling, playing | 15.3 |
| Big list open, paused | 1.6 |

**perf.drawn** (perf line field)

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.5 |
| Player closed, playing | 5 |
| Player open, playing | 5 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 312 |
| Big list, wheel-scrolling, playing | 312.5 |
| Big list open, paused | 0 |

**perf.drawn_per_s** (perf line field)

| State | parity |
|---|---|
| Idle on Home, nothing played | 0.1 |
| Player closed, playing | 1 |
| Player open, playing | 1 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 62.4 |
| Big list, wheel-scrolling, playing | 62.5 |
| Big list open, paused | 0 |

**perf.skipped** (perf line field)

| State | parity |
|---|---|
| Idle on Home, nothing played | 0 |
| Player closed, playing | 0 |
| Player open, playing | 0 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 0 |
| Big list, wheel-scrolling, playing | 0 |
| Big list open, paused | 0 |

**perf.unchanged** (perf line field)

| State | parity |
|---|---|
| Idle on Home, nothing played | 0 |
| Player closed, playing | 0 |
| Player open, playing | 0 |
| Player open, paused | 0 |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | 0 |
| Playing, window on a hidden workspace | 0 |
| Playing, window in the scratchpad (hidden) | 0 |
| Big list open, playing (row shows playing bars) | 0 |
| Big list, wheel-scrolling, playing | 0 |
| Big list open, paused | 0 |

**Busiest threads (median % of a core)**

| State | parity |
|---|---|
| Idle on Home, nothing played | – |
| Player closed, playing | llvmpipe-# 5.9 |
| Player open, playing | llvmpipe-# 27.5 |
| Player open, paused | – |
| Player open, playing, window unfocused (visible) | – |
| Playing, window covered by a fullscreen window | – |
| Playing, window on a hidden workspace | – |
| Playing, window in the scratchpad (hidden) | – |
| Big list open, playing (row shows playing bars) | llvmpipe-# 235.3, sone-rune 13.9 |
| Big list, wheel-scrolling, playing | llvmpipe-# 243, sone-rune 21.1 |
| Big list open, paused | – |

</details>

| Timing (s) | parity |
|---|---|
| Launch → window mapped (first frame) | 3.100 |
| Launch → MCP answers | 1.499 |
| Launch → Home button on AT-SPI | 3.199 |
| Launch → Home content on AT-SPI | 3.250 |
| Close request → process exited | 2.639 |
| Big list: press → its heading on AT-SPI | 0.220 |

States that did not take in some runs (their numbers are left out):

- parity / open_unfocused: 0/1: perf line says open playing focused, expected unfocused

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- parity run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 426 MB

Sessions (machine load, 1-min loadavg at start → end):

- parity: 01:11:45 6.0→2.5 (226 s)

