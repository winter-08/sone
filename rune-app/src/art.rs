//! Real artwork: covers and pictures from TIDAL's image CDN, cached on disk, drawn by Rune.
//!
//! Performance, in order of what matters:
//! - Decode to the size that's drawn: the CDN file is the smallest of its sizes at least the
//!   element's points times the display scale, and it's resized to exactly that many pixels
//!   (a 132 pt card at 1.5x: the 320 px file, kept as 198 px, ~160 KB of RGBA). No mips are
//!   needed when nothing is minified, and memory goes with the square of the side.
//! - Never rebuild the view for a cover: each (url, size) gets a stable `Pixels` slot the first
//!   time a view asks, drawn empty over the placeholder; when the image is ready it's `set` from
//!   a worker, which wakes only the windows showing it. (An earlier version rebuilt the page to
//!   attach each cover; the accessibility tree then published the page ~60 ms late.)
//! - Decode off the UI thread: on the blocking pool, `DECODES` at a time.
//! - Bound memory: `HELD` keeps the `KEEP` most recently drawn slots; an element still showing
//!   an evicted slot keeps its pixels until the view moves on.
//! - Download once: the JPEG bytes are kept under `$XDG_CACHE_HOME/sone-rune/art` (pruned to
//!   `DISK_BUDGET` at startup, oldest first).
//! - Bounded network: `DOWNLOADS` at a time on the backend's shared client (connection reuse,
//!   and the user's proxy settings), in the order views ask, which is layout order. A failed URL
//!   isn't asked again this run.
//! - No fade-in: covers appear without animating anything.
//!
//! The Afterglow placeholder shows until a cover is filled in, and whenever there is none
//! (offline, no image on the item, `SONE_ART=0`).
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};

use rune::Pixels;
use serde_json::Value;
use sone_host_shim::Manager;
use tauri_app_lib::AppState;

const KEEP: usize = 200;
const DOWNLOADS: usize = 6;
const DECODES: usize = 4;
const DISK_BUDGET: u64 = 300 * 1024 * 1024;

static SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0
static NET: LazyLock<tokio::sync::Semaphore> = LazyLock::new(|| tokio::sync::Semaphore::new(DOWNLOADS));
static CPU: LazyLock<tokio::sync::Semaphore> = LazyLock::new(|| tokio::sync::Semaphore::new(DECODES));
static OFF: LazyLock<bool> = LazyLock::new(|| std::env::var("SONE_ART").is_ok_and(|v| v == "0"));

type Slot = (String, u32); // (url, side in pixels)

#[derive(Default)]
struct Held {
    slots: HashMap<Slot, Pixels>,
    order: VecDeque<Slot>,
    /// Downloads in flight, with the slots waiting on each.
    waiting: HashMap<String, Vec<(u32, Pixels)>>,
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

/// The slot for `url` drawn `pt` points square: empty until the image is in, then filled in
/// place (the windows showing it redraw; the view doesn't re-run). None with `SONE_ART=0` or
/// for a URL that failed.
pub fn pixels(url: &str, pt: f32) -> Option<Pixels> {
    if *OFF {
        return None;
    }
    let side = px(pt).ceil().clamp(1.0, 2048.0) as u32;
    let key: Slot = (url.to_string(), side);
    let mut held = HELD.lock().unwrap();
    if held.failed.contains(url) {
        return None;
    }
    if let Some(p) = held.slots.get(&key).cloned() {
        if let Some(pos) = held.order.iter().position(|k| *k == key) {
            let k = held.order.remove(pos).unwrap();
            held.order.push_back(k);
        }
        return Some(p);
    }
    let slot = Pixels::new();
    held.slots.insert(key.clone(), slot.clone());
    held.order.push_back(key);
    while held.order.len() > KEEP {
        if let Some(old) = held.order.pop_front() {
            held.slots.remove(&old);
        }
    }
    let path = disk_path(url);
    if path.exists() {
        drop(held);
        let s = slot.clone();
        sone_host_shim::async_runtime::spawn(async move {
            // a prune racing this read just leaves the placeholder this time
            if let Ok(bytes) = tokio::fs::read(&path).await {
                fill(bytes, vec![(side, s)]).await;
            }
        });
    } else if let Some(waiters) = held.waiting.get_mut(url) {
        waiters.push((side, slot.clone()));
    } else {
        held.waiting.insert(url.to_string(), vec![(side, slot.clone())]);
        drop(held);
        download(url.to_string(), path);
    }
    Some(slot)
}

/// Decode `bytes` once and fill each waiting slot at its size, off the UI thread.
async fn fill(bytes: Vec<u8>, slots: Vec<(u32, Pixels)>) {
    let _permit = CPU.acquire().await;
    let _ = tokio::task::spawn_blocking(move || {
        let Ok(img) = image::load_from_memory(&bytes) else { return };
        let img = img.to_rgba8();
        for (side, slot) in slots {
            // Covers at exactly the drawn size (never upscaled); others keep their aspect and
            // are drawn with Fill.
            let (w, h) = img.dimensions();
            let k = (side as f32 / w.min(h) as f32).min(1.0);
            let (nw, nh) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
            let out = if (nw, nh) == (w, h) { img.clone() } else { image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle) };
            slot.set(nw, nh, out.into_raw());
        }
    })
    .await;
}

fn download(url: String, path: PathBuf) {
    sone_host_shim::async_runtime::spawn(async move {
        let bytes = {
            let _permit = NET.acquire().await;
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
        let waiters = {
            let mut held = HELD.lock().unwrap();
            let w = held.waiting.remove(&url).unwrap_or_default();
            if bytes.is_none() {
                held.failed.insert(url.clone());
            }
            w
        };
        let Some(bytes) = bytes else { return };
        if let Some(dir) = path.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let tmp = path.with_extension("part");
        if tokio::fs::write(&tmp, &bytes).await.is_ok() {
            let _ = tokio::fs::rename(&tmp, &path).await;
        }
        fill(bytes, waiters).await;
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
