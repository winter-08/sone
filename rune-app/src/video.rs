//! Music videos: TIDAL serves HLS (H.264 + AAC, 180p to 1080p). GStreamer plays it: `playbin3`
//! downloads and demuxes, decodes on the GPU where it can (NVDEC `nvh264dec`, VA-API) and in
//! software otherwise, and plays the sound through the default output. Pictures go to an
//! `appsink`, which hands each frame over on the pipeline clock; we copy its NV12 planes into a
//! Rune [`NativeSurface`], which converts them to colour on the GPU and wakes the window.
//!
//! Pacing: the window runs at the display's refresh (120 Hz here) whatever the video's rate.
//! A frame is released when its presentation time comes up, `RENDER_DELAY` early to cover the
//! wake, the draw and the compositor, and Rune shows it at the next refresh, so each frame stays
//! up for the refreshes until the next one is due (25 fps at 120 Hz: 5, 5, 5, 5, 4). Sound and
//! picture share GStreamer's clock, which keeps them in sync. Nothing redraws between frames,
//! and a paused video draws nothing at all.
//!
//! Rune's own `Player` covers macOS only, so this is the Linux path; the surface is the same.

use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gst::prelude::*;
use gst_video::prelude::*;
use rune::native_surface::{ByteFrame, ColorInfo, Format, Matrix, Transfer};
use rune::{NativeSurface, Waker};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How early a frame leaves the pipeline: about one 120 Hz refresh for the wake and draw, plus
/// a little for the compositor. Lip sync tolerates far more; this keeps the picture on time.
const RENDER_DELAY: Duration = Duration::from_millis(12);

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Opening, buffering, or seeking: show the poster or the last frame with a spinner.
    Loading,
    Playing,
    Paused,
    Ended,
    Failed(String),
}

struct Shared {
    state: Mutex<Inner>,
    frames: AtomicU64,
    generation: u64,
}

struct Inner {
    want_play: bool,
    buffering: bool,
    seeking: bool,
    first_frame: bool,
    ended: bool,
    failed: Option<String>,
    size: Option<(u32, u32)>,
    /// Where to start once the pipeline has prerolled (a quality switch carries on from here).
    start: Option<f64>,
}

struct Session {
    pipeline: gst::Element,
    shared: Arc<Shared>,
}

static SURFACE: OnceLock<NativeSurface> = OnceLock::new();
static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static TRACE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

/// The surface every video draws into (one video plays at a time).
pub fn surface() -> &'static NativeSurface {
    SURFACE.get_or_init(NativeSurface::new)
}

/// From the main window's setup: each pushed frame wakes it to draw.
pub fn set_waker(w: Waker) {
    surface().set_waker(w);
}

fn changed() {
    crate::controller::bump();
}

fn notify(name: &str, payload: String) {
    crate::controller::on_backend_event(name, &payload);
}

/// `SONE_VIDEO_TRACE=path`: one line per frame handed to the surface and per frame Rune
/// submitted to the GPU (`push`/`draw`, frame id, monotonic ms, pts ms), to check pacing.
fn trace(kind: &str, id: u64, pts_ms: f64) {
    let t = TRACE.get_or_init(|| std::env::var_os("SONE_VIDEO_TRACE").or_else(|| std::env::var_os("SPIKE_VIDEO_TRACE")).and_then(|p| std::fs::File::create(p).ok()).map(Mutex::new));
    if let Some(f) = t {
        use std::io::Write;
        static T0: OnceLock<Instant> = OnceLock::new();
        let ms = T0.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0;
        let _ = writeln!(f.lock().unwrap(), "{kind} {id} {ms:.3} {pts_ms:.3}");
    }
}

fn init() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        gst::init().map_err(|e| e.to_string())?;
        // Prefer GPU decoders when present; software (avdec_h264) stays the fallback.
        let reg = gst::Registry::get();
        for name in ["nvh264dec", "vah264dec"] {
            if let Some(f) = reg.find_feature(name, gst::ElementFactory::static_type()) {
                f.set_rank(gst::Rank::PRIMARY + 10);
            }
        }
        if std::env::var_os("SONE_VIDEO_TRACE").or_else(|| std::env::var_os("SPIKE_VIDEO_TRACE")).is_some() {
            surface().set_observer(Arc::new(|t| {
                if t.completed_at.is_none() {
                    trace("draw", t.frame_id, 0.0);
                }
            }));
        }
        Ok(())
    })
    .clone()
}

/// Start playing `url` (an HLS master playlist, through hls_relay.rs) from `at` seconds,
/// replacing whatever video was open.
pub fn open(url: &str, volume: f32, at: f64) -> Result<(), String> {
    close();
    init()?;
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let shared = Arc::new(Shared {
        state: Mutex::new(Inner { want_play: true, buffering: false, seeking: false, first_frame: false, ended: false, failed: None, size: None, start: (at > 0.5).then_some(at) }),
        frames: AtomicU64::new(0),
        generation,
    });

    let pipeline = gst::ElementFactory::make("playbin3").property("uri", url).build().map_err(|e| e.to_string())?;
    // Start on the best variant the master playlist offers (React's hls.js player shows High
    // from the first segment); adaptive streaming still steps down if the network can't keep up.
    pipeline.downcast_ref::<gst::Bin>().ok_or("playbin3 is not a bin")?.connect_deep_element_added(|_, _, e| {
        let factory = e.factory().map(|f| f.name().to_string()).unwrap_or_default();
        if e.factory().is_some_and(|f| f.klass().contains("Decoder")) {
            log::info!("[video] decoder: {factory}");
        }
        if factory == "hlsdemux2" || factory == "hlsdemux" {
            if e.find_property("start-bitrate").is_some() {
                e.set_property("start-bitrate", 50_000_000u32);
            }
        }
    });
    // No subtitles; sound and picture only, with the software volume (it follows the app's).
    pipeline.set_property_from_str("flags", "video+audio+soft-volume");
    pipeline.set_property("volume", volume.clamp(0.0, 1.0) as f64);
    // Pacing checks only: no sound, so the pipeline runs on the system clock rather than the
    // audio device's (the verifier's virtual sink runs a few percent slow).
    if std::env::var_os("SPIKE_VIDEO_NO_AUDIO").is_some() {
        if let Ok(fake) = gst::ElementFactory::make("fakesink").property("sync", true).build() {
            pipeline.set_property("audio-sink", &fake);
        }
    }

    let sink = gst_app::AppSink::builder()
        .caps(&gst::Caps::builder("video/x-raw").field("format", gst::List::new(["NV12", "I420"])).build())
        .sync(true)
        .max_buffers(2)
        .enable_last_sample(false)
        .build();
    sink.set_property("render-delay", RENDER_DELAY.as_nanos() as u64);
    sink.set_property("qos", true);
    let sh = shared.clone();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |s| {
                let sample = s.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                push_sample(&sh, &sample);
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    let convert = gst::ElementFactory::make("videoconvert").build().map_err(|e| e.to_string())?;
    let bin = gst::Bin::new();
    bin.add_many([&convert, sink.upcast_ref()]).map_err(|e| e.to_string())?;
    convert.link(&sink).map_err(|e| e.to_string())?;
    let pad = convert.static_pad("sink").ok_or("videoconvert has no sink pad")?;
    bin.add_pad(&gst::GhostPad::with_target(&pad).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    pipeline.set_property("video-sink", &bin);

    let bus = pipeline.bus().ok_or("no bus")?;
    *SESSION.lock().unwrap() = Some(Session { pipeline: pipeline.clone(), shared: shared.clone() });
    let sh = shared.clone();
    let pl = pipeline.downgrade();
    std::thread::Builder::new().name("video-bus".into()).spawn(move || watch(bus, sh, pl)).map_err(|e| e.to_string())?;
    // Starting later in the video: preroll paused, seek, then play (see `watch`).
    let first = if at > 0.5 { gst::State::Paused } else { gst::State::Playing };
    pipeline.set_state(first).map_err(|e| format!("could not start: {e}"))?;
    changed();
    Ok(())
}

fn current() -> Option<(gst::Element, Arc<Shared>)> {
    SESSION.lock().unwrap().as_ref().map(|s| (s.pipeline.clone(), s.shared.clone()))
}

fn alive(sh: &Shared) -> bool {
    GENERATION.load(Ordering::Relaxed) == sh.generation
}

/// The bus, on its own thread for the session's lifetime: end, errors, buffering, seeks.
fn watch(bus: gst::Bus, sh: Arc<Shared>, pipeline: gst::glib::WeakRef<gst::Element>) {
    use gst::MessageView as M;
    while alive(&sh) {
        let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) else { continue };
        let Some(pl) = pipeline.upgrade() else { break };
        match msg.view() {
            M::Eos(_) => {
                sh.state.lock().unwrap().ended = true;
                changed();
                notify("video-ended", String::new());
            }
            M::Error(e) => {
                let text = e.error().to_string();
                log::warn!("[video] error: {text} ({:?})", e.debug());
                sh.state.lock().unwrap().failed = Some(text.clone());
                let _ = pl.set_state(gst::State::Null);
                changed();
                notify("video-error", serde_json::json!({ "message": text }).to_string());
            }
            // Network streams: hold playback while the buffer refills, as playbin asks.
            M::Buffering(b) => {
                let p = b.percent();
                let mut st = sh.state.lock().unwrap();
                if p < 100 && !st.buffering {
                    st.buffering = true;
                    drop(st);
                    let _ = pl.set_state(gst::State::Paused);
                    changed();
                } else if p >= 100 && st.buffering {
                    st.buffering = false;
                    let play = st.want_play;
                    drop(st);
                    if play {
                        let _ = pl.set_state(gst::State::Playing);
                    }
                    changed();
                }
            }
            M::AsyncDone(_) => {
                let mut st = sh.state.lock().unwrap();
                if let Some(at) = st.start.take() {
                    st.seeking = true;
                    drop(st);
                    let _ = pl.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, gst::ClockTime::from_nseconds((at * 1e9) as u64));
                    continue;
                }
                if st.seeking && st.want_play && pl.current_state() != gst::State::Playing && !st.buffering {
                    st.seeking = false;
                    drop(st);
                    let _ = pl.set_state(gst::State::Playing);
                    changed();
                    continue;
                }
                if st.seeking {
                    st.seeking = false;
                    drop(st);
                    changed();
                }
            }
            M::StateChanged(s) if msg.src().is_some_and(|src| src == pl.upcast_ref::<gst::Object>()) => {
                if s.current() == gst::State::Playing || s.current() == gst::State::Paused {
                    changed();
                }
            }
            _ => {}
        }
    }
}

/// Copy one decoded frame's planes, tightly packed, into the surface (NV12; I420's two chroma
/// planes are interleaved on the way, which costs less than converting in the pipeline).
fn push_sample(sh: &Arc<Shared>, sample: &gst::Sample) {
    if !alive(sh) {
        return;
    }
    let (Some(buffer), Some(caps)) = (sample.buffer(), sample.caps()) else { return };
    let Ok(info) = gst_video::VideoInfo::from_caps(caps) else { return };
    let Ok(frame) = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info) else { return };
    // NV12 needs even dimensions; drop a stray last row or column.
    let (w, h) = ((info.width() & !1) as usize, (info.height() & !1) as usize);
    if w == 0 || h == 0 {
        return;
    }
    let mut y = Vec::with_capacity(w * h);
    let mut uv = Vec::with_capacity(w * h / 2);
    let Ok(py) = frame.plane_data(0) else { return };
    let sy = frame.plane_stride()[0] as usize;
    for r in 0..h {
        y.extend_from_slice(&py[r * sy..r * sy + w]);
    }
    match info.format() {
        gst_video::VideoFormat::Nv12 => {
            let Ok(p) = frame.plane_data(1) else { return };
            let s = frame.plane_stride()[1] as usize;
            for r in 0..h / 2 {
                uv.extend_from_slice(&p[r * s..r * s + w]);
            }
        }
        gst_video::VideoFormat::I420 => {
            let (Ok(pu), Ok(pv)) = (frame.plane_data(1), frame.plane_data(2)) else { return };
            let (su, sv) = (frame.plane_stride()[1] as usize, frame.plane_stride()[2] as usize);
            for r in 0..h / 2 {
                let (ru, rv) = (&pu[r * su..r * su + w / 2], &pv[r * sv..r * sv + w / 2]);
                for c in 0..w / 2 {
                    uv.push(ru[c]);
                    uv.push(rv[c]);
                }
            }
        }
        _ => return,
    }
    let c = info.colorimetry();
    let matrix = if c.matrix() == gst_video::VideoColorMatrix::Bt601 || (c.matrix() == gst_video::VideoColorMatrix::Unknown && h < 720) { Matrix::Bt601 } else { Matrix::Bt709 };
    let format = if c.range() == gst_video::VideoColorRange::Range0_255 { Format::Nv12Full } else { Format::Nv12Video };
    // Displayed as browsers and mpv do on an sRGB screen: no transfer conversion of its own.
    let color = ColorInfo { matrix, transfer: Transfer::Srgb };
    let id = sh.frames.fetch_add(1, Ordering::Relaxed) + 1;
    let pts = buffer.pts().map(|t| t.nseconds() as f64 / 1e6).unwrap_or(0.0);
    if let Err(e) = surface().push_bytes(ByteFrame { width: w as u32, height: h as u32, format, color, planes: [y, uv] }, id) {
        log::warn!("[video] frame rejected: {e}");
        return;
    }
    trace("push", id, pts);
    let mut st = sh.state.lock().unwrap();
    let first = !st.first_frame || st.size != Some((w as u32, h as u32));
    st.first_frame = true;
    st.size = Some((w as u32, h as u32));
    drop(st);
    if first {
        changed();
    }
}

pub fn close() {
    let old = SESSION.lock().unwrap().take();
    GENERATION.fetch_add(1, Ordering::Relaxed);
    if let Some(s) = old {
        let _ = s.pipeline.set_state(gst::State::Null);
    }
    surface().clear();
    changed();
}

pub fn is_open() -> bool {
    SESSION.lock().unwrap().is_some()
}

pub fn play() {
    let Some((pl, sh)) = current() else { return };
    let mut st = sh.state.lock().unwrap();
    st.want_play = true;
    if st.ended {
        // Play after the end starts over, as the <video> element does.
        st.ended = false;
        st.seeking = true;
        drop(st);
        let _ = pl.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, gst::ClockTime::ZERO);
    } else {
        let buffering = st.buffering;
        drop(st);
        if buffering {
            changed();
            return;
        }
    }
    let _ = pl.set_state(gst::State::Playing);
    changed();
}

pub fn pause() {
    let Some((pl, sh)) = current() else { return };
    sh.state.lock().unwrap().want_play = false;
    let _ = pl.set_state(gst::State::Paused);
    changed();
}

pub fn seek(secs: f64) {
    let Some((pl, sh)) = current() else { return };
    {
        let mut st = sh.state.lock().unwrap();
        st.seeking = true;
        st.ended = false;
    }
    let t = gst::ClockTime::from_nseconds((secs.max(0.0) * 1e9) as u64);
    let _ = pl.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, t);
    changed();
}

pub fn set_volume(v: f32) {
    if let Some((pl, _)) = current() {
        pl.set_property("volume", v.clamp(0.0, 1.0) as f64);
    }
}

/// Playback position in seconds, if the pipeline knows it yet.
pub fn position() -> Option<f64> {
    let (pl, _) = current()?;
    pl.query_position::<gst::ClockTime>().map(|t| t.nseconds() as f64 / 1e9)
}

pub fn duration() -> Option<f64> {
    let (pl, _) = current()?;
    pl.query_duration::<gst::ClockTime>().map(|t| t.nseconds() as f64 / 1e9)
}

pub fn state() -> Option<State> {
    let (pl, sh) = current()?;
    let st = sh.state.lock().unwrap();
    Some(if let Some(e) = &st.failed {
        State::Failed(e.clone())
    } else if st.ended {
        State::Ended
    } else if !st.first_frame || st.buffering || st.seeking {
        if st.want_play || !st.first_frame { State::Loading } else { State::Paused }
    } else if st.want_play && pl.current_state() == gst::State::Playing {
        State::Playing
    } else if st.want_play {
        State::Loading
    } else {
        State::Paused
    })
}

/// The picture's size in pixels, once the first frame arrived.
pub fn size() -> Option<(u32, u32)> {
    let (_, sh) = current()?;
    let s = sh.state.lock().unwrap().size;
    s
}
