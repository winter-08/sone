# States benchmark 2026-11-13-120110 (baseline-provisional)

3 runs per build and mode (re-collected from /home/winter/dev/sone/verifier/runs/states-2026-11-13-120110), sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 0 min 0 s.

Builds: `base-debug` = `rune-app/target/baseline/debug/sone-rune-wrapped`; `base-release` = `rune-app/target/baseline/release/sone-rune-wrapped`

## Software rendering (pixman sway, lavapipe)

| State | took | fps base-debug | fps base-release | views/s base-debug | views/s base-release | polls/s base-debug | polls/s base-release | CPU % base-debug | CPU % base-release | PSS MB base-debug | PSS MB base-release |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.1 (0.1–0.2) | 0.1 (0.0–0.1) | 238 (227–270) | 224 (214–260) |
| Player closed, playing | 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 7.9 (7.7–7.9) | 7.8 (7.6–7.9) | 244 (231–287) | 231 (210–273) |
| Player open, playing | 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 28.6 (28.2–28.7) | 28.5 (28.2–28.7) | 331 (290–348) | 296 (249–335) |
| Player open, paused | 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.2 (0.1–0.2) | 0.1 (0.1–0.1) | 331 (303–335) | 335 (251–336) |
| Player open, playing, window unfocused (visible) | 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 28.8 (28.3–28.8) | 28.5 (28.1–28.7) | 301 (292–329) | 333 (252–334) |
| Playing, window covered by a fullscreen window | 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 99.8 (83.5–99.9) | 99.8 (99.7–99.8) | 301 (272–319) | 303 (245–303) |
| Playing, window on a hidden workspace | 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 99.7 (90.7–99.8) | 0.5 (0.4–0.6) | 314 (286–331) | 279 (261–319) |
| Playing, window in the scratchpad (hidden) | 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 99.8 (99.7–99.8) | 99.7 (99.6–99.8) | 270 (256–318) | 266 (242–307) |
| Big list open, playing (row shows playing bars) | 3/3 3/3 | 60.4 (60.2–60.4) | 60.1 (59.3–60.5) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 60.4 (60.2–60.5) | 60.1 (59.4–60.5) | 236.1 (233.9–238.5) | 233.0 (226.8–237.6) | 457 (440–504) | 439 (428–478) |
| Big list, wheel-scrolling, playing | 3/3 3/3 | 17.2 (14.4–17.6) | 18.5 (13.7–19.1) | 17.2 (14.5–17.6) | 18.5 (13.7–19.1) | 17.2 (14.5–17.6) | 18.5 (13.7–19.1) | 156.6 (144.1–158.4) | 159.6 (138.7–162.8) | 496 (442–505) | 440 (439–479) |
| Big list open, paused | 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.2 (0.1–0.2) | 0.0 (0.0–0.1) | 498 (442–505) | 479 (478–491) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 60.4 (60.2–60.4) | 60.1 (59.3–60.5) |
| Big list, wheel-scrolling, playing | 17.2 (14.4–17.6) | 18.5 (13.7–19.1) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Unchanged frames/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Big list, wheel-scrolling, playing | 17.2 (14.5–17.6) | 18.5 (13.7–19.1) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Polls/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player closed, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing, window unfocused (visible) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 60.4 (60.2–60.5) | 60.1 (59.4–60.5) |
| Big list, wheel-scrolling, playing | 17.2 (14.5–17.6) | 18.5 (13.7–19.1) |
| Big list open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |

**CPU, % of a core (perf line)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.1 (0.1–0.2) | 0.1 (0.0–0.1) |
| Player closed, playing | 7.9 (7.7–7.9) | 7.8 (7.6–7.9) |
| Player open, playing | 28.6 (28.2–28.7) | 28.5 (28.2–28.7) |
| Player open, paused | 0.2 (0.1–0.2) | 0.1 (0.1–0.1) |
| Player open, playing, window unfocused (visible) | 28.8 (28.3–28.8) | 28.5 (28.1–28.7) |
| Playing, window covered by a fullscreen window | 99.8 (83.5–99.9) | 99.8 (99.7–99.8) |
| Playing, window on a hidden workspace | 99.7 (90.7–99.8) | 0.5 (0.4–0.6) |
| Playing, window in the scratchpad (hidden) | 99.8 (99.7–99.8) | 99.7 (99.6–99.8) |
| Big list open, playing (row shows playing bars) | 236.1 (233.9–238.5) | 233.0 (226.8–237.6) |
| Big list, wheel-scrolling, playing | 156.6 (144.1–158.4) | 159.6 (138.7–162.8) |
| Big list open, paused | 0.2 (0.1–0.2) | 0.0 (0.0–0.1) |

**Compositor CPU, % of a core**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.1 (0.0–0.1) |
| Player open, playing | 0.0 (0.0–0.1) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.6 (0.6–0.6) | 0.6 (0.5–0.6) |
| Big list, wheel-scrolling, playing | 0.3 (0.3–0.3) | 0.3 (0.3–0.3) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**RSS MB (perf line)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 285 (281–286) | 272 (271–274) |
| Player closed, playing | 299 (298–301) | 286 (285–287) |
| Player open, playing | 357 (345–361) | 349 (328–350) |
| Player open, paused | 357 (345–361) | 349 (328–350) |
| Player open, playing, window unfocused (visible) | 357 (345–361) | 349 (328–350) |
| Playing, window covered by a fullscreen window | 347 (333–355) | 317 (317–317) |
| Playing, window on a hidden workspace | 362 (345–368) | 333 (333–333) |
| Playing, window in the scratchpad (hidden) | 328 (324–332) | 320 (320–320) |
| Big list open, playing (row shows playing bars) | 518 (511–518) | 494 (492–504) |
| Big list, wheel-scrolling, playing | 519 (512–520) | 494 (493–505) |
| Big list open, paused | 519 (512–520) | 494 (493–505) |

**PSS MB (smaps_rollup)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 238 (227–270) | 224 (214–260) |
| Player closed, playing | 244 (231–287) | 231 (210–273) |
| Player open, playing | 331 (290–348) | 296 (249–335) |
| Player open, paused | 331 (303–335) | 335 (251–336) |
| Player open, playing, window unfocused (visible) | 301 (292–329) | 333 (252–334) |
| Playing, window covered by a fullscreen window | 301 (272–319) | 303 (245–303) |
| Playing, window on a hidden workspace | 314 (286–331) | 279 (261–319) |
| Playing, window in the scratchpad (hidden) | 270 (256–318) | 266 (242–307) |
| Big list open, playing (row shows playing bars) | 457 (440–504) | 439 (428–478) |
| Big list, wheel-scrolling, playing | 496 (442–505) | 440 (439–479) |
| Big list open, paused | 498 (442–505) | 479 (478–491) |

**Machine busy % during window (all 24 cores)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 12.8 (12.7–16.6) | 14.0 (13.5–20.8) |
| Player closed, playing | 15.7 (13.1–26.8) | 19.3 (13.5–25.3) |
| Player open, playing | 14.1 (13.6–38.0) | 15.6 (14.8–27.0) |
| Player open, paused | 20.8 (13.8–25.2) | 13.4 (12.7–35.7) |
| Player open, playing, window unfocused (visible) | 15.6 (14.8–22.7) | 14.4 (13.7–22.0) |
| Playing, window covered by a fullscreen window | 18.9 (18.3–99.7) | 17.7 (17.3–32.8) |
| Playing, window on a hidden workspace | 19.2 (18.0–88.1) | 22.8 (13.3–28.9) |
| Playing, window in the scratchpad (hidden) | 18.4 (17.4–30.4) | 20.1 (17.5–30.3) |
| Big list open, playing (row shows playing bars) | 24.4 (24.2–32.6) | 46.0 (24.1–51.9) |
| Big list, wheel-scrolling, playing | 20.8 (20.6–44.1) | 25.7 (21.3–56.7) |
| Big list open, paused | 13.5 (13.1–29.1) | 13.5 (13.2–20.5) |

**perf.drawn** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 5 (5–5) | 5 (5–5) |
| Player open, playing | 5 (5–5) | 5 (5–5) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 5 (5–5) | 5 (5–5) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 302 (301–302) | 300.5 (296.5–302.5) |
| Big list, wheel-scrolling, playing | 86 (72–88) | 92.5 (68.5–95.5) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**perf.drawn_per_s** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 1 (1–1) | 1 (1–1) |
| Player open, playing | 1 (1–1) | 1 (1–1) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 1 (1–1) | 1 (1–1) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 60.4 (60.2–60.4) | 60.1 (59.3–60.5) |
| Big list, wheel-scrolling, playing | 17.2 (14.4–17.6) | 18.5 (13.7–19.1) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**perf.skipped** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | – | – |
| Player closed, playing | llvmpipe-# 6.4, sone-rune 0.9 | llvmpipe-# 6.3, sone-rune 0.7 |
| Player open, playing | llvmpipe-# 27.4, sone-rune 0.8 | llvmpipe-# 27.1, sone-rune 0.8 |
| Player open, paused | – | – |
| Player open, playing, window unfocused (visible) | llvmpipe-# 26.9, sone-rune 1 | llvmpipe-# 27.1, sone-rune 0.8 |
| Playing, window covered by a fullscreen window | sone-rune 99.4 | sone-rune 99.3 |
| Playing, window on a hidden workspace | sone-rune 99.2 | – |
| Playing, window in the scratchpad (hidden) | sone-rune 99.5 | sone-rune 99.3 |
| Big list open, playing (row shows playing bars) | llvmpipe-# 211, sone-rune 24.6 | llvmpipe-# 208, sone-rune 24.3 |
| Big list, wheel-scrolling, playing | sone-rune 96.5, llvmpipe-# 59.8 | sone-rune 95.7, llvmpipe-# 62.9 |
| Big list open, paused | – | – |

</details>

| Timing (s) | base-debug | base-release |
|---|---|---|
| Launch → window mapped (first frame) | 2.970 (2.901–2.972) | 2.927 (2.888–3.668) |
| Launch → MCP answers | 2.140 (2.116–2.166) | 2.147 (2.127–2.877) |
| Launch → Home button on AT-SPI | 3.002 (2.967–3.023) | 2.954 (2.926–3.730) |
| Launch → Home content on AT-SPI | 3.128 (3.013–3.151) | 3.077 (3.045–3.863) |
| Close request → process exited | 0.042 (0.042–0.069) | 0.042 (0.042–0.042) |
| Big list: press → its heading on AT-SPI | 0.219 (0.217–0.432) | 0.315 (0.208–0.445) |

Sessions (machine load, 1-min loadavg at start → end):

- base-debug: 12:04:53 nan→nan (0 s), 12:23:38 nan→nan (0 s), 12:34:55 nan→nan (0 s)
- base-release: 12:08:37 nan→nan (0 s), 12:19:54 nan→nan (0 s), 12:38:40 nan→nan (0 s)

## GPU

| State | took | fps base-debug | fps base-release | views/s base-debug | views/s base-release | polls/s base-debug | polls/s base-release | CPU % base-debug | CPU % base-release | PSS MB base-debug | PSS MB base-release |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 2/2 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.3 (0.3–0.4) | 0.4 (0.3–0.5) | 160 (151–161) | 141 (138–144) |
| Player closed, playing | 3/3 2/2 | 1.0 (1.0–1.0) | 0.9 (0.9–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 1.3 (1.0–1.8) | 1.2 (0.9–1.6) | 182 (172–190) | 172 (171–173) |
| Player open, playing | 3/3 2/2 | 1.0 (1.0–1.0) | 1.1 (1.0–1.1) | 1.0 (1.0–1.0) | 1.1 (1.0–1.1) | 5.0 (4.9–5.0) | 5.0 (5.0–5.0) | 1.1 (1.1–2.0) | 1.1 (0.9–1.4) | 194 (191–203) | 183 (178–188) |
| Player open, paused | 3/3 2/2 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.3 (0.3–0.3) | 0.5 (0.4–0.6) | 194 (191–202) | 183 (178–188) |
| Player open, playing, window unfocused (visible) | 3/3 2/2 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 1.1 (1.0–1.1) | 1.2 (1.1–1.3) | 194 (193–196) | 184 (178–189) |
| Playing, window covered by a fullscreen window | 3/3 2/2 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 100.0 (99.9–100.1) | 99.3 (99.0–99.7) | 202 (194–202) | 177 (175–178) |
| Playing, window on a hidden workspace | 3/3 2/2 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 100.1 (100.0–100.1) | 0.8 (0.7–0.9) | 204 (196–205) | 182 (179–184) |
| Playing, window in the scratchpad (hidden) | 3/3 2/2 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 99.9 (99.0–100.1) | 100.1 (99.9–100.2) | 205 (204–205) | 181 (177–184) |
| Big list open, playing (row shows playing bars) | 3/3 2/2 | 59.8 (52.9–60.1) | 60.0 (60.0–60.1) | 1.0 (1.0–1.1) | 1.0 (1.0–1.0) | 59.9 (52.9–60.1) | 60.0 (60.0–60.1) | 18.8 (17.4–36.9) | 17.8 (17.3–18.2) | 362 (355–367) | 339 (337–342) |
| Big list, wheel-scrolling, playing | 3/3 2/2 | 14.9 (7.1–17.9) | 20.6 (20.4–20.9) | 14.9 (7.1–17.9) | 19.3 (19.2–19.4) | 14.9 (7.1–17.9) | 20.6 (20.4–20.9) | 97.7 (83.3–100.6) | 98.8 (98.4–99.3) | 357 (355–362) | 340 (339–342) |
| Big list open, paused | 3/3 2/2 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.4 (0.3–0.7) | 0.3 (0.3–0.3) | 358 (357–363) | 336 (334–339) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 0.9 (0.9–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.1 (1.0–1.1) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 59.8 (52.9–60.1) | 60.0 (60.0–60.1) |
| Big list, wheel-scrolling, playing | 14.9 (7.1–17.9) | 20.6 (20.4–20.9) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Unchanged frames/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.1 (1.0–1.1) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 1.0 (1.0–1.1) | 1.0 (1.0–1.0) |
| Big list, wheel-scrolling, playing | 14.9 (7.1–17.9) | 19.3 (19.2–19.4) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Polls/s**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player closed, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing | 5.0 (4.9–5.0) | 5.0 (5.0–5.0) |
| Player open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing, window unfocused (visible) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 59.9 (52.9–60.1) | 60.0 (60.0–60.1) |
| Big list, wheel-scrolling, playing | 14.9 (7.1–17.9) | 20.6 (20.4–20.9) |
| Big list open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |

**CPU, % of a core (perf line)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.3 (0.3–0.4) | 0.4 (0.3–0.5) |
| Player closed, playing | 1.3 (1.0–1.8) | 1.2 (0.9–1.6) |
| Player open, playing | 1.1 (1.1–2.0) | 1.1 (0.9–1.4) |
| Player open, paused | 0.3 (0.3–0.3) | 0.5 (0.4–0.6) |
| Player open, playing, window unfocused (visible) | 1.1 (1.0–1.1) | 1.2 (1.1–1.3) |
| Playing, window covered by a fullscreen window | 100.0 (99.9–100.1) | 99.3 (99.0–99.7) |
| Playing, window on a hidden workspace | 100.1 (100.0–100.1) | 0.8 (0.7–0.9) |
| Playing, window in the scratchpad (hidden) | 99.9 (99.0–100.1) | 100.1 (99.9–100.2) |
| Big list open, playing (row shows playing bars) | 18.8 (17.4–36.9) | 17.8 (17.3–18.2) |
| Big list, wheel-scrolling, playing | 97.7 (83.3–100.6) | 98.8 (98.4–99.3) |
| Big list open, paused | 0.4 (0.3–0.7) | 0.3 (0.3–0.3) |

**Compositor CPU, % of a core**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0.1 (0.1–0.1) | 0.2 (0.1–0.3) |
| Player closed, playing | 0.2 (0.1–0.2) | 0.2 (0.2–0.3) |
| Player open, playing | 0.2 (0.1–0.4) | 0.2 (0.1–0.2) |
| Player open, paused | 0.1 (0.0–0.1) | 0.2 (0.2–0.2) |
| Player open, playing, window unfocused (visible) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |
| Playing, window covered by a fullscreen window | 0.1 (0.1–0.2) | 0.2 (0.1–0.2) |
| Playing, window on a hidden workspace | 0.1 (0.1–0.2) | 0.2 (0.1–0.3) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.2) | 0.1 (0.0–0.1) |
| Big list open, playing (row shows playing bars) | 1.7 (1.5–3.1) | 1.9 (1.9–1.9) |
| Big list, wheel-scrolling, playing | 0.9 (0.8–0.9) | 0.9 (0.9–0.9) |
| Big list open, paused | 0.1 (0.1–0.2) | 0.2 (0.1–0.2) |

**RSS MB (perf line)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 279 (278–279) | 263 (263–263) |
| Player closed, playing | 312 (311–315) | 294 (291–298) |
| Player open, playing | 331 (330–332) | 308 (300–315) |
| Player open, paused | 331 (330–332) | 308 (300–315) |
| Player open, playing, window unfocused (visible) | 332 (331–333) | 308 (300–316) |
| Playing, window covered by a fullscreen window | 333 (331–333) | 308 (301–316) |
| Playing, window on a hidden workspace | 334 (333–334) | 314 (307–322) |
| Playing, window in the scratchpad (hidden) | 334 (333–334) | 314 (307–322) |
| Big list open, playing (row shows playing bars) | 493 (491–496) | 473 (465–481) |
| Big list, wheel-scrolling, playing | 494 (491–496) | 473 (465–481) |
| Big list open, paused | 494 (491–496) | 473 (465–481) |

**PSS MB (smaps_rollup)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 160 (151–161) | 141 (138–144) |
| Player closed, playing | 182 (172–190) | 172 (171–173) |
| Player open, playing | 194 (191–203) | 183 (178–188) |
| Player open, paused | 194 (191–202) | 183 (178–188) |
| Player open, playing, window unfocused (visible) | 194 (193–196) | 184 (178–189) |
| Playing, window covered by a fullscreen window | 202 (194–202) | 177 (175–178) |
| Playing, window on a hidden workspace | 204 (196–205) | 182 (179–184) |
| Playing, window in the scratchpad (hidden) | 205 (204–205) | 181 (177–184) |
| Big list open, playing (row shows playing bars) | 362 (355–367) | 339 (337–342) |
| Big list, wheel-scrolling, playing | 357 (355–362) | 340 (339–342) |
| Big list open, paused | 358 (357–363) | 336 (334–339) |

**GPU memory MiB (nvidia-smi, app)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 100 (100–100) | 98 (96–100) |
| Player closed, playing | 100 (100–100) | 98 (96–100) |
| Player open, playing | 164 (164–164) | 162 (160–164) |
| Player open, paused | 164 (164–164) | 162 (160–164) |
| Player open, playing, window unfocused (visible) | 164 (164–164) | 162 (160–164) |
| Playing, window covered by a fullscreen window | 158 (158–158) | 92 (90–94) |
| Playing, window on a hidden workspace | 164 (164–164) | 98 (96–100) |
| Playing, window in the scratchpad (hidden) | 94 (94–94) | 92 (90–94) |
| Big list open, playing (row shows playing bars) | 100 (100–164) | 98 (96–100) |
| Big list, wheel-scrolling, playing | 100 (100–164) | 98 (96–100) |
| Big list open, paused | 100 (100–164) | 98 (96–100) |

**Machine busy % during window (all 24 cores)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 13.6 (13.4–33.6) | 72.1 (44.4–99.8) |
| Player closed, playing | 32.7 (15.3–69.3) | 44.6 (14.0–75.3) |
| Player open, playing | 22.3 (14.7–91.3) | 25.0 (12.4–37.6) |
| Player open, paused | 16.0 (13.6–18.9) | 64.0 (46.7–81.2) |
| Player open, playing, window unfocused (visible) | 13.6 (12.9–19.4) | 28.0 (17.2–38.7) |
| Playing, window covered by a fullscreen window | 18.3 (17.1–19.7) | 33.0 (25.3–40.8) |
| Playing, window on a hidden workspace | 20.5 (17.4–22.5) | 32.2 (22.0–42.4) |
| Playing, window in the scratchpad (hidden) | 16.8 (16.6–36.0) | 22.7 (22.3–23.1) |
| Big list open, playing (row shows playing bars) | 30.3 (13.8–99.6) | 20.1 (19.7–20.6) |
| Big list, wheel-scrolling, playing | 49.7 (28.3–99.9) | 29.2 (28.2–30.3) |
| Big list open, paused | 38.4 (15.8–99.7) | 29.4 (23.8–35.0) |

**perf.drawn** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 5 (5–5) | 4.75 (4.5–5) |
| Player open, playing | 5 (5–5) | 5.25 (5–5.5) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 5 (5–5) | 5 (5–5) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 299 (264.5–300.5) | 300.25 (300–300.5) |
| Big list, wheel-scrolling, playing | 74.5 (35.5–89.5) | 103.25 (102–104.5) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**perf.drawn_per_s** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 1 (1–1) | 0.95 (0.9–1) |
| Player open, playing | 1 (1–1) | 1.05 (1–1.1) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 1 (1–1) | 1 (1–1) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 59.8 (52.9–60.1) | 60.05 (60–60.1) |
| Big list, wheel-scrolling, playing | 14.9 (7.1–17.9) | 20.65 (20.4–20.9) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**perf.skipped** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | base-debug | base-release |
|---|---|---|
| Idle on Home, nothing played | – | – |
| Player closed, playing | sone-rune 0.6 | sone-rune 0.7 |
| Player open, playing | sone-rune 0.7 | sone-rune 0.65 |
| Player open, paused | – | – |
| Player open, playing, window unfocused (visible) | sone-rune 0.6 | sone-rune 0.5 |
| Playing, window covered by a fullscreen window | sone-rune 99.5 | sone-rune 98.75 |
| Playing, window on a hidden workspace | sone-rune 99.6 | – |
| Playing, window in the scratchpad (hidden) | sone-rune 99.4 | sone-rune 99.5 |
| Big list open, playing (row shows playing bars) | sone-rune 18.4 | sone-rune 17.2 |
| Big list, wheel-scrolling, playing | sone-rune 97.2 | sone-rune 98.1 |
| Big list open, paused | sone-rune 0.5 | – |

</details>

| Timing (s) | base-debug | base-release |
|---|---|---|
| Launch → window mapped (first frame) | 2.750 (2.742–5.482) | 3.562 (2.815–4.310) |
| Launch → MCP answers | 2.176 (2.167–4.488) | 2.742 (2.258–3.225) |
| Launch → Home button on AT-SPI | 2.827 (2.799–5.533) | 3.645 (2.875–4.415) |
| Launch → Home content on AT-SPI | 2.864 (2.836–5.574) | 3.761 (2.913–4.609) |
| Close request → process exited | 0.130 (0.109–0.175) | 0.108 (0.105–0.111) |
| Big list: press → its heading on AT-SPI | 0.340 (0.207–0.614) | 0.360 (0.302–0.418) |

Sessions (machine load, 1-min loadavg at start → end):

- base-debug: 12:12:21 nan→nan (0 s), 12:31:10 nan→nan (0 s), 12:42:24 nan→nan (0 s)
- base-release: 12:16:09 nan→nan (0 s), 12:27:23 nan→nan (0 s)

Failed sessions: gpu base-release run2: no states.json (session cut short)
