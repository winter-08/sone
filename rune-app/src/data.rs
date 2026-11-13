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
    /// Loved tracks, first page (FavoritesView: 100, newest first).
    LovedTracks,
    /// The signed-in user's full profile (React getProfile): ProfilePage and the account avatar.
    UserProfile,
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
pub fn want(key: Key) -> Load {
    let mut c = CACHE.lock().unwrap();
    let mut retries = 0;
    if let Some(e) = c.get(&key) {
        // Landed after the view read the version: it was loading as far as this view knows.
        if after_frozen(e.version) {
            return Load::Loading;
        }
        let due = matches!(e.load, Load::Failed(_)) && RETRY_FAILED_AFTER.get(e.retries).is_some_and(|d| e.at.elapsed() >= *d);
        if !due {
            return e.load.clone();
        }
        retries = e.retries + 1;
    }
    let fetch_id = FETCHES.fetch_add(1, Ordering::Relaxed) + 1;
    c.insert(key.clone(), Entry { load: Load::Loading, fetch: fetch_id, at: Instant::now(), retries, nudged: false, version: 0 });
    drop(c);
    sone_host_shim::async_runtime::spawn(async move {
        let res = fetch(&key).await;
        let load = match res {
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
            c.insert(key, Entry { load, fetch: fetch_id, at: Instant::now(), retries, nudged: false, version });
        }
    });
    Load::Loading
}

/// The cached value if it's there, without starting a fetch (for views that must not request
/// on render, e.g. debounced suggestions).
pub fn peek(key: &Key) -> Option<Value> {
    match CACHE.lock().unwrap().get(key) {
        Some(Entry { load: Load::Ready(v), version, .. }) if !after_frozen(*version) => Some(v.clone()),
        _ => None,
    }
}

pub fn ready(key: Key) -> Option<Value> {
    match want(key) {
        Load::Ready(v) => Some(v),
        _ => None,
    }
}

/// Awaitable fetch for the controller (e.g. an artist's top tracks before playing them).
pub async fn get(key: Key) -> Result<Value, String> {
    if let Some(v) = peek(&key) {
        return Ok(v);
    }
    let v = fetch(&key).await?;
    let fetch_id = FETCHES.fetch_add(1, Ordering::Relaxed) + 1;
    let mut c = CACHE.lock().unwrap();
    let version = VERSION.fetch_add(1, Ordering::Relaxed) + 1;
    c.insert(key, Entry { load: Load::Ready(v.clone()), fetch: fetch_id, at: Instant::now(), retries: 0, nudged: false, version });
    Ok(v)
}

/// Forget a cached entry so the next ask fetches again (e.g. after signing in).
pub fn forget(key: &Key) {
    CACHE.lock().unwrap().remove(key);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Forget every cached entry, so everything shown is fetched again (Refresh app data).
pub fn clear() {
    CACHE.lock().unwrap().clear();
    VERSION.fetch_add(1, Ordering::Relaxed);
}

pub fn user_id() -> u64 {
    crate::host().state::<AppState>().load_settings().and_then(|s| s.auth_tokens).and_then(|t| t.user_id).unwrap_or(0)
}

async fn fetch(key: &Key) -> Result<Value, String> {
    let h = crate::host().clone();
    let st = || h.state::<AppState>();
    let j = |r: Result<Value, String>| r;
    macro_rules! v {
        ($e:expr) => {
            $e.await.map_err(|e| e.to_string()).and_then(|x| serde_json::to_value(x).map_err(|e| e.to_string()))
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
        Key::Playlist(uuid) => {
            // The React playlist page fires these three together.
            let (d, t, r) = tokio::join!(
                metadata::get_playlist_details(st(), uuid.clone()),
                library::get_playlist_tracks_page(st(), h.clone(), uuid.clone(), 0, 100, None, None),
                library::get_playlist_recommendations(st(), uuid.clone(), 0, 50)
            );
            let d = d.map_err(|e| e.to_string())?;
            let t = serde_json::to_value(t.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let r = r.ok().and_then(|x| serde_json::to_value(x).ok()).unwrap_or(Value::Null);
            Ok(serde_json::json!({"details": d, "tracks": t, "recommendations": r}))
        }
        Key::FavAlbums => v!(library::get_favorite_albums(st(), h.clone(), user_id(), 0, 50, "DATE".into(), "DESC".into())),
        Key::FavArtists => v!(library::get_favorite_artists(st(), h.clone(), user_id(), 0, 50, "DATE".into(), "DESC".into())),
        Key::FavMixes => v!(library::get_favorite_mixes(st(), h.clone(), 0, 50, "DATE".into(), "DESC".into())),
        Key::Profile => v!(tauri_app_lib::commands::auth::get_user_profile(st(), user_id())),
        Key::Lyrics(id) => v!(metadata::get_track_lyrics(st(), *id)),
        Key::Page(path) => v!(pages::get_page_section(st(), h.clone(), path.clone())),
        Key::Feed => v!(tauri_app_lib::commands::feed::get_feed(st(), user_id())),
        Key::Credits(id) => v!(metadata::get_track_credits(st(), *id)),
        Key::ArtistBio(id) => v!(pages::get_artist_bio(st(), h.clone(), *id)),
        Key::Mix(id) => v!(pages::get_mix_items(st(), id.clone())),
        Key::UserProfile => v!(tauri_app_lib::commands::profile::get_profile(st(), user_id())),
        Key::LovedTracks => v!(library::get_favorite_tracks(st(), h.clone(), user_id(), 0, 100, "DATE".into(), "DESC".into())),
        Key::AllPlaylists => v!(library::get_all_playlists(st(), h.clone(), user_id(), 0, 500, "DATE_UPDATED".into(), "DESC".into())),
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
        .unwrap_or_default()
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
