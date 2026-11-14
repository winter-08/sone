//! The window shell and desktop integration: what React's App/Layout chrome, TitleBar,
//! ResizeEdges, useZoom and AppInitializer's deep-link and tray wiring did, and what Tauri's
//! setup gave the webview window (single instance, saved window state, close to tray, raise).
//!
//! - **Store**: `rune-shell.json` in SONE's config folder holds what the React app kept in
//!   localStorage for the shell, under the same keys and JSON (`sone.shortcuts.v2`,
//!   `sone.zoom.v1`, `sone.hideTitleBar.v1`, `sone.miniplayer.geometry`), plus the main window's
//!   geometry (Tauri's window-state plugin kept that in its own file).
//! - **Title bar**: the backend's `decorations` setting and the store's `hideTitleBar` give the
//!   mode, as in React: System (native frame), Custom (SONE's own 32 pt bar) or Hidden.
//!   Rune decides a window's frame when it creates it and can't change it afterwards, so a
//!   switch to or from System applies at the next start; Custom and Hidden switch at once.
//! - **Window**: a second launch raises this one and hands over its `tidal://` link
//!   (`rune::single_instance`); the size (and, where the platform tells it, the position) is
//!   saved and restored; closing hides to the tray when the backend's `minimize_to_tray` says so;
//!   the tray's Show and MPRIS Raise bring it back.

use crate::glyph::{G, glyph};
use crate::ui::{t, white};
use crate::views::{self, Page};
use crate::{App, controller, interact, theme};
use rune::{Cx, Drag, El, Phase, Tx, col, el, row, text};
use serde_json::{Map, Value, json};
use sone_core::host::Manager;
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};
use sone_core::AppState;

// ------------------------------------------------------------------ the store

fn store_path() -> Option<std::path::PathBuf> {
    sone_core::config_dir_for_env().map(|d| d.join("rune-shell.json"))
}

static STORE: LazyLock<Mutex<Map<String, Value>>> = LazyLock::new(|| {
    let map = store_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    Mutex::new(map)
});
/// Each write is numbered; a writer whose number is no longer the latest leaves the file to the
/// newer one, so writes off the UI thread can't land out of order.
static WRITES: AtomicU64 = AtomicU64::new(0);
static WRITER: Mutex<()> = Mutex::new(());

pub fn store_get(key: &str) -> Option<Value> {
    STORE.lock().unwrap().get(key).cloned()
}

/// Sets `key` and saves the store off the UI thread.
pub fn store_set(key: &str, value: Value) {
    {
        let mut s = STORE.lock().unwrap();
        if s.get(key) == Some(&value) {
            return;
        }
        s.insert(key.to_string(), value);
    }
    let n = WRITES.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || write_store(Some(n)));
}

/// Writes the store now (`only_if`: unless a later write was asked for since).
fn write_store(only_if: Option<u64>) {
    let _w = WRITER.lock().unwrap();
    if only_if.is_some_and(|n| n != WRITES.load(Ordering::SeqCst)) {
        return;
    }
    let Some(path) = store_path() else { return };
    let text = serde_json::to_string_pretty(&Value::Object(STORE.lock().unwrap().clone())).unwrap_or_default();
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
        log::warn!("[shell] couldn't save {}", path.display());
    }
}

// ------------------------------------------------------------------ zoom (React useZoom)

const ZOOM_KEY: &str = "sone.zoom.v1";
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 2.0;

static ZOOM: LazyLock<Mutex<f32>> = LazyLock::new(|| {
    // React stores `String(zoom)`; a number is read too.
    let v = store_get(ZOOM_KEY).and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(1.0) as f32;
    Mutex::new(if (ZOOM_MIN..=ZOOM_MAX).contains(&v) { v } else { 1.0 })
});

/// The UI zoom, 1 unzoomed; the main window's view applies it (`Cx::zoom`).
pub fn zoom() -> f32 {
    *ZOOM.lock().unwrap()
}

/// The next zoom: one step in (1) or out (-1), rounded to the hundredth as React rounds, within
/// 50–200 %; 0 resets to 100 %.
pub fn next_zoom(z: f32, dir: i32) -> f32 {
    let round = |v: f32| (v * 100.0).round() / 100.0;
    match dir.signum() {
        1 => round(z + ZOOM_STEP).min(ZOOM_MAX),
        -1 => round(z - ZOOM_STEP).max(ZOOM_MIN),
        _ => 1.0,
    }
}

pub fn zoom_step(dir: i32) {
    let z = {
        let mut z = ZOOM.lock().unwrap();
        *z = next_zoom(*z, dir);
        *z
    };
    store_set(ZOOM_KEY, Value::String(format!("{z}")));
}

// ------------------------------------------------------------------ title bar mode

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleBarMode {
    /// The native window frame and controls.
    System,
    /// SONE's own title bar, in the app's colours.
    Custom,
    /// Frameless: no title bar at all.
    Hidden,
}

const HIDE_TITLE_BAR_KEY: &str = "sone.hideTitleBar.v1";

fn decorations() -> bool {
    crate::host().state::<AppState>().decorations.load(Ordering::Relaxed)
}

fn hide_title_bar() -> bool {
    store_get(HIDE_TITLE_BAR_KEY).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// The mode as the settings say now (React GeneralTab: hidden wins over decorations).
pub fn title_bar_mode() -> TitleBarMode {
    if hide_title_bar() {
        TitleBarMode::Hidden
    } else if decorations() {
        TitleBarMode::System
    } else {
        TitleBarMode::Custom
    }
}

/// Whether the main window was created with the native frame (the mode at start-up).
static NATIVE_FRAME: AtomicBool = AtomicBool::new(false);

/// For the settings page's Title bar choice: saves the mode as React's GeneralTab does
/// (`set_decorations` for the frame, `hideTitleBar` for Hidden). Custom and Hidden apply at once;
/// a change of frame (to or from System) at the next start, which a toast says.
pub fn set_title_bar_mode(mode: TitleBarMode) {
    let st = crate::host().state::<AppState>();
    let save_decorations = |on: bool| {
        st.decorations.store(on, Ordering::Relaxed);
        let mut settings = st.load_settings().unwrap_or_default();
        settings.decorations = on;
        if st.save_settings(&settings).is_err() {
            interact::toast("Failed to update window decorations".into(), interact::Kind::Error);
        }
    };
    match mode {
        TitleBarMode::System => {
            save_decorations(true);
            store_set(HIDE_TITLE_BAR_KEY, Value::Bool(false));
        }
        TitleBarMode::Custom => {
            save_decorations(false);
            store_set(HIDE_TITLE_BAR_KEY, Value::Bool(false));
        }
        TitleBarMode::Hidden => {
            store_set(HIDE_TITLE_BAR_KEY, Value::Bool(true));
            if decorations() {
                save_decorations(false);
            }
        }
    }
    if (mode == TitleBarMode::System) != NATIVE_FRAME.load(Ordering::Relaxed) {
        interact::toast(format!("Restart {} to change the window frame", crate::TITLE), interact::Kind::Info);
    }
}

/// Close to tray, as the backend keeps it (React GeneralTab reads `get_minimize_to_tray`; the
/// backend turns it off when the tray couldn't be created).
fn minimize_to_tray() -> bool {
    crate::host().state::<AppState>().minimize_to_tray.load(Ordering::Relaxed)
}

// ------------------------------------------------------------------ window geometry

const GEOMETRY_KEY: &str = "sone.window.geometry";
const MINI_GEOMETRY_KEY: &str = "sone.miniplayer.geometry";

/// The main window as last seen: its content size in window points, and where the platform says
/// its frame is (physical pixels; never on Wayland).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Geometry {
    size: Option<(f32, f32)>,
    position: Option<(i32, i32)>,
    maximized: bool,
}

static GEOMETRY: Mutex<Geometry> = Mutex::new(Geometry { size: None, position: None, maximized: false });

fn saved_geometry() -> Geometry {
    let v = store_get(GEOMETRY_KEY).unwrap_or(Value::Null);
    let pair = |k: &str| Some((v[k][0].as_f64()?, v[k][1].as_f64()?));
    Geometry {
        size: pair("size").map(|(w, h)| (w as f32, h as f32)).filter(|(w, h)| *w >= 200.0 && *h >= 200.0),
        position: pair("position").map(|(x, y)| (x as i32, y as i32)),
        maximized: false,
    }
}

/// The main window's first size: `SONE_WINDOW_SIZE` (the verifier's), else the saved size, else
/// 1280×800.
pub fn initial_size() -> (f32, f32) {
    std::env::var("SONE_WINDOW_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .or_else(|| saved_geometry().size)
        .unwrap_or((1280.0, 800.0))
}

/// The main window's options: its frame per the title bar mode, and the saved position (X11;
/// Wayland places windows itself).
pub fn window_options() -> rune::WindowOptions {
    let native = title_bar_mode() == TitleBarMode::System;
    NATIVE_FRAME.store(native, Ordering::Relaxed);
    let position = if std::env::var_os("SONE_WINDOW_SIZE").is_some() { None } else { saved_geometry().position };
    rune::WindowOptions { borderless: !native, position, ..Default::default() }
}

thread_local! {
    /// The main window's native handle, for its placement (UI thread).
    static MAIN: RefCell<Option<rune::WindowHandle>> = const { RefCell::new(None) };
    static PLACED_AT: Cell<Option<Instant>> = const { Cell::new(None) };
    static FOCUSED: Cell<bool> = const { Cell::new(true) };
}

pub fn on_window(w: &rune::WindowHandle) {
    MAIN.with_borrow_mut(|m| *m = Some(w.clone()));
}

/// The view's laid-out size (view points) at zoom `zoom`: the window's content size.
pub fn note_size(view: (f32, f32), zoom: f32) {
    let size = ((view.0 * zoom).round(), (view.1 * zoom).round());
    let mut g = GEOMETRY.lock().unwrap();
    let by_edges = EDGE_AT.get().is_some_and(|t| t.elapsed() < Duration::from_millis(500));
    if g.size != Some(size) && keeps_size(PLACEMENT_KNOWN.load(Ordering::Relaxed), g.maximized, FULL_SCREEN.load(Ordering::Relaxed), g.size.is_some(), by_edges) {
        g.size = Some(size);
    }
}

/// Whether a new content size is the one the window restores to. A full-screen window keeps
/// the one it had. Where the platform says whether the window is maximized (X11 and others),
/// any size of a restored window is. On Wayland the maximized flag is only a guess from the
/// title bar's button (a maximize the compositor does itself isn't seen), so whatever it says,
/// only a size the app gave the window counts: its first, or one its resize edges set.
fn keeps_size(placement_known: bool, maximized: bool, full_screen: bool, have_size: bool, by_edges: bool) -> bool {
    if full_screen {
        return false;
    }
    if placement_known { !maximized } else { !have_size || by_edges }
}

/// Where the main window is, where the platform says (X11 and others; not Wayland). Called from
/// the poll, at most every two seconds, and before saving.
fn note_placement() {
    let Some(p) = MAIN.with_borrow(|m| m.as_ref().and_then(|w| w.placement())) else { return };
    PLACEMENT_KNOWN.store(true, Ordering::Relaxed);
    let mut g = GEOMETRY.lock().unwrap();
    g.maximized = p.maximized || p.full_screen;
    if !g.maximized && !p.minimized {
        g.position = Some(p.position);
    }
}

fn maximized() -> bool {
    GEOMETRY.lock().unwrap().maximized
}

/// The window is OS full screen for the full-screen or the video player (they toggle it with
/// `Tx::toggle_full_screen`): no resize edges then, and its size isn't the one to restore.
static FULL_SCREEN: AtomicBool = AtomicBool::new(false);

/// Called by the main window's view with whether a player holds the window full screen.
pub fn set_full_screen(on: bool) {
    FULL_SCREEN.store(on, Ordering::Relaxed);
}

/// What a player last asked of the window (full screen or not), and when.
static ASKED_FULL: Mutex<(bool, Option<Instant>)> = Mutex::new((false, None));

/// Whether the main window is OS full screen: what the platform says (X11 and others), else
/// what a player last asked (Wayland hides it). Just after an ask, the ask: Rune carries the
/// toggle out after the handler.
pub fn window_full_screen() -> bool {
    let (asked, at) = *ASKED_FULL.lock().unwrap();
    if at.is_some_and(|t| t.elapsed() < Duration::from_millis(750)) {
        return asked;
    }
    MAIN.with_borrow(|m| m.as_ref().and_then(|w| w.placement())).map_or(asked, |p| p.full_screen)
}

/// For the video player's poll, which holds the window full screen: whether the window has left
/// it from outside. Looked at once a second (the platform answers with a round trip).
pub fn full_screen_left() -> bool {
    thread_local! {
        static LOOKED: Cell<Option<Instant>> = const { Cell::new(None) };
    }
    if LOOKED.get().is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
        return false;
    }
    LOOKED.set(Some(Instant::now()));
    !window_full_screen()
}

/// Takes the main window full screen or back (React's `setFullscreen(on)`). Rune only toggles,
/// so this toggles only when the window isn't that way already: a full screen the user left
/// by a window-manager shortcut isn't entered again by the player closing.
pub fn put_full_screen(tx: &mut Tx, on: bool) {
    if window_full_screen() != on {
        tx.toggle_full_screen();
    }
    *ASKED_FULL.lock().unwrap() = (on, Some(Instant::now()));
}

/// The platform tells where the window is and whether it's maximized (Rune's
/// `WindowHandle::placement`; on Wayland it says nothing, since it asks for the position first).
static PLACEMENT_KNOWN: AtomicBool = AtomicBool::new(false);

/// The title bar's Maximize/Restore was pressed. Where the platform reports the state, the
/// next placement read settles the label; on Wayland, where it doesn't, the press is all there
/// is to go by, so it flips what the label shows (a maximize the compositor does by itself,
/// a keyboard shortcut or a double click on the bar, isn't seen there).
fn toggled_maximized() {
    if !PLACEMENT_KNOWN.load(Ordering::Relaxed) {
        let mut g = GEOMETRY.lock().unwrap();
        g.maximized = !g.maximized;
    } else {
        PLACED_AT.set(None);
    }
}

/// Saves the main window's geometry to the store and writes it now (on close and quit).
fn save_geometry(write_now: bool) {
    let g = *GEOMETRY.lock().unwrap();
    if g.size.is_none() {
        return;
    }
    let mut v = json!({"size": g.size.map(|(w, h)| [w, h])});
    if let Some((x, y)) = g.position {
        v["position"] = json!([x, y]);
    }
    STORE.lock().unwrap().insert(GEOMETRY_KEY.into(), v);
    if write_now {
        WRITES.fetch_add(1, Ordering::SeqCst);
        write_store(None);
    }
}

/// The miniplayer's last size (its view writes it) and saved place.
static MINI_SIZE: Mutex<Option<(f32, f32)>> = Mutex::new(None);

pub fn note_mini_size(size: (f32, f32)) {
    *MINI_SIZE.lock().unwrap() = Some(size);
}

/// Before the miniplayer opens: back to its saved size and (where the platform allows) place
/// (React useMiniplayerWindow's `sone.miniplayer.geometry`).
pub fn restore_mini(w: &rune::WindowHandle) {
    let v = store_get(MINI_GEOMETRY_KEY).unwrap_or(Value::Null);
    if let (Some(width), Some(height)) = (v["width"].as_f64(), v["height"].as_f64()) {
        w.resize_instant((width as f32, height as f32));
    }
    if let (Some(x), Some(y)) = (v["x"].as_i64(), v["y"].as_i64()) {
        w.set_position((x as i32, y as i32));
    }
}

/// As the miniplayer closes: its size, and its place where the platform says it (UI thread).
pub fn save_mini(w: &rune::WindowHandle) {
    let Some((width, height)) = *MINI_SIZE.lock().unwrap() else { return };
    let mut v = store_get(MINI_GEOMETRY_KEY).unwrap_or_else(|| json!({}));
    if !v.is_object() {
        v = json!({});
    }
    v["width"] = json!(width);
    v["height"] = json!(height);
    if let Some(p) = w.placement() {
        v["x"] = json!(p.position.0);
        v["y"] = json!(p.position.1);
    }
    store_set(MINI_GEOMETRY_KEY, v);
}

// ------------------------------------------------------------------ app lifetime

static APP: OnceLock<rune::AppHandle<App>> = OnceLock::new();

pub fn set_app_handle(h: rune::AppHandle<App>) {
    let _ = APP.set(h);
}

/// Shows, restores and focuses the main window, recreating it if it was closed to the tray (the
/// tray's Show and activate, MPRIS Raise, a second launch). Wayland focuses only with an
/// activation token; without one the compositor may only mark the window urgent.
pub fn raise() {
    if let Some(a) = APP.get() {
        a.open();
    }
}

/// The main window's close (its button, Ctrl+W, the compositor): to the tray when that's on,
/// else the app quits (Tauri's CloseRequested handler).
pub fn on_close(_: &mut App, tx: &mut Tx) {
    note_placement();
    save_geometry(true);
    if !minimize_to_tray() {
        tx.quit();
    }
}

/// Every quit inside the loop (Quit, the last window): the geometry is written first.
pub fn on_quit(_: &mut App, _: &mut Tx) {
    note_placement();
    save_geometry(true);
}

/// The Quit shortcut.
pub fn quit(tx: &mut Tx) {
    tx.quit();
}

/// On the way out, from any thread (the loop ended, or the backend's `exit`: tray Quit, MPRIS
/// Quit, SIGTERM): the geometry seen last is written, then what Tauri's RunEvent::Exit did
/// (Discord disconnected, the idle inhibitor let go, pending scrobbles and play reports
/// flushed), and the single-instance name freed for a launch waiting on it. Runs once.
pub fn on_exit() {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    save_geometry(true);
    // (On a thread of its own: the backend's exit can come from inside the async runtime,
    // the tray's Quit and MPRIS Quit among them, where blocking on it would panic.)
    join_off_runtime(|| {
        sone_core::host::async_runtime::block_on(async {
            sone_core::on_exit(&crate::host().state::<AppState>()).await;
        })
    });
    rune::release_instance();
}

/// Runs `f` on a thread of its own and waits for it. Called on one of the runtime's workers
/// (the tray's and MPRIS's Quit run there), the wait first hands that worker's tasks and the
/// I/O and timer driving to another thread, so the futures `f` blocks on still make progress
/// however few workers the runtime has.
fn join_off_runtime<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    let run = || std::thread::scope(|s| s.spawn(f).join().expect("exit thread panicked"));
    match tokio::runtime::Handle::try_current() {
        Ok(h) if h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => tokio::task::block_in_place(run),
        _ => run(),
    }
}

/// SIGTERM and SIGINT (a session logout, `kill`, Ctrl+C in a terminal) end the app the way its
/// Quit does, so the queue and pending scrobbles are written (Tauri's GTK loop died on them).
pub fn on_terminate_signals() {
    use tokio::signal::unix::{SignalKind, signal};
    sone_core::host::async_runtime::spawn(async {
        let (Ok(mut term), Ok(mut int)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) else {
            log::warn!("[rune] can't listen for SIGTERM/SIGINT");
            return;
        };
        tokio::select! {
            _ = term.recv() => log::info!("[rune] SIGTERM: quitting"),
            _ = int.recv() => log::info!("[rune] SIGINT: quitting"),
        }
        // (The backend's exit, the tray's Quit path: off the runtime, since it blocks on it.)
        std::thread::spawn(|| crate::host().exit(0));
    });
}

/// A launch of the app while it runs, without a link: bring this window back (rather than
/// whichever window Rune hid last, which might be the miniplayer).
pub fn on_reopen(_: &mut App, _: bool, _: &mut Tx) -> bool {
    raise();
    true
}

pub fn on_focus(_: &mut App, focused: bool, _: &mut Tx) {
    FOCUSED.set(focused);
    // an animated cover holds its frame while the window is unfocused
    crate::cover_video::set_focused(focused);
    // the SONE_PERF line reports window focus (one relaxed store)
    crate::perf::set_focused(focused);
}

// ------------------------------------------------------------------ deep links

/// What a `tidal://` link asks for (React parseTidalUrl).
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    PlayTrack(u64),
    Open(Page),
}

pub fn parse_link(url: &str) -> Option<Link> {
    let path = url.strip_prefix("tidal://")?;
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    let (kind, id) = (parts.next()?.to_ascii_lowercase(), parts.next()?);
    let num = || id.parse::<u64>().ok();
    Some(match kind.as_str() {
        "track" => Link::PlayTrack(num()?),
        "album" => Link::Open(Page::Album(num()?)),
        "artist" => Link::Open(Page::Artist(num()?)),
        "playlist" => Link::Open(Page::Playlist(id.to_string())),
        "mix" => Link::Open(Page::Mix(id.to_string(), "Mix".into(), String::new())),
        _ => return None,
    })
}

/// The track a link plays, if it's a track link.
pub fn track_link(url: &str) -> Option<u64> {
    match parse_link(url)? {
        Link::PlayTrack(id) => Some(id),
        Link::Open(_) => None,
    }
}

/// Links that came before sign-in, opened once signed in (React deepLinkQueueRef).
static WAITING_LINK: Mutex<Option<String>> = Mutex::new(None);

/// Opens a `tidal://` link from anywhere (a second launch, MPRIS OpenUri): a track plays, the
/// rest open their page; before sign-in it waits.
pub fn open_link(url: String) {
    if !controller::signed_in() {
        *WAITING_LINK.lock().unwrap() = Some(url);
        return;
    }
    match parse_link(&url) {
        // The controller plays it as it does MPRIS OpenUri (getTrack, then play it alone).
        Some(Link::PlayTrack(_)) => controller::on_backend_event("mpris:open-uri", &Value::String(url).to_string()),
        Some(Link::Open(page)) => {
            if let Some(a) = APP.get() {
                a.update(move |s, _| views::go(s, page));
            }
        }
        None => log::info!("[shell] not a link SONE opens: {url}"),
    }
}

/// `App::on_open_url`: a link this app was launched with, or a later launch handed over.
pub fn on_open_url(_: &mut App, url: &str, _: &mut Tx) {
    raise();
    open_link(url.to_string());
}

// ------------------------------------------------------------------ the poll's part

/// The full-screen player keeps the screen awake (React MaximizedPlayer's inhibit_idle).
static INHIBITED: AtomicBool = AtomicBool::new(false);

/// Called by the main window's poll.
pub fn tick(s: &mut App) {
    if controller::signed_in() && let Some(url) = WAITING_LINK.lock().unwrap().take() {
        open_link(url);
    }
    let due = PLACED_AT.get().is_none_or(|t| t.elapsed() >= Duration::from_secs(2));
    if due {
        PLACED_AT.set(Some(Instant::now()));
        note_placement();
    }
    let want = s.nav.full.open;
    if want != INHIBITED.load(Ordering::Relaxed) {
        INHIBITED.store(want, Ordering::Relaxed);
        crate::mini::hide_for_full(want);
        sone_core::host::async_runtime::spawn(async move {
            let st = crate::host().state::<AppState>();
            let mut inhibitor = st.idle_inhibitor.lock().await;
            if want { inhibitor.inhibit().await } else { inhibitor.uninhibit().await }
        });
    }
}

/// Refresh app data (the shortcut, and Settings → Utilities → Refresh): the backend's disk cache
/// and every page's data are dropped, so what shows is fetched again (React refreshApp clears
/// both caches and reloads the webview; playback carries on either way).
pub fn refresh_app(_: &mut App) {
    sone_core::host::async_runtime::spawn(async {
        let _ = sone_core::commands::utility::clear_disk_cache(crate::host().state::<AppState>()).await;
        crate::data::clear();
        crate::interact::reload_favorites();
    });
}

// ------------------------------------------------------------------ the title bar

pub const TITLE_BAR_H: f32 = 32.0;

/// The app's icon at 16 pt (the React title bar's /sone-icon.png), decoded once.
fn icon() -> Option<rune::Pixels> {
    static ICON: OnceLock<Option<rune::Pixels>> = OnceLock::new();
    ICON.get_or_init(|| {
        let img = image::load_from_memory(include_bytes!("../../core/icons/32x32.png")).ok()?.to_rgba8();
        let px = rune::Pixels::new();
        px.set(img.width(), img.height(), img.into_raw());
        Some(px)
    })
    .clone()
}

/// A title bar button: 28 pt round, muted until hovered; Close turns red.
fn bar_button(key: &str, label: &str, g: G, size: f32, close: bool) -> El {
    let hover = if close { theme::danger().alpha(0.8) } else { white(0.1) };
    theme::focusable(el().key(key.to_string()).role("button").label(label))
        .size(28.0, 28.0)
        .radius(14.0)
        .items_center()
        .justify_center()
        .hover(hover)
        .press(white(0.16))
        .cursor(rune::Cursor::Pointer)
        .help(label)
        .child(glyph(g, size, white(0.6)))
}

/// SONE's own title bar (React TitleBar): the icon, the name and an accent dot on the left, the
/// rest a drag region (a double press maximizes), the window buttons on the right. Dimmed while
/// the window is in the background. None in System and Hidden modes (and while the window still
/// has the native frame it started with).
pub fn title_bar() -> Option<El> {
    let mode = title_bar_mode();
    let native = NATIVE_FRAME.load(Ordering::Relaxed);
    // A window started frameless keeps its controls until a restart gives it the native frame.
    let show = !native && (mode == TitleBarMode::Custom || mode == TitleBarMode::System);
    if !show {
        return None;
    }
    let dim = if FOCUSED.get() { 1.0 } else { 0.5 };
    let mut brand = row().h_full().pl(12.0).pr(16.0).gap(8.0).items_center().opacity(dim).a11y_hidden();
    if let Some(px) = icon() {
        brand = brand.child(el().size(16.0, 16.0).pixels_fit(&px, rune::SurfaceFit::Fill));
    }
    brand = brand
        .child(text(crate::TITLE).font(t(11.0, 600.0).tracking(1.0)).color(white(0.95)))
        .child(el().size(6.0, 6.0).radius(3.0).fill(theme::accent()));
    let max = maximized();
    let buttons = row()
        .h_full()
        .pr(8.0)
        .gap(4.0)
        .items_center()
        .opacity(if FOCUSED.get() { 1.0 } else { 0.6 })
        .child(bar_button("win-min", "Minimize", G::Minus, 14.0, false).on_press_tx(|_: &mut App, tx: &mut Tx| tx.minimize()))
        .child(bar_button("win-max", if max { "Restore" } else { "Maximize" }, G::Square, 12.0, false).on_press_tx(|_: &mut App, tx: &mut Tx| {
            toggled_maximized();
            tx.toggle_maximized()
        }))
        .child(bar_button("win-close", "Close", G::Close, 14.0, true).on_press_tx(|_: &mut App, tx: &mut Tx| tx.close_window()));
    Some(
        row()
            .key("title-bar")
            .w_full()
            .h(TITLE_BAR_H)
            .items_center()
            .fill(theme::sidebar_bg())
            .border_sides([0.0, 0.0, 1.0, 0.0], white(0.06))
            .window_drag()
            .child(brand)
            .child(el().grow(1.0).h_full())
            .child(buttons),
    )
}

// ------------------------------------------------------------------ resize edges

thread_local! {
    /// The window's size when an edge's drag started.
    static GRAB: Cell<Option<(f32, f32)>> = const { Cell::new(None) };
    /// When an edge's drag last resized the window (the size it lands at is noted after).
    static EDGE_AT: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// A frameless window's edges to resize it by (React ResizeEdges), since there's no native frame
/// to drag: the right and bottom edges and the bottom-right corner. The top and left edges
/// (and their corners) need the compositor to resize from that side (winit's
/// `drag_resize_window`, xdg_toplevel.resize on Wayland), which Rune doesn't expose: an app
/// can only set its size, which grows the window right and down. None with the native frame,
/// and none while the window is maximized, as React drops them. `size` is the window's
/// content size in view points.
pub fn resize_edges(size: (f32, f32)) -> Vec<El> {
    if NATIVE_FRAME.load(Ordering::Relaxed) || maximized() || FULL_SCREEN.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let drag = move |dx: bool, dy: bool| {
        move |_: &mut App, d: &mut Drag, tx: &mut Tx| {
            if matches!(d.phase, Phase::Up) {
                GRAB.set(None);
                return;
            }
            let z = zoom();
            let (w, h) = GRAB.get().unwrap_or((size.0 * z, size.1 * z));
            GRAB.set(Some((w, h)));
            EDGE_AT.set(Some(Instant::now()));
            tx.instant();
            tx.resize_window_instant((if dx { w + d.dx * z } else { w }, if dy { h + d.dy * z } else { h }));
        }
    };
    vec![
        el().key("edge-right").pin(Some(0.0), None, Some(0.0), Some(16.0)).w(2.0).cursor(rune::Cursor::ResizeColumn).a11y_hidden().on_drag(drag(true, false)),
        el().key("edge-bottom").pin(None, Some(0.0), Some(16.0), Some(0.0)).h(4.0).cursor(rune::Cursor::ResizeRow).a11y_hidden().on_drag(drag(false, true)),
        el().key("edge-corner").pin(None, None, Some(0.0), Some(0.0)).size(16.0, 4.0).cursor(diagonal_cursor()).a11y_hidden().on_drag(drag(true, true)),
    ]
}

/// The bottom-right corner's pointer: a diagonal double arrow (CSS `nwse-resize`), which
/// Rune's cursor set lacks, drawn once as a 2x PNG (black with a white outline, as the system
/// arrows are).
fn diagonal_cursor() -> rune::Cursor {
    static PNG: OnceLock<Option<&'static [u8]>> = OnceLock::new();
    match PNG.get_or_init(|| nwse_png().map(|b| &*Box::leak(b.into_boxed_slice()))) {
        Some(bytes) => rune::Cursor::Image(rune::CursorImage::png(bytes, [12.0, 12.0])),
        None => rune::Cursor::Move,
    }
}

/// A 48 px (24 pt at 2x) diagonal double arrow, top-left to bottom-right, as PNG bytes.
pub fn nwse_png() -> Option<Vec<u8>> {
    const N: i32 = 48;
    // inside a triangle (a, b, c), by the signs of the edge functions
    let tri = |p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
        let e = |u: (f32, f32), v: (f32, f32)| (v.0 - u.0) * (p.1 - u.1) - (v.1 - u.1) * (p.0 - u.0);
        let (d1, d2, d3) = (e(a, b), e(b, c), e(c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    let shape = |x: f32, y: f32| {
        // the shaft: within 1.6 px of the diagonal between the heads
        let along = (x + y) / 2.0;
        let off = (x - y).abs() / std::f32::consts::SQRT_2;
        let shaft = (14.0..=34.0).contains(&along) && off <= 1.6;
        shaft || tri((x, y), (8.0, 8.0), (8.0, 21.0), (21.0, 8.0)) || tri((x, y), (40.0, 40.0), (40.0, 27.0), (27.0, 40.0))
    };
    let mut img = image::RgbaImage::new(N as u32, N as u32);
    for y in 0..N {
        for x in 0..N {
            let c = (x as f32 + 0.5, y as f32 + 0.5);
            let px = if shape(c.0, c.1) {
                [0, 0, 0, 255]
            } else if (-2..=2).any(|dy| (-2..=2).any(|dx| dx * dx + dy * dy <= 5 && shape(c.0 + dx as f32, c.1 + dy as f32))) {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 0]
            };
            img.put_pixel(x as u32, y as u32, image::Rgba(px));
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// What the main window's view does for the shell each run: applies the zoom and notes the size.
pub fn view(cx: &mut Cx<App>) -> (f32, f32) {
    let z = zoom();
    cx.zoom(z);
    let size = cx.layout_with(FRAME_KEY, |r| (r[2], r[3])).unwrap_or_else(|| {
        let (w, h) = initial_size();
        (w / z, h / z)
    });
    note_size(size, z);
    size
}

/// The element that fills the window, around everything else.
const FRAME_KEY: &str = "shell-root";

/// The window's content around a screen (the splash, the sign-in screen, the signed-in app): the
/// title bar above it, as React's AppChrome and Layout put it, and a frameless window's resize
/// edges. `size` is what [`view`] returned.
pub fn frame(body: El, size: (f32, f32)) -> El {
    let inner = match title_bar() {
        Some(bar) => col().w_full().grow(1.0).min_h(0.0).child(bar).child(col().w_full().grow(1.0).min_h(0.0).child(body)),
        None => body,
    };
    col().key(FRAME_KEY).w_full().h_full().child(inner).children(resize_edges(size))
}

/// [`frame`] while a player covers the whole window (the full-screen player, the video
/// player), which in React sit over the custom title bar: the bar's place is empty, so the
/// player reaches the top edge (it's OS full screen then too). The tree keeps its shape, so
/// nothing under it is built again.
pub fn frame_covered(body: El, size: (f32, f32)) -> El {
    let inner = match title_bar() {
        Some(_) => col().w_full().grow(1.0).min_h(0.0).child(el().key("title-bar-covered")).child(col().w_full().grow(1.0).min_h(0.0).child(body)),
        None => body,
    };
    col().key(FRAME_KEY).w_full().h_full().child(inner).children(resize_edges(size))
}

#[cfg(test)]
mod exit_tests {
    #[test]
    fn waiting_on_the_exit_from_a_lone_worker_does_not_stall_the_runtime() {
        // One worker, and the wait runs on it, as the tray's Quit does: the exit work needs
        // the runtime's timer, which only a worker drives.
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let h = rt.handle().clone();
        let (tx, rx) = std::sync::mpsc::channel();
        rt.spawn(async move {
            let v = super::join_off_runtime(move || h.block_on(async {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                7
            }));
            let _ = tx.send(v);
        });
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(7));
    }
}

#[cfg(test)]
mod size_tests {
    #[test]
    fn the_restore_size_survives_a_wrong_maximized_flag() {
        use super::keeps_size;
        // X11: the platform's flag decides
        assert!(keeps_size(true, false, false, true, false));
        assert!(!keeps_size(true, true, false, true, false));
        // full screen never
        assert!(!keeps_size(true, false, true, true, true));
        assert!(!keeps_size(false, false, true, false, true));
        // Wayland: a compositor maximize with the flag still "restored" isn't kept
        assert!(!keeps_size(false, false, false, true, false));
        // the edges' sizes are, even with the flag wrongly "maximized"
        assert!(keeps_size(false, true, false, true, true));
        // and the first one
        assert!(keeps_size(false, false, false, false, false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corner_cursor_is_a_diagonal_arrow() {
        let png = nwse_png().unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (48, 48));
        // dark on the diagonal, clear off it
        assert_eq!(img.get_pixel(24, 24).0, [0, 0, 0, 255]);
        assert_eq!(img.get_pixel(40, 8).0[3], 0);
        assert_eq!(img.get_pixel(8, 40).0[3], 0);
    }

    #[test]
    fn zoom_steps_like_use_zoom() {
        assert_eq!(next_zoom(1.0, 1), 1.1);
        assert_eq!(next_zoom(1.1, 1), 1.2);
        assert_eq!(next_zoom(2.0, 1), 2.0);
        assert_eq!(next_zoom(0.5, -1), 0.5);
        assert_eq!(next_zoom(0.6, -1), 0.5);
        assert_eq!(next_zoom(1.7, 0), 1.0);
    }

    #[test]
    fn links_parse_like_parse_tidal_url() {
        assert_eq!(parse_link("tidal://track/123"), Some(Link::PlayTrack(123)));
        assert_eq!(parse_link("tidal://album/77"), Some(Link::Open(Page::Album(77))));
        assert_eq!(parse_link("tidal://Artist/5/"), Some(Link::Open(Page::Artist(5))));
        assert_eq!(parse_link("tidal://playlist/ab-cd"), Some(Link::Open(Page::Playlist("ab-cd".into()))));
        assert_eq!(parse_link("tidal://mix/0123abc"), Some(Link::Open(Page::Mix("0123abc".into(), "Mix".into(), String::new()))));
        assert_eq!(parse_link("tidal://track/x"), None);
        assert_eq!(parse_link("tidal://video/1"), None);
        assert_eq!(parse_link("https://tidal.com/track/1"), None);
        assert_eq!(parse_link("tidal://album"), None);
    }
}
