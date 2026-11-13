//! The full-screen "now playing" player (PLAYER_DESIGN.md, "Sleeve"; React MaximizedPlayer).
//!
//! A full-window layer over the app, the bar included, as the video player is: the cover
//! blurred into the ground under a scrim, the cover itself alone and centred above a bottom bar
//! (title, artist, heart and More, as the player bar's left side | transport and seek |
//! quality, lyrics, volume, exit). The bar's centre and right fade after three seconds without
//! the pointer moving; what is playing stays. The mic button puts the synced lyrics beside the
//! cover. Pressing the player bar's cover opens it, flying that cover up to its place (a
//! `matched` flight: the bar's cover leaves the tree while this one mounts, ui::nowbar);
//! closing flies it back, a quarter faster.
//!
//! fx.rs (off with `SONE_FX=0`) adds the open's overshoot, the pause shrink, the directional
//! track change, the heart pop and the bar's rise.
//!
//! Rune has no OS fullscreen, so "full screen" fills the window, as the video player does.

use crate::controller::{self, Cmd, QItem, View};
use crate::data::{self, Key};
use crate::fx;
use crate::glyph::{G, glyph};
use crate::interact::{self, Anchor, Fav, Say};
use crate::theme;
use crate::ui::{self, caps, t, tnum, white};
use crate::App;
use rune::{Align, Curve, Easing, El, ScrollSpec, Shadow, Spring, SpringPair, Transition, Tx, col, el, oklch, row, text};
use sone_host_shim::Manager;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

const CONTROLS_IDLE: Duration = Duration::from_secs(3);

/// The cover's shared-element id: the bar's cover and this player's carry it.
pub const COVER: &str = "now-cover";
/// The flight back to the bar, about a quarter faster (the bar's cover springs on it).
pub const CLOSE: Spring = Spring::new(0.33, 0.0);

/// Whether the player is up, for the bar, which hands its cover over while it is.
static SHOWN: AtomicBool = AtomicBool::new(false);
/// An MPRIS `Fullscreen` set (§7.2) waiting for the window's next poll: 1 off, 2 on.
static MPRIS_ASK: AtomicU8 = AtomicU8::new(0);

/// Window state for the player, kept in `Nav`.
#[derive(Default)]
pub struct Full {
    pub open: bool,
    /// The lyrics column beside the cover.
    pub lyrics: bool,
    /// The bottom bar is shown; it hides at `hide_at`.
    pub controls: bool,
    pub hide_at: Option<Instant>,
    /// The synced lyric line showing, kept centred by main::lyric_tick.
    pub lyric_line: Option<usize>,
    /// The track the player opened on: its cover, title and ground arrive with the open itself
    /// rather than cross-fading in (see `swap`).
    opened_on: Option<u64>,
    /// A key woke the controls; the next poll redraws.
    dirty: bool,
    /// The track liked from this player and how many times, keying the heart's pop (fx) so it
    /// plays on the like itself, never for a track that was already loved when it showed.
    pop: Option<(u64, u32)>,
    /// What MPRIS was last told (§8.3: pushed once, then on every change).
    told_mpris: Option<bool>,
}

/// The bar shows an empty slot where its cover was while the player is up.
pub fn covers_bar() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

/// From the controller: an MPRIS client set `Fullscreen`.
pub fn request(on: bool) {
    MPRIS_ASK.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}

pub fn open(s: &mut App) {
    let f = &mut s.nav.full;
    f.open = true;
    // (the lyrics column stays as it was left: React's maximizedLyricsAtom outlives the player)
    f.lyric_line = None;
    f.opened_on = controller::now_playing().0;
    f.pop = None;
    f.controls = true;
    f.hide_at = Some(Instant::now() + CONTROLS_IDLE);
    SHOWN.store(true, Ordering::Relaxed);
}

pub fn close(s: &mut App) {
    let f = &mut s.nav.full;
    f.open = false;
    f.lyric_line = None;
    SHOWN.store(false, Ordering::Relaxed);
}

pub fn toggle_lyrics(s: &mut App) {
    let f = &mut s.nav.full;
    f.lyrics = !f.lyrics;
    // The next lyric tick centres the active line at once in the new column.
    f.lyric_line = None;
}

/// Escape: the player is one layer (lyrics are not a layer of their own); it closes.
pub fn on_escape(s: &mut App) -> bool {
    if !s.nav.full.open {
        return false;
    }
    close(s);
    true
}

/// Any key brings the controls back.
pub fn wake(s: &mut App) {
    let f = &mut s.nav.full;
    if f.open {
        f.dirty |= !f.controls;
        f.controls = true;
        f.hide_at = Some(Instant::now() + CONTROLS_IDLE);
    }
}

/// From the window's poll: MPRIS asks, the player closes when nothing (or a video) plays, and
/// the controls hide when the pointer has rested. Returns whether anything the view draws
/// changed, and when to look again.
pub fn tick(s: &mut App) -> (bool, Option<Duration>) {
    let track = controller::now_playing().0;
    let (is_video, _, _) = controller::video_status();
    let mut changed = std::mem::take(&mut s.nav.full.dirty);
    match MPRIS_ASK.swap(0, Ordering::Relaxed) {
        1 if s.nav.full.open => {
            close(s);
            changed = true;
        }
        2 if !s.nav.full.open && track.is_some() && !is_video => {
            open(s);
            changed = true;
        }
        _ => {}
    }
    // It and the video player are never open together; with nothing playing it has nothing
    // to show.
    if s.nav.full.open && (track.is_none() || is_video) {
        close(s);
        changed = true;
    }
    let f = &mut s.nav.full;
    if f.told_mpris != Some(f.open) {
        f.told_mpris = Some(f.open);
        if let Err(e) = tauri_app_lib::commands::playback::update_mpris_fullscreen(crate::host().state::<tauri_app_lib::AppState>(), f.open) {
            log::warn!("[rune] update_mpris_fullscreen: {e}");
        }
    }
    if !f.open {
        return (changed, None);
    }
    // Never hide under a dragged slider.
    if ui::dragging() {
        f.hide_at = Some(Instant::now() + CONTROLS_IDLE);
    }
    if f.hide_at.is_some_and(|at| Instant::now() >= at) {
        f.hide_at = None;
        if f.controls {
            f.controls = false;
            changed = true;
        }
    }
    (changed, f.hide_at.map(|at| at.saturating_duration_since(Instant::now())))
}

fn show_controls(s: &mut App, tx: &mut Tx) {
    let f = &mut s.nav.full;
    // Pointer moves can arrive every frame; re-arm at most twice a second, and otherwise tell
    // Rune nothing it draws changed, so a resting pointer costs no rebuilds (as video_ui).
    let due = f.hide_at.is_none_or(|at| at.saturating_duration_since(Instant::now()) < CONTROLS_IDLE - Duration::from_millis(500));
    if due || !f.controls {
        f.controls = true;
        f.hide_at = Some(Instant::now() + CONTROLS_IDLE);
    } else {
        tx.live_only();
    }
}

/// How a per-track layer (cover picture, title, ground) replaces the last track's: a
/// cross-fade over `secs`. The one the player opened on comes in with the open instead, so
/// the flying cover isn't a bare fill fading to its picture.
fn swap(first: bool, secs: f32) -> Transition {
    let fade = Spring::new(secs, 0.0);
    Transition::opacity().springs(SpringPair::new(if first { Spring::NONE } else { fade }, fade))
}

/// The quality tier, short, as the React player's badge names it (`v.badge` ends with it).
fn tier(v: &View) -> Option<&'static str> {
    let b = v.badge.as_deref()?;
    Some(if b.contains("HI-RES") { "Max" } else if b.ends_with("LOSSLESS") { "High" } else if b.ends_with("HIGH") { "Low" } else { return None })
}

/// The player over the whole window (`win`: its width and height), or None when it's closed.
pub fn overlay(s: &App, v: &View, win: (f32, f32)) -> Option<El> {
    let f = &s.nav.full;
    if !f.open || v.video.is_some() {
        return None;
    }
    let cur = v.current.as_ref()?;
    let (w, h) = win;
    let first = f.opened_on == Some(cur.id);
    let hue = ui::hue(v.track_id);
    let shown = f.controls;

    // Sizes follow the prototype's clamps (sleeve.css): the stage leaves the bar 128 pt. With
    // no text under it the cover may take a little more than the prototype's 62%, but always
    // fits the stage, clear of the bar.
    let (sx, sy) = ((w * 0.04).clamp(24.0, 64.0), (h * 0.06).clamp(24.0, 64.0));
    let vmin = w.min(h);
    let stage_h = h - sy - 128.0;
    let art = if f.lyrics { (vmin * 0.55).min(600.0).min(w * 0.5 - 128.0) } else { (vmin * 0.66).min(760.0).min(w - 96.0) };
    let art = art.min(stage_h - 8.0).max(140.0).floor();

    // ---- ground: black, the cover blurred (drawn 1.18x, as when the blur was live, so the crop
    // is the same), then the scrim. The cover comes blurred from art::ground, once per track:
    // a live 48 pt blur re-ran over the whole window on every frame the player drew.
    let blurred = el().key(format!("full-bg-{}", cur.id)).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).transition(swap(first, 0.7));
    let blurred = match crate::art::cover(v.cover.as_deref(), 320.0).and_then(|u| crate::art::ground(&u)) {
        Some(px) => blurred.child(el().absolute(-w * 0.09, -h * 0.09).size(w * 1.18, h * 1.18).pixels_fit(&px, rune::SurfaceFit::Fill)),
        // No cover: the Afterglow disc, blurred live (no picture to blur ahead).
        None => {
            let d = w.max(h) * 1.15;
            blurred.blur(48.0).child(ui::art(hue, d, cur.id, 0.0).absolute((w - d) / 2.0, (h - d) / 2.0))
        }
    };
    // A bright sleeve gets up to 0.22 more scrim, so white text keeps its contrast on it.
    let mean_l = crate::art::cover_color(v.cover.as_deref()).map_or(0.3, |c| c.mean_l);
    let scrim = 0.6 + ((mean_l - 0.45) * 0.5).clamp(0.0, 0.22);
    let ground = el().key("full-ground").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(theme::black(1.0)).a11y_hidden()
        .child(blurred)
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(theme::black(scrim)));

    // ---- the cover: radius 10, a layered shadow, a hairline that keeps a black or white
    // sleeve's edge. The picture is fetched at the largest size the player draws, so opening
    // the lyrics or resizing the window doesn't decode it again.
    const COVER_PT: f32 = 760.0;
    let mut picture = ui::tile(hue, art, cur.id, 10.0);
    if let Some(px) = crate::art::cover(v.cover.as_deref(), COVER_PT).and_then(|u| crate::art::pixels(&u, COVER_PT)) {
        picture = picture.child(el().absolute(0.0, 0.0).size(art, art).radius(10.0).pixels_fit(&px, rune::SurfaceFit::Fill));
    }
    let picture = picture.key(format!("full-art-{}", cur.id)).absolute(0.0, 0.0);
    let hairline = el().absolute(0.0, 0.0).size(art, art).radius(10.0).border(1.0, white(0.1));
    let lifted = |e: El| e
        .shadow(2.0, 4.0, theme::black(0.22))
        .shadow(12.0, 28.0, theme::black(0.32))
        .box_shadow(Shadow { y: 32.0, blur: 72.0, spread: -12.0, color: theme::black(0.5), ..Shadow::default() });
    let cover = el().key("full-cover").matched(COVER).size(art, art).radius(10.0).motion(fx::open_spring()).a11y_hidden();
    let cover = if fx::on() {
        // The whole sleeve is per track, so a track change can push it aside; it settles lower
        // with a softer shadow while paused.
        let paused = !v.playing;
        let mut sleeve = el().key(format!("full-sleeve-{}", cur.id)).size(art, art).radius(10.0).fill(oklch(0.2, 0.02, hue))
            .transition(fx::cover_change(first))
            .scale(if paused { fx::PAUSED_SCALE } else { 1.0 }).scale_motion(if paused { fx::PAUSE } else { fx::RESUME });
        sleeve = if paused {
            sleeve.shadow(1.0, 3.0, theme::black(0.18))
                .shadow(8.0, 18.0, theme::black(0.24))
                .box_shadow(Shadow { y: 18.0, blur: 44.0, spread: -12.0, color: theme::black(0.36), ..Shadow::default() })
        } else {
            lifted(sleeve)
        };
        cover.child(sleeve.child(picture).child(hairline))
    } else {
        lifted(cover.fill(oklch(0.2, 0.02, hue))).child(picture.transition(swap(first, 0.32))).child(hairline)
    };

    // ---- stage: the cover alone and centred, or cover left and lyrics right
    let mut stage = row().key("full-stage").pin(Some(sx), Some(sy), Some(sx), Some(128.0))
        .child(col().key("full-left").grow(1.0).h_full().items_center().justify_center().child(cover));
    if f.lyrics {
        let half = (w - 2.0 * sx) / 2.0;
        let pad = (w * 0.02).clamp(8.0, 40.0);
        stage = stage.child(col().key("full-lyrics-col").w(half).h_full().pl(pad)
            .transition(Transition::offset(24.0, 0.0).combined(Transition::opacity()).springs(SpringPair::new(Spring::new(0.46, 0.0), Spring::new(0.2, 0.0))))
            .enter_delay(0.1)
            .child(lyrics(s, cur.id, w, stage_h)));
    }

    let layer = el().key("full-layer").role("dialog").label("Now playing").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        // ground and text fade in with the flight; out a quarter faster
        .transition(Transition::opacity().springs(SpringPair::new(Spring::new(0.35, 0.0), CLOSE)))
        .modal(false, |s: &mut App| close(s))
        .cursor(if shown { rune::Cursor::Default } else { rune::Cursor::Hidden })
        .on_pointer_move_local(|s: &mut App, _, tx| show_controls(s, tx));
    Some(layer.child(ground).child(stage).child(bar(s, v, cur, w, first, shown)))
}

/// The bottom bar over a black gradient: what is playing, with its heart and More (the player
/// bar's left side, without the thumbnail: the big cover is the artwork) | transport and seek |
/// quality, lyrics, volume, exit. The centre and right fade out (240 ms) when the pointer rests
/// and back (150 ms) when it moves; the left stays, so what is playing always shows.
fn bar(s: &App, v: &View, cur: &QItem, w: f32, first: bool, shown: bool) -> El {
    let f = &s.nav.full;
    // Each side keeps at least this much as the window narrows; the volume slider and the
    // bit-perfect caption give way first.
    let wide = w >= 1260.0;
    let side = if wide { 300.0 } else if w >= 780.0 { 210.0 } else { 124.0 };

    let loved = interact::is_fav(Fav::Track, &cur.id.to_string());
    let (fid, fl) = (cur.id.to_string(), cur.title.clone());
    let mut mark = glyph(if loved { G::HeartFilled } else { G::Heart }, 16.0, if loved { theme::accent() } else { white(0.45) });
    // (fx: a like pops the heart; an unlike just swaps it)
    if let Some((_, n)) = f.pop.filter(|&(id, _)| loved && id == cur.id) {
        mark = mark.key(format!("full-love-pop-{n}")).sequence(rune::Sequence::new(fx::HEART_POP));
    }
    let id = cur.id;
    let heart = theme::focusable(el().key("full-love").role("button").label(if loved { "Unlike" } else { "Like" }))
        .size(28.0, 28.0).radius(14.0).items_center().justify_center()
        .hover(white(0.06)).press_scale(0.9).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            if fx::on() && !interact::is_fav(Fav::Track, &fid) {
                let f = &mut s.nav.full;
                f.pop = Some((id, f.pop.map_or(0, |(_, n)| n) + 1));
            }
            interact::toggle_fav(Fav::Track, fid.clone(), fl.clone(), Say::Nothing)
        })
        .child(mark);
    let track = interact::json_of_track(cur.id, &cur.title, &cur.artist);
    let more = ui::icon_button::<App>("full-more", "More options", G::More, 16.0, white(0.45))
        .on_press(move |s: &mut App| interact::open_track(s, track.clone(), None, Anchor::Below("full-more".into())));
    // A new track's words fade in over the last one's place.
    let words = col().key(format!("full-words-{}", cur.id)).shrink().min_w(0.0).gap(2.0)
        .transition(if fx::on() { fx::words_change(first) } else { Transition::opacity().springs(SpringPair::new(if first { Spring::NONE } else { Spring::new(0.26, 0.0) }, Spring::NONE)) })
        .child({
            let title = text(cur.title.clone()).font(t(13.0, 600.0)).color(white(0.95)).truncate().role("heading").label(cur.title.clone());
            match crate::content::badges(cur.explicit, cur.ai) {
                Some(b) => row().gap(6.0).min_w(0.0).items_center().child(title.shrink()).child(b),
                None => title,
            }
        })
        .child(text(cur.artist.clone()).font(t(12.0, 400.0)).color(white(0.6)).truncate());
    let left = row().grow(1.0).min_w(side).gap(12.0).pl(8.0).items_center()
        .child(words)
        .child(row().gap(2.0).items_center().child(heart).child(more));

    let time = |secs: f64| text(ui::fmt_time(secs)).font(tnum(t(12.0, 500.0))).color(white(0.6)).w(52.0);
    let middle = col().w(560.0).shrink().min_w(300.0).gap(4.0).items_center()
        .child(ui::transport::<App>(v, "full", 48.0))
        .child(row().w_full().gap(12.0).items_center()
            .child(time(v.position).align(Align::End))
            .child(col().grow(1.0).child(ui::seek::<App>(v, "full-seek", 10000.0, 4.0).w_full()))
            .child(time(v.duration)));

    let mut right = row().grow(1.0).min_w(side).gap(4.0).items_center().justify_end();
    if let Some(q) = tier(v).filter(|_| w >= 780.0) {
        let detail = v.quality.as_deref().map_or(String::new(), |d| format!(", {d}"));
        // (QualityBadge: a press shows the signal path, as on the player bar)
        right = right.child(theme::focusable(row().key("full-quality").role("button").label(format!("Quality: {q}{detail}"))).h(22.0).px(8.0).mr(6.0).radius(11.0).border(1.0, white(0.2)).items_center()
            .hover(white(0.08)).cursor(rune::Cursor::Pointer).on_press(crate::signal_path::open)
            .child(text(q.to_uppercase()).font(t(11.0, 600.0).tracking(1.2)).color(white(0.72)).a11y_hidden()));
    }
    let mut mic = ui::icon_button::<App>("full-lyrics-btn", "Lyrics", G::Mic, 18.0, if f.lyrics { theme::accent() } else { white(0.45) })
        .checked(f.lyrics)
        .on_press(toggle_lyrics);
    if f.lyrics {
        mic = mic.child(el().absolute(16.0, 30.0).size(4.0, 4.0).radius(2.0).fill(theme::accent()));
    }
    right = right.child(mic);
    right = if v.bit_perfect {
        let locked = ui::unavailable(ui::icon_button::<App>("full-volume", "Volume disabled in bit-perfect mode", G::Speaker, 16.0, white(0.35)));
        let r = right.child(locked);
        if wide { r.child(caps("Bit-perfect", 0.5).mr(6.0).a11y_hidden()) } else { r }
    } else {
        right.child(ui::volume::<App>(v, "full-volume", if wide { 88.0 } else { 48.0 }).mr(6.0))
    };
    right = right.child(ui::icon_button::<App>("full-exit", "Exit full screen", G::Minimize, 18.0, white(0.75)).on_press(close));

    let faded = |e: El| {
        let fade = if shown { Curve::tween(0.15, Easing::cubic(0.0, 0.0, 0.58, 1.0)) } else { Curve::tween(0.24, Easing::cubic(0.42, 0.0, 1.0, 1.0)) };
        let e = e.opacity(if shown { 1.0 } else { 0.0 }).opacity_motion(fade);
        if fx::on() { fx::bar_rise(e, shown) } else { e }
    };
    // (the gradient stays with the left side, under what is playing)
    let inner = row().w_full().gap(24.0).items_center().pt(64.0).px(24.0).pb(18.0)
        .linear_gradient(0.0, [(0.0, theme::black(0.72)), (0.35, theme::black(0.5)), (0.7, theme::black(0.18)), (1.0, theme::black(0.0))])
        .child(left).child(faded(middle)).child(faded(right));
    col().key("full-bar").pin(Some(0.0), None, Some(0.0), Some(0.0))
        .transition(Transition::offset(0.0, 16.0).combined(Transition::opacity()).springs(SpringPair::new(Spring::new(0.31, 0.0), Spring::new(0.18, 0.0))))
        .enter_delay(0.13)
        .child(inner)
}

/// The lyrics column (`h` tall): synced lines kept centred, the active one white, past ones
/// 0.3, coming ones 0.45, a press seeking to a line; the plain text when there's no timing;
/// otherwise a quiet "No synced lyrics" state.
fn lyrics(s: &App, id: u64, w: f32, h: f32) -> El {
    let l = match data::want(Key::Lyrics(id)) {
        data::Load::Ready(l) => l,
        data::Load::Failed(_) => return no_lyrics(),
        data::Load::Loading => return col().key("full-lyrics-loading").busy(true).grow(1.0).justify_center().child(crate::views::loading()),
    };
    let provider = l["lyricsProvider"].as_str().filter(|p| !p.is_empty()).map(|p| text(format!("Lyrics provided by {p}")).font(t(11.0, 400.0)).color(white(0.5)).pt(12.0));
    let Some(lines) = crate::views::synced_lyrics(id) else {
        let plain = l["lyrics"].as_str().unwrap_or("").trim();
        if plain.is_empty() {
            return no_lyrics();
        }
        let fs = (w * 0.006 + 12.0).clamp(18.0, 24.0);
        let mut body = col().w_full().min_h(h).justify_center().py(16.0)
            .child(caps("No synced lyrics for this track", 0.5).mb(20.0));
        for line in plain.lines() {
            body = body.child(text(line.to_string()).font(t(fs, 500.0).lh(fs * 1.6)).color(white(0.82)).max_w(fs * 19.0).wrap());
        }
        let viewport = col().grow(1.0).min_h(0.0).w_full().role("document").label("Lyrics (not synced)")
            .scroll(ScrollSpec { stick_bottom: false }).scroll_edge_fade(24.0).child(body);
        return ui::smooth_wheel::<App>(viewport, "full-plain-lyrics".into());
    };
    let fs = (w * 0.015 + 12.0).clamp(24.0, 40.0);
    let active = s.nav.full.lyric_line;
    // Room above the first line and under the last, so either can sit at the centre.
    let mut list = col().w_full().max_w(680.0).child(el().h(h * 0.5 - fs));
    for (i, (at, line)) in lines.iter().enumerate() {
        let color = match active {
            Some(a) if a == i => white(0.95),
            Some(a) if i < a => white(0.3),
            _ => white(0.45),
        };
        let at = *at;
        let gap = line.trim() == "♪";
        let mut item = col().key(format!("fly-{i}")).role("button")
            .label(if gap { format!("Instrumental break at {}", ui::fmt_time(at)) } else { line.clone() })
            .w_full().py(fs * if gap { 0.55 } else { 0.24 }).hover_group().cursor(rune::Cursor::Pointer)
            .on_press(move |_: &mut App| controller::send(Cmd::SeekTo(at)));
        let hover = |e: El| if active == Some(i) { e } else { e.group_color(white(0.7)) };
        item = if gap {
            let dot = || el().size(fs * 0.3, fs * 0.3).radius(fs * 0.15).fill(color);
            item.child(row().gap(fs * 0.3).a11y_hidden().child(dot()).child(dot()).child(dot()))
        } else {
            item.child(hover(text(line.clone()).font(t(fs, 600.0).lh(fs * 1.22).tracking(-0.012 * fs)).color(color).w_full().wrap().a11y_hidden()))
        };
        list = list.child(item);
    }
    list = list.child(el().h(h * 0.5 - fs));
    let viewport = col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).scroll_edge_fade(h * 0.16).child(list);
    let mut c = col().w_full().h_full().child(ui::smooth_wheel::<App>(viewport, "full-lyrics".into()));
    if let Some(p) = provider {
        c = c.child(p);
    }
    c
}

fn no_lyrics() -> El {
    col().grow(1.0).w_full().items_center().justify_center().gap(6.0)
        .child(el().size(52.0, 52.0).radius(26.0).mb(8.0).fill(white(0.07)).items_center().justify_center().a11y_hidden().child(glyph(G::Mic, 22.0, white(0.5))))
        .child(text("No synced lyrics for this track").font(t(18.0, 600.0)).color(white(0.95)))
        .child(text("They will appear here when they are available.").font(t(14.0, 400.0)).color(white(0.6)))
}
