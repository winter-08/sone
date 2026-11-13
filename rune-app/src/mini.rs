//! The miniplayer: React's second Tauri window, redrawn as the "Sleeve" design
//! (PLAYER_DESIGN.md).
//!
//! One borderless, always-on-top window whose layout follows its height: a single row under
//! 100 pt, cover and details over the seek bar and transport under 220 pt, and above that the
//! cover filling the window with the transport over it on hover. The prototype insets its card
//! in a transparent margin for a shadow; here the card is the whole (opaque) window, since a
//! Linux surface may be opaque only (NVIDIA under X11 or XWayland, Rune's translucency.rs) and
//! the margin would then draw black. So the card has square corners and no shadow, and its
//! hairline is the window's edge.

use crate::controller::{self, Cmd, Repeat, View, send};
use crate::glyph::{G, glyph};
use crate::interact::{self, Fav, Say};
use crate::ui::{t, tnum, white};
use crate::{art, theme, ui};
use rune::{Align, AppSpec, Color, Curve, Cx, Drag, Easing, El, Key, KeyEvent, Phase, Tx, col, el, oklch, row, text};
use std::cell::Cell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static WINDOW: OnceLock<rune::WindowHandle> = OnceLock::new();
static OPEN: AtomicBool = AtomicBool::new(false);
/// The window is open and on screen (not minimised, hidden or covered whole).
static SHOWN: AtomicBool = AtomicBool::new(false);

const SIZE: (f32, f32) = (372.0, 196.0);
const MIN: (f32, f32) = (244.0, 88.0);
const MAX: (f32, f32) = (672.0, 776.0);

/// The window chrome (close button, grips) fades in with the pointer; the cover's transport
/// overlay a little slower, as it covers more.
const CHROME_FADE: Curve = Curve::tween(0.15, Easing::EASE);
const OVERLAY_FADE: Curve = Curve::tween(0.2, Easing::EASE);

pub fn mini_open() -> bool {
    OPEN.load(Ordering::Relaxed)
}

/// Open or close the miniplayer window.
pub fn set_mini(open: bool) {
    OPEN.store(open, Ordering::Relaxed);
    set_shown(open);
    if let Some(w) = WINDOW.get() {
        // its saved size and place (shell.rs), as React's useMiniplayerWindow keeps them
        if open {
            crate::shell::restore_mini(w);
            CHECK_PLACE.store(true, Ordering::Relaxed);
            w.open()
        } else {
            crate::shell::save_mini(w);
            w.close()
        }
    }
    controller::bump();
}

/// The window just opened at its saved place: its first poll checks that place is on a screen.
static CHECK_PLACE: AtomicBool = AtomicBool::new(false);

/// Where to move a window at `pos` and `size` (physical pixels) so that it's on `display`
/// (its position and size), or None when at least 50 px of it already are: then near the
/// display's bottom-right corner (useMiniplayerWindow's off-screen clamping).
fn back_on_screen(pos: (i32, i32), size: (u32, u32), display: ((i32, i32), (u32, u32))) -> Option<(i32, i32)> {
    let ((mx, my), (mw, mh)) = display;
    let (right, bottom) = (pos.0 + size.0 as i32, pos.1 + size.1 as i32);
    let on = pos.0 < mx + mw as i32 - 50 && right > mx + 50 && pos.1 < my + mh as i32 - 50 && bottom > my + 50;
    (!on).then(|| (mx + mw as i32 - size.0 as i32 - 40, my + mh as i32 - size.1 as i32 - 100))
}

/// From the poll, once after opening: a saved place on a screen that's gone (a monitor
/// unplugged) moves back onto the one the window is on. Where the platform hides positions
/// (Wayland) the compositor places the window and there's nothing to do.
fn keep_on_screen() {
    let Some(w) = WINDOW.get().filter(|w| w.is_ready()) else { return };
    if !CHECK_PLACE.swap(false, Ordering::Relaxed) {
        return;
    }
    let Some(p) = w.placement() else { return };
    if let Some(at) = p.display.and_then(|d| back_on_screen(p.position, p.content_size, d)) {
        w.set_position(at);
    }
}

/// While the full-screen player is up, an open miniplayer steps out of the way (hidden, still
/// open) and comes back after, as React's MaximizedPlayer hides it to avoid the always-on-top
/// clash.
pub fn hide_for_full(hidden: bool) {
    if !mini_open() {
        return;
    }
    if let Some(w) = WINDOW.get() {
        if hidden { w.hide() } else { w.show() }
    }
    set_shown(!hidden);
}

/// Whether the miniplayer is on screen: its tick runs, and it keeps the controller's position
/// poll going while the main window is away.
fn set_shown(shown: bool) {
    SHOWN.store(shown, Ordering::Relaxed);
    controller::set_clock_shown(controller::MINI, shown);
}

/// Adds the miniplayer to `app` as a second, hidden window; [`set_mini`] shows it.
pub fn attach<S: 'static>(app: &mut rune::App<S>) {
    let spec = AppSpec::<Mini> {
        fonts: crate::FONTS,
        families: ("Inter", "ui-monospace"),
        poll: Some(poll),
        on_key: Some(on_key),
        min_size: Some(MIN),
        ..AppSpec::new("SONE Miniplayer", SIZE, Mini::new, view)
    };
    // Borderless: no compositor edges to resize by on Wayland, so the card has its own grip.
    let options = rune::WindowOptions { borderless: true, always_on_top: true, resizable: true, visible: false, min_size: Some(MIN), max_size: Some(MAX), ..Default::default() };
    let mini = rune::App::new(spec).on_close(|_, _| {
        if let Some(w) = WINDOW.get() {
            crate::shell::save_mini(w);
        }
        OPEN.store(false, Ordering::Relaxed);
        set_shown(false);
        controller::bump(); // the main window's miniplayer button says "Open" again
    })
    // Minimised, hidden or covered whole, and back: showing it draws a frame, whose poll
    // restarts the tick.
    .on_visibility(|_, shown, _| {
        log::info!("[rune] miniplayer {}", if shown { "on screen" } else { "off screen" });
        set_shown(shown && mini_open());
    });
    let (_, window) = app.window(rune::WindowSpec::from_app("mini", mini).options(options));
    let _ = WINDOW.set(window);
}

pub struct Mini {
    seen: (u64, u64, u64),
    /// When the 150 ms tick's next wake is due (Rune's clock).
    tick_at: f64,
    /// The window's size as last laid out. The view writes it; the resize grip reads it when
    /// a drag starts.
    size: Cell<(f32, f32)>,
    /// The size the window had when the resize grip's drag started.
    grab: Option<(f32, f32)>,
    /// When the seek bar was last dragged, so the changes it causes land at once.
    seek_drag: Option<Instant>,
    /// Tab moved focus here and the pointer hasn't come back since: the chrome and the cover's
    /// transport stay shown, so a focused control is never invisible.
    keyboard: bool,
    /// The speaker's volume slider is open.
    vol_open: bool,
    /// The playback error the card shows, while it's under four seconds old.
    error: Option<String>,
}

impl Mini {
    fn new() -> Mini {
        Mini { seen: (0, 0, 0), tick_at: 0.0, size: Cell::new(SIZE), grab: None, seek_drag: None, keyboard: false, vol_open: false, error: None }
    }
}

/// How long the card shows a playback error (React MiniPlayer's ErrorOverlay).
const ERROR_FOR: Duration = Duration::from_secs(4);

fn poll(s: &mut Mini, t: f64, tx: &mut Tx) -> bool {
    if CHECK_PLACE.load(Ordering::Relaxed) {
        keep_on_screen();
    }
    let (state, ticks) = controller::versions();
    // (favourites too: a heart toggled in the main window shows here)
    let v = (state, ticks, interact::version());
    // One 150 ms tick, asked for again only by the poll it wakes (a fresh wake_after from every
    // poll starts another endless chain each time, as in main::poll), and only while the window
    // is on screen: hidden, it still polls on its timer.
    if t >= s.tick_at && SHOWN.load(Ordering::Relaxed) {
        s.tick_at = t + 0.15;
        tx.wake_after(Duration::from_millis(150));
    }
    // A playback error shows for four seconds; the tick notices when it's due to go.
    let error = controller::last_error().filter(|(_, at)| at.elapsed() < ERROR_FOR).map(|(e, _)| e);
    let error_changed = error != s.error;
    s.error = error;
    let changed = v != s.seen || error_changed;
    let dragging = s.seek_drag.is_some_and(|t| t.elapsed() < Duration::from_millis(250));
    if changed && ((v.0, v.2) == (s.seen.0, s.seen.2) || dragging) {
        tx.instant(); // a clock tick lands at once, as in the main window
    }
    s.seen = v;
    changed
}

fn on_key(s: &mut Mini, e: &KeyEvent, _: &mut Tx) -> bool {
    if e.mods.cmd || e.mods.ctrl || e.mods.alt {
        return false;
    }
    match &e.key {
        // (the volume slider first, then the window)
        Key::Escape if s.vol_open => s.vol_open = false,
        Key::Escape => set_mini(false),
        Key::Text(t) if t == " " => send(Cmd::TogglePlay),
        Key::Tab => {
            s.keyboard = true;
            return false;
        }
        _ => return false,
    }
    true
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    /// Under 100 pt: one row.
    Narrow,
    /// Under 220 pt: details, then seek and transport.
    Compact,
    /// The cover fills the window.
    Full,
}

impl Tier {
    fn of(h: f32) -> Tier {
        if h < 100.0 { Tier::Narrow } else if h < 220.0 { Tier::Compact } else { Tier::Full }
    }
}

fn view(s: &Mini, cx: &mut Cx<Mini>) -> El {
    let v = controller::view();
    let hue = ui::hue(v.track_id);
    // This window's own frame: its hue and a fresh glass budget, nothing left from the main one.
    theme::set_ambient(hue);
    let (w, h) = cx.layout_with("mini-root", |r| (r[2].round(), r[3].round())).unwrap_or(SIZE);
    s.size.set((w, h));
    crate::shell::note_mini_size((w, h));
    let tier = Tier::of(h);
    let body = if v.current.is_none() && v.video.is_none() {
        empty(tier, w, h)
    } else {
        match tier {
            Tier::Narrow => narrow(&v, w, h),
            Tier::Compact => compact(&v, w, h),
            Tier::Full => full(s, &v, w, h),
        }
    };
    let rest = if s.keyboard { 1.0 } else { 0.0 };
    let close_at = match tier {
        // over the cover, the only quiet spot in one row
        Tier::Narrow => (8.0 + (h - 16.0) / 2.0 - 14.0, h / 2.0 - 14.0),
        _ => (w - 31.0, 3.0),
    };
    let mut root = col()
        .key("mini-root")
        .w_full()
        .h_full()
        .fill(ground(&v, hue))
        .border_gradient(1.0, white(0.13), white(0.07))
        .window_drag()
        .hover_group()
        .hover_motion(CHROME_FADE, CHROME_FADE)
        .on_hover_change(|s: &mut Mini, inside, _| if inside { s.keyboard = false })
        .on_key(|s: &mut Mini, e, _| {
            if matches!(e.key, Key::Tab) {
                s.keyboard = true;
            }
            false
        })
        // A tier change follows a resize, and a new track replaces the card's contents: both
        // land at once. (The ground's colour keeps its spring.)
        .child(body.key("mini-body").w_full().h_full().instant_changes())
        .child(el().absolute(close_at.0, close_at.1).group_opacity(rest, 1.0).child(close()));
    if tier != Tier::Narrow {
        root = root.child(grip().absolute((w - 29.0) / 2.0, 3.0).group_opacity(rest, 1.0));
    }
    // A failed play says why for a moment, over the middle of the card (ErrorOverlay).
    if let Some(e) = s.error.as_ref().filter(|_| v.current.is_some() || v.video.is_some()) {
        root = root.child(row().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).items_center().justify_center().a11y_hidden()
            .child(row().max_w((w * 0.9).max(80.0)).px(12.0).py(6.0).radius(6.0).fill(oklch(0.38, 0.13, 25.0).alpha(0.7))
                .child(text(e.clone()).font(t(11.0, 500.0)).color(white(1.0)).truncate())));
    }
    // The speaker's slider, over everything (it's anchored to the speaker, where there's one).
    let speaker = v.current.is_some() && !volume_locked(&v) && (tier != Tier::Narrow || w >= 420.0);
    if s.vol_open && speaker {
        root = root.child(volume_slider(&v, tier == Tier::Full));
    }
    root.child(resize_grip().group_opacity(rest, 1.0))
}

/// Near-black tinted with the cover's most vivid colour at low chroma, or the track's hue
/// for a grey cover or none.
fn ground(v: &View, hue: f32) -> Color {
    match art::cover_color(v.cover.as_deref()) {
        Some(c) if c.c >= 0.03 => oklch(0.215, (c.c * 0.35).min(0.06), c.h),
        _ => oklch(0.2, 0.02, hue),
    }
}

// ---- tiers

fn narrow(v: &View, w: f32, h: f32) -> El {
    // next, then previous, then volume as the width allows
    let mut controls = row().gap(2.0).items_center().child(heart(v));
    if w >= 330.0 {
        controls = controls.child(prev(28.0, 16.0, white(0.7)));
    }
    controls = controls.child(play(v, 32.0));
    if w >= 290.0 {
        controls = controls.child(next(28.0, 16.0, white(0.7)));
    }
    if w >= 420.0 {
        controls = controls.child(volume(v));
    }
    row()
        .pt(8.0)
        .pb(8.0)
        .pl(8.0)
        .pr(10.0)
        .gap(10.0)
        .items_center()
        .child(cover(v, h - 16.0))
        .child(col().grow(1.0).min_w(0.0).gap(2.0).child(title(v, 13.0)).child(artist(v, 12.0)))
        .child(controls)
}

fn compact(v: &View, w: f32, h: f32) -> El {
    let tight = h < 110.0;
    let (pad_y, gap, play_size) = if tight { (8.0, 6.0, 30.0) } else { (10.0, 8.0, 36.0) };
    let seek = h >= 140.0;
    let heart_top = h >= 150.0;
    let top_h = h - 2.0 * pad_y - play_size - gap - if seek { 20.0 + gap } else { 0.0 };
    let mut meta = col().grow(1.0).min_w(0.0).justify_center().gap(2.0).pr(if heart_top { 4.0 } else { 24.0 }).child(title(v, if tight { 14.0 } else { 15.0 }));
    if h >= 104.0 {
        meta = meta.child(artist(v, 13.0));
    }
    if let Some(from) = playing_from(v).filter(|_| seek) {
        meta = meta.child(from);
    }
    let mut top = row().grow(1.0).min_h(0.0).gap(12.0).child(cover(v, top_h.min(96.0)).self_center()).child(meta);
    if heart_top {
        top = top.child(heart(v).self_end());
    }
    let wide = w >= 300.0;
    let mut middle = row().gap(4.0).items_center();
    if wide {
        middle = middle.child(shuffle(v));
    }
    middle = middle.child(prev(28.0, 16.0, white(0.7))).child(play(v, play_size).mx(2.0)).child(next(28.0, 16.0, white(0.7)));
    if wide {
        middle = middle.child(repeat(v));
    }
    let mut left = row().grow(1.0).items_center();
    if !heart_top {
        left = left.child(heart(v));
    }
    let transport = row().items_center().child(left).child(middle).child(row().grow(1.0).justify_end().items_center().child(volume(v)));
    let mut c = col().px(10.0).py(pad_y).gap(gap).child(top);
    if seek {
        c = c.child(row().gap(8.0).items_center().child(time(v.position, 0.6).align(Align::End)).child(seek_bar(v)).child(time(v.duration, 0.6)));
    }
    c.child(transport)
}

fn full(s: &Mini, v: &View, w: f32, h: f32) -> El {
    // (React's full tier names the collection under the artist too)
    let from = playing_from(v);
    let sz = (w - 16.0).min(h - if from.is_some() { 94.0 } else { 76.0 });
    let small = h < 330.0 || w < 230.0;
    let (play_size, side, side_glyph, gap) = if small { (38.0, 30.0, 18.0, 6.0) } else { (52.0, 40.0, 22.0, 14.0) };
    let mut seek = row().pin(Some(10.0), None, Some(10.0), Some(6.0)).gap(6.0).items_center();
    seek = if small { seek.child(seek_bar(v)) } else { seek.child(time(v.position, 0.75).align(Align::End)).child(seek_bar(v)).child(time(v.duration, 0.75)) };
    // Shown while the pointer is over the cover, and always while paused or reached by Tab.
    let rest = if !v.playing || s.keyboard { 1.0 } else { 0.0 };
    let overlay = el()
        .absolute(0.0, 0.0)
        .size(sz, sz)
        .radius(6.0)
        .linear_gradient(0.0, [(0.0, theme::black(0.72)), (0.45, theme::black(0.38)), (1.0, theme::black(0.2))])
        .group_opacity(rest, 1.0)
        .child(
            // the transport, and under it volume, shuffle and repeat (React's full tier)
            col().absolute(0.0, 0.0).size(sz, sz).gap(if small { 6.0 } else { 12.0 }).items_center().justify_center()
                .child(row().gap(gap).items_center()
                    .child(prev(side, side_glyph, white(0.92)))
                    .child(play(v, play_size))
                    .child(next(side, side_glyph, white(0.92))))
                .child(row().gap(if small { 4.0 } else { 12.0 }).items_center().child(volume(v)).child(shuffle(v)).child(repeat(v))),
        )
        .child(seek);
    let art = el().key("mini-art").size(sz, sz).hover_group().hover_motion(OVERLAY_FADE, OVERLAY_FADE).child(cover(v, sz)).child(overlay);
    let meta = row()
        .w(sz.max((w - 16.0).min(200.0)))
        .px(2.0)
        .gap(8.0)
        .items_center()
        .child(col().grow(1.0).min_w(0.0).gap(2.0).child(title(v, 14.0)).child(artist(v, 12.0)).children(from))
        .child(heart(v));
    col().padding(8.0).gap(10.0).items_center().justify_center().child(art).child(meta)
}

/// Nothing playing: an empty sleeve and a line saying so, with the play button there but
/// unavailable.
fn empty(tier: Tier, w: f32, h: f32) -> El {
    let words = || {
        col().grow(1.0).min_w(0.0).gap(2.0)
            .child(text("Nothing playing").font(t(14.0, 600.0)).color(white(0.95)).truncate())
            .child(text(format!("Start something in {}", crate::TITLE)).font(t(12.0, 400.0)).color(white(0.6)).truncate())
    };
    let sleeve = |size: f32, g: f32| {
        el().size(size, size).radius(6.0).fill(white(0.05)).border(1.0, white(0.08)).items_center().justify_center().a11y_hidden()
            .child(glyph(G::Music, g, white(0.35)))
    };
    let idle = |size: f32| play_button(None, size);
    match tier {
        // laid out as the playing card's: the sleeve, then the words with the button beside them
        Tier::Full => {
            let sz = (w - 16.0).min(h - 76.0);
            col().padding(8.0).gap(10.0).items_center().justify_center()
                .child(sleeve(sz, 36.0))
                .child(row().w(sz.max((w - 16.0).min(200.0))).px(2.0).gap(8.0).items_center().child(words()).child(idle(32.0)))
        }
        _ => {
            let pad = if tier == Tier::Narrow { 8.0 } else { 10.0 };
            row().padding(pad).pr(pad + 2.0).gap(12.0).items_center()
                .child(sleeve((h - 2.0 * pad).min(96.0), 20.0))
                .child(words())
                .child(idle(if tier == Tier::Narrow { 32.0 } else { 36.0 }))
        }
    }
}

// ---- parts

/// The miniplayer's title, artist and "Playing from" open their pages in the main window
/// (useMiniplayerEmitter show-album / show-artist / show-source).
fn link(e: El, key: &str, label: String, page: Option<crate::views::Page>) -> El {
    let Some(page) = page else { return e };
    theme::focusable(el().key(key.to_string()).role("link").label(label)).shrink().min_w(0.0).radius(3.0).hover_group().cursor(rune::Cursor::Pointer)
        .on_press(move |_: &mut Mini| {
            crate::queue_ui::request_nav(Some(page.clone()));
            // the page opens in the main window: bring it forward (from the tray too)
            crate::shell::raise();
        })
        .child(e.group_color(white(1.0)).a11y_hidden())
}

/// "Playing from" and the collection: the one the context queue came from, else the playing
/// source, for both the name and where it leads (useMiniplayerEmitter's `contextSource ||
/// playbackSource`). A source with no page (a search, a radio) is plain text.
fn playing_from(v: &View) -> Option<El> {
    let src = v.context_source.as_ref().or(v.source.as_ref()).filter(|s| !s.name.is_empty())?;
    let page = Some(src).filter(|s| crate::queue_ui::navigable(s)).and_then(crate::queue_ui::source_page);
    let name = src.name.clone();
    Some(
        row().mt(2.0).gap(4.0).min_w(0.0)
            .child(text("Playing from").font(t(12.0, 400.0)).color(white(0.4)))
            .child(col().shrink().min_w(0.0).child(link(text(name.clone()).font(t(12.0, 400.0)).color(white(0.5)).truncate(), "mini-source", name, page))),
    )
}

fn title(v: &View, size: f32) -> El {
    let words = text(v.title.clone()).font(t(size, 600.0)).color(white(0.95)).truncate();
    let album = v.current.as_ref().filter(|_| v.video.is_none()).and_then(|c| c.album_id).map(crate::views::Page::Album);
    let words = link(words, "mini-title", v.title.clone(), album);
    // (MiniPlayer marks AI-generated tracks; it shows no explicit badge)
    if v.current.as_ref().is_some_and(|c| c.ai) && v.video.is_none() {
        return rune::row().min_w(0.0).gap(6.0).items_center().child(words.shrink()).child(crate::content::ai_badge());
    }
    words
}

fn artist(v: &View, size: f32) -> El {
    let words = text(v.artist.clone()).font(t(size, 400.0)).color(white(0.6)).truncate();
    let page = v.current.as_ref().filter(|_| v.video.is_none()).and_then(|c| c.artist_id).map(crate::views::Page::Artist);
    link(words, "mini-artist", v.artist.clone(), page)
}

/// A clock time beside the seek bar, wide enough for "1:07:00" so nothing shifts as it ticks.
fn time(secs: f64, alpha: f32) -> El {
    text(ui::fmt_time(secs)).font(tnum(t(11.0, 500.0))).color(white(alpha)).w(42.0)
}

/// The cover at `size` points (the Afterglow tile until it's in). Its pixels are asked for
/// at a few fixed sizes, so dragging the window's size doesn't decode a new one each step.
fn cover(v: &View, size: f32) -> El {
    const SIDES: [f32; 6] = [64.0, 96.0, 160.0, 320.0, 480.0, 672.0];
    let side = SIDES.into_iter().find(|s| *s >= size).unwrap_or(SIDES[5]);
    let tile = ui::tile(ui::hue(v.track_id), size, v.track_id.unwrap_or(0), 6.0);
    match art::cover(v.cover.as_deref(), side).and_then(|u| art::pixels(&u, side)) {
        Some(px) => tile.child(el().absolute(0.0, 0.0).size(size, size).radius(6.0).pixels_fit(&px, rune::SurfaceFit::Fill)),
        None => tile,
    }
}

/// A round icon button `size` points across (28 in the card, the prototype's `.sl-mb`). With
/// `brighten`, the glyph whitens under the pointer; an on-state (accent) glyph keeps its colour.
fn button(key: &str, label: &str, g: G, c: Color, size: f32, glyph_size: f32, brighten: bool) -> El {
    let mark = glyph(g, glyph_size, c);
    theme::focusable(el().key(key.to_string()).role("button").label(label))
        .size(size, size)
        .radius(size / 2.0)
        .items_center()
        .justify_center()
        .hover(white(0.08))
        .press_scale(0.92)
        .cursor(rune::Cursor::Pointer)
        .child(if brighten { mark.group_color(white(0.95)) } else { mark })
}

/// `b`, unavailable and dimmed.
fn off(b: El) -> El {
    ui::unavailable(b).opacity(0.3)
}

/// The white play button; `playing` is None when there's nothing to play.
fn play_button(playing: Option<bool>, size: f32) -> El {
    let on = playing == Some(true);
    let b = theme::focusable(el().key("mini-play").role("button").label(if on { "Pause" } else { "Play" }))
        .size(size, size)
        .radius(size / 2.0)
        .fill(white(0.97))
        .press_scale(0.94)
        .cursor(rune::Cursor::Pointer)
        .items_center()
        .justify_center()
        // the play triangle's optical centre is right of its box's
        .child(glyph(if on { G::Pause } else { G::Play }, size * 0.44, theme::ink()).offset(if on { 0.0 } else { 1.5 }, 0.0));
    if playing.is_some() { b.on_press(|_: &mut Mini| send(Cmd::TogglePlay)) } else { off(b) }
}

fn play(v: &View, size: f32) -> El {
    play_button(Some(v.playing), size)
}

fn prev(size: f32, glyph_size: f32, c: Color) -> El {
    button("mini-prev", "Previous", G::Prev, c, size, glyph_size, true).on_press(|_: &mut Mini| send(Cmd::Previous))
}

/// Never disabled, as the player bar's: with the queue empty and repeat off, Next still starts
/// the Autoplay radio when that's on (React MiniPlayer's play-next).
fn next(size: f32, glyph_size: f32, c: Color) -> El {
    button("mini-next", "Next", G::Next, c, size, glyph_size, true).on_press(|_: &mut Mini| send(Cmd::Next))
}

/// Shuffle and repeat: muted when off, the accent with a dot under them when on.
fn mode(key: &str, label: &str, g: G, on: bool) -> El {
    let b = button(key, label, g, if on { theme::accent() } else { white(0.45) }, 28.0, 16.0, !on).checked(on);
    if on { b.child(el().absolute(12.5, 26.0).size(3.0, 3.0).radius(1.5).fill(theme::accent())) } else { b }
}

fn shuffle(v: &View) -> El {
    mode("mini-shuffle", "Shuffle", G::Shuffle, v.shuffle).on_press(|_: &mut Mini| send(Cmd::ToggleShuffle))
}

fn repeat(v: &View) -> El {
    let label = match v.repeat {
        Repeat::Off => "Repeat: off",
        Repeat::All => "Repeat: all",
        Repeat::One => "Repeat: one track",
    };
    let mut b = mode("mini-repeat", label, G::Repeat, v.repeat != Repeat::Off);
    if v.repeat == Repeat::One {
        b = b.child(
            row().absolute(19.0, -1.0).h(11.0).min_w(11.0).px(2.0).radius(5.5).fill(theme::accent()).items_center().justify_center().a11y_hidden()
                .child(text("1").font(tnum(t(8.0, 600.0))).color(theme::ink())),
        );
    }
    b.on_press(|_: &mut Mini| send(Cmd::CycleRepeat))
}

/// The favourite toggle, as the player bar's heart: a video's is a video favourite.
fn heart(v: &View) -> El {
    let fav = match (&v.video, &v.current) {
        (Some(vi), _) => Some((Fav::Video, vi.id.to_string(), vi.title.clone())),
        (None, Some(c)) => Some((Fav::Track, c.id.to_string(), c.title.clone())),
        (None, None) => None,
    };
    let loved = fav.as_ref().is_some_and(|(k, id, _)| interact::is_fav(*k, id));
    let b = button(
        "mini-love",
        if loved { "Remove from favorites" } else { "Add to favorites" },
        if loved { G::HeartFilled } else { G::Heart },
        if loved { theme::accent() } else { white(0.7) },
        28.0,
        16.0,
        !loved,
    )
    .checked(loved);
    match fav {
        Some((k, id, label)) => b.on_press(move |_: &mut Mini| interact::toggle_fav(k, id.clone(), label.clone(), Say::Nothing)),
        None => off(b),
    }
}

/// Whether the volume control is locked: bit-perfect audio, though a video's own sound isn't.
fn volume_locked(v: &View) -> bool {
    v.bit_perfect && v.video.is_none()
}

/// The speaker: a press opens the volume slider beside it (React MiniPlayer's VolumeSlider);
/// the wheel over it steps the volume. Locked in bit-perfect mode.
fn volume(v: &View) -> El {
    if volume_locked(v) {
        return ui::unavailable(button("mini-volume", "Volume disabled in bit-perfect mode", G::Speaker, white(0.35), 28.0, 16.0, false));
    }
    let vol = v.volume;
    let g = if vol <= 0.0 { G::VolumeX } else if vol < 0.5 { G::Volume1 } else { G::Speaker };
    button("mini-volume", "Volume", g, white(0.7), 28.0, 16.0, true)
        .on_press(|s: &mut Mini| s.vol_open = !s.vol_open)
        .on_wheel(move |_: &mut Mini, ev, _| {
            if ev.dy == 0.0 {
                return false;
            }
            // a notch is 5%; a trackpad moves it in proportion, 0.4% a Wayland axis unit, which
            // Rune now reports as 12 pt (Chromium's kWheelDelta / kAxisValueScale) where it was 1
            const PER_POINT: f32 = if cfg!(target_os = "linux") { 0.004 / 12.0 } else { 0.004 };
            let step = if ev.precise { ev.dy * PER_POINT } else { ev.dy.signum() * 0.05 };
            send(Cmd::SetVolume((vol + step).clamp(0.0, 1.0)));
            true
        })
}

/// The slider the speaker opens: 80 pt on a dark pill, beside the speaker in a row (`tall`
/// false) or over it in the large tier, as React's horizontal and vertical VolumeSlider. It
/// closes when the pointer leaves it, or with another press on the speaker or Escape.
fn volume_slider(v: &View, tall: bool) -> El {
    let vol = v.volume.clamp(0.0, 1.0);
    let knob = el().size(12.0, 12.0).radius(6.0).fill(white(1.0)).shadow(1.0, 3.0, theme::black(0.45));
    let track = if tall {
        el().size(6.0, 80.0).radius(3.0).fill(white(0.2))
            .child(el().pin(Some(0.0), None, Some(0.0), Some(0.0)).h(80.0 * vol).radius(3.0).fill(white(0.7)))
            .child(knob.absolute(-3.0, 80.0 * (1.0 - vol) - 6.0))
    } else {
        el().size(80.0, 6.0).radius(3.0).fill(white(0.2))
            .child(el().absolute(0.0, 0.0).h(6.0).w(80.0 * vol).radius(3.0).fill(white(0.7)))
            .child(knob.absolute(80.0 * vol - 6.0, -3.0))
    };
    let slider = el().key("mini-volume-slider").role("slider").label("Volume").range_value(vol as f64, 0.0, 1.0).focusable(true)
        .focus_ring(2.0, 2.0, white(0.6)).items_center().justify_center().cursor(rune::Cursor::Pointer)
        .on_step(move |_: &mut Mini, dir, _| send(Cmd::SetVolume((vol + dir as f32 * 0.05).clamp(0.0, 1.0))))
        .on_drag(move |_: &mut Mini, d: &mut Drag, tx: &mut Tx| {
            tx.instant();
            let f = if tall { 1.0 - d.y / d.h } else { d.x / d.w };
            send(Cmd::SetVolume(f.clamp(0.0, 1.0)));
        })
        .child(track);
    let slider = if tall { slider.size(20.0, 80.0) } else { slider.size(80.0, 20.0) };
    let pill = el().key("mini-volume-pop").radius(8.0).fill(oklch(0.2, 0.0, 0.0).alpha(0.95)).items_center().justify_center()
        .on_hover_change(|s: &mut Mini, inside, _| if !inside { s.vol_open = false })
        .child(slider);
    let pill = if tall { pill.px(8.0).py(10.0) } else { pill.px(12.0).py(6.0) };
    if tall { pill.anchored("mini-volume", rune::Side::Top, Align::Center, 4.0) } else { pill.anchored("mini-volume", rune::Side::Right, Align::Center, 4.0) }
}

/// The seek bar: a 4 pt track in a 20 pt hit area, its knob shown under the pointer. The fill
/// follows a drag exactly; arrow keys step five seconds.
fn seek_bar(v: &View) -> El {
    let (pos, dur) = (v.position, v.duration.max(0.0));
    let frac = if dur > 0.0 { (pos / dur).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let knob = el().pin(None, Some(-4.0), Some(-6.0), None).size(12.0, 12.0).radius(6.0).fill(white(1.0)).shadow(1.0, 3.0, theme::black(0.45)).group_opacity(0.0, 1.0);
    col()
        .key("mini-seek")
        .role("slider")
        .label("Position")
        .range_value(pos, 0.0, dur)
        .focusable(true)
        .focus_ring(2.0, 2.0, white(0.6))
        .grow(1.0)
        .min_w(0.0)
        .h(20.0)
        .radius(4.0)
        .justify_center()
        .hover_group()
        .cursor(rune::Cursor::Pointer)
        .on_step(move |_: &mut Mini, dir, _| {
            if dur > 0.0 {
                send(Cmd::SeekFraction(((pos + dir as f64 * 5.0) / dur).clamp(0.0, 1.0) as f32));
            }
        })
        .on_drag(|s: &mut Mini, d: &mut Drag, tx: &mut Tx| {
            s.seek_drag = Some(Instant::now());
            tx.instant();
            send(Cmd::SeekFraction((d.x / d.w).clamp(0.0, 1.0)));
        })
        .child(el().w_full().h(4.0).radius(2.0).fill(white(0.2)).child(el().absolute(0.0, 0.0).h(4.0).w_frac(frac).radius(2.0).fill(white(0.92)).child(knob)))
}

fn close() -> El {
    theme::focusable(el().key("mini-close").role("button").label("Close miniplayer"))
        .size(28.0, 28.0)
        .radius(14.0)
        .fill(theme::black(0.45))
        .hover(theme::black(0.45)) // over the fill: black at 0.7
        .press_scale(0.92)
        .cursor(rune::Cursor::Pointer)
        .items_center()
        .justify_center()
        .on_press(|_: &mut Mini| set_mini(false))
        .child(glyph(G::Close, 14.0, white(0.85)).group_color(white(0.95)))
}

/// Six dots on a dark pill at the top: says the card moves. The whole card is the drag region
/// (`window_drag` on the root), so the grip needs no handler of its own.
fn grip() -> El {
    let dots = || row().gap(2.0).children((0..3).map(|_| el().size(3.0, 3.0).radius(1.5).fill(white(0.7))));
    col().py(4.0).px(8.0).gap(2.0).radius(8.0).fill(theme::black(0.35)).cursor(rune::Cursor::Grab).a11y_hidden().child(dots()).child(dots())
}

/// The bottom-right corner resizes the window: a borderless window has no frame to drag.
fn resize_grip() -> El {
    let dot = |x: f32, y: f32| el().absolute(x, y).size(2.0, 2.0).radius(1.0).fill(white(0.55));
    el().key("mini-resize")
        .pin(None, None, Some(0.0), Some(0.0))
        .size(14.0, 14.0)
        .a11y_hidden()
        .child(dot(9.0, 9.0))
        .child(dot(5.0, 9.0))
        .child(dot(9.0, 5.0))
        .on_drag(|s: &mut Mini, d: &mut Drag, tx: &mut Tx| {
            if matches!(d.phase, Phase::Up) {
                s.grab = None;
                return;
            }
            // The top-left corner stays put, so the pointer's travel is the size's change.
            let (w, h) = *s.grab.get_or_insert(s.size.get());
            tx.instant();
            tx.resize_window_instant(((w + d.dx).clamp(MIN.0, MAX.0), (h + d.dy).clamp(MIN.1, MAX.1)));
        })
}

#[cfg(test)]
mod place_tests {
    use super::back_on_screen;

    #[test]
    fn a_saved_place_off_every_screen_comes_back() {
        let screen = ((0, 0), (1920, 1080));
        // on screen, or 50 px and more of it: stays
        assert_eq!(back_on_screen((100, 100), (372, 196), screen), None);
        assert_eq!(back_on_screen((1800, 900), (372, 196), screen), None);
        // a monitor to the right that's gone: near this one's bottom-right corner
        assert_eq!(back_on_screen((2500, 300), (372, 196), screen), Some((1920 - 372 - 40, 1080 - 196 - 100)));
        // barely peeking in (under 50 px)
        assert_eq!(back_on_screen((-340, 100), (372, 196), screen), Some((1508, 784)));
        // a second screen left of the first
        assert_eq!(back_on_screen((-1500, 200), (372, 196), ((-1920, 0), (1920, 1080))), None);
    }
}
