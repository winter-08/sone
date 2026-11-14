# States benchmark 2026-11-14-024228 (startup-prime)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 1 interval(s) kept (5 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 0 tracks (served `limit` rows per request, as TIDAL). Total wall time 1 min 18 s.

Builds: `final-warm` = `../../../../tmp/claude-1000/-home-winter-dev-sone/219761a7-de7c-48a5-8ba2-7fb47c6eb530/scratchpad/launch/warm-final.sh`; `parity-warm` = `../../../../tmp/claude-1000/-home-winter-dev-sone/219761a7-de7c-48a5-8ba2-7fb47c6eb530/scratchpad/launch/warm-parity.sh`

## Software rendering (pixman sway, lavapipe)

| State | took | fps final-warm | fps parity-warm | views/s final-warm | views/s parity-warm | polls/s final-warm | polls/s parity-warm | CPU % final-warm | CPU % parity-warm | PSS MB final-warm | PSS MB parity-warm |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 1/1 1/1 | 0.0 | 0.2 | 0.0 | 0.2 | 5.0 | 5.4 | 0.2 | 1.6 | 283 | 307 |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.2 |

**Unchanged frames/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.0 |

**Skipped frames/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.0 |

**View runs/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.2 |

**Polls/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 5.0 | 5.4 |

**Regions/s (perf line, newer builds only)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.0 |

**CPU, % of a core (perf line)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.2 | 1.6 |

**Compositor CPU, % of a core**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.0 |

**RSS MB (perf line)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 302 | 326 |

**PSS MB (smaps_rollup)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 283 | 307 |

**Machine busy % during window (all 24 cores)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 4.7 | 2.3 |

**perf.drawn** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 1 |

**perf.drawn_per_s** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 0.2 |

**perf.skipped** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 0 |

**perf.unchanged** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 0 |

**Busiest threads (median % of a core)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | – | llvmpipe-# 1.6 |

</details>

| Timing (s) | final-warm | parity-warm |
|---|---|---|
| Launch → window mapped (first frame) | 2.829 | 3.030 |
| Launch → MCP answers | 1.480 | 1.458 |
| Launch → Home button on AT-SPI | 2.882 | 3.070 |
| Launch → Home content on AT-SPI | 2.931 | 3.126 |
| Close request → process exited | 0.017 | 2.324 |
| Big list: press → its heading on AT-SPI | – | – |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- final-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 303 MB
- parity-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 326 MB

Sessions (machine load, 1-min loadavg at start → end):

- final-warm: 02:42:28 0.8→1.3 (18 s)
- parity-warm: 02:42:46 1.3→1.4 (20 s)

## GPU

| State | took | fps final-warm | fps parity-warm | views/s final-warm | views/s parity-warm | polls/s final-warm | polls/s parity-warm | CPU % final-warm | CPU % parity-warm | PSS MB final-warm | PSS MB parity-warm |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 1/1 1/1 | 0.0 | 0.2 | 0.0 | 0.2 | 5.0 | 5.2 | 0.2 | 0.0 | 151 | 161 |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.2 |

**Unchanged frames/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.0 |

**Skipped frames/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.0 |

**View runs/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.2 |

**Polls/s**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 5.0 | 5.2 |

**Regions/s (perf line, newer builds only)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.0 |

**CPU, % of a core (perf line)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.2 | 0.0 |

**Compositor CPU, % of a core**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0.0 | 0.2 |

**RSS MB (perf line)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 269 | 279 |

**PSS MB (smaps_rollup)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 151 | 161 |

**GPU memory MiB (nvidia-smi, app)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 68 | 96 |

**Machine busy % during window (all 24 cores)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 2.3 | 2.2 |

**perf.drawn** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 1 |

**perf.drawn_per_s** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 0.2 |

**perf.skipped** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 0 |

**perf.unchanged** (perf line field)

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | 0 | 0 |

**Busiest threads (median % of a core)**

| State | final-warm | parity-warm |
|---|---|---|
| Idle on Home, nothing played | – | – |

</details>

| Timing (s) | final-warm | parity-warm |
|---|---|---|
| Launch → window mapped (first frame) | 2.238 | 2.310 |
| Launch → MCP answers | 1.441 | 1.438 |
| Launch → Home button on AT-SPI | 2.308 | 2.368 |
| Launch → Home content on AT-SPI | 2.356 | 2.422 |
| Close request → process exited | 0.221 | 1.740 |
| Big list: press → its heading on AT-SPI | – | – |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- final-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 271 MB
- parity-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 280 MB

Sessions (machine load, 1-min loadavg at start → end):

- final-warm: 02:43:06 1.4→1.3 (19 s)
- parity-warm: 02:43:25 1.3→1.0 (21 s)

