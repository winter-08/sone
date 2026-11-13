//! Loved Tracks (React FavoritesView): Play and Shuffle over the whole collection, the filter,
//! and two tabs: Tracks (sortable, paged in as the list scrolls, un-loved tracks dropping out)
//! and Videos (the loved videos, paged likewise). A list of thousands builds only the rows in
//! view (views::track_list) and keeps one copy of the items (paged.rs).

use rune::{El, col, row, text, text_input};
use serde_json::{Value, json};

use crate::App;
use crate::coll;
use crate::controller::View;
use crate::interact::{self, Fav};
use crate::paged::{self, PKey};
use crate::theme;
use crate::ui::{self, t, white};
use crate::views;

const SORT_KEY: &str = "__favorites__";

pub fn tracks_key(s: &App) -> PKey {
    let (order, dir) = s.nav.coll.ui.track_sort.get(SORT_KEY).cloned().unwrap_or(("DATE".into(), "DESC".into()));
    PKey::Loved { order, dir }
}

/// Tracks un-loved this session drop out (until the favourites are known, nothing does).
fn still_loved(t: &Value) -> bool {
    !interact::favorites_loaded() || interact::is_fav(Fav::Track, &t["id"].as_u64().unwrap_or(0).to_string())
}

pub fn page(s: &App, v: &View) -> El {
    let videos = s.nav.coll.ui.loved_videos;
    let key = if videos { PKey::LovedVideos } else { tracks_key(s) };
    let snap = paged::get(&key);
    // React loads both tabs' first pages on opening.
    paged::get(&PKey::LovedVideos);
    paged::get(&tracks_key(s));
    let needle = s.nav.filter.text.clone();
    let filtering = !needle.trim().is_empty();
    if filtering {
        paged::all(&key);
    }
    let total = if videos { snap.len } else { snap.total.unwrap_or(snap.len) };
    let (source, name) = if videos {
        (json!({"type": "favorites", "id": "favorites-videos", "name": "Loved Videos"}), "Loved Videos")
    } else {
        (json!({"type": "favorites", "id": "favorites", "name": "Loved Tracks"}), "Loved Tracks")
    };
    let (k1, src1) = (key.clone(), source.clone());
    let include: fn(&Value) -> bool = if videos { loved_video } else { still_loved };
    let head = row().w_full().gap(24.0).items_end().child(views::loved_art(180.0, 14.0, 64.0))
        .child(col().gap(10.0).min_w(0.0).child(views::heading("Loved Tracks", theme::PAGE_TITLE))
            .child(text(if videos { coll::count(total, "video", "videos") } else { coll::count(total, "track", "tracks") }).font(t(13.0, 500.0)).color(white(0.6)))
            .child(coll::play_pair(v, name, move |shuffle| paged::play(k1.clone(), src1.clone(), shuffle, None, include))));
    let filter = text_input(&s.nav.filter, t(13.0, 500.0).lh(18.0))
        .key("loved-filter").label("Filter on title, artist or album").single_line().w(320.0)
        .color(white(0.95)).caret(white(0.95), white(0.25)).placeholder("Filter on title, artist or album", white(0.4))
        .on_edit(|s: &mut App| &mut s.nav.filter);
    let tab = |key: &str, label: &str, on: bool, to_videos: bool| {
        theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(30.0).px(14.0).radius(15.0).items_center()
            .fill(if on { white(0.9) } else { white(0.08) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| if s.nav.coll.ui.loved_videos != to_videos { s.nav.coll.ui.loved_videos = to_videos })
            .child(text(label.to_string()).font(t(12.0, 600.0)).color(if on { theme::ink() } else { white(0.9) }).a11y_hidden())
    };
    let tabs = row().gap(6.0).focus_group().child(tab("loved-tab-tracks", "Tracks", !videos, false)).child(tab("loved-tab-videos", "Videos", videos, true));
    let mut c = col().w_full().gap(16.0)
        .child(head)
        .child(row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).child(filter))
        .child(tabs);
    if snap.first_pending() {
        return c.child(views::loading());
    }
    if snap.failed.is_some() && snap.len == 0 {
        let k = key.clone();
        return c.child(col().gap(10.0).items_start().child(ui::empty("Couldn't load favorites"))
            .child(views::button::<App>("loved-retry", "Retry", None, false).on_press(move |_: &mut App| paged::retry(&k))));
    }
    if videos {
        return c.child(videos_tab(&snap, &needle, source));
    }
    let sort = s.nav.coll.ui.track_sort.get(SORT_KEY).cloned().unwrap_or(("DATE".into(), "DESC".into()));
    c = c.child(coll::sort_header("lv", Some(&sort), true, |s: &mut App, sort| {
        // Clearing goes back to newest first (FavoritesView handleSort).
        match sort {
            Some(x) if x != ("DATE".to_string(), "DESC".to_string()) => { s.nav.coll.ui.track_sort.insert(SORT_KEY.into(), x); }
            _ => { s.nav.coll.ui.track_sort.remove(SORT_KEY); }
        }
        coll::save(&s.nav.coll.ui);
    }));
    let loaded = interact::favorites_loaded();
    let shown: Vec<usize> = (0..snap.len).filter(|i| {
        let tr = &snap.items[*i];
        (!loaded || still_loved(data_of(tr))) && crate::playlist::track_matches(tr, &needle)
    }).collect();
    if shown.is_empty() && !snap.loading {
        return c.child(views::tracks_empty(s, "No loved tracks yet"));
    }
    let n = shown.len();
    let (k2, src2) = (key.clone(), source.clone());
    let rows = views::track_list("lv-t", snap.items.clone(), shown, move |tracks, i| {
        let tr = paged::track_of(&tracks[i]);
        let id = tr["id"].as_u64();
        let (k, src) = (k2.clone(), src2.clone());
        views::track_row(format!("lv-t{i}"), i + 1, &tr, move |_: &mut App| paged::play(k.clone(), src.clone(), false, id, still_loved))
    });
    c = c.child(rows.on_visible_rows(crate::playlist::more_near_end(key.clone(), n)));
    if snap.loading && snap.len > 0 {
        c = c.child(views::loading());
    } else if !snap.more {
        c = c.child(row().w_full().justify_center().py(12.0).child(text(coll::count(total, "TRACK", "TRACKS")).font(t(12.0, 500.0)).color(white(0.35))));
    }
    c
}

fn data_of(v: &Value) -> &Value {
    crate::data::data(v)
}

fn loved_video(t: &Value) -> bool {
    !interact::favorites_loaded() || interact::is_fav(Fav::Video, &t["id"].as_u64().unwrap_or(0).to_string())
}

/// The Videos tab: loved videos as cards, paged in at the end; unhearted ones drop out.
fn videos_tab(snap: &paged::Snap, needle: &str, source: Value) -> El {
    let vids: Vec<Value> = snap.shown().iter().map(|v| crate::data::data(v).clone())
        .filter(|v| loved_video(v) && crate::data::matches(&format!("{} {}", crate::data::name(v), crate::data::artist_of(v)), needle))
        .collect();
    if snap.len == 0 {
        // "Go to videos": Explore's Videos page, when Explore has one.
        let target = crate::data::peek(&crate::data::Key::Page("pages/explore".into())).and_then(|e| {
            e["sections"].as_array()?.iter().flat_map(|s| s["items"].as_array().cloned().unwrap_or_default())
                .find(|i| crate::data::name(i) == "Videos" && i["apiPath"].is_string())
                .and_then(|i| i["apiPath"].as_str().map(str::to_string))
        });
        crate::data::want(crate::data::Key::Page("pages/explore".into()));
        return col().gap(10.0).items_start()
            .child(text("No favorite videos yet").font(t(15.0, 650.0)).color(white(0.9)))
            .child(ui::empty("Favorite a video and it'll show up here."))
            .child(views::button::<App>("loved-go-videos", "Go to videos", None, true).on_press(move |s: &mut App| match &target {
                Some(p) => views::go(s, views::Page::ExploreSub(p.clone(), "Videos".into())),
                None => views::go(s, views::Page::Explore),
            }));
    }
    if vids.is_empty() {
        return ui::empty(&format!("No videos match \u{201c}{}\u{201d}.", needle.trim()));
    }
    let mut c = col().w_full().gap(10.0).child(views::grid(crate::video_ui::video_cards("lvv", &vids, source)));
    if snap.more && needle.trim().is_empty() {
        let n = snap.len;
        c = c.child(rune::el().key(format!("lvv-more-{n}")).w_full().h(2.0).a11y_hidden()
            .on_visible(|_: &mut App, on, _| if on { paged::more(&PKey::LovedVideos) }));
    }
    if snap.loading {
        c = c.child(views::loading());
    }
    c
}
