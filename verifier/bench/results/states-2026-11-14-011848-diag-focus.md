# States benchmark 2026-11-14-011848 (diag-focus)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 300 tracks (served `limit` rows per request, as TIDAL). Total wall time 2 min 50 s.

Builds: `final` = `rune-app/target/stage-final/release/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1'}; `parity` = `rune-app/target/stage-parity/release/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1'}

## Software rendering (pixman sway, lavapipe)

| State | took | fps final | fps parity | views/s final | views/s parity | polls/s final | polls/s parity | CPU % final | CPU % parity | PSS MB final | PSS MB parity |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Player closed, playing | 1/1 1/1 | 1.0 | 1.1 | 1.0 | 1.1 | 5.0 | 5.2 | 6.9 | 7.3 | 289 | 305 |
| closed_unfocused | 1/1 0/1 | 1.0 | – | 1.0 | – | 5.0 | – | 6.7 | – | 286 | – |
| Player open, playing | 1/1 1/1 | 1.0 | 1.0 | 1.0 | 1.0 | 5.0 | 5.0 | 27.8 | 28.0 | 338 | 364 |
| Player open, playing, window unfocused (visible) | 1/1 0/1 | 1.0 | – | 1.0 | – | 5.0 | – | 27.6 | – | 338 | – |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | final | parity |
|---|---|---|
| Player closed, playing | 1.0 | 1.1 |
| closed_unfocused | 1.0 | – |
| Player open, playing | 1.0 | 1.0 |
| Player open, playing, window unfocused (visible) | 1.0 | – |

**Unchanged frames/s**

| State | final | parity |
|---|---|---|
| Player closed, playing | 0.0 | 0.0 |
| closed_unfocused | 0.0 | – |
| Player open, playing | 0.0 | 0.0 |
| Player open, playing, window unfocused (visible) | 0.0 | – |

**Skipped frames/s**

| State | final | parity |
|---|---|---|
| Player closed, playing | 0.0 | 0.0 |
| closed_unfocused | 0.0 | – |
| Player open, playing | 0.0 | 0.0 |
| Player open, playing, window unfocused (visible) | 0.0 | – |

**View runs/s**

| State | final | parity |
|---|---|---|
| Player closed, playing | 1.0 | 1.1 |
| closed_unfocused | 1.0 | – |
| Player open, playing | 1.0 | 1.0 |
| Player open, playing, window unfocused (visible) | 1.0 | – |

**Polls/s**

| State | final | parity |
|---|---|---|
| Player closed, playing | 5.0 | 5.2 |
| closed_unfocused | 5.0 | – |
| Player open, playing | 5.0 | 5.0 |
| Player open, playing, window unfocused (visible) | 5.0 | – |

**Regions/s (perf line, newer builds only)**

| State | final | parity |
|---|---|---|
| Player closed, playing | 1.0 | 1.0 |
| closed_unfocused | 1.0 | – |
| Player open, playing | 1.0 | 1.0 |
| Player open, playing, window unfocused (visible) | 1.0 | – |

**CPU, % of a core (perf line)**

| State | final | parity |
|---|---|---|
| Player closed, playing | 6.9 | 7.3 |
| closed_unfocused | 6.7 | – |
| Player open, playing | 27.8 | 28.0 |
| Player open, playing, window unfocused (visible) | 27.6 | – |

**Compositor CPU, % of a core**

| State | final | parity |
|---|---|---|
| Player closed, playing | 0.0 | 0.0 |
| closed_unfocused | 0.0 | – |
| Player open, playing | 0.0 | 0.1 |
| Player open, playing, window unfocused (visible) | 0.0 | – |

**RSS MB (perf line)**

| State | final | parity |
|---|---|---|
| Player closed, playing | 314 | 330 |
| closed_unfocused | 314 | – |
| Player open, playing | 366 | 392 |
| Player open, playing, window unfocused (visible) | 366 | – |

**PSS MB (smaps_rollup)**

| State | final | parity |
|---|---|---|
| Player closed, playing | 289 | 305 |
| closed_unfocused | 286 | – |
| Player open, playing | 338 | 364 |
| Player open, playing, window unfocused (visible) | 338 | – |

**Machine busy % during window (all 24 cores)**

| State | final | parity |
|---|---|---|
| Player closed, playing | 2.7 | 7.7 |
| closed_unfocused | 2.7 | – |
| Player open, playing | 3.4 | 13.1 |
| Player open, playing, window unfocused (visible) | 19.6 | – |

**perf.drawn** (perf line field)

| State | final | parity |
|---|---|---|
| Player closed, playing | 5 | 5.5 |
| closed_unfocused | 5 | – |
| Player open, playing | 5 | 5 |
| Player open, playing, window unfocused (visible) | 5 | – |

**perf.drawn_per_s** (perf line field)

| State | final | parity |
|---|---|---|
| Player closed, playing | 1 | 1.1 |
| closed_unfocused | 1 | – |
| Player open, playing | 1 | 1 |
| Player open, playing, window unfocused (visible) | 1 | – |

**perf.skipped** (perf line field)

| State | final | parity |
|---|---|---|
| Player closed, playing | 0 | 0 |
| closed_unfocused | 0 | – |
| Player open, playing | 0 | 0 |
| Player open, playing, window unfocused (visible) | 0 | – |

**perf.unchanged** (perf line field)

| State | final | parity |
|---|---|---|
| Player closed, playing | 0 | 0 |
| closed_unfocused | 0 | – |
| Player open, playing | 0 | 0 |
| Player open, playing, window unfocused (visible) | 0 | – |

**Busiest threads (median % of a core)**

| State | final | parity |
|---|---|---|
| Player closed, playing | llvmpipe-# 6.1 | llvmpipe-# 6.3, sone-rune 0.6 |
| closed_unfocused | llvmpipe-# 5.9 | – |
| Player open, playing | llvmpipe-# 26.9, sone-rune 0.5 | llvmpipe-# 27.3, sone-rune 0.6 |
| Player open, playing, window unfocused (visible) | llvmpipe-# 26.1, sone-rune 0.5 | – |

</details>

| Timing (s) | final | parity |
|---|---|---|
| Launch → window mapped (first frame) | 2.891 | 5.971 |
| Launch → MCP answers | 1.498 | 2.464 |
| Launch → Home button on AT-SPI | 2.987 | 6.236 |
| Launch → Home content on AT-SPI | 3.030 | 6.324 |
| Close request → process exited | 0.033 | 2.499 |
| Big list: press → its heading on AT-SPI | – | – |

States that did not take in some runs (their numbers are left out):

- parity / closed_unfocused: 0/1: perf line says closed playing focused, expected unfocused
- parity / open_unfocused: 0/1: perf line says open playing focused, expected unfocused

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- final run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 367 MB
- parity run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 393 MB

Sessions (machine load, 1-min loadavg at start → end):

- final: 01:18:48 2.4→1.9 (83 s)
- parity: 01:20:11 1.9→3.3 (87 s)

