//! Afterglow, from ~/dev/music-player: hue-tinted darks, white at fixed alphas, generated ring
//! art, round icon buttons, the now-playing bar and the track row. Rebound here to SONE's
//! playback controller instead of music-player's engine.
//!
//! Every control has a role and an accessible name; the verifier drives and inventories the UI
//! through AT-SPI, so an unnamed control is a defect.

use crate::controller::{Cmd, QItem, Repeat, View, send};
use crate::glyph::{G, glyph};
use crate::theme;
use rune::{Align, Color, El, Spring, TextStyle, col, el, oklch, oklcha, row, spacer, text};

pub fn white(a: f32) -> Color {
    oklcha(0.985, 0.0, 0.0, a)
}
pub fn base(h: f32) -> Color {
    theme::surface(0.155, h)
}
pub fn tnum(s: TextStyle) -> TextStyle {
    s.features("'tnum' 1")
}
pub fn t(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
}
pub fn caps(s: &str, a: f32) -> El {
    text(s.to_uppercase()).font(t(11.0, 600.0).tracking(1.2)).color(white(a))
}

/// A stable hue per track, so the bar and art tint follow what's playing.
pub fn hue(track_id: Option<u64>) -> f32 {
    match track_id {
        Some(id) => ((id.wrapping_mul(2654435761) % 360) as f32 + 200.0) % 360.0,
        None => 270.0,
    }
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// The Afterglow disc: concentric rings in one hue family and an off-centre sun.
pub fn art(hue: f32, size: f32, seed: u64, spin: f32) -> El {
    let k = size / 340.0;
    let mut disc = el()
        .size(size, size)
        .radius(size * 0.5)
        .clip_rounded(size * 0.5)
        .fill(oklch(0.3, 0.09, hue))
        .rotate(spin)
        // (no rotate_motion: the disc never spins here, and a running spring keeps redrawing)
        .a11y_hidden();
    for i in 0..6 {
        let d = size * (0.92 - i as f32 * 0.14);
        let off = (size - d) / 2.0 + i as f32 * 9.0 * k;
        disc = disc.child(el().absolute(off, (size - d) / 2.0).size(d, d).radius(d / 2.0).fill(oklch(0.36 + i as f32 * 0.075, 0.1 + i as f32 * 0.012, hue + i as f32 * 14.0)));
    }
    let a = (seed % 97) as f32 * 1.9 + 0.6;
    let sun = 54.0 * k;
    let (x, y) = (size / 2.0 + a.cos() * size * 0.3 - sun / 2.0, size / 2.0 + a.sin() * size * 0.3 - sun / 2.0);
    disc.child(el().absolute(x, y).size(sun, sun).radius(sun / 2.0).fill(oklch(0.92, 0.07, hue + 60.0)))
}

pub fn tile(hue: f32, size: f32, seed: u64, radius: f32) -> El {
    el().size(size, size)
        .radius(radius)
        .clip_rounded(radius)
        .fill(oklch(0.22, 0.07, hue))
        .items_center()
        .justify_center()
        .a11y_hidden()
        .child(art(hue, size * 1.15, seed, 0.0))
}

/// `placeholder` (an Afterglow tile or disc, `size` points square) with the image at `url`
/// laid over it, once there is one: the placeholder shows while it downloads and decodes, and
/// whenever there's no image. Corners follow `radius`.
pub fn with_image(placeholder: El, url: Option<String>, size: f32, radius: f32) -> El {
    match url.and_then(|u| crate::art::pixels(&u, size)) {
        Some(px) => placeholder.child(el().absolute(0.0, 0.0).size(size, size).radius(radius).pixels_fit(&px, rune::SurfaceFit::Fill)),
        None => placeholder,
    }
}

pub fn icon_button<S: 'static>(key: &str, label: &str, g: G, size: f32, c: Color) -> El {
    theme::focusable(el().key(key.to_string()).role("button").label(label))
        .size(36.0, 36.0)
        .radius(18.0)
        .items_center()
        .justify_center()
        .hover(white(0.08))
        .press(white(0.14))
        .press_scale(0.92)
        .cursor(rune::Cursor::Pointer)
        .child(glyph(g, size, c))
}

/// `b`, a control that can't be used now: read as unavailable, out of the Tab order, and with
/// no hover, press or pointer. Its name and role stay; draw it dimmed and leave its press
/// handler off.
pub fn unavailable(b: El) -> El {
    b.disabled(true).hover(Color::TRANSPARENT).press(Color::TRANSPARENT).press_scale(1.0).cursor(rune::Cursor::Default)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pill {
    /// White, dark ink: the one main action of a view.
    Primary,
    Secondary,
    /// Red: an action that destroys something.
    Danger,
}

/// The app's one pill button: 36 pt tall, an optional icon, then the label (the accessible name).
pub fn pill(key: &str, label: &str, g: Option<G>, kind: Pill) -> El {
    let (bg, fg, hover) = match kind {
        Pill::Primary => (white(0.97), theme::ink(), white(0.9)),
        Pill::Secondary => (white(0.1), white(0.92), white(0.16)),
        Pill::Danger => (theme::danger(), white(0.98), white(0.12)),
    };
    let mut b = theme::focusable(row().key(key.to_string()).role("button").label(label))
        .h(36.0)
        .px(14.0)
        .gap(8.0)
        .radius(18.0)
        .items_center()
        .fill(bg)
        .hover(hover)
        .press_scale(0.96)
        .cursor(rune::Cursor::Pointer);
    if let Some(g) = g {
        b = b.child(glyph(g, 15.0, fg));
    }
    b.child(text(label.to_string()).font(t(13.0, 600.0)).color(fg).a11y_hidden())
}

/// The app's one dialog: a modal scrim over the window (Escape, or a press outside the panel,
/// calls `close`) and a panel `w` points wide (and `h` tall, if given) on the overlay surface,
/// with a close button (`close_btn`: key and accessible name) in its top-right corner. `head`
/// (usually a [`dialog_title`]) stays at the top; `body` goes under it and scrolls.
///
/// The panel never outgrows the window: it keeps a margin all round, narrowing and shortening
/// as the window does, and then only the body scrolls (in a viewport keyed `<key>-scroll`).
#[allow(clippy::too_many_arguments)]
pub fn dialog_frame<S: 'static>(layer_key: &str, key: &str, label: &str, close_btn: (&str, &str), w: f32, h: Option<f32>, head: Option<El>, body: El, close: fn(&mut S)) -> El {
    // A column item that may shrink below its content's height, down to nothing.
    let shrinks = |e: El| e.shrink().min_h(0.0);
    let mut panel = shrinks(theme::overlay_surface(col().key(key.to_string()).role("dialog").label(label).w_full().max_w(w).padding(24.0).gap(16.0), theme::Overlay::Dialog));
    if let Some(h) = h {
        panel = panel.h(h);
    }
    if let Some(head) = head {
        panel = panel.child(head);
    }
    // The viewport reaches 4 pt past the body on every side, so focus rings drawn just outside
    // its controls aren't clipped.
    let viewport = shrinks(col()).mx(-4.0).my(-4.0).scroll(rune::ScrollSpec { stick_bottom: false }).scroll_edge_fade(16.0);
    let body = smooth_wheel::<S>(viewport.child(col().padding(4.0).child(body)), format!("{key}-scroll"));
    col()
        .key(layer_key.to_string())
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .padding(24.0)
        .fill(theme::scrim())
        .items_center()
        .justify_center()
        .modal(false, close)
        .on_press(close)
        .child(
            // a press on the panel itself isn't an outside click
            panel.on_press(|_: &mut S| {}).child(body)
                .child(icon_button::<S>(close_btn.0, close_btn.1, G::Close, 16.0, white(0.6)).pin(None, Some(12.0), Some(12.0), None).on_press(close)),
        )
}

/// A dialog's title, kept clear of the close button.
pub fn dialog_title(title: &str) -> El {
    text(title.to_string()).font(t(theme::DIALOG_TITLE, 700.0)).color(white(theme::TEXT)).role("heading").label(title).mr(40.0)
}

/// A switch's track and knob, drawn on or off. Decorative: the control around it carries the
/// role, name and state.
pub fn switch_mark(on: bool) -> El {
    el().size(36.0, 20.0).radius(10.0).fill(if on { theme::accent() } else { white(0.2) }).a11y_hidden()
        .child(el().absolute(if on { 18.0 } else { 2.0 }, 2.0).size(16.0, 16.0).radius(8.0).fill(white(0.98)))
}

/// A radio button's ring, with the accent dot when it's the chosen one. Decorative, as
/// [`switch_mark`].
pub fn radio_mark(on: bool) -> El {
    let ring = el().size(18.0, 18.0).radius(9.0).border(1.5, if on { theme::accent() } else { white(0.35) }).items_center().justify_center().a11y_hidden();
    if on { ring.child(el().size(8.0, 8.0).radius(4.0).fill(theme::accent())) } else { ring }
}

/// One muted line where a list or page has nothing to show.
pub fn empty(msg: &str) -> El {
    text(msg.to_string()).font(t(13.0, 500.0)).color(white(theme::TEXT_MUTED))
}

/// A horizontal slider: thin track filled to `frac`, 24 pt hit height. `set` gets 0..1.
pub fn slider<S: 'static>(key: &str, label: &str, w: f32, h: f32, frac: f32, value: (f64, f64), set: fn(f32) -> Cmd) -> El {
    let frac = frac.clamp(0.0, 1.0);
    col()
        .key(key.to_string())
        .role("slider")
        .label(label)
        .range_value(value.0, 0.0, value.1)
        .focusable(true)
        .focus_ring(2.0, 2.0, white(0.6))
        .on_step(move |_: &mut S, dir, _| send(set((frac + dir as f32 * 0.03).clamp(0.0, 1.0))))
        .on_drag(move |_: &mut S, d, tx| {
            // the fill follows the pointer exactly; a click or a key glides (see main::poll)
            DRAG_AT.with(|t| t.set(Some(std::time::Instant::now())));
            tx.instant();
            send(set((d.x / d.w).clamp(0.0, 1.0)))
        })
        .cursor(rune::Cursor::Pointer)
        .w(w)
        .h(24.0)
        .justify_center()
        // The fill glides on a short spring when a seek, a click or a new track moves it; the
        // playback clock's once-a-second ticks land at once (main::poll), so playing draws one
        // frame a second instead of running a spring all the time.
        .child(el().w_full().h(h).radius(h / 2.0).fill(white(0.18)).child(el().absolute(0.0, 0.0).h(h).w_frac(frac).radius(h / 2.0).fill(white(0.9)).motion(Spring::new(0.12, 0.0))))
}

thread_local! {
    static DRAG_AT: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

/// Whether a slider was dragged in the last quarter second.
pub fn dragging() -> bool {
    DRAG_AT.with(|t| t.get().is_some_and(|t| t.elapsed().as_millis() < 250))
}

pub fn seek<S: 'static>(v: &View, key: &str, w: f32, h: f32) -> El {
    let frac = if v.duration > 0.0 { (v.position / v.duration) as f32 } else { 0.0 };
    slider::<S>(key, "Position", w, h, frac, (v.position, v.duration.max(0.0)), Cmd::SeekFraction)
}

pub fn volume<S: 'static>(v: &View, key: &str, w: f32) -> El {
    row()
        .gap(8.0)
        .items_center()
        // VolumeSlider.tsx: muted, below half, above
        .child(glyph(if v.volume <= 0.0 { G::VolumeX } else if v.volume < 0.5 { G::Volume1 } else { G::Speaker }, 16.0, white(0.6)))
        .child(slider::<S>(key, "Volume", w, 4.0, v.volume, (v.volume as f64, 1.0), Cmd::SetVolume))
}

/// Three bars that bounce while playing: the "this one is playing" mark in a track list.
///
/// SONE's bars are a CSS loop (`barBounce`: scaleY 0.4 -> 1 -> 0.4 every second, ease-in-out,
/// the bars 0.2 s apart). Here each bar is a renderer-owned [`rune::Sequence`] of scale
/// beats on the same curve, drawn at the display's rate with no view rebuilds, layout or app
/// wakeups. A sequence scales uniformly, so each bar is a wide block scaled about its centre
/// inside a 3 pt clip: only its height shows the scale. (An opacity `Cycle` would be wall-clock
/// too, but it wakes on a timer, which landed every other 120 Hz frame.) `key` must be unique
/// and stable for the row: a new key restarts the bounce.
pub fn eq(playing: bool, key: &str) -> El {
    const H: f32 = 16.0;
    let mut r = row().size(15.0, H).gap(3.0).items_center().a11y_hidden();
    for bar in 0..3 {
        let clip = col().w(3.0).h(H).clip_rounded(1.5).justify_center();
        let fill = el().absolute(-13.5, 0.0).w(30.0).h(H).fill(white(0.92));
        r = r.child(if playing {
            let seq = rune::Sequence::new(eq_beats()).delay(bar as f64 * 0.2);
            clip.child(fill.key(format!("{key}-eq{bar}")).sequence(seq)) // beat 0 (0.4) holds through the delay
        } else {
            clip.child(fill.scale(0.4)) // paused: at rest, at their smallest
        });
    }
    r
}

/// `barBounce` as beats: 0.4 at each whole second, 1.0 at each half, eased in and out between.
/// A sequence is finite and holds its last pose, so this runs for an hour of play per mount.
fn eq_beats() -> &'static [rune::Beat] {
    static BEATS: std::sync::OnceLock<Vec<rune::Beat>> = std::sync::OnceLock::new();
    BEATS.get_or_init(|| {
        (0..=7200).map(|i| rune::Beat::new(i as f64 * 0.5).scale(if i % 2 == 0 { 0.4 } else { 1.0 })).collect()
    })
}

pub fn transport<S: 'static>(v: &View, prefix: &str, play_size: f32) -> El {
    let accent = oklch(0.3, 0.09, hue(v.track_id));
    let play = el()
        .key(format!("{prefix}-play"))
        .role("button")
        .label(if v.playing { "Pause" } else { "Play" })
        .focusable(true)
        .focus_ring(2.0, 3.0, white(0.6))
        .size(play_size, play_size)
        .radius(play_size / 2.0)
        .fill(white(0.97))
        .press_scale(0.93)
        .cursor(rune::Cursor::Pointer)
        .items_center()
        .justify_center()
        .on_press(|_: &mut S| send(Cmd::TogglePlay))
        .child(glyph(if v.playing { G::Pause } else { G::Play }, play_size * 0.42, accent));
    let dim = |on: bool| if on { white(0.95) } else { white(0.45) };
    let repeat_label = match v.repeat {
        Repeat::Off => "Repeat",
        Repeat::All => "Repeat all",
        Repeat::One => "Repeat one",
    };
    row()
        .gap(10.0)
        .items_center()
        .child(icon_button::<S>(&format!("{prefix}-shuffle"), "Shuffle", G::Shuffle, 18.0, dim(v.shuffle)).checked(v.shuffle).on_press(|_: &mut S| send(Cmd::ToggleShuffle)))
        .child(icon_button::<S>(&format!("{prefix}-prev"), "Previous", G::Prev, 22.0, white(0.92)).on_press(|_: &mut S| send(Cmd::Previous)))
        .child(play)
        .child(icon_button::<S>(&format!("{prefix}-next"), "Next", G::Next, 22.0, white(0.92)).on_press(|_: &mut S| send(Cmd::Next)))
        .child(
            icon_button::<S>(&format!("{prefix}-repeat"), repeat_label, G::Repeat, 18.0, dim(v.repeat != Repeat::Off))
                .checked(v.repeat != Repeat::Off)
                .on_press(|_: &mut S| send(Cmd::CycleRepeat)),
        )
}

/// What pressing the bar's cover does: its accessible name and the action.
pub type CoverAction<S> = (&'static str, fn(&mut S));

/// The now-playing bar, tinted by the track.
/// `track_actions` sit after the title, as React's PlayerBar has its heart and More there.
///
/// `room` is the window's width and the width the extras side's content took last layout (the
/// row keyed `nowbar-extras`, `None` before the first), which centre the transport.
/// `on_cover` makes the cover a button: its accessible name and what a press does.
pub fn nowbar<S: 'static>(v: &View, track_actions: El, extras: El, on_cover: Option<CoverAction<S>>, room: (f32, Option<f32>)) -> El {
    let h = hue(v.track_id);
    let seed = v.track_id.unwrap_or(0);
    // The cover flies to the full-screen player and back (full_ui): it leaves the bar while the
    // player is up, so the player's cover can take it over, and returns on the close spring.
    let mut cover = if crate::full_ui::covers_bar() {
        el().key("bar-art-slot").size(48.0, 48.0).a11y_hidden()
    } else {
        with_image(tile(h, 48.0, seed, 8.0), crate::art::cover(v.cover.as_deref(), 48.0), 48.0, 8.0)
            .key("bar-art").matched(crate::full_ui::COVER).motion(crate::full_ui::CLOSE)
    };
    if let Some((label, f)) = on_cover {
        let mut button = theme::focusable(el().key("bar-cover").role("button").label(label)).radius(8.0).cursor(rune::Cursor::Pointer).press_scale(0.95).hover_group().on_press(f).child(cover);
        // Hovering the cover says what a press does: the expand mark over a scrim. It sits beside
        // the artwork, not inside it, so it stays behind when the cover flies to the player.
        if !crate::full_ui::covers_bar() {
            button = button.child(
                el().absolute(0.0, 0.0).size(48.0, 48.0).radius(8.0).fill(theme::black(0.5)).items_center().justify_center().group_opacity(0.0, 1.0).a11y_hidden()
                    .child(glyph(G::Maximize, 18.0, white(0.95))),
            );
        }
        cover = button;
    }
    // The transport sits on the window's centre line whenever there's room. The two sides grow
    // from the same zero basis with the same minimum, the extras' width, so they split what the
    // middle leaves equally, each with its content against its outer edge; the middle shrinks
    // first, from 560 down to the transport and a usable seek slider (300). When even that
    // leaves too little for two sides as wide as the extras, the title side gives way instead,
    // down to its cover and actions (160, truncating the title and artist): the transport then
    // sits as near the centre as the extras allow, and nothing overlaps. The extras side is
    // what the caller put in it (views::bar_extras drops the least useful extras when narrow).
    const PAD: f32 = 16.0;
    const GAP: f32 = 16.0;
    const MIDDLE_MIN: f32 = 300.0;
    let (win_w, extras_w) = room;
    let sides = win_w - 2.0 * PAD - 2.0 * GAP - MIDDLE_MIN;
    let left_min = extras_w.map_or(160.0, |e| e.min(sides - e)).max(160.0);
    let left = row()
        .grow(1.0)
        .min_w(left_min)
        .gap(12.0)
        .items_center()
        .child(cover)
        .child(
            // shrinks (and truncates) rather than grows, so the heart follows the title
            col()
                .shrink()
                .min_w(0.0)
                .gap(2.0)
                .child(text(if v.title.is_empty() { "Nothing playing".to_string() } else { v.title.clone() }).font(t(13.0, 600.0)).color(white(0.95)).truncate())
                .child(text(v.artist.clone()).font(t(12.0, 400.0)).color(white(0.6)).truncate()),
        )
        .child(track_actions);
    let middle = col()
        .w(560.0)
        .shrink()
        .min_w(MIDDLE_MIN)
        .items_center()
        .child(transport::<S>(v, "bar", 40.0))
        .child(
            row()
                .w_full()
                .max_w(560.0)
                .gap(10.0)
                .items_center()
                .child(text(fmt_time(v.position)).font(tnum(t(11.0, 500.0))).color(white(0.55)).w(44.0).align(Align::End))
                .child(col().grow(1.0).child(seek::<S>(v, "bar-seek", 10000.0, 4.0).w_full()))
                .child(text(fmt_time(v.duration)).font(tnum(t(11.0, 500.0))).color(white(0.55)).w(44.0)),
        );
    // (the content row keeps its own width, so measuring it doesn't feed back into the layout)
    let right = row().grow(1.0).justify_end()
        .child(row().key("nowbar-extras").gap(6.0).items_center().child(extras).child(volume::<S>(v, "bar-volume", 90.0)));
    row()
        .key("nowbar")
        .w_full()
        .h(84.0)
        .px(PAD)
        .gap(GAP)
        .items_center()
        .linear_gradient(std::f32::consts::FRAC_PI_2, theme::bar_bg())
        .border(1.0, white(0.06))
        .child(left)
        .child(middle)
        .child(right)
}

/// One row of the queue.
pub fn track_row<S: 'static>(v: &View, key: String, n: usize, item: &QItem, current: bool, play: impl Fn(&mut S) + 'static) -> El {
    let num: El = if current { eq(v.playing, &key) } else { text(format!("{n}")).font(tnum(t(13.0, 500.0))).color(white(0.45)) };
    row()
        .key(key)
        .role("button")
        .label(item.title.clone())
        .focusable(true)
        .focus_ring(2.0, -2.0, white(0.5))
        .w_full()
        .h(52.0)
        .px(12.0)
        .gap(14.0)
        .radius(8.0)
        .items_center()
        .fill(if current { white(0.07) } else { white(0.0) })
        .hover(white(0.05))
        .cursor(rune::Cursor::Pointer)
        .on_press(play)
        .child(el().w(24.0).items_center().child(num))
        .child(
            col()
                .grow(2.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text(item.title.clone()).font(t(14.0, if current { 600.0 } else { 500.0 })).color(white(0.95)).truncate())
                .child(text(item.artist.clone()).font(t(12.0, 400.0)).color(white(0.55)).truncate()),
        )
        .child(text(fmt_time(item.duration as f64)).font(tnum(t(13.0, 400.0))).color(white(0.55)).w(56.0).align(Align::End))
}

pub fn section(title: &str) -> El {
    row().items_center().child(text(title).font(t(20.0, 650.0)).color(white(0.95))).role("heading").label(title).child(spacer())
}

// ---- smooth wheel scrolling
//
// Rune moves a notched mouse wheel's line steps at once (40 pt a notch) and leaves smoothing to
// the app (`WheelEvent::precise`). Here a notch glides instead: each one moves the glide's
// target, and `Tx::scroll_to_with` keeps the running glide's speed, so a spin of the wheel is one
// continuous motion at the display's rate, like WebKit's smooth scrolling. Trackpads and
// free-spinning wheels (precise deltas) scroll as Rune does.

struct WheelGlide {
    offset: f32,
    max: f32,
    target: Option<f32>,
    last: std::time::Instant,
}

thread_local! {
    static GLIDES: std::cell::RefCell<std::collections::HashMap<String, WheelGlide>> = Default::default();
}

const WHEEL_SPRING: Spring = Spring::new(0.22, 0.0);

/// A scroll viewport keyed `key` whose notched-wheel scrolling glides.
pub fn smooth_wheel<S: 'static>(e: El, key: String) -> El {
    let (k_offset, k_wheel) = (key.clone(), key.clone());
    e.key(key)
        .on_scroll_offset(move |p| {
            GLIDES.with(|g| {
                let mut g = g.borrow_mut();
                let st = g.entry(k_offset.clone()).or_insert(WheelGlide {
                    offset: 0.0, max: 0.0, target: None, last: std::time::Instant::now() });
                st.offset = p.offset[1];
                st.max = p.max[1];
                if st.target.is_some_and(|t| (t - p.offset[1]).abs() < 0.5) {
                    st.target = None;
                }
            })
        })
        .on_wheel(move |_: &mut S, ev, tx| {
            if ev.precise || ev.dy == 0.0 || ev.mods.ctrl || ev.mods.cmd {
                return false;
            }
            GLIDES.with(|g| {
                let mut g = g.borrow_mut();
                let Some(st) = g.get_mut(&k_wheel) else { return false };
                // A glide left over from a while ago (or overtaken by a drag) restarts from here.
                let from = match st.target {
                    Some(t) if st.last.elapsed().as_secs_f32() < 0.4 => t,
                    _ => st.offset,
                };
                let to = (from - ev.dy).clamp(0.0, st.max);
                st.target = Some(to);
                st.last = std::time::Instant::now();
                tx.scroll_to_with(k_wheel.clone(), 0.0, to, WHEEL_SPRING);
                true
            })
        })
}
