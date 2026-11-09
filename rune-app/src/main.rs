//! SONE with a native Rune UI.
//!
//! The backend is SONE's own `src-tauri` library, unchanged, built with the `host` feature so
//! `tauri::` resolves to a webview-free stand-in (host-shim). Where the React frontend used to
//! listen to backend events and call `#[tauri::command]`s, the Rust playback controller does
//! (controller.rs), the views fetch catalogue data through the same commands (data.rs), and
//! the Rune window draws both (views.rs, ui.rs).

mod art;
mod controller;
mod data;
mod glyph;
mod icons;
mod interact;
mod ui;
mod views;

use rune::{AppSpec, Cx, El, Key, KeyEvent, ScrollSpec, Tx, col, el, row};
use sone_host_shim::AppHandle;
use std::sync::OnceLock;
use std::time::Duration;
use tauri_app_lib as backend;

static HOST: OnceLock<AppHandle> = OnceLock::new();
static MINI: OnceLock<rune::WindowHandle> = OnceLock::new();
static MINI_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn mini_open() -> bool {
    MINI_OPEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// Open or close the floating miniplayer window (React's second Tauri window).
pub fn set_mini(open: bool) {
    MINI_OPEN.store(open, std::sync::atomic::Ordering::Relaxed);
    if let Some(w) = MINI.get() {
        if open { w.open() } else { w.close() }
    }
    controller::bump();
}

struct Mini {
    seen: (u64, u64),
}

fn mini_poll(s: &mut Mini, _: f64, tx: &mut Tx) -> bool {
    let v = controller::versions();
    tx.wake_after(Duration::from_millis(150));
    let changed = v != s.seen;
    if changed && (v.0 == s.seen.0 || ui::dragging()) {
        tx.instant(); // a clock tick lands at once, as in the main window
    }
    s.seen = v;
    changed
}

fn mini_view(_: &Mini, _: &mut Cx<Mini>) -> El {
    let v = controller::view();
    let h = ui::hue(v.track_id);
    let volume = if v.bit_perfect {
        ui::icon_button::<Mini>("mini-vol", "Volume disabled in bit-perfect mode", glyph::G::Speaker, 16.0, ui::white(0.35))
    } else {
        ui::volume::<Mini>(&v, "mini-volume", 90.0)
    };
    col().key("mini-root").w_full().h_full().padding(14.0).gap(10.0).fill(ui::base(h))
        .child(row().gap(12.0).items_center()
            .child(ui::with_image(ui::tile(h, 56.0, v.track_id.unwrap_or(0), 8.0), art::cover(v.cover.as_deref(), 56.0), 56.0, 8.0))
            .child(col().grow(1.0).min_w(0.0)
                .child(rune::text(if v.title.is_empty() { "Nothing playing".to_string() } else { v.title.clone() }).font(ui::t(14.0, 600.0)).color(ui::white(0.95)).truncate())
                .child(rune::text(v.artist.clone()).font(ui::t(12.0, 400.0)).color(ui::white(0.6)).truncate()))
            .child(ui::icon_button::<Mini>("mini-close", "Close miniplayer", glyph::G::Close, 14.0, ui::white(0.6)).on_press(|_: &mut Mini| set_mini(false))))
        .child(row().items_center().gap(8.0).child(ui::transport::<Mini>(&v, "mini", 34.0)).child(rune::spacer()).child(volume))
}

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
    if controller::view().signed_in {
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
    let (state, ticks) = controller::versions();
    // (Covers don't appear here: art.rs fills their pixels in place, without a rebuild.)
    let v = (state, ticks, data::version().wrapping_add(interact::version() << 40));
    tx.wake_after(Duration::from_millis(200));
    let changed = v != s.seen;
    // A clock tick (or a seek bar drag) lands at once; a seek, a new track or new data animate.
    if changed && ((state, v.2) == (s.seen.0, s.seen.2) || ui::dragging()) {
        tx.instant();
    }
    s.seen = v;
    changed
}

fn on_key(s: &mut App, e: &KeyEvent, _: &mut Tx) -> bool {
    // a context menu is the top Escape layer (React dismissStack)
    if matches!(e.key, Key::Escape) && interact::on_escape(s) {
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
        _ => return false,
    }
    true
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    art::set_scale(cx.env().scale);
    let v = controller::view();
    let h = ui::hue(v.track_id);
    // Shell data (Home, folders, ...) is loaded by the controller once signed in.
    let main = col().grow(1.0).min_w(0.0).h_full().child(views::header(s)).child(views::page(s, &v));
    if !v.signed_in {
        return views::login(s, &v);
    }
    let mut body = row().grow(1.0).min_h(0.0).w_full().child(views::sidebar(s)).child(main);
    if let Some(d) = views::drawer(s, &v) {
        body = body.child(d);
    }
    let mut root = col().key("root").w_full().h_full().fill(ui::base(h)).child(body).child(ui::nowbar::<App>(&v, views::bar_track_actions(&v), views::bar_extras(&v)));
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
    for o in interact::overlay(s) {
        root = root.child(o);
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
        ..AppSpec::new("SONE", initial_size(), || App { seen: (0, 0, 0), nav: views::Nav::default() }, view)
    };
    let mut app = rune::App::new(spec);
    let mini_spec = AppSpec::<Mini> {
        fonts: FONTS,
        families: ("Inter", "ui-monospace"),
        poll: Some(mini_poll),
        min_size: Some((380.0, 150.0)),
        ..AppSpec::new("SONE Miniplayer", (420.0, 160.0), || Mini { seen: (0, 0) }, mini_view)
    };
    let options = rune::WindowOptions { always_on_top: true, resizable: false, visible: false, ..Default::default() };
    let mini_app = rune::App::new(mini_spec).on_close(|_, _| MINI_OPEN.store(false, std::sync::atomic::Ordering::Relaxed));
    let (_, mini_window) = app.window(rune::WindowSpec::from_app("mini", mini_app).configured_options(options.into()));
    let _ = MINI.set(mini_window);
    handle.on_exit(|code| std::process::exit(code));
    app.run();
}

