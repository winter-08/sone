# States benchmark 2026-11-14-115510 (vcover-regions)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 1 min 8 s.

Builds: `cover` = `rune-app/target/debug/sone-rune-wrapped` {'SPIKE_VIDEO_COVER': '1', 'RUNE_REGION_MISS': '1'}

## Software rendering (pixman sway, lavapipe)

| State | took | fps cover | views/s cover | polls/s cover | CPU % cover | PSS MB cover |
|---|---|---|---|---|---|---|
| Album page, nothing playing | 1/1 | 30.1 | 0.1 | 30.1 | 152.7 | 406 |
| Playing on the album page, drawer open (region check) | 1/1 | 62.5 | 1.0 | 62.4 | 397.4 | 428 |
| Player open, playing | 1/1 | 55.5 | 1.0 | 55.5 | 1654.7 | 498 |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | cover |
|---|---|
| Album page, nothing playing | 30.1 |
| Playing on the album page, drawer open (region check) | 62.5 |
| Player open, playing | 55.5 |

**Unchanged frames/s**

| State | cover |
|---|---|
| Album page, nothing playing | 0.0 |
| Playing on the album page, drawer open (region check) | 0.0 |
| Player open, playing | 0.0 |

**Skipped frames/s**

| State | cover |
|---|---|
| Album page, nothing playing | 0.0 |
| Playing on the album page, drawer open (region check) | 0.0 |
| Player open, playing | 0.0 |

**View runs/s**

| State | cover |
|---|---|
| Album page, nothing playing | 0.1 |
| Playing on the album page, drawer open (region check) | 1.0 |
| Player open, playing | 1.0 |

**Polls/s**

| State | cover |
|---|---|
| Album page, nothing playing | 30.1 |
| Playing on the album page, drawer open (region check) | 62.4 |
| Player open, playing | 55.5 |

**Regions/s (perf line, newer builds only)**

| State | cover |
|---|---|
| Album page, nothing playing | 0.0 |
| Playing on the album page, drawer open (region check) | 1.0 |
| Player open, playing | 1.0 |

**CPU, % of a core (perf line)**

| State | cover |
|---|---|
| Album page, nothing playing | 152.7 |
| Playing on the album page, drawer open (region check) | 397.4 |
| Player open, playing | 1654.7 |

**Compositor CPU, % of a core**

| State | cover |
|---|---|
| Album page, nothing playing | 0.2 |
| Playing on the album page, drawer open (region check) | 0.3 |
| Player open, playing | 0.4 |

**RSS MB (perf line)**

| State | cover |
|---|---|
| Album page, nothing playing | 422 |
| Playing on the album page, drawer open (region check) | 445 |
| Player open, playing | 515 |

**PSS MB (smaps_rollup)**

| State | cover |
|---|---|
| Album page, nothing playing | 406 |
| Playing on the album page, drawer open (region check) | 428 |
| Player open, playing | 498 |

**Machine busy % during window (all 24 cores)**

| State | cover |
|---|---|
| Album page, nothing playing | 24.9 |
| Playing on the album page, drawer open (region check) | 21.0 |
| Player open, playing | 75.3 |

**perf.drawn** (perf line field)

| State | cover |
|---|---|
| Album page, nothing playing | 150.5 |
| Playing on the album page, drawer open (region check) | 312.5 |
| Player open, playing | 277.5 |

**perf.drawn_per_s** (perf line field)

| State | cover |
|---|---|
| Album page, nothing playing | 30.1 |
| Playing on the album page, drawer open (region check) | 62.5 |
| Player open, playing | 55.5 |

**perf.skipped** (perf line field)

| State | cover |
|---|---|
| Album page, nothing playing | 0 |
| Playing on the album page, drawer open (region check) | 0 |
| Player open, playing | 0 |

**perf.unchanged** (perf line field)

| State | cover |
|---|---|
| Album page, nothing playing | 0 |
| Playing on the album page, drawer open (region check) | 0 |
| Player open, playing | 0 |

**Busiest threads (median % of a core)**

| State | cover |
|---|---|
| Album page, nothing playing | llvmpipe-# 141.4, sone-rune 7.9, multiqueue#:src 1.1, av:h#:df# 0.6 |
| Playing on the album page, drawer open (region check) | llvmpipe-# 376.2, sone-rune 18.7, av:h#:df# 1.6, multiqueue#:src 0.8 |
| Player open, playing | llvmpipe-# 1631.1, sone-rune 18.4, multiqueue#:src 3.5, av:h#:df# 1.5 |

</details>

| Timing (s) | cover |
|---|---|
| Launch → window mapped (first frame) | 3.183 |
| Launch → MCP answers | 1.455 |
| Launch → Home button on AT-SPI | 3.223 |
| Launch → Home content on AT-SPI | 3.277 |
| Close request → process exited | 0.071 |
| Big list: press → its heading on AT-SPI | – |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- cover run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 516 MB

Region rebuilds (RUNE_REGION_MISS) per second in the kept window, by region (first stale read):

- cover run0 album_idle: total 0.0/s: none
- cover run0 region_album: total 1.0/s: `nowbar` 1.0/s (state 13 ×10)
- cover run0 open_playing: total 1.0/s: `nowbar` 1.0/s (state 13 ×10)

Sessions (machine load, 1-min loadavg at start → end):

- cover: 11:55:10 11.8→9.7 (68 s)

