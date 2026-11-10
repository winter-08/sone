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

## Later: motion spike

Extra motion (cover spring, pause shrink, directional track change, heart pop, audio-reactive
cover and ground) is a separate spike, in its own commit after this lands, with measured cost:
player open, player closed (must be zero) and window unfocused.
