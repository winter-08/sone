//! The music video player and video cards (React VideoPlayer, MediaCard `aspect="video"`).
//!
//! A video plays as the current queue item (controller.rs, video.rs). The player is a
//! full-window layer over everything, the bar included: the picture, letterboxed on black, with
//! a top bar (title, artist, minimise) and a bottom bar (seek, favourite, transport, volume,
//! quality, fullscreen) that fade out after three seconds without the pointer moving, as React's
//! do. Minimising keeps the video playing behind the app; the player bar then shows it, and its
//! picture opens the player again. Escape closes the video (React useEscapeDismiss).

use crate::controller::{self, Cmd, View};
use crate::glyph::{G, glyph};
use crate::interact::{self, Fav, Say};
use crate::theme;
use crate::ui::{self, t, white};
use crate::video;
use crate::App;
use rune::{El, Tx, col, el, row, spacer, text};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const CONTROLS_IDLE: Duration = Duration::from_secs(3);

/// A video as a queue item (itemHelpers videoToTrack): `itemType: "video"` is what makes the
/// controller play it through video.rs.
pub fn video_track(d: &Value) -> Value {
    let artist = if d["artist"].is_object() { d["artist"].clone() } else { d["artists"][0].clone() };
    json!({
        "id": d["id"], "title": d["title"], "itemType": "video", "imageId": image_id(d),
        "duration": d["duration"], "explicit": d["explicit"], "artist": artist, "artists": d["artists"],
    })
}

/// A video's picture id: `imageId` in most responses, `image` on artist pages (v2 pages API).
pub fn image_id(d: &Value) -> Option<&str> {
    [&d["imageId"], &d["image"]].into_iter().filter_map(|v| v.as_str()).find(|s| !s.is_empty())
}

fn image_url(id: &str, pt: f32) -> Option<String> {
    crate::art::cover(Some(id), pt)
}

/// A 16:9 card; pressing it plays the video with the rest of `list` queued after it (React
/// SearchView handlePlayVideo: videos play through the queue so prev/next walk the grid).
/// With `source` null (Home's cards: useMediaPlay) the video plays alone instead (playSingle).
pub fn video_card(key: String, d: &Value, list: &[Value], idx: usize, source: Value) -> El {
    let tracks: Vec<Value> = list.iter().map(video_track).collect();
    let play = move |_: &mut App| {
        if source.is_null() {
            controller::send(Cmd::PlaySingle(tracks[idx].clone()));
        } else {
            controller::send(Cmd::PlayFrom { tracks: tracks.clone(), index: idx, source: source.clone(), album_mode: false });
        }
    };
    video_card_with(key, d, play)
}

/// A 16:9 card pressing which runs `play`. Right-click opens the video's menu at the pointer
/// (React's MediaContextMenu on a video: play next, queue, add to playlist, share); an
/// explicit video is badged as MediaCard does.
pub fn video_card_with(key: String, d: &Value, play: impl Fn(&mut App) + Clone + 'static) -> El {
    const W: f32 = 220.0;
    const H: f32 = 124.0;
    let id = d["id"].as_u64().unwrap_or(0);
    let title = d["title"].as_str().unwrap_or("").to_string();
    let artist = d["artist"]["name"].as_str().or(d["artists"][0]["name"].as_str()).unwrap_or("").to_string();
    let h = ui::hue(Some(id));
    let mut art = el().size(W, H).radius(10.0).clip_rounded(10.0).fill(rune::oklch(0.22, 0.07, h)).a11y_hidden();
    if let Some(px) = image_id(d).and_then(|i| image_url(i, W)).and_then(|u| crate::art::pixels(&u, W)) {
        art = art.child(el().absolute(0.0, 0.0).size(W, H).pixels_fit(&px, rune::SurfaceFit::Fill));
    }
    let menu_item = video_track(d);
    let on = interact::is_fav(Fav::Video, &id.to_string());
    let (fid, fl) = (id.to_string(), title.clone());
    let play2 = play.clone();
    art = art
        .child(el().absolute(10.0, H - 50.0).key(format!("{key}-play")).role("button").label(format!("Play {title}")).size(40.0, 40.0).radius(20.0).fill(theme::accent())
            .items_center().justify_center().group_opacity(0.0, 1.0).press_scale(0.94).cursor(rune::Cursor::Pointer)
            .on_press(play2)
            .child(glyph(G::Play, 20.0, theme::ink())))
        .child(el().absolute(W - 42.0, H - 42.0).key(format!("{key}-fav")).role("button").label(if on { format!("Remove {title} from favorites") } else { format!("Add {title} to favorites") }).size(32.0, 32.0).radius(16.0).fill(theme::black(0.5))
            .items_center().justify_center().group_opacity(0.0, 1.0).cursor(rune::Cursor::Pointer)
            .on_press(move |_: &mut App| interact::toggle_fav(Fav::Video, fid.clone(), fl.clone(), Say::Nothing))
            .child(glyph(if on { G::HeartFilled } else { G::Heart }, 16.0, if on { theme::accent() } else { white(0.95) })));
    let heading_play = play.clone();
    let heading = text(title.clone()).font(t(13.0, 650.0)).color(white(0.95)).truncate().role("heading").label(title).focusable(true).no_focus_ring().on_press(heading_play);
    let head = match crate::content::badges(d["explicit"].as_bool() == Some(true), false) {
        Some(b) => row().w_full().min_w(0.0).gap(6.0).items_center().child(heading.shrink()).child(b),
        None => heading,
    };
    col().key(key).role("presentation").w(W + 16.0).gap(6.0).padding(8.0).radius(12.0)
        .hover(white(0.06)).hover_group().focus_within_ring(2.0, -2.0, white(0.6)).cursor(rune::Cursor::Pointer)
        .on_press(play)
        .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, menu_item.clone(), None, interact::Anchor::Point(p.x, p.y)))
        .child(art)
        .child(head)
        .child(text(artist).font(t(11.0, 400.0)).color(white(0.55)).truncate())
}

/// Cards for every video in `items` (bare video objects or `{type: VIDEO, item}` wrappers).
pub fn video_cards(prefix: &str, items: &[Value], source: Value) -> Vec<El> {
    let list: Vec<Value> = items.iter().map(|it| crate::data::data(it).clone()).collect();
    list.iter().enumerate().map(|(i, d)| video_card(format!("{prefix}-{i}"), d, &list, i, source.clone())).collect()
}

/// Window state for the player: open over the app or minimised to the bar, controls shown.
#[derive(Default)]
pub struct Player {
    pub expanded: bool,
    pub seen_session: u64,
    pub hide_at: Option<Instant>,
    pub controls: bool,
    pub quality_menu: bool,
    /// The window is full screen for the video (the OS's, `Tx::toggle_full_screen`, as React's
    /// `setFullscreen`). Leaving it puts the window back as it was, maximised or not.
    pub fullscreen: bool,
    /// Full screen was asked for while the video was still starting: it applies when it opens.
    pub full_pending: bool,
}

/// Takes the window full screen for the video, or back. Only ever toggled from here, so the
/// flag and the window agree.
pub fn set_fullscreen(p: &mut Player, tx: &mut Tx, on: bool) {
    if p.fullscreen != on {
        p.fullscreen = on;
        tx.toggle_full_screen();
    }
}

/// The bar's and the drawer's Fullscreen for a video: the player, full screen. A closed (or
/// restored) video starts again first, and goes full screen as it opens.
pub fn open_fullscreen(s: &mut App, tx: &mut Tx, open: bool) {
    let p = &mut s.nav.video;
    if open {
        p.expanded = true;
        set_fullscreen(p, tx, true);
    } else {
        p.full_pending = true;
        controller::send(Cmd::TogglePlay);
    }
}

/// From the window's poll: a newly started video opens the player; the controls hide when the
/// pointer has rested; a video that closed takes full screen with it. Returns whether anything
/// the view draws changed, and when to look again.
pub fn tick(s: &mut App, tx: &mut Tx) -> (bool, Option<Duration>) {
    let (is_video, session, open) = controller::video_status();
    // (a menu open over the player keeps its controls up, as the quality menu does)
    let menu = s.nav.ctx.is_some();
    let p = &mut s.nav.video;
    let mut changed = false;
    // No video, or it was closed (it ended with nothing after it, or Escape): no player, and
    // the window back as it was.
    if !is_video || !open {
        if p.expanded {
            p.expanded = false;
            p.quality_menu = false;
            changed = true;
        }
        if p.fullscreen {
            set_fullscreen(p, tx, false);
            changed = true;
        }
        if !is_video {
            p.full_pending = false;
        }
        return (changed, None);
    }
    if session != p.seen_session {
        p.seen_session = session;
        p.expanded = true;
        if std::mem::take(&mut p.full_pending) {
            set_fullscreen(p, tx, true);
        }
        p.controls = true;
        p.hide_at = Some(Instant::now() + CONTROLS_IDLE);
        changed = true;
    }
    if !p.expanded {
        return (changed, None);
    }
    match p.hide_at {
        Some(at) if p.quality_menu || menu => p.hide_at = Some(at.max(Instant::now() + CONTROLS_IDLE)),
        Some(at) if Instant::now() >= at => {
            p.hide_at = None;
            if p.controls {
                p.controls = false;
                changed = true;
            }
        }
        _ => {}
    }
    (changed, p.hide_at.map(|at| at.saturating_duration_since(Instant::now())))
}

fn show_controls(s: &mut App, tx: &mut Tx) {
    let p = &mut s.nav.video;
    // Pointer moves can arrive every frame; re-arm at most twice a second, and otherwise tell
    // Rune nothing it draws changed, so a resting pointer costs no rebuilds.
    let due = p.hide_at.is_none_or(|at| at.saturating_duration_since(Instant::now()) < CONTROLS_IDLE - Duration::from_millis(500));
    if due || !p.controls {
        p.controls = true;
        p.hide_at = Some(Instant::now() + CONTROLS_IDLE);
    } else {
        tx.live_only();
    }
}

pub fn minimize(s: &mut App, tx: &mut Tx) {
    let p = &mut s.nav.video;
    p.expanded = false;
    p.quality_menu = false;
    set_fullscreen(p, tx, false);
}

/// Escape and the close path: stop the video and dismiss the player (React closeVideo).
pub fn close(s: &mut App) {
    controller::send(Cmd::CloseVideo);
    let p = &mut s.nav.video;
    p.expanded = false;
    p.quality_menu = false;
}

/// Escape while the player is open: the quality menu first, then fullscreen, then the video.
pub fn on_escape(s: &mut App, tx: &mut Tx) -> bool {
    if !s.nav.video.expanded {
        return false;
    }
    let p = &mut s.nav.video;
    if p.quality_menu {
        p.quality_menu = false;
    } else if p.fullscreen {
        set_fullscreen(p, tx, false);
    } else {
        close(s);
    }
    true
}

fn btn(key: &str, label: &str, g: G, size: f32, c: rune::Color) -> El {
    ui::icon_button::<App>(key, label, g, size, c)
}

pub fn overlay(s: &App, v: &View) -> Option<El> {
    let info = v.video.as_ref()?;
    let p = &s.nav.video;
    if !p.expanded {
        return None;
    }
    let state = video::state();
    let shown = p.controls || p.quality_menu || s.nav.ctx.is_some() || !matches!(state, Some(video::State::Playing));
    let faded = |e: El| e.opacity(if shown { 1.0 } else { 0.0 }).opacity_motion(rune::Spring::new(0.3, 0.0));

    let mut layer = el().key("video-layer").role("dialog").label(info.title.clone()).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(theme::black(1.0))
        // Escape with a control focused comes here rather than to the app's key handler: it
        // steps back the same way (quality menu, full screen, then the video).
        .modal_tx(false, |s: &mut App, tx: &mut Tx| {
            on_escape(s, tx);
        })
        .cursor(if shown { rune::Cursor::Default } else { rune::Cursor::Hidden })
        .on_pointer_move_local(|s: &mut App, _, tx| show_controls(s, tx));

    // The poster until the first frame (React's <video poster>), then the picture itself.
    if video::size().is_none() {
        if let Some(px) = info.image.as_deref().and_then(|i| image_url(i, 1280.0)).and_then(|u| crate::art::pixels(&u, 1280.0)) {
            layer = layer.child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).pixels_fit(&px, rune::SurfaceFit::Fit).a11y_hidden());
        }
    }
    layer = layer.child(el().key("video-picture").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).a11y_hidden()
        .native_surface(video::surface(), rune::SurfaceFit::Fit)
        .on_press(|_: &mut App| controller::send(Cmd::TogglePlay)));

    // Loading pulse, or the failure line.
    match &state {
        Some(video::State::Failed(_)) => {
            layer = layer.child(row().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).items_center().justify_center()
                .child(text("Unable to play this video").font(t(14.0, 500.0)).color(white(0.6))));
        }
        Some(video::State::Loading) | None => {
            static PULSE: std::sync::OnceLock<Vec<rune::Beat>> = std::sync::OnceLock::new();
            let beats = PULSE.get_or_init(|| (0..=3600).map(|i| rune::Beat::new(i as f64 * 0.5).opacity(if i % 2 == 0 { 0.35 } else { 1.0 })).collect());
            layer = layer.child(row().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).items_center().justify_center().a11y_hidden()
                .child(el().key("video-loading").sequence(rune::Sequence::new(beats)).child(glyph(G::Loader, 40.0, white(0.9)))));
        }
        _ => {}
    }

    // Top: title, explicit badge, artist; minimise.
    let mut title_row = row().gap(8.0).items_center().min_w(0.0)
        .child(text(info.title.clone()).font(t(17.0, 700.0)).color(white(0.97)).truncate().role("heading"));
    if info.explicit {
        title_row = title_row.child(crate::content::explicit_badge());
    }
    let top = row().key("video-top").pin(Some(0.0), Some(0.0), Some(0.0), None).px(24.0).pt(20.0).pb(40.0).items_start()
        .linear_gradient(std::f32::consts::FRAC_PI_2 * 2.0, [(0.0, theme::black(0.6)), (1.0, theme::black(0.0))])
        .child(col().shrink().min_w(0.0).gap(2.0).child(title_row)
            .child(text(info.artist.clone()).font(t(13.0, 400.0)).color(white(0.6)).truncate()))
        .child(spacer())
        .child(btn("video-minimize", "Minimize video", G::ChevronDown, 22.0, white(0.7)).on_press_tx(|s: &mut App, tx: &mut Tx| minimize(s, tx)));

    // Bottom: seek with times; favourite | transport | volume, quality, fullscreen.
    let fav = interact::is_fav(Fav::Video, &info.id.to_string());
    let (fid, fl) = (info.id.to_string(), info.title.clone());
    let quality_label = match v.video_quality.as_str() { "MEDIUM" => "Medium", "LOW" => "Low", _ => "High" };
    let quality = theme::focusable(row().key("video-quality").role("button").label("Quality")).h(32.0).px(8.0).gap(6.0).radius(8.0).items_center()
        .hover(white(0.08)).cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| s.nav.video.quality_menu = !s.nav.video.quality_menu)
        .child(glyph(G::Settings, 16.0, if p.quality_menu { theme::accent() } else { white(0.6) }))
        .child(text(quality_label).font(ui::tnum(t(13.0, 500.0))).color(white(0.7)).a11y_hidden());
    let seek = row().w_full().gap(12.0).items_center()
        .child(text(ui::fmt_time(v.position)).font(ui::tnum(t(11.0, 500.0))).color(white(0.6)).w(44.0).align(rune::Align::End))
        .child(col().grow(1.0).child(ui::seek::<App>(v, "video-seek", 10000.0, 4.0).w_full()))
        // (a press shows the time left, as React's VideoScrubber)
        .child(ui::duration_label::<App>(v, "video-duration", ui::tnum(t(11.0, 500.0)), 44.0));
    let controls = row().w_full().mt(14.0).items_center()
        .child(row().w(260.0).gap(4.0).items_center()
            .child(btn("video-fav", if fav { "Remove from favorites" } else { "Add to favorites" }, if fav { G::HeartFilled } else { G::Heart }, 16.0, if fav { theme::accent() } else { white(0.6) })
                .on_press(move |_: &mut App| interact::toggle_fav(Fav::Video, fid.clone(), fl.clone(), Say::Nothing)))
            // VideoPlayer's More options: the track menu for the video (Play next, playlists,
            // Loved videos, Share)
            .children(v.current.as_ref().map(|c| {
                let item = (*c.track).clone();
                btn("video-more", "More options", G::More, 16.0, white(0.6))
                    .on_press(move |s: &mut App| interact::open_track(s, item.clone(), None, interact::Anchor::Below("video-more".into())))
            })))
        .child(spacer())
        .child(ui::transport::<App>(v, "video", 36.0))
        .child(spacer())
        .child(row().w(260.0).gap(8.0).items_center().justify_end()
            .child(ui::volume::<App>(v, "video-volume", 100.0))
            .child(quality)
            .child(btn("video-fullscreen", if p.fullscreen { "Exit fullscreen" } else { "Fullscreen" }, if p.fullscreen { G::Minimize } else { G::Maximize }, 16.0, white(0.6))
                .on_press_tx(|s: &mut App, tx: &mut Tx| {
                    let on = !s.nav.video.fullscreen;
                    set_fullscreen(&mut s.nav.video, tx, on);
                })));
    let bottom = col().key("video-bottom").pin(Some(0.0), None, Some(0.0), Some(0.0)).px(24.0).pb(20.0).pt(48.0)
        .linear_gradient(0.0, [(0.0, theme::black(0.7)), (1.0, theme::black(0.0))])
        .child(seek)
        .child(controls);
    layer = layer.child(faded(top)).child(faded(bottom));

    if p.quality_menu {
        let mut menu = theme::overlay_surface(col().key("video-quality-menu").role("menu").label("Quality").pin(None, None, Some(24.0), Some(76.0)).w(140.0).padding(4.0), theme::Overlay::Menu).focus_group();
        for (q, label) in [("HIGH", "High"), ("MEDIUM", "Medium"), ("LOW", "Low")] {
            let on = v.video_quality == q;
            menu = menu.child(theme::focusable_row(row().key(format!("video-q-{q}")).role("menuitemradio").label(label)).checked(on).h(32.0).px(12.0).radius(theme::inset_radius(4.0)).items_center()
                .hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    s.nav.video.quality_menu = false;
                    controller::send(Cmd::SetVideoQuality(q.to_string()));
                })
                .child(text(label).font(t(13.0, 500.0)).color(if on { theme::accent() } else { white(0.8) }).a11y_hidden()));
        }
        layer = layer.child(menu);
    }
    Some(layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_page_videos_have_pictures() {
        // Search and favourites say `imageId`; the artist page (v2 pages API) says `image`.
        assert_eq!(image_id(&json!({"imageId": "a-b", "image": "x"})), Some("a-b"));
        assert_eq!(image_id(&json!({"imageId": null, "image": "c-d"})), Some("c-d"));
        assert_eq!(video_track(&json!({"id": 1, "image": "c-d"}))["imageId"], "c-d");
        assert_eq!(image_id(&json!({"imageId": ""})), None);
    }
}
