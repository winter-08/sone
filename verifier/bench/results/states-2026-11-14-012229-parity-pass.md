# States benchmark 2026-11-14-012229 (parity-pass)

3 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 67 min 29 s.

Builds: `base` = `rune-app/target/baseline/release/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1'}; `final` = `rune-app/target/stage-final/release/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1'}; `parity` = `rune-app/target/stage-parity/release/sone-rune-wrapped` {'SONE_NO_SCHEME_REGISTER': '1'}

## Software rendering (pixman sway, lavapipe)

| State | took | fps base | fps final | fps parity | views/s base | views/s final | views/s parity | polls/s base | polls/s final | polls/s parity | CPU % base | CPU % final | CPU % parity | PSS MB base | PSS MB final | PSS MB parity |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.1 (5.1–5.2) | 0.1 (0.0–0.1) | 0.0 (0.0–0.0) | 0.7 (0.6–0.7) | 255 (252–255) | 286 (283–287) | 307 (307–308) |
| Player closed, playing | 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 7.4 (7.2–7.4) | 6.8 (6.5–7.1) | 6.9 (6.6–6.9) | 263 (263–267) | 290 (289–291) | 312 (312–312) |
| Player open, playing | 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 28.1 (27.4–28.1) | 27.6 (26.9–27.7) | 27.9 (27.8–27.9) | 312 (310–326) | 338 (338–346) | 371 (371–373) |
| Player open, paused | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.1 (0.0–0.1) | 0.0 (0.0–0.1) | 0.1 (0.1–0.2) | 312 (310–326) | 339 (338–346) | 372 (371–373) |
| Player open, playing, window unfocused (visible) | 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 28.3 (28.1–28.4) | 27.8 (27.6–27.8) | 27.4 (26.5–27.7) | 310 (307–323) | 336 (336–344) | 369 (368–370) |
| Playing, window covered by a fullscreen window | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 99.9 (99.8–99.9) | 0.3 (0.3–0.7) | 0.4 (0.3–0.4) | 300 (293–303) | 332 (330–338) | 362 (362–364) |
| Playing, window on a hidden workspace | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 0.2 (0.2–0.7) | 0.3 (0.2–0.4) | 0.3 (0.3–0.3) | 316 (309–319) | 344 (343–350) | 375 (374–376) |
| Playing, window in the scratchpad (hidden) | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 99.9 (99.9–100.0) | 0.3 (0.3–0.3) | 0.3 (0.3–0.3) | 303 (296–307) | 331 (330–338) | 363 (362–364) |
| Big list open, playing (row shows playing bars) | 3/3 3/3 3/3 | 61.0 (61.0–61.0) | 62.4 (62.4–62.5) | 62.4 (47.2–62.5) | 1.1 (1.1–1.1) | 1.1 (1.0–1.2) | 1.2 (1.0–1.2) | 61.1 (61.0–61.1) | 62.4 (62.4–62.4) | 62.4 (47.2–62.5) | 225.8 (225.7–226.7) | 231.2 (229.8–231.3) | 249.3 (194.2–249.5) | 478 (471–480) | 358 (355–364) | 398 (389–404) |
| Big list, wheel-scrolling, playing | 3/3 3/3 3/3 | 25.6 (13.4–25.9) | 62.4 (62.4–62.5) | 62.4 (62.4–62.4) | 20.3 (12.6–20.4) | 1.0 (0.9–1.0) | 1.0 (1.0–1.0) | 25.6 (13.4–25.9) | 62.5 (62.4–62.6) | 62.4 (62.4–62.4) | 180.6 (137.9–181.7) | 230.7 (229.2–231.4) | 270.4 (265.2–275.9) | 476 (469–477) | 358 (355–363) | 396 (391–404) |
| Big list open, paused | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.0–0.1) | 474 (466–476) | 353 (350–358) | 396 (386–398) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 61.0 (61.0–61.0) | 62.4 (62.4–62.5) | 62.4 (47.2–62.5) |
| Big list, wheel-scrolling, playing | 25.6 (13.4–25.9) | 62.4 (62.4–62.5) | 62.4 (62.4–62.4) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Unchanged frames/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.1) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 1.1 (1.1–1.1) | 1.1 (1.0–1.2) | 1.2 (1.0–1.2) |
| Big list, wheel-scrolling, playing | 20.3 (12.6–20.4) | 1.0 (0.9–1.0) | 1.0 (1.0–1.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Polls/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.1 (5.1–5.2) |
| Player closed, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing, window unfocused (visible) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Big list open, playing (row shows playing bars) | 61.1 (61.0–61.1) | 62.4 (62.4–62.4) | 62.4 (47.2–62.5) |
| Big list, wheel-scrolling, playing | 25.6 (13.4–25.9) | 62.5 (62.4–62.6) | 62.4 (62.4–62.4) |
| Big list open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |

**Regions/s (perf line, newer builds only)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | – | 1.1 (1.0–1.2) | 1.2 (1.0–1.2) |
| Big list, wheel-scrolling, playing | – | 1.0 (0.9–1.0) | 1.0 (1.0–1.0) |
| Big list open, paused | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**CPU, % of a core (perf line)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.1 (0.0–0.1) | 0.0 (0.0–0.0) | 0.7 (0.6–0.7) |
| Player closed, playing | 7.4 (7.2–7.4) | 6.8 (6.5–7.1) | 6.9 (6.6–6.9) |
| Player open, playing | 28.1 (27.4–28.1) | 27.6 (26.9–27.7) | 27.9 (27.8–27.9) |
| Player open, paused | 0.1 (0.0–0.1) | 0.0 (0.0–0.1) | 0.1 (0.1–0.2) |
| Player open, playing, window unfocused (visible) | 28.3 (28.1–28.4) | 27.8 (27.6–27.8) | 27.4 (26.5–27.7) |
| Playing, window covered by a fullscreen window | 99.9 (99.8–99.9) | 0.3 (0.3–0.7) | 0.4 (0.3–0.4) |
| Playing, window on a hidden workspace | 0.2 (0.2–0.7) | 0.3 (0.2–0.4) | 0.3 (0.3–0.3) |
| Playing, window in the scratchpad (hidden) | 99.9 (99.9–100.0) | 0.3 (0.3–0.3) | 0.3 (0.3–0.3) |
| Big list open, playing (row shows playing bars) | 225.8 (225.7–226.7) | 231.2 (229.8–231.3) | 249.3 (194.2–249.5) |
| Big list, wheel-scrolling, playing | 180.6 (137.9–181.7) | 230.7 (229.2–231.4) | 270.4 (265.2–275.9) |
| Big list open, paused | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.0–0.1) |

**Compositor CPU, % of a core**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.1) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.1) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.4 (0.3–0.4) | 0.3 (0.3–0.4) | 0.4 (0.3–0.6) |
| Big list, wheel-scrolling, playing | 0.3 (0.3–0.4) | 0.4 (0.4–0.5) | 0.6 (0.4–0.6) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**RSS MB (perf line)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 271 (267–271) | 305 (302–306) | 326 (326–327) |
| Player closed, playing | 286 (285–290) | 316 (314–317) | 338 (337–338) |
| Player open, playing | 335 (332–348) | 363 (363–372) | 397 (396–398) |
| Player open, paused | 335 (332–348) | 364 (364–372) | 398 (396–398) |
| Player open, playing, window unfocused (visible) | 335 (332–348) | 364 (364–372) | 397 (396–398) |
| Playing, window covered by a fullscreen window | 323 (316–326) | 357 (356–364) | 388 (387–390) |
| Playing, window on a hidden workspace | 339 (332–342) | 370 (369–376) | 401 (400–402) |
| Playing, window in the scratchpad (hidden) | 326 (319–329) | 357 (356–363) | 389 (388–389) |
| Big list open, playing (row shows playing bars) | 501 (494–502) | 384 (380–389) | 424 (415–429) |
| Big list, wheel-scrolling, playing | 501 (494–503) | 386 (383–391) | 426 (419–432) |
| Big list open, paused | 502 (494–503) | 386 (383–392) | 426 (419–432) |

**PSS MB (smaps_rollup)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 255 (252–255) | 286 (283–287) | 307 (307–308) |
| Player closed, playing | 263 (263–267) | 290 (289–291) | 312 (312–312) |
| Player open, playing | 312 (310–326) | 338 (338–346) | 371 (371–373) |
| Player open, paused | 312 (310–326) | 339 (338–346) | 372 (371–373) |
| Player open, playing, window unfocused (visible) | 310 (307–323) | 336 (336–344) | 369 (368–370) |
| Playing, window covered by a fullscreen window | 300 (293–303) | 332 (330–338) | 362 (362–364) |
| Playing, window on a hidden workspace | 316 (309–319) | 344 (343–350) | 375 (374–376) |
| Playing, window in the scratchpad (hidden) | 303 (296–307) | 331 (330–338) | 363 (362–364) |
| Big list open, playing (row shows playing bars) | 478 (471–480) | 358 (355–364) | 398 (389–404) |
| Big list, wheel-scrolling, playing | 476 (469–477) | 358 (355–363) | 396 (391–404) |
| Big list open, paused | 474 (466–476) | 353 (350–358) | 396 (386–398) |

**Machine busy % during window (all 24 cores)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 2.7 (2.7–11.5) | 2.6 (2.4–2.8) | 2.7 (2.5–9.9) |
| Player closed, playing | 2.9 (2.8–6.7) | 4.1 (2.9–7.4) | 2.9 (2.6–5.6) |
| Player open, playing | 3.7 (3.4–73.6) | 4.0 (3.9–83.5) | 3.7 (3.6–5.7) |
| Player open, paused | 2.5 (2.5–10.2) | 2.6 (2.4–11.5) | 16.6 (2.8–91.4) |
| Player open, playing, window unfocused (visible) | 3.9 (3.5–3.9) | 4.7 (3.9–9.1) | 13.8 (3.7–86.1) |
| Playing, window covered by a fullscreen window | 6.6 (6.5–6.9) | 2.8 (2.7–57.7) | 3.1 (2.8–11.6) |
| Playing, window on a hidden workspace | 2.5 (2.1–70.0) | 3.0 (2.4–9.4) | 2.8 (2.5–4.7) |
| Playing, window in the scratchpad (hidden) | 6.7 (6.5–10.9) | 2.4 (2.3–2.5) | 2.6 (2.5–2.7) |
| Big list open, playing (row shows playing bars) | 12.4 (11.9–12.7) | 12.4 (12.4–12.8) | 13.2 (12.9–78.8) |
| Big list, wheel-scrolling, playing | 10.4 (10.0–64.2) | 12.6 (12.2–12.8) | 17.3 (14.4–19.0) |
| Big list open, paused | 2.9 (2.7–6.5) | 2.3 (2.1–2.6) | 2.9 (2.3–75.1) |

**perf.drawn** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0.5 (0.5–0.5) |
| Player closed, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 305 (305–305) | 312 (312–312.5) | 312 (236–312.5) |
| Big list, wheel-scrolling, playing | 128 (67–129.5) | 312 (312–312.5) | 312 (312–312) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.drawn_per_s** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0.1 (0.1–0.1) |
| Player closed, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 61 (61–61) | 62.4 (62.4–62.5) | 62.4 (47.2–62.5) |
| Big list, wheel-scrolling, playing | 25.6 (13.4–25.9) | 62.4 (62.4–62.5) | 62.4 (62.4–62.4) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.skipped** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0.5) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | – | – | llvmpipe-# 0.6 |
| Player closed, playing | llvmpipe-# 6.8, sone-rune 0.6 | llvmpipe-# 6.9, sone-rune 0.5 | llvmpipe-# 6.1, sone-rune 0.5 |
| Player open, playing | llvmpipe-# 26.9, sone-rune 0.6 | llvmpipe-# 26.5, sone-rune 0.7 | llvmpipe-# 27.1, sone-rune 0.5 |
| Player open, paused | – | – | – |
| Player open, playing, window unfocused (visible) | llvmpipe-# 27.1, sone-rune 0.6 | llvmpipe-# 27.1, sone-rune 0.6 | llvmpipe-# 26.5, sone-rune 0.6 |
| Playing, window covered by a fullscreen window | sone-rune 99.5 | – | – |
| Playing, window on a hidden workspace | – | – | – |
| Playing, window in the scratchpad (hidden) | sone-rune 99.6 | – | – |
| Big list open, playing (row shows playing bars) | llvmpipe-# 209.1, sone-rune 16.7 | llvmpipe-# 218.6, sone-rune 11.8 | llvmpipe-# 234.5, sone-rune 14.9 |
| Big list, wheel-scrolling, playing | sone-rune 91.1, llvmpipe-# 89.4 | llvmpipe-# 214.1, sone-rune 15.7 | llvmpipe-# 247.3, sone-rune 22.9 |
| Big list open, paused | – | – | – |

</details>

| Timing (s) | base | final | parity |
|---|---|---|---|
| Launch → window mapped (first frame) | 2.240 (2.199–2.866) | 2.949 (2.885–3.186) | 3.068 (3.065–3.088) |
| Launch → MCP answers | 1.517 (1.480–2.099) | 1.540 (1.532–1.767) | 1.499 (1.498–1.506) |
| Launch → Home button on AT-SPI | 2.290 (2.260–2.899) | 2.996 (2.944–3.223) | 3.113 (3.107–3.121) |
| Launch → Home content on AT-SPI | 2.378 (2.336–3.025) | 3.050 (2.993–3.274) | 3.168 (3.161–3.179) |
| Close request → process exited | 0.037 (0.037–0.037) | 0.022 (0.021–0.022) | 2.580 (2.570–2.881) |
| Big list: press → its heading on AT-SPI | 0.197 (0.193–0.202) | 0.186 (0.146–0.188) | 0.224 (0.183–0.225) |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- base run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 495 MB
- base run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 504 MB
- base run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 502 MB
- final run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 387 MB
- final run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 392 MB
- final run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 383 MB
- parity run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 420 MB
- parity run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 427 MB
- parity run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 432 MB

Sessions (machine load, 1-min loadavg at start → end):

- base: 01:22:29 6.4→8.4 (224 s), 01:52:28 3.7→0.8 (223 s), 02:07:28 1.0→2.1 (223 s)
- final: 01:26:13 8.4→5.7 (224 s), 01:48:44 7.2→3.7 (223 s), 02:11:11 2.1→2.3 (223 s)
- parity: 01:29:57 5.7→5.7 (226 s), 01:44:59 1.0→7.2 (226 s), 02:14:54 2.3→1.8 (226 s)

## GPU

| State | took | fps base | fps final | fps parity | views/s base | views/s final | views/s parity | polls/s base | polls/s final | polls/s parity | CPU % base | CPU % final | CPU % parity | PSS MB base | PSS MB final | PSS MB parity |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.1 (5.1–5.1) | 0.2 (0.2–0.2) | 0.3 (0.1–0.5) | 0.3 (0.2–0.3) | 153 (153–153) | 152 (151–155) | 161 (161–164) |
| Player closed, playing | 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.7 (0.7–0.7) | 0.6 (0.5–0.7) | 0.6 (0.6–0.6) | 176 (175–176) | 171 (169–174) | 183 (181–184) |
| Player open, playing | 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.7 (0.7–0.8) | 0.6 (0.5–0.6) | 0.7 (0.6–0.8) | 185 (184–187) | 182 (179–183) | 194 (193–196) |
| Player open, paused | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.2 (0.1–0.2) | 0.2 (0.1–0.2) | 0.2 (0.2–0.2) | 185 (184–187) | 183 (180–184) | 195 (195–197) |
| Player open, playing, window unfocused (visible) | 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.7 (0.7–0.8) | 0.6 (0.5–0.6) | 0.7 (0.6–0.7) | 186 (185–187) | 185 (182–186) | 195 (195–197) |
| Playing, window covered by a fullscreen window | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 100.1 (100.0–100.1) | 0.5 (0.5–1.0) | 0.5 (0.4–0.5) | 187 (185–187) | 186 (184–192) | 201 (195–202) |
| Playing, window on a hidden workspace | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 0.4 (0.4–0.5) | 0.5 (0.5–0.5) | 0.5 (0.4–0.6) | 188 (186–191) | 186 (184–192) | 201 (195–202) |
| Playing, window in the scratchpad (hidden) | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 100.0 (99.9–100.0) | 0.4 (0.3–0.5) | 0.5 (0.4–0.6) | 188 (186–191) | 186 (184–192) | 201 (195–202) |
| Big list open, playing (row shows playing bars) | 3/3 3/3 3/3 | 60.8 (60.8–60.9) | 62.0 (62.0–62.1) | 62.0 (62.0–62.0) | 1.0 (1.0–1.0) | 1.1 (0.9–1.4) | 1.0 (0.9–1.1) | 60.8 (60.7–60.9) | 62.0 (62.0–62.1) | 62.0 (62.0–62.0) | 10.8 (10.5–11.1) | 5.7 (5.6–6.0) | 7.0 (6.9–7.2) | 351 (349–354) | 197 (196–204) | 212 (207–214) |
| Big list, wheel-scrolling, playing | 3/3 3/3 3/3 | 28.2 (28.1–28.9) | 62.0 (61.8–62.0) | 62.1 (62.0–62.1) | 20.3 (20.3–20.4) | 1.0 (0.9–1.2) | 1.0 (0.9–1.0) | 28.2 (28.1–28.9) | 62.0 (62.0–62.0) | 62.0 (62.0–62.1) | 90.7 (90.3–91.5) | 9.5 (8.5–13.7) | 12.2 (11.0–12.4) | 352 (349–355) | 199 (198–206) | 215 (210–217) |
| Big list open, paused | 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.2 (0.1–0.2) | 0.2 (0.1–0.2) | 0.2 (0.2–0.2) | 352 (349–355) | 199 (198–206) | 215 (210–217) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 60.8 (60.8–60.9) | 62.0 (62.0–62.1) | 62.0 (62.0–62.0) |
| Big list, wheel-scrolling, playing | 28.2 (28.1–28.9) | 62.0 (61.8–62.0) | 62.1 (62.0–62.1) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Unchanged frames/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.2) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.1) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 1.0 (1.0–1.0) | 1.1 (0.9–1.4) | 1.0 (0.9–1.1) |
| Big list, wheel-scrolling, playing | 20.3 (20.3–20.4) | 1.0 (0.9–1.2) | 1.0 (0.9–1.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Polls/s**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.1 (5.1–5.1) |
| Player closed, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing, window unfocused (visible) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Big list open, playing (row shows playing bars) | 60.8 (60.7–60.9) | 62.0 (62.0–62.1) | 62.0 (62.0–62.0) |
| Big list, wheel-scrolling, playing | 28.2 (28.1–28.9) | 62.0 (62.0–62.0) | 62.0 (62.0–62.1) |
| Big list open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |

**Regions/s (perf line, newer builds only)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | – | 1.1 (0.9–1.4) | 1.0 (0.9–1.1) |
| Big list, wheel-scrolling, playing | – | 1.0 (0.9–1.2) | 1.0 (0.9–1.0) |
| Big list open, paused | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**CPU, % of a core (perf line)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.2 (0.2–0.2) | 0.3 (0.1–0.5) | 0.3 (0.2–0.3) |
| Player closed, playing | 0.7 (0.7–0.7) | 0.6 (0.5–0.7) | 0.6 (0.6–0.6) |
| Player open, playing | 0.7 (0.7–0.8) | 0.6 (0.5–0.6) | 0.7 (0.6–0.8) |
| Player open, paused | 0.2 (0.1–0.2) | 0.2 (0.1–0.2) | 0.2 (0.2–0.2) |
| Player open, playing, window unfocused (visible) | 0.7 (0.7–0.8) | 0.6 (0.5–0.6) | 0.7 (0.6–0.7) |
| Playing, window covered by a fullscreen window | 100.1 (100.0–100.1) | 0.5 (0.5–1.0) | 0.5 (0.4–0.5) |
| Playing, window on a hidden workspace | 0.4 (0.4–0.5) | 0.5 (0.5–0.5) | 0.5 (0.4–0.6) |
| Playing, window in the scratchpad (hidden) | 100.0 (99.9–100.0) | 0.4 (0.3–0.5) | 0.5 (0.4–0.6) |
| Big list open, playing (row shows playing bars) | 10.8 (10.5–11.1) | 5.7 (5.6–6.0) | 7.0 (6.9–7.2) |
| Big list, wheel-scrolling, playing | 90.7 (90.3–91.5) | 9.5 (8.5–13.7) | 12.2 (11.0–12.4) |
| Big list open, paused | 0.2 (0.1–0.2) | 0.2 (0.1–0.2) | 0.2 (0.2–0.2) |

**Compositor CPU, % of a core**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0.1 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.0–0.1) |
| Player closed, playing | 0.1 (0.0–0.1) | 0.1 (0.1–0.2) | 0.1 (0.0–0.2) |
| Player open, playing | 0.1 (0.1–0.1) | 0.1 (0.0–0.2) | 0.1 (0.0–0.2) |
| Player open, paused | 0.1 (0.0–0.1) | 0.1 (0.0–0.2) | 0.0 (0.0–0.1) |
| Player open, playing, window unfocused (visible) | 0.1 (0.1–0.2) | 0.1 (0.1–0.2) | 0.1 (0.0–0.2) |
| Playing, window covered by a fullscreen window | 0.1 (0.0–0.2) | 0.1 (0.1–0.2) | 0.1 (0.0–0.1) |
| Playing, window on a hidden workspace | 0.1 (0.1–0.1) | 0.1 (0.0–0.1) | 0.1 (0.0–0.1) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.0 (0.0–0.2) |
| Big list open, playing (row shows playing bars) | 1.2 (1.1–1.2) | 1.1 (1.1–1.2) | 1.1 (1.1–1.2) |
| Big list, wheel-scrolling, playing | 0.8 (0.7–0.9) | 1.2 (1.1–1.8) | 1.2 (1.2–1.3) |
| Big list open, paused | 0.1 (0.0–0.2) | 0.1 (0.0–0.2) | 0.1 (0.1–0.1) |

**RSS MB (perf line)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 268 (267–268) | 269 (268–272) | 279 (278–282) |
| Player closed, playing | 297 (297–298) | 296 (294–299) | 308 (306–309) |
| Player open, playing | 309 (308–310) | 306 (303–308) | 319 (319–321) |
| Player open, paused | 309 (308–310) | 307 (305–309) | 322 (322–324) |
| Player open, playing, window unfocused (visible) | 309 (308–310) | 312 (309–313) | 322 (322–324) |
| Playing, window covered by a fullscreen window | 311 (309–311) | 312 (310–318) | 328 (322–329) |
| Playing, window on a hidden workspace | 311 (310–315) | 312 (311–318) | 328 (322–330) |
| Playing, window in the scratchpad (hidden) | 311 (310–315) | 312 (311–318) | 328 (322–330) |
| Big list open, playing (row shows playing bars) | 476 (473–479) | 323 (322–330) | 339 (334–341) |
| Big list, wheel-scrolling, playing | 476 (473–479) | 325 (324–333) | 342 (337–345) |
| Big list open, paused | 476 (473–479) | 325 (325–333) | 343 (337–345) |

**PSS MB (smaps_rollup)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 153 (153–153) | 152 (151–155) | 161 (161–164) |
| Player closed, playing | 176 (175–176) | 171 (169–174) | 183 (181–184) |
| Player open, playing | 185 (184–187) | 182 (179–183) | 194 (193–196) |
| Player open, paused | 185 (184–187) | 183 (180–184) | 195 (195–197) |
| Player open, playing, window unfocused (visible) | 186 (185–187) | 185 (182–186) | 195 (195–197) |
| Playing, window covered by a fullscreen window | 187 (185–187) | 186 (184–192) | 201 (195–202) |
| Playing, window on a hidden workspace | 188 (186–191) | 186 (184–192) | 201 (195–202) |
| Playing, window in the scratchpad (hidden) | 188 (186–191) | 186 (184–192) | 201 (195–202) |
| Big list open, playing (row shows playing bars) | 351 (349–354) | 197 (196–204) | 212 (207–214) |
| Big list, wheel-scrolling, playing | 352 (349–355) | 199 (198–206) | 215 (210–217) |
| Big list open, paused | 352 (349–355) | 199 (198–206) | 215 (210–217) |

**GPU memory MiB (nvidia-smi, app)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 100 (96–100) | 68 (68–68) | 96 (96–96) |
| Player closed, playing | 100 (96–100) | 68 (68–68) | 96 (96–96) |
| Player open, playing | 164 (160–164) | 100 (100–100) | 160 (160–160) |
| Player open, paused | 164 (160–164) | 100 (100–100) | 160 (160–160) |
| Player open, playing, window unfocused (visible) | 164 (160–164) | 100 (100–100) | 160 (160–160) |
| Playing, window covered by a fullscreen window | 94 (90–94) | 94 (94–94) | 154 (154–154) |
| Playing, window on a hidden workspace | 100 (96–100) | 100 (100–100) | 160 (160–160) |
| Playing, window in the scratchpad (hidden) | 94 (90–94) | 94 (94–94) | 154 (154–154) |
| Big list open, playing (row shows playing bars) | 164 (160–164) | 100 (100–100) | 160 (160–160) |
| Big list, wheel-scrolling, playing | 164 (160–164) | 100 (100–100) | 160 (160–160) |
| Big list open, paused | 164 (160–164) | 100 (100–100) | 160 (160–160) |

**Machine busy % during window (all 24 cores)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 2.6 (2.5–2.7) | 2.5 (2.5–60.5) | 2.6 (2.4–2.8) |
| Player closed, playing | 2.9 (2.6–4.4) | 2.5 (2.5–10.4) | 2.7 (2.3–2.9) |
| Player open, playing | 2.6 (2.5–2.7) | 2.5 (2.4–2.5) | 2.8 (2.7–14.8) |
| Player open, paused | 2.4 (2.4–2.4) | 2.6 (2.4–2.7) | 2.8 (2.6–3.2) |
| Player open, playing, window unfocused (visible) | 2.7 (2.7–3.3) | 2.6 (2.4–6.2) | 2.7 (2.7–3.2) |
| Playing, window covered by a fullscreen window | 6.7 (6.6–6.7) | 2.7 (2.6–75.7) | 2.8 (2.8–3.0) |
| Playing, window on a hidden workspace | 2.5 (2.5–2.5) | 2.7 (2.5–11.1) | 2.6 (2.6–2.7) |
| Playing, window in the scratchpad (hidden) | 6.7 (6.6–7.0) | 2.7 (2.5–2.8) | 2.7 (2.6–19.5) |
| Big list open, playing (row shows playing bars) | 3.1 (3.0–3.2) | 3.0 (2.8–3.0) | 3.3 (3.0–3.3) |
| Big list, wheel-scrolling, playing | 6.8 (6.5–6.8) | 3.4 (2.9–20.1) | 3.1 (3.0–3.6) |
| Big list open, paused | 2.7 (2.7–6.4) | 2.5 (2.4–2.9) | 2.3 (2.3–2.4) |

**perf.drawn** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0.5 (0.5–0.5) |
| Player closed, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 304 (304–304.5) | 310 (310–310.5) | 310 (310–310) |
| Big list, wheel-scrolling, playing | 141 (140.5–144.5) | 310 (309–310) | 310.5 (310–310.5) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.drawn_per_s** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0.1 (0.1–0.1) |
| Player closed, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 60.8 (60.8–60.9) | 62 (62–62.1) | 62 (62–62) |
| Big list, wheel-scrolling, playing | 28.2 (28.1–28.9) | 62 (61.8–62) | 62.1 (62–62.1) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.skipped** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–1) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | base | final | parity |
|---|---|---|---|
| Idle on Home, nothing played | – | – | – |
| Player closed, playing | sone-rune 0.5 | – | sone-rune 0.5 |
| Player open, playing | – | – | – |
| Player open, paused | – | – | – |
| Player open, playing, window unfocused (visible) | – | – | – |
| Playing, window covered by a fullscreen window | sone-rune 99.7 | – | – |
| Playing, window on a hidden workspace | – | – | – |
| Playing, window in the scratchpad (hidden) | sone-rune 99.6 | – | – |
| Big list open, playing (row shows playing bars) | sone-rune 10.5 | sone-rune 5.3 | sone-rune 6.5 |
| Big list, wheel-scrolling, playing | sone-rune 90.4 | sone-rune 9.2 | sone-rune 11.7 |
| Big list open, paused | – | – | – |

</details>

| Timing (s) | base | final | parity |
|---|---|---|---|
| Launch → window mapped (first frame) | 1.968 (1.931–1.977) | 2.289 (2.285–2.470) | 2.341 (2.292–2.410) |
| Launch → MCP answers | 1.485 (1.449–1.501) | 1.478 (1.472–1.603) | 1.480 (1.450–1.537) |
| Launch → Home button on AT-SPI | 2.032 (2.002–2.051) | 2.344 (2.344–2.545) | 2.403 (2.374–2.462) |
| Launch → Home content on AT-SPI | 2.065 (2.036–2.085) | 2.390 (2.389–2.601) | 2.455 (2.426–2.510) |
| Close request → process exited | 0.251 (0.189–0.256) | 0.200 (0.169–0.251) | 2.025 (1.956–2.047) |
| Big list: press → its heading on AT-SPI | 0.176 (0.175–0.183) | 0.134 (0.129–0.267) | 0.172 (0.125–0.173) |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- base run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 476 MB
- base run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 474 MB
- base run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 479 MB
- final run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 326 MB
- final run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 325 MB
- final run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 333 MB
- parity run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 345 MB
- parity run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 338 MB
- parity run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0; RSS peak 343 MB

Sessions (machine load, 1-min loadavg at start → end):

- base: 01:33:43 5.7→1.5 (225 s), 02:03:43 0.8→1.0 (225 s), 02:18:40 1.8→1.3 (225 s)
- final: 01:37:28 1.5→3.6 (225 s), 01:59:58 0.5→0.8 (225 s), 02:22:25 1.3→0.7 (225 s)
- parity: 01:41:13 3.6→1.0 (225 s), 01:56:11 0.8→0.5 (227 s), 02:26:10 0.7→0.7 (227 s)

