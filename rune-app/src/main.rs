//! SONE with a native Rune UI.
//!
//! The backend is SONE's own `src-tauri` library, unchanged, built with the `host` feature so
//! `tauri::` resolves to a webview-free stand-in (host-shim). Where the React frontend used to
//! listen to backend events and call `#[tauri::command]`s, the Rust playback controller does
//! (controller.rs), the views fetch catalogue data through the same commands (data.rs), and
//! the Rune window draws both (views.rs, ui.rs).

mod art;
mod bio;
mod controller;
mod data;
mod full_ui;
mod glyph;
mod hls_relay;
mod home;
mod icons;
mod interact;
mod mini;
mod theme;
mod ui;
mod video;
mod video_ui;
mod views;

use rune::{AppSpec, Cx, El, Key, KeyEvent, ScrollSpec, Tx, col, el, row};
use sone_host_shim::AppHandle;
use std::sync::OnceLock;
use std::time::Duration;
use tauri_app_lib as backend;

static HOST: OnceLock<AppHandle> = OnceLock::new();

pub use mini::{mini_open, set_mini};

pub fn host() -> &'static AppHandle {
    HOST.get().expect("backend host not started")
}

pub struct App {
    seen: (u64, u64, u64),
    pub nav: views::Nav,
}

fn poll(s: &mut App, _: f64, tx: &mut Tx) -> bool {
    if s.nav.suggest_due.is_some_and(|t| std::time::Instant::now() >= t) {
        s.nav.suggest_due = None;
        let q = s.nav.search.text.trim().to_string();
        if !q.is_empty() {
            data::want(data::Key::Suggest(q));
        }
    }
    views::flush_settings(s);
    if s.nav.suggest_due.is_some() || s.nav.discord_due.is_some() || s.nav.proxy_due.is_some() {
        tx.wake_after(Duration::from_millis(50));
    }
    // menus and favourites (interact.rs): favourite ids once signed in, async results, toasts
    if controller::signed_in() {
        interact::load_favorites_once();
    }
    interact::apply_pending(s);
    if let Some(next) = interact::tick_hover(s) {
        tx.wake_after(next);
    }
    interact::settle_create(s);
    if let Some(next) = interact::toast_tick() {
        tx.wake_after(next.min(Duration::from_millis(200)));
    }
    // The video player: open it for a new video, fade its controls when the pointer rests.
    let (video_changed, video_next) = video_ui::tick(s);
    if let Some(next) = video_next {
        tx.wake_after(next + Duration::from_millis(5));
    }
    // The full-screen player: closes with nothing playing, follows MPRIS, fades its controls.
    let (full_changed, full_next) = full_ui::tick(s);
    if let Some(next) = full_next {
        tx.wake_after(next + Duration::from_millis(5));
    }
    let (state, ticks) = controller::versions();
    // (Covers don't appear here: art.rs fills their pixels in place, without a rebuild.)
    // (Home's carousel arrows change as a row reaches an end: home::version.)
    let v = (state, ticks, data::version().wrapping_add(interact::version() << 40).wrapping_add(home::version() << 20));
    tx.wake_after(Duration::from_millis(200));
    let mut changed = v != s.seen;
    // Synced lyrics: the active line changes at its timestamp, not on the next clock tick, and
    // (unless the user scrolled away) the drawer glides to keep it centred (LyricsTab).
    let lyric_moved = lyric_tick(s, tx);
    // A clock tick (or a seek bar drag) lands at once; a seek, a new track, new data or the next
    // lyric line animate, as does the full player's opening, closing or controls fading.
    if changed && !lyric_moved && !full_changed && ((state, v.2) == (s.seen.0, s.seen.2) || ui::dragging()) {
        tx.instant();
    }
    changed |= lyric_moved || video_changed || full_changed;
    s.seen = v;
    changed
}

/// The drawer's Lyrics tab and the full player's lyrics column each follow the active line.
fn lyric_tick(s: &mut App, tx: &mut Tx) -> bool {
    let drawer = s.nav.drawer == Some(views::Drawer::Lyrics);
    let full = s.nav.full.open && s.nav.full.lyrics;
    let lines = (drawer || full)
        .then(|| controller::now_playing().0).flatten()
        .and_then(views::synced_lyrics);
    let Some(lines) = lines else {
        let was = s.nav.lyric_line.take().is_some() | s.nav.full.lyric_line.take().is_some();
        return was;
    };
    let (pos, playing) = controller::clock();
    let (line, next) = views::lyric_at(&lines, pos);
    if playing && let Some(at) = next {
        tx.wake_after(Duration::from_secs_f64((at - pos).max(0.0) + 0.005));
    }
    let mut moved = false;
    if !drawer {
        moved |= s.nav.lyric_line.take().is_some();
    } else if line != s.nav.lyric_line {
        s.nav.lyric_line = line;
        if let Some(i) = line
            && !s.nav.lyrics_scrolled
        {
            tx.scroll_center(format!("ly-{i}"));
        }
        moved = true;
    }
    if !full {
        moved |= s.nav.full.lyric_line.take().is_some();
    } else if line != s.nav.full.lyric_line {
        // A column that just opened (or a new track) lands on its line; then it glides.
        let fresh = s.nav.full.lyric_line.is_none();
        s.nav.full.lyric_line = line;
        if let Some(i) = line {
            if fresh { tx.scroll_center_in_instant("full-lyrics", format!("fly-{i}")) } else { tx.scroll_center_in("full-lyrics", format!("fly-{i}")) }
        }
        moved = true;
    }
    moved
}

fn on_key(s: &mut App, e: &KeyEvent, tx: &mut Tx) -> bool {
    // any key brings the full player's controls back
    full_ui::wake(s);
    // a context menu is the top Escape layer (React dismissStack), then the full-screen or
    // the video player (never open together)
    if matches!(e.key, Key::Escape) && (interact::on_escape(s) || full_ui::on_escape(s) || video_ui::on_escape(s, tx)) {
        return true;
    }
    // "/" (outside a text field) and Ctrl+K focus the search bar
    let typing = s.nav.search.focused || s.nav.filter.focused;
    if (!typing && !e.mods.cmd && !e.mods.alt && matches!(&e.key, Key::Text(t) if t == "/")) || e.is_command('k') {
        s.nav.search.focused = true;
        s.nav.search_open = !s.nav.search.text.trim().is_empty();
        return true;
    }
    if matches!(e.key, Key::Escape) && (s.nav.menu_open || s.nav.settings_open) {
        if s.nav.menu_open { s.nav.menu_open = false } else { s.nav.settings_open = false }
        return true;
    }
    if e.mods.cmd || e.mods.ctrl || e.mods.alt || s.nav.search.focused || s.nav.filter.focused {
        return false;
    }
    match &e.key {
        Key::Text(t) if t == " " => controller::send(controller::Cmd::TogglePlay),
        Key::Text(t) if s.nav.full.open && t.eq_ignore_ascii_case("l") => full_ui::toggle_lyrics(s),
        _ => return false,
    }
    true
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    art::set_scale(cx.env().scale);
    let v = controller::view();
    theme::set_ambient(ui::hue(v.track_id));
    // The root fills the window, so its width is the window's.
    let shell = cx.layout_with("root", |r| views::Shell::for_width(r[2])).unwrap_or_else(|| views::Shell::for_width(initial_size().0));
    let (header, page) = (views::header(s), views::page(s, &v));
    // Shell data (Home, folders, ...) is loaded by the controller once signed in.
    if !v.signed_in {
        return views::login(s, &v);
    }
    // The header spans the page and the drawer, so the window controls stay in the top-right
    // corner; the drawer sits beside the page below it, or over the page's right edge.
    let mut content = row().grow(1.0).min_h(0.0).w_full().child(page);
    if let Some(d) = views::drawer(s, &v, shell) {
        content = content.child(d);
    }
    let main = col().grow(1.0).min_w(0.0).h_full().child(header).child(content);
    let body = row().grow(1.0).min_h(0.0).w_full().child(views::sidebar(s, shell.sidebar_w)).child(main);
    // The bar's cover opens the video player again, or the full-screen player.
    let expand = views::bar_cover_action(&v);
    // The player bar centres its transport from the window's width and its extras' width.
    let win_w = cx.layout_with("root", |r| r[2].round()).unwrap_or(initial_size().0);
    let room = (win_w, cx.layout_with("nowbar-extras", |r| r[2].ceil()));
    let mut root = col().key("root").w_full().h_full().fill(theme::ground()).child(body).child(ui::nowbar::<App>(&v, views::bar_track_actions(&v), views::bar_extras(&v, shell), expand, room));
    if let Some(d) = views::dropdown(s) {
        root = root.child(d);
    }
    if let Some(m) = views::settings_modal(s) {
        root = root.child(m);
    }
    if let Some(d) = views::dialog(s) {
        root = root.child(d);
    }
    if let Some(m) = views::account_menu(s, &v) {
        root = root.child(m);
    }
    // (under the context menus: its More button opens the track menu over it)
    let win_h = cx.layout_with("root", |r| r[3].round()).unwrap_or(initial_size().1);
    if let Some(p) = full_ui::overlay(s, &v, (win_w, win_h)) {
        root = root.child(p);
    }
    for o in interact::overlay(s) {
        root = root.child(o);
    }
    if let Some(p) = video_ui::overlay(s, &v) {
        root = root.child(p);
    }
    if let Some(t) = interact::toast_layer() {
        root = root.child(t);
    }
    let _ = row;
    root
}

/// The window's first size.  overrides it (the verifier uses a tall one).
fn initial_size() -> (f32, f32) {
    std::env::var("SONE_WINDOW_SIZE").ok().and_then(|v| {
        let (w, h) = v.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    }).unwrap_or((1280.0, 800.0))
}

const FONTS: &[&[u8]] = &[include_bytes!("../assets/Inter-Regular.ttf"), include_bytes!("../assets/Inter-Medium.ttf"), include_bytes!("../assets/Inter-SemiBold.ttf")];

/// The app's name in the window title and login screen; package.nix builds it as "RONE".
pub const TITLE: &str = match option_env!("SONE_RUNE_TITLE") {
    Some(t) => t,
    None => "SONE",
};

fn main() {
    // The backend's own logger (file + stderr, honouring logging.toggle).
    let sone_dir = backend::config_dir_for_env().unwrap_or_else(|| std::path::PathBuf::from("./.sone"));
    let logging_enabled = backend::logging::read_logging_preference(&sone_dir.join("logging.toggle"));
    let _logger = backend::logging::init_logging(sone_dir.join("logs"), logging_enabled);

    art::prune_disk_cache();
    let handle = AppHandle::new();
    let _ = HOST.set(handle.clone());
    // Route every backend event to the controller before anything can emit.
    handle.listen_any(|ev| controller::on_backend_event(ev.event(), ev.payload()));
    backend::setup_core(&handle);
    controller::start(handle.clone());

    let spec = AppSpec::<App> {
        min_size: Some((900.0, 600.0)),
        fonts: FONTS,
        families: ("Inter", "ui-monospace"),
        poll: Some(poll),
        on_key: Some(on_key),
        // Video frames (video.rs) wake the window as they arrive.
        setup: Some(|_: &mut App, w| video::set_waker(w)),
        ..AppSpec::new(TITLE, initial_size(), || App { seen: (0, 0, 0), nav: views::Nav::default() }, view)
    };
    // Matches the desktop entry package.nix installs, so the desktop shows its name and icon.
    rune::set_app_id(option_env!("SONE_RUNE_APP_ID").unwrap_or("sone-rune"));
    let mut app = rune::App::new(spec);
    mini::attach(&mut app);
    // A queue change still inside its save debounce is written on the way out (§1.5).
    handle.on_exit(|code| {
        controller::flush_snapshot();
        std::process::exit(code)
    });
    app.run();
    controller::flush_snapshot();
}

