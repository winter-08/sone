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
use tauri_app_lib::AppState;
use tauri_app_lib::commands::{library, metadata, pages, search};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Home,
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
}

#[derive(Clone)]
pub enum Load {
    Loading,
    Ready(Value),
    Failed(String),
}

static CACHE: LazyLock<Mutex<HashMap<Key, Load>>> = LazyLock::new(Default::default);
static VERSION: AtomicU64 = AtomicU64::new(0);

pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

/// The cached value, starting the fetch if this is the first ask.
pub fn want(key: Key) -> Load {
    let mut c = CACHE.lock().unwrap();
    if let Some(v) = c.get(&key) {
        return v.clone();
    }
    c.insert(key.clone(), Load::Loading);
    drop(c);
    sone_host_shim::async_runtime::spawn(async move {
        let res = fetch(&key).await;
        CACHE.lock().unwrap().insert(key, match res {
            Ok(v) => Load::Ready(v),
            Err(e) => {
                log::warn!("[rune] data fetch failed: {e}");
                Load::Failed(e)
            }
        });
        VERSION.fetch_add(1, Ordering::Relaxed);
    });
    Load::Loading
}

/// The cached value if it's there, without starting a fetch (for views that must not request
/// on render, e.g. debounced suggestions).
pub fn peek(key: &Key) -> Option<Value> {
    match CACHE.lock().unwrap().get(key) {
        Some(Load::Ready(v)) => Some(v.clone()),
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
    if let Some(Load::Ready(v)) = CACHE.lock().unwrap().get(&key).cloned() {
        return Ok(v);
    }
    let v = fetch(&key).await?;
    CACHE.lock().unwrap().insert(key, Load::Ready(v.clone()));
    VERSION.fetch_add(1, Ordering::Relaxed);
    Ok(v)
}

/// Forget a cached entry so the next ask fetches again (e.g. after signing in).
pub fn forget(key: &Key) {
    CACHE.lock().unwrap().remove(key);
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
        Key::Home => v!(pages::get_home_page(st(), h.clone(), None)),
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
    d["title"].as_str().or(d["name"].as_str()).unwrap_or("").to_string()
}
pub fn artist_of(item: &Value) -> String {
    let d = data(item);
    d["artists"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty())
        .or(d["artist"]["name"].as_str().map(str::to_string)).unwrap_or_default()
}
