//! The drawer's Play queue tab (React NowPlayingDrawer QueueTab and its TrackRow), and the
//! "Playing from" navigation the player bar shares (PlayingFromLabel navigateToSource).
//!
//! History (the last ten, oldest first, a press plays one again), Now playing, then the
//! upcoming list: the manual "Next in queue" items, a divider, and the context queue "Next up
//! from" its collection. Upcoming rows drag by their grip to a new place (setQueueTracks
//! `reorder`); a row carried across the divider changes section. The grip also takes the arrow
//! keys, a step per press, which the React list has no equivalent of.

use crate::App;
use crate::controller::{self, Cmd, QItem, Source, View};
use crate::glyph::{G, glyph};
use crate::interact::{self, Anchor, Fav, Say};
use crate::theme::{self, accent};
use crate::ui::{self, t, white};
use crate::views::{self, Page};
use rune::{Drag, El, Phase, Tracked, Tx, col, el, row, spacer, text};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// A row and the gap under it: the drag's step.
const ROW_H: f32 = 52.0;
const GAP: f32 = 4.0;
const PITCH: f32 = ROW_H + GAP;

static GRIP: rune::Icon = rune::Icon { name: "lucide-grip_vertical", view: 24.0, body: r#"<circle cx="9" cy="12" r="1" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="9" cy="5" r="1" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="9" cy="19" r="1" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="15" cy="12" r="1" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="15" cy="5" r="1" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="15" cy="19" r="1" fill="none" stroke="currentColor" stroke-width="2"/>"# };

/// A row being dragged: which (its qid and upcoming index), where it would land, and how far
/// the pointer has carried it.
#[derive(Clone, Debug, PartialEq)]
struct Held {
    qid: String,
    from: usize,
    to: usize,
    dy: f32,
}

thread_local! {
    /// The drag in progress. `Tracked`, so the drawer's region rebuilds as it moves.
    static HELD: RefCell<Tracked<Option<Held>>> = RefCell::new(Tracked::new(None));
}

fn held() -> Option<Held> {
    HELD.with_borrow(|h| (**h).clone())
}

/// The drawer's scroll viewport while the queue shows (it's keyed "drawer-scroll"): its top and
/// height in the window, and how far it is scrolled of how far it can be.
#[derive(Clone, Copy, Default)]
struct Viewport {
    top: f32,
    h: f32,
    offset: f32,
    max: f32,
}

/// A drag's edge scrolling (dnd-kit's autoScroll in React's queue): the pointer's window y, the
/// viewport's offset at the press, and what `drop_index` needs to place the row again as the
/// list scrolls under a pointer held still.
#[derive(Clone, Copy)]
struct Edge {
    y: f32,
    from_offset: f32,
    dy: f32,
    from: usize,
    len: usize,
    mc: usize,
    divider: bool,
}

thread_local! {
    static VIEWPORT: std::cell::Cell<Viewport> = std::cell::Cell::new(Viewport::default());
    static EDGE: std::cell::Cell<Option<(String, Edge)>> = const { std::cell::Cell::new(None) };
}

/// Where the drawer's viewport is in the window (its layout; the queue tab only).
pub fn note_viewport(r: [f32; 4]) {
    VIEWPORT.with(|v| v.set(Viewport { top: r[1], h: r[3], ..v.get() }));
}

/// Where the drawer's viewport is scrolled (the queue tab only).
pub fn note_scroll(p: rune::ScrollPos) {
    VIEWPORT.with(|v| v.set(Viewport { offset: p.offset[1], max: p.max[1], ..v.get() }));
}

/// How far to scroll this frame with the pointer at window y `y` in a viewport from `top`,
/// `h` tall: nothing outside the 48 pt bands at its ends, up to 14 pt a frame at the very edge
/// (and beyond it), upward negative.
fn edge_speed(y: f32, top: f32, h: f32) -> f32 {
    const BAND: f32 = 48.0;
    const MAX: f32 = 14.0;
    if h <= 2.0 * BAND {
        return 0.0;
    }
    let up = top + BAND - y;
    let down = y - (top + h - BAND);
    if up > 0.0 {
        -MAX * (up / BAND).min(1.0)
    } else if down > 0.0 {
        MAX * (down / BAND).min(1.0)
    } else {
        0.0
    }
}

/// The pointer's travel through the list: its own, plus how far the list scrolled under it.
fn list_dy(e: &Edge, offset: f32) -> f32 {
    e.dy + (offset - e.from_offset)
}

/// From the window's poll while a row is dragged: near the viewport's top or bottom edge the
/// list scrolls, a step each frame while the pointer stays there, and the row's landing place
/// follows. Whether anything the view draws changed.
pub fn tick(tx: &mut Tx) -> bool {
    let Some((qid, e)) = EDGE.with(|c| c.take()) else { return false };
    // (the drag is over however it ended: nothing scrolls on)
    if held().is_none() {
        return false;
    }
    EDGE.with(|c| c.set(Some((qid.clone(), e))));
    let v = VIEWPORT.with(|c| c.get());
    let speed = edge_speed(e.y, v.top, v.h);
    let to = (v.offset + speed).clamp(0.0, v.max.max(0.0));
    if speed == 0.0 || (to - v.offset).abs() < 0.5 {
        return false;
    }
    tx.scroll_to_y("drawer-scroll", to);
    VIEWPORT.with(|c| c.set(Viewport { offset: to, ..v }));
    let dy = list_dy(&e, to);
    set_held(Some(Held { qid, from: e.from, to: drop_index(e.from, dy, e.len, e.mc, e.divider), dy }));
    tx.wake_after(std::time::Duration::from_millis(16));
    true
}

fn set_held(v: Option<Held>) {
    HELD.with_borrow_mut(|h| {
        if *Tracked::untracked(h) != v {
            **h = v;
        }
    });
}

/// The source types a "Playing from" link can open (isNavigableSource).
pub fn navigable(s: &Source) -> bool {
    matches!(s.kind.as_str(), "album" | "playlist" | "playlist-recs" | "mix" | "artist" | "artist-tracks" | "favorites" | "radio")
}

/// navigateToSource: the source's page.
pub fn open_source(s: &mut App, src: &Source) {
    if let Some(p) = source_page(src) {
        views::go(s, p);
    }
}

/// A page the miniplayer asked the main window to show (its show-album, show-artist and
/// show-source commands), taken by the main window's next poll.
static NAV: std::sync::Mutex<Option<Page>> = std::sync::Mutex::new(None);

pub fn request_nav(p: Option<Page>) {
    if let Some(p) = p {
        *NAV.lock().unwrap() = Some(p);
        controller::bump();
    }
}

/// From the main window's poll.
pub fn apply_nav(s: &mut App) {
    let p = NAV.lock().unwrap().take();
    if let Some(p) = p {
        views::go(s, p);
    }
}

pub fn source_page(src: &Source) -> Option<Page> {
    match src.kind.as_str() {
        "album" => src.id.parse().ok().map(Page::Album),
        "playlist" | "playlist-recs" => Some(Page::Playlist(src.id.clone())),
        "mix" => Some(Page::Mix(src.id.clone(), src.name.clone(), src.subtitle.clone().unwrap_or_default())),
        "radio" => Some(Page::Mix(src.id.clone(), src.name.clone(), String::new())),
        "artist" => src.id.parse().ok().map(Page::Artist),
        "artist-tracks" => src.id.parse().ok().map(|id| Page::ArtistTracks(id, src.name.clone())),
        "favorites" => Some(Page::Loved),
        _ => None,
    }
}

/// Text that brightens under the pointer, as the React links do (`hover:text-th-text-primary`).
fn hover_text(t: El) -> El {
    el().shrink().min_w(0.0).hover_group().child(t.group_color(white(0.92)))
}

/// A queue or Suggested row's artists: each a link to its page (the track rows' shared
/// views::artist_links_in, at the rows' 11 pt).
pub fn row_artists(key: &str, track: &Value) -> El {
    views::artist_links_in(key, track, t(11.0, 400.0), white(0.5)).shrink()
}

fn heading(label: &str) -> El {
    text(label.to_uppercase()).font(t(13.0, 700.0).tracking(0.8)).color(white(theme::TEXT_MUTED)).role("heading").label(label.to_string())
}

/// "Next up from <source>" with the name as a link when it has a page; "Next up" without one.
fn next_up_from(src: Option<&Source>, key: &str) -> El {
    let Some(src) = src else { return heading("Next up") };
    let mut r = row().gap(4.0).items_center().min_w(0.0).shrink()
        .child(text("NEXT UP FROM").font(t(13.0, 700.0).tracking(0.8)).color(white(theme::TEXT_MUTED)).a11y_hidden());
    let name = text(src.name.to_uppercase()).font(t(13.0, 700.0).tracking(0.8)).color(white(theme::TEXT_MUTED)).truncate();
    if navigable(src) {
        let s2 = src.clone();
        r = r.child(theme::focusable(row().key(key.to_string()).role("link").label(src.name.clone())).shrink().min_w(0.0).radius(4.0).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| open_source(s, &s2))
            .child(hover_text(name)));
    } else {
        r = r.child(name);
    }
    r.role("heading").label(format!("Next up from {}", src.name))
}

/// The queue tab.
pub fn panel(v: &View) -> El {
    let mut c = col().w_full().gap(24.0);
    if !v.history.is_empty() {
        let mut list = col().w_full().gap(2.0);
        for (i, item) in v.history.iter().enumerate() {
            list = list.child(track_row(v, item, Kind::History, format!("qh-{i}")));
        }
        c = c.child(col().w_full().gap(12.0).child(heading("History")).child(list));
    }
    if let Some(cur) = &v.current {
        c = c.child(col().w_full().gap(12.0).child(heading("Now playing")).child(track_row(v, cur, Kind::Current, "qp-now".into())));
    }
    if !v.queue.is_empty() {
        let head: El = if v.manual_len > 0 { heading("Next in queue") } else { next_up_from(v.source.as_ref(), "qp-source") };
        let clear = theme::focusable(row().key("qp-clear").role("button").label("Clear")).h(26.0).px(10.0).radius(13.0).items_center().hover(white(0.08)).cursor(rune::Cursor::Pointer)
            .on_press(|_: &mut App| controller::send(Cmd::ClearQueue))
            .child(text("Clear").font(t(11.0, 600.0)).color(white(0.7)).a11y_hidden());
        c = c.child(col().w_full().gap(12.0)
            .child(row().w_full().gap(8.0).items_center().child(head).child(spacer()).child(clear))
            .child(upcoming(v)));
    }
    if v.queue.is_empty() && v.current.is_none() {
        c = c.child(col().w_full().py(64.0).gap(12.0).items_center()
            .child(glyph(G::Music, 40.0, white(0.25)))
            .child(text("Queue is empty").font(t(13.0, 500.0)).color(white(0.4))));
    }
    c
}

/// The upcoming list, built only while in or near view (a queue can hold thousands): the
/// manual items, a divider row when the context queue follows them, then the context queue.
fn upcoming(v: &View) -> El {
    let items = Rc::new(v.queue.clone());
    let mc = v.manual_len.min(items.len());
    let divider = mc > 0 && mc < items.len();
    let count = items.len() + divider as usize;
    // virtual index -> upcoming index (None: the divider)
    let at = move |vi: usize| -> Option<usize> {
        if divider && vi == mc { None } else if divider && vi > mc { Some(vi - 1) } else { Some(vi) }
    };
    let ctx = v.context_source.clone().or(v.source.clone());
    let hold = held();
    let keys_version = {
        use std::hash::{BuildHasher, BuildHasherDefault};
        BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one((items.iter().map(|q| q.qid.as_str()).collect::<Vec<_>>(), mc))
    };
    let (keys, view) = (items.clone(), Rc::new(v.clone()));
    col().key("qp-rows").w_full().gap(GAP).growth(rune::GrowthSpec::default()).instant_changes()
        .virtual_rows(count, ROW_H, move |_: &App, vi| match at(vi) {
            Some(i) => format!("qp-{}", keys[i].qid).into(),
            None => "qp-divider".into(),
        }, move |_: &App, vi| match at(vi) {
            None => row().key("qp-divider").w_full().h(ROW_H).items_end().pb(10.0).child(next_up_from(ctx.as_ref(), "qp-ctx-source")),
            Some(i) => queued_row(&view, &items, i, mc, divider, hold.as_ref()),
        })
        .virtual_rows_version(keys_version)
}

/// Where a drag that started on upcoming item `from` and moved `dy` would land. Rows are
/// `PITCH` apart; the divider takes a row's place. Dropped on the divider, an item stays in its
/// own section, at the end nearest it.
fn drop_index(from: usize, dy: f32, len: usize, mc: usize, divider: bool) -> usize {
    let vfrom = if divider && from >= mc { from + 1 } else { from };
    let vlen = len + divider as usize;
    let vto = (vfrom as f32 + (dy / PITCH).round()).clamp(0.0, (vlen - 1) as f32) as usize;
    if !divider {
        return vto;
    }
    match vto.cmp(&mc) {
        std::cmp::Ordering::Less => vto,
        std::cmp::Ordering::Greater => vto - 1,
        std::cmp::Ordering::Equal => if from < mc { mc - 1 } else { mc },
    }
}

fn queued_row(v: &View, items: &Rc<Vec<QItem>>, i: usize, mc: usize, divider: bool, hold: Option<&Held>) -> El {
    let item = &items[i];
    let len = items.len();
    let dragged = hold.is_some_and(|h| h.qid == item.qid);
    let mut r = track_row(v, item, Kind::Queued, format!("qp-{}", item.qid));
    // the drop line, above or below the row it would land on (React's indicator)
    if let Some(h) = hold.filter(|h| h.to == i && h.from != i) {
        let y = if h.from > i { -GAP / 2.0 - 1.0 } else { ROW_H + GAP / 2.0 - 1.0 };
        r = r.child(el().absolute(24.0, y).h(2.0).w_frac(1.0).mr(24.0).radius(1.0).fill(accent()).a11y_hidden());
    }
    let (qid, qid_keys, title) = (item.qid.clone(), item.qid.clone(), item.title.clone());
    let grip = theme::focusable(el().key(format!("qp-{}-grip", item.qid)).role("button").label(format!("Move {title}")))
        .size(24.0, ROW_H).radius(6.0).items_center().justify_center().cursor(if dragged { rune::Cursor::Grabbing } else { rune::Cursor::Grab })
        .on_drag(move |_: &mut App, d: &mut Drag, tx: &mut Tx| match d.phase {
            // (the viewport's offset at the press, so a list scrolled under the drag counts)
            Phase::Down => d.data = VIEWPORT.with(|c| c.get().offset),
            Phase::Move => {
                let e = Edge { y: d.window_point()[1], from_offset: d.data, dy: d.dy, from: i, len, mc, divider };
                let dy = list_dy(&e, VIEWPORT.with(|c| c.get().offset));
                EDGE.with(|c| c.set(Some((qid.clone(), e))));
                let to = drop_index(i, dy, len, mc, divider);
                set_held(Some(Held { qid: qid.clone(), from: i, to, dy }));
                tx.instant();
                // the poll scrolls at an edge, even with the pointer held still
                tx.wake_after(std::time::Duration::ZERO);
            }
            Phase::Up => {
                let h = held();
                EDGE.with(|c| c.set(None));
                set_held(None);
                if let Some(h) = h.filter(|h| !d.cancelled() && h.to != h.from) {
                    controller::send(Cmd::MoveQueued { qid: h.qid, to: h.to });
                }
            }
        })
        // (arrow keys: a step up or down, Page Up/Down ten)
        .on_step(move |_: &mut App, dir: i32, _| {
            let to = (i as i64 - dir as i64).clamp(0, len as i64 - 1) as usize;
            if to != i {
                controller::send(Cmd::MoveQueued { qid: qid_keys.clone(), to });
            }
        })
        .child(rune::icon(&GRIP, 14.0).color(white(0.35)).a11y_hidden());
    let mut line = row().w_full().gap(4.0).items_center().child(grip).child(r.grow(1.0).min_w(0.0));
    if dragged {
        line = line.opacity(0.3).offset(0.0, hold.map_or(0.0, |h| h.dy));
    }
    line
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    History,
    Current,
    Queued,
}

/// TrackRow: cover (the playing bars over it while it plays), title with its badges, artist
/// and album as links, then More options, the heart and (for an upcoming item) Remove. An
/// unavailable row greys and only says so; History rows are dimmed.
fn track_row(v: &View, item: &QItem, kind: Kind, key: String) -> El {
    let active = kind == Kind::Current;
    let unavailable = item.unavailable && !active;
    let mut cover = ui::with_image(ui::tile(ui::hue(Some(item.id)), 40.0, item.id, 4.0), crate::art::cover(item.cover.as_deref(), 40.0), 40.0, 4.0);
    if active && v.playing && !unavailable {
        cover = cover.child(row().absolute(0.0, 0.0).size(40.0, 40.0).radius(4.0).fill(theme::black(0.4)).items_center().justify_center().child(ui::eq(true, &format!("{key}-eq"))));
    }
    let mut title = row().gap(6.0).min_w(0.0).items_center()
        .child(text(item.title.clone()).font(t(13.0, 500.0)).color(if active { accent() } else { white(theme::TEXT) }).truncate().shrink());
    if let Some(b) = crate::content::badges(item.explicit, item.ai) {
        title = title.child(b);
    }
    let mut sub = row().gap(0.0).min_w(0.0).items_center().child(row_artists(&key, &item.track));
    if !item.album.is_empty() {
        sub = sub.child(text(" · ").font(t(11.0, 400.0)).color(white(0.5)).a11y_hidden());
        let album = text(item.album.clone()).font(t(11.0, 400.0)).color(white(0.5)).truncate().shrink();
        sub = match item.album_id {
            Some(id) => sub.child(hover_text(album).cursor(rune::Cursor::Pointer).on_press(move |s: &mut App| views::go(s, Page::Album(id)))),
            None => sub.child(album),
        };
    }
    let track = (*item.track).clone();
    let (t1, t2, more_key) = (track.clone(), track, format!("{key}-more"));
    let mk = more_key.clone();
    let loved = interact::is_fav(Fav::Track, &item.id.to_string());
    let (fid, fl) = (item.id.to_string(), item.title.clone());
    let mut actions = row().gap(2.0).items_center()
        .child(ui::icon_button::<App>(&more_key, "More options", G::More, 15.0, white(0.45))
            .on_press(move |s: &mut App| interact::open_track(s, t1.clone(), None, Anchor::Below(mk.clone()))))
        .child(ui::icon_button::<App>(&format!("{key}-love"), if loved { "Remove from Loved tracks" } else { "Add to Loved tracks" }, if loved { G::HeartFilled } else { G::Heart }, 15.0, if loved { accent() } else { white(0.45) })
            .on_press(move |_: &mut App| interact::toggle_fav(Fav::Track, fid.clone(), fl.clone(), Say::LovedShort)));
    if kind == Kind::Queued {
        let q = item.qid.clone();
        actions = actions.child(ui::icon_button::<App>(&format!("{key}-rm"), "Remove", G::Close, 14.0, white(0.45))
            .on_press(move |_: &mut App| controller::send(Cmd::RemoveQueued(q.clone()))));
    }
    let q = item.qid.clone();
    let play = move |_: &mut App| {
        if unavailable {
            interact::toast("Track unavailable".into(), interact::Kind::Info);
            return;
        }
        match kind {
            Kind::History => controller::send(Cmd::PlayHistory(q.clone())),
            Kind::Queued => controller::send(Cmd::PlayQueued(q.clone())),
            Kind::Current => {}
        }
    };
    // Not an accessible node (it has none in React either); its buttons are.
    let mut r = theme::focusable_row(row().key(key).role("presentation")).w_full().h(ROW_H).px(10.0).gap(12.0).radius(8.0).items_center()
        .on_press(play)
        .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, t2.clone(), None, Anchor::Point(p.x, p.y)))
        .child(cover)
        .child(col().grow(1.0).min_w(0.0).gap(2.0).child(title).child(sub))
        .child(actions);
    r = if active && !unavailable {
        r.fill(white(0.08))
    } else if unavailable {
        r
    } else {
        r.hover(white(0.05)).cursor(rune::Cursor::Pointer)
    };
    if kind == Kind::History || unavailable {
        r = r.opacity(0.5);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::drop_index;

    #[test]
    fn a_drag_scrolls_only_near_the_edges() {
        assert_eq!(super::edge_speed(300.0, 100.0, 600.0), 0.0);
        assert!(super::edge_speed(110.0, 100.0, 600.0) < 0.0);
        assert_eq!(super::edge_speed(40.0, 100.0, 600.0), -14.0);
        assert!(super::edge_speed(690.0, 100.0, 600.0) > 0.0);
        assert_eq!(super::edge_speed(900.0, 100.0, 600.0), 14.0);
        // a viewport too short for both bands doesn't scroll
        assert_eq!(super::edge_speed(100.0, 100.0, 90.0), 0.0);
        // the landing place counts the list scrolled under the pointer
        let e = super::Edge { y: 0.0, from_offset: 100.0, dy: 20.0, from: 0, len: 5, mc: 0, divider: false };
        assert_eq!(super::list_dy(&e, 212.0), 132.0);
    }

    #[test]
    fn drags_land_by_whole_rows() {
        // five items, no divider
        assert_eq!(drop_index(0, 10.0, 5, 0, false), 0);
        assert_eq!(drop_index(0, 60.0, 5, 0, false), 1);
        assert_eq!(drop_index(4, -1000.0, 5, 0, false), 0);
        assert_eq!(drop_index(1, 1000.0, 5, 0, false), 4);
    }

    #[test]
    fn the_divider_keeps_an_item_in_its_section() {
        // two manual, three context: virtual rows m0 m1 | c2 c3 c4
        assert_eq!(drop_index(1, 56.0, 5, 2, true), 1, "manual dropped on the divider stays last manual");
        assert_eq!(drop_index(2, -56.0, 5, 2, true), 2, "context dropped on the divider stays first context");
        assert_eq!(drop_index(0, 168.0, 5, 2, true), 2, "past the divider joins the context queue");
        assert_eq!(drop_index(3, -168.0, 5, 2, true), 1, "above it joins the manual items");
    }
}
