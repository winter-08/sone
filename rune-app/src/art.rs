//! Real artwork: covers and pictures from TIDAL's image CDN, cached on disk, decoded by Rune.
//!
//! Performance, in order of what matters:
//! - Fetch the size that's drawn: the smallest CDN size at least the element's points times the
//!   display scale (a 132 pt card at 1.5x wants 198 px, so the 320 px file). Decoded memory goes
//!   with the square of the side.
//! - Decode off the UI thread: Rune's `ImageCache` decodes and builds mips on its worker. A
//!   decoded image wakes only the windows showing it, without rebuilding the view.
//! - Bound decoded memory: the cache holds images weakly, so `HELD` decides what stays: the most
//!   recently drawn `KEEP` images (a 320 px cover with mips is about 0.55 MB). Older ones reload
//!   from disk when drawn again.
//! - Download once: the JPEG bytes are kept under `$XDG_CACHE_HOME/sone-rune/art` (pruned to
//!   `DISK_BUDGET` at startup, oldest first).
//! - Bounded network: `PERMITS` downloads at a time on the backend's shared client (connection
//!   reuse, and the user's proxy settings), in the order views ask, which is layout order. A
//!   failed URL isn't asked again this run.
//! - No redraw churn: views rebuild for new covers at most once per poll (`version()`), and
//!   covers appear without a fade, so nothing animates while they stream in.
//!
//! The Afterglow placeholder stays underneath until a cover is decoded, and whenever there is
//! none (offline, no image on the item).
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use rune::{Image, ImageCache, Pixels};
use serde_json::Value;
use sone_host_shim::Manager;
use tauri_app_lib::AppState;

const KEEP: usize = 160;
const PERMITS: usize = 6;
const DISK_BUDGET: u64 = 300 * 1024 * 1024;

static SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0
static VERSION: AtomicU64 = AtomicU64::new(0);
static CACHE: LazyLock<ImageCache> = LazyLock::new(|| ImageCache::new(Arc::new(|| {})));
static DOWNLOADS: LazyLock<tokio::sync::Semaphore> = LazyLock::new(|| tokio::sync::Semaphore::new(PERMITS));

#[derive(Default)]
struct Held {
    images: HashMap<String, Image>,
    order: VecDeque<String>,
    fetching: HashSet<String>,
    failed: HashSet<String>,
}
static HELD: LazyLock<Mutex<Held>> = LazyLock::new(Default::default);

/// The display scale the main view draws at (from `Cx::env().scale`).
pub fn set_scale(scale: f32) {
    if scale.is_finite() && scale > 0.0 {
        SCALE.store(scale.to_bits(), Ordering::Relaxed);
    }
}

fn px(pt: f32) -> f32 {
    pt * f32::from_bits(SCALE.load(Ordering::Relaxed))
}

/// Bumps when a downloaded cover is ready to attach (main::poll redraws).
pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

fn cdn(uuid: &str, side: u32) -> String {
    format!("https://resources.tidal.com/images/{}/{side}x{side}.jpg", uuid.replace('-', "/"))
}

/// Album, playlist and track covers: 160, 320, 640, 1280 (types.ts getTidalImageUrl).
fn cover_url(uuid: &str, pt: f32) -> String {
    if uuid.starts_with("http") {
        return uuid.to_string();
    }
    let p = px(pt);
    cdn(uuid, if p <= 160.0 { 160 } else if p <= 320.0 { 320 } else if p <= 640.0 { 640 } else { 1280 })
}

/// Artist pictures: 160, 320, 480, 750; there is no 640 or 1280 (types.ts getTidalArtistImageUrl).
fn artist_url(uuid: &str, pt: f32) -> String {
    if uuid.starts_with("http") {
        return uuid.to_string();
    }
    let p = px(pt);
    cdn(uuid, if p <= 160.0 { 160 } else if p <= 320.0 { 320 } else if p <= 480.0 { 480 } else { 750 })
}

fn nonempty(v: &Value) -> Option<&str> {
    v.as_str().filter(|s| !s.is_empty())
}

/// An item's image at `pt` points, as React's `getItemImage` picks it (itemHelpers.ts).
pub fn item_url(item: &Value, pt: f32) -> Option<String> {
    let d = crate::data::data(item);
    let p = px(pt);
    if let Some(images) = d["images"].as_object() {
        let url = |k: &str| images.get(k).and_then(|i| nonempty(&i["url"])).map(str::to_string);
        let pick = if p <= 320.0 { url("SMALL") } else if p <= 640.0 { url("MEDIUM") } else { None };
        if let Some(u) = pick.or_else(|| url("LARGE")).or_else(|| url("SMALL")) {
            return Some(u);
        }
    }
    for key in ["mixImages", "detailMixImages"] {
        if let Some(u) = d[key].as_array().and_then(|a| a.first()).and_then(|i| nonempty(&i["url"])) {
            return Some(u.to_string());
        }
    }
    if let Some(u) = nonempty(&d["detailImages"]["MEDIUM"]["url"]).or(nonempty(&d["detailImages"]["SMALL"]["url"])) {
        return Some(u.to_string());
    }
    for key in ["cover", "squareImage", "image"] {
        if let Some(u) = nonempty(&d[key]) {
            return Some(cover_url(u, pt));
        }
    }
    if let Some(u) = artist_picture(d, pt) {
        return Some(u);
    }
    if let Some(u) = nonempty(&d["album"]["cover"]) {
        return Some(cover_url(u, pt));
    }
    if let Some(u) = nonempty(&d["imageUrl"]) {
        return Some(u.to_string());
    }
    nonempty(&d["imageId"]).map(|u| cover_url(u, pt))
}

/// An artist's picture (React getArtistImage: `picture` before `artworkId`, then an album cover).
pub fn artist_picture(d: &Value, pt: f32) -> Option<String> {
    if let Some(u) = nonempty(&d["picture"]).or(nonempty(&d["artworkId"])) {
        return Some(artist_url(u, pt));
    }
    nonempty(&d["selectedAlbumCoverFallback"]).or(nonempty(&d["albumCoverFallback"])).map(|u| cover_url(u, pt))
}

/// A track's album cover by its uuid (or URL).
pub fn cover(uuid: Option<&str>, pt: f32) -> Option<String> {
    uuid.filter(|u| !u.is_empty()).map(|u| cover_url(u, pt))
}

fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("sone-rune").join("art")
}

/// A stable file name for a URL (the CDN path, flattened; anything else, hashed).
fn disk_path(url: &str) -> PathBuf {
    let name = match url.strip_prefix("https://resources.tidal.com/images/") {
        Some(rest) => rest.replace('/', "_"),
        None => {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            url.hash(&mut h);
            format!("u{:016x}.img", h.finish())
        }
    };
    cache_dir().join(name)
}

fn hold(held: &mut Held, url: &str, image: Image) {
    held.images.insert(url.to_string(), image);
    held.order.push_back(url.to_string());
    while held.order.len() > KEEP {
        if let Some(old) = held.order.pop_front() {
            held.images.remove(&old);
        }
    }
}

/// The image at `url` for drawing: pending (draws when decoded) or ready. None while it
/// downloads, or when there is none.
pub fn pixels(url: &str) -> Option<Pixels> {
    let mut held = HELD.lock().unwrap();
    if let Some(img) = held.images.get(url) {
        let px = (img.error().is_none()).then(|| img.pixels().clone());
        if let Some(pos) = held.order.iter().position(|u| u == url) {
            let u = held.order.remove(pos).unwrap();
            held.order.push_back(u);
        }
        return px;
    }
    if held.fetching.contains(url) || held.failed.contains(url) {
        return None;
    }
    let path = disk_path(url);
    if path.exists() {
        // read and decoded on Rune's worker, not here
        let img = CACHE.file(&path, 0.0, 1.0);
        let px = img.pixels().clone();
        hold(&mut held, url, img);
        return Some(px);
    }
    held.fetching.insert(url.to_string());
    drop(held);
    download(url.to_string(), path);
    None
}

fn download(url: String, path: PathBuf) {
    sone_host_shim::async_runtime::spawn(async move {
        let bytes = {
            let _permit = DOWNLOADS.acquire().await;
            let client = crate::host().state::<AppState>().proxied_http.client();
            match client {
                Ok(c) => match c.get(&url).send().await {
                    Ok(r) if r.status().is_success() => r.bytes().await.ok().map(|b| b.to_vec()),
                    Ok(r) => {
                        log::debug!("[art] {} for {url}", r.status());
                        None
                    }
                    Err(e) => {
                        log::debug!("[art] {url}: {e}");
                        None
                    }
                },
                Err(_) => None, // the proxy plan blocks traffic: no artwork
            }
        };
        let mut held = HELD.lock().unwrap();
        held.fetching.remove(&url);
        match bytes {
            Some(b) => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let tmp = path.with_extension("part");
                if std::fs::write(&tmp, &b).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
                let img = CACHE.load(b);
                hold(&mut held, &url, img);
                VERSION.fetch_add(1, Ordering::Relaxed);
            }
            None => {
                held.failed.insert(url);
            }
        }
    });
}

/// Keep the disk cache under its budget: delete the least recently written files.
pub fn prune_disk_cache() {
    std::thread::spawn(|| {
        let Ok(rd) = std::fs::read_dir(cache_dir()) else { return };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                m.is_file().then(|| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.0);
        for (_, len, path) in files {
            if total <= DISK_BUDGET {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
    });
}
