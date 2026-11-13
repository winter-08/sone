//! SONE with a native Rune UI.
//!
//! The backend is SONE's own `src-tauri` library, unchanged, built with the `host` feature so
//! `tauri::` resolves to a webview-free stand-in (host-shim). Where the React frontend used to
//! listen to backend events and call `#[tauri::command]`s, the Rust playback controller does
//! (controller.rs), the views fetch catalogue data through the same commands (data.rs), and
//! the Rune window draws both (views.rs, ui.rs).

mod art;
mod bio;
mod content;
mod controller;
mod data;
mod drawer_tabs;
mod full_ui;
mod fx;
mod glyph;
mod hls_relay;
mod home;
mod icons;
mod interact;
mod keys;
mod mini;
mod perf;
mod queue_ui;
mod settings;
mod shell;
mod signal_path;
mod sources;
mod theme;
mod ui;
mod video;
mod video_ui;
mod views;

use rune::{AppSpec, Cx, El, Key, KeyEvent, ScrollSpec, Tracked, Tx, col, el, row};
use sone_host_shim::{AppHandle, Listener};
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
    /// When the 200 ms tick's next wake is due (Rune's clock).
    tick_at: f64,
    /// The window is on screen (`on_visibility`); off it, nothing wakes the poll just to draw.
    shown: bool,
    pub nav: views::Nav,
    /// The controller's view as the panes show it: all of it but the clock (`position` stays
    /// 0). The poll writes it only when something in it changed, so a clock tick rebuilds no
    /// pane region that reads it.
    pub ctl: Tracked<controller::View>,
    /// The playback clock as of the last tick or change: what the player bar shows.
    pub pos: Tracked<f64>,
}

impl App {
    fn new() -> App {
        let mut app = App { seen: (0, 0, 0), tick_at: 0.0, shown: true, nav: views::Nav::default(), ctl: Tracked::default(), pos: Tracked::new(0.0) };
        app.sync_controller();
        app
    }

    /// Copies the controller's view into `ctl` and `pos`, writing each only if it changed.
    fn sync_controller(&mut self) {
        let mut v = controller::view();
        let pos = std::mem::take(&mut v.position);
        if *Tracked::untracked(&self.ctl) != v {
            *self.ctl = v;
        }
        if *Tracked::untracked(&self.pos) != pos {
            *self.pos = pos;
        }
    }
}

fn poll(s: &mut App, t: f64, tx: &mut Tx) -> bool {
    perf::poll_ran();
    // One 200 ms tick, asked for again only by the poll it wakes. Rune keeps every distinct
    // deadline, so a fresh wake_after from every poll (frames and input poll too) starts
    // another endless chain each time, and they pile up to a poll per frame interval. It only
    // serves drawing, so it stops while the window is off screen (on_visibility restarts it).
    if t >= s.tick_at && s.shown {
        s.tick_at = t + 0.2;
        tx.wake_after(Duration::from_millis(200));
    }
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
    queue_ui::apply_nav(s);
    // links waiting for sign-in, the window's place, the full player's idle inhibit
    shell::tick(s);
    data::nudge_due_retries();
    // (Hover intent, the players' control fades and the lyric line only draw: off screen they
    // wait, and catch up on the poll that showing the window runs.)
    if let Some(next) = interact::tick_hover(s).filter(|_| s.shown) {
        tx.wake_after(next);
    }
    interact::settle_create(s);
    if let Some(next) = interact::toast_tick() {
        tx.wake_after(next.min(Duration::from_millis(200)));
    }
    interact::announce_track(tx);
    // The video player: open it for a new video, fade its controls when the pointer rests.
    let (video_changed, video_next) = video_ui::tick(s);
    if let Some(next) = video_next.filter(|_| s.shown) {
        tx.wake_after(next + Duration::from_millis(5));
    }
    // The full-screen player: closes with nothing playing, follows MPRIS, fades its controls.
    let (full_changed, full_next) = full_ui::tick(s);
    if let Some(next) = full_next.filter(|_| s.shown) {
        tx.wake_after(next + Duration::from_millis(5));
    }
    // The signal path panel's 2 s probe while it's open.
    let (path_changed, path_next) = signal_path::tick();
    if let Some(next) = path_next.filter(|_| s.shown) {
        tx.wake_after(next);
    }
    // A space press that toggled playback under the open player lands without the pause shrink.
    if fx::follow(s.nav.full.open, controller::clock().1) {
        tx.instant();
    }
    let (state, ticks) = controller::versions();
    // (Covers don't appear here: art.rs fills their pixels in place, without a rebuild.)
    // (Home's carousel arrows change as a row reaches an end: home::version.)
    let v = (state, ticks, data::version().wrapping_add(interact::version() << 40).wrapping_add(home::version() << 20));
    let mut changed = v != s.seen;
    if (state, ticks) != (s.seen.0, s.seen.1) {
        s.sync_controller();
    }
    // Synced lyrics: the active line changes at its timestamp, not on the next clock tick, and
    // (unless the user scrolled away) the drawer glides to keep it centred (LyricsTab).
    let lyric_moved = lyric_tick(s, tx);
    // A clock tick (or a seek bar drag) lands at once; a seek, a new track, new data or the next
    // lyric line animate, as does the full player's opening, closing or controls fading.
    if changed && !lyric_moved && !full_changed && ((state, v.2) == (s.seen.0, s.seen.2) || ui::dragging()) {
        tx.instant();
    }
    changed |= lyric_moved || video_changed || full_changed || path_changed;
    s.seen = v;
    changed
}

/// The drawer's Lyrics tab and the full player's lyrics column each follow the active line.
fn lyric_tick(s: &mut App, tx: &mut Tx) -> bool {
    let drawer = s.nav.drawer == Some(views::Drawer::Lyrics);
    let full = s.nav.full.open && s.nav.full.lyrics;
    // LyricsTab forgets a manual scroll for each new track, and whenever it is shown again.
    if LYRICS_TAB.with(|l| l.shown(drawer.then(|| controller::now_playing().0).flatten())) && *Tracked::untracked(&s.nav.lyrics_scrolled) {
        *s.nav.lyrics_scrolled = false;
    }
    let lines = (drawer || full)
        .then(|| controller::now_playing().0).flatten()
        .and_then(views::synced_lyrics);
    let Some(lines) = lines else {
        let was = clear_line(&mut s.nav.lyric_line) | s.nav.full.lyric_line.take().is_some();
        return was;
    };
    let (pos, playing) = controller::clock();
    let (line, next) = views::lyric_at(&lines, pos);
    if playing && s.shown && let Some(at) = next {
        tx.wake_after(Duration::from_secs_f64((at - pos).max(0.0) + 0.005));
    }
    let mut moved = false;
    if !drawer {
        moved |= clear_line(&mut s.nav.lyric_line);
    } else if line != *Tracked::untracked(&s.nav.lyric_line) {
        *s.nav.lyric_line = line;
        if let Some(i) = line
            && !*s.nav.lyrics_scrolled
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

/// Which track the drawer's Lyrics tab shows (None while it is hidden). In React each new track
/// (or reopening the tab) starts a fresh LyricsTab, with `userScrolled` false again.
#[derive(Default)]
struct LyricsTab(std::cell::Cell<Option<u64>>);

impl LyricsTab {
    /// Records what the tab shows now; whether that is a new track on a shown tab.
    fn shown(&self, track: Option<u64>) -> bool {
        let was = self.0.replace(track);
        track.is_some() && was != track
    }
}

thread_local! {
    static LYRICS_TAB: LyricsTab = LyricsTab::default();
}

/// Clears the drawer's lyric line; whether it had one. Writes only then: every poll calls it.
fn clear_line(line: &mut Tracked<Option<usize>>) -> bool {
    Tracked::untracked(line).is_some() && line.take().is_some()
}

fn on_key(s: &mut App, e: &KeyEvent, tx: &mut Tx) -> bool {
    // any key brings the full player's controls back
    full_ui::wake(s);
    // the Keyboard Shortcuts dialog waiting for a new key takes every key, Escape included
    if keys::editing().is_some() && keys::on_key(s, e, tx) {
        return true;
    }
    // a context menu is the top Escape layer (React dismissStack), then the full-screen or
    // the video player (never open together)
    if matches!(e.key, Key::Escape) && (interact::on_escape(s) || full_ui::on_escape(s) || video_ui::on_escape(s, tx)) {
        return true;
    }
    if matches!(e.key, Key::Escape) && (s.nav.menu_open || s.nav.settings_open) {
        if s.nav.menu_open { s.nav.menu_open = false } else { s.nav.settings_open = false }
        return true;
    }
    // the now-playing drawer is the last Escape layer (NowPlayingDrawer useEscapeDismiss)
    if matches!(e.key, Key::Escape) && Tracked::untracked(&s.nav.drawer).is_some() {
        *s.nav.drawer = None;
        return true;
    }
    // the bound shortcuts (keys.rs): Space, Ctrl+K, Alt+S, … as the user set them
    keys::on_key(s, e, tx)
}

/// The window went off screen (minimised, hidden or covered whole) or came back. Off screen,
/// nothing wakes the poll only to draw (the 200 ms tick, the lyric line, hover intent and the
/// players' fades wait), the controller stops its position poll unless the miniplayer shows the
/// clock, and toasts hold their time. Showing the window draws a frame, whose poll restarts the
/// tick and catches the rest up.
fn on_visibility(s: &mut App, shown: bool, _: &mut Tx) {
    log::info!("[rune] main window {}", if shown { "on screen" } else { "off screen" });
    s.shown = shown;
    controller::set_clock_shown(controller::MAIN, shown);
    interact::hold_toasts(!shown);
}

thread_local! {
    /// Each pane's inputs and `Tracked` reads at its last build, for counting builds (SONE_PERF).
    static PANES: std::cell::RefCell<std::collections::HashMap<String, (u64, rune::Reads)>> = Default::default();
}

/// A pane of the main window: `rune::region_with`, built again only when a `Tracked` value it
/// read was written or `inputs` changed. With SONE_PERF it counts its builds (`regions`): a
/// build is due when its inputs or reads changed, which leaves out the build debug builds run
/// to check a reused region.
fn pane(id: &str, inputs: impl std::hash::Hash, build: impl FnOnce() -> El) -> El {
    if !perf::on() {
        return rune::region_with(id, inputs, build);
    }
    let inputs = {
        use std::hash::{BuildHasher, BuildHasherDefault};
        BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(inputs)
    };
    let due = PANES.with_borrow(|p| p.get(id).is_none_or(|(h, reads)| *h != inputs || reads.changed()));
    rune::region_with(id, inputs, || {
        let (el, reads) = rune::tracked_reads(build);
        if due {
            perf::region_built();
            PANES.with_borrow_mut(|p| p.insert(id.to_string(), (inputs, reads)));
        }
        el
    })
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    perf::view_ran();
    // the UI zoom (Ctrl+= / Ctrl+-), and the window's size for the next start
    let win_size = shell::view(cx);
    art::set_scale(cx.env().scale);
    let v = controller::view();
    // What the panes read besides `App`'s `Tracked` fields, read once for this build and given
    // to their regions as inputs: the catalogue (data.rs, answering as of this version until
    // the build ends), favourites, toasts and menu results (interact.rs), Home's row arrows, and
    // the track playing and whether it plays (rows mark it; the ambient hue follows it).
    let (data_version, _frozen) = data::freeze();
    let now_playing = controller::now_playing();
    views::set_now_playing(now_playing);
    let stores = (data_version, interact::version(), home::version(), now_playing);
    theme::set_ambient(ui::hue(now_playing.0));
    // The root fills the window, so its width is the window's.
    let shell = cx.layout_with("root", |r| views::Shell::for_width(r[2])).unwrap_or_else(|| views::Shell::for_width(initial_size().0));
    // Until the saved sign-in has been checked (about 0.6 s, the keyring among it), neither the
    // login screen nor the shell: a signed-in launch went through the login screen first.
    if !v.auth_known {
        return shell::frame(views::splash(), win_size);
    }
    // Shell data (Home, folders, ...) is loaded by the controller once signed in.
    if !v.signed_in {
        return shell::frame(views::login(s, &v), win_size);
    }
    // The panes are regions: each builds again only when something it read changed (a `Tracked`
    // field of `App`), or its inputs did (the window's width bands, the stores outside `App`).
    // A clock tick changes only `pos`, which only the player bar reads.
    let header = pane("header", &s.nav.search, || views::header(s));
    // Settings reads the backend's settings directly, so it builds every time.
    let page = if *Tracked::untracked(&s.nav.page) == views::Page::Settings {
        views::page(s, &s.ctl)
    } else {
        pane(&views::page_key(s), (stores, &s.nav.filter), || views::page(s, &s.ctl))
    };
    let shell_in = (shell.sidebar_w.to_bits(), shell.drawer_docked, shell.drawer_w.to_bits(), shell.bar_source, shell.bar_badge);
    // The header spans the page and the drawer, so the window controls stay in the top-right
    // corner; the drawer sits beside the page below it, or over the page's right edge.
    let mut content = row().grow(1.0).min_h(0.0).w_full().child(page);
    if s.nav.drawer.is_some() {
        content = content.child(pane("drawer", (shell_in, stores), || views::drawer(s, &s.ctl, shell).unwrap_or_else(el)));
    }
    let main = col().grow(1.0).min_w(0.0).h_full().child(header).child(content);
    let sidebar = pane("sidebar", (shell.sidebar_w.to_bits(), stores), || views::sidebar(s, shell.sidebar_w));
    let body = row().grow(1.0).min_h(0.0).w_full().child(sidebar).child(main);
    // The player bar centres its transport from the window's width and its extras' width.
    let win_w = cx.layout_with("root", |r| r[2].round()).unwrap_or(initial_size().0);
    let room = (win_w, cx.layout_with("nowbar-extras", |r| r[2].ceil()));
    let bar_in = (shell_in, room.0.to_bits(), room.1.map(f32::to_bits), stores, crate::mini_open(), full_ui::covers_bar());
    let bar = pane("nowbar", bar_in, || {
        let mut v = (*s.ctl).clone();
        v.position = *s.pos;
        // The bar's cover opens the video player again, or the full-screen player.
        ui::nowbar_with::<App>(&v, views::bar_info(&v), views::bar_track_actions(&v), views::bar_extras(&v, shell, *s.nav.drawer), views::bar_cover_action(&v), room)
    });
    let mut root = col().key("root").w_full().h_full().fill(theme::ground()).child(body).child(bar);
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
    // (the quality badge's panel opens from the bar and from the full-screen player, over both)
    if let Some(p) = signal_path::overlay(&v) {
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
    // The perf line reports window focus; nothing redraws.
    if perf::on() {
        root = root.on_window_focus(|_: &mut App, focused, tx| {
            perf::set_focused(focused);
            tx.live_only();
        });
    }
    let _ = row;
    // the title bar above, a frameless window's resize edges (shell.rs)
    shell::frame(root, win_size)
}

/// The window's first size: SONE_WINDOW_SIZE (the verifier uses a tall one), else the size it
/// had when it last closed.
fn initial_size() -> (f32, f32) {
    shell::initial_size()
}

const FONTS: &[&[u8]] = &[include_bytes!("../assets/Inter-Regular.ttf"), include_bytes!("../assets/Inter-Medium.ttf"), include_bytes!("../assets/Inter-SemiBold.ttf")];

/// The app's name in the window title and login screen; package.nix builds it as "RONE".
pub const TITLE: &str = match option_env!("SONE_RUNE_TITLE") {
    Some(t) => t,
    None => "SONE",
};

fn main() {
    // One process per user, as Tauri's single-instance plugin kept it: a second launch hands its
    // `tidal://` link (or, without one, a reopen) to the running app, which raises its window,
    // and ends here before it starts a backend of its own.
    let app_id = option_env!("SONE_RUNE_APP_ID").unwrap_or("sone-rune");
    if !rune::single_instance(app_id) {
        return;
    }
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
        ..AppSpec::new(TITLE, initial_size(), App::new, view)
    };
    // Matches the desktop entry package.nix installs, so the desktop shows its name and icon.
    rune::set_app_id(app_id);
    // Rune now ends the process at once when the app quits, skipping everything after `run`;
    // the queue flush below and the logger's final flush (dropping `_logger`) need the
    // orderly teardown.
    rune::orderly_exit();
    // The window shell (shell.rs): the frame the title bar mode asks for and the saved place;
    // closing hides to the tray when Close to tray is on (keep_running), else quits; links and
    // second launches raise the window.
    let mut app = rune::App::new(spec)
        .on_visibility(on_visibility)
        .window_options(shell::window_options())
        .keep_running()
        .on_close(shell::on_close)
        .on_quit(shell::on_quit)
        .on_reopen(shell::on_reopen)
        .on_open_url(shell::on_open_url)
        .on_focus(shell::on_focus)
        .on_window(shell::on_window);
    shell::set_app_handle(app.handle());
    // The tray's Show and MPRIS Raise (the backend's restore_window, which has no window here).
    handle.listen("window:raise", |_| shell::raise());
    // Frame counts for SONE_PERF.
    if perf::on() {
        app = app.on_frame_encoded(perf::frame);
    }
    perf::start();
    mini::attach(&mut app);
    // A queue change still inside its save debounce is written on the way out (§1.5).
    handle.on_exit(|code| {
        controller::flush_snapshot();
        shell::on_exit();
        std::process::exit(code)
    });
    app.run();
    controller::flush_snapshot();
    shell::on_exit();
}

#[cfg(test)]
mod lyrics_tab_tests {
    use super::LyricsTab;

    #[test]
    fn a_manual_scroll_lasts_one_track() {
        let tab = LyricsTab::default();
        assert!(tab.shown(Some(1)), "opening the tab starts fresh");
        assert!(!tab.shown(Some(1)), "the same track keeps the user's scroll");
        assert!(tab.shown(Some(2)), "a new track follows again");
        assert!(!tab.shown(None), "a hidden tab resets nothing");
        assert!(tab.shown(Some(2)), "showing the tab again follows again");
    }
}

