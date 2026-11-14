//! Catalogue data for the views: one cache entry per backend call, fetched on first need.
//!
//! Each key calls the same backend command, with the same arguments, that the React view
//! invoked, so the requests on the wire match the reference build (the verifier compares
//! them step by step). Results are kept as JSON, which is what the React views rendered from.

use serde_json::Value;
use sone_host_shim::Manager;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri_app_lib::AppState;
use tauri_app_lib::commands::{library, metadata, pages, search};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A Home feed tab by slug ("static" is For you, "editorial" Staff Picks, …).
    Home(String),
    /// The next page of a Home tab's sections, by the cursor the previous page ended with.
    HomeMore(String, String),
    Folders,
    Suggest(String),
    Search(String),
    Album(u64),
    Artist(u64),
    ArtistTopTracks(u64),
    Playlist(String),
    FavAlbums,
    FavArtists,
    FavMixes,
    Profile,
    Lyrics(u64),
    /// A TIDAL page by api path (Explore: "pages/explore", genre pages, view-all pages).
    Page(String),
    Feed,
    Credits(u64),
    ArtistBio(u64),
    Mix(String),
    /// Every playlist of the user, for the Add to playlist submenu (React getAllPlaylists).
    AllPlaylists,
    /// The signed-in user's full profile (React getProfile): ProfilePage and the account avatar.
    UserProfile,
    /// A further batch of a playlist's recommended tracks (its Refresh), by api offset.
    PlaylistRecs(String, u32),
    /// Every playlist in every folder (LibraryViewAll's filter: getFlattenedPlaylists).
    FlattenedPlaylists,
    /// Every top-level folder, all pages (MoveToFolderMenu).
    AllFolders,
}

#[derive(Clone)]
pub enum Load {
    Loading,
    Ready(Value),
    Failed(String),
}

/// A cache entry: its state, the fetch that owns it, when it got there, and how many automatic
/// retries of a failure it has had.
struct Entry {
    load: Load,
    fetch: u64,
    at: Instant,
    retries: usize,
    /// `nudge_due_retries` has bumped the version for this failure's retry.
    nudged: bool,
    /// The version whose bump made it visible (0 for a fetch just started, which needs none).
    version: u64,
    /// A background fetch still waiting for its turn (sched.rs): asking for the key in the
    /// foreground sends it at once.
    promote: Option<crate::sched::Promote>,
    /// Out of date after an edit ([`refresh`]): the next ask fetches it again, showing this
    /// value until the new one lands.
    outdated: bool,
}

impl Entry {
    fn landed(load: Load, fetch: u64, retries: usize, version: u64) -> Entry {
        Entry { load, fetch, at: Instant::now(), retries, nudged: false, version, promote: None, outdated: false }
    }
}

/// Which lane a fetch goes in (sched.rs).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lane {
    /// The page on screen needs it, or a click asked for it.
    Fg,
    /// Wanted later: a Home tab not showing, the feed's unread dot, the account's name.
    Bg,
}

static CACHE: LazyLock<Mutex<HashMap<Key, Entry>>> = LazyLock::new(Default::default);
static VERSION: AtomicU64 = AtomicU64::new(0);
static FETCHES: AtomicU64 = AtomicU64::new(0);
/// A failed entry is fetched again when asked this long after each failure (offline, a
/// timeout), then left failed until `forget` (views redraw every second while music plays, and
/// some treat a failure as an answer, e.g. a track without lyrics).
const RETRY_FAILED_AFTER: [Duration; 3] = [Duration::from_secs(5), Duration::from_secs(20), Duration::from_secs(60)];

pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

thread_local! {
    /// The version the view building now read (`freeze`).
    static FROZEN: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// For one view build: the version now, and until the guard drops, `want` and `peek` on this
/// thread answer as of that version, so a fetch landing mid-build (on another thread) shows
/// from the next build on. The panes are regions keyed by this version: without it a region
/// reused for the version it read could differ from building it again a moment later.
pub fn freeze() -> (u64, Frozen) {
    let v = version();
    FROZEN.set(Some(v));
    (v, Frozen(()))
}

/// Ends [`freeze`] when dropped.
pub struct Frozen(());

impl Drop for Frozen {
    fn drop(&mut self) {
        FROZEN.set(None);
    }
}

/// Bumps the catalogue version and returns the new one: for stores beside this cache (paged.rs)
/// whose changes the panes must see, as they see this cache's.
pub fn bump_version() -> u64 {
    VERSION.fetch_add(1, Ordering::Relaxed) + 1
}

/// The version the view being built read ([`freeze`]), while it builds.
pub fn frozen_version() -> Option<u64> {
    FROZEN.get()
}

fn after_frozen(version: u64) -> bool {
    FROZEN.get().is_some_and(|f| version > f)
}

/// Bumps the version once for each failed entry whose retry has come due, so the view asks for
/// it again: the panes are regions, which build (and ask) only when something they read changed,
/// not every second as the whole view used to while music played. Called by main::poll.
pub fn nudge_due_retries() {
    let mut due = false;
    for e in CACHE.lock().unwrap().values_mut() {
        if !e.nudged && matches!(e.load, Load::Failed(_)) && RETRY_FAILED_AFTER.get(e.retries).is_some_and(|d| e.at.elapsed() >= *d) {
            e.nudged = true;
            due = true;
        }
    }
    if due {
        VERSION.fetch_add(1, Ordering::Relaxed);
    }
}

/// The cached value, starting the fetch if this is the first ask (or a failure has cooled down).
/// The fetch goes at once: the caller shows it now. A background fetch of the
/// same key still waiting for a slot goes at once too.
pub fn want(key: Key) -> Load {
    want_in(key, Lane::Fg)
}

/// [`want`] for something not on screen yet (a Home tab not showing, the feed's unread dot):
/// the fetch goes as background work (sched.rs: at most two at once).
pub fn want_bg(key: Key) -> Load {
    want_in(key, Lane::Bg)
}

/// What an ask for a key does, given its cache entry.
#[derive(Debug, PartialEq)]
enum Ask {
    /// Answer from the entry (in, failed and not due again, or on its way): no request.
    Answer,
    /// Waiting for a background slot, asked for by the page on screen: send it now.
    Promote,
    /// Fetch it (never asked, or a failure whose retry is due), as the nth retry.
    Fetch(usize),
    /// In but out of date after an edit: answer the old value and fetch it again behind it.
    Revalidate,
}

fn ask(e: Option<&Entry>, lane: Lane) -> Ask {
    let Some(e) = e else { return Ask::Fetch(0) };
    match &e.load {
        Load::Ready(_) if e.outdated => Ask::Revalidate,
        Load::Loading if lane == Lane::Fg && e.promote.is_some() => Ask::Promote,
        Load::Failed(_) if RETRY_FAILED_AFTER.get(e.retries).is_some_and(|d| e.at.elapsed() >= *d) => Ask::Fetch(e.retries + 1),
        _ => Ask::Answer,
    }
}

fn want_in(key: Key, lane: Lane) -> Load {
    let mut c = CACHE.lock().unwrap();
    let retries = match ask(c.get(&key), lane) {
        Ask::Fetch(n) => n,
        a => {
            if a == Ask::Revalidate {
                revalidate(&mut c, &key);
            }
            let e = &c[&key];
            if a == Ask::Promote && let Some(p) = &e.promote {
                crate::sched::promote(p);
            }
            // Landed after the view read the version: it was loading as far as this view knows.
            return if after_frozen(e.version) { Load::Loading } else { e.load.clone() };
        }
    };
    // (a due failure that landed after the view read the version is still loading for it)
    if c.get(&key).is_some_and(|e| after_frozen(e.version)) {
        return Load::Loading;
    }
    let fetch_id = FETCHES.fetch_add(1, Ordering::Relaxed) + 1;
    let promote = (lane == Lane::Bg).then(crate::sched::Promote::new);
    c.insert(key.clone(), Entry { promote: promote.clone(), ..Entry::landed(Load::Loading, fetch_id, retries, 0) });
    drop(c);
    // (a background slot is held until the answer is in the cache, so the next background
    // request can use it: the account name reads the profile's)
    let work = async move {
        let load = match fetch(&key).await {
            Ok(v) => Load::Ready(v),
            Err(e) => {
                log::warn!("[rune] data fetch failed: {e}");
                Load::Failed(e)
            }
        };
        // Only if the entry is still ours: a `forget` (or a newer fetch) since means this
        // result is older than what was asked for.
        let mut c = CACHE.lock().unwrap();
        if c.get(&key).is_some_and(|e| e.fetch == fetch_id) {
            let version = VERSION.fetch_add(1, Ordering::Relaxed) + 1;
            c.insert(key, Entry::landed(load, fetch_id, retries, version));
        }
        drop(c);
        LANDED.notify_waiters();
    };
    sone_host_shim::async_runtime::spawn(async move {
        match &promote {
            Some(p) => crate::sched::bg(Some(p), work).await,
            None => work.await,
        }
    });
    Load::Loading
}

/// Woken whenever a fetch lands (for [`get`] waiting on one already running).
static LANDED: LazyLock<tokio::sync::Notify> = LazyLock::new(tokio::sync::Notify::new);

/// The cached value if it's there, without starting a fetch (for views that must not request
/// on render, e.g. debounced suggestions).
pub fn peek(key: &Key) -> Option<Value> {
    match CACHE.lock().unwrap().get(key) {
        Some(Entry { load: Load::Ready(v), version, .. }) if !after_frozen(*version) => Some(v.clone()),
        _ => None,
    }
}

/// Where the key's fetch stands, without starting one: None when it was never asked for. One
/// that landed after the view read the version is still loading for that view.
pub fn peek_load(key: &Key) -> Option<Load> {
    match CACHE.lock().unwrap().get(key) {
        Some(e) if after_frozen(e.version) => Some(Load::Loading),
        Some(e) => Some(e.load.clone()),
        None => None,
    }
}

pub fn ready(key: Key) -> Option<Value> {
    match want(key) {
        Load::Ready(v) => Some(v),
        _ => None,
    }
}

/// Awaitable fetch for the controller (e.g. an artist's top tracks before playing them). A
/// fetch of the key already running (a page asked for it) is waited for, not
/// made again.
pub async fn get(key: Key) -> Result<Value, String> {
    // (an earlier failure is asked again at once: the user is waiting on this)
    {
        let mut c = CACHE.lock().unwrap();
        if c.get(&key).is_some_and(|e| matches!(e.load, Load::Failed(_))) {
            c.remove(&key);
        }
    }
    loop {
        let landed = LANDED.notified();
        tokio::pin!(landed);
        landed.as_mut().enable();
        match want(key.clone()) {
            Load::Ready(v) => return Ok(v),
            Load::Failed(e) => return Err(e),
            Load::Loading => {
                if tokio::time::timeout(Duration::from_secs(60), landed).await.is_err() {
                    return Err("timed out".into());
                }
            }
        }
    }
}

/// Put `v` in the cache as `key`'s value, as if it had just been fetched: a refresh that kept
/// the old value showing until the new one came. A fetch of `key` still running is then stale.
pub fn replace(key: Key, v: Value) {
    let fetch_id = FETCHES.fetch_add(1, Ordering::Relaxed) + 1;
    let mut c = CACHE.lock().unwrap();
    let version = VERSION.fetch_add(1, Ordering::Relaxed) + 1;
    c.insert(key, Entry::landed(Load::Ready(v), fetch_id, 0, version));
    drop(c);
    LANDED.notify_waiters();
}

/// How long ago `key`'s value landed, if it's there.
pub fn age(key: &Key) -> Option<Duration> {
    match CACHE.lock().unwrap().get(key) {
        Some(Entry { load: Load::Ready(_), at, .. }) => Some(at.elapsed()),
        _ => None,
    }
}

/// Forget a cached entry so the next ask fetches again (e.g. after signing in).
pub fn forget(key: &Key) {
    // A playlist forgotten after an edit (a track added or removed) takes its paged tracks
    // with it, and the playlist folders take the library lists.
    match key {
        Key::Playlist(u) => {
            crate::paged::forget_where(|k| matches!(k, crate::paged::PKey::Playlist { uuid, .. } if uuid == u));
            CACHE.lock().unwrap().remove(&Key::PlaylistRecs(u.clone(), 0));
        }
        Key::Folders => crate::paged::forget_where(|k| matches!(k, crate::paged::PKey::Library { kind: crate::paged::Lib::Playlists, .. })),
        Key::FavAlbums => crate::paged::forget_where(|k| matches!(k, crate::paged::PKey::Library { kind: crate::paged::Lib::Albums, .. })),
        Key::FavArtists => crate::paged::forget_where(|k| matches!(k, crate::paged::PKey::Library { kind: crate::paged::Lib::Artists, .. })),
        Key::FavMixes => crate::paged::forget_where(|k| matches!(k, crate::paged::PKey::Library { kind: crate::paged::Lib::Mixes, .. })),
        _ => {}
    }
    CACHE.lock().unwrap().remove(key);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Fetch `key` again after an edit, keeping what it shows on screen until the new answer lands
/// (no flash back to a skeleton); one not loaded is forgotten as [`forget`] does. A playlist
/// takes its first batch of recommendations and its paged tracks with it, as with `forget`.
pub fn refresh(key: &Key) {
    if let Key::Playlist(u) = key {
        crate::paged::refresh_where(|k| matches!(k, crate::paged::PKey::Playlist { uuid, .. } if uuid == u));
        refresh_one(&Key::PlaylistRecs(u.clone(), 0));
    }
    refresh_one(key);
}

/// [`refresh`] without a playlist's paged tracks (which a removal already patched in place).
pub fn refresh_entry(key: &Key) {
    if let Key::Playlist(u) = key {
        refresh_one(&Key::PlaylistRecs(u.clone(), 0));
    }
    refresh_one(key);
}

/// Marks a loaded entry out of date: the next ask fetches it again and gets the old value until
/// the new one lands (so nothing is fetched for a page nobody opens). One not loaded is
/// forgotten.
fn refresh_one(key: &Key) {
    let mut c = CACHE.lock().unwrap();
    match c.get_mut(key) {
        Some(e) if matches!(e.load, Load::Ready(_)) => e.outdated = true,
        _ => {
            c.remove(key);
        }
    }
    drop(c);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Fetches an out-of-date entry again, leaving its value in place until the
/// new one lands (a failure keeps the old value).
fn revalidate(c: &mut HashMap<Key, Entry>, key: &Key) {
    let fetch_id = FETCHES.fetch_add(1, Ordering::Relaxed) + 1;
    if let Some(e) = c.get_mut(key) {
        e.outdated = false;
        // (a fetch still running for the old value no longer owns the entry)
        e.fetch = fetch_id;
    }
    let key = key.clone();
    sone_host_shim::async_runtime::spawn(async move {
        let res = fetch(&key).await;
        let mut c = CACHE.lock().unwrap();
        if c.get(&key).is_some_and(|e| e.fetch == fetch_id) {
            match res {
                Ok(v) => {
                    let version = VERSION.fetch_add(1, Ordering::Relaxed) + 1;
                    c.insert(key, Entry::landed(Load::Ready(v), fetch_id, 0, version));
                }
                Err(e) => log::warn!("[rune] refresh of {key:?} failed: {e}"),
            }
        }
        drop(c);
        LANDED.notify_waiters();
    });
}

/// [`forget`] this entry alone: a playlist's details without its paged tracks (which a
/// removal already patched in place).
pub fn forget_entry(key: &Key) {
    CACHE.lock().unwrap().remove(key);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Forget every cached entry, so everything shown is fetched again (Refresh app data).
pub fn clear() {
    // The paged collections and the playlist page's local overrides go too (paged.rs,
    // playlist.rs), so Refresh fetches every list again.
    crate::paged::forget_where(|_| true);
    crate::playlist::clear_overrides();
    CACHE.lock().unwrap().clear();
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// The signed-in user's id once known (0 before): read from the saved settings once, then kept,
/// so a settings file caught mid-write (it is rewritten in place) can't turn it into 0, and view
/// builds don't decrypt the file every time they ask.
static USER_ID: AtomicU64 = AtomicU64::new(0);

/// The signed-in user's id, or 0 while it isn't known (signed out, or not yet loaded).
pub fn user_id() -> u64 {
    let known = USER_ID.load(Ordering::Relaxed);
    if known != 0 {
        return known;
    }
    let id = crate::host().state::<AppState>().load_settings().and_then(|s| s.auth_tokens).and_then(|t| t.user_id).unwrap_or(0);
    if id != 0 {
        USER_ID.store(id, Ordering::Relaxed);
    }
    id
}

/// Signed out: the next account's id is read afresh.
pub fn forget_user_id() {
    USER_ID.store(0, Ordering::Relaxed);
}

/// [`user_id`] for a request that needs it: waits (up to 20 s) until it is known rather than
/// asking TIDAL about user 0.
pub async fn uid() -> Result<u64, String> {
    for _ in 0..400 {
        match user_id() {
            0 => tokio::time::sleep(Duration::from_millis(50)).await,
            id => return Ok(id),
        }
    }
    Err("not signed in".into())
}

async fn fetch(key: &Key) -> Result<Value, String> {
    let h = crate::host().clone();
    let st = || h.state::<AppState>();
    let j = |r: Result<Value, String>| r;
    macro_rules! v {
        ($e:expr) => {
            $e.await.map_err(|e| crate::proxy_banner::checked(e.to_string())).and_then(|x| serde_json::to_value(x).map_err(|e| e.to_string()))
        };
    }
    j(match key {
        Key::Home(slug) => v!(pages::get_home_page(st(), h.clone(), Some(slug.clone()))),
        Key::HomeMore(slug, cursor) => v!(pages::get_home_page_more(st(), Some(slug.clone()), cursor.clone())),
        Key::Folders => v!(library::get_playlist_folders(st(), h.clone(), "root".into(), None, 0, 50, "DATE_UPDATED".into(), "DESC".into(), None)),
        Key::Suggest(q) => v!(search::get_suggestions(st(), q.clone(), 10)),
        Key::Search(q) => v!(search::search_tidal(st(), q.clone(), 50)),
        Key::Album(id) => v!(pages::get_album_page(st(), h.clone(), *id)),
        Key::Artist(id) => v!(pages::get_artist_page(st(), h.clone(), *id)),
        Key::ArtistTopTracks(id) => v!(pages::get_artist_top_tracks_all(st(), h.clone(), *id, 0, 50)),
        // The React playlist page fires this with its first page of tracks (paged.rs) and its
        // recommendations (`PlaylistRecs(uuid, 0)`); each shows as it lands.
        Key::Playlist(uuid) => {
            let d = metadata::get_playlist_details(st(), uuid.clone()).await.map_err(|e| crate::proxy_banner::checked(e.to_string()))?;
            Ok(serde_json::json!({"details": d}))
        }
        Key::FlattenedPlaylists => v!(library::get_all_flattened_playlists(st())),
        Key::AllFolders => {
            let mut out = vec![];
            let mut cursor = String::new();
            for _ in 0..40 {
                let v = library::get_playlist_folders(st(), h.clone(), "root".into(), Some(String::new()), 0, 50, "DATE_UPDATED".into(), "DESC".into(), Some(cursor.clone())).await.map_err(|e| e.to_string())?;
                out.extend(v["items"].as_array().cloned().unwrap_or_default().into_iter().filter(|it| it["itemType"].as_str() == Some("FOLDER")));
                match v["cursor"].as_str().filter(|c| !c.is_empty()) {
                    Some(c) => cursor = c.to_string(),
                    None => break,
                }
            }
            Ok(Value::Array(out))
        }
        Key::PlaylistRecs(uuid, offset) => v!(library::get_playlist_recommendations(st(), uuid.clone(), *offset, 50)),
        Key::FavAlbums => v!(library::get_favorite_albums(st(), h.clone(), uid().await?, 0, 50, "DATE".into(), "DESC".into())),
        Key::FavArtists => v!(library::get_favorite_artists(st(), h.clone(), uid().await?, 0, 50, "DATE".into(), "DESC".into())),
        Key::FavMixes => v!(library::get_favorite_mixes(st(), h.clone(), 0, 50, "DATE".into(), "DESC".into())),
        Key::Profile => match peek(&Key::UserProfile) {
            // The full profile of an account without an artist profile was built from the very
            // request get_user_profile makes (GET /users/{id}), with the same name and handle:
            // no need to make it a third time.
            Some(p) if p["artistId"].is_null() && p["name"].is_string() => Ok(serde_json::json!([p["name"], p["handle"]])),
            _ => v!(tauri_app_lib::commands::auth::get_user_profile(st(), uid().await?)),
        },
        Key::Lyrics(id) => v!(metadata::get_track_lyrics(st(), *id)),
        Key::Page(path) => v!(pages::get_page_section(st(), h.clone(), path.clone())),
        Key::Feed => v!(tauri_app_lib::commands::feed::get_feed(st(), uid().await?)),
        Key::Credits(id) => v!(metadata::get_track_credits(st(), *id)),
        Key::ArtistBio(id) => v!(pages::get_artist_bio(st(), h.clone(), *id)),
        Key::Mix(id) => v!(pages::get_mix_items(st(), id.clone())),
        Key::UserProfile => v!(tauri_app_lib::commands::profile::get_profile(st(), uid().await?)),
        // AddToPlaylistMenu: the first 500, then every further page (the picker lists them all).
        Key::AllPlaylists => {
            const PAGE: u32 = 500;
            let me = uid().await?;
            let page = |offset: u32| library::get_all_playlists(st(), h.clone(), me, offset, PAGE, "DATE_UPDATED".into(), "DESC".into());
            let mut first = v!(page(0))?;
            let total = first["totalNumberOfItems"].as_u64().unwrap_or(0) as u32;
            let mut offset = PAGE;
            while offset < total {
                let more = v!(page(offset))?;
                let got = more["items"].as_array().cloned().unwrap_or_default();
                if got.is_empty() {
                    break;
                }
                if let Some(items) = first["items"].as_array_mut() {
                    items.extend(got);
                }
                offset += PAGE;
            }
            Ok(first)
        }
    })
}

// ---- JSON accessors shared by the views (catalogue items come as {type, data} or bare)

pub fn data(item: &Value) -> &Value {
    if item.get("data").is_some_and(Value::is_object) { &item["data"] } else if item.get("value").is_some_and(Value::is_object) { &item["value"] } else { item }
}
pub fn kind(item: &Value) -> String {
    item["_itemType"].as_str().or(item["type"].as_str()).or(item["hitType"].as_str()).unwrap_or("").trim_end_matches('S').to_uppercase()
}
pub fn name(item: &Value) -> String {
    let d = data(item);
    // getItemTitle: title, name, then v2 mixes' titleTextInfo
    d["title"].as_str().or(d["name"].as_str()).or(d["titleTextInfo"]["text"].as_str()).unwrap_or("").to_string()
}
pub fn artist_of(item: &Value) -> String {
    let d = data(item);
    d["artists"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty())
        .or(d["artist"]["name"].as_str().map(str::to_string))
        // getItemSubtitle's fallbacks, for mixes
        .or_else(|| ["subTitle", "shortSubtitle"].iter().find_map(|k| d[*k].as_str().map(str::to_string)))
        .or_else(|| ["subtitleTextInfo", "subTitleTextInfo", "shortSubtitleTextInfo"].iter().find_map(|k| d[*k]["text"].as_str().map(str::to_string)))
        // and for playlists: who made it and how much is in it, else its description
        .or_else(|| playlist_subtitle(d, user_id_cached()))
        .or_else(|| d["description"].as_str().filter(|s| !s.is_empty()).map(str::to_string))
        .unwrap_or_default()
}

/// [`user_id`] as read at most a second ago: it decrypts the settings file, and a page of cards
/// asks once per playlist card.
fn user_id_cached() -> u64 {
    static AT: Mutex<Option<(Instant, u64)>> = Mutex::new(None);
    let mut at = AT.lock().unwrap();
    match *at {
        Some((t, id)) if t.elapsed() < Duration::from_secs(1) => id,
        _ => {
            let id = user_id();
            *at = Some((Instant::now(), id));
            id
        }
    }
}

/// getItemSubtitle for a playlist: "By You" (the signed-in user's), "By <creator>" or "By
/// TIDAL", and its count ("12 Tracks", "3 Videos · 12 Tracks"), joined by " · ".
fn playlist_subtitle(d: &Value, me: u64) -> Option<String> {
    let c = d.get("creator").filter(|c| c.is_object())?;
    let by = if me != 0 && c["id"].as_u64() == Some(me) {
        Some("By You".to_string())
    } else if let Some(n) = c["name"].as_str().filter(|n| !n.is_empty()) {
        Some(format!("By {n}"))
    } else {
        (c["id"].as_u64() == Some(0)).then(|| "By TIDAL".to_string())
    };
    let count = (!d["numberOfTracks"].is_null() || !d["numberOfVideos"].is_null()).then(|| crate::library::count_label(d));
    let parts: Vec<String> = [by, count].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Whether `text` holds the filter `query` as a reader expects: case folded fully (`ß` as `ss`),
/// an unaccented query matching accented text (`beyonce` finds `Beyoncé`), and in a Turkish or
/// Azerbaijani locale the dotted and dotless i. An empty query matches everything.
pub fn matches(text: &str, query: &str) -> bool {
    static FOLD: LazyLock<rune_kit::search::Fold> = LazyLock::new(|| rune_kit::search::Fold::for_language(&locale_tag(|k| std::env::var(k).ok())));
    FOLD.contains(text, query)
}

/// The user's language as a BCP 47 tag (`tr-TR`), from the POSIX locale the session runs in
/// (`LC_ALL`, then `LC_MESSAGES`, then `LANG`); empty for `C`, `POSIX` or none.
fn locale_tag(var: impl Fn(&str) -> Option<String>) -> String {
    let posix = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| var(k).filter(|v| !v.is_empty())).unwrap_or_default();
    let lang = posix.split(['.', '@']).next().unwrap_or("");
    if lang == "C" || lang == "POSIX" { String::new() } else { lang.replace('_', "-") }
}

#[cfg(test)]
mod tests {
    use super::{locale_tag, matches};
    use rune_kit::search::Fold;

    #[test]
    fn a_failed_fetch_peeks_as_failed_not_loading() {
        use super::{CACHE, Entry, Key, Load, peek, peek_load};
        let key = Key::Suggest("peek-failed-test".into());
        assert!(peek_load(&key).is_none());
        let entry = |load| Entry::landed(load, 0, 0, 0);
        CACHE.lock().unwrap().insert(key.clone(), entry(Load::Loading));
        assert!(matches!(peek_load(&key), Some(Load::Loading)));
        CACHE.lock().unwrap().insert(key.clone(), entry(Load::Failed("offline".into())));
        assert!(peek(&key).is_none());
        assert!(matches!(peek_load(&key), Some(Load::Failed(_))));
        CACHE.lock().unwrap().remove(&key);
    }

    #[test]
    fn asking_again_never_fetches_twice() {
        use super::{Ask, Entry, Lane, Load, ask};
        use std::time::{Duration, Instant};
        let e = |load| Entry::landed(load, 1, 0, 0);
        assert_eq!(ask(None, Lane::Fg), Ask::Fetch(0));
        // on its way, in, or just failed: answered from the cache, in either lane
        for lane in [Lane::Fg, Lane::Bg] {
            assert_eq!(ask(Some(&e(Load::Loading)), lane), Ask::Answer);
            assert_eq!(ask(Some(&e(Load::Ready(serde_json::json!({"a": 1})))), lane), Ask::Answer);
            assert_eq!(ask(Some(&e(Load::Failed("offline".into()))), lane), Ask::Answer);
        }
        // a failure whose retry is due is asked again, once
        let mut old = e(Load::Failed("offline".into()));
        old.at = Instant::now() - Duration::from_secs(6);
        assert_eq!(ask(Some(&old), Lane::Fg), Ask::Fetch(1));
        // a background fetch still waiting goes now when the page on screen asks for it
        let mut waiting = e(Load::Loading);
        waiting.promote = Some(crate::sched::Promote::new());
        assert_eq!(ask(Some(&waiting), Lane::Fg), Ask::Promote);
        assert_eq!(ask(Some(&waiting), Lane::Bg), Ask::Answer);
        // out of date after an edit: fetched again (behind the old value), once
        let mut outdated = e(Load::Ready(serde_json::json!(1)));
        outdated.outdated = true;
        assert_eq!(ask(Some(&outdated), Lane::Fg), Ask::Revalidate);
        outdated.outdated = false;
        assert_eq!(ask(Some(&outdated), Lane::Fg), Ask::Answer);
    }

    #[test]
    fn filters_fold_case_and_accents() {
        assert!(matches("Beyoncé", "beyonce"));
        assert!(matches("Sigur Rós Hoppípolla", "sigur ros"));
        assert!(matches("Motörhead Ace of Spades", "MOTORHEAD"));
        assert!(matches("Die Ärzte", "arzte"));
        assert!(matches("Straße", "strasse"));
        assert!(matches("Straße", "STRASSE"));
        assert!(matches("Łódź", "lodz"));
        assert!(matches("Anything", ""));
        assert!(matches("Anything", "  "));
        // A query with an accent asks for it.
        assert!(!matches("Beyonce", "beyoncé"));
        assert!(!matches("Sigur Rós", "sigur rosa"));
    }

    #[test]
    fn playlist_cards_say_who_made_them_and_how_long_they_are() {
        use serde_json::json;
        let sub = |v: serde_json::Value| super::playlist_subtitle(&v, 424242);
        assert_eq!(sub(json!({"creator": {"id": 424242, "name": "fixture"}, "numberOfTracks": 4})).as_deref(), Some("By You · 4 Tracks"));
        assert_eq!(sub(json!({"creator": {"id": 0}, "numberOfTracks": 3, "numberOfVideos": 1})).as_deref(), Some("By TIDAL · 1 Video · 3 Tracks"));
        assert_eq!(sub(json!({"creator": {"id": 9, "name": "Ann"}})).as_deref(), Some("By Ann"));
        assert_eq!(sub(json!({"creator": {"id": 9}})), None);
        assert_eq!(sub(json!({"title": "No creator"})), None);
    }

    #[test]
    fn turkish_locale_folds_dotless_i() {
        let tr = Fold::for_language(&locale_tag(|k| (k == "LANG").then(|| "tr_TR.UTF-8".to_string())));
        assert!(tr.contains("ISTANBUL", "ıstanbul"));
        assert!(tr.contains("İstanbul", "istanbul"));
        assert!(!tr.contains("ISTANBUL", "istanbul"));
        // Everywhere else `I` is `i`.
        assert!(Fold::default().contains("ISTANBUL", "istanbul"));
        assert!(Fold::default().contains("İstanbul", "istanbul"));
    }

    #[test]
    fn locale_tag_reads_posix_locale() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|p| p.0 == k).map(|p| p.1.to_string());
        assert_eq!(locale_tag(env(&[("LANG", "tr_TR.UTF-8")])), "tr-TR");
        assert_eq!(locale_tag(env(&[("LC_ALL", "az_AZ@latin"), ("LANG", "en_US.UTF-8")])), "az-AZ");
        assert_eq!(locale_tag(env(&[("LC_ALL", ""), ("LC_MESSAGES", "de_DE.UTF-8"), ("LANG", "tr_TR")])), "de-DE");
        assert_eq!(locale_tag(env(&[("LANG", "C.UTF-8")])), "");
        assert_eq!(locale_tag(env(&[])), "");
    }
}
