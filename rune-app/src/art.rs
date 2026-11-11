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
//!   and the user's proxy settings), in the order views ask, which is layout order. A URL the
//!   server refused isn't asked again this run; one that failed in transit, not for
//!   `RETRY_FAILED_AFTER`.
//! - No fade-in: covers appear without animating anything.
//!
//! The Afterglow placeholder shows until a cover is filled in, and whenever there is none
//! (offline, no image on the item, `SONE_ART=0`).
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use rune::Pixels;
use serde_json::Value;
use sone_host_shim::Manager;
use tauri_app_lib::AppState;

const KEEP: usize = 200;
const DOWNLOADS: usize = 6;
const DECODES: usize = 4;
const DISK_BUDGET: u64 = 300 * 1024 * 1024;
const RETRY_FAILED_AFTER: Duration = Duration::from_secs(30);

static SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0
static NET: LazyLock<tokio::sync::Semaphore> = LazyLock::new(|| tokio::sync::Semaphore::new(DOWNLOADS));
static CPU: LazyLock<tokio::sync::Semaphore> = LazyLock::new(|| tokio::sync::Semaphore::new(DECODES));
static OFF: LazyLock<bool> = LazyLock::new(|| std::env::var("SONE_ART").is_ok_and(|v| v == "0"));

/// The full-screen player's ground (full_ui), cut from the cover this many pixels square and
/// blurred here, once, by `GROUND_SIGMA`; the renderer then stretches it over the window with
/// its bilinear filter, which costs a textured quad a frame instead of a live blur.
///
/// The live blur was 48 pt over a ground drawn 1.18 times the window's long side: 1652 pt at
/// the 1400x900 design size, so 48 / 1652 x 128 = 3.7 of these pixels. The blur is now a fixed
/// share of the ground, so it scales with the window as the picture does (and HiDPI gets the
/// full blur: the renderer caps a live one at 64 device pixels). Why 128 and not 64: the
/// stretch interpolates linearly between texels, which is off a smooth curve by at most
/// h^2/8 x |f''|; for a step of contrast A blurred by sigma texels, |f''| <= 0.24 A / sigma^2,
/// so the error is 0.2% of A at sigma 3.7 (under one level of 255, before the scrim), and the
/// slope changes at each texel (every ~13 pt) are too small to band. At 64 pixels sigma would
/// be 1.9: 0.8% and kinks every 26 pt, which can show on a hard-edged cover. Aliasing is
/// nil either way: the gaussian passes exp(-2 pi^2 sigma^2 / 4) of the texture's Nyquist.
const GROUND_PX: u32 = 128;
const GROUND_SIGMA: f32 = 3.7;

/// What a slot holds: the image at a side in pixels, or the full-screen player's ground.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Cut {
    Side(u32),
    Ground,
}

type Slot = (String, Cut);

#[derive(Default)]
struct Held {
    slots: HashMap<Slot, Pixels>,
    order: VecDeque<Slot>,
    /// Downloads in flight, with the slots waiting on each.
    waiting: HashMap<String, Vec<(Cut, Pixels)>>,
    /// URLs whose download failed: a transport failure with its time (asked again after
    /// `RETRY_FAILED_AFTER`), or None for an HTTP error status (not asked again this run).
    failed: HashMap<String, Option<Instant>>,
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
/// for a URL that failed (see `Held::failed`).
pub fn pixels(url: &str, pt: f32) -> Option<Pixels> {
    slot(url, Cut::Side(px(pt).ceil().clamp(1.0, 2048.0) as u32))
}

/// The full-screen player's ground for the cover at `url`: `GROUND_PX` square, blurred, filled
/// in place like [`pixels`].
pub fn ground(url: &str) -> Option<Pixels> {
    slot(url, Cut::Ground)
}

fn slot(url: &str, cut: Cut) -> Option<Pixels> {
    if *OFF {
        return None;
    }
    let key: Slot = (url.to_string(), cut);
    let mut held = HELD.lock().unwrap();
    match held.failed.get(url) {
        Some(None) => return None,
        Some(Some(at)) if at.elapsed() < RETRY_FAILED_AFTER => return None,
        Some(Some(_)) => {
            held.failed.remove(url);
        }
        None => {}
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
        let url = url.to_string();
        sone_host_shim::async_runtime::spawn(async move {
            // a prune racing this read just leaves the placeholder this time
            if let Ok(bytes) = tokio::fs::read(&path).await {
                fill(url, bytes, vec![(cut, s)]).await;
            }
        });
    } else if let Some(waiters) = held.waiting.get_mut(url) {
        waiters.push((cut, slot.clone()));
    } else {
        held.waiting.insert(url.to_string(), vec![(cut, slot.clone())]);
        drop(held);
        download(url.to_string(), path);
    }
    Some(slot)
}

/// Decode `bytes` once and fill each waiting slot at its size, off the UI thread. The first
/// decode of an image also measures its colour (see [`cover_color`]).
async fn fill(url: String, bytes: Vec<u8>, slots: Vec<(Cut, Pixels)>) {
    let _permit = CPU.acquire().await;
    let _ = tokio::task::spawn_blocking(move || {
        let Ok(img) = image::load_from_memory(&bytes) else { return };
        let img = img.to_rgba8();
        let key = color_key(&url);
        if !COLORS.lock().unwrap().contains_key(key) {
            let c = measure(&img);
            COLORS.lock().unwrap().insert(key.to_string(), c);
            crate::controller::bump(); // views tinted by the cover draw again
        }
        for (cut, slot) in slots {
            // Covers at exactly the drawn size (never upscaled); others keep their aspect and
            // are drawn with Fill.
            let side = match cut {
                Cut::Side(side) => side,
                Cut::Ground => GROUND_PX,
            };
            let (w, h) = img.dimensions();
            let k = (side as f32 / w.min(h) as f32).min(1.0);
            let (nw, nh) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
            let out = if (nw, nh) == (w, h) { img.clone() } else { image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle) };
            let out = if cut == Cut::Ground { blur(&out) } else { out };
            slot.set(nw, nh, out.into_raw());
        }
    })
    .await;
}

/// The ground's blur: `GROUND_SIGMA` on the sRGB values, edges clamped. Side by side with the
/// live blur in the verifier sandbox the ground differs by 0.16 of 255 on average (at most 3):
/// the renderer's blur works on the stored sRGB values too (a linear-light blur visibly moved
/// edges towards the dark side), and only its fade towards black at the ground's edges, mostly
/// off-window, is gone.
fn blur(img: &image::RgbaImage) -> image::RgbaImage {
    image::imageops::blur(img, GROUND_SIGMA)
}

// ---- cover colour
//
// The full-screen player and the miniplayer tint themselves from the cover: its most vivid
// colour, and how light the whole image is (a white cover needs a heavier scrim under text).
// Measured once per image from whichever size is decoded first, and kept for the run.

/// A cover's colour in OKLCH.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverColor {
    /// The most vivid colour's lightness, chroma and hue. `c` is 0 for a grey cover, which
    /// has no hue of its own: use the track's (`ui::hue`).
    pub l: f32,
    pub c: f32,
    pub h: f32,
    /// The whole cover's mean lightness, 0 (black) to 1 (white).
    pub mean_l: f32,
}

static COLORS: LazyLock<Mutex<HashMap<String, CoverColor>>> = LazyLock::new(Default::default);

/// What a URL's colour is filed under: the image without its size, so every size of one cover
/// shares a measurement.
fn color_key(url: &str) -> &str {
    match url.strip_prefix("https://resources.tidal.com/images/") {
        Some(_) => url.rsplit_once('/').map_or(url, |(image, _size)| image),
        None => url,
    }
}

/// The colour of a track's cover (by uuid or URL), once any size of it has been decoded. None
/// before that and without a cover; the views showing it are rebuilt when it arrives.
pub fn cover_color(uuid: Option<&str>) -> Option<CoverColor> {
    let url = cover(uuid, 48.0)?;
    COLORS.lock().unwrap().get(color_key(&url)).copied()
}

fn oklch(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(r), lin(g), lin(b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let ok_l = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    (ok_l, a.hypot(bb), bb.atan2(a).to_degrees().rem_euclid(360.0))
}

/// Sample the image on a 24 x 24 grid, sort the samples into twelve hue buckets weighted by
/// chroma squared (and a little by lightness), and take the heaviest bucket's weighted mean.
fn measure(img: &image::RgbaImage) -> CoverColor {
    const N: u32 = 24;
    #[derive(Default, Clone, Copy)]
    struct Bucket { w: f32, l: f32, c: f32, x: f32, y: f32 }
    let (w, h) = img.dimensions();
    let mut buckets = [Bucket::default(); 12];
    let mut sum_l = 0.0;
    for j in 0..N {
        for i in 0..N {
            let p = img.get_pixel((i * 2 + 1) * w / (N * 2), (j * 2 + 1) * h / (N * 2)).0;
            let (l, c, hue) = oklch(p[0], p[1], p[2]);
            sum_l += l;
            let weight = c * c * (0.3 + l);
            let b = &mut buckets[(hue / 30.0) as usize % 12];
            b.w += weight;
            b.l += l * weight;
            b.c += c * weight;
            b.x += hue.to_radians().cos() * weight;
            b.y += hue.to_radians().sin() * weight;
        }
    }
    let mean_l = sum_l / (N * N) as f32;
    let best = buckets.iter().copied().fold(Bucket::default(), |a, b| if b.w > a.w { b } else { a });
    if best.w < 0.02 {
        return CoverColor { l: mean_l, c: 0.0, h: 0.0, mean_l };
    }
    CoverColor { l: best.l / best.w, c: best.c / best.w, h: best.y.atan2(best.x).to_degrees().rem_euclid(360.0), mean_l }
}

fn download(url: String, path: PathBuf) {
    sone_host_shim::async_runtime::spawn(async move {
        // Err(true): a transport failure, worth asking again later; Err(false): the server's answer.
        let bytes: Result<Vec<u8>, bool> = {
            let _permit = NET.acquire().await;
            let client = crate::host().state::<AppState>().proxied_http.client();
            match client {
                Ok(c) => match c.get(&url).send().await {
                    Ok(r) if r.status().is_success() => r.bytes().await.map(|b| b.to_vec()).map_err(|_| true),
                    Ok(r) => {
                        log::debug!("[art] {} for {url}", r.status());
                        Err(false)
                    }
                    Err(e) => {
                        log::debug!("[art] {url}: {e}");
                        Err(true)
                    }
                },
                Err(_) => Err(true), // the proxy plan blocks traffic: no artwork (for now)
            }
        };
        let waiters = {
            let mut held = HELD.lock().unwrap();
            let w = held.waiting.remove(&url).unwrap_or_default();
            match bytes {
                Err(true) => {
                    held.failed.insert(url.clone(), Some(Instant::now()));
                    // Drop its empty slots too, or the retry would find one and never download.
                    held.slots.retain(|(u, _), _| *u != url);
                    held.order.retain(|(u, _)| *u != url);
                }
                Err(false) => {
                    held.failed.insert(url.clone(), None);
                }
                Ok(_) => {}
            }
            w
        };
        let Ok(bytes) = bytes else { return };
        if let Some(dir) = path.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let tmp = path.with_extension("part");
        if tokio::fs::write(&tmp, &bytes).await.is_ok() {
            let _ = tokio::fs::rename(&tmp, &path).await;
        }
        fill(url, bytes, waiters).await;
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
