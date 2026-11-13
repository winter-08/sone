# Full-screen player and miniplayer (decided 2026-11-10 from an HTML prototype, since deleted)

**Direction: Sleeve.** Art-led: the record sleeve is the screen. A faithful evolution of React's
`MaximizedPlayer` / `MiniPlayer`, redrawn in the port's own tokens.

## Full screen

- Ground: black, the cover blurred over it (sigma 48 at the 1400x900 design size, drawn 1.18x
  the window), then a black scrim at 0.6. The blur is done once per track on the CPU
  (`art::ground`, 128 px; see "Motion spike"). A bright cover gets up to +0.22 more scrim, by
  its mean lightness (`art::cover_color`), so text keeps its contrast on a white sleeve. No
  cover: the Afterglow disc, blurred live.
- Opened by pressing the album art in the player bar (not a button on the bar's right).
- A centred square cover, 62% of the window's shorter side (smaller with lyrics open), radius
  10, standing alone: nothing under it. (The prototype had title, artist and heart under the
  cover; decided against.)
- Open: the cover flies from the player bar's 48 pt cover to its place (a shared-element
  flight), 440 ms on `cubic-bezier(0.32, 0.72, 0, 1)`; ground and text fade in with it. Close
  is the same flight back, about a quarter faster.
- Lyrics (the mic button): cover left, synced lyrics right; the active line white, past 0.3,
  coming 0.45, kept centred; a press on a line seeks.
- Bottom bar over a black gradient, laid out as the player bar is: title, artist, heart and
  More on the left | transport and seek | quality, lyrics, volume, exit. The centre and right
  fade (240 ms) after 3 s without the pointer moving, and the cursor with them; never while a
  slider is dragged or focus is inside. The cover and the track info on the left stay.
- Escape closes: lyrics first is *not* a layer; Escape leaves the player.

## Miniplayer

One borderless, always-on-top window whose layout follows its height:

- under 100 pt: one row (cover, title and artist, heart, play; next, previous and volume as
  the width allows);
- under 220 pt: cover, title, artist, "Playing from", heart; then seek and transport;
- 220 pt and up: the cover fills the window, with the transport over it on hover and the
  title, artist and heart beneath.

Default 372 x 196, resizable from 244 x 88 to 672 x 776. Its ground is near-black tinted with
the cover's colour. A drag grip and a close button appear on hover.

## Rejected

- **Libretto** (lyrics as a page of display type): only pays off for tracks with synced
  lyrics; everything else falls back to a title poster.
- **Afterglow** (ambient colour field, gestures instead of buttons): nothing tells you the
  gestures exist, the drifting field redraws for as long as music plays, and text contrast
  over the brightest ring isn't guaranteed.
- **Console** (cover + signal path, lyrics and queue at once): the densest and the least
  lean-back; the drawer already does queue and lyrics beside the page.

Not carried over from React: the 3D cover tilt (Rune has no perspective transform). True OS
fullscreen, first left out because Rune had none, came with the pinned Rune's
`Tx::toggle_full_screen`: the window goes full screen while the player (or a video's Fullscreen)
is up and returns as it was, maximised or not, when it closes (2026-11-13).

## Motion spike

One-shot motion inside the full-screen player only (`src/fx.rs`; every tunable number is a
const there). Nothing in the normal view changed. Each effect plays for a few hundred ms after
an event and draws nothing at rest.

- **Cover flight:** the open spring overshoots a little (bounce 0.12); the close stays critically
  damped and a quarter faster.
- **Pause:** the cover eases to 0.94 with a smaller, softer shadow (320 ms); play springs it back
  (420 ms, bounce 0.15). Interruptible (it is state). A space press lands it without motion.
- **Track change:** after a Next/Previous press (any button that goes through `controller::send`,
  within 2 s) the whole sleeve is pushed out one side by 32 pt and the new one comes in from the
  other (300 ms in, 220 ms out); title and artist come in with an 8 pt nudge the same way. A change
  with no known direction (auto-advance, MPRIS, the queue) cross-fades. The blurred ground keeps
  its 0.7 s cross-fade.
- **Heart:** a like pops it (1 -> 1.25 -> 1, 300 ms `Sequence`), keyed to the like itself so a
  track that was already loved never pops when it shows; an unlike just swaps the glyph.
- **Bar reveal:** the auto-hiding centre and right rise 7 pt as they fade in; the exit is opacity
  only (the offset drops back after the fade).
- **Reduced motion:** Rune snaps the flight, the pause scale and the pushes, and lands the heart's
  final pose.

**Tried and removed: the audio-reactive cover and scrim.** While playing, the cover scaled
1.000-1.020 with loudness and the scrim opened up to 12%, driven by a `sone-fx` thread stepping
an envelope at 30 Hz over an RMS level tap in the backend (`audio_level.rs`, on both output
paths). It took the open, playing player from ~4 to ~27 frames/s (192% -> 1257% CPU under
lavapipe), focused or not: Rune has no partial redraw, so each step redrew the whole window,
the 48 pt ground blur included. The loop, the level tap, and the focus/occlusion/reduced-motion
tracking that only it needed are gone; `src-tauri` is back to its pre-spike state.

**The ground is blurred once per track.** The live 48 pt blur re-ran over the whole window on
every frame the player drew: ~40% of an open frame in software (below). `art::ground` now cuts
the cover to 128 px, blurs it there (sigma 3.7, the same share of the 1.18x-window ground as
48 pt at 1400x900) off the UI thread like the other covers, and the renderer stretches it with
its bilinear filter. Linear interpolation of a gaussian of sigma texels is off by at most
0.24 A / (8 sigma^2) for a step of contrast A: 0.2% at 128 px (64 px would be 0.8%, with slope
kinks every 26 pt). Side by side with the live blur in the sandbox the ground differs by 0.16
of 255 on average, 3 at most (a linear-light blur did not match: the renderer blurs the stored
sRGB values). The blur now scales with the window, as the picture does, and HiDPI gets all of
it (the renderer caps a live blur at 64 device pixels). A track with no cover still blurs the
Afterglow disc live.

### Switches (read once at startup)

| Env | Effect |
| --- | --- |
| `SONE_FX=0` | The one-shot motion off; the player draws as before the spike. |
| `SONE_PERF=1` | Every 5 s, one `[perf]` line on the log and stderr: open/closed, playing/paused, focused/unfocused, `fx=on/off`, frames/s (Rune's `on_frame_encoded` `Drawn` reports), view runs/s, polls/s, process CPU (utime+stime, all threads) as % of one core, RSS. Unset: no thread, no frame hook, no focus handler; one relaxed load per view and poll run. |

### Measured (2026-11-11)

Verifier sandbox (`verifier/harness/sandbox.sh`): headless sway at 1400x900@60, fake TIDAL,
PipeWire null sink; fixture 1002 (10 s of 96 kHz noise) on repeat-one. A driver plays, opens the
player through MPRIS `Fullscreen`, pauses and resumes through MCP, and unfocuses by creating a
second headless output and focusing it. Phases of 21 s; the `[perf]` intervals fully inside a
phase are kept; medians over 3 runs per cell. Software: lavapipe, the sandbox default. GPU:
`SPIKE_GPU=1` (sway's Vulkan renderer, Rune on the RTX 3090's NVIDIA driver; sway stayed under
210 MB, well inside its leak watchdog).

| State | lavapipe `SONE_FX=0` | lavapipe default | GPU `SONE_FX=0` | GPU default |
| --- | --- | --- | --- | --- |
| closed, playing | 7.8 f/s, 1.4 v/s, 54% | 7.8, 1.4, 55% | 9.2, 1.4, 2.8% | 8.6, 1.2, 1.8% |
| open, playing, focused | 7.0, 1.4, 189% | 7.0, 1.2, 189% | 10.2, 1.0, 1.8% | 10.3, 1.0, 1.8% |
| open, paused | 0.0, 0.0, 0.4% | 0.0, 0.0, 0.4% | 0.0, 0.0, 0.6% | 0.0, 0.0, 0.6% |
| open, playing, unfocused | 6.4, 1.2, 173% | 4.4, 1.2, 119% | 7.8, 1.0, 1.8% | 7.6, 1.0, 1.8% |

- **The one-shots cost nothing at rest:** every state matches `SONE_FX=0` within the
  interval-to-interval spread (each cell's 5 s intervals range over 4-12 frames/s, see below).
- **What draws ~8 frames/s with music playing is the sandbox, not the app.** The clock ticks
  land with `tx.instant()`, one frame a second, as `ui.rs` says. On top of that, the 10 s fixture
  restarts every 1.4-4.7 s of wall time (each restart re-locks the decoder's caps: a real
  pipeline restart, so the sandbox's sink isn't paced in real time). Each restart is a position
  jump, which the controller signals as a change, and the seek fill glides back on its 0.12 s
  spring: ~17 frames at 60 Hz each time. With that spring temporarily removed: 1.0 frames/s and
  7.4% CPU closed, 1.0 frames/s open. Both the spring and the jump rule predate the player work
  (`03152f3`); on real music they cost one short glide per track or seek.
- **Why an open frame is expensive in software:** 0.47 core-seconds above a closed frame with the
  live blur (47% at 1 frame/s, against 7.4% closed), 0.28 without any blur, and 0.28 with the
  pre-blurred ground: the live blur was 0.19 core-seconds, ~40% of the frame; the rest is
  lavapipe rasterising the full-window player (the cover's 72 pt shadow, scrim, bar gradient).
  Open+playing now draws ~7 frames/s (was 4.2) at the same CPU: cheaper frames let more of each
  seek glide through.
- **On the GPU it is all small:** 1.8% of a core open and playing (about 10 frames/s, glides
  included), 0.6% paused. The lavapipe CPU column measures software rasterisation; frames/s and
  views/s are what carry over.
- `polls/s` sits at 55-60 in every state, frames or not; a poll costs little (7.4% total closed
  in software includes the audio path) and wasn't looked into.

Not measured: real music (a track change every few minutes instead of every few seconds), a
HiDPI display, X11. To measure on a real machine: `SONE_PERF=1`, play something, read the
`[perf]` lines after 10 s in each state; with the seek glide only on track changes and seeks,
expect about 1 frame/s while playing, open or closed, and 0 paused.
