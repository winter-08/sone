//! Afterglow, from ~/dev/music-player: hue-tinted darks, white at fixed alphas, generated ring
//! art, round icon buttons, the now-playing bar and the track row. Rebound here to SONE's
//! playback controller instead of music-player's engine.
//!
//! Every control has a role and an accessible name; the verifier drives and inventories the UI
//! through AT-SPI, so an unnamed control is a defect.

use crate::controller::{Cmd, QItem, Repeat, View, send};
use crate::glyph::{G, glyph};
use rune::{Align, Color, El, Spring, TextStyle, col, el, oklch, oklcha, row, spacer, text};

pub fn white(a: f32) -> Color {
    oklcha(0.985, 0.0, 0.0, a)
}
pub fn base(h: f32) -> Color {
    oklch(0.155, 0.025, h)
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
        .rotate_motion(Spring::new(0.6, 0.0))
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

pub fn icon_button<S: 'static>(key: &str, label: &str, g: G, size: f32, c: Color) -> El {
    el().key(key.to_string())
        .role("button")
        .label(label)
        .focusable(true)
        .focus_ring(2.0, 2.0, white(0.6))
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
        .on_drag(move |_: &mut S, d, _| send(set((d.x / d.w).clamp(0.0, 1.0))))
        .cursor(rune::Cursor::Pointer)
        .w(w)
        .h(24.0)
        .justify_center()
        .child(el().w_full().h(h).radius(h / 2.0).fill(white(0.18)).child(el().absolute(0.0, 0.0).h(h).w_frac(frac).radius(h / 2.0).fill(white(0.9))))
}

pub fn seek<S: 'static>(v: &View, key: &str, w: f32, h: f32) -> El {
    let frac = if v.duration > 0.0 { (v.position / v.duration) as f32 } else { 0.0 };
    slider::<S>(key, "Position", w, h, frac, (v.position, v.duration.max(0.0)), Cmd::SeekFraction)
}

pub fn volume<S: 'static>(v: &View, key: &str, w: f32) -> El {
    row()
        .gap(8.0)
        .items_center()
        .child(glyph(G::Speaker, 18.0, white(0.6)))
        .child(slider::<S>(key, "Volume", w, 4.0, v.volume, (v.volume as f64, 1.0), Cmd::SetVolume))
}

/// Three bars that bounce while playing: the "this one is playing" mark in a track list.
pub fn eq(v: &View) -> El {
    let l = (v.position * 1.7) as f32;
    let mut r = row().size(16.0, 14.0).gap(2.0).items_end().a11y_hidden();
    for (i, k) in [0.7f32, 1.0, 0.55].iter().enumerate() {
        let h = if v.playing { 3.0 + 11.0 * (l * k + i as f32 * 0.13).fract().max(0.15) } else { 3.0 };
        r = r.child(el().w(4.0).h(h).radius(1.0).fill(white(0.92)).motion(Spring::new(0.12, 0.0)));
    }
    r
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

/// The now-playing bar, tinted by the track.
pub fn nowbar<S: 'static>(v: &View, extras: El) -> El {
    let h = hue(v.track_id);
    let seed = v.track_id.unwrap_or(0);
    let left = row()
        .w(300.0)
        .gap(12.0)
        .items_center()
        .child(tile(h, 48.0, seed, 8.0))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text(if v.title.is_empty() { "Nothing playing".to_string() } else { v.title.clone() }).font(t(13.0, 600.0)).color(white(0.95)).truncate())
                .child(text(v.artist.clone()).font(t(12.0, 400.0)).color(white(0.6)).truncate()),
        );
    let middle = col()
        .grow(1.0)
        .min_w(0.0)
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
    let mut right = row().w(420.0).gap(6.0).items_center().justify_end();
    right = right.child(extras).child(volume::<S>(v, "bar-volume", 90.0));
    row()
        .key("nowbar")
        .w_full()
        .h(84.0)
        .px(16.0)
        .gap(16.0)
        .items_center()
        .linear_gradient(std::f32::consts::FRAC_PI_2, [(0.0, oklch(0.24, 0.07, h)), (1.0, oklch(0.18, 0.03, h + 20.0))])
        .border(1.0, white(0.06))
        .child(left)
        .child(middle)
        .child(right)
}

/// One row of the queue.
pub fn track_row<S: 'static>(v: &View, key: String, n: usize, item: &QItem, current: bool, play: impl Fn(&mut S) + 'static) -> El {
    let num: El = if current { eq(v) } else { text(format!("{n}")).font(tnum(t(13.0, 500.0))).color(white(0.45)) };
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
