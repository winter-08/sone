# Full-screen player and miniplayer (decided 2026-11-10 from an HTML prototype, since deleted)

**Direction: Sleeve.** Art-led: the record sleeve is the screen. A faithful evolution of React's
`MaximizedPlayer` / `MiniPlayer`, redrawn in the port's own tokens.

## Full screen

- Ground: black, the cover blurred over it (sigma 48, drawn ~1.15x so no edge shows), then a
  black scrim at 0.6. A bright cover gets up to +0.22 more scrim, by its mean lightness
  (`art::cover_color`), so text keeps its contrast on a white sleeve. No cover: the Afterglow
  disc, blurred the same way.
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

Not carried over from React: the 3D cover tilt (Rune has no perspective transform) and true OS
fullscreen (Rune has none; the player fills the window, as the video player does).

## Motion spike

Extra motion inside the full-screen player only (`src/fx.rs`; every tunable number is a const
there). Nothing in the normal view changed.

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
- **Audio-reactive:** while the player is open and playing, the cover scales 1.000-1.020 with
  loudness and the scrim opens up to 12% of itself (about -0.07 alpha). Blur is never animated.
  A `sone-fx` thread steps an envelope at 30 Hz (attack 60 ms, release 250 ms) over the backend's
  `audio_level` RMS, mapped through dB against a running reference (jumps up, falls 1.5 dB/s, 18 dB
  range, squared) so quiet masters move and loud ones don't pin. It publishes two `Live` values
  (`live_scale` on the sleeve, `live_opacity` on the scrim): each step is a frame with no view,
  layout or poll. Values are rounded to 1/4096, so silence publishes nothing. The tap leads the
  sound (it measures each decoder buffer as it leaves for the sink, ~10-26 a second), so the loop
  reads it 100 ms late, 350 ms in exclusive/bit-perfect mode; true sync would need the device's
  delay (`snd_pcm_delay`) with each level. `audio_level::set_enabled(true)` exactly while the
  loop should run: player open, playing, `SONE_FX` on, no reduced motion, not occluded, and
  focused unless `SONE_FX_UNFOCUSED` allows otherwise. On any exit (close, pause, track end, focus
  loss with the switch, covered window) it is disabled at once, the envelope falls back to rest
  and the thread parks.
- **Reduced motion:** Rune's `cx.system_reduced_motion()` (GNOME enable-animations, KDE duration
  factor) turns the reactive loop off; Rune itself snaps the flight, pause scale and pushes and
  lands the heart's final pose.
- **Occlusion:** Rune reports a covered or minimised window only as `Skipped` frames
  (`on_frame_encoded`; macOS and X11 report covering, Wayland reports neither), which stop the
  loop. On Wayland a minimised player keeps the loop and the tap running, though Rune draws
  nothing for a hidden window and `Live` doesn't wake one.

### Switches (read once at startup)

| Env | Effect |
| --- | --- |
| `SONE_FX=0` | Everything above off; the player draws as before the spike. |
| `SONE_FX_UNFOCUSED=0` | Pause the reactive loop (and the tap) while the window isn't focused. Default: keep going (a player on a second monitor is usually unfocused). |
| `SONE_PERF=1` | Every 5 s, one `[perf]` line on the log and stderr: state, frames/s (Rune's `on_frame_encoded` `Drawn` reports), view runs/s, polls/s, level updates/s, process CPU (utime+stime, all threads) as % of one core, RSS. Unset: no thread, one relaxed load per view and poll run. |
| `SONE_LEVEL_TAP=1` | (backend) Force the level tap on whatever the UI asks, for measuring the tap alone. |

### Measured (2026-11-11)

Verifier sandbox (`verifier/harness/sandbox.sh`): headless sway at 1400x900@60, lavapipe
(software Vulkan), fake TIDAL, PipeWire null sink; fixture 1002 (96 kHz noise) on repeat-one.
A driver plays, opens the player through MPRIS `Fullscreen`, pauses and resumes through MCP, and
unfocuses by creating a second headless output and focusing it (a virtual keyboard on the seat
gives the window keyboard focus at all; without it Rune never hears a focus change). Phases of
21 s; the `[perf]` intervals fully inside a phase are kept; medians over 3 baseline runs and 4
spike runs (12 intervals each).

| State | Baseline `SONE_FX=0`: frames/s, views/s, CPU | Spike: frames/s, views/s, CPU |
| --- | --- | --- |
| closed, playing | 7.6, 1.4, 53% | 7.5, 1.4, 53% (tap off) |
| open, playing, focused | 4.2, 1.0, 192% | 27.3, 1.2, 1257% |
| open, paused | 0.0, 0.0, 0.2% | 0.0, 0.0, 0.2% (tap off) |
| open, playing, unfocused | 3.2, 1.2, 148% | 27.0, 1.2, 1240% |
| open, playing, unfocused, `SONE_FX_UNFOCUSED=0` | - | 3.0, 1.2, 139% (tap off) |

- **Closed is zero.** Same frames and view runs as the baseline; the level tap stays disabled and
  the `sone-fx` thread is never started until the player first opens. Cross-check against the
  binary built from the commit before the spike (no `[perf]`, CPU from /proc by the driver):
  51.7% closed+playing, against 48.0% baseline and 44-52% spike, inside the run-to-run noise
  (closed+playing itself swings 4-14 frames/s between intervals in every build: the bar's clock and
  the 10 s repeat). The 7 frames/s closed is the app's own, before and after.
- **Paused is zero** once the shrink settles (about 1 s including the envelope falling back).
- **The reactive loop is the whole cost:** ~23 extra frames/s (its 30 Hz target, less what
  lavapipe could draw), each a full-window redraw that includes the 48-sigma ground blur. In
  software that is ~0.45 core-seconds a frame across lavapipe's threads, so the CPU column is
  dominated by software rendering and says little about a GPU; frames/s and views/s are the
  numbers to carry over. View runs don't move (the loop never runs the view).
- One-shot effects (open overshoot, pause, push, heart, bar rise) cost frames only while they
  play (≤ 0.45 s each) and nothing at rest; they don't show at a 5 s resolution.
- In 2 of the first 6 spike runs the first open phase drew like the baseline although `tap=on`:
  the tap delivered no levels then (the `levels/s` field was added after that and shows ~26/s in
  every later run). Worth a look on the backend side if it recurs.

Not measured here: a GPU (the sandbox's `SPIKE_GPU=1` sway leaks; see sandbox.sh), real music
(noise keeps the envelope busy, which is close to the worst case), Wayland occlusion (not
reported) and X11 covering. To measure on a real machine: run with `SONE_PERF=1`, play something,
open the player, and read the `[perf]` lines (log and stderr) after 10 s in each state: focused,
paused, focused on another monitor, minimised (on X11 expect `skipped` frames and `fx=on`, not
`reacting`, with `tap=off`), closed (expect `tap=off` and the same frames/s as with `SONE_FX=0`).

### Recommendation

| Effect | Cost | Verdict |
| --- | --- | --- |
| Open overshoot | none at rest | keep |
| Pause shrink | none at rest | keep |
| Directional track change | none at rest | keep |
| Heart pop | none at rest | keep |
| Bar rise | none at rest | keep |
| Audio-reactive cover + scrim | ~25 full redraws/s for as long as music plays with the player up | tune: measure on a GPU first. If it costs more than a few % of a core there, halve the rate (`REACT_HZ` 15 still reads as motion through the envelope). Either value alone costs the same: Rune has no partial redraw, so any change redraws the window, blur included, and a subtree with image pixels can't be `cache()`d. |
