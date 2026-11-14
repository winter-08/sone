# States benchmark startup (startup-cold-warm)

4 runs per build and mode (re-collected from /home/winter/dev/sone/verifier/runs/states-startup), sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 0 min 0 s.

Builds: `final-cold` = `../../../../nonexistent/a`; `final-warm` = `../../../../nonexistent/b`; `parity-cold` = `../../../../nonexistent/c`; `parity-warm` = `../../../../nonexistent/d`

## Software rendering (pixman sway, lavapipe)

| State | took | fps final-cold | fps final-warm | fps parity-cold | fps parity-warm | views/s final-cold | views/s final-warm | views/s parity-cold | views/s parity-warm | polls/s final-cold | polls/s final-warm | polls/s parity-cold | polls/s parity-warm | CPU % final-cold | CPU % final-warm | CPU % parity-cold | CPU % parity-warm | PSS MB final-cold | PSS MB final-warm | PSS MB parity-cold | PSS MB parity-warm |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 3/3 3/3 4/4 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.2 (5.2–5.2) | 5.2 (5.2–5.2) | 0.0 (0.0–0.0) | 0.0 (0.0–0.2) | 1.2 (1.2–1.4) | 1.4 (1.2–1.4) | 285 (285–286) | 248 (247–248) | 306 (306–311) | 267 (266–268) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |

**Unchanged frames/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |

**Polls/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.2 (5.2–5.2) | 5.2 (5.2–5.2) |

**Regions/s (perf line, newer builds only)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**CPU, % of a core (perf line)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.2) | 1.2 (1.2–1.4) | 1.4 (1.2–1.4) |

**Compositor CPU, % of a core**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**RSS MB (perf line)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 304 (304–305) | 267 (266–267) | 325 (325–330) | 286 (285–287) |

**PSS MB (smaps_rollup)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 285 (285–286) | 248 (247–248) | 306 (306–311) | 267 (266–268) |

**Machine busy % during window (all 24 cores)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 2.3 (1.2–2.7) | 2.5 (1.2–3.4) | 2.2 (1.2–2.6) | 2.4 (1.4–3.1) |

**perf.drawn** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 1 (1–1) | 1 (1–1) |

**perf.drawn_per_s** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |

**perf.skipped** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | – | – | llvmpipe-# 0.8 | llvmpipe-# 1 |

</details>

| Timing (s) | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Launch → window mapped (first frame) | 2.885 (2.880–2.906) | 1.939 (1.878–1.952) | 3.098 (3.069–3.099) | 1.994 (1.928–2.038) |
| Launch → MCP answers | 1.490 (1.483–1.516) | 1.500 (1.443–1.519) | 1.490 (1.477–1.520) | 1.524 (1.451–1.564) |
| Launch → Home button on AT-SPI | 2.973 (2.967–2.999) | 2.010 (1.954–2.023) | 3.167 (3.139–3.195) | 2.091 (2.011–2.129) |
| Launch → Home content on AT-SPI | 3.018 (3.014–3.044) | 2.056 (2.000–2.066) | 3.219 (3.198–3.246) | 2.142 (2.058–2.178) |
| Close request → process exited | 0.017 (0.017–0.017) | 0.017 (0.017–0.017) | 2.366 (2.357–2.407) | 2.349 (2.282–2.393) |
| Big list: press → its heading on AT-SPI | – | – | – | – |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- final-cold run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 305 MB
- final-cold run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 305 MB
- final-cold run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 304 MB
- final-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 268 MB
- final-warm run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 267 MB
- final-warm run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 268 MB
- parity-cold run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 326 MB
- parity-cold run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 326 MB
- parity-cold run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 330 MB
- parity-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 287 MB
- parity-warm run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 286 MB
- parity-warm run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 287 MB
- parity-warm run3: requests none; rows served 0; on/off-screen log lines 0; RSS peak 286 MB

Sessions (machine load, 1-min loadavg at start → end):

- final-cold: 02:46:41 7.2→5.5 (18 s), 02:50:15 0.9→0.8 (18 s), 02:51:52 0.7→0.8 (18 s)
- final-warm: 02:47:20 4.2→3.5 (18 s), 02:49:37 1.2→1.1 (18 s), 02:52:30 0.9→0.8 (18 s)
- parity-cold: 02:46:59 5.5→4.2 (20 s), 02:49:55 1.1→0.9 (20 s), 02:52:10 0.8→0.9 (20 s)
- parity-warm: 02:47:38 3.5→2.7 (20 s), 02:49:17 1.5→1.2 (20 s), 02:52:49 0.8→0.7 (20 s), 02:54:27 0.6→0.7 (20 s)

## GPU

| State | took | fps final-cold | fps final-warm | fps parity-cold | fps parity-warm | views/s final-cold | views/s final-warm | views/s parity-cold | views/s parity-warm | polls/s final-cold | polls/s final-warm | polls/s parity-cold | polls/s parity-warm | CPU % final-cold | CPU % final-warm | CPU % parity-cold | CPU % parity-warm | PSS MB final-cold | PSS MB final-warm | PSS MB parity-cold | PSS MB parity-warm |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.2 (5.2–5.2) | 5.2 (5.2–5.2) | 0.2 (0.0–0.2) | 0.0 (0.0–0.2) | 0.2 (0.2–0.4) | 0.2 (0.2–0.4) | 151 (150–151) | 128 (128–128) | 161 (160–161) | 137 (136–137) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |

**Unchanged frames/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |

**Polls/s**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.2 (5.2–5.2) | 5.2 (5.2–5.2) |

**Regions/s (perf line, newer builds only)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**CPU, % of a core (perf line)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.2 (0.0–0.2) | 0.0 (0.0–0.2) | 0.2 (0.2–0.4) | 0.2 (0.2–0.4) |

**Compositor CPU, % of a core**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.2 (0.0–0.2) | 0.2 (0.0–0.2) | 0.2 (0.0–0.2) | 0.0 (0.0–0.2) |

**RSS MB (perf line)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 268 (267–269) | 234 (234–234) | 278 (278–279) | 244 (244–244) |

**PSS MB (smaps_rollup)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 151 (150–151) | 128 (128–128) | 161 (160–161) | 137 (136–137) |

**GPU memory MiB (nvidia-smi, app)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 68 (64–68) | 68 (68–68) | 96 (96–96) | 96 (96–96) |

**Machine busy % during window (all 24 cores)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 2.4 (2.3–4.0) | 2.4 (1.3–2.4) | 2.2 (2.2–2.9) | 2.1 (1.9–2.2) |

**perf.drawn** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 1 (1–1) | 1 (1–1) |

**perf.drawn_per_s** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |

**perf.skipped** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Idle on Home, nothing played | – | – | – | – |

</details>

| Timing (s) | final-cold | final-warm | parity-cold | parity-warm |
|---|---|---|---|---|
| Launch → window mapped (first frame) | 2.266 (2.265–2.364) | 1.756 (1.710–1.758) | 2.338 (2.263–2.432) | 1.725 (1.716–1.740) |
| Launch → MCP answers | 1.482 (1.476–1.563) | 1.499 (1.453–1.500) | 1.459 (1.413–1.563) | 1.451 (1.438–1.458) |
| Launch → Home button on AT-SPI | 2.348 (2.345–2.434) | 1.806 (1.757–1.811) | 2.433 (2.335–2.488) | 1.807 (1.798–1.822) |
| Launch → Home content on AT-SPI | 2.392 (2.392–2.479) | 1.855 (1.803–1.857) | 2.483 (2.383–2.535) | 1.857 (1.846–1.870) |
| Close request → process exited | 0.175 (0.170–0.241) | 0.180 (0.144–0.200) | 1.823 (1.714–1.875) | 1.645 (1.634–1.748) |
| Big list: press → its heading on AT-SPI | – | – | – | – |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- final-cold run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 270 MB
- final-cold run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 269 MB
- final-cold run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 271 MB
- final-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 235 MB
- final-warm run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 235 MB
- final-warm run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 235 MB
- parity-cold run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 279 MB
- parity-cold run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 279 MB
- parity-cold run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 279 MB
- parity-warm run0: requests none; rows served 0; on/off-screen log lines 0; RSS peak 244 MB
- parity-warm run1: requests none; rows served 0; on/off-screen log lines 0; RSS peak 244 MB
- parity-warm run2: requests none; rows served 0; on/off-screen log lines 0; RSS peak 245 MB

Sessions (machine load, 1-min loadavg at start → end):

- final-cold: 02:47:58 2.7→2.4 (19 s), 02:51:33 0.8→0.7 (19 s), 02:53:09 0.7→0.6 (19 s)
- final-warm: 02:48:38 1.9→1.9 (19 s), 02:50:54 0.7→0.8 (19 s), 02:53:48 0.6→0.6 (19 s)
- parity-cold: 02:48:17 2.4→1.9 (21 s), 02:51:13 0.8→0.8 (21 s), 02:53:28 0.6→0.6 (21 s)
- parity-warm: 02:48:56 1.9→1.5 (21 s), 02:50:34 0.8→0.7 (21 s), 02:54:07 0.6→0.6 (21 s)

Failed sessions: software final-warm run3: no states.json (session cut short)
