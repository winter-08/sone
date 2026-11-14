# SONE on Rune

SONE with a native [Rune](https://github.com/anomalyco/rune) UI over the `sone-core` backend
(`../core`). This is the app: the Tauri/React build it replaced is on the `main` branch, and its
React frontend was the specification this UI was written and audited against (the audits are
recorded in `../verifier/DESIGN.md`). What is still missing is listed at the end of this file.

```
rune-app/
  src/main.rs            boot the backend, route its events, the view's panes
  src/controller.rs      SONE's React playback/queue logic re-implemented in Rust (one actor)
  src/content.rs         explicit / AI content filter        src/sources.rs   paged play sources
  src/ui.rs, glyph.rs    the Afterglow kit, the player bar   src/icons.rs     lucide icons
  src/views.rs           header, sidebar, search, album, artist, mix, feed, explore, drawer
  src/home.rs            Home tabs and sections              src/viewall.rs   view-all pages
  src/paged.rs, coll.rs  paged collections, sort, shared collection state
  src/playlist.rs, loved.rs, library.rs                      playlist, Loved, library and folders
  src/interact.rs        context menus, favourites, playlist dialogs, toasts
  src/queue_ui.rs, drawer_tabs.rs, signal_path.rs            the now-playing panel
  src/full_ui.rs, mini.rs                                    full-screen player, miniplayer
  src/video.rs, video_ui.rs, hls_relay.rs                    music videos (GStreamer)
  src/cover_video.rs     animated album covers (looping muted mp4s, their own pipelines)
  src/settings.rs, login.rs, profile.rs, theme.rs            settings, sign-in, profile, themes
  src/shell.rs, keys.rs  title bar, window state, tray, deep links; shortcuts and rebinding
  src/obs.rs, mcp_sync.rs, proxy_banner.rs, notices.rs, scheme.rs   integrations and notices
  src/data.rs, art.rs, formats.rs, skeleton.rs               catalogue cache, artwork, formatting
  src/sched.rs           background requests: at most two of the client's six slots
  src/perf.rs            SONE_PERF=1: one line every 5 s on what the window did
  PLAYBACK_SPEC.md       the behavioural contract, written from the React sources
  PLAYER_DESIGN.md       decisions on the full-screen player and miniplayer
  shell.nix, wrap.sh     build shell; wrapper with the runtime environment baked in
../core                the backend library (sone-core)
```

How the app and the backend meet (`core/src/host.rs`): an `AppHandle` carries the backend's
shared state and an in-process event bus. `setup_core()` starts everything that needs no window,
the controller calls the functions in `core/src/commands` directly and hears events through
`listen_any`, and `on_exit()` is the shutdown. `launch::prepare_environment()` runs first in
`main()`: the proxy environment and libsoup's HTTP/1.1 switch must be settled before any thread
exists.

## Build and run

`shell.nix` uses the nixpkgs revision `../flake.lock` pins, links with lld, and shares compiled
dependencies between checkouts and worktrees through sccache (`~/.cache/sccache`):

```bash
nix-shell rune-app/shell.nix --run 'cd rune-app && cargo build && ./wrap.sh'
rune-app/target/debug/sone-rune-wrapped
```

It uses your normal SONE profile (`~/.config/sone`) and keeps its own state beside
`settings.json`: `rune-shell.json` (shortcut bindings, zoom, title bar, window and miniplayer
geometry, under the keys the React app used in localStorage), `rune-ui.json`, `rune-library.json`,
`rune-recent-playlists.json`, and `theme.json`. Rust comes from `rust-toolchain.toml`.

- One instance at a time: a second launch raises the first and hands over a `tidal://` link.
- A release build registers itself as the `tidal://` handler at launch (a `.desktop` file in
  `~/.local/share/applications` and `xdg-mime default`), as the Tauri app did;
  `SONE_NO_SCHEME_REGISTER=1` turns that off. Debug builds never register.
- `SONE_PERF=1` logs frames, view runs, polls, region rebuilds, CPU and memory every 5 s.
  `SONE_FX=0` turns the one-shot motion off. `SONE_GLASS=0` draws menus, toasts and dialogs on an
  opaque surface instead of frosted glass. `SONE_SOFTWARE_VULKAN=1` renders on lavapipe.
- `RUNE_REGION_MISS=1` prints why a pane rebuilt. The view is five Rune regions (header, page,
  drawer, sidebar, nowbar) over tracked state; a playback position tick should rebuild only
  `nowbar`, and a debug build warns (`rune: region …`) when a reused region is stale.

Rune is pinned in `Cargo.toml`. `nix build .#sone` builds the package with crane: the dependencies
are one derivation (`sone-deps`) and the app another, so a source change rebuilds only the app
(about 100 s here, against about 5 minutes from nothing). `package.nix` carries one hash, for the
Rune source, to update when the Rune rev changes; a wrong one fails with the right one in "got:".

## Verify against the original

```bash
nix-shell verifier/harness/shell.nix --run \
  'python3 -I verifier/verify.py check --display 1400x2000 --ignore controls,unnamed_controls --bin rune-app/target/debug/sone-rune-wrapped'
```

Rune exposes only on-screen content to assistive technology, so Rune builds are scored on a tall
display. `--ignore controls,unnamed_controls` judges behaviour (MPRIS, MCP, network, bit-exact
audio) and leaves out the accessibility inventory, whose control names still differ from the
reference's. The sandbox never touches real audio hardware or your profile. The state benchmark
is `verifier/bench/states.sh` (see `verifier/bench/states.py`). Loading and navigation times
against a slow TIDAL (300 ms a request by default, as the real service is) are
`verifier/bench/latency.py`: the command to rerun is at the top of that file.

The backend sends TIDAL requests concurrently (at most six at once, one token refresh for many
expired requests: `TidalClientCell` in `core/src/tidal_api.rs`). Work the user hasn't asked
for yet (other Home tabs, the feed, the profile, favourite ids) holds at most two of those
slots (`src/sched.rs`).

## Not done

- **Needs Rune changes:** top and left resize edges on a frameless window; switching to or from
  the system title bar without a restart; global media keys (MPRIS still carries them on GNOME
  and KDE); the Maximize/Restore label following the compositor on Wayland.
- **Cannot exist without a webview:** the embedded TIDAL login window. "Sign in with TIDAL" and
  "Open in browser instead" both run the browser flow.
- **Deliberately different** from React: the album art in the player bar opens the full-screen
  player (`PLAYER_DESIGN.md`); every press of Next counts; the track menu has "Go to album" and
  "Go to artist"; a video's heart and menu act on the video, not on a track with its id.
  Animated covers play in the drawer's cover column (windows about 1580 pt wide and up), not
  in the narrower drawer's 52 pt thumbnail; they pause while the window is unfocused and stop
  while the album page's cover is scrolled out of view, since each frame redraws the window.
- **A playing animated cover redraws the window** at its frame rate (at most 30 frames/s;
  Rune has no partial redraw, and never skips a frame with a video surface on screen). On a
  GPU that is cheap (album page 5.7% of a core, full player 7.8%, against 0.2% and 1.0% with
  a still cover); under software rendering (lavapipe) it is not (149% and 871%, against 0.5%
  and 26%), so there they are off unless turned on in Settings > Playback ("Animated album
  covers"); software rendering is recognised by a Vulkan driver list naming only lavapipe,
  `LIBGL_ALWAYS_SOFTWARE`, or no GPU render node. Unfocused, a cover costs nothing.
  Numbers: `../verifier/bench/results/states-2026-11-14-114331-vcover.md`.
- **Not verified on real hardware:** `on_visibility` (the headless compositor never reports it;
  animated covers stop on it, unit-tested only),
  touchpads, profile picture upload (needs a file chooser), X11, and real music.
