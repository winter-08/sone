//! The signal path panel behind the quality badge (React SignalPathPanel, signal-path/types.ts
//! and FlowDiagramBody): what happens between the decoded stream and the DAC. A compact verdict
//! (PRISTINE or MODIFIED, a headline, the source and DAC), and the full path: TIDAL, decode,
//! mix and DAC with the three cables between them, every alteration and the verdict.
//!
//! While open it refreshes the backend's probe every 2 s (useSignalPathRefresh: OS mixer and
//! kernel hw_params drift that no event reports), off the UI thread.

use crate::App;
use crate::controller::View;
use crate::theme;
use crate::ui::{self, t, white};
use rune::{Color, El, Tracked, col, el, oklch, row, text};
use serde_json::Value;
use sone_host_shim::Manager;
use std::cell::RefCell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const HEARTBEAT: Duration = Duration::from_secs(2);
const EPS: f64 = 1e-3;

static ARROW_RIGHT: rune::Icon = rune::Icon { name: "lucide-arrow_right", view: 24.0, body: r#"<path d="M5 12h14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="m12 5 7 7-7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"# };
static ARROW_LEFT: rune::Icon = rune::Icon { name: "lucide-arrow_left", view: 24.0, body: r#"<path d="m12 19-7-7 7-7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="M19 12H5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"# };

/// The panel's window state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Panel {
    open: bool,
    /// The full path rather than the verdict.
    expanded: bool,
}

thread_local! {
    static PANEL: RefCell<Tracked<Panel>> = RefCell::new(Tracked::new(Panel::default()));
    static NEXT_REFRESH: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

/// The latest snapshot, and whether one landed since the window looked.
static SNAPSHOT: Mutex<Option<Value>> = Mutex::new(None);
static FRESH: AtomicBool = AtomicBool::new(false);

fn panel() -> Panel {
    PANEL.with_borrow(|p| **p)
}

fn set_panel(p: Panel) {
    PANEL.with_borrow_mut(|cur| {
        if *Tracked::untracked(cur) != p {
            **cur = p;
        }
    });
}

/// The quality badge's press: open the panel at its verdict.
pub fn open(_: &mut App) {
    set_panel(Panel { open: true, expanded: false });
    refresh();
}

pub fn close(_: &mut App) {
    set_panel(Panel::default());
    NEXT_REFRESH.set(None);
}

pub fn is_open() -> bool {
    PANEL.with_borrow(|p| Tracked::untracked(p).open)
}

fn refresh() {
    NEXT_REFRESH.set(Some(Instant::now() + HEARTBEAT));
    sone_host_shim::async_runtime::spawn(async move {
        let sp = tauri_app_lib::commands::utility::refresh_signal_path(crate::host().state::<tauri_app_lib::AppState>());
        *SNAPSHOT.lock().unwrap() = serde_json::to_value(sp).ok();
        FRESH.store(true, Ordering::Relaxed);
    });
}

/// From the window's poll: the heartbeat while open. Whether a new snapshot arrived (the view
/// should run), and when to look again.
pub fn tick() -> (bool, Option<Duration>) {
    if !is_open() {
        return (false, None);
    }
    let fresh = FRESH.swap(false, Ordering::Relaxed);
    let due = NEXT_REFRESH.get().unwrap_or_else(Instant::now);
    if Instant::now() >= due {
        refresh();
    }
    let next = NEXT_REFRESH.get().map(|d| d.saturating_duration_since(Instant::now()));
    // (the snapshot lands a moment after the refresh starts)
    (fresh, next.map(|n| n.min(Duration::from_millis(250))))
}

// ---- reading the snapshot (camelCase JSON, as the React panel reads it)

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k)?.as_str().filter(|s| !s.is_empty())
}
fn n(v: &Value, k: &str) -> Option<u64> {
    v.get(k)?.as_u64()
}
fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k)?.as_f64()
}
fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// Any naming (ALSA, GStreamer, pactl) to GStreamer's.
fn to_gst(s: &str) -> String {
    let up = s.to_uppercase().replace('-', "_");
    match up.as_str() {
        "S16_LE" => "S16LE".into(),
        "S24_LE" => "S24_32LE".into(),
        "S24_3LE" => "S24LE".into(),
        "S32_LE" => "S32LE".into(),
        "FLOAT_LE" => "F32LE".into(),
        "S16_BE" => "S16BE".into(),
        "S24_BE" => "S24_32BE".into(),
        "S24_3BE" => "S24BE".into(),
        "S32_BE" => "S32BE".into(),
        "FLOAT_BE" => "F32BE".into(),
        "FLOAT" | "FLOAT32" | "FLOAT32LE" => "F32LE".into(),
        "FLOAT64" | "FLOAT64LE" => "F64LE".into(),
        _ => up,
    }
}

pub fn display_format(s: Option<&str>) -> String {
    s.filter(|s| !s.is_empty()).map_or("—".into(), to_gst)
}

fn equivalent(a: Option<&str>, b: Option<&str>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() && to_gst(a) == to_gst(b))
}

/// "44.1 kHz", "48 kHz", "800 Hz"; None for nothing.
pub fn format_rate(hz: Option<u64>) -> Option<String> {
    let hz = hz.filter(|h| *h > 0)?;
    Some(if hz >= 1000 { format!("{} kHz", khz(hz)) } else { format!("{hz} Hz") })
}

fn khz(hz: u64) -> String {
    if hz % 1000 == 0 { format!("{}", hz / 1000) } else { format!("{:.1}", hz as f64 / 1000.0) }
}

fn short_rate(hz: Option<u64>) -> String {
    match hz.filter(|h| *h > 0) {
        None => "—".into(),
        Some(h) => format!("{}k", khz(h)),
    }
}

pub fn gain_db(factor: f64) -> String {
    if (factor - 1.0).abs() < EPS {
        return "0.0 dB".into();
    }
    if factor <= 0.0 {
        return "−∞ dB".into();
    }
    let db = 20.0 * factor.log10();
    format!("{}{db:.1} dB", if db >= 0.0 { "+" } else { "" })
}

/// The volume slider's position for the amplitude the backend applies (a cubic taper).
pub fn slider_percent(amplitude: f64) -> u64 {
    if amplitude <= 0.0 { 0 } else { (amplitude.cbrt() * 100.0).round() as u64 }
}

/// "iFi (by AMR) HD USB Audio" from "USB-Audio - iFi (by AMR) HD USB Audio at usb-…".
pub fn dac_name(sp: Option<&Value>) -> Option<String> {
    let sp = sp?;
    if let Some(card) = sp.get("dac").and_then(|d| s(d, "cardName")) {
        let mut c = card;
        if let Some(i) = c.find(" - ").filter(|i| *i > 0) {
            c = &c[i + 3..];
        }
        if let Some(i) = c.find(" at ").filter(|i| *i > 0) {
            c = &c[..i];
        }
        let c = c.trim();
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }
    s(sp, "outputDevice").map(str::to_string)
}

fn audio_bits(fmt: Option<&str>) -> u32 {
    match fmt.map(to_gst).as_deref() {
        Some("S16LE" | "S16BE") => 16,
        Some("S24LE" | "S24BE" | "S24_32LE" | "S24_32BE") => 24,
        Some("S32LE" | "S32BE") => 32,
        Some("F32LE" | "F32BE") => 24,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Pristine,
    Altered,
    Lossy,
}

/// conversionState: lossy only when bits are dropped or the rate changes.
fn conversion(from_fmt: Option<&str>, to_fmt: Option<&str>, from_rate: Option<u64>, to_rate: Option<u64>, source_fmt: Option<&str>) -> State {
    let rate_changed = matches!((from_rate, to_rate), (Some(a), Some(b)) if a != b);
    let source_bits = audio_bits(source_fmt);
    let from_bits = if source_bits > 0 { audio_bits(from_fmt).min(source_bits) } else { audio_bits(from_fmt) };
    let narrowed = audio_bits(to_fmt) < from_bits;
    if rate_changed || narrowed { State::Lossy } else { State::Altered }
}

/// deriveAlterations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Derived {
    pub user_vol: f64,
    pub norm_factor: f64,
    pub user_vol_altered: bool,
    pub norm_altered: bool,
    pub direct_alsa: bool,
    pub pristine: bool,
    pub lossy_format_change: bool,
    pub lossless_promotion: bool,
}

pub fn derive(sp: Option<&Value>) -> Derived {
    let null = Value::Null;
    let v = sp.unwrap_or(&null);
    let user_vol = f(v, "userVolume").unwrap_or(1.0);
    let norm_factor = f(v, "normGainFactor").unwrap_or(1.0);
    let user_vol_altered = (user_vol - 1.0).abs() > EPS;
    let norm_altered = b(v, "volumeNormalization") && (norm_factor - 1.0).abs() > EPS;
    let direct_alsa = s(v, "backend") == Some("DirectAlsa");
    let mixer = v.get("osMixer").filter(|m| m.is_object());
    let (up_fmt, up_rate) = if direct_alsa {
        (s(v, "outputFormat"), n(v, "outputRate"))
    } else {
        (mixer.and_then(|m| s(m, "sinkFormat")).or(s(v, "outputFormat")), mixer.and_then(|m| n(m, "sinkRate")).or(n(v, "outputRate")))
    };
    let dac = v.get("dac").filter(|d| d.is_object());
    let dac_matches = match dac {
        None => true,
        Some(d) if s(d, "state") != Some("Active") => true,
        Some(d) => equivalent(s(d, "format"), up_fmt) && n(d, "rate") == up_rate,
    };
    let format_changed = s(v, "decodedFormat").is_some() && s(v, "outputFormat").is_some() && !equivalent(s(v, "decodedFormat"), s(v, "outputFormat"));
    let lossy_format_change = format_changed && audio_bits(s(v, "outputFormat")) < audio_bits(s(v, "decodedFormat"));
    let pristine = sp.is_some()
        && direct_alsa
        && b(v, "exclusiveMode")
        && v.get("resampledFrom").is_none_or(Value::is_null)
        && v.get("formatFallbackFrom").is_none_or(Value::is_null)
        && !user_vol_altered
        && !norm_altered
        && !lossy_format_change
        && dac_matches;
    Derived { user_vol, norm_factor, user_vol_altered, norm_altered, direct_alsa, pristine, lossy_format_change, lossless_promotion: format_changed && !lossy_format_change }
}

/// "FLAC 24/96": the codec and bit depth / rate of what TIDAL sent.
fn source_summary(stream: Option<&Value>) -> String {
    let Some(i) = stream else { return String::new() };
    let mut parts = vec![];
    if let Some(c) = s(i, "codec") {
        parts.push(c.to_uppercase());
    }
    if let (Some(bits), Some(rate)) = (n(i, "bitDepth").filter(|x| *x > 0), n(i, "sampleRate").filter(|x| *x > 0)) {
        parts.push(format!("{bits}/{}", khz(rate)));
    }
    parts.join(" ")
}

/// The verdict's one line (SignalPathPanel's headline).
pub fn headline(sp: Option<&Value>, stream: Option<&Value>, d: &Derived) -> String {
    let Some(v) = sp.filter(|v| s(v, "backend").is_some()) else { return "Idle — no track playing".into() };
    let dac = v.get("dac").filter(|x| x.is_object());
    let summary = source_summary(stream);
    if dac.is_none() && s(v, "outputFormat").is_none() {
        "Pipeline starting…".into()
    } else if dac.and_then(|x| s(x, "state")) == Some("Closed") {
        "DAC inactive — output may be routed elsewhere".into()
    } else if d.pristine {
        if d.lossless_promotion {
            format!("{} → {} — lossless promotion, every source bit preserved", display_format(s(v, "decodedFormat")), display_format(s(v, "outputFormat")))
        } else if !summary.is_empty() {
            format!("{summary} reaches your DAC untouched")
        } else {
            "Source PCM reaches your DAC untouched".into()
        }
    } else if let (Some(a), Some(b)) = (n(v, "resampledFrom").filter(|x| *x > 0), n(v, "resampledTo").filter(|x| *x > 0)) {
        format!("Resampled {} → {}", format_rate(Some(a)).unwrap_or_default(), format_rate(Some(b)).unwrap_or_default())
    } else if let (Some(a), Some(b)) = (s(v, "formatFallbackFrom"), s(v, "formatFallbackTo")) {
        format!("DAC refused {a} — fell back to {b}")
    } else if d.lossy_format_change {
        format!("Bit-depth reduced {} → {}", display_format(s(v, "decodedFormat")), display_format(s(v, "outputFormat")))
    } else if d.norm_altered && d.user_vol_altered {
        "Samples scaled by volume slider and ReplayGain".into()
    } else if d.norm_altered {
        format!("ReplayGain applied · {}", gain_db(d.norm_factor))
    } else if d.user_vol_altered {
        format!("Volume slider scaling samples · {}%", slider_percent(d.user_vol))
    } else if let Some(server) = v.get("osMixer").and_then(|m| s(m, "server")).filter(|_| !d.direct_alsa) {
        format!("Routed through {server}")
    } else if !b(v, "bitPerfect") {
        "Bit-perfect mode off — pipeline at unity, not guaranteed".into()
    } else {
        "Pipeline pass-through".into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Alteration {
    pub state: State,
    pub label: &'static str,
    pub detail: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub title: &'static str,
    pub primary: String,
    pub secondary: Option<String>,
    pub tertiary: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cable {
    pub state: State,
    pub caption: String,
    pub alterations: Vec<Alteration>,
}

fn state_of(alts: &[Alteration]) -> State {
    if alts.iter().any(|a| a.state == State::Lossy) {
        State::Lossy
    } else if alts.is_empty() {
        State::Pristine
    } else {
        State::Altered
    }
}

/// FlowDiagramBody's nodes and cables.
pub fn flow(sp: Option<&Value>, stream: Option<&Value>, track_quality: Option<&str>) -> ([Node; 4], [Cable; 3]) {
    let null = Value::Null;
    let v = sp.unwrap_or(&null);
    let d = derive(sp);
    let codec = stream.and_then(|i| s(i, "codec")).map(str::to_uppercase);
    let quality = stream.and_then(|i| s(i, "audioQuality")).or(track_quality).map(str::to_string);
    let line = source_summary(stream);
    let mixer = v.get("osMixer").filter(|m| m.is_object());
    let (mix_fmt, mix_rate, mix_tertiary) = if d.direct_alsa {
        (s(v, "outputFormat"), n(v, "outputRate"), Some(if d.user_vol_altered || d.norm_altered { "+gain stage".to_string() } else { "pass-thru".to_string() }))
    } else if let Some(m) = mixer {
        let server = s(m, "server").unwrap_or("").to_string();
        let ch = n(m, "sinkChannels").filter(|c| *c > 0);
        (s(m, "sinkFormat"), n(m, "sinkRate"), Some(match ch { Some(c) => format!("{server} · {c}ch"), None => server }))
    } else {
        (None, None, Some("sw mixer".to_string()))
    };
    let dac = v.get("dac").filter(|x| x.is_object());
    let nodes = [
        Node { title: "TIDAL", primary: "stream".into(), secondary: Some(if line.is_empty() { "—".into() } else { line.clone() }), tertiary: quality },
        Node { title: "DECODE", primary: display_format(s(v, "decodedFormat")), secondary: format_rate(n(v, "decodedRate")), tertiary: n(v, "decodedChannels").filter(|c| *c > 0).map(|c| format!("{c}ch")) },
        Node { title: "MIX", primary: display_format(mix_fmt), secondary: format_rate(mix_rate), tertiary: mix_tertiary },
        Node {
            title: "DAC",
            primary: display_format(dac.and_then(|x| s(x, "format")).or(if d.direct_alsa { s(v, "outputFormat") } else { None })),
            secondary: format_rate(dac.and_then(|x| n(x, "rate")).or(if d.direct_alsa { n(v, "outputRate") } else { None })),
            tertiary: dac.and_then(|x| s(x, "cardName")).map(str::to_string).or_else(|| {
                if d.direct_alsa { n(v, "outputChannels").filter(|c| *c > 0).map(|c| format!("{c}ch")) } else if s(v, "backend").is_some() { Some("not detected".into()) } else { None }
            }),
        },
    ];

    let cable0 = Cable { state: State::Pristine, caption: codec.as_ref().map_or("lossless decompression".into(), |c| format!("{c} → PCM")), alterations: vec![] };

    // decode -> mix
    let mut c1 = vec![];
    let resampled = match (n(v, "resampledFrom").filter(|x| *x > 0), n(v, "resampledTo").filter(|x| *x > 0)) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    };
    if let Some((a, b)) = resampled {
        c1.push(Alteration { state: State::Lossy, label: "RESAMPLED", detail: format!("{} → {}", format_rate(Some(a)).unwrap_or_default(), format_rate(Some(b)).unwrap_or_default()), reason: "DAC does not accept the source rate at the chosen exclusive mode".into() });
    }
    let promotion = s(v, "promotedFrom").is_some() && s(v, "promotedTo").is_some() && s(v, "promotedTo") == s(v, "outputFormat");
    if promotion {
        c1.push(Alteration { state: State::Altered, label: "PROMOTED", detail: format!("{} → {}", display_format(s(v, "promotedFrom")), display_format(s(v, "promotedTo"))), reason: "Lossless zero-pad — sample values preserved".into() });
    }
    let repack = d.direct_alsa && s(v, "decodedFormat").is_some() && s(v, "outputFormat").is_some() && !equivalent(s(v, "decodedFormat"), s(v, "outputFormat"));
    if repack {
        c1.push(Alteration { state: State::Altered, label: "CONTAINER REPACK", detail: format!("{} → {}", display_format(s(v, "decodedFormat")), display_format(s(v, "outputFormat"))), reason: "audioconvert repacked the sample container — audio bits preserved, byte layout differs".into() });
    }
    let mixer_rate = mixer.and_then(|m| n(m, "sinkRate"));
    let sink_fmt = mixer.and_then(|m| s(m, "sinkFormat"));
    let decoded_rate = n(v, "decodedRate");
    let mix_rate_changed = matches!((decoded_rate, mixer_rate), (Some(a), Some(b)) if a != b);
    let mixer_diverges = !d.direct_alsa && sink_fmt.is_some() && s(v, "decodedFormat").is_some() && (!equivalent(s(v, "decodedFormat"), sink_fmt) || mix_rate_changed);
    let server = mixer.and_then(|m| s(m, "server")).unwrap_or("OS mixer").to_string();
    if mixer_diverges {
        let st = conversion(s(v, "decodedFormat"), sink_fmt, decoded_rate, mixer_rate, None);
        let spec = match mixer_rate {
            Some(r) => format!("{}/{}", display_format(sink_fmt), format_rate(Some(r)).unwrap_or_default()),
            None => display_format(sink_fmt),
        };
        c1.push(Alteration {
            state: st,
            label: "MIX CONVERSION",
            detail: format!("pipeline {}/{} → mixer {spec}", display_format(s(v, "decodedFormat")), format_rate(decoded_rate).unwrap_or_else(|| "null".into())),
            reason: if st == State::Altered {
                format!("{server} widened/repacked the stream into its internal mix format — audio bits preserved")
            } else if mix_rate_changed {
                format!("{server} resampled the stream to its internal mix rate")
            } else {
                format!("{server} reduced bit depth converting into its internal mix format")
            },
        });
    }
    let caption1 = if let Some((a, b)) = resampled {
        format!("resample {}→{}", short_rate(Some(a)), short_rate(Some(b)))
    } else if promotion {
        format!("promote {}→{}", display_format(s(v, "promotedFrom")), display_format(s(v, "promotedTo")))
    } else if mixer_diverges {
        "mix conversion".into()
    } else if repack {
        "container repack".into()
    } else {
        "pass-thru".into()
    };
    let cable1 = Cable { state: state_of(&c1), caption: caption1, alterations: c1 };

    // mix -> DAC
    let mut c2 = vec![];
    let fallback = match (s(v, "formatFallbackFrom"), s(v, "formatFallbackTo")) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    };
    if let Some((a, b)) = fallback {
        c2.push(Alteration { state: State::Lossy, label: "FORMAT FALLBACK", detail: format!("{} → {}", display_format(Some(a)), display_format(Some(b))), reason: "DAC rejected requested format — fell back to nearest accepted".into() });
    }
    if d.user_vol_altered {
        c2.push(Alteration { state: State::Altered, label: "VOLUME", detail: format!("{}% ({})", slider_percent(d.user_vol), gain_db(d.user_vol)), reason: "Software volume scales samples in the writer thread".into() });
    }
    if d.norm_altered {
        c2.push(Alteration { state: State::Altered, label: "REPLAYGAIN", detail: format!("{} (×{:.3})", gain_db(d.norm_factor), d.norm_factor), reason: "Loudness normalization scales samples before output".into() });
    }
    let os_muted = !d.direct_alsa && mixer.is_some_and(|m| b(m, "sinkMuted"));
    let sink_volume = mixer.and_then(|m| f(m, "sinkVolume")).unwrap_or(1.0);
    let os_volume_altered = !d.direct_alsa && mixer.is_some_and(|m| !b(m, "sinkMuted")) && (sink_volume - 1.0).abs() > 1e-3;
    if os_muted {
        c2.push(Alteration { state: State::Lossy, label: "OS MUTED", detail: format!("{server} sink is muted"), reason: "OS mixer is muted — samples are zeroed before reaching the DAC".into() });
    } else if os_volume_altered {
        c2.push(Alteration { state: State::Altered, label: "OS VOLUME", detail: format!("{}% ({})", mixer.and_then(|m| n(m, "sinkVolumePercent")).unwrap_or(0), gain_db(sink_volume)), reason: format!("{server} scales samples by this factor before the kernel write") });
    }
    let route: Vec<(String, Option<u64>)> = if d.direct_alsa {
        vec![]
    } else {
        mixer.and_then(|m| m.get("virtualRoute")).and_then(Value::as_array).map(|a| a.iter().map(|h| (h["name"].as_str().unwrap_or("").to_string(), h["rate"].as_u64())).collect()).unwrap_or_default()
    };
    if !route.is_empty() {
        c2.push(Alteration {
            state: State::Altered,
            label: "VIRTUAL ROUTING",
            detail: route.iter().map(|(name, r)| match r { Some(r) => format!("{name} ({})", short_rate(Some(*r))), None => name.clone() }).collect::<Vec<_>>().join(" → "),
            reason: "Audio passes through virtual sinks before the DAC — any processing they apply is invisible to SONE".into(),
        });
    }
    let known: Vec<&(String, Option<u64>)> = route.iter().filter(|h| h.1.is_some()).collect();
    let mut route_resamples = 0;
    for w in known.windows(2) {
        if w[0].1 != w[1].1 {
            route_resamples += 1;
            c2.push(Alteration { state: State::Lossy, label: "ROUTE RESAMPLE", detail: format!("{} {} → {} {}", w[0].0, short_rate(w[0].1), w[1].0, short_rate(w[1].1)), reason: "Virtual sinks run at different rates — audio is resampled between them".into() });
        }
    }
    let upstream_fmt = if d.direct_alsa { s(v, "outputFormat") } else { sink_fmt.or(s(v, "outputFormat")) };
    let route_out_rate = if let Some(last) = route.last() { last.1 } else { mixer_rate };
    let upstream_rate = if d.direct_alsa { n(v, "outputRate") } else if mixer.is_some() { route_out_rate } else { n(v, "outputRate") };
    let dac_active = dac.is_some_and(|x| s(x, "state") == Some("Active"));
    let dac_fmt = dac.and_then(|x| s(x, "format"));
    let dac_rate = dac.and_then(|x| n(x, "rate"));
    let dac_diverges = dac_active && upstream_fmt.is_some() && upstream_rate.is_some() && (!equivalent(dac_fmt, upstream_fmt) || dac_rate != upstream_rate);
    let untouched = !d.direct_alsa && route.is_empty() && !os_muted && !os_volume_altered && decoded_rate.is_some() && mixer_rate.is_some() && mixer_rate == decoded_rate;
    if dac_diverges {
        let st = conversion(upstream_fmt, dac_fmt, upstream_rate, dac_rate, if untouched { s(v, "decodedFormat") } else { None });
        c2.push(Alteration {
            state: st,
            label: "OS-LAYER CONVERSION",
            detail: format!("{} {}/{} → DAC {}/{}", if d.direct_alsa { "pipeline" } else { "mixer" }, display_format(upstream_fmt), format_rate(upstream_rate).unwrap_or_default(), display_format(dac_fmt), format_rate(dac_rate).unwrap_or_default()),
            reason: if st == State::Altered {
                format!("{server} widened/repacked the stream before it reached ALSA — audio bits preserved")
            } else if dac_rate != upstream_rate {
                format!("{server} resampled the stream before it reached ALSA")
            } else {
                format!("{server} reduced bit depth before it reached ALSA")
            },
        });
    }
    let device_rate = mixer.and_then(|m| n(m, "deviceRate"));
    let device_resample = !d.direct_alsa && !dac_active && upstream_rate.is_some() && device_rate.is_some() && upstream_rate != device_rate;
    if device_resample {
        c2.push(Alteration { state: State::Lossy, label: "OS-LAYER CONVERSION", detail: format!("mixer {} → device {}", format_rate(upstream_rate).unwrap_or_default(), format_rate(device_rate).unwrap_or_default()), reason: format!("{server} resampled the stream before it reached the device") });
    }
    let caption2 = if let Some((a, b)) = fallback {
        format!("fallback {}→{}", display_format(Some(a)), display_format(Some(b)))
    } else if os_muted {
        "OS muted".into()
    } else if dac_diverges || device_resample {
        "OS-layer conversion".into()
    } else if route_resamples > 0 {
        "route resample".into()
    } else if !route.is_empty() {
        "virtual routing".into()
    } else if os_volume_altered {
        format!("OS vol {}", gain_db(sink_volume))
    } else if d.user_vol_altered || d.norm_altered {
        "gain applied".into()
    } else {
        "pass-thru".into()
    };
    let cable2 = Cable { state: state_of(&c2), caption: caption2, alterations: c2 };
    (nodes, [cable0, cable1, cable2])
}

/// The full path's closing verdict.
pub fn verdict_line(d: &Derived, cables: &[Cable; 3]) -> (State, String) {
    let all: Vec<&Alteration> = cables.iter().flat_map(|c| &c.alterations).collect();
    let lossy = all.iter().filter(|a| a.state == State::Lossy).count();
    let altered = all.iter().filter(|a| a.state == State::Altered).count();
    if d.pristine {
        return (State::Pristine, if d.lossless_promotion { "PRISTINE · BIT-TRANSPARENT — LOSSLESS PROMOTION".into() } else { "PRISTINE — NO ALTERATIONS DETECTED".into() });
    }
    if lossy > 0 {
        let more = if altered > 0 { format!(" · {altered} MODIFIED") } else { String::new() };
        return (State::Lossy, format!("NOT PRISTINE — {lossy} LOSSY STAGE{}{more}", if lossy == 1 { "" } else { "S" }));
    }
    (State::Altered, if altered > 0 { format!("MODIFIED — {altered} STAGE{}", if altered == 1 { "" } else { "S" }) } else { "MODIFIED".into() })
}

// ---- drawing

fn green() -> Color {
    oklch(0.8, 0.17, 150.0)
}
fn amber() -> Color {
    oklch(0.84, 0.15, 80.0)
}
fn red() -> Color {
    oklch(0.7, 0.19, 25.0)
}
fn color(s: State) -> Color {
    match s {
        State::Pristine => green(),
        State::Altered => amber(),
        State::Lossy => red(),
    }
}

fn mono(size: f32, weight: f32) -> rune::TextStyle {
    t(size, weight).mono()
}

/// The panel, when open: over everything, the full-screen player included.
pub fn overlay(v: &View) -> Option<El> {
    let p = panel();
    if !p.open {
        return None;
    }
    let snap = SNAPSHOT.lock().unwrap().clone();
    let sp = snap.as_ref();
    let stream = v.stream_info.as_ref();
    let track_quality = v.current.as_ref().and_then(|c| c.track["audioQuality"].as_str().map(str::to_string));
    let d = derive(sp);
    let head = headline(sp, stream, &d);
    let (word, ring) = if d.pristine { ("PRISTINE", green()) } else { ("MODIFIED", amber()) };
    let body = if p.expanded { full(v, sp, stream, track_quality.as_deref(), &d, word, ring, &head) } else { compact(sp, stream, &d, word, ring, &head) };
    Some(ui::dialog_frame::<App>("sp-layer", "signal-path", "Signal path", ("sp-close", "Close dialog"), if p.expanded { 680.0 } else { 480.0 }, None, None, body, close))
}

fn compact(sp: Option<&Value>, stream: Option<&Value>, d: &Derived, word: &str, ring: Color, head: &str) -> El {
    let mut c = col().w_full().items_center().pt(20.0).gap(0.0)
        .child(el().size(176.0, 176.0).radius(88.0).border(3.0, ring.alpha(0.55)).mb(28.0).items_center().justify_center().a11y_hidden()
            .child(el().absolute(8.0, 8.0).size(160.0, 160.0).radius(80.0).border(1.0, ring.alpha(0.18)))
            .child(text(word.to_string()).font(mono(19.0, 700.0).tracking(3.4)).color(ring)))
        .child(text(head.to_string()).font(t(13.0, 500.0).lh(20.0)).color(white(theme::TEXT)).max_w(360.0).wrap().align(rune::Align::Center).mb(16.0).label(format!("{word}: {head}")));
    let summary = source_summary(stream);
    let dac = dac_name(sp);
    if !summary.is_empty() || dac.is_some() {
        let mut facts = col().items_center().gap(2.0).mb(16.0).max_w(360.0);
        if !summary.is_empty() {
            let q = stream.and_then(|i| s(i, "audioQuality")).map_or(String::new(), |q| format!(" · {q}"));
            facts = facts.child(text(format!("{summary}{q}")).font(mono(11.0, 500.0)).color(white(0.6)));
        }
        if let Some(n) = dac {
            facts = facts.child(text(n).font(t(11.0, 500.0)).color(white(0.6)).truncate());
        }
        c = c.child(facts);
    }
    if let Some(v) = sp {
        let mut flags = vec![];
        if b(v, "exclusiveMode") {
            flags.push("exclusive".to_string());
        }
        if b(v, "bitPerfect") {
            flags.push("bit-perfect".to_string());
        }
        if let Some(server) = v.get("osMixer").and_then(|m| s(m, "server")).filter(|_| !d.direct_alsa) {
            flags.push(server.to_string());
        }
        if let Some(be) = s(v, "backend") {
            flags.push(be.to_string());
        }
        if !flags.is_empty() {
            c = c.child(text(flags.join(" · ").to_uppercase()).font(mono(10.5, 500.0).tracking(1.0)).color(white(0.45)).mb(24.0));
        }
    }
    c.child(theme::focusable(row().key("sp-expand").role("button").label("See the full path")).h(32.0).px(16.0).gap(8.0).radius(16.0).items_center()
        .fill(white(0.06)).border(1.0, white(0.08)).hover(white(0.12)).cursor(rune::Cursor::Pointer)
        .on_press(|_: &mut App| set_panel(Panel { open: true, expanded: true }))
        .child(text("SEE THE FULL PATH").font(t(11.0, 600.0).tracking(1.2)).color(white(0.7)).a11y_hidden())
        .child(rune::icon(&ARROW_RIGHT, 12.0).color(white(0.7)).a11y_hidden()))
}

#[allow(clippy::too_many_arguments)]
fn full(v: &View, sp: Option<&Value>, stream: Option<&Value>, track_quality: Option<&str>, d: &Derived, word: &str, ring: Color, head: &str) -> El {
    let (nodes, cables) = flow(sp, stream, track_quality);
    let mut c = col().w_full().gap(0.0)
        .child(row().w_full().gap(12.0).items_center().pb(14.0).mr(40.0)
            .child(el().size(10.0, 10.0).radius(5.0).fill(ring).a11y_hidden())
            .child(col().grow(1.0).min_w(0.0).gap(2.0)
                .child(text(word.to_string()).font(mono(14.0, 700.0).tracking(2.1)).color(ring))
                .child(text(head.to_string()).font(t(11.0, 500.0)).color(white(0.6)).truncate())));
    // the track
    if let Some(cur) = &v.current {
        let summary = source_summary(stream);
        let quality = stream.and_then(|i| s(i, "audioQuality")).or(track_quality);
        let mut facts: Vec<String> = vec![];
        if !summary.is_empty() {
            facts.push(summary);
        }
        if let Some(q) = quality {
            facts.push(q.to_string());
        }
        if let Some(n) = dac_name(sp) {
            facts.push(n);
        }
        let mut chips = row().gap(6.0).items_center();
        if sp.is_some_and(|x| b(x, "exclusiveMode")) {
            chips = chips.child(text("EXCLUSIVE").font(mono(10.0, 500.0).tracking(1.0)).color(white(0.6)).px(6.0).py(2.0).radius(4.0).fill(white(0.06)));
        }
        if sp.is_some_and(|x| b(x, "bitPerfect")) {
            chips = chips.child(text("BIT-PERFECT").font(mono(10.0, 500.0).tracking(1.0)).color(green()).px(6.0).py(2.0).radius(4.0).fill(white(0.06)));
        }
        c = c.child(col().w_full().gap(4.0).py(12.0).border_sides([1.0, 0.0, 0.0, 0.0], white(0.06))
            .child(row().w_full().min_w(0.0).child(text(cur.title.clone()).font(t(13.0, 600.0)).color(white(theme::TEXT)).truncate().shrink())
                .child(text(format!(" — {}", cur.artist)).font(t(13.0, 400.0)).color(white(0.55)).truncate().shrink()))
            .child(row().w_full().gap(8.0).items_center().child(text(facts.join(" · ")).font(mono(10.0, 500.0)).color(white(0.45)).truncate().shrink()).child(rune::spacer()).child(chips)));
    }
    // the flow: four stages, three cables
    let mut diagram = row().w_full().items_stretch().pt(8.0).pb(28.0);
    for (i, node) in nodes.iter().enumerate() {
        let mut box_ = col().w(120.0).padding(8.0).radius(8.0).fill(white(0.04)).border(1.0, white(0.08)).items_center().gap(2.0)
            .child(text(node.title).font(t(10.0, 700.0).tracking(1.0)).color(white(0.45)))
            .child(text(node.primary.clone()).font(mono(12.0, 500.0)).color(white(theme::TEXT)).mt(4.0));
        if let Some(sec) = &node.secondary {
            box_ = box_.child(text(sec.clone()).font(mono(11.0, 500.0)).color(white(0.6)));
        }
        if let Some(ter) = &node.tertiary {
            box_ = box_.child(text(ter.clone()).font(t(9.0, 500.0)).color(white(0.45)).mt(2.0).truncate());
        }
        diagram = diagram.child(box_.label(format!("{}: {}", node.title, [Some(node.primary.clone()), node.secondary.clone(), node.tertiary.clone()].into_iter().flatten().collect::<Vec<_>>().join(", "))));
        if let Some(cable) = cables.get(i) {
            let col_c = color(cable.state);
            let wire = if cable.state == State::Pristine {
                col().w_full().gap(3.0).child(el().w_full().h(2.0).fill(col_c)).child(el().w_full().h(2.0).fill(col_c))
            } else {
                row().w_full().h(20.0).items_center().justify_center()
                    .child(el().pin(Some(0.0), Some(8.0), Some(0.0), None).h(4.0).fill(col_c))
                    .child(el().size(20.0, 20.0).radius(10.0).fill(theme::drawer_bg()).border(2.0, col_c).items_center().justify_center()
                        .child(text("╳").font(t(11.0, 700.0)).color(col_c)))
            };
            diagram = diagram.child(col().grow(1.0).min_w(40.0).px(4.0).gap(4.0).justify_center().items_center()
                .child(col().w_full().h(32.0).justify_center().child(wire))
                .child(text(cable.caption.clone()).font(mono(9.0, 500.0)).color(col_c))
                .label(format!("{}: {}", match cable.state { State::Pristine => "pristine", State::Altered => "altered", State::Lossy => "lossy" }, cable.caption)));
        }
    }
    c = c.child(diagram);
    // legend
    c = c.child(row().w_full().gap(16.0).justify_center().pb(16.0).a11y_hidden()
        .child(row().gap(6.0).items_center().child(col().gap(2.0).child(el().size(16.0, 2.0).fill(green())).child(el().size(16.0, 2.0).fill(green()))).child(text("PRISTINE").font(t(9.0, 500.0).tracking(1.0)).color(white(0.45))))
        .child(row().gap(6.0).items_center().child(el().size(16.0, 3.0).fill(amber())).child(text("ALTERED LOSSLESS").font(t(9.0, 500.0).tracking(1.0)).color(white(0.45))))
        .child(row().gap(6.0).items_center().child(el().size(16.0, 3.0).fill(red())).child(text("╳").font(t(9.0, 700.0)).color(red().alpha(0.7))).child(text("LOSSY").font(t(9.0, 500.0).tracking(1.0)).color(white(0.45)))));
    // every alteration
    let all: Vec<&Alteration> = cables.iter().flat_map(|c| &c.alterations).collect();
    if !all.is_empty() {
        let mut list = col().w_full().gap(8.0).py(14.0).border_sides([1.0, 0.0, 0.0, 0.0], white(0.06));
        for a in all {
            let ac = color(a.state);
            list = list.child(row().w_full().gap(12.0).items_start().label(format!("{}: {}. {}", a.label, a.detail, a.reason))
                .child(text("╳").font(t(12.0, 700.0)).color(ac).a11y_hidden())
                .child(text(a.label).font(t(11.0, 700.0).tracking(0.8)).color(ac).w(120.0).wrap().a11y_hidden())
                .child(text(a.detail.clone()).font(mono(11.0, 500.0)).color(white(theme::TEXT)).w(170.0).wrap().a11y_hidden())
                .child(text(a.reason.clone()).font(t(11.0, 400.0)).color(white(0.55)).grow(1.0).min_w(0.0).wrap().a11y_hidden()));
        }
        c = c.child(list);
    }
    // verdict, with the way back
    let (vs, line) = verdict_line(d, &cables);
    c.child(row().w_full().py(12.0).items_center().border_sides([1.0, 0.0, 0.0, 0.0], white(0.06))
        .child(theme::focusable(row().key("sp-back").role("button").label("Back to verdict")).h(26.0).px(10.0).gap(6.0).radius(13.0).items_center()
            .fill(white(0.06)).border(1.0, white(0.08)).hover(white(0.12)).cursor(rune::Cursor::Pointer)
            .on_press(|_: &mut App| set_panel(Panel { open: true, expanded: false }))
            .child(rune::icon(&ARROW_LEFT, 11.0).color(white(0.7)).a11y_hidden())
            .child(text("BACK").font(t(10.0, 600.0).tracking(1.0)).color(white(0.7)).a11y_hidden()))
        .child(text(line).font(t(11.0, 700.0).tracking(1.6)).color(color(vs)).grow(1.0).align(rune::Align::Center).wrap().mr(60.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn names_formats_and_rates_as_the_panel_does() {
        assert_eq!(display_format(Some("S24_LE")), "S24_32LE");
        assert_eq!(display_format(Some("s32le")), "S32LE");
        assert_eq!(display_format(None), "—");
        assert_eq!(format_rate(Some(44100)).as_deref(), Some("44.1 kHz"));
        assert_eq!(format_rate(Some(96000)).as_deref(), Some("96 kHz"));
        assert_eq!(gain_db(1.0), "0.0 dB");
        assert_eq!(gain_db(0.5), "-6.0 dB");
        assert_eq!(slider_percent(0.125), 50);
        assert_eq!(dac_name(Some(&json!({"dac": {"cardName": "USB-Audio - iFi HD USB Audio at usb-0000:00:14.0-3, high speed"}}))).as_deref(), Some("iFi HD USB Audio"));
    }

    #[test]
    fn exclusive_direct_alsa_at_unity_is_pristine() {
        let sp = json!({"backend": "DirectAlsa", "exclusiveMode": true, "bitPerfect": true, "userVolume": 1.0, "normGainFactor": 1.0,
            "decodedFormat": "S24LE", "outputFormat": "S24LE", "outputRate": 96000,
            "dac": {"cardName": "X", "format": "S24_3LE", "rate": 96000, "state": "Active"}});
        let d = derive(Some(&sp));
        assert!(d.pristine);
        let stream = json!({"codec": "flac", "bitDepth": 24, "sampleRate": 96000});
        assert_eq!(headline(Some(&sp), Some(&stream), &d), "FLAC 24/96 reaches your DAC untouched");
        let (_, cables) = flow(Some(&sp), Some(&stream), None);
        assert_eq!(verdict_line(&d, &cables).1, "PRISTINE — NO ALTERATIONS DETECTED");
    }

    #[test]
    fn a_shared_mixer_that_resamples_is_lossy() {
        let sp = json!({"backend": "Normal", "userVolume": 0.5, "normGainFactor": 1.0, "decodedFormat": "S16LE", "decodedRate": 44100,
            "osMixer": {"server": "PipeWire", "sinkFormat": "float32le", "sinkRate": 48000, "sinkChannels": 2, "sinkVolume": 1.0, "sinkVolumePercent": 100, "sinkMuted": false, "virtualRoute": [], "deviceRate": 48000}});
        let d = derive(Some(&sp));
        assert!(!d.pristine);
        assert_eq!(headline(Some(&sp), None, &d), "Pipeline starting…", "no output format or DAC yet");
        let (nodes, cables) = flow(Some(&sp), None, Some("LOSSLESS"));
        assert_eq!(nodes[2].tertiary.as_deref(), Some("PipeWire · 2ch"));
        assert_eq!(cables[1].state, State::Lossy);
        assert_eq!(cables[1].caption, "mix conversion");
        assert_eq!(cables[2].caption, "gain applied");
        assert_eq!(verdict_line(&d, &cables).1, "NOT PRISTINE — 1 LOSSY STAGE · 1 MODIFIED");
    }

    #[test]
    fn nothing_playing_reads_idle() {
        assert_eq!(headline(None, None, &derive(None)), "Idle — no track playing");
    }
}
