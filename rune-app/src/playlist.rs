//! A playlist (React PlaylistView): header with the description and its "Read more", Play and
//! Shuffle over the whole playlist, Edit and Public/Private on your own (the heart on anyone
//! else's), Share and More; the filter (title, artist or album); the sortable track list,
//! paged in as it scrolls; and, once every track is in, the Recommended Tracks with their Add
//! and Refresh. The playlist editor (EditPlaylistModal) and the description dialog are here
//! too.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use rune::{El, TextEdit, Tx, col, el, row, spacer, text, text_input};
use serde_json::{Value, json};
use sone_host_shim::Manager;
use tauri_app_lib::AppState;
use tauri_app_lib::commands::library;

use crate::App;
use crate::coll::{self, Edit};
use crate::controller::{self, View};
use crate::data::{self, Key};
use crate::glyph::G;
use crate::interact::{self, Kind};
use crate::paged::{self, PKey};
use crate::theme;
use crate::ui::{self, t, white};
use crate::views::{self, header_btn};

/// Public/private set here, shown before the server's details catch up.
static ACCESS: LazyLock<Mutex<HashMap<String, bool>>> = LazyLock::new(Default::default);
/// Recommended tracks added this session (out of the list at once, back on failure).
static RECS_ADDED: LazyLock<Mutex<HashSet<(String, u64)>>> = LazyLock::new(Default::default);

/// Drops what this session set ahead of the server (Refresh: data::clear).
pub fn clear_overrides() {
    ACCESS.lock().unwrap().clear();
    RECS_ADDED.lock().unwrap().clear();
}

const RECS_PAGE: usize = 10;
const RECS_BATCH: u32 = 50;

pub fn key_for(s: &App, uuid: &str) -> PKey {
    let sort = s.nav.coll.ui.track_sort.get(uuid).cloned();
    PKey::Playlist { uuid: uuid.to_string(), order: sort.as_ref().map(|s| s.0.clone()), dir: sort.map(|s| s.1) }
}

/// Whether a track matches the filter as React's does: its title, any artist, or its album.
pub fn track_matches(tr: &Value, needle: &str) -> bool {
    let d = data::data(tr);
    data::matches(&format!("{} {} {}", data::name(tr), data::artist_of(tr), d["album"]["title"].as_str().unwrap_or("")), needle)
}

/// "N TRACKS" / "N TRACKS, M VIDEOS" (playlistCountLabel) and " (1 HR 5 MIN)".
fn count_line(tracks: u64, videos: u64, secs: Option<u64>) -> String {
    let mut s = coll::count(tracks as usize, "TRACK", "TRACKS");
    if videos > 0 {
        s = format!("{s}, {}", coll::count(videos as usize, "VIDEO", "VIDEOS"));
    }
    // formatTotalDuration: h:mm:ss or m:ss.
    if let Some(d) = secs.filter(|d| *d > 0) {
        let (h, m, sec) = (d / 3600, (d % 3600) / 60, d % 60);
        s = if h > 0 { format!("{s} ({h}:{m:02}:{sec:02})") } else { format!("{s} ({m}:{sec:02})") };
    }
    s
}

pub fn page(s: &App, uuid: &str, v: &View) -> El {
    let p = match views::ready_or(Key::Playlist(uuid.to_string())) { Ok(v) => v, Err(e) => return e };
    let d = &p["details"];
    let title = d["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Playlist").to_string();
    let key = key_for(s, uuid);
    let snap = paged::get(&key);
    let me = data::user_id();
    let own = d["creator"]["id"].as_u64().is_some_and(|c| c == me) || d["creator"]["id"].is_null();
    let source = json!({"type": "playlist", "id": uuid, "name": title});

    // Header
    let (k1, src1) = (key.clone(), source.clone());
    let mut actions = row().gap(8.0).items_center().child(coll::play_pair(v, &source, move |shuffle| paged::play(k1.clone(), src1.clone(), shuffle, None, |_| true)));
    if own {
        let public = ACCESS.lock().unwrap().get(uuid).copied().unwrap_or_else(|| is_public(d));
        let (u1, t1, desc1) = (uuid.to_string(), title.clone(), d["description"].as_str().unwrap_or("").to_string());
        let (u2, t2, desc2) = (u1.clone(), t1.clone(), desc1.clone());
        actions = actions
            .child(header_btn("pl-edit", "Edit playlist", "Edit", G::Pencil, false).on_press(move |s: &mut App| {
                s.nav.coll.edit = Some(Edit { uuid: u1.clone(), title: TextEdit::new(&t1), desc: TextEdit::new(&desc1), public, saving: false, error: None });
            }))
            .child(header_btn("pl-public", if public { "Make playlist private" } else { "Make playlist public" }, if public { "Public" } else { "Private" }, if public { G::Unlock } else { G::Lock }, public)
                .on_press(move |_: &mut App| set_access(u2.clone(), t2.clone(), desc2.clone(), !public)));
    } else {
        let (on, u, l) = (interact::is_fav(interact::Fav::Playlist, uuid), uuid.to_string(), title.clone());
        actions = actions.child(header_btn("pl-fav", if on { "Remove from favorites" } else { "Add to favorites" }, if on { "Added" } else { "Add" }, if on { G::HeartFilled } else { G::Heart }, on)
            .on_press(move |_: &mut App| interact::toggle_fav(interact::Fav::Playlist, u.clone(), l.clone(), interact::Say::Nothing)));
    }
    let share_uuid = uuid.to_string();
    let item = json!({"type": "PLAYLIST", "data": d.clone()});
    actions = actions
        .child(header_btn("pl-share", "Share playlist", "Share", G::Share, false).on_press_tx(move |_: &mut App, tx: &mut Tx| {
            tx.copy_rich(rune::ClipboardContent::new(format!("https://tidal.com/playlist/{share_uuid}")));
            interact::toast("Copied share link to clipboard".into(), Kind::Success);
        }))
        .child(header_btn("pl-more", "More options", "More", G::More, false)
            .on_press(move |s: &mut App| interact::open_media(s, item.clone(), interact::Anchor::Below("pl-more".into()))));
    let videos = d["numberOfVideos"].as_u64().unwrap_or(0);
    let total = snap.total.map(|t| t as u64).unwrap_or_else(|| d["numberOfTracks"].as_u64().unwrap_or(0) + videos);
    // PlaylistView: your own playlist names you (unless TIDAL calls you "TIDAL User"), others
    // their creator
    let me = match data::want(Key::Profile) {
        data::Load::Ready(p) => p.get(0).and_then(|n| n.as_str()).filter(|n| !n.is_empty() && *n != "TIDAL User").map(str::to_string),
        _ => None,
    };
    let creator = if own { me.unwrap_or_else(|| "You".to_string()) } else { d["creator"]["name"].as_str().map(str::to_string).unwrap_or_else(|| if d["creator"]["id"].as_u64() == Some(0) { "TIDAL".into() } else { String::new() }) };
    let line = count_line(total.saturating_sub(videos), videos, d["duration"].as_u64());
    let sub = if creator.is_empty() { line } else { format!("{creator} · {line}") };
    let mut sub_col = col().gap(6.0).min_w(0.0).child(coll::dim(&sub));
    if let Some(desc) = d["description"].as_str().filter(|x| !x.trim().is_empty()) {
        sub_col = sub_col.child(text(desc.to_string()).font(t(13.0, 400.0)).color(white(0.55)).wrap().max_lines(2).max_w(800.0));
        if desc.chars().count() > 120 {
            let (t2, d2) = (title.clone(), desc.to_string());
            sub_col = sub_col.child(theme::focusable(row().key("pl-desc-more").role("button").label("Read more")).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| s.nav.coll.description = Some((t2.clone(), d2.clone())))
                .child(text("Read more").font(t(12.0, 650.0)).color(white(0.85)).a11y_hidden()));
        }
    }
    let hero = views::hero_with(&title, true, sub_col, 30, false, crate::art::item_url(d, 180.0), actions);

    // Filter: every page comes in while it filters, so it covers the whole playlist.
    let needle = s.nav.filter.text.clone();
    if !needle.trim().is_empty() {
        paged::all(&key);
    }
    let filter = text_input(&s.nav.filter, t(13.0, 500.0).lh(18.0))
        .key("pl-filter").label("Filter playlist on title, artist or album").single_line().w(320.0)
        .color(white(0.95)).caret(white(0.95), white(0.25)).placeholder("Filter", white(0.4))
        .on_edit(|s: &mut App| &mut s.nav.filter);

    let mut c = col().w_full().gap(16.0).child(hero).child(row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).child(filter));
    if snap.first_pending() {
        return c.child(crate::skeleton::rows(8, true));
    }
    if let Some(e) = snap.failed.as_ref().filter(|_| snap.len == 0) {
        let k = key.clone();
        log::debug!("[rune] playlist tracks failed: {e}");
        return c.child(col().gap(10.0).items_start().child(ui::empty("Couldn't load this playlist's tracks"))
            .child(views::button::<App>("pl-retry", "Retry", None, false).on_press(move |_: &mut App| paged::retry(&k))));
    }
    let sort = s.nav.coll.ui.track_sort.get(uuid).cloned();
    let u3 = uuid.to_string();
    c = c.child(coll::sort_header("pl", sort.as_ref(), own, move |s: &mut App, sort| {
        match sort {
            Some(x) => { s.nav.coll.ui.track_sort.insert(u3.clone(), x); }
            None => { s.nav.coll.ui.track_sort.remove(&u3); }
        }
        coll::save(&s.nav.coll.ui);
    }));
    let shown: Vec<usize> = (0..snap.len).filter(|i| track_matches(&snap.items[*i], &needle)).collect();
    if shown.is_empty() && !snap.loading {
        return c.child(views::tracks_empty(s, "This playlist is empty"));
    }
    let n_shown = shown.len();
    let (uuid_s, sorted, k2, src2) = (uuid.to_string(), sort.is_some(), key.clone(), source.clone());
    let rows = views::track_list("pl-t", snap.items.clone(), shown, move |tracks, i| {
        let tr = paged::track_of(&tracks[i]);
        let id = tr["id"].as_u64();
        let (k, src) = (k2.clone(), src2.clone());
        // Remove from playlist goes by the item's place in the playlist's own order.
        let in_pl = (own && !sorted).then(|| interact::InPlaylist { uuid: uuid_s.clone(), index: i });
        views::track_row_look(format!("pl-t{i}"), i + 1, &tr, views::Look { date: own, ..Default::default() }, in_pl, move |_: &mut App| paged::play(k.clone(), src.clone(), false, id, |_| true))
    });
    c = c.child(rows.on_visible_rows(more_near_end(key.clone(), n_shown)));
    if snap.loading && snap.len > 0 {
        c = c.child(crate::skeleton::rows(3, true));
    }
    if !snap.more && !snap.loading {
        c = c.child(recommended(s, uuid, &title));
    }
    c
}

fn is_public(d: &Value) -> bool {
    d["accessType"].as_str() == Some("PUBLIC") || d["publicPlaylist"].as_bool() == Some(true)
}

/// Make it public or private (handleToggleAccess): shows at once, then saves; put back if the
/// server refuses.
fn set_access(uuid: String, title: String, desc: String, public: bool) {
    ACCESS.lock().unwrap().insert(uuid.clone(), public);
    data::bump_version();
    sone_host_shim::async_runtime::spawn(async move {
        let st = crate::host().state::<AppState>();
        match library::update_playlist(st, uuid.clone(), title, desc, (if public { "PUBLIC" } else { "UNLISTED" }).into()).await {
            Ok(_) => {
                data::forget(&Key::Playlist(uuid));
                interact::toast((if public { "Playlist is now public" } else { "Playlist is now private" }).into(), Kind::Success);
            }
            Err(e) => {
                log::warn!("[rune] playlist access: {e}");
                ACCESS.lock().unwrap().insert(uuid, !public);
                data::bump_version();
            }
        }
    });
}

/// Recommended Tracks: ten at a time from a batch of 50; Refresh shows the next ten, then the
/// next batch, wrapping to the first when the server has no more. Each has "Add to this
/// playlist"; a click plays the ten shown.
fn recommended(s: &App, uuid: &str, title: &str) -> El {
    let (page, offset) = match &s.nav.coll.ui.recs {
        Some((u, p, o)) if u == uuid => (*p, *o),
        _ => (0, 0),
    };
    let batch: Vec<Value> = if offset == 0 {
        data::peek(&Key::Playlist(uuid.to_string())).map(|p| p["recommendations"]["items"].as_array().cloned().unwrap_or_default()).unwrap_or_default()
    } else {
        match data::want(Key::PlaylistRecs(uuid.to_string(), offset as u32)) {
            data::Load::Ready(v) => v["items"].as_array().cloned().unwrap_or_default(),
            _ => vec![],
        }
    };
    let added = RECS_ADDED.lock().unwrap();
    let pool: Vec<Value> = batch.into_iter().map(|t| paged::track_of(&t)).filter(|t| !added.contains(&(uuid.to_string(), t["id"].as_u64().unwrap_or(0)))).collect();
    drop(added);
    let visible: Vec<Value> = pool.iter().skip(page * RECS_PAGE).take(RECS_PAGE).cloned().collect();
    // PlaylistView shows them under any playlist once all its tracks are in; adding goes to
    // this playlist, which the server refuses on someone else's (a failure toast, as React).
    if visible.is_empty() {
        return el();
    }
    let n_pool = pool.len();
    let source = json!({"type": "playlist-recs", "id": uuid, "name": format!("{title}: Recommended")});
    let list = std::rc::Rc::new(visible);
    let mut rows = col().w_full().gap(2.0);
    for (i, tr) in list.iter().enumerate() {
        let (all, src) = (list.clone(), source.clone());
        let (u, tr2) = (uuid.to_string(), tr.clone());
        // TrackList keeps the heart and More on these rows and swaps only + for "Add to this
        // playlist".
        let look = views::Look { add_here: Some(std::rc::Rc::new(move |_: &mut App| add_rec(u.clone(), tr2.clone()))), ..Default::default() };
        rows = rows.child(views::track_row_look(format!("pl-rec{i}"), i + 1, tr, look, None, move |_: &mut App| {
            controller::send(controller::Cmd::PlayFrom { tracks: all.to_vec(), index: i, source: src.clone(), album_mode: false });
        }));
    }
    let u = uuid.to_string();
    col().w_full().gap(12.0).pt(24.0)
        .child(views::heading("Recommended Tracks", theme::SECTION))
        .child(rows)
        .child(row().w_full().child(spacer()).child(views::button::<App>("pl-rec-refresh", "Refresh", Some(G::Repeat), false).on_press(move |s: &mut App| {
            let next = page + 1;
            s.nav.coll.ui.recs = Some(if next * RECS_PAGE < n_pool {
                (u.clone(), next, offset)
            } else {
                (u.clone(), 0, offset + RECS_BATCH as usize)
            });
        })))
        .child(wrap_empty_batch(s, uuid, offset))
}

/// A Refresh whose next batch came back empty starts again from the first (handleRefreshRecs).
fn wrap_empty_batch(_s: &App, uuid: &str, offset: usize) -> El {
    if offset > 0
        && let Some(v) = data::peek(&Key::PlaylistRecs(uuid.to_string(), offset as u32))
        && v["items"].as_array().is_none_or(|a| a.is_empty())
    {
        let u = uuid.to_string();
        coll::later(move |s| s.nav.coll.ui.recs = Some((u, 0, 0)));
    }
    el()
}

fn add_rec(uuid: String, track: Value) {
    let id = track["id"].as_u64().unwrap_or(0);
    let title = data::name(&track);
    RECS_ADDED.lock().unwrap().insert((uuid.clone(), id));
    data::bump_version();
    sone_host_shim::async_runtime::spawn(async move {
        match library::add_track_to_playlist(crate::host().state::<AppState>(), uuid.clone(), id).await {
            Ok(()) => {
                paged::forget_where(|k| matches!(k, PKey::Playlist { uuid: u, .. } if *u == uuid));
                interact::toast(format!("Added \"{title}\" to playlist"), Kind::Success);
            }
            Err(e) => {
                log::warn!("[rune] add recommended track: {e}");
                RECS_ADDED.lock().unwrap().remove(&(uuid, id));
                data::bump_version();
                interact::toast("Failed to add track".into(), Kind::Error);
            }
        }
    });
}

/// The playlist editor and the description dialog.
pub fn overlay(s: &App) -> Vec<El> {
    let mut out = vec![];
    if let Some((title, desc)) = &s.nav.coll.description {
        let mut body = col().w_full().gap(14.0);
        for p in desc.split('\n').filter(|p| !p.trim().is_empty()) {
            body = body.child(text(p.to_string()).font(t(14.0, 400.0).lh(24.0)).color(white(0.75)).wrap());
        }
        let head = col().gap(2.0).child(views::heading(title, 16.0)).child(text("Description").font(t(13.0, 400.0)).color(white(0.5)));
        out.push(ui::dialog_frame("pl-desc-layer", "pl-desc", "Description", ("pl-desc-x", "Close dialog"), 700.0, None, Some(head), body, |s: &mut App| s.nav.coll.description = None));
    }
    if let Some(e) = &s.nav.coll.edit {
        let can = !e.title.text.trim().is_empty();
        let body = col().gap(14.0)
            .child(coll::field(&e.title, "ep-title", "Title", |s| &mut s.nav.coll.edit.as_mut().expect("the field shows with its dialog").title, save_edit))
            .child(col().gap(4.0)
                .child(row().h(88.0).px(12.0).py(10.0).radius(10.0).fill(white(0.06))
                    .child(text_input(&e.desc, t(13.0, 400.0).lh(18.0)).key("ep-desc").label("Write a description").described_by("ep-desc-count").grow(1.0).color(white(0.9))
                        .caret(white(0.95), white(0.25)).placeholder("Write a description", white(0.4))
                        .on_edit(|s: &mut App| &mut s.nav.coll.edit.as_mut().expect("the field shows with its dialog").desc)))
                .child(text(format!("{}/500 characters", e.desc.text.chars().count().min(500))).key("ep-desc-count").font(t(11.0, 400.0)).color(white(0.45))))
            .child(theme::focusable(row().key("ep-public").role("switch").label("Make it public")).described_by("ep-public-note").checked(e.public).gap(10.0).items_center().cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut App| if let Some(e) = &mut s.nav.coll.edit { e.public = !e.public })
                .child(col().grow(1.0).a11y_hidden()
                    .child(text("Make it public").font(t(14.0, 600.0)).color(white(0.92)))
                    .child(text("Your playlist will be visible on your Profile and accessible by anyone.").key("ep-public-note").font(t(12.0, 400.0)).color(white(0.5)).wrap()))
                .child(ui::switch_mark(e.public)))
            .child(coll::error_line(&e.error))
            .child(row().child(spacer()).child(coll::save_button("ep-save", "Save", can, e.saving, save_edit)));
        out.push(ui::dialog_frame("ep-layer", "ep-dialog", "Edit playlist", ("ep-x", "Close"), 440.0, None, Some(ui::dialog_title("Edit playlist")), body, |s: &mut App| s.nav.coll.edit = None));
    }
    out
}

fn save_edit(s: &mut App) {
    let Some(e) = &mut s.nav.coll.edit else { return };
    let title = e.title.text.trim().to_string();
    if title.is_empty() || e.saving {
        return;
    }
    e.saving = true;
    e.error = None;
    let (uuid, desc, public) = (e.uuid.clone(), e.desc.text.chars().take(500).collect::<String>(), e.public);
    sone_host_shim::async_runtime::spawn(async move {
        let st = crate::host().state::<AppState>();
        match library::update_playlist(st, uuid.clone(), title.clone(), desc, (if public { "PUBLIC" } else { "UNLISTED" }).into()).await {
            Ok(_) => {
                ACCESS.lock().unwrap().insert(uuid.clone(), public);
                data::forget(&Key::Playlist(uuid));
                data::forget(&Key::Folders);
                data::forget(&Key::AllPlaylists);
                interact::toast(format!("Updated playlist \"{title}\""), Kind::Success);
                coll::later(|s| s.nav.coll.edit = None);
            }
            Err(err) => {
                log::warn!("[rune] update playlist: {err}");
                coll::later(|s| if let Some(e) = &mut s.nav.coll.edit {
                    e.saving = false;
                    e.error = Some("Failed to update playlist".into());
                });
            }
        }
    });
}

/// React's sentinel at a list's end: the next page once the visible rows come within a screen
/// or so of the last of `n` rows. Only bookkeeping for the view (the page lands later).
pub fn more_near_end(key: PKey, n: usize) -> impl Fn(&mut App, std::ops::Range<usize>) -> bool + 'static {
    move |_: &mut App, r: std::ops::Range<usize>| {
        if r.end + 30 >= n {
            paged::more(&key);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{count_line, track_matches};
    use serde_json::json;

    #[test]
    fn filter_matches_title_artist_or_album() {
        let t = json!({"title": "Song", "artists": [{"name": "Band"}], "album": {"title": "Record"}});
        assert!(track_matches(&t, "song"));
        assert!(track_matches(&t, "band"));
        assert!(track_matches(&t, "record"));
        assert!(!track_matches(&t, "nothing"));
    }

    #[test]
    fn counts_and_durations_read_as_react() {
        assert_eq!(count_line(4, 0, Some(36)), "4 TRACKS (0:36)");
        assert_eq!(count_line(1, 2, Some(3725)), "1 TRACK, 2 VIDEOS (1:02:05)");
        assert_eq!(count_line(0, 0, None), "0 TRACKS");
    }
}
