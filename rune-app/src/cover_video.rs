//! Animated album covers (React `TidalVideoCover`, `videoCoversAtom`): an album with a
//! `videoCover` plays a looping, muted mp4 from `resources.tidal.com/videos/<uuid>/<n>x<n>.mp4`
//! in place of its still cover, on the album page, in the now-playing drawer's cover column and
//! in the full-screen player. Settings > Playback > "Animated album covers" turns it off
//! (`sone.videoCovers.v1` in the shell store, on by default as in React).
//!
//! Separate from music videos (video.rs): no audio branch, its own pipelines and surfaces.
//!
//! - **Two slots, at most two pipelines.** `Page` is the album page's cover; `Now` is the
//!   playing track's, shared by the drawer and the full player (they show the same track, and
//!   the full player covers the drawer), so one decode serves both.
//! - **Nothing runs unless it shows.** `main::view` works out what each slot should play
//!   ([`wants`]) from what is on screen: the album page with its cover scrolled into view and
//!   no player over it, the drawer's cover column, the full player. The toggle off, `SONE_FX=0`,
//!   the window off screen (`on_visibility`) or a failed cover all leave a slot empty, and an
//!   empty slot has no pipeline and no download. A track change to another album, leaving the
//!   page or closing the drawer/player stops it. While the window is unfocused a pipeline
//!   pauses (kept, so focus resumes it at once) and the still shows: see [`showing`].
//! - **Frames go to a surface, not through the view.** Each slot has a `NativeSurface` that
//!   its element draws; a pushed frame wakes the window to draw, and no view or region runs for
//!   it. The element is in the view only while its cover plays (one rebuild of the page or
//!   drawer region when that starts or stops), since Rune redraws every frame a surface is on
//!   screen. Rune has no partial redraw, so a playing cover redraws the window at the cover's frame
//!   rate: `videorate` caps that at 30 frames/s (or the video's own rate, if lower).
//! - **Decode at the drawn size.** The 640 file for covers drawn at up to 640 pixels, the 1280
//!   one only above that (React's getTidalVideoUrl brackets), scaled in the pipeline to the
//!   drawn pixels so the surface holds no more than is shown.
//! - **Download once, loop locally.** The mp4 is fetched with the backend's shared HTTP client
//!   (`AppState::proxied_http`, what artwork and the video relay use, so the user's proxy
//!   applies) into `$XDG_CACHE_HOME/sone-rune/covers`, and the pipeline plays the file: a loop
//!   is a local seek to 0 on end of stream, not another download.
//! - **The still cover shows** until the first frame, and for good after any error (logged
//!   once per cover; that cover is not tried again this run).

use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gst::prelude::*;
use gst_video::prelude::*;
use rune::native_surface::{ByteFrame, ColorInfo, Format, Matrix, Transfer};
use rune::{El, NativeSurface, Waker, el};
use sone_host_shim::Manager;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use tauri_app_lib::AppState;

/// The React localStorage key, kept in the shell store (rune-shell.json).
const STORE_KEY: &str = "sone.videoCovers.v1";
/// Frames per second at most: each frame redraws the whole window.
const MAX_FPS: i32 = 30;
/// Drawn sizes are rounded up to this many pixels, so a window resize renegotiates the scale
/// at steps rather than on every pixel.
const PX_STEP: u32 = 32;
/// Cached mp4s are pruned to this at startup, oldest first.
const DISK_BUDGET: u64 = 200 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// The album page's header cover.
    Page = 0,
    /// The playing track's cover: the drawer's cover column and the full-screen player.
    Now = 1,
}

impl Slot {
    fn name(self) -> &'static str {
        match self { Slot::Page => "page", Slot::Now => "now" }
    }
}

/// What a slot should play: the cover's uuid (or URL), the file size to fetch (640 or 1280),
/// and the pixels it is drawn at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub uuid: String,
    pub variant: u32,
    pub px: u32,
}

impl Want {
    /// A cover drawn `px` device pixels square.
    pub fn new(uuid: &str, px: f32) -> Option<Want> {
        if uuid.is_empty() || !px.is_finite() || px <= 0.0 {
            return None;
        }
        let variant = variant_for(px);
        let stepped = (px.ceil() as u32).div_ceil(PX_STEP) * PX_STEP;
        Some(Want { uuid: uuid.to_string(), variant, px: stepped.min(variant) })
    }
}

/// The file for a cover drawn `px` pixels square: 640 up to 640, else 1280 (getTidalVideoUrl).
pub fn variant_for(px: f32) -> u32 {
    if px <= 640.0 { 640 } else { 1280 }
}

/// getTidalVideoUrl: a uuid's dashes become path separators; a full URL is used as it is.
pub fn video_url(uuid: &str, variant: u32) -> Option<String> {
    if uuid.is_empty() {
        return None;
    }
    if uuid.starts_with("http") {
        return Some(uuid.to_string());
    }
    Some(format!("https://resources.tidal.com/videos/{}/{variant}x{variant}.mp4", uuid.replace('-', "/")))
}

/// What is on screen that could animate, as `main::view` sees it. Each cover is its
/// `videoCover` and drawn size in points; None when it isn't showing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene<'a> {
    pub page: Option<(&'a str, f32)>,
    pub drawer: Option<(&'a str, f32)>,
    pub full: Option<(&'a str, f32)>,
    pub scale: f32,
}

/// What each slot should play for `scene`. The full player, when open, is what the `Now`
/// slot serves (it covers the drawer); the page cover never shows under it.
pub fn wants(scene: &Scene) -> [Option<Want>; 2] {
    let scale = if scene.scale > 0.0 { scene.scale } else { 1.0 };
    let page = if scene.full.is_some() { None } else { scene.page };
    let now = scene.full.or(scene.drawer);
    [page, now].map(|c| c.and_then(|(uuid, pt)| Want::new(uuid, pt * scale)))
}

/// The playing track's animated cover (React `currentTrack.album.videoCover`); none while a
/// music video plays.
pub fn now_cover(v: &crate::controller::View) -> Option<&str> {
    if v.video.is_some() {
        return None;
    }
    v.current.as_ref()?.track["album"]["videoCover"].as_str().filter(|s| !s.is_empty())
}

/// An album page's animated cover (the backend's AlbumPageResponse: the header album's
/// `videoCover`, also given beside it).
pub fn album_cover(page: &serde_json::Value) -> Option<&str> {
    [&page["album"]["videoCover"], &page["videoCover"]].into_iter().filter_map(|v| v.as_str()).find(|s| !s.is_empty())
}

/// A slot's pipeline (or download) as the lifecycle knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Running {
    pub want: Want,
    pub paused: bool,
}

/// One slot's next move.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Keep,
    /// Start (replacing whatever runs: a new cover, or another file size of the same one).
    Start { want: Want, paused: bool },
    Stop,
    /// The same file, drawn at another size: rescale in place.
    Resize(u32),
    Pause,
    Resume,
}

/// The lifecycle for one slot: `want` is what should play (already None when the toggle is
/// off, motion is off, the window is off screen or the cover failed), `focused` whether the
/// window has focus (unfocused, a cover holds its frame).
pub fn step(running: Option<&Running>, want: Option<&Want>, focused: bool) -> Step {
    match (running, want) {
        (None, None) => Step::Keep,
        (Some(_), None) => Step::Stop,
        (None, Some(w)) => Step::Start { want: w.clone(), paused: !focused },
        (Some(r), Some(w)) if r.want.uuid != w.uuid || r.want.variant != w.variant => Step::Start { want: w.clone(), paused: !focused },
        (Some(r), Some(w)) if r.want.px != w.px => Step::Resize(w.px),
        (Some(r), Some(_)) if r.paused == focused => if focused { Step::Resume } else { Step::Pause },
        _ => Step::Keep,
    }
}

/// The toggle's state and motion together: whether covers may animate at all.
pub fn on() -> bool {
    enabled() && crate::fx::on()
}

/// Whether this process draws on a software renderer. Rune can't redraw part of a window, so a
/// playing cover redraws all of it 30 times a second: a few percent of a core on a GPU, several
/// whole cores on lavapipe. Rune doesn't say which adapter it picked, so this goes by what
/// selects one: a Vulkan driver list naming only lavapipe (`wrap.sh`'s SONE_SOFTWARE_VULKAN),
/// Mesa's software switch, or a machine with no GPU render node at all.
pub fn software_renderer() -> bool {
    static SOFTWARE: LazyLock<bool> = LazyLock::new(|| {
        let env = |k: &str| std::env::var(k).ok();
        let nodes = std::fs::read_dir("/dev/dri")
            .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("renderD")))
            .unwrap_or(false);
        let software = software_from(env("VK_ICD_FILENAMES").or_else(|| env("VK_DRIVER_FILES")).as_deref(), env("LIBGL_ALWAYS_SOFTWARE").as_deref(), nodes);
        if software {
            log::info!("[cover] software rendering: animated covers are off unless turned on in Settings");
        }
        software
    });
    *SOFTWARE
}

/// `software_renderer`'s rule: `icds` is the Vulkan driver list when one is forced, `mesa` is
/// LIBGL_ALWAYS_SOFTWARE, `render_node` whether /dev/dri has a render node.
fn software_from(icds: Option<&str>, mesa: Option<&str>, render_node: bool) -> bool {
    let only_lavapipe = icds.is_some_and(|l| {
        let mut files = l.split(':').filter(|f| !f.is_empty()).peekable();
        files.peek().is_some() && files.all(|f| f.contains("lvp_icd"))
    });
    only_lavapipe || mesa.is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true")) || !render_node
}

/// On unless the user said otherwise, except on a software renderer, where it is off unless the
/// user turned it on.
static ENABLED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(crate::shell::store_get(STORE_KEY).and_then(|v| v.as_bool()).unwrap_or(!software_renderer())));

/// Settings' "Animated album covers" (React videoCoversAtom, default on).
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    crate::shell::store_set(STORE_KEY, serde_json::Value::Bool(on));
    log::info!("[cover] animated covers {}", if on { "on" } else { "off" });
    reconcile();
}

// ------------------------------------------------------------------ surfaces

static SURFACES: [OnceLock<NativeSurface>; 2] = [OnceLock::new(), OnceLock::new()];

fn surface(slot: Slot) -> &'static NativeSurface {
    SURFACES[slot as usize].get_or_init(NativeSurface::new)
}

/// From the main window's setup: a pushed frame wakes it to draw.
pub fn set_waker(w: Waker) {
    for slot in [Slot::Page, Slot::Now] {
        surface(slot).set_waker(w.clone());
    }
}

/// The slot's pipeline has a frame and is playing: only then does a view lay [`layer`] over the
/// still cover. Rune never counts a frame with a native surface in it as unchanged, so a
/// surface on screen makes every frame the window runs for anything (the 200 ms tick, a
/// position tick) a full redraw; a paused, loading or failed cover leaves it out and shows the
/// still. Each change wakes the poll (`controller::bump`), so the view catches up.
pub fn showing(slot: Slot) -> bool {
    FRAMED[slot as usize].load(Ordering::Relaxed) && {
        let m = M.lock().unwrap();
        m.live[slot as usize].as_ref().is_some_and(|l| !l.paused)
    }
}

/// A frame has come from the slot's current pipeline.
static FRAMED: [AtomicBool; 2] = [AtomicBool::new(false), AtomicBool::new(false)];

/// The element that shows `slot`'s frames, `side` points square with `radius` corners, to lay
/// over the still cover while [`showing`].
pub fn layer(slot: Slot, side: f32, radius: f32) -> El {
    el().absolute(0.0, 0.0).size(side, side).radius(radius).clip_rounded(radius).a11y_hidden()
        .native_surface(surface(slot), rune::SurfaceFit::Fill)
}

// ------------------------------------------------------------------ the lifecycle

struct Live {
    want: Want,
    paused: bool,
    /// Cleared when this pipeline is replaced or stopped: its callbacks and bus thread stop.
    alive: Arc<AtomicBool>,
    /// None while the file downloads.
    pipe: Option<Pipe>,
}

struct Pipe {
    pipeline: gst::Pipeline,
    filter: gst::Element,
}

#[derive(Default)]
struct Module {
    wants: [Option<Want>; 2],
    hidden: bool,
    unfocused: bool,
    live: [Option<Live>; 2],
    failed: HashSet<String>,
}

static M: LazyLock<Mutex<Module>> = LazyLock::new(Default::default);
/// Pipelines alive now (for the log and the tests of teardown).
static PIPELINES: AtomicU64 = AtomicU64::new(0);

/// From `main::view`: what each slot should play now.
pub fn sync(wants: [Option<Want>; 2]) {
    {
        let mut m = M.lock().unwrap();
        if m.wants == wants {
            return;
        }
        m.wants = wants;
    }
    reconcile();
}

/// From `on_visibility`: off screen, every cover stops (and starts again when shown).
pub fn set_shown(shown: bool) {
    let changed = {
        let mut m = M.lock().unwrap();
        std::mem::replace(&mut m.hidden, !shown) != !shown
    };
    if changed {
        reconcile();
    }
}

/// From the window's focus hook: unfocused, covers hold their frame.
pub fn set_focused(focused: bool) {
    let changed = {
        let mut m = M.lock().unwrap();
        std::mem::replace(&mut m.unfocused, !focused) != !focused
    };
    if changed {
        reconcile();
    }
}

fn reconcile() {
    let mut retired = Vec::new();
    let mut starts = Vec::new();
    // whether what `showing` says may have changed
    let mut changed = false;
    {
        let mut m = M.lock().unwrap();
        let gate = on() && !m.hidden;
        let focused = !m.unfocused;
        for slot in [Slot::Page, Slot::Now] {
            let i = slot as usize;
            let want = m.wants[i].clone().filter(|w| gate && !m.failed.contains(&w.uuid));
            let running = m.live[i].as_ref().map(|l| Running { want: l.want.clone(), paused: l.paused });
            match step(running.as_ref(), want.as_ref(), focused) {
                Step::Keep => {}
                Step::Stop => {
                    if let Some(l) = m.live[i].take() {
                        log::info!("[cover] stop {} {}", slot.name(), l.want.uuid);
                        retired.push(l);
                    }
                    FRAMED[i].store(false, Ordering::Relaxed);
                    surface(slot).clear();
                    changed = true;
                }
                Step::Start { want, paused } => {
                    let old = m.live[i].take();
                    // Another cover: the still shows until its first frame. The same cover in
                    // another size keeps its last frame until the new one comes.
                    if old.as_ref().is_none_or(|o| o.want.uuid != want.uuid) {
                        FRAMED[i].store(false, Ordering::Relaxed);
                        surface(slot).clear();
                    }
                    changed = true;
                    retired.extend(old);
                    log::info!("[cover] start {} {} {}px of {}{}", slot.name(), want.uuid, want.px, want.variant, if paused { " (paused)" } else { "" });
                    let alive = Arc::new(AtomicBool::new(true));
                    starts.push((slot, want.clone(), alive.clone()));
                    m.live[i] = Some(Live { want, paused, alive, pipe: None });
                }
                Step::Resize(px) => {
                    if let Some(l) = m.live[i].as_mut() {
                        log::info!("[cover] resize {} {px}px", slot.name());
                        l.want.px = px;
                        if let Some(p) = &l.pipe {
                            p.filter.set_property("caps", &scale_caps(px));
                        }
                    }
                }
                Step::Pause | Step::Resume => {
                    let paused = !focused;
                    if let Some(l) = m.live[i].as_mut() {
                        log::info!("[cover] {} {}", if paused { "pause" } else { "resume" }, slot.name());
                        l.paused = paused;
                        changed = true;
                        if let Some(p) = &l.pipe {
                            let _ = p.pipeline.set_state(if paused { gst::State::Paused } else { gst::State::Playing });
                        }
                    }
                }
            }
        }
    }
    for l in retired {
        retire(l);
    }
    if changed {
        crate::controller::bump();
    }
    // (after the lock: a cached file launches its pipeline at once, which takes it)
    for (slot, want, alive) in starts {
        fetch(slot, want, alive);
    }
}

/// Stops a pipeline off the UI thread (going to Null joins its streaming threads).
fn retire(l: Live) {
    l.alive.store(false, Ordering::Relaxed);
    if let Some(p) = l.pipe {
        std::thread::Builder::new().name("cover-stop".into()).spawn(move || {
            let _ = p.pipeline.set_state(gst::State::Null);
            drop(p);
            let n = PIPELINES.fetch_sub(1, Ordering::Relaxed) - 1;
            log::info!("[cover] pipeline down ({n} alive)");
        }).ok();
    }
}

/// A cover that can't play: the still stays, and this cover isn't tried again this run.
fn fail(slot: Slot, alive: &Arc<AtomicBool>, uuid: &str, why: &str) {
    let gone = {
        let mut m = M.lock().unwrap();
        if m.failed.insert(uuid.to_string()) {
            log::warn!("[cover] {uuid}: {why}; showing the still cover");
        }
        match &m.live[slot as usize] {
            Some(l) if Arc::ptr_eq(&l.alive, alive) => m.live[slot as usize].take(),
            _ => None,
        }
    };
    if let Some(l) = gone {
        FRAMED[slot as usize].store(false, Ordering::Relaxed);
        surface(slot).clear();
        retire(l);
        crate::controller::bump();
    }
}

// ------------------------------------------------------------------ download

fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("sone-rune").join("covers")
}

/// The cached file's name: the uuid and size (a URL is hashed).
fn file_name(uuid: &str, variant: u32) -> String {
    if !uuid.is_empty() && uuid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        format!("{uuid}-{variant}.mp4")
    } else {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        uuid.hash(&mut h);
        format!("{:016x}-{variant}.mp4", h.finish())
    }
}

/// Fetches the cover's file if it isn't cached, then starts its pipeline (unless the slot
/// moved on meanwhile).
fn fetch(slot: Slot, want: Want, alive: Arc<AtomicBool>) {
    let path = cache_dir().join(file_name(&want.uuid, want.variant));
    if path.is_file() {
        // (touched, so the pruning keeps covers in use)
        let _ = std::fs::File::options().append(true).open(&path).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        launch(slot, &want, &alive, &path);
        return;
    }
    let Some(url) = video_url(&want.uuid, want.variant) else { return };
    sone_host_shim::async_runtime::spawn(async move {
        let got: Result<Vec<u8>, String> = async {
            let client = crate::host().state::<AppState>().proxied_http.client().map_err(|e| e.to_string())?;
            let r = client.get(&url).send().await.map_err(|e| e.to_string())?;
            if !r.status().is_success() {
                return Err(format!("HTTP {}", r.status()));
            }
            r.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
        }.await;
        if !alive.load(Ordering::Relaxed) {
            return;
        }
        let bytes = match got {
            Ok(b) => b,
            Err(e) => return fail(slot, &alive, &want.uuid, &e),
        };
        let tmp = path.with_extension(format!("part{}", std::process::id()));
        let saved = std::fs::create_dir_all(cache_dir()).and_then(|_| std::fs::write(&tmp, &bytes)).and_then(|_| std::fs::rename(&tmp, &path));
        if let Err(e) = saved {
            let _ = std::fs::remove_file(&tmp);
            return fail(slot, &alive, &want.uuid, &format!("couldn't cache it: {e}"));
        }
        launch(slot, &want, &alive, &path);
    });
}

/// Trims the cover cache to `DISK_BUDGET`, oldest first (at startup, off the UI thread).
pub fn prune_disk_cache() {
    std::thread::spawn(|| {
        let Ok(rd) = std::fs::read_dir(cache_dir()) else { return };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd.filter_map(|e| e.ok())
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

// ------------------------------------------------------------------ the pipeline

fn scale_caps(px: u32) -> gst::Caps {
    // Width only: the height follows the picture's shape (covers are square), even for NV12.
    gst::Caps::builder("video/x-raw").field("format", "NV12").field("width", (px & !1) as i32).field("pixel-aspect-ratio", gst::Fraction::new(1, 1)).build()
}

/// filesrc → decodebin (video only) → videorate (≤ 30 fps, dropping only) → videoscale →
/// videoconvert (NV12) → appsink, on the system clock; no audio branch.
fn launch(slot: Slot, want: &Want, alive: &Arc<AtomicBool>, path: &std::path::Path) {
    if let Err(e) = try_launch(slot, want, alive, path) {
        fail(slot, alive, &want.uuid, &e);
    }
}

fn try_launch(slot: Slot, want: &Want, alive: &Arc<AtomicBool>, path: &std::path::Path) -> Result<(), String> {
    crate::video::init()?;
    let make = |name: &str| gst::ElementFactory::make(name).build().map_err(|e| format!("{name}: {e}"));
    let uri = gst::glib::filename_to_uri(path, None).map_err(|e| e.to_string())?;
    let src = gst::ElementFactory::make("uridecodebin").property("uri", uri.as_str())
        .build().map_err(|e| format!("uridecodebin: {e}"))?;
    // Never decode a sound track: stop autoplugging at it (its pad stays unlinked, which the
    // demuxer tolerates while the picture's is linked).
    src.connect("autoplug-continue", false, |args| {
        let caps = args.get(2).and_then(|v| v.get::<gst::Caps>().ok());
        let audio = caps.as_ref().and_then(|c| c.structure(0)).is_some_and(|s| s.name().starts_with("audio/"));
        Some((!audio).to_value())
    });
    let rate = gst::ElementFactory::make("videorate").property("drop-only", true).property("max-rate", MAX_FPS)
        .build().map_err(|e| format!("videorate: {e}"))?;
    let scale = make("videoscale")?;
    let convert = make("videoconvert")?;
    let filter = gst::ElementFactory::make("capsfilter").property("caps", scale_caps(want.px)).build().map_err(|e| format!("capsfilter: {e}"))?;
    let sink = gst_app::AppSink::builder().sync(true).max_buffers(1).enable_last_sample(false).build();
    let (live, uuid) = (alive.clone(), want.uuid.clone());
    let first = Arc::new(AtomicBool::new(true));
    sink.set_callbacks(gst_app::AppSinkCallbacks::builder()
        .new_sample(move |s| {
            let sample = s.pull_sample().map_err(|_| gst::FlowError::Eos)?;
            if !live.load(Ordering::Relaxed) {
                return Err(gst::FlowError::Flushing);
            }
            if let Some((w, h)) = push(slot, &sample)
                && first.swap(false, Ordering::Relaxed)
            {
                log::info!("[cover] first frame {} {uuid} {w}x{h}", slot.name());
                // the view lays the surface over the still now (see `showing`)
                FRAMED[slot as usize].store(true, Ordering::Relaxed);
                crate::controller::bump();
            }
            Ok(gst::FlowSuccess::Ok)
        })
        .build());

    let pipeline = gst::Pipeline::new();
    pipeline.add_many([&src, &rate, &scale, &convert, &filter, sink.upcast_ref()]).map_err(|e| e.to_string())?;
    gst::Element::link_many([&rate, &scale, &convert, &filter, sink.upcast_ref()]).map_err(|e| e.to_string())?;
    let rate_sink = rate.static_pad("sink").ok_or("videorate has no sink pad")?;
    src.connect_pad_added(move |_, pad| {
        let video = pad.current_caps().and_then(|c| c.structure(0).map(|s| s.name().starts_with("video/"))).unwrap_or(false);
        if video && !rate_sink.is_linked() {
            let _ = pad.link(&rate_sink);
        }
    });

    let paused = {
        let mut m = M.lock().unwrap();
        match m.live[slot as usize].as_mut() {
            Some(l) if Arc::ptr_eq(&l.alive, alive) => {
                // (drawn at another size while it downloaded)
                if l.want.px != want.px {
                    filter.set_property("caps", scale_caps(l.want.px));
                }
                l.pipe = Some(Pipe { pipeline: pipeline.clone(), filter: filter.clone() });
                PIPELINES.fetch_add(1, Ordering::Relaxed);
                l.paused
            }
            // the slot moved on while this downloaded
            _ => return Ok(()),
        }
    };
    let bus = pipeline.bus().ok_or("no bus")?;
    let (pl, live, uuid) = (pipeline.downgrade(), alive.clone(), want.uuid.clone());
    std::thread::Builder::new().name("cover-bus".into()).spawn(move || watch(slot, bus, pl, live, uuid)).map_err(|e| e.to_string())?;
    log::info!("[cover] pipeline up {} ({} alive)", slot.name(), PIPELINES.load(Ordering::Relaxed));
    pipeline.set_state(if paused { gst::State::Paused } else { gst::State::Playing }).map_err(|e| format!("could not start: {e}"))?;
    Ok(())
}

/// The bus, on its own thread while the pipeline lives: loop at the end, give up on an error.
fn watch(slot: Slot, bus: gst::Bus, pipeline: gst::glib::WeakRef<gst::Pipeline>, alive: Arc<AtomicBool>, uuid: String) {
    use gst::MessageView as M;
    while alive.load(Ordering::Relaxed) {
        let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) else { continue };
        let Some(pl) = pipeline.upgrade() else { break };
        match msg.view() {
            M::Eos(_) => {
                let _ = pl.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, gst::ClockTime::ZERO);
            }
            M::Error(e) => {
                log::debug!("[cover] {:?}", e.debug());
                drop(pl);
                fail(slot, &alive, &uuid, &e.error().to_string());
                break;
            }
            _ => {}
        }
    }
}

/// Copies an NV12 frame, tightly packed, into the slot's surface; its size.
fn push(slot: Slot, sample: &gst::Sample) -> Option<(u32, u32)> {
    let (buffer, caps) = (sample.buffer()?, sample.caps()?);
    let info = gst_video::VideoInfo::from_caps(caps).ok()?;
    if info.format() != gst_video::VideoFormat::Nv12 {
        return None;
    }
    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info).ok()?;
    let (w, h) = ((info.width() & !1) as usize, (info.height() & !1) as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let mut planes = [Vec::with_capacity(w * h), Vec::with_capacity(w * h / 2)];
    for (p, rows) in [(0usize, h), (1, h / 2)] {
        let data = frame.plane_data(p as u32).ok()?;
        let stride = frame.plane_stride()[p] as usize;
        for r in 0..rows {
            planes[p].extend_from_slice(&data[r * stride..r * stride + w]);
        }
    }
    let c = info.colorimetry();
    let matrix = if c.matrix() == gst_video::VideoColorMatrix::Bt601 { Matrix::Bt601 } else { Matrix::Bt709 };
    let format = if c.range() == gst_video::VideoColorRange::Range0_255 { Format::Nv12Full } else { Format::Nv12Video };
    static IDS: AtomicU64 = AtomicU64::new(0);
    let frame = ByteFrame { width: w as u32, height: h as u32, format, color: ColorInfo { matrix, transfer: Transfer::Srgb }, planes };
    surface(slot).push_bytes(frame, IDS.fetch_add(1, Ordering::Relaxed) + 1).ok()?;
    Some((w as u32, h as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_rendering_is_recognised() {
        use super::software_from;
        // a GPU machine with nothing forced
        assert!(!software_from(None, None, true));
        // wrap.sh's SONE_SOFTWARE_VULKAN: only lavapipe's driver file
        assert!(software_from(Some("/nix/store/x-mesa/share/vulkan/icd.d/lvp_icd.x86_64.json"), None, true));
        // a forced list that still has a hardware driver
        assert!(!software_from(Some("/a/nvidia_icd.json:/b/lvp_icd.x86_64.json"), None, true));
        assert!(!software_from(Some(""), None, true));
        // Mesa's switch, and a machine without a render node
        assert!(software_from(None, Some("1"), true));
        assert!(!software_from(None, Some("0"), true));
        assert!(software_from(None, None, false));
    }

    #[test]
    fn urls_follow_get_tidal_video_url() {
        assert_eq!(video_url("ab-cd-ef", 640).as_deref(), Some("https://resources.tidal.com/videos/ab/cd/ef/640x640.mp4"));
        assert_eq!(video_url("ab-cd-ef", 1280).as_deref(), Some("https://resources.tidal.com/videos/ab/cd/ef/1280x1280.mp4"));
        assert_eq!(video_url("https://x.test/a.mp4", 640).as_deref(), Some("https://x.test/a.mp4"));
        assert_eq!(video_url("", 640), None);
    }

    #[test]
    fn the_file_follows_the_drawn_pixels() {
        assert_eq!(variant_for(180.0), 640);
        assert_eq!(variant_for(640.0), 640);
        assert_eq!(variant_for(641.0), 1280);
        // the album page's 180 pt cover at 2x: the 640 file, scaled to 384 px (a 32 px step)
        assert_eq!(Want::new("u", 360.0), Some(Want { uuid: "u".into(), variant: 640, px: 384 }));
        // never scaled above the file
        assert_eq!(Want::new("u", 630.0).unwrap().px, 640);
        assert_eq!(Want::new("u", 1500.0), Some(Want { uuid: "u".into(), variant: 1280, px: 1280 }));
        assert_eq!(Want::new("", 300.0), None);
        assert_eq!(Want::new("u", 0.0), None);
    }

    #[test]
    fn the_full_player_takes_the_shared_slot_and_hides_the_page() {
        let page = Some(("p", 180.0));
        let drawer = Some(("n", 300.0));
        let s = Scene { page, drawer, full: None, scale: 1.0 };
        let [p, n] = wants(&s);
        assert_eq!(p.unwrap().uuid, "p");
        assert_eq!(n.as_ref().unwrap().px, 320);
        assert_eq!(n.unwrap().variant, 640);
        // the full player opens over both, drawn large: one decode, the 1280 file
        let s = Scene { full: Some(("n", 868.0)), ..s };
        let [p, n] = wants(&s);
        assert_eq!(p, None);
        assert_eq!(n, Some(Want { uuid: "n".into(), variant: 1280, px: 896 }));
        assert_eq!(wants(&Scene::default()), [None, None]);
    }

    #[test]
    fn lifecycle() {
        let w = Want::new("a", 180.0).unwrap();
        let run = |want: &Want, paused| Running { want: want.clone(), paused };
        // nothing wanted, nothing running
        assert_eq!(step(None, None, true), Step::Keep);
        // shows: start, paused if the window is unfocused
        assert_eq!(step(None, Some(&w), true), Step::Start { want: w.clone(), paused: false });
        assert_eq!(step(None, Some(&w), false), Step::Start { want: w.clone(), paused: true });
        // steady
        assert_eq!(step(Some(&run(&w, false)), Some(&w), true), Step::Keep);
        assert_eq!(step(Some(&run(&w, true)), Some(&w), false), Step::Keep);
        // focus
        assert_eq!(step(Some(&run(&w, false)), Some(&w), false), Step::Pause);
        assert_eq!(step(Some(&run(&w, true)), Some(&w), true), Step::Resume);
        // hidden, toggled off, page left, failed: the want is gone, so it stops
        assert_eq!(step(Some(&run(&w, false)), None, true), Step::Stop);
        // another album (a track change, another page): restart
        let b = Want::new("b", 180.0).unwrap();
        assert_eq!(step(Some(&run(&w, false)), Some(&b), true), Step::Start { want: b, paused: false });
        // the same cover drawn larger than its file: the 1280 file
        let big = Want::new("a", 900.0).unwrap();
        assert_eq!(step(Some(&run(&w, false)), Some(&big), true), Step::Start { want: big, paused: false });
        // the same file at another size: rescale in place
        let wider = Want::new("a", 300.0).unwrap();
        assert_eq!(step(Some(&run(&w, false)), Some(&wider), true), Step::Resize(320));
    }

    #[test]
    fn cache_names() {
        assert_eq!(file_name("ab-cd", 640), "ab-cd-640.mp4");
        assert!(file_name("https://x/y.mp4", 1280).ends_with("-1280.mp4"));
        assert!(!file_name("../../etc", 640).contains('/'));
    }
}
