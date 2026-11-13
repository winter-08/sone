# States benchmark 2026-11-13-154847 (stress3000-base-guarded)

1 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (every row in one response). Total wall time 3 min 24 s.

Builds: `base` = `rune-app/target/baseline/release/sone-rune-wrapped`

## Software rendering (pixman sway, lavapipe)

| State | took | fps base | views/s base | polls/s base | CPU % base | PSS MB base |
|---|---|---|---|---|---|---|

<details><summary>All metrics</summary>

**Busiest threads (median % of a core)**

| State | base |
|---|---|

</details>

| Timing (s) | base |
|---|---|
| Launch → window mapped (first frame) | 2.913 |
| Launch → MCP answers | 2.128 |
| Launch → Home button on AT-SPI | 2.934 |
| Launch → Home content on AT-SPI | 3.062 |
| Close request → process exited | – |
| Big list: press → its heading on AT-SPI | – |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- base run0: requests none; rows served 0; on/off-screen log lines 0; KILLED by guard at 3006 MB during biglist_playing (entering); errors 2

Sessions (machine load, 1-min loadavg at start → end):

- base: 15:48:47 3.4→3.1 (101 s)

## GPU

| State | took | fps base | views/s base | polls/s base | CPU % base | PSS MB base |
|---|---|---|---|---|---|---|

<details><summary>All metrics</summary>

**Busiest threads (median % of a core)**

| State | base |
|---|---|

</details>

| Timing (s) | base |
|---|---|
| Launch → window mapped (first frame) | 2.705 |
| Launch → MCP answers | 2.145 |
| Launch → Home button on AT-SPI | 2.760 |
| Launch → Home content on AT-SPI | 2.800 |
| Close request → process exited | – |
| Big list: press → its heading on AT-SPI | – |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- base run0: requests none; rows served 0; on/off-screen log lines 0; KILLED by guard at 3131 MB during biglist_playing (entering); errors 2

Sessions (machine load, 1-min loadavg at start → end):

- base: 15:50:28 3.1→3.3 (103 s)

