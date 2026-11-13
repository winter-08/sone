# States benchmark 2026-11-13-135329 (stages-of-record)

3 runs per build and mode, sessions one at a time, builds alternating. Median (min–max) over the runs where the state took. Each state: the first 5 s perf interval after the change is dropped, then 2 interval(s) kept (10 s). Display 1400x900 at 60 Hz, fake TIDAL, virtual audio, Loved Tracks = 3000 tracks (served `limit` rows per request, as TIDAL). Total wall time 89 min 49 s.

Builds: `base` = `rune-app/target/baseline/release/sone-rune-wrapped`; `bump` = `rune-app/target/stage-bump/release/sone-rune-wrapped`; `perf` = `rune-app/target/stage-perf/release/sone-rune-wrapped`; `final` = `rune-app/target/stage-final/release/sone-rune-wrapped`

## Software rendering (pixman sway, lavapipe)

| State | took | fps base | fps bump | fps perf | fps final | views/s base | views/s bump | views/s perf | views/s final | polls/s base | polls/s bump | polls/s perf | polls/s final | CPU % base | CPU % bump | CPU % perf | CPU % final | PSS MB base | PSS MB bump | PSS MB perf | PSS MB final |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.1 (0.0–0.1) | 0.1 (0.1–0.1) | 0.1 (0.0–0.2) | 0.1 (0.0–0.1) | 260 (259–263) | 282 (282–285) | 288 (288–290) | 289 (289–292) |
| Player closed, playing | 3/3 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 7.7 (7.6–7.7) | 7.3 (7.3–7.4) | 7.2 (6.5–7.3) | 7.1 (6.9–7.2) | 274 (273–275) | 295 (292–296) | 299 (298–301) | 301 (299–302) |
| Player open, playing | 3/3 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 28.7 (28.6–28.7) | 28.0 (28.0–28.2) | 28.1 (28.0–28.1) | 28.0 (28.0–28.1) | 338 (337–341) | 340 (339–344) | 348 (342–349) | 346 (345–349) |
| Player open, paused | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.1 (0.0–0.1) | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.1–0.1) | 338 (337–341) | 340 (339–344) | 348 (342–350) | 346 (345–349) |
| Player open, playing, window unfocused (visible) | 3/3 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 28.5 (28.5–28.5) | 28.1 (28.1–28.1) | 28.0 (27.9–28.0) | 27.9 (27.8–28.1) | 335 (334–339) | 338 (337–342) | 346 (340–348) | 344 (343–347) |
| Playing, window covered by a fullscreen window | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 99.8 (99.7–99.9) | 0.4 (0.3–0.5) | 0.5 (0.4–0.5) | 0.4 (0.4–0.5) | 304 (303–308) | 334 (333–340) | 340 (338–344) | 341 (338–344) |
| Playing, window on a hidden workspace | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.4 (0.4–0.5) | 0.4 (0.4–0.5) | 0.4 (0.3–0.4) | 0.4 (0.4–0.5) | 321 (320–325) | 346 (345–353) | 353 (350–356) | 353 (350–356) |
| Playing, window in the scratchpad (hidden) | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 99.8 (99.8–100.0) | 0.4 (0.4–0.6) | 0.5 (0.4–0.5) | 0.5 (0.4–0.5) | 308 (307–312) | 334 (333–340) | 340 (338–344) | 340 (338–344) |
| Big list open, playing (row shows playing bars) | 3/3 3/3 3/3 3/3 | 60.7 (60.7–60.7) | 60.9 (60.8–60.9) | 62.3 (62.3–62.4) | 62.4 (62.4–62.4) | 1.0 (1.0–1.1) | 1.0 (1.0–1.0) | 1.0 (1.0–1.1) | 1.0 (1.0–1.0) | 60.8 (60.7–60.8) | 60.9 (60.8–60.9) | 62.4 (62.4–62.4) | 62.4 (62.3–62.4) | 237.5 (237.4–237.7) | 241.3 (240.2–243.2) | 242.2 (240.8–243.7) | 240.7 (238.4–241.5) | 486 (481–495) | 507 (505–512) | 370 (362–376) | 369 (368–375) |
| Big list, wheel-scrolling, playing | 3/3 3/3 3/3 3/3 | 19.3 (19.3–19.9) | 28.5 (28.2–29.0) | 62.4 (62.4–62.4) | 62.4 (62.3–62.4) | 19.3 (19.2–19.8) | 20.5 (20.5–20.6) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 19.3 (19.3–19.9) | 28.5 (28.2–29.1) | 62.4 (62.4–62.4) | 62.5 (62.5–62.6) | 163.1 (162.9–165.4) | 190.1 (189.3–190.9) | 241.2 (241.1–242.3) | 240.7 (239.2–243.0) | 486 (481–496) | 507 (506–512) | 372 (364–378) | 370 (370–377) |
| Big list open, paused | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.1–0.2) | 0.0 (0.0–0.0) | 486 (481–496) | 507 (505–512) | 374 (366–379) | 372 (372–378) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 60.7 (60.7–60.7) | 60.9 (60.8–60.9) | 62.3 (62.3–62.4) | 62.4 (62.4–62.4) |
| Big list, wheel-scrolling, playing | 19.3 (19.3–19.9) | 28.5 (28.2–29.0) | 62.4 (62.4–62.4) | 62.4 (62.3–62.4) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Unchanged frames/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.1–0.2) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 1.0 (1.0–1.1) | 1.0 (1.0–1.0) | 1.0 (1.0–1.1) | 1.0 (1.0–1.0) |
| Big list, wheel-scrolling, playing | 19.3 (19.2–19.8) | 20.5 (20.5–20.6) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Polls/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player closed, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing, window unfocused (visible) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Big list open, playing (row shows playing bars) | 60.8 (60.7–60.8) | 60.9 (60.8–60.9) | 62.4 (62.4–62.4) | 62.4 (62.3–62.4) |
| Big list, wheel-scrolling, playing | 19.3 (19.3–19.9) | 28.5 (28.2–29.1) | 62.4 (62.4–62.4) | 62.5 (62.5–62.6) |
| Big list open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |

**Regions/s (perf line, newer builds only)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | – | – | 1.0 (1.0–1.1) | 1.0 (1.0–1.0) |
| Big list, wheel-scrolling, playing | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Big list open, paused | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**CPU, % of a core (perf line)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.1 (0.0–0.1) | 0.1 (0.1–0.1) | 0.1 (0.0–0.2) | 0.1 (0.0–0.1) |
| Player closed, playing | 7.7 (7.6–7.7) | 7.3 (7.3–7.4) | 7.2 (6.5–7.3) | 7.1 (6.9–7.2) |
| Player open, playing | 28.7 (28.6–28.7) | 28.0 (28.0–28.2) | 28.1 (28.0–28.1) | 28.0 (28.0–28.1) |
| Player open, paused | 0.1 (0.0–0.1) | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.1–0.1) |
| Player open, playing, window unfocused (visible) | 28.5 (28.5–28.5) | 28.1 (28.1–28.1) | 28.0 (27.9–28.0) | 27.9 (27.8–28.1) |
| Playing, window covered by a fullscreen window | 99.8 (99.7–99.9) | 0.4 (0.3–0.5) | 0.5 (0.4–0.5) | 0.4 (0.4–0.5) |
| Playing, window on a hidden workspace | 0.4 (0.4–0.5) | 0.4 (0.4–0.5) | 0.4 (0.3–0.4) | 0.4 (0.4–0.5) |
| Playing, window in the scratchpad (hidden) | 99.8 (99.8–100.0) | 0.4 (0.4–0.6) | 0.5 (0.4–0.5) | 0.5 (0.4–0.5) |
| Big list open, playing (row shows playing bars) | 237.5 (237.4–237.7) | 241.3 (240.2–243.2) | 242.2 (240.8–243.7) | 240.7 (238.4–241.5) |
| Big list, wheel-scrolling, playing | 163.1 (162.9–165.4) | 190.1 (189.3–190.9) | 241.2 (241.1–242.3) | 240.7 (239.2–243.0) |
| Big list open, paused | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.1–0.2) | 0.0 (0.0–0.0) |

**Compositor CPU, % of a core**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.1) | 0.0 (0.0–0.0) | 0.0 (0.0–0.1) | 0.0 (0.0–0.1) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.1) | 0.0 (0.0–0.0) | 0.1 (0.0–0.1) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.1 (0.0–0.1) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.5 (0.5–0.6) | 0.5 (0.5–0.6) | 0.5 (0.5–0.6) | 0.6 (0.5–0.6) |
| Big list, wheel-scrolling, playing | 0.4 (0.4–0.4) | 0.4 (0.3–0.5) | 0.7 (0.6–0.7) | 0.6 (0.5–0.8) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**RSS MB (perf line)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 271 (271–274) | 297 (297–299) | 303 (303–304) | 304 (303–307) |
| Player closed, playing | 288 (287–289) | 311 (308–312) | 316 (314–318) | 318 (316–318) |
| Player open, playing | 351 (350–355) | 357 (356–361) | 364 (359–366) | 363 (362–366) |
| Player open, paused | 351 (350–355) | 357 (356–361) | 365 (359–367) | 363 (362–366) |
| Player open, playing, window unfocused (visible) | 351 (350–355) | 357 (356–361) | 365 (359–367) | 363 (362–366) |
| Playing, window covered by a fullscreen window | 318 (317–322) | 351 (350–357) | 357 (354–360) | 357 (355–361) |
| Playing, window on a hidden workspace | 334 (333–338) | 363 (362–369) | 370 (367–373) | 370 (367–373) |
| Playing, window in the scratchpad (hidden) | 322 (321–326) | 350 (349–357) | 357 (354–360) | 357 (355–360) |
| Big list open, playing (row shows playing bars) | 499 (495–509) | 524 (522–529) | 387 (379–393) | 386 (385–392) |
| Big list, wheel-scrolling, playing | 500 (495–510) | 524 (522–529) | 389 (381–394) | 387 (387–394) |
| Big list open, paused | 500 (495–510) | 524 (522–530) | 391 (382–396) | 389 (389–396) |

**PSS MB (smaps_rollup)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 260 (259–263) | 282 (282–285) | 288 (288–290) | 289 (289–292) |
| Player closed, playing | 274 (273–275) | 295 (292–296) | 299 (298–301) | 301 (299–302) |
| Player open, playing | 338 (337–341) | 340 (339–344) | 348 (342–349) | 346 (345–349) |
| Player open, paused | 338 (337–341) | 340 (339–344) | 348 (342–350) | 346 (345–349) |
| Player open, playing, window unfocused (visible) | 335 (334–339) | 338 (337–342) | 346 (340–348) | 344 (343–347) |
| Playing, window covered by a fullscreen window | 304 (303–308) | 334 (333–340) | 340 (338–344) | 341 (338–344) |
| Playing, window on a hidden workspace | 321 (320–325) | 346 (345–353) | 353 (350–356) | 353 (350–356) |
| Playing, window in the scratchpad (hidden) | 308 (307–312) | 334 (333–340) | 340 (338–344) | 340 (338–344) |
| Big list open, playing (row shows playing bars) | 486 (481–495) | 507 (505–512) | 370 (362–376) | 369 (368–375) |
| Big list, wheel-scrolling, playing | 486 (481–496) | 507 (506–512) | 372 (364–378) | 370 (370–377) |
| Big list open, paused | 486 (481–496) | 507 (505–512) | 374 (366–379) | 372 (372–378) |

**Machine busy % during window (all 24 cores)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 11.6 (11.2–11.6) | 11.6 (11.0–11.8) | 11.8 (11.1–12.3) | 12.2 (11.4–12.4) |
| Player closed, playing | 11.6 (11.5–12.4) | 11.6 (11.3–11.8) | 12.2 (11.4–13.3) | 12.5 (11.9–12.7) |
| Player open, playing | 12.4 (12.4–12.7) | 12.6 (12.5–13.3) | 13.5 (12.3–13.8) | 12.6 (12.4–13.4) |
| Player open, paused | 11.6 (11.3–11.7) | 11.2 (11.0–11.8) | 12.1 (11.4–12.6) | 12.1 (11.4–12.4) |
| Player open, playing, window unfocused (visible) | 12.7 (12.4–13.0) | 12.5 (12.3–12.7) | 13.3 (12.3–13.9) | 13.7 (13.0–14.0) |
| Playing, window covered by a fullscreen window | 15.6 (15.4–15.8) | 11.2 (11.2–11.8) | 12.3 (11.4–12.5) | 12.3 (11.2–12.6) |
| Playing, window on a hidden workspace | 11.3 (11.0–11.6) | 11.7 (11.2–11.8) | 12.1 (11.1–12.7) | 12.6 (11.1–12.8) |
| Playing, window in the scratchpad (hidden) | 15.6 (15.3–16.1) | 11.2 (11.2–11.6) | 12.2 (11.2–12.2) | 12.8 (11.4–12.9) |
| Big list open, playing (row shows playing bars) | 21.7 (21.5–21.8) | 22.0 (21.5–22.6) | 23.0 (21.7–23.3) | 23.4 (21.6–23.4) |
| Big list, wheel-scrolling, playing | 19.0 (18.9–19.3) | 20.2 (20.0–20.3) | 23.1 (22.2–23.4) | 22.2 (22.0–23.4) |
| Big list open, paused | 11.3 (11.2–11.8) | 11.2 (10.9–11.8) | 11.5 (11.3–13.0) | 12.3 (11.5–12.6) |

**perf.drawn** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 5 (5–5) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 303.5 (303.5–303.5) | 304.5 (304–304.5) | 311.5 (311.5–312) | 312 (312–312) |
| Big list, wheel-scrolling, playing | 96.5 (96.5–99.5) | 142.5 (141–145) | 312 (312–312) | 312 (311.5–312) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.drawn_per_s** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 1 (1–1) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 60.7 (60.7–60.7) | 60.9 (60.8–60.9) | 62.3 (62.3–62.4) | 62.4 (62.4–62.4) |
| Big list, wheel-scrolling, playing | 19.3 (19.3–19.9) | 28.5 (28.2–29) | 62.4 (62.4–62.4) | 62.4 (62.3–62.4) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.skipped** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0.5 (0.5–1) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | – | – | – | – |
| Player closed, playing | llvmpipe-# 7.1, sone-rune 0.7 | llvmpipe-# 6.6, sone-rune 0.6 | llvmpipe-# 6.8, sone-rune 0.5 | llvmpipe-# 6.7, sone-rune 0.5 |
| Player open, playing | llvmpipe-# 27.5, sone-rune 0.7 | llvmpipe-# 27, sone-rune 0.7 | llvmpipe-# 27, sone-rune 0.5 | llvmpipe-# 27.1, sone-rune 0.5 |
| Player open, paused | – | – | – | – |
| Player open, playing, window unfocused (visible) | llvmpipe-# 27.4, sone-rune 0.8 | llvmpipe-# 27.2, sone-rune 0.7 | llvmpipe-# 27, sone-rune 0.5 | llvmpipe-# 26.8, sone-rune 0.5 |
| Playing, window covered by a fullscreen window | sone-rune 99.5 | – | – | – |
| Playing, window on a hidden workspace | – | – | – | – |
| Playing, window in the scratchpad (hidden) | sone-rune 99.5 | – | – | – |
| Big list open, playing (row shows playing bars) | llvmpipe-# 214.9, sone-rune 22.4 | llvmpipe-# 219.2, sone-rune 22 | llvmpipe-# 227, sone-rune 15.5 | llvmpipe-# 224.9, sone-rune 15.7 |
| Big list, wheel-scrolling, playing | sone-rune 95.9, llvmpipe-# 66.9 | llvmpipe-# 101.3, sone-rune 88.5 | llvmpipe-# 221.2, sone-rune 20.4 | llvmpipe-# 220.2, sone-rune 20.2 |
| Big list open, paused | – | – | – | – |

</details>

| Timing (s) | base | bump | perf | final |
|---|---|---|---|---|
| Launch → window mapped (first frame) | 2.938 (2.910–2.958) | 3.685 (3.585–3.770) | 3.654 (3.653–3.696) | 3.689 (3.585–3.733) |
| Launch → MCP answers | 2.171 (2.171–2.187) | 2.192 (2.129–2.208) | 2.160 (2.142–2.161) | 2.161 (2.121–2.166) |
| Launch → Home button on AT-SPI | 2.993 (2.971–3.027) | 3.711 (3.670–3.859) | 3.709 (3.679–3.746) | 3.731 (3.644–3.838) |
| Launch → Home content on AT-SPI | 3.097 (3.072–3.130) | 3.753 (3.710–3.898) | 3.788 (3.751–3.791) | 3.790 (3.697–3.892) |
| Close request → process exited | 0.037 (0.037–0.042) | 0.037 (0.037–0.037) | 0.022 (0.022–0.022) | 0.022 (0.022–0.022) |
| Big list: press → its heading on AT-SPI | 0.204 (0.201–0.208) | 0.283 (0.279–0.304) | 0.136 (0.130–0.232) | 0.207 (0.203–0.207) |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- base run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- base run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- base run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- bump run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- bump run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- bump run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- perf run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- perf run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- perf run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- final run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- final run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- final run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0

Sessions (machine load, 1-min loadavg at start → end):

- base: 13:53:29 2.8→2.4 (224 s), 14:34:37 5.8→4.3 (224 s), 14:53:21 2.8→5.3 (224 s)
- bump: 13:57:13 2.4→3.4 (224 s), 14:30:53 3.7→5.8 (224 s), 14:57:05 5.3→4.3 (224 s)
- perf: 14:00:57 3.4→3.1 (224 s), 14:27:09 4.9→3.7 (224 s), 15:00:49 4.3→8.2 (224 s)
- final: 14:04:40 3.1→3.8 (224 s), 14:23:25 3.1→4.9 (224 s), 15:04:33 8.2→3.1 (224 s)

## GPU

| State | took | fps base | fps bump | fps perf | fps final | views/s base | views/s bump | views/s perf | views/s final | polls/s base | polls/s bump | polls/s perf | polls/s final | CPU % base | CPU % bump | CPU % perf | CPU % final | PSS MB base | PSS MB bump | PSS MB perf | PSS MB final |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Idle on Home, nothing played | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.2 (0.2–0.3) | 0.2 (0.2–0.3) | 0.2 (0.2–0.3) | 0.2 (0.2–0.2) | 160 (160–160) | 157 (156–157) | 163 (163–166) | 164 (164–166) |
| Player closed, playing | 3/3 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.9 (0.9–0.9) | 0.9 (0.8–0.9) | 0.7 (0.7–0.8) | 0.8 (0.8–0.9) | 190 (190–190) | 183 (182–184) | 188 (187–190) | 189 (189–190) |
| Player open, playing | 3/3 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.9 (0.9–0.9) | 1.0 (0.9–1.0) | 0.8 (0.8–0.8) | 0.8 (0.7–0.8) | 202 (200–204) | 192 (190–195) | 196 (196–197) | 197 (196–198) |
| Player open, paused | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.2 (0.2–0.3) | 0.3 (0.2–0.4) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) | 202 (200–204) | 192 (191–194) | 197 (197–198) | 198 (196–199) |
| Player open, playing, window unfocused (visible) | 3/3 3/3 3/3 3/3 | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.9 (0.8–1.1) | 0.9 (0.8–1.0) | 0.8 (0.7–0.8) | 0.7 (0.7–0.8) | 203 (201–204) | 195 (194–197) | 199 (199–200) | 200 (199–201) |
| Playing, window covered by a fullscreen window | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 100.1 (100.0–100.2) | 0.6 (0.5–0.7) | 0.5 (0.5–0.6) | 0.6 (0.6–0.6) | 204 (201–204) | 198 (194–200) | 202 (202–205) | 203 (201–206) |
| Playing, window on a hidden workspace | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.6 (0.5–0.6) | 0.6 (0.6–0.7) | 0.5 (0.4–0.5) | 0.5 (0.5–0.6) | 207 (206–207) | 198 (195–201) | 202 (202–205) | 203 (202–206) |
| Playing, window in the scratchpad (hidden) | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 100.1 (100.1–100.2) | 0.6 (0.6–0.7) | 0.5 (0.5–0.6) | 0.6 (0.6–0.6) | 207 (206–207) | 198 (195–201) | 202 (202–206) | 203 (202–206) |
| Big list open, playing (row shows playing bars) | 3/3 3/3 3/3 3/3 | 60.5 (60.3–60.8) | 60.8 (60.8–60.9) | 61.9 (61.9–62.0) | 62.0 (62.0–62.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (0.9–1.0) | 60.5 (60.4–60.8) | 60.8 (60.8–60.9) | 61.9 (61.9–62.0) | 62.0 (62.0–62.0) | 13.9 (11.1–16.4) | 11.9 (11.4–13.0) | 7.8 (6.1–8.0) | 6.6 (6.4–7.0) | 369 (368–369) | 356 (354–358) | 214 (214–218) | 215 (214–218) |
| Big list, wheel-scrolling, playing | 3/3 3/3 3/3 3/3 | 25.7 (24.5–26.3) | 36.2 (34.1–37.0) | 61.8 (61.8–62.0) | 61.9 (61.8–61.9) | 20.3 (20.2–20.3) | 20.4 (20.3–20.7) | 1.0 (1.0–1.0) | 1.0 (1.0–1.1) | 25.6 (24.5–26.3) | 36.1 (34.2–36.9) | 61.8 (61.7–62.1) | 61.9 (61.9–62.0) | 94.5 (93.3–95.6) | 82.5 (81.7–85.1) | 12.4 (10.2–14.2) | 12.3 (10.9–13.3) | 369 (368–369) | 356 (354–359) | 216 (216–220) | 217 (216–220) |
| Big list open, paused | 3/3 3/3 3/3 3/3 | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 0.2 (0.1–0.3) | 0.3 (0.2–0.3) | 0.2 (0.2–0.3) | 0.2 (0.2–0.2) | 369 (368–369) | 356 (354–359) | 218 (218–222) | 219 (218–222) |

<details><summary>All metrics</summary>

**Frames/s (drawn)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 60.5 (60.3–60.8) | 60.8 (60.8–60.9) | 61.9 (61.9–62.0) | 62.0 (62.0–62.0) |
| Big list, wheel-scrolling, playing | 25.7 (24.5–26.3) | 36.2 (34.1–37.0) | 61.8 (61.8–62.0) | 61.9 (61.8–61.9) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Unchanged frames/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Skipped frames/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list, wheel-scrolling, playing | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**View runs/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) | 1.0 (0.9–1.0) |
| Big list, wheel-scrolling, playing | 20.3 (20.2–20.3) | 20.4 (20.3–20.7) | 1.0 (1.0–1.0) | 1.0 (1.0–1.1) |
| Big list open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**Polls/s**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player closed, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Player open, playing, window unfocused (visible) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |
| Playing, window covered by a fullscreen window | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Playing, window on a hidden workspace | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | 0.0 (0.0–0.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) | 4.0 (4.0–4.0) |
| Big list open, playing (row shows playing bars) | 60.5 (60.4–60.8) | 60.8 (60.8–60.9) | 61.9 (61.9–62.0) | 62.0 (62.0–62.0) |
| Big list, wheel-scrolling, playing | 25.6 (24.5–26.3) | 36.1 (34.2–36.9) | 61.8 (61.7–62.1) | 61.9 (61.9–62.0) |
| Big list open, paused | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) | 5.0 (5.0–5.0) |

**Regions/s (perf line, newer builds only)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player closed, playing | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, playing | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Player open, paused | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Player open, playing, window unfocused (visible) | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.0) |
| Playing, window covered by a fullscreen window | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window on a hidden workspace | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Playing, window in the scratchpad (hidden) | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |
| Big list open, playing (row shows playing bars) | – | – | 1.0 (1.0–1.0) | 1.0 (0.9–1.0) |
| Big list, wheel-scrolling, playing | – | – | 1.0 (1.0–1.0) | 1.0 (1.0–1.1) |
| Big list open, paused | – | – | 0.0 (0.0–0.0) | 0.0 (0.0–0.0) |

**CPU, % of a core (perf line)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.2 (0.2–0.3) | 0.2 (0.2–0.3) | 0.2 (0.2–0.3) | 0.2 (0.2–0.2) |
| Player closed, playing | 0.9 (0.9–0.9) | 0.9 (0.8–0.9) | 0.7 (0.7–0.8) | 0.8 (0.8–0.9) |
| Player open, playing | 0.9 (0.9–0.9) | 1.0 (0.9–1.0) | 0.8 (0.8–0.8) | 0.8 (0.7–0.8) |
| Player open, paused | 0.2 (0.2–0.3) | 0.3 (0.2–0.4) | 0.2 (0.2–0.2) | 0.2 (0.2–0.2) |
| Player open, playing, window unfocused (visible) | 0.9 (0.8–1.1) | 0.9 (0.8–1.0) | 0.8 (0.7–0.8) | 0.7 (0.7–0.8) |
| Playing, window covered by a fullscreen window | 100.1 (100.0–100.2) | 0.6 (0.5–0.7) | 0.5 (0.5–0.6) | 0.6 (0.6–0.6) |
| Playing, window on a hidden workspace | 0.6 (0.5–0.6) | 0.6 (0.6–0.7) | 0.5 (0.4–0.5) | 0.5 (0.5–0.6) |
| Playing, window in the scratchpad (hidden) | 100.1 (100.1–100.2) | 0.6 (0.6–0.7) | 0.5 (0.5–0.6) | 0.6 (0.6–0.6) |
| Big list open, playing (row shows playing bars) | 13.9 (11.1–16.4) | 11.9 (11.4–13.0) | 7.8 (6.1–8.0) | 6.6 (6.4–7.0) |
| Big list, wheel-scrolling, playing | 94.5 (93.3–95.6) | 82.5 (81.7–85.1) | 12.4 (10.2–14.2) | 12.3 (10.9–13.3) |
| Big list open, paused | 0.2 (0.1–0.3) | 0.3 (0.2–0.3) | 0.2 (0.2–0.3) | 0.2 (0.2–0.2) |

**Compositor CPU, % of a core**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) | 0.1 (0.0–0.2) | 0.1 (0.1–0.2) |
| Player closed, playing | 0.1 (0.0–0.1) | 0.1 (0.1–0.2) | 0.2 (0.1–0.2) | 0.1 (0.0–0.2) |
| Player open, playing | 0.2 (0.2–0.2) | 0.1 (0.1–0.3) | 0.1 (0.1–0.2) | 0.1 (0.0–0.1) |
| Player open, paused | 0.0 (0.0–0.0) | 0.0 (0.0–0.1) | 0.2 (0.1–0.2) | 0.1 (0.1–0.1) |
| Player open, playing, window unfocused (visible) | 0.1 (0.1–0.2) | 0.1 (0.1–0.2) | 0.2 (0.1–0.2) | 0.1 (0.1–0.3) |
| Playing, window covered by a fullscreen window | 0.2 (0.1–0.2) | 0.1 (0.0–0.2) | 0.1 (0.0–0.1) | 0.1 (0.1–0.1) |
| Playing, window on a hidden workspace | 0.1 (0.1–0.1) | 0.1 (0.1–0.1) | 0.0 (0.0–0.1) | 0.1 (0.0–0.1) |
| Playing, window in the scratchpad (hidden) | 0.2 (0.1–0.2) | 0.1 (0.1–0.1) | 0.2 (0.1–0.2) | 0.1 (0.1–0.1) |
| Big list open, playing (row shows playing bars) | 1.4 (1.2–1.6) | 1.2 (1.2–1.4) | 1.5 (1.2–1.6) | 1.2 (1.2–1.4) |
| Big list, wheel-scrolling, playing | 1.0 (0.9–1.0) | 1.1 (1.0–1.2) | 1.7 (1.4–1.9) | 1.6 (1.3–1.7) |
| Big list open, paused | 0.1 (0.0–0.2) | 0.1 (0.0–0.1) | 0.1 (0.1–0.1) | 0.1 (0.1–0.1) |

**RSS MB (perf line)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 263 (263–264) | 263 (262–263) | 269 (269–272) | 269 (269–272) |
| Player closed, playing | 300 (299–300) | 291 (290–292) | 297 (295–299) | 297 (297–298) |
| Player open, playing | 315 (314–317) | 301 (299–303) | 305 (304–306) | 306 (304–306) |
| Player open, paused | 315 (314–317) | 301 (299–303) | 306 (305–307) | 307 (305–307) |
| Player open, playing, window unfocused (visible) | 317 (315–318) | 306 (304–307) | 310 (310–311) | 311 (309–311) |
| Playing, window covered by a fullscreen window | 317 (315–318) | 308 (305–311) | 312 (312–316) | 313 (312–316) |
| Playing, window on a hidden workspace | 320 (320–321) | 309 (305–311) | 312 (312–316) | 313 (312–317) |
| Playing, window in the scratchpad (hidden) | 321 (320–321) | 309 (305–311) | 312 (312–316) | 314 (312–317) |
| Big list open, playing (row shows playing bars) | 482 (482–483) | 466 (464–468) | 324 (324–328) | 325 (324–328) |
| Big list, wheel-scrolling, playing | 482 (482–483) | 467 (464–469) | 326 (326–330) | 327 (326–330) |
| Big list open, paused | 483 (482–483) | 467 (465–469) | 328 (328–332) | 329 (328–332) |

**PSS MB (smaps_rollup)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 160 (160–160) | 157 (156–157) | 163 (163–166) | 164 (164–166) |
| Player closed, playing | 190 (190–190) | 183 (182–184) | 188 (187–190) | 189 (189–190) |
| Player open, playing | 202 (200–204) | 192 (190–195) | 196 (196–197) | 197 (196–198) |
| Player open, paused | 202 (200–204) | 192 (191–194) | 197 (197–198) | 198 (196–199) |
| Player open, playing, window unfocused (visible) | 203 (201–204) | 195 (194–197) | 199 (199–200) | 200 (199–201) |
| Playing, window covered by a fullscreen window | 204 (201–204) | 198 (194–200) | 202 (202–205) | 203 (201–206) |
| Playing, window on a hidden workspace | 207 (206–207) | 198 (195–201) | 202 (202–205) | 203 (202–206) |
| Playing, window in the scratchpad (hidden) | 207 (206–207) | 198 (195–201) | 202 (202–206) | 203 (202–206) |
| Big list open, playing (row shows playing bars) | 369 (368–369) | 356 (354–358) | 214 (214–218) | 215 (214–218) |
| Big list, wheel-scrolling, playing | 369 (368–369) | 356 (354–359) | 216 (216–220) | 217 (216–220) |
| Big list open, paused | 369 (368–369) | 356 (354–359) | 218 (218–222) | 219 (218–222) |

**GPU memory MiB (nvidia-smi, app)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 96 (96–100) | 68 (68–68) | 68 (68–68) | 68 (64–68) |
| Player closed, playing | 96 (96–100) | 68 (68–68) | 68 (68–68) | 68 (64–68) |
| Player open, playing | 160 (160–164) | 100 (100–100) | 100 (100–100) | 100 (96–100) |
| Player open, paused | 160 (160–164) | 100 (100–100) | 100 (100–100) | 100 (96–100) |
| Player open, playing, window unfocused (visible) | 160 (160–164) | 100 (100–100) | 100 (100–100) | 100 (96–100) |
| Playing, window covered by a fullscreen window | 90 (90–94) | 94 (94–94) | 94 (94–94) | 94 (90–94) |
| Playing, window on a hidden workspace | 96 (96–100) | 100 (100–100) | 100 (100–100) | 100 (96–100) |
| Playing, window in the scratchpad (hidden) | 90 (90–94) | 94 (94–94) | 94 (94–94) | 94 (90–94) |
| Big list open, playing (row shows playing bars) | 160 (160–164) | 100 (100–100) | 100 (100–100) | 100 (96–100) |
| Big list, wheel-scrolling, playing | 160 (160–164) | 100 (100–100) | 100 (100–100) | 100 (96–100) |
| Big list open, paused | 160 (160–164) | 100 (100–100) | 100 (100–100) | 100 (96–100) |

**Machine busy % during window (all 24 cores)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 11.5 (11.4–12.5) | 12.0 (11.4–12.5) | 11.2 (11.1–13.2) | 11.5 (11.0–13.0) |
| Player closed, playing | 11.7 (11.3–12.5) | 11.2 (11.1–12.3) | 11.2 (11.1–13.2) | 11.4 (11.4–13.1) |
| Player open, playing | 11.2 (11.2–12.5) | 11.2 (11.1–12.4) | 11.3 (11.0–13.0) | 11.5 (11.3–13.0) |
| Player open, paused | 11.6 (11.0–12.3) | 11.4 (11.2–12.4) | 11.2 (11.1–13.1) | 12.0 (11.1–12.2) |
| Player open, playing, window unfocused (visible) | 11.6 (11.3–12.4) | 11.3 (11.1–12.3) | 11.2 (11.1–13.3) | 11.8 (11.3–12.8) |
| Playing, window covered by a fullscreen window | 15.9 (15.8–16.7) | 11.2 (11.2–12.7) | 11.2 (11.0–11.4) | 11.5 (11.4–12.7) |
| Playing, window on a hidden workspace | 11.5 (11.3–12.4) | 11.9 (11.5–12.5) | 11.3 (11.3–13.1) | 12.0 (11.2–12.9) |
| Playing, window in the scratchpad (hidden) | 15.6 (15.6–16.3) | 11.2 (11.0–12.9) | 11.5 (11.1–12.8) | 11.3 (11.2–12.6) |
| Big list open, playing (row shows playing bars) | 12.1 (12.1–13.4) | 11.7 (11.6–13.3) | 11.5 (11.4–13.3) | 11.5 (11.4–13.3) |
| Big list, wheel-scrolling, playing | 16.3 (15.7–16.3) | 15.5 (15.3–17.2) | 12.0 (11.6–13.8) | 11.9 (11.8–13.7) |
| Big list open, paused | 11.8 (11.2–12.6) | 11.8 (11.2–12.7) | 11.2 (10.9–12.9) | 11.5 (11.3–12.7) |

**perf.drawn** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, playing | 5 (5–5) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 5 (5–5) | 5 (5–5) | 5 (5–5) | 5 (5–5) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 302.5 (301.5–304) | 304 (304–304.5) | 309.5 (309.5–310) | 310 (310–310) |
| Big list, wheel-scrolling, playing | 128.5 (122.5–131.5) | 181 (170.5–185) | 309 (309–310) | 309.5 (309–309.5) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.drawn_per_s** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, playing | 1 (1–1) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 1 (1–1) | 1 (1–1) | 1 (1–1) | 1 (1–1) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 60.5 (60.3–60.8) | 60.8 (60.8–60.9) | 61.9 (61.9–62) | 62 (62–62) |
| Big list, wheel-scrolling, playing | 25.7 (24.5–26.3) | 36.2 (34.1–37) | 61.8 (61.8–62) | 61.9 (61.8–61.9) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.skipped** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**perf.unchanged** (perf line field)

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player closed, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Player open, playing, window unfocused (visible) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window covered by a fullscreen window | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window on a hidden workspace | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Playing, window in the scratchpad (hidden) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list open, playing (row shows playing bars) | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |
| Big list, wheel-scrolling, playing | 0 (0–0) | 0 (0–0) | 0 (0–0.5) | 0.5 (0–0.5) |
| Big list open, paused | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

**Busiest threads (median % of a core)**

| State | base | bump | perf | final |
|---|---|---|---|---|
| Idle on Home, nothing played | – | – | – | – |
| Player closed, playing | sone-rune 0.55 | – | – | – |
| Player open, playing | sone-rune 0.55 | sone-rune 0.5 | – | – |
| Player open, paused | – | – | – | – |
| Player open, playing, window unfocused (visible) | sone-rune 0.5 | – | – | – |
| Playing, window covered by a fullscreen window | sone-rune 99.7 | – | – | – |
| Playing, window on a hidden workspace | – | – | – | – |
| Playing, window in the scratchpad (hidden) | sone-rune 99.5 | – | – | – |
| Big list open, playing (row shows playing bars) | sone-rune 13.5 | sone-rune 11.5 | sone-rune 7.3 | sone-rune 6.2 |
| Big list, wheel-scrolling, playing | sone-rune 93.7 | sone-rune 82.1 | sone-rune 11.8 | sone-rune 11.8 |
| Big list open, paused | – | – | – | – |

</details>

| Timing (s) | base | bump | perf | final |
|---|---|---|---|---|
| Launch → window mapped (first frame) | 2.679 (2.677–2.723) | 3.068 (3.049–3.077) | 3.079 (3.024–3.092) | 3.095 (3.084–3.154) |
| Launch → MCP answers | 2.144 (2.139–2.165) | 2.144 (2.143–2.176) | 2.167 (2.151–2.169) | 2.170 (2.156–2.190) |
| Launch → Home button on AT-SPI | 2.750 (2.748–2.781) | 3.120 (3.112–3.152) | 3.138 (3.062–3.140) | 3.160 (3.149–3.227) |
| Launch → Home content on AT-SPI | 2.784 (2.783–2.819) | 3.159 (3.150–3.190) | 3.174 (3.097–3.180) | 3.214 (3.202–3.282) |
| Close request → process exited | 0.104 (0.099–0.104) | 0.099 (0.093–0.099) | 0.083 (0.083–0.084) | 0.083 (0.083–0.084) |
| Big list: press → its heading on AT-SPI | 0.195 (0.195–0.203) | 0.323 (0.200–0.329) | 0.114 (0.110–0.115) | 0.145 (0.143–0.274) |

Per session: Loved Tracks requests (offset+limit → rows served), visibility log lines, RSS guard:

- base run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- base run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- base run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- bump run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- bump run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- bump run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- perf run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- perf run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- perf run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- final run0: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- final run1: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0
- final run2: requests 0+50→50, 0+100→100; rows served 150; on/off-screen log lines 0

Sessions (machine load, 1-min loadavg at start → end):

- base: 14:08:24 3.8→3.5 (225 s), 14:49:36 3.6→2.8 (225 s), 15:08:17 3.1→3.1 (225 s)
- bump: 14:12:09 3.5→3.0 (225 s), 14:45:51 3.2→3.6 (225 s), 15:12:02 3.1→2.7 (225 s)
- perf: 14:15:55 3.0→3.3 (225 s), 14:42:06 2.7→3.2 (225 s), 15:15:47 2.7→2.5 (225 s)
- final: 14:19:40 3.3→3.1 (225 s), 14:38:20 4.3→2.7 (225 s), 15:19:32 2.5→2.8 (225 s)

