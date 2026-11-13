# SONE on Rune (no WebKitGTK)

SONE's backend (`../src-tauri`, unchanged behaviour) under a native [Rune](https://github.com/anomalyco/rune)
UI instead of Tauri's WebKitGTK webview. Work in progress: this is the **playback core slice**.

```
rune-app/
  src/main.rs          boot the backend through host-shim, route its events, open the Rune window
  src/controller.rs    SONE's React playback/queue logic re-implemented in Rust (one actor)
  src/ui.rs, glyph.rs  the Afterglow kit from ~/dev/music-player, rebound to the controller
  PLAYBACK_SPEC.md     the behavioural contract, written from the React sources
  shell.nix, wrap.sh   build shell; wrapper with the runtime environment baked in
../src-tauri/host-shim the slice of Tauri's API the backend uses, minus the webview
```

How the backend runs without Tauri: `src-tauri` has two features. `tauri-ui` (default) is the
shipping app, unchanged. `host` makes Tauri/WebKit/GTK optional and aliases `tauri::` to
`host-shim`, an in-process event bus with managed state; `#[tauri::command]`s become plain
functions the controller calls. `setup_core()` (in `src-tauri/src/lib.rs`) is the window-free
part of startup, shared by both.

## Build and run

```bash
nix-shell rune-app/shell.nix --run 'cd rune-app && cargo build && ./wrap.sh'
rune-app/target/debug/sone-rune-wrapped
```

It uses your normal SONE profile (`~/.config/sone`). Rust 1.99.1 comes from `rust-toolchain.toml`.

Menus, the search dropdown, toasts and dialogs are frosted glass (a backdrop blur under a dark
tint in the playing cover's hue, `theme::overlay_surface`); the page, sidebar, drawer and player
bar stay opaque. `SONE_GLASS=0` (read at startup) draws the overlays on the plain opaque surface
instead, as a Reduce Transparency setting would.

## Verify against the original

```bash
nix-shell verifier/harness/shell.nix --run \
  'python3 -I verifier/verify.py check --ignore controls,unnamed_controls --bin rune-app/target/debug/sone-rune-wrapped'
```

`--ignore controls,unnamed_controls` judges behaviour (MPRIS, MCP, network, bit-exact audio) and
leaves out the accessibility inventory, which can't match until SONE's other views are ported.
The sandbox never touches real audio hardware.

## Not in this slice yet

Library/search/home/album/artist/playlist views, sign-in UI (an existing signed-in profile works),
settings UI, miniplayer, lyrics, music videos (blocked on Rune Linux video, anomalyco/rune#30) and
toasts beyond a one-line notice.

The window shell (`src/shell.rs`) keeps what the React app kept in localStorage for it (shortcut
bindings, zoom, the hidden title bar, the miniplayer's place) in `rune-shell.json` beside
`settings.json`, under the same keys, with the main window's geometry. Keyboard shortcuts and
their dialog are `src/keys.rs`.
