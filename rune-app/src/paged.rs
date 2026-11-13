//! Collections that come a page at a time (React's infinite scroll + `useInfiniteScroll`):
//! playlist and Loved Tracks lists, Loved Videos, an artist's popular tracks and view-all
//! lists, and the library lists (sidebar and Show all, playlists and folders by cursor).
//!
//! The first page is fetched on first need with the same command and arguments the React view
//! used; further pages come when the view asks (`more`, from a list's visible rows or a
//! sentinel), or all at once (`all`, when a filter needs every item, as React's filter focus
//! does). The items live in one shared `Arc<Vec<Value>>` per collection, so a list of thousands
//! costs one copy, and each landing page bumps the catalogue version (data.rs), so the panes
//! rebuild as for any other data. A view being built sees only the pages that had landed when it
//! read the version (`data::freeze`), as with the catalogue cache.

use serde_json::Value;
use sone_host_shim::Manager;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use tauri_app_lib::AppState;
use tauri_app_lib::commands::{library, pages};

use crate::data;

/// A library list's kind (the sidebar's chips, React's `libraryType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lib {
    Playlists,
    Albums,
    Artists,
    Mixes,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PKey {
    /// A playlist's items, in the order asked for (None: the playlist's own order).
    Playlist { uuid: String, order: Option<String>, dir: Option<String> },
    /// Loved tracks (FavoritesView: DATE DESC unless sorted).
    Loved { order: String, dir: String },
    LovedVideos,
    /// An artist's popular tracks (ArtistTracksPage, the artist play source).
    ArtistTracks(u64),
    /// An artist page section's view-all list (ViewAllPage with an artist).
    ArtistViewAll(u64, String),
    /// A library list: the playlists and folders in `folder` ("root" at the top), or the
    /// favourite albums, artists or mixes; sorted as asked.
    Library { kind: Lib, folder: String, order: String, dir: String },
}

impl PKey {
    fn page_size(&self) -> u32 {
        match self {
            PKey::Playlist { .. } | PKey::Loved { .. } => 100,
            _ => 50,
        }
    }
}

/// What a collection showed as of one catalogue version: its item count and paging state.
#[derive(Clone, Debug, Default)]
struct Status {
    len: usize,
    total: Option<usize>,
    more: bool,
    loading: bool,
    failed: Option<String>,
}

struct Entry {
    items: Arc<Vec<Value>>,
    /// (catalogue version, status): each change as of the version it bumped to, so a view build
    /// sees the collection as of the version it read (`data::freeze`), and a region checked again
    /// in the same build agrees with itself.
    marks: Vec<(u64, Status)>,
    /// The list as it was before a `forget_where` at this version, for builds that read an
    /// earlier one.
    prev: Option<(u64, Arc<Vec<Value>>, Vec<(u64, Status)>)>,
    total: Option<usize>,
    /// The raw server offset the next page starts at.
    next: usize,
    cursor: Option<String>,
    more: bool,
    loading: bool,
    failed: Option<String>,
    /// Keep fetching until the end.
    all: bool,
    /// Which fetch owns this entry (a `forget` makes a running fetch's result stale).
    epoch: u64,
    /// The first page is still to be asked for (a `forget_where` left it for the next ask).
    idle: bool,
    /// Library and artist lists drop items they already have (React dedupes those pages).
    seen: HashSet<String>,
}

impl Entry {
    fn fresh(epoch: u64) -> Entry {
        Entry { items: Arc::new(vec![]), marks: vec![], prev: None, total: None, next: 0, cursor: None, more: true, loading: true, failed: None, all: false, epoch, idle: false, seen: HashSet::new() }
    }

    fn status(&self) -> Status {
        Status { len: self.items.len(), total: self.total, more: self.more, loading: self.loading, failed: self.failed.clone() }
    }

    /// Records the entry's state as of a new catalogue version (the panes rebuild for it).
    fn mark(&mut self) {
        let v = data::bump_version();
        let s = self.status();
        self.marks.push((v, s));
        if self.marks.len() > 64 {
            self.marks.drain(..self.marks.len() - 64);
        }
    }
}

static STORE: LazyLock<Mutex<HashMap<PKey, Entry>>> = LazyLock::new(Default::default);
static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_epoch() -> u64 {
    GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

/// A collection as a view sees it.
#[derive(Clone)]
pub struct Snap {
    pub items: Arc<Vec<Value>>,
    /// How many of `items` this view build may show (the rest landed after it read the version).
    pub len: usize,
    pub total: Option<usize>,
    pub more: bool,
    pub loading: bool,
    pub failed: Option<String>,
}

impl Snap {
    pub fn shown(&self) -> &[Value] {
        &self.items[..self.len]
    }
    /// Nothing is in yet: the first page is still coming (or failed).
    pub fn first_pending(&self) -> bool {
        self.len == 0 && (self.loading || self.more) && self.failed.is_none()
    }
}

/// The status as of version `v` among `marks`; before the first, the first page loading.
fn as_of(marks: &[(u64, Status)], v: u64) -> Status {
    marks.iter().rev().find(|(at, _)| *at <= v).map(|m| m.1.clone()).unwrap_or(Status { more: true, loading: true, ..Default::default() })
}

/// The collection, starting its first page if nobody asked for it yet.
pub fn get(key: &PKey) -> Snap {
    let mut st = STORE.lock().unwrap();
    let frozen = data::frozen_version();
    if let Some(e) = st.get(key)
        && let Some((at, items, marks)) = &e.prev
        && frozen.is_some_and(|v| v < *at)
    {
        let s = as_of(marks, frozen.unwrap_or(0));
        return Snap { items: items.clone(), len: s.len.min(items.len()), total: s.total, more: s.more, loading: s.loading, failed: s.failed };
    }
    let start = match st.get_mut(key) {
        None => {
            let epoch = next_epoch();
            st.insert(key.clone(), Entry::fresh(epoch));
            Some(epoch)
        }
        Some(e) if e.idle => {
            e.idle = false;
            Some(e.epoch)
        }
        Some(_) => None,
    };
    if let Some(epoch) = start {
        drop(st);
        spawn(key.clone(), epoch);
        st = STORE.lock().unwrap();
    }
    let e = &st[key];
    let s = match frozen {
        Some(v) => as_of(&e.marks, v),
        None => e.status(),
    };
    Snap { items: e.items.clone(), len: s.len.min(e.items.len()), total: s.total, more: s.more, loading: s.loading, failed: s.failed }
}

/// Starts `key` from a first page fetched elsewhere (the playlist folders the sign-in
/// bootstrap loads, which are the sidebar's first page), unless it's already there.
pub fn seed(key: &PKey, first: &Value) {
    let mut st = STORE.lock().unwrap();
    if st.contains_key(key) {
        return;
    }
    let items = first["items"].as_array().cloned().unwrap_or_default();
    let cursor = first["cursor"].as_str().filter(|c| !c.is_empty()).map(str::to_string);
    let more = cursor.is_some() && !items.is_empty();
    let seen = items.iter().filter_map(|v| ident(key, v)).collect();
    let n = items.len();
    let mut e = Entry::fresh(next_epoch());
    e.items = Arc::new(items);
    e.total = first["totalNumberOfItems"].as_u64().map(|t| t as usize);
    e.next = n;
    e.cursor = cursor;
    e.more = more;
    e.loading = false;
    e.seen = seen;
    // Visible to the build that seeds it: it read the version this data landed at.
    let v = data::frozen_version().unwrap_or_else(data::version);
    let s = e.status();
    e.marks.push((v, s));
    st.insert(key.clone(), e);
}

/// Ask for the next page, if there is one and none is on its way.
pub fn more(key: &PKey) {
    let mut st = STORE.lock().unwrap();
    let Some(e) = st.get_mut(key) else { drop(st); get(key); return };
    if e.loading || e.idle || !e.more || e.failed.is_some() {
        return;
    }
    e.loading = true;
    e.mark();
    let epoch = e.epoch;
    drop(st);
    spawn(key.clone(), epoch);
}

/// Fetch every remaining page (a filter that must see the whole collection).
pub fn all(key: &PKey) {
    {
        let mut st = STORE.lock().unwrap();
        if let Some(e) = st.get_mut(key) {
            if e.all {
                return;
            }
            e.all = true;
        }
    }
    more(key);
}

/// Start every collection `f` picks again from its first page, on its next ask (e.g. all
/// library lists after a folder changed). Builds that read an earlier version still see the
/// old list.
pub fn forget_where(f: impl Fn(&PKey) -> bool) {
    let mut st = STORE.lock().unwrap();
    let v = data::bump_version();
    for (_, e) in st.iter_mut().filter(|(k, _)| f(k)) {
        let old = std::mem::replace(e, Entry::fresh(next_epoch()));
        e.idle = true;
        e.prev = Some((v, old.items, old.marks));
    }
}


/// Everything loaded so far and where paging stands: (items, next raw offset, more to come),
/// for a play source that goes on fetching the rest.
pub fn loaded(key: &PKey) -> (Vec<Value>, usize, bool) {
    let st = STORE.lock().unwrap();
    match st.get(key) {
        Some(e) => (e.items.to_vec(), e.next, e.more),
        None => (vec![], 0, true),
    }
}

/// One page of `key` from raw offset `offset` (and `cursor`): its items, the total if the
/// server said, the next cursor, and whether more follow.
pub async fn fetch_page(key: &PKey, offset: usize, cursor: Option<String>) -> Result<(Vec<Value>, Option<usize>, Option<String>, bool), String> {
    let h = crate::host().clone();
    let st = || h.state::<AppState>();
    let limit = key.page_size();
    let off = offset as u32;
    fn to<T: serde::Serialize>(x: Result<T, tauri_app_lib::SoneError>) -> Result<Value, String> {
        x.map_err(|e| e.to_string()).and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
    }
    // A page with a total: more while short of it.
    let totaled = |v: Value| {
        let items = v["items"].as_array().cloned().unwrap_or_default();
        let total = v["totalNumberOfItems"].as_u64().map(|t| t as usize);
        let more = total.is_some_and(|t| offset + items.len() < t) && !items.is_empty();
        (items, total, None, more)
    };
    // A v2 page without a total: a full page means there's probably more.
    let full = |v: Value| {
        let items: Vec<Value> = v["items"].as_array().or(v.as_array()).cloned().unwrap_or_default();
        let more = items.len() >= limit as usize;
        (items, None, None, more)
    };
    Ok(match key {
        PKey::Playlist { uuid, order, dir } => totaled(to(library::get_playlist_tracks_page(st(), h.clone(), uuid.clone(), off, limit, order.clone(), dir.clone()).await)?),
        PKey::Loved { order, dir } => totaled(to(library::get_favorite_tracks(st(), h.clone(), data::user_id(), off, limit, order.clone(), dir.clone()).await)?),
        PKey::LovedVideos => {
            let v = to(library::get_favorite_videos(st(), data::user_id(), off, limit).await)?;
            let items = v.as_array().cloned().unwrap_or_default();
            let more = items.len() == limit as usize;
            (items, None, None, more)
        }
        PKey::ArtistTracks(id) => full(to(pages::get_artist_top_tracks_all(st(), h.clone(), *id, off, limit).await)?),
        PKey::ArtistViewAll(id, path) => full(to(pages::get_artist_view_all(st(), h.clone(), *id, path.clone(), off, limit).await)?),
        PKey::Library { kind, folder, order, dir } => match kind {
            Lib::Playlists => {
                // Cursor paging; the API's total is unreliable, so a cursor means more.
                let v = to(library::get_playlist_folders(st(), h.clone(), folder.clone(), Some(String::new()), off, limit, order.clone(), dir.clone(), Some(cursor.unwrap_or_default())).await)?;
                let items = v["items"].as_array().cloned().unwrap_or_default();
                let next = v["cursor"].as_str().filter(|c| !c.is_empty()).map(str::to_string);
                let total = v["totalNumberOfItems"].as_u64().map(|t| t as usize);
                let more = next.is_some() && !items.is_empty();
                (items, total, next, more)
            }
            Lib::Albums => totaled(to(library::get_favorite_albums(st(), h.clone(), data::user_id(), off, limit, order.clone(), dir.clone()).await)?),
            Lib::Artists => totaled(to(library::get_favorite_artists(st(), h.clone(), data::user_id(), off, limit, order.clone(), dir.clone()).await)?),
            Lib::Mixes => totaled(to(library::get_favorite_mixes(st(), h.clone(), off, limit, order.clone(), dir.clone()).await)?),
        },
    })
}

/// The identity a library or artist list dedupes by.
fn ident(key: &PKey, v: &Value) -> Option<String> {
    match key {
        PKey::Library { kind: Lib::Playlists, .. } => v["trn"].as_str().map(str::to_string).or_else(|| v["data"]["uuid"].as_str().map(str::to_string)),
        PKey::Library { .. } | PKey::ArtistTracks(_) | PKey::ArtistViewAll(..) => {
            let d = data::data(v);
            let d = if d.get("item").is_some_and(Value::is_object) { &d["item"] } else { d };
            Some(d["uuid"].as_str().map(str::to_string).unwrap_or_else(|| d["id"].to_string()))
        }
        _ => None,
    }
}

fn spawn(key: PKey, epoch: u64) {
    sone_host_shim::async_runtime::spawn(async move {
        loop {
            let (offset, cursor) = {
                let st = STORE.lock().unwrap();
                match st.get(&key) {
                    Some(e) if e.epoch == epoch => (e.next, e.cursor.clone()),
                    _ => return,
                }
            };
            let res = fetch_page(&key, offset, cursor).await;
            let mut st = STORE.lock().unwrap();
            let Some(e) = st.get_mut(&key).filter(|e| e.epoch == epoch) else { return };
            match res {
                Ok((items, total, cursor, more)) => {
                    let n = items.len();
                    let fresh: Vec<Value> = items.into_iter().filter(|v| match ident(&key, v) {
                        Some(id) => e.seen.insert(id),
                        None => true,
                    }).collect();
                    // A page of nothing but repeats ends a list the server keeps repeating.
                    let stalled = n > 0 && fresh.is_empty() && matches!(key, PKey::ArtistTracks(_) | PKey::ArtistViewAll(..));
                    Arc::make_mut(&mut e.items).extend(fresh);
                    e.next = offset + n;
                    e.cursor = cursor;
                    if total.is_some() {
                        e.total = total;
                    }
                    e.more = more && !stalled;
                    e.failed = None;
                    let done = !(e.all && e.more);
                    if done {
                        e.loading = false;
                    }
                    e.mark();
                    if done {
                        return;
                    }
                }
                Err(err) => {
                    log::warn!("[rune] page of {key:?} failed: {err}");
                    e.loading = false;
                    e.failed = Some(err);
                    e.mark();
                    return;
                }
            }
        }
    });
}

/// Lets a failed collection try again (its Retry).
pub fn retry(key: &PKey) {
    let mut st = STORE.lock().unwrap();
    let Some(e) = st.get_mut(key) else { return };
    if e.failed.take().is_some() {
        e.loading = true;
        e.mark();
        let epoch = e.epoch;
        drop(st);
        spawn(key.clone(), epoch);
    }
}

/// The whole collection, fetched to the end (for Play next / Add to queue / Add to playlist
/// on a long playlist, and for a play source's first batch).
pub async fn fetch_all(key: &PKey) -> Result<Vec<Value>, String> {
    let mut out = vec![];
    let mut offset = 0;
    let mut cursor = None;
    for _ in 0..400 {
        let (items, _, next, more) = fetch_page(key, offset, cursor).await?;
        offset += items.len();
        out.extend(items);
        cursor = next;
        if !more {
            break;
        }
    }
    Ok(out)
}

/// A track as the queue takes it: page items come bare, as `{item}` or as `{data}`.
pub fn track_of(v: &Value) -> Value {
    if v.get("item").is_some_and(Value::is_object) { v["item"].clone() } else { data::data(v).clone() }
}

/// An item of `key` as the queue takes it: a favourite video as a video queue item.
pub fn playable_of(key: &PKey, v: &Value) -> Value {
    if *key == PKey::LovedVideos { crate::video_ui::video_track(data::data(v)) } else { track_of(v) }
}

/// Plays a paged collection as usePlaySource does, through the controller's source player (the
/// one MCP play_source uses, sources.rs): from what is loaded, fetching more first only while
/// fewer than 50 playable tracks are in and the clicked track (`start_at`) isn't among them;
/// then the rest page by page, appended for as long as the queue is still this play's. The
/// content filter applies to all of it. `include` drops items (tracks un-loved this session).
pub fn play(key: PKey, source: Value, shuffle: bool, start_at: Option<u64>, include: fn(&Value) -> bool) {
    play_with(key, vec![], source, shuffle, start_at, include)
}

/// [`play`] with what the page already shows (an artist page's top tracks) as the first tracks
/// when nothing of the collection is loaded yet: ArtistPage's `loaded` with `nextOffset: 0`,
/// the pages' repeats dropped (dedupe); if the pages can't be fetched it plays those.
pub fn play_with(key: PKey, preview: Vec<Value>, source: Value, shuffle: bool, start_at: Option<u64>, include: fn(&Value) -> bool) {
    let (loaded, next, more) = loaded(&key);
    let (items, offset) = if loaded.is_empty() && !preview.is_empty() {
        (preview, 0)
    } else {
        (loaded.iter().map(|v| playable_of(&key, v)).collect(), next)
    };
    let dedupe = matches!(key, PKey::ArtistTracks(_) | PKey::ArtistViewAll(..));
    let more = more || items.is_empty();
    let mut l = crate::sources::Loader::with(source, items, Some(crate::sources::Pager::Paged(key)), offset as u32, more, dedupe, false, include);
    l.play_shuffle = shuffle;
    l.start_at = start_at;
    crate::controller::send(crate::controller::Cmd::PlaySource(Box::new(l)));
}

