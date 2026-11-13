# States benchmark 2026-11-13-152629 (scrollcheck)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 1 interval(s) kept (5 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 2 min 21 s.

Builds: `base` = `rune-app/target/baseline/release/sone-rune-wrapped`; `bump` = `rune-app/target/stage-bump/release/sone-rune-wrapped`; `perf` = `rune-app/target/stage-perf/release/sone-rune-wrapped`; `final` = `rune-app/target/stage-final/release/sone-rune-wrapped`

## Software rendering (pixman sway, lavapipe)

| State | took | fps base | fps bump | fps perf | fps final | views/s base | views/s bump | views/s perf | views/s final | polls/s base | polls/s bump | polls/s perf | polls/s final | CPU % base | CPU % bump | CPU % perf | CPU % final | PSS MB base | PSS MB bump | PSS MB perf | PSS MB final |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 1/1 1/1 1/1 1/1 | 61.0 | 60.8 | 62.4 | 62.4 | 1.0 | 1.0 | 1.0 | 1.0 | 60.8 | 61.0 | 62.4 | 62.4 | 238.2 | 241.0 | 243.8 | 242.0 | 437 | 478 | 311 | 310 |
| Big list, wheel-scrolling, playing | 1/1 1/1 1/1 1/1 | 20.0 | 28.2 | 62.2 | 62.4 | 19.6 | 20.6 | 1.0 | 1.0 | 20.0 | 28.4 | 62.4 | 62.4 | 164.8 | 188.2 | 240.6 | 241.8 | 435 | 476 | 310 | 310 |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 61.0 | 60.8 | 62.4 | 62.4 |
| Big list, wheel-scrolling, playing | 20.0 | 28.2 | 62.2 | 62.4 |

**Unchanged frames/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 0.0 | 0.0 | 0.0 | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 | 0.0 | 0.2 | 0.0 |

**Skipped frames/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 0.0 | 0.0 | 0.0 | 0.0 |
| Big list, wheel-scrolling, playing | 0.0 | 0.0 | 0.0 | 0.0 |

**View runs/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 1.0 | 1.0 | 1.0 | 1.0 |
| Big list, wheel-scrolling, playing | 19.6 | 20.6 | 1.0 | 1.0 |

**Polls/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 60.8 | 61.0 | 62.4 | 62.4 |
| Big list, wheel-scrolling, playing | 20.0 | 28.4 | 62.4 | 62.4 |

**Regions/s (perf line, newer builds only)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | – | – | 1.0 | 1.0 |
| Big list, wheel-scrolling, playing | – | – | 1.0 | 1.0 |

**CPU, % of a core (perf line)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 238.2 | 241.0 | 243.8 | 242.0 |
| Big list, wheel-scrolling, playing | 164.8 | 188.2 | 240.6 | 241.8 |

**Compositor CPU, % of a core**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 0.6 | 0.6 | 0.4 | 0.6 |
| Big list, wheel-scrolling, playing | 0.2 | 0.4 | 0.4 | 0.6 |

**RSS MB (perf line)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 450 | 494 | 328 | 327 |
| Big list, wheel-scrolling, playing | 451 | 495 | 330 | 329 |

**PSS MB (smaps_rollup)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 437 | 478 | 311 | 310 |
| Big list, wheel-scrolling, playing | 435 | 476 | 310 | 310 |

**Machine busy % during window (all 24 cores)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 21.6 | 21.9 | 21.5 | 21.7 |
| Big list, wheel-scrolling, playing | 19.3 | 20.4 | 21.9 | 21.9 |

**perf.drawn** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 305 | 304 | 312 | 312 |
| Big list, wheel-scrolling, playing | 100 | 141 | 311 | 312 |

**perf.drawn_per_s** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 61 | 60.8 | 62.4 | 62.4 |
| Big list, wheel-scrolling, playing | 20 | 28.2 | 62.2 | 62.4 |

**perf.skipped** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 0 | 0 | 0 | 0 |
| Big list, wheel-scrolling, playing | 0 | 0 | 0 | 0 |

**perf.unchanged** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | 0 | 0 | 0 | 0 |
| Big list, wheel-scrolling, playing | 0 | 0 | 1 | 0 |

**Busiest threads (median % of a core)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Big list open, playing (row shows playing bars) | llvmpipe-# 215.6, sone-rune 21.9 | llvmpipe-# 219.4, sone-rune 21.8 | llvmpipe-# 227.9, sone-rune 14.8 | llvmpipe-# 227.7, sone-rune 14.6 |
| Big list, wheel-scrolling, playing | sone-rune 96.1, llvmpipe-# 69.5 | llvmpipe-# 100.6, sone-rune 88.6 | llvmpipe-# 220.5, sone-rune 19.4 | llvmpipe-# 221.2, sone-rune 19.8 |

</details>

| Timing (s) | base | bump | perf | final |
|---|---|---|---|---|
| Launch → window mapped (first frame) | 2.922 | 3.634 | 3.623 | 3.576 |
| Launch → MCP answers | 2.149 | 2.130 | 2.124 | 2.103 |
| Launch → Home button on AT-SPI | 2.944 | 3.721 | 3.647 | 3.628 |
| Launch → Home content on AT-SPI | 3.068 | 3.759 | 3.758 | 3.682 |
| Close request → process exited | 0.037 | 0.037 | 0.022 | 0.022 |
| Big list: press → its heading on AT-SPI | 0.216 | 0.306 | 0.138 | 0.163 |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- base run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- bump run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- perf run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- final run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0

Sessions (machine load, 1-min loadavg at start → end):

- base: 15:26:29 3.8→3.3 (35 s)
- bump: 15:27:04 3.3→5.9 (35 s)
- perf: 15:27:39 5.9→4.4 (35 s)
- final: 15:28:14 4.4→3.8 (35 s)

