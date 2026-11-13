//! A section's whole list (React ViewAllPage) and an artist's popular tracks (React
//! ArtistTracksPage).
//!
//! View all from an artist page pages through get_artist_view_all, 50 at a time as the grid's
//! end comes into view; from Home, an album page or anywhere else it is one get_page_section,
//! every section's items in one grid. Popular tracks pages through get_artist_top_tracks_all.

use rune::{El, col, el, row, text};
use serde_json::{Value, json};

use crate::App;
use crate::coll;
use crate::controller::{self, Cmd, View};
use crate::data::{self, Key};
use crate::paged::{self, PKey};
use crate::theme;
use crate::ui::{self, t, white};
use crate::views::{self, Page};

/// An item with its kind: sections give `{type, data}`, or bare objects whose section type
/// says what they are, and view-all pages bare objects; a bare one is known by its fields.
pub fn typed(it: &Value, section_kind: Option<&str>) -> Value {
    if !data::kind(it).is_empty() && it.get("data").is_some_and(Value::is_object) {
        return it.clone();
    }
    let d = data::data(it);
    let k = section_kind.map(str::to_string).filter(|k| !k.is_empty() && k != "MIXED_TYPES").unwrap_or_else(|| {
        let k = data::kind(it);
        if !k.is_empty() {
            k
        } else if d.get("uuid").is_some() {
            "PLAYLIST".into()
        } else if d["mixType"].is_string() {
            "MIX".into()
        } else if d.get("imageId").is_some() && d.get("duration").is_some() && d.get("album").is_none() {
            "VIDEO".into()
        } else if d.get("duration").is_some() && d.get("album").is_some() {
            "TRACK".into()
        } else if d.get("numberOfTracks").is_some() || d.get("releaseDate").is_some() || d.get("cover").is_some() {
            "ALBUM".into()
        } else if d.get("name").is_some() {
            "ARTIST".into()
        } else {
            String::new()
        }
    });
    json!({"type": k, "data": d.clone()})
}

/// Cards, video cards and track rows for a mixed list of typed items, in their order: tracks
/// play from the clicked one through `play_track` (with its index among the tracks).
/// From an artist's page (`artist`), albums are subtitled with their year and kind.
fn items_view(prefix: &str, items: &[Value], source: Value, artist: Option<u64>, play_track: impl Fn(usize) + Clone + 'static) -> El {
    let tracks: Vec<Value> = items.iter().filter(|it| data::kind(it) == "TRACK").map(|it| data::data(it).clone()).collect();
    let videos: Vec<Value> = items.iter().filter(|it| data::kind(it) == "VIDEO").cloned().collect();
    let cards = match artist {
        Some(id) => views::cards_with(prefix, items, move |it| views::artist_card_sub(it, id)),
        None => views::cards_for(prefix, items),
    };
    let my_tracks: Vec<&Value> = items.iter().filter(|it| crate::home::is_my_tracks(it)).collect();
    let mut c = col().w_full().gap(16.0);
    if !my_tracks.is_empty() || !cards.is_empty() {
        let mut all = vec![];
        if !my_tracks.is_empty() {
            // "My Tracks" shows as Loved Tracks (ViewAllPage).
            all.push(col().key(format!("{prefix}-loved")).role("presentation").w(148.0).gap(6.0).padding(8.0).radius(12.0).hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut App| views::go(s, Page::Loved))
                .child(views::loved_art(132.0, 10.0, 48.0))
                .child(views::heading("Loved Tracks", 13.0).truncate().focusable(true).no_focus_ring().on_press(|s: &mut App| views::go(s, Page::Loved)))
                .child(text("Collection").font(t(11.0, 400.0)).color(white(0.55))));
        }
        all.extend(cards);
        c = c.child(views::grid(all));
    }
    if !videos.is_empty() {
        c = c.child(views::grid(crate::video_ui::video_cards(&format!("{prefix}-v"), &videos, source)));
    }
    if !tracks.is_empty() {
        let mut rows = col().w_full().gap(2.0);
        for (i, tr) in tracks.iter().enumerate() {
            let p = play_track.clone();
            rows = rows.child(views::track_row(format!("{prefix}-t{i}"), i + 1, tr, move |_: &mut App| p(i)));
        }
        c = c.child(rows);
    }
    c
}

pub fn page(s: &App, title: &str, path: &str, artist: Option<u64>) -> El {
    let head = views::heading(title, theme::PAGE_TITLE);
    let source = json!({"type": "view-all", "id": path, "name": title});
    match artist {
        Some(id) => {
            let key = PKey::ArtistViewAll(id, path.to_string());
            let snap = paged::get(&key);
            let mut c = col().w_full().gap(18.0).child(head);
            if snap.first_pending() {
                return c.child(crate::skeleton::page(&crate::data::Key::Page(String::new())));
            }
            if snap.failed.is_some() && snap.len == 0 {
                let k = key.clone();
                return c.child(col().gap(10.0).items_start().child(ui::empty("Couldn't load this"))
                    .child(views::button::<App>("va-retry", "Retry", None, false).on_press(move |_: &mut App| paged::retry(&k))));
            }
            let items: Vec<Value> = snap.shown().iter().map(|it| typed(it, None)).collect();
            if items.is_empty() && !snap.loading {
                return c.child(ui::empty("No items found"));
            }
            let (k1, src1) = (key.clone(), source.clone());
            let ids: Vec<Option<u64>> = items.iter().filter(|it| data::kind(it) == "TRACK").map(|it| data::data(it)["id"].as_u64()).collect();
            c = c.child(items_view("va", &items, source, Some(id), move |i| paged::play(k1.clone(), src1.clone(), false, ids.get(i).copied().flatten(), |_| true)));
            if snap.more {
                let n = snap.len;
                c = c.child(el().key(format!("va-more-{n}")).w_full().h(8.0).a11y_hidden().on_visible(move |_: &mut App, on, _| if on { paged::more(&key) }));
            }
            if snap.loading && snap.len > 0 {
                c = c.child(views::loading());
            }
            let _ = s;
            c
        }
        None => {
            let v = match views::ready_or(Key::Page(path.to_string())) { Ok(v) => v, Err(e) => return col().w_full().gap(18.0).child(head).child(e) };
            let mut items = vec![];
            for sec in v["sections"].as_array().cloned().unwrap_or_default() {
                let k = sec["sectionType"].as_str().and_then(|t| t.strip_suffix("_LIST")).map(str::to_string);
                for it in sec["items"].as_array().cloned().unwrap_or_default() {
                    items.push(typed(&it, k.as_deref()));
                }
            }
            let mut c = col().w_full().gap(18.0).child(head);
            if items.is_empty() {
                return c.child(ui::empty("No items found"));
            }
            let tracks: Vec<Value> = items.iter().filter(|it| data::kind(it) == "TRACK").map(|it| data::data(it).clone()).collect();
            let src = source.clone();
            c = c.child(items_view("va", &items, source, None, move |i| controller::send(Cmd::PlayFrom { tracks: tracks.clone(), index: i, source: src.clone(), album_mode: false })));
            c
        }
    }
}

/// Popular tracks: the artist's top tracks, paged in as the list scrolls; Play, Shuffle and a
/// click play them all (source "artist-tracks", which keeps the manual queue).
pub fn artist_tracks(s: &App, id: u64, name: &str, v: &View) -> El {
    let key = PKey::ArtistTracks(id);
    let snap = paged::get(&key);
    let source = json!({"type": "artist-tracks", "id": id, "name": name});
    let (k1, src1) = (key.clone(), source.clone());
    let mut c = col().w_full().gap(16.0)
        .child(col().gap(6.0).child(views::heading("Popular tracks", theme::PAGE_TITLE)).child(coll::dim(name)))
        .child(coll::play_pair(v, &source, move |shuffle| paged::play(k1.clone(), src1.clone(), shuffle, None, |_| true)));
    if snap.first_pending() {
        return c.child(crate::skeleton::rows(8, true));
    }
    if let Some(e) = snap.failed.as_ref().filter(|_| snap.len == 0) {
        let k = key.clone();
        return c.child(col().gap(10.0).items_start().child(ui::empty("Couldn't load tracks")).child(text(e.clone()).font(t(12.0, 400.0)).color(white(0.45)))
            .child(views::button::<App>("at-retry", "Retry", None, false).on_press(move |_: &mut App| paged::retry(&k))));
    }
    if snap.len == 0 && !snap.loading {
        return c.child(col().gap(6.0).child(text("No tracks available").font(t(15.0, 650.0)).color(white(0.9))).child(ui::empty("This artist doesn't have any popular tracks yet.")));
    }
    let n = snap.len;
    let (k2, src2) = (key.clone(), source.clone());
    let rows = views::track_list("at-t", snap.items.clone(), (0..snap.len).collect(), move |tracks, i| {
        let tr = paged::track_of(&tracks[i]);
        let tid = tr["id"].as_u64();
        let (k, src) = (k2.clone(), src2.clone());
        views::track_row(format!("at-t{i}"), i + 1, &tr, move |_: &mut App| paged::play(k.clone(), src.clone(), false, tid, |_| true))
    });
    c = c.child(rows.on_visible_rows(crate::playlist::more_near_end(key, n)));
    if snap.loading && snap.len > 0 {
        c = c.child(crate::skeleton::rows(3, true));
    }
    let _ = (s, row());
    c
}

#[cfg(test)]
mod tests {
    use super::typed;
    use crate::data;
    use serde_json::json;

    #[test]
    fn bare_items_get_their_kind() {
        assert_eq!(data::kind(&typed(&json!({"uuid": "u", "title": "P"}), None)), "PLAYLIST");
        assert_eq!(data::kind(&typed(&json!({"id": "m", "mixType": "ARTIST_MIX"}), None)), "MIX");
        assert_eq!(data::kind(&typed(&json!({"id": 1, "title": "A", "cover": "c", "releaseDate": "2020"}), None)), "ALBUM");
        assert_eq!(data::kind(&typed(&json!({"id": 2, "name": "N", "picture": "p"}), None)), "ARTIST");
        assert_eq!(data::kind(&typed(&json!({"id": 3, "title": "T", "duration": 9, "album": {}}), None)), "TRACK");
        assert_eq!(data::kind(&typed(&json!({"id": 4, "title": "V", "duration": 9, "imageId": "i"}), None)), "VIDEO");
        // A section's type names bare items; typed ones keep theirs.
        assert_eq!(data::kind(&typed(&json!({"id": 1, "title": "X"}), Some("ALBUM"))), "ALBUM");
        assert_eq!(data::kind(&typed(&json!({"type": "ARTIST", "data": {"id": 1}}), Some("ALBUM"))), "ARTIST");
    }
}
