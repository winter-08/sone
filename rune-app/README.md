# SONE on Rune (no WebKitGTK)

SONE's backend (`../src-tauri`, unchanged behaviour) under a native [Rune](https://github.com/anomalyco/rune)
UI instead of Tauri's WebKitGTK webview. The React frontend in `../src` is the specification: what
a user can do there, they should be able to do here. Two audits against it are recorded in
`../verifier/DESIGN.md`; what is still missing is listed at the end of this file.

```
rune-app/
  src/main.rs            boot the backend through host-shim, route its events, the view's panes
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
  src/perf.rs            SONE_PERF=1: one line every 5 s on what the window did
  PLAYBACK_SPEC.md       the behavioural contract, written from the React sources
  PLAYER_DESIGN.md       decisions on the full-screen player and miniplayer
  shell.nix, wrap.sh     build shell; wrapper with the runtime environment baked in
../src-tauri/host-shim   the slice of Tauri's API the backend uses, minus the webview
```

How the backend runs without Tauri: `src-tauri` has two features. `tauri-ui` (default) is the
shipping app, unchanged. `host` makes Tauri/WebKit/GTK optional and aliases `tauri::` to
`host-shim`, an in-process event bus with managed state; `#[tauri::command]`s become plain
functions the controller calls. `setup_core()` (in `src-tauri/src/lib.rs`) is the window-free
part of startup, shared by both, and `on_exit()` the shared shutdown.

## Build and run

`shell.nix` takes the floating nixpkgs, where the GStreamer patch no longer applies, so pin it:

```bash
nix-shell rune-app/shell.nix --arg pkgs 'import (builtins.getFlake "github:NixOS/nixpkgs/f9a75f41be3a74eab55d3a7679d34ad6cc0da977") {}' --run 'cd rune-app && cargo build && ./wrap.sh'
rune-app/target/debug/sone-rune-wrapped
```

It uses your normal SONE profile (`~/.config/sone`) and keeps its own state beside
`settings.json`: `rune-shell.json` (shortcut bindings, zoom, title bar, window and miniplayer
geometry, under the keys the React app used in localStorage), `rune-ui.json`, `rune-library.json`,
`rune-recent-playlists.json`, and `theme.json`. Rust comes from `rust-toolchain.toml`.

- One instance at a time: a second launch raises the first and hands over a `tidal://` link.
- A release build registers itself as the `tidal://` handler at launch (a `.desktop` file in
  `~/.local/share/applications` and `xdg-mime default`), as the Tauri app does;
  `SONE_NO_SCHEME_REGISTER=1` turns that off. Debug builds never register.
- `SONE_PERF=1` logs frames, view runs, polls, region rebuilds, CPU and memory every 5 s.
  `SONE_FX=0` turns the one-shot motion off. `SONE_GLASS=0` draws menus, toasts and dialogs on an
  opaque surface instead of frosted glass. `SONE_SOFTWARE_VULKAN=1` renders on lavapipe.
- `RUNE_REGION_MISS=1` prints why a pane rebuilt. The view is five Rune regions (header, page,
  drawer, sidebar, nowbar) over tracked state; a playback position tick should rebuild only
  `nowbar`, and a debug build warns (`rune: region …`) when a reused region is stale.

Rune is pinned in `Cargo.toml`; `package.nix` carries a `cargoHash` that must be updated whenever
`Cargo.lock` changes (`nix build .#rone` reports the wanted value).

## Verify against the original

```bash
nix-shell verifier/harness/shell.nix --arg pkgs 'import (builtins.getFlake "github:NixOS/nixpkgs/f9a75f41be3a74eab55d3a7679d34ad6cc0da977") {}' --run \
  'python3 -I verifier/verify.py check --display 1400x2000 --ignore controls,unnamed_controls --bin rune-app/target/debug/sone-rune-wrapped'
```

Rune exposes only on-screen content to assistive technology, so Rune builds are scored on a tall
display. `--ignore controls,unnamed_controls` judges behaviour (MPRIS, MCP, network, bit-exact
audio) and leaves out the accessibility inventory, whose control names still differ from the
reference's. The sandbox never touches real audio hardware or your profile. The state benchmark
is `verifier/bench/states.sh` (see `verifier/bench/states.py`).

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
  and 26%). Unfocused, it costs nothing. Settings > Playback > "Animated album covers" turns
  it off. Numbers: `../verifier/bench/results/states-2026-11-14-114331-vcover.md`.
- **Not verified on real hardware:** `on_visibility` (the headless compositor never reports it;
  animated covers stop on it, unit-tested only),
  touchpads, profile picture upload (needs a file chooser), X11, and real music.
