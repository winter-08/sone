//! The playback controller: SONE's React playback/queue logic, re-implemented in Rust.
//!
//! The contract is `PLAYBACK_SPEC.md` (written from the React sources; section numbers below
//! refer to it). The verifier's goldens, recorded from the React build, are the test.
//!
//! One actor owns all playback state and handles one message at a time, which gives the
//! "single logical player" and run-to-completion semantics the spec asks for (§0.1, §0.2).
//! Backend events, UI commands and timers all arrive as messages. After every message the
//! derived outputs (MPRIS, MCP mirror, tray tooltip, gapless slot, queue snapshot) are
//! recomputed and pushed only when they changed, which is what React's store subscriptions
//! and effects did (§0.3, §8).
//!
//! Music videos are queue items (`itemType: "video"`, as in React's playTrack): starting one
//! stops the audio and opens it in video.rs; pause, seek, volume, next/previous, repeat and
//! MPRIS act on it; its end advances the queue; any audio track closes it.
//!
//! Not in this slice yet (logged when hit): MCP `play_source` / `shuffle_source` and autoplay
//! radio.

use serde_json::{Value, json};
use sone_host_shim::{AppHandle, Manager};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri_app_lib::AppState;
use tauri_app_lib::commands::{auth, mcp as mcp_cmd, metadata, playback, scrobble, utility};
use tauri_app_lib::error::SoneError;
use tauri_app_lib::mcp::{NowPlayingSnapshot, QueueTrackSnapshot};
use tokio::sync::mpsc;

// ------------------------------------------------------------------ public surface (UI)

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Repeat::All,
            2 => Repeat::One,
            _ => Repeat::Off,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Cmd {
    TogglePlay,
    Next,
    Previous,
    SeekFraction(f32),
    SetVolume(f32),
    ToggleShuffle,
    CycleRepeat,
    /// Play this track id from the upcoming list (first occurrence).
    JumpTo(u64),
    /// Play a collection from its page (Play / Shuffle). `source` is {type,id,name}.
    PlayCollection { tracks: Vec<Value>, source: Value, album_mode: bool, shuffle: bool },
    /// A track row clicked in a collection (playFromSource, §2.3): play `tracks[index]` (even
    /// with shuffle on), the rest queued after it. `tracks` in the collection's own order.
    PlayFrom { tracks: Vec<Value>, index: usize, source: Value, album_mode: bool },
    /// An artist page's Play: fetch the top tracks (as the React page does), then play them.
    PlayArtist { id: u64, name: String, shuffle: bool },
    /// Play these track ids now (a track clicked in search or on Home).
    PlayIds(Vec<u64>),
    /// Put these track ids at the front of the manual queue, in order ("Play next").
    PlayNextIds(Vec<u64>),
    /// Append these track ids to the manual queue ("Add to queue").
    QueueIds(Vec<u64>),
    /// Remove the item at this index of the upcoming list (queue panel).
    RemoveAt(usize),
    /// Clear the upcoming list (does not stop the current track).
    ClearQueue,
    /// Bit-perfect on/off (UPA:715-766): saves and ramps volume, toggles normalization.
    SetBitPerfect(bool),
    /// Exclusive output on/off; turning it off also turns bit-perfect off (AI:1383-1432).
    SetExclusive(bool),
    SetMaxQuality(String),
    /// "Get Login Code": start the device-code sign-in with the embedded credentials.
    StartDeviceLogin,
    CancelLogin,
    /// "Sign in with TIDAL": PKCE with the embedded credentials, in the user's browser (no web
    /// view here for the embedded login window).
    StartPkceLogin,
    /// The redirect URL (or bare code) pasted back after the browser login.
    CompletePkceLogin(String),
    /// The video player's quality menu: re-resolve the stream at HIGH / MEDIUM / LOW and carry on
    /// from the same moment (VideoPlayer handleQualityChange).
    SetVideoQuality(String),
    /// The video overlay's close on a finished video (VideoPlayer onEnded → closeVideo).
    CloseVideo,
}

/// The current item when it is a music video (React currentVideoAtom).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VideoInfo {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub image: Option<String>,
    pub explicit: bool,
}

/// The sign-in screen's state (React Login).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Login {
    #[default]
    Idle,
    /// Waiting for the user to approve the shown code at the verification address.
    Waiting { code: String, uri: String },
    /// PKCE in the browser: TIDAL's login page is open there; the user pastes back the
    /// address it lands on (React Login handleSimplePkceBrowser / handleSubmitEmbeddedUrl).
    PkceWaiting { url: String, error: Option<String> },
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QItem {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub duration: u32,
}

/// What the window draws. A snapshot, replaced after every controller message.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub track_id: Option<u64>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub volume: f32,
    pub quality: Option<String>,
    pub current: Option<QItem>,
    pub queue: Vec<QItem>,
    /// The "playing from" collection's name, when there is one.
    pub playing_from: Option<String>,
    pub history: Vec<QItem>,
    pub bit_perfect: bool,
    pub exclusive: bool,
    pub signed_in: bool,
    pub login: Login,
    /// The current track's radio (mixes.TRACK_MIX) and main artist, for the drawer tabs.
    pub track_mix: Option<String>,
    pub artist_id: Option<u64>,
    /// The stream's quality badge text, as the React bar renders it.
    pub badge: Option<String>,
    /// The current track's album cover (a CDN uuid or URL), for the bar and the miniplayer.
    pub cover: Option<String>,
    /// The current item is a music video (video.rs plays it).
    pub video: Option<VideoInfo>,
    /// Counts videos started, so the window can open the overlay for each new one.
    pub video_session: u64,
    pub video_quality: String,
    /// A video is loaded in video.rs (playing, paused or loading), not closed.
    pub video_open: bool,
}

static VIEW: OnceLock<Mutex<View>> = OnceLock::new();
static VERSION: AtomicU64 = AtomicU64::new(0);
static TX: OnceLock<mpsc::UnboundedSender<Msg>> = OnceLock::new();

pub fn view() -> View {
    let mut v = VIEW.get_or_init(Default::default).lock().unwrap().clone();
    // The bar's clock runs between controller updates, like the React progress bar did.
    if let Some(p) = POSITION.get() {
        let p = p.lock().unwrap();
        v.position = p.interpolated().min(v.duration.max(0.0));
    }
    v
}

/// Ask every window to redraw (state outside the controller changed, e.g. the miniplayer).
pub fn bump() {
    VERSION.fetch_add(1, Ordering::Relaxed);
}

static SHOWN_SECOND: AtomicU64 = AtomicU64::new(u64::MAX);
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Changes a window should redraw for: (controller state, playback clock ticks).
///
/// While playing, the clock ticks only when the displayed second changes (the time label and
/// seek bar show whole seconds); redrawing every poll cost a full frame each time. Ticks are
/// counted apart from state changes so a window can land a tick without animation (a seek bar
/// spring re-run every second keeps the window drawing) while a seek or a new track glides.
pub fn versions() -> (u64, u64) {
    if view_playing() {
        let sec = POSITION.get().map(|p| p.lock().unwrap().interpolated() as u64).unwrap_or(0);
        if SHOWN_SECOND.swap(sec, Ordering::Relaxed) != sec {
            TICKS.fetch_add(1, Ordering::Relaxed);
        }
    }
    (VERSION.load(Ordering::Relaxed), TICKS.load(Ordering::Relaxed))
}

pub fn version() -> u64 {
    let (state, ticks) = versions();
    state.wrapping_add(ticks)
}

fn view_playing() -> bool {
    VIEW.get().map(|v| v.lock().unwrap().playing).unwrap_or(false)
}

/// The current track's id and whether it's playing, without cloning the whole view.
/// The playback clock now (interpolated between backend updates), and whether it's running.
pub fn clock() -> (f64, bool) {
    let playing = view_playing();
    let pos = POSITION.get().map(|p| p.lock().unwrap().interpolated()).unwrap_or(0.0);
    (pos, playing)
}

/// What the video player's window state follows: whether the current item is a video, how
/// many videos have started, and whether one is loaded (without cloning the whole view).
pub fn video_status() -> (bool, u64, bool) {
    VIEW.get().map(|v| { let v = v.lock().unwrap(); (v.video.is_some(), v.video_session, v.video_open) }).unwrap_or((false, 0, false))
}

pub fn signed_in() -> bool {
    VIEW.get().map(|v| v.lock().unwrap().signed_in).unwrap_or(false)
}

pub fn now_playing() -> (Option<u64>, bool) {
    VIEW.get().map(|v| { let v = v.lock().unwrap(); (v.track_id, v.playing) }).unwrap_or((None, false))
}

/// The streaming quality ceiling, read from the backend (it owns the setting).
pub fn max_quality() -> String {
    utility::get_max_quality(crate::host().state::<AppState>())
}

pub fn send_play_ids(ids: Vec<u64>) {
    send(Cmd::PlayIds(ids));
}

pub fn send(cmd: Cmd) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(Msg::Ui(cmd));
    }
}

/// The queue snapshot waiting on its 2 s debounce (§1.5), for `flush_snapshot`.
static PENDING_SNAPSHOT: Mutex<Option<String>> = Mutex::new(None);

/// Write a snapshot still waiting on its debounce now (the React teardown flush, AI:808-814).
/// Called on the way out, from whichever thread is exiting.
pub fn flush_snapshot() {
    let Some(s) = PENDING_SNAPSHOT.lock().unwrap().take() else { return };
    if let Err(e) = playback::save_playback_queue(crate::host().state::<AppState>(), s) {
        log::warn!("[rune] save_playback_queue on exit: {e}");
    }
}

/// Called for every event the backend emits (see main.rs).
pub fn on_backend_event(name: &str, payload: &str) {
    let relevant = name.starts_with("mcp:")
        || name.starts_with("mpris:")
        || name.starts_with("tray:")
        || matches!(name, "video-ended" | "video-error" | "track-finished" | "track-advanced" | "audio-error" | "audio-resampled" | "audio-bit-depth-changed" | "scrobble-auth-error");
    if relevant {
        if let Some(tx) = TX.get() {
            let _ = tx.send(Msg::Event(name.to_string(), payload.to_string()));
        }
    }
}

pub fn start(handle: AppHandle) {
    let (tx, rx) = mpsc::unbounded_channel();
    let _ = TX.set(tx.clone());
    let _ = POSITION.set(Mutex::new(Position::default()));
    sone_host_shim::async_runtime::spawn(async move {
        let mut p = Player::new(handle, tx);
        p.startup().await;
        p.run(rx).await;
    });
}

// ------------------------------------------------------------------ internals

enum Msg {
    Ui(Cmd),
    Event(String, String),
    /// Debounced gapless refresh; only the latest id fires (§3.3).
    GaplessDue(u64),
    GaplessRetry,
    PositionPoll(u64),
    SeekCorrection(u64),
    RateLimitResume { pgen: u64, retry: Option<Value>, context_first: bool },
    SaveDue(u64),
    LoginPoll(u64, Duration),
    /// Tracks fetched off the actor for an id request (§2.5): the successful ones in request
    /// order, how many were asked, and whether the UI asked (it hears about failures; MCP doesn't).
    Fetched { tracks: Vec<Value>, action: String, asked: usize, seq: u64, from_ui: bool },
}

const HISTORY_CAP: usize = 500;
const REENTRY_GUARD: Duration = Duration::from_millis(250);
const DEVICE_BUSY_RETRIES: u32 = 10;
const MAX_CONSECUTIVE_FAILS: u32 = 3;
const GAPLESS_DEBOUNCE: Duration = Duration::from_millis(250);
const GAPLESS_FAILURE_MEMO: Duration = Duration::from_millis(10_000);
const POLL_EVERY: Duration = Duration::from_millis(2000);
const VIDEO_POLL_EVERY: Duration = Duration::from_millis(500);
const SEEK_CORRECTION: Duration = Duration::from_millis(300);
const SETTLE_WINDOW: Duration = Duration::from_millis(3000);
const SAVE_DEBOUNCE: Duration = Duration::from_millis(2000);
const UNPLAYABLE_SUBSTATUS: [i64; 7] = [4005, 4010, 4030, 4031, 4032, 4034, 4035];

// ---- position interpolator (§8.4); shared so the UI clock can read it between messages

static POSITION: OnceLock<Mutex<Position>> = OnceLock::new();

#[derive(Default)]
struct Position {
    last: f64,
    at: Option<Instant>,
    playing: bool,
    loading: bool,
    generation: u64,
    reset_at: Option<Instant>,
    correction: u64,
}

impl Position {
    fn interpolated(&self) -> f64 {
        if !self.playing || self.loading {
            return self.last;
        }
        self.last + self.at.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0)
    }
}

fn pos() -> std::sync::MutexGuard<'static, Position> {
    POSITION.get().expect("position").lock().unwrap()
}

// ---- track JSON helpers (tracks stay JSON, as in the frontend and queue.json)

fn tid(t: &Value) -> u64 {
    t["id"].as_u64().unwrap_or(0)
}
fn qid_of(t: &Value) -> String {
    t["_qid"].as_str().map(str::to_string).unwrap_or_else(|| tid(t).to_string())
}
fn title(t: &Value) -> String {
    t["title"].as_str().unwrap_or("").to_string()
}
fn display_title(t: &Value) -> String {
    match t["version"].as_str().filter(|v| !v.is_empty()) {
        Some(v) => format!("{} ({v})", title(t)),
        None => title(t),
    }
}
fn duration(t: &Value) -> u32 {
    t["duration"].as_u64().unwrap_or(0) as u32
}
fn artist_names(t: &Value) -> Vec<String> {
    t["artists"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str().map(str::to_string)).collect()).unwrap_or_default()
}
/// `artists.map(name).join(", ") || artist.name || "Unknown Artist"` (MCP, scrobble, tooltip).
fn artist_join(t: &Value) -> String {
    let names = artist_names(t).join(", ");
    if !names.is_empty() {
        return names;
    }
    t["artist"]["name"].as_str().filter(|s| !s.is_empty()).unwrap_or("Unknown Artist").to_string()
}
/// getTrackArtistDiscordDisplay (§8.1): MAIN artists by `artistType`, featured after "ft.".
fn artist_mpris(t: &Value) -> String {
    let arts = t["artists"].as_array().cloned().unwrap_or_default();
    if arts.is_empty() {
        return t["artist"]["name"].as_str().filter(|s| !s.is_empty()).unwrap_or("Unknown Artist").to_string();
    }
    if arts.len() == 1 {
        return arts[0]["name"].as_str().unwrap_or("").to_string();
    }
    let main: Vec<String> = arts.iter().filter(|a| a["artistType"].as_str() == Some("MAIN")).filter_map(|a| a["name"].as_str().map(str::to_string)).collect();
    if main.is_empty() {
        return artist_names(t).join(", ");
    }
    let feat: Vec<String> = arts.iter().filter(|a| a["artistType"].as_str() != Some("MAIN")).filter_map(|a| a["name"].as_str().map(str::to_string)).collect();
    if feat.is_empty() {
        return main.join(", ");
    }
    let feat_s = if feat.len() == 1 { feat[0].clone() } else { format!("{} & {}", feat[..feat.len() - 1].join(", "), feat[feat.len() - 1]) };
    format!("{} ft. {}", main.join(", "), feat_s)
}
fn artist_primary(t: &Value) -> String {
    let arts = t["artists"].as_array().cloned().unwrap_or_default();
    let main = arts.iter().find(|a| a["artistType"].as_str().or(a["type"].as_str()) == Some("MAIN")).or(arts.first());
    main.and_then(|a| a["name"].as_str())
        .or(t["artist"]["name"].as_str())
        .unwrap_or("Unknown Artist")
        .to_string()
}
fn album_title(t: &Value) -> Option<String> {
    t["album"]["title"].as_str().filter(|s| !s.is_empty()).map(str::to_string)
}
fn is_video(t: &Value) -> bool {
    t["itemType"].as_str() == Some("video")
}
fn art_url(t: &Value) -> String {
    if is_video(t) {
        return t["imageId"].as_str().filter(|c| !c.is_empty()).map(|c| format!("https://resources.tidal.com/images/{}/320x180.jpg", c.replace('-', "/"))).unwrap_or_default();
    }
    match t["album"]["cover"].as_str() {
        Some(c) if c.starts_with("http") => c.to_string(),
        Some(c) if !c.is_empty() => format!("https://resources.tidal.com/images/{}/320x320.jpg", c.replace('-', "/")),
        _ => String::new(),
    }
}
fn normalize(mut t: Value) -> Value {
    if t.get("artist").map(Value::is_null).unwrap_or(true) {
        if let Some(a0) = t["artists"].get(0).cloned() {
            t["artist"] = a0;
        }
    }
    t
}
fn unavailable(t: &Value) -> bool {
    if t["streamReady"].as_bool() == Some(false) || t["allowStreaming"].as_bool() == Some(false) {
        return true;
    }
    // streamStartDate in the future (§1.4); compare as RFC 3339 text against now's prefix.
    false
}
fn qitem(t: &Value) -> QItem {
    QItem { id: tid(t), title: display_title(t), artist: artist_join(t), duration: duration(t) }
}
fn source_meta(s: &Value) -> Value {
    json!({"type": s["type"], "id": s["id"], "name": s["name"], "image": s["image"], "subtitle": s["subtitle"], "mixType": s["mixType"]})
}

/// "<bitDepth>-BIT <kHz>KHZ <CODEC>" (§8.1 qualityText).
fn quality_text(info: &Option<Value>) -> String {
    let Some(i) = info else { return String::new() };
    let mut parts = vec![];
    if let Some(b) = i["bitDepth"].as_u64() {
        parts.push(format!("{b}-BIT"));
    }
    if let Some(r) = i["sampleRate"].as_u64() {
        let k = r as f64 / 1000.0;
        parts.push(if k.fract() == 0.0 { format!("{}KHZ", k as u64) } else { format!("{k:.1}KHZ") });
    }
    if let Some(c) = i["codec"].as_str() {
        parts.push(c.to_uppercase());
    }
    parts.join(" ")
}

#[derive(Debug, Clone, PartialEq)]
enum Fail {
    Network,
    Unplayable,
    Transient,
    RateLimited(Option<u64>),
    Blocked(String),
}

fn classify(e: &SoneError) -> Fail {
    match e {
        SoneError::ProxyBlocked { reason } => Fail::Blocked(reason.clone()),
        SoneError::Network(_) => Fail::Network,
        SoneError::Api { status: 429, body } => Fail::RateLimited(serde_json::from_str::<Value>(body).ok().and_then(|v| v["retryAfterSecs"].as_u64())),
        SoneError::Api { status: 404 | 410 | 451, .. } => Fail::Unplayable,
        SoneError::Api { status: 401, body } => {
            let sub = serde_json::from_str::<Value>(body).ok().and_then(|v| v["subStatus"].as_i64());
            if sub.is_some_and(|s| UNPLAYABLE_SUBSTATUS.contains(&s)) { Fail::Unplayable } else { Fail::Transient }
        }
        _ => Fail::Transient,
    }
}

/// Persisted UI preferences (the React localStorage keys `sone.volume.v1` etc.).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Prefs {
    volume: f32,
    shuffle: bool,
    repeat: u8,
    autoplay: bool,
    /// What bit-perfect replaced, to restore when it's turned off (sone.bitPerfect.previousState.v1).
    #[serde(default)]
    bit_perfect_prev: Option<(f32, bool)>,
}
impl Default for Prefs {
    fn default() -> Self {
        Prefs { volume: 1.0, shuffle: false, repeat: 0, autoplay: false, bit_perfect_prev: None }
    }
}
fn prefs_path() -> Option<std::path::PathBuf> {
    tauri_app_lib::config_dir_for_env().map(|d| d.join("rune-ui.json"))
}

#[derive(Default, PartialEq, Clone)]
struct Published {
    meta: Option<(u64, String, String, bool)>, // track id, qid, quality text, favourite
    playing: Option<bool>,
    shuffle: Option<bool>,
    repeat: Option<u8>,
    mcp: Option<String>,
    tooltip: Option<String>,
    gapless_key: Option<String>,
    snapshot_key: Option<String>,
    prefs_key: Option<String>,
}

struct PendingNext {
    track_id: u64,
    qid: String,
    track: Value,
    info: Value,
}

struct Player {
    h: AppHandle,
    tx: mpsc::UnboundedSender<Msg>,
    current: Option<Value>,
    queue: Vec<Value>,
    manual: Vec<Value>,
    history: Vec<Value>,
    original: Option<Vec<Value>>,
    playback_source: Option<Value>,
    context_source: Option<Value>,
    playing: bool,
    user_paused: bool,
    stream_info: Option<Value>,
    prefs: Prefs,
    use_track_gain: bool,
    generation: u64,
    last_play_invoke: Option<Instant>,
    fail_count: u32,
    autoplay_ids: HashSet<u64>,
    qid_counter: u64,
    bit_perfect: bool,
    exclusive: bool,
    gapless_setting: bool,
    gapless_supported: bool,
    pending_next: Option<PendingNext>,
    gapless_failed: Option<(u64, String, Instant)>,
    gapless_seq: u64,
    poll_seq: u64,
    save_seq: u64,
    resume_seq_gen: Option<u64>,
    restored: bool,
    /// Bumped per UI/MCP "play_now" id request; a fetch that lands after a newer one is dropped.
    fetch_seq: u64,
    signed_in: bool,
    login: Login,
    login_seq: u64,
    login_creds: Option<(String, String, String)>, // device code, client id, secret
    pkce: Option<(String, String)>, // code verifier, client unique key
    pubd: Published,
    video_session: u64,
    video_quality: String,
}

impl Player {
    fn new(h: AppHandle, tx: mpsc::UnboundedSender<Msg>) -> Self {
        let prefs = prefs_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Player {
            h,
            tx,
            current: None,
            queue: vec![],
            manual: vec![],
            history: vec![],
            original: None,
            playback_source: None,
            context_source: None,
            playing: false,
            user_paused: false,
            stream_info: None,
            prefs,
            use_track_gain: true,
            generation: 0,
            last_play_invoke: None,
            fail_count: 0,
            autoplay_ids: HashSet::new(),
            qid_counter: 0,
            bit_perfect: false,
            exclusive: false,
            gapless_setting: true,
            gapless_supported: false,
            pending_next: None,
            gapless_failed: None,
            gapless_seq: 0,
            poll_seq: 0,
            save_seq: 0,
            resume_seq_gen: None,
            restored: false,
            fetch_seq: 0,
            signed_in: false,
            login: Login::Idle,
            login_seq: 0,
            login_creds: None,
            pkce: None,
            pubd: Published::default(),
            video_session: 0,
            video_quality: "HIGH".into(),
        }
    }

    fn state(&self) -> sone_host_shim::State<'_, AppState> {
        self.h.state::<AppState>()
    }

    fn stamp(&mut self, mut t: Value) -> Value {
        self.qid_counter += 1;
        t["_qid"] = json!(format!("q{}", self.qid_counter));
        t
    }
    fn ensure_qid(&mut self, t: Value) -> Value {
        if t["_qid"].is_string() { t } else { self.stamp(t) }
    }

    /// An info toast in the main window (interact.rs draws them; its version wakes the poll).
    fn toast(&self, s: impl Into<String>) {
        let s = s.into();
        log::info!("[ui notice] {s}");
        crate::interact::toast(s, crate::interact::Kind::Info);
    }

    /// An error toast (React showToast(..., "error")).
    fn toast_error(&self, s: impl Into<String>) {
        let s = s.into();
        log::info!("[ui notice] {s}");
        crate::interact::toast(s, crate::interact::Kind::Error);
    }

    async fn startup(&mut self) {
        log::info!("[rune] controller startup");
        // Auth bootstrap, as AppInitializer does at launch (AI:285-304): load the saved tokens
        // into the TIDAL client, and resolve the user id from the session if they lack one.
        let h = self.h.clone();
        match auth::load_saved_auth(h.state::<AppState>()).await {
            Ok(Some(t)) => {
                if t.user_id.is_none() {
                    let _ = auth::get_session_user_id(h.state::<AppState>()).await;
                }
                self.signed_in = true;
                self.bootstrap_library();
            }
            Ok(None) => {}
            Err(e) => log::warn!("[rune] load_saved_auth: {e}"),
        }
        // Audio settings are backend-authoritative (AI:326-343).
        self.bit_perfect = utility::get_bit_perfect(self.state());
        self.exclusive = utility::get_exclusive_mode(self.state());
        self.gapless_setting = utility::get_gapless(self.state());
        self.gapless_supported = utility::get_gapless_supported();
        // Startup volume sync (§6.6).
        if !self.bit_perfect {
            let _ = playback::set_volume(self.state(), self.prefs.volume);
        }
        self.restore_snapshot();
        self.restored = true;
        log::info!("[rune] controller restored; first sync");
        self.sync().await;
    }

    async fn run(&mut self, mut rx: mpsc::UnboundedReceiver<Msg>) {
        log::info!("[rune] controller running");
        while let Some(msg) = rx.recv().await {
            match msg {
                Msg::Ui(c) => self.on_ui(c).await,
                Msg::Event(n, p) => self.on_event(&n, &p).await,
                Msg::GaplessDue(seq) if seq == self.gapless_seq => self.gapless_refresh().await,
                Msg::GaplessDue(_) => {}
                Msg::GaplessRetry => self.gapless_refresh().await,
                Msg::PositionPoll(seq) if seq == self.poll_seq => self.poll_position(true).await,
                Msg::PositionPoll(_) => {}
                Msg::SeekCorrection(seq) => {
                    if pos().correction == seq {
                        self.poll_position(false).await;
                    }
                }
                Msg::RateLimitResume { pgen, retry, context_first } => self.rate_limit_fire(pgen, retry, context_first).await,
                Msg::SaveDue(seq) if seq == self.save_seq => self.save_snapshot(),
                Msg::SaveDue(_) => {}
                Msg::LoginPoll(seq, every) => self.login_poll(seq, every).await,
                Msg::Fetched { tracks, action, asked, seq, from_ui } => self.on_fetched(tracks, &action, asked, seq, from_ui).await,
            }
            self.sync().await;
        }
    }

    fn later(&self, after: Duration, msg: Msg) {
        let tx = self.tx.clone();
        sone_host_shim::async_runtime::spawn(async move {
            tokio::time::sleep(after).await;
            let _ = tx.send(msg);
        });
    }

    /// What AppInitializer loads once signed in (AI:285-470): profile, favourite ids and the
    /// first pages of favourites, mixes, the feed (for the bell), Home and the playlist folders.
    fn bootstrap_library(&self) {
        use crate::data::{self, Key};
        for k in [Key::Home("static".into()), Key::Folders, Key::Feed, Key::Profile] {
            data::forget(&k);
            data::want(k);
        }
        let h = self.h.clone();
        sone_host_shim::async_runtime::spawn(async move {
            use tauri_app_lib::commands::library;
            let uid = data::user_id();
            let st = || h.state::<AppState>();
            let _ = tokio::join!(
                library::get_all_favorite_ids(st(), uid),
                library::get_favorite_albums(st(), h.clone(), uid, 0, 20, "DATE".into(), "DESC".into()),
                library::get_favorite_artists(st(), h.clone(), uid, 0, 20, "DATE".into(), "DESC".into()),
                library::get_favorite_tracks(st(), h.clone(), uid, 0, 50, "DATE".into(), "DESC".into()),
                // The id lists AppInitializer loads (same endpoints, but they don't fill the
                // caches the library tabs read, so opening a tab still fetches, as in React).
                library::get_favorite_video_ids(st(), uid),
                library::get_favorite_mix_ids(st())
            );
        });
    }

    async fn start_device_login(&mut self) {
        self.login_seq += 1;
        let (id, secret) = match auth::get_default_credentials() {
            Ok(c) => c,
            Err(e) => {
                self.login = Login::Failed(format!("No embedded credentials available. ({e})"));
                return;
            }
        };
        let h = self.h.clone();
        match auth::start_device_auth(h.state::<AppState>(), id.clone(), secret.clone()).await {
            Ok(r) => {
                let uri = r.verification_uri_complete.clone().unwrap_or_else(|| r.verification_uri.clone());
                let uri = if uri.starts_with("http") { uri } else { format!("https://{uri}") };
                // Like the React screen: open the approval page in the user's browser.
                let _ = std::process::Command::new("xdg-open").arg(&uri).spawn();
                self.login = Login::Waiting { code: r.user_code.clone(), uri };
                self.login_creds = Some((r.device_code.clone(), id, secret));
                let every = Duration::from_secs(r.interval.max(5));
                self.later(every, Msg::LoginPoll(self.login_seq, every));
            }
            Err(e) => self.login = Login::Failed(e.to_string()),
        }
    }

    async fn login_poll(&mut self, seq: u64, every: Duration) {
        if seq != self.login_seq || !matches!(self.login, Login::Waiting { .. }) {
            return;
        }
        let Some((code, id, secret)) = self.login_creds.clone() else { return };
        let h = self.h.clone();
        match auth::poll_device_auth(h.state::<AppState>(), h.clone(), code, id, secret).await {
            Ok(Some(_tokens)) => {
                self.login = Login::Idle;
                self.login_creds = None;
                self.signed_in = true;
                self.bootstrap_library();
            }
            Ok(None) => self.later(every, Msg::LoginPoll(seq, every)),
            Err(e) => {
                self.login = Login::Failed(e.to_string());
                self.login_creds = None;
            }
        }
    }

    fn start_pkce_login(&mut self) {
        self.login_seq += 1;
        let params = match auth::start_pkce_browser_login() {
            Ok(p) => serde_json::to_value(p).unwrap_or_default(),
            Err(e) => {
                self.login = Login::Failed(e.to_string());
                return;
            }
        };
        let field = |k: &str| params[k].as_str().unwrap_or_default().to_string();
        let url = field("authorizeUrl");
        self.pkce = Some((field("codeVerifier"), field("clientUniqueKey")));
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
        self.login = Login::PkceWaiting { url, error: None };
    }

    async fn complete_pkce_login(&mut self, pasted: String) {
        let Login::PkceWaiting { url, .. } = self.login.clone() else { return };
        let Some((verifier, key)) = self.pkce.clone() else { return };
        let Some(code) = pkce_code(&pasted) else {
            self.login = Login::PkceWaiting { url, error: Some("Could not find authorization code in the URL.".into()) };
            return;
        };
        match auth::complete_pkce_browser_login(self.h.clone(), code, verifier, key).await {
            Ok(_tokens) => {
                self.login = Login::Idle;
                self.pkce = None;
                self.signed_in = true;
                self.bootstrap_library();
            }
            // Like React: stay on the paste step with the error, so the user can try again.
            Err(e) => self.login = Login::PkceWaiting { url, error: Some(e.to_string()) },
        }
    }

    // -------------------------------------------------------------- inputs

    async fn on_ui(&mut self, c: Cmd) {
        match c {
            Cmd::TogglePlay => self.toggle_play().await,
            Cmd::Next => self.play_next(true, false).await,
            Cmd::Previous => self.play_previous().await,
            Cmd::SeekFraction(f) => {
                if let Some(t) = &self.current {
                    let s = f as f64 * duration(t) as f64;
                    self.seek_to(s).await;
                }
            }
            Cmd::SetVolume(v) => self.set_volume(v),
            Cmd::ToggleShuffle => self.toggle_shuffle(),
            Cmd::CycleRepeat => self.prefs.repeat = (self.prefs.repeat + 1) % 3,
            Cmd::PlayCollection { tracks, source, album_mode, shuffle } => {
                // usePlaySource: playAllFromSource refused (nothing playable) → info toast.
                if !self.play_all_src(tracks, Some(source), album_mode, shuffle).await {
                    self.toast("No playable tracks");
                }
            }
            Cmd::PlayFrom { tracks, index, source, album_mode } => self.play_from_src(tracks, index, source, album_mode).await,
            Cmd::PlayArtist { id, name, shuffle } => {
                // The React artist source pages the top tracks (view-all); when that fails it
                // plays what the page already loaded (its Top tracks module).
                let paged: Vec<Value> = match crate::data::get(crate::data::Key::ArtistTopTracks(id)).await {
                    Ok(v) => v["items"].as_array().or(v.as_array()).cloned().unwrap_or_default()
                        .into_iter().map(|t| if t.get("item").is_some_and(Value::is_object) { t["item"].clone() } else { crate::data::data(&t).clone() }).collect(),
                    Err(_) => vec![],
                };
                let tracks = if paged.is_empty() {
                    crate::data::get(crate::data::Key::Artist(id)).await.ok().map(|p| {
                        p["items"].as_array().cloned().unwrap_or_default().iter()
                            .flat_map(|sec| sec["items"].as_array().cloned().unwrap_or_default())
                            .filter(|it| crate::data::kind(it) == "TRACK")
                            .map(|it| crate::data::data(&it).clone()).collect::<Vec<_>>()
                    }).unwrap_or_default()
                } else { paged };
                // usePlaySource: nothing loaded → "Couldn't load tracks" (error).
                if tracks.is_empty() {
                    self.toast_error("Couldn't load tracks");
                    return;
                }
                let source = json!({"type": "artist", "id": id, "name": name});
                if !self.play_all_src(tracks, Some(source), false, shuffle).await {
                    self.toast("No playable tracks");
                }
            }
            Cmd::RemoveAt(i) => self.remove_from_queue(i),
            Cmd::ClearQueue => self.clear_queue(),
            Cmd::SetBitPerfect(on) => self.set_bit_perfect(on).await,
            Cmd::SetExclusive(on) => {
                if utility::set_exclusive_mode(self.state(), on).is_ok() {
                    self.exclusive = on;
                    if !on && self.bit_perfect {
                        self.set_bit_perfect(false).await;
                    }
                }
            }
            Cmd::StartDeviceLogin => self.start_device_login().await,
            Cmd::CancelLogin => {
                self.login_seq += 1;
                self.login = Login::Idle;
                self.login_creds = None;
                self.pkce = None;
            }
            Cmd::StartPkceLogin => self.start_pkce_login(),
            Cmd::CompletePkceLogin(pasted) => self.complete_pkce_login(pasted).await,
            Cmd::SetVideoQuality(q) => {
                if self.video_quality != q && self.current_is_video() {
                    self.video_quality = q;
                    let (id, at, was_playing) = (self.current.as_ref().map(tid).unwrap_or(0), crate::video::position().unwrap_or(0.0), self.playing);
                    match self.start_video(id, at).await {
                        Ok(()) => {
                            if !was_playing {
                                crate::video::pause();
                            }
                        }
                        Err(e) => self.toast_error(format!("Couldn't switch quality: {e}")),
                    }
                }
            }
            Cmd::CloseVideo => {
                crate::video::close();
                self.playing = false;
            }
            Cmd::SetMaxQuality(q) => {
                let _ = utility::set_max_quality(self.state(), q);
            }
            Cmd::QueueIds(ids) => self.fetch_tracks(ids, "queue", true),
            Cmd::PlayIds(ids) => self.fetch_tracks(ids, "play_now", true),
            Cmd::PlayNextIds(ids) => self.fetch_tracks(ids, "play_next", true),
            Cmd::JumpTo(id) => {
                if let Some(i) = self.upcoming().iter().position(|t| tid(t) == id) {
                    self.play_from_queue(i).await;
                }
            }
        }
    }

    async fn on_event(&mut self, name: &str, payload: &str) {
        log::debug!("[rune] event {name} {payload}");
        let p: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
        match name {
            // §7.1
            "mcp:play-tracks" => {
                let ids: Vec<u64> = p["trackIds"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
                self.fetch_tracks(ids, p["action"].as_str().unwrap_or(""), false);
            }
            "mcp:pause" => self.pause().await,
            "mcp:resume" => self.resume().await,
            "mcp:skip-next" | "tray:next-track" => self.play_next(true, false).await,
            "mcp:skip-previous" | "tray:prev-track" => self.play_previous().await,
            "mcp:seek" => {
                let s = p["positionSeconds"].as_f64().unwrap_or(0.0);
                self.seek_to(s).await
            }
            "mcp:set-volume" => self.set_volume(p["level"].as_f64().unwrap_or(1.0) as f32),
            "mcp:clear-queue" => self.clear_queue(),
            "mcp:remove-from-queue" => {
                let id = p["trackId"].as_u64().unwrap_or(0);
                if let Some(i) = self.upcoming().iter().position(|t| tid(t) == id) {
                    self.remove_from_queue(i);
                }
            }
            "mcp:set-repeat" => match p["mode"].as_str() {
                Some("off") => self.prefs.repeat = 0,
                Some("all") => self.prefs.repeat = 1,
                Some("one") => self.prefs.repeat = 2,
                _ => {}
            },
            "mcp:toggle-shuffle" => self.toggle_shuffle(),
            "mcp:play-source" | "mcp:shuffle-source" => log::warn!("[rune] {name} not ported yet: {payload}"),
            // §7.2 / §7.3
            "tray:toggle-play" => self.toggle_play().await,
            "mpris:play" => {
                if !self.playing {
                    self.resume().await
                }
            }
            "mpris:pause" => {
                if self.playing {
                    self.pause().await
                }
            }
            "mpris:stop" => {
                self.resume_seq_gen = None;
                crate::video::close();
                let _ = playback::stop_track(self.state()).await;
                self.pending_next = None;
                self.playing = false;
            }
            "mpris:seek" if self.current_is_video() => {
                let new = (crate::video::position().unwrap_or(0.0) + p.as_f64().unwrap_or(0.0)).max(0.0);
                self.seek_to(new).await;
            }
            "mpris:seek" => {
                let cur = playback::get_playback_position(self.state()).await.unwrap_or(0.0) as f64;
                let new = (cur + p.as_f64().unwrap_or(0.0)).max(0.0);
                if playback::seek_track(self.state(), new as f32).await.is_ok() {
                    self.notify_seek(new);
                }
            }
            "mpris:set-position" => self.seek_to(p.as_f64().unwrap_or(0.0).max(0.0)).await,
            "mpris:set-volume" => self.set_volume(p.as_f64().unwrap_or(1.0).clamp(0.0, 1.0) as f32),
            "mpris:set-shuffle" => {
                if p.as_bool().unwrap_or(false) != self.prefs.shuffle {
                    self.toggle_shuffle()
                }
            }
            "mpris:set-loop-status" => self.prefs.repeat = p.as_u64().unwrap_or(0).min(2) as u8,
            "mpris:set-fullscreen" => crate::full_ui::request(p.as_bool().unwrap_or(false)),
            "mpris:open-uri" => {
                if let Some(id) = p.as_str().and_then(|u| u.strip_prefix("tidal://track/")).and_then(|s| s.parse::<u64>().ok()) {
                    if let Ok(t) = metadata::get_track(self.state(), id).await {
                        self.play_single(t).await;
                    }
                }
            }
            // A video's end advances like a track's; when nothing follows (the same video is still
            // current), the player closes (VideoPlayer onEnded).
            "video-ended" => {
                let before = self.current.as_ref().map(qid_of);
                self.play_next(false, false).await;
                if self.current_is_video() && self.current.as_ref().map(qid_of) == before && self.prefs.repeat != 2 {
                    crate::video::close();
                    self.playing = false;
                }
            }
            "video-error" => {
                self.playing = false;
                self.toast_error("Unable to play this video");
            }
            // §3
            "track-finished" => {
                self.stream_info = None;
                self.play_next(false, false).await;
            }
            "track-advanced" => self.on_track_advanced(&p).await,
            "audio-error" => {
                self.playing = false;
                let kind = p["kind"].as_str().unwrap_or("");
                if matches!(kind, "device_disconnected" | "playback_error") {
                    self.stream_info = None;
                }
                let msg = if kind == "device_busy" {
                    "Audio device is busy — close other apps using it".to_string()
                } else {
                    match p["message"].as_str().filter(|m| !m.is_empty()) {
                        Some(m) if m.chars().count() > 80 => m.chars().take(80).collect::<String>() + "…",
                        Some(m) => m.to_string(),
                        None => "Playback error".into(),
                    }
                };
                self.toast_error(msg);
            }
            // AI:1051-1105: what exclusive / bit-perfect output had to change, and an expired
            // scrobbling session.
            "audio-resampled" => {
                let hz = |v: &Value| {
                    let n = v.as_f64().unwrap_or(0.0);
                    if n >= 1000.0 { format!("{}kHz", n / 1000.0) } else { format!("{n}Hz") }
                };
                self.toast(format!("DAC doesn't support {} — resampling to {}", hz(&p["from"]), hz(&p["to"])));
            }
            "audio-bit-depth-changed" => {
                self.toast(format!("DAC doesn't support {} — playing as {} (lossless)", p["from"].as_str().unwrap_or(""), p["to"].as_str().unwrap_or("")));
            }
            "scrobble-auth-error" => {
                let provider = p.as_str().unwrap_or("");
                let name = match provider {
                    "lastfm" => "Last.fm",
                    "listenbrainz" => "ListenBrainz",
                    "librefm" => "Libre.fm",
                    other => other,
                };
                self.toast_error(format!("{name} session expired — reconnect in Scrobbling settings"));
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------- §2 starting playback

    fn upcoming(&self) -> Vec<Value> {
        self.manual.iter().chain(self.queue.iter()).cloned().collect()
    }

    fn push_history(&mut self, t: Value) {
        self.history.push(t);
        if self.history.len() > HISTORY_CAP {
            let n = self.history.len() - HISTORY_CAP;
            self.history.drain(..n);
        }
    }

    fn mark_loading(&mut self, b: bool) {
        let mut p = pos();
        p.loading = b;
        if !b {
            p.at = Some(Instant::now());
        }
    }

    fn notify_seek(&mut self, t: f64) {
        let seq = {
            let mut p = pos();
            p.last = t;
            p.at = Some(Instant::now());
            p.loading = false;
            p.reset_at = None;
            p.correction += 1;
            p.correction
        };
        self.later(SEEK_CORRECTION, Msg::SeekCorrection(seq));
    }

    /// Start `t`: a music video through video.rs (stopping the audio first, as
    /// startVideoSession does), or a track through play_tidal_track (closing any video, the
    /// React mutual exclusion in playTrack).
    async fn invoke_play(&mut self, t: &Value) -> Result<Value, SoneError> {
        if is_video(t) {
            let _ = playback::stop_track(self.state()).await;
            self.pending_next = None;
            self.start_video(tid(t), 0.0).await?;
            return Ok(Value::Null);
        }
        if crate::video::is_open() {
            crate::video::close();
        }
        self.invoke_play_track(tid(t)).await
    }

    /// Resolve the video's HLS stream at the chosen quality and open it at `at` seconds.
    async fn start_video(&mut self, id: u64, at: f64) -> Result<(), SoneError> {
        // The stream goes through hls_relay.rs on the shared HTTP client, so the user's proxy
        // applies; when the plan blocks traffic, say so instead of failing quietly.
        if let Err(b) = self.state().proxied_http.client() {
            return Err(SoneError::ProxyBlocked { reason: b.to_string() });
        }
        let info = playback::get_video_stream_info(self.state(), id, Some(self.video_quality.clone())).await?;
        let url = crate::hls_relay::wrap(&info.url).map_err(SoneError::Audio)?;
        crate::video::open(&url, if self.bit_perfect { 1.0 } else { self.prefs.volume }, at).map_err(SoneError::Audio)?;
        self.video_session += 1;
        Ok(())
    }

    /// play_tidal_track with the device_busy retry (UPA:198-219).
    async fn invoke_play_track(&mut self, id: u64) -> Result<Value, SoneError> {
        let h = self.h.clone();
        let mut attempt = 0;
        loop {
            match playback::play_tidal_track(h.state::<AppState>(), id, self.use_track_gain).await {
                Ok(info) => {
                    // Starting a track drops the backend's armed next track (§3.3 note):
                    // forget ours too, so the following boundary re-arms.
                    self.pending_next = None;
                    return Ok(serde_json::to_value(info).unwrap_or(Value::Null));
                }
                Err(SoneError::Audio(m)) if m == "device_busy" && attempt < DEVICE_BUSY_RETRIES => {
                    if attempt == 0 {
                        self.playing = false;
                        self.toast("Preparing exclusive audio…");
                    }
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// playTrack (§2.1). `chosen_by_user`, `skip_history`, `quiet_unplayable`, `retry_rl`, `context_first`.
    async fn play_track(&mut self, track: Value, chosen_by_user: bool, skip_history: bool, quiet_unplayable: bool, retry_rl: bool, context_first: bool) -> Result<(), Fail> {
        if let Some(t) = self.last_play_invoke {
            if t.elapsed() < REENTRY_GUARD {
                return Err(Fail::Transient);
            }
        }
        self.last_play_invoke = Some(Instant::now());
        self.resume_seq_gen = None;
        self.user_paused = false;
        self.generation += 1;
        let pgen = self.generation;
        let n = normalize(track);
        let mut stamped = self.ensure_qid(n);
        let prev_current = self.current.clone();
        let prev_history = self.history.clone();
        if let Some(p) = prev_current.clone() {
            if !skip_history {
                self.push_history(p);
            }
        }
        stamped["_playingFrom"] = self.playback_source.clone().unwrap_or(Value::Null);
        stamped["_contextFrom"] = self.context_source.clone().unwrap_or(Value::Null);
        self.mark_loading(true);
        self.set_current(Some(stamped.clone()));
        // Every newly chosen video starts at High (VideoPlayer resets its quality per video).
        self.video_quality = "HIGH".into();
        let res = self.invoke_play(&stamped).await;
        if pgen != self.generation {
            return Err(Fail::Transient);
        }
        match res {
            Ok(info) => {
                self.stream_info = Some(info);
                self.playing = true;
                self.fail_count = 0;
                self.mark_loading(false);
                self.notify_started(&stamped, chosen_by_user).await;
                Ok(())
            }
            Err(e) => {
                self.set_current(prev_current);
                self.history = prev_history;
                self.playing = false;
                self.mark_loading(false);
                let f = classify(&e);
                match &f {
                    Fail::Blocked(r) => self.toast_error(r.clone()),
                    Fail::Network => self.toast_error("Network error — check your internet connection"),
                    Fail::RateLimited(after) => {
                        if retry_rl {
                            let _ = playback::stop_track(self.state()).await;
                            self.schedule_rate_limit(*after, Some(stamped.clone()), context_first);
                        } else {
                            self.schedule_rate_limit(*after, None, false);
                        }
                    }
                    Fail::Unplayable => {
                        if !quiet_unplayable {
                            self.toast("Track unavailable")
                        }
                    }
                    Fail::Transient => self.toast_error(format!("Playback failed: {e}")),
                }
                Err(f)
            }
        }
    }

    fn set_current(&mut self, t: Option<Value>) {
        let changed = self.current.as_ref().map(qid_of) != t.as_ref().map(qid_of) || self.current.is_some() != t.is_some();
        self.current = t;
        if changed {
            // §8.4: on current-track change, reset the clock and open the settle window.
            let mut p = pos();
            p.generation += 1;
            p.last = 0.0;
            p.at = Some(Instant::now());
            p.reset_at = Some(Instant::now());
            p.correction += 1;
            drop(p);
            self.gapless_failed = None;
        }
    }

    async fn notify_started(&mut self, t: &Value, chosen_by_user: bool) {
        // Videos don't scrobble or report plays (startVideoSession doesn't).
        if is_video(t) {
            return;
        }
        let payload = scrobble::TrackStartedPayload {
            artist: artist_join(t),
            artist_primary: artist_primary(t),
            title: title(t),
            album: album_title(t),
            album_artist: None,
            duration_secs: duration(t),
            track_number: t["trackNumber"].as_u64().map(|n| n as u32),
            chosen_by_user,
            isrc: t["isrc"].as_str().map(str::to_string),
            track_id: Some(tid(t)),
            source_type: self.playback_source.as_ref().and_then(|s| s["type"].as_str().map(str::to_string)),
            source_id: self.playback_source.as_ref().map(|s| match &s["id"] {
                Value::String(x) => x.clone(),
                v => v.to_string(),
            }),
        };
        let h = self.h.clone();
        sone_host_shim::async_runtime::spawn(async move {
            let _ = scrobble::notify_track_started(h.state::<AppState>(), payload).await;
        });
    }

    fn schedule_rate_limit(&mut self, after: Option<u64>, retry: Option<Value>, context_first: bool) {
        let wait = after.unwrap_or(5) + 1;
        self.toast(format!("Too many requests — resuming in {wait}s"));
        self.resume_seq_gen = Some(self.generation);
        self.later(Duration::from_secs(wait), Msg::RateLimitResume { pgen: self.generation, retry, context_first });
    }

    async fn rate_limit_fire(&mut self, pgen: u64, retry: Option<Value>, context_first: bool) {
        if self.resume_seq_gen != Some(pgen) || pgen != self.generation || self.user_paused {
            return;
        }
        self.resume_seq_gen = None;
        match retry {
            None => self.play_next(false, false).await,
            Some(t) => {
                let r = self.play_track(t, true, false, false, true, context_first).await;
                if r == Err(Fail::Unplayable) && context_first {
                    self.play_next(true, true).await;
                }
            }
        }
    }

    /// setQueueTracks / setShuffledQueue (§2.3). `source` is the "playing from" collection
    /// meta ({type,id,name,...}); `all` its full track list (kept for repeat-all / previous).
    fn set_queue(&mut self, tracks: Vec<Value>, clear_manual: bool, shuffled: bool) {
        self.set_queue_src(tracks, clear_manual, shuffled, None, vec![], false)
    }

    fn set_queue_src(&mut self, tracks: Vec<Value>, clear_manual: bool, shuffled: bool, source: Option<Value>, all: Vec<Value>, album_mode: bool) {
        self.original = None;
        if clear_manual {
            self.manual.clear();
        }
        self.context_source = None;
        self.playback_source = source.map(|mut s| {
            let all: Vec<Value> = all.into_iter().map(|t| {
                let n = normalize(t);
                self.stamp(n)
            }).collect();
            s["tracks"] = Value::Array(all);
            s
        });
        self.use_track_gain = !album_mode;
        let stamped: Vec<Value> = tracks.into_iter().map(|t| {
            let n = normalize(t);
            self.stamp(n)
        }).collect();
        if shuffled {
            self.original = if self.prefs.shuffle { Some(stamped.clone()) } else { None };
            self.queue = fisher_yates(stamped);
        } else {
            self.queue = stamped;
        }
    }

    /// playAllFromSource without a source (§2.3), as MCP play_now uses it.
    async fn play_all(&mut self, tracks: Vec<Value>) -> bool {
        self.play_all_src(tracks, None, false, false).await
    }

    /// playAllFromSource (§2.3): the Play / Shuffle buttons on album, playlist and artist pages.
    async fn play_all_src(&mut self, tracks: Vec<Value>, source: Option<Value>, album_mode: bool, shuffle: bool) -> bool {
        let all = tracks.clone();
        let eligible: Vec<Value> = tracks.into_iter().filter(|t| !unavailable(t)).collect();
        if eligible.is_empty() {
            return false;
        }
        self.fail_count = 0;
        // Artist sources keep the manual queue; everything else clears it (UPA:182).
        let clear_manual = !source.as_ref().and_then(|s| s["type"].as_str()).is_some_and(|t| t == "artist" || t == "artist-tracks");
        let shuffled = shuffle || self.prefs.shuffle;
        let (first, rest) = if shuffled {
            let i = rand::random::<usize>() % eligible.len();
            let mut rest = eligible.clone();
            let first = rest.remove(i);
            (first, rest)
        } else {
            let mut rest = eligible;
            let first = rest.remove(0);
            (first, rest)
        };
        self.set_queue_src(rest, clear_manual, shuffled, source, all, album_mode);
        if self.play_track(first, true, false, false, true, true).await == Err(Fail::Unplayable) {
            self.play_next(true, true).await;
        }
        true
    }

    async fn play_single(&mut self, t: Value) {
        self.set_queue(vec![], false, false);
        let _ = self.play_track(t, true, false, false, true, false).await;
    }

    /// playFromSource (§2.3): play `tracks[index]`; the rest is what follows it, then what came
    /// before (wrap-around), shuffled when shuffle mode is on. The manual queue survives.
    async fn play_from_src(&mut self, tracks: Vec<Value>, index: usize, source: Value, album_mode: bool) {
        let Some(track) = tracks.get(index).cloned() else { return };
        let rest = wrap_after(&tracks, index);
        self.set_queue_src(rest, false, self.prefs.shuffle, Some(source), tracks, album_mode);
        self.fail_count = 0;
        if self.play_track(track, true, false, false, true, true).await == Err(Fail::Unplayable) {
            self.play_next(true, true).await;
        }
    }

    /// mcp:play-tracks and the UI's id plays (§2.5): fetch every id off the actor (a long list
    /// would otherwise hold up pause/next/seek and track-finished), then `on_fetched` applies it.
    fn fetch_tracks(&mut self, ids: Vec<u64>, action: &str, from_ui: bool) {
        if ids.is_empty() {
            return;
        }
        if action == "play_now" {
            self.fetch_seq += 1;
        }
        let (tx, h, action, asked, seq) = (self.tx.clone(), self.h.clone(), action.to_string(), ids.len(), self.fetch_seq);
        sone_host_shim::async_runtime::spawn(async move {
            // Keep the successful ones in their original order (MCPB:221-251).
            let futs = ids.into_iter().map(|id| {
                let h = h.clone();
                async move { metadata::get_track(h.state::<AppState>(), id).await.ok() }
            });
            let tracks: Vec<Value> = join_all(futs).await.into_iter().flatten().collect();
            let _ = tx.send(Msg::Fetched { tracks, action, asked, seq, from_ui });
        });
    }

    async fn on_fetched(&mut self, tracks: Vec<Value>, action: &str, asked: usize, seq: u64, from_ui: bool) {
        // play_now: the latest request wins; queue / play_next always land.
        if action == "play_now" && seq != self.fetch_seq {
            return;
        }
        // MCP stays silent on failures (§2.5); the UI says what it couldn't load (useMediaPlay).
        if from_ui {
            let missed = asked - tracks.len();
            if tracks.is_empty() {
                self.toast_error(if action == "play_now" { "Failed to play" } else { "Couldn't load tracks" });
            } else if missed > 0 {
                self.toast_error(format!("Couldn't load {missed} track{}", if missed == 1 { "" } else { "s" }));
            }
        }
        if tracks.is_empty() {
            return;
        }
        match action {
            "play_now" => {
                self.play_all(tracks).await;
            }
            "queue" => {
                for t in tracks {
                    let s = self.stamp(normalize(t));
                    self.manual.push(s);
                }
            }
            "play_next" => {
                for t in tracks.into_iter().rev() {
                    let s = self.stamp(normalize(t));
                    self.manual.insert(0, s);
                }
            }
            _ => {}
        }
    }

    fn remove_from_queue(&mut self, i: usize) {
        if i < self.manual.len() {
            self.manual.remove(i);
        } else if i - self.manual.len() < self.queue.len() {
            let t = self.queue.remove(i - self.manual.len());
            let q = qid_of(&t);
            if let Some(o) = &mut self.original {
                o.retain(|x| qid_of(x) != q);
            }
        }
    }

    async fn play_from_queue(&mut self, i: usize) {
        let up = self.upcoming();
        let Some(t) = up.get(i).cloned() else { return };
        if unavailable(&t) {
            self.toast("Track unavailable");
            return;
        }
        self.fail_count = 0;
        self.remove_from_queue(i);
        let _ = self.play_track(t, true, false, false, true, false).await;
    }

    fn clear_queue(&mut self) {
        self.resume_seq_gen = None;
        self.queue.clear();
        self.manual.clear();
        self.original = None;
        self.playback_source = None;
        self.context_source = None;
    }

    // -------------------------------------------------------------- §3 advancing

    fn record_unplayable(&mut self, toasted: &mut bool) -> bool {
        self.fail_count += 1;
        if self.fail_count >= MAX_CONSECUTIVE_FAILS {
            self.toast_error("Multiple tracks failed to play — stopped");
            self.fail_count = 0;
            self.playing = false;
            return true;
        }
        if !*toasted {
            self.toast("Track unavailable — skipping");
            *toasted = true;
        }
        false
    }

    /// playNext (§3.2). Boxed because the repeat-all/unplayable paths recurse.
    fn play_next(&mut self, explicit: bool, context_first: bool) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.user_paused = false;
            // Repeat-one, not explicit: replay in place (§3.2 step 2).
            if self.prefs.repeat == 2 && !explicit {
                if let Some(cur) = self.current.clone() {
                    self.mark_loading(true);
                    match self.invoke_play(&cur).await {
                        Ok(info) => {
                            self.stream_info = Some(info);
                            self.playing = true;
                            self.notify_seek(0.0);
                            self.notify_started(&cur, false).await;
                        }
                        Err(e) => {
                            self.mark_loading(false);
                            self.playing = false;
                            match classify(&e) {
                                Fail::RateLimited(a) => self.schedule_rate_limit(a, None, false),
                                Fail::Unplayable => self.toast("Track unavailable"),
                                Fail::Network => self.toast_error("Network error — check your internet connection"),
                                _ => self.toast_error(format!("Playback failed: {e}")),
                            }
                        }
                    }
                    return;
                }
            }
            let _ = playback::stop_track(self.state()).await;
            self.pending_next = None;
            if explicit {
                self.fail_count = 0;
            }
            let mut toasted = false;
            let skip_manual = context_first && self.queue.iter().any(|t| !unavailable(t));
            // Manual drain (step 7).
            if !skip_manual {
                while !self.manual.is_empty() {
                    let next = self.manual[0].clone();
                    if unavailable(&next) {
                        self.manual.remove(0);
                        if self.record_unplayable(&mut toasted) {
                            return;
                        }
                        continue;
                    }
                    self.manual.remove(0);
                    let saved = (self.playback_source.clone(), self.context_source.clone());
                    if let Some(src) = next.get("_source").filter(|s| !s.is_null()).cloned() {
                        if self.context_source.is_none() {
                            self.context_source = self.playback_source.clone();
                        }
                        let mut s = src;
                        s["tracks"] = json!([]);
                        self.playback_source = Some(s);
                    }
                    self.last_play_invoke = None;
                    match self.play_track(next.clone(), explicit, false, true, false, false).await {
                        Ok(()) => return,
                        Err(Fail::Unplayable) => {
                            (self.playback_source, self.context_source) = saved;
                            if self.record_unplayable(&mut toasted) {
                                return;
                            }
                        }
                        Err(_) => {
                            (self.playback_source, self.context_source) = saved;
                            self.manual.insert(0, next);
                            return;
                        }
                    }
                }
            }
            // Restore stashed context (step 8).
            if let Some(cs) = self.context_source.take() {
                self.playback_source = Some(cs);
            }
            // Context drain (step 9).
            while !self.queue.is_empty() {
                let next = self.queue[0].clone();
                let is_autoplay = self.autoplay_ids.contains(&tid(&next));
                self.autoplay_ids.remove(&tid(&next));
                self.queue.remove(0);
                let q = qid_of(&next);
                let orig = self.original.clone();
                if let Some(o) = &mut self.original {
                    o.retain(|x| qid_of(x) != q);
                }
                if unavailable(&next) {
                    if self.record_unplayable(&mut toasted) {
                        return;
                    }
                    continue;
                }
                self.last_play_invoke = None;
                match self.play_track(next.clone(), !is_autoplay, false, true, false, false).await {
                    Ok(()) => return,
                    Err(Fail::Unplayable) => {
                        if self.record_unplayable(&mut toasted) {
                            return;
                        }
                    }
                    Err(_) => {
                        self.queue.insert(0, next);
                        self.original = orig;
                        return;
                    }
                }
            }
            // Step 10.
            if skip_manual && !self.manual.is_empty() {
                return self.play_next(true, false).await;
            }
            // End of queue (step 11).
            if self.prefs.repeat == 1 {
                let src = self.context_source.clone().or(self.playback_source.clone());
                let src_tracks = src.as_ref().and_then(|s| s["tracks"].as_array().cloned()).filter(|t| !t.is_empty());
                let has_source = src_tracks.is_some();
                let raw: Vec<Value> = match src_tracks {
                    Some(t) => t,
                    None => self.history.iter().cloned().chain(self.current.clone()).collect(),
                };
                let all: Vec<Value> = raw.into_iter().filter(|t| !unavailable(t)).map(|t| {
                    let mut t = t;
                    if let Some(o) = t.as_object_mut() {
                        o.remove("_qid");
                    }
                    self.stamp(t)
                }).collect();
                if all.is_empty() {
                    self.playing = false;
                    return;
                }
                let ordered = if self.prefs.shuffle { fisher_yates(all.clone()) } else { all.clone() };
                let first = ordered[0].clone();
                self.queue = ordered[1..].to_vec();
                let fq = qid_of(&first);
                self.original = if self.prefs.shuffle { Some(all.into_iter().filter(|t| qid_of(t) != fq).collect()) } else { None };
                if !has_source {
                    self.history.clear();
                }
                match self.play_track(first.clone(), true, !has_source, false, false, false).await {
                    Err(Fail::Unplayable) => return self.play_next(false, false).await,
                    Err(Fail::RateLimited(_)) => {
                        self.queue.insert(0, first.clone());
                        if let Some(o) = &mut self.original {
                            o.insert(0, first);
                        }
                    }
                    _ => {}
                }
                return;
            }
            if self.prefs.autoplay {
                log::warn!("[rune] autoplay radio not ported yet; stopping at end of queue");
            }
            self.playing = false;
        })
    }

    /// track-advanced (§3.4): the audio thread already switched; reconcile.
    async fn on_track_advanced(&mut self, p: &Value) {
        let track_id = p["trackId"].as_u64().unwrap_or(0);
        let qid = p["qid"].as_str().unwrap_or("").to_string();
        let pending = self.pending_next.take();
        if let Some(i) = self.manual.iter().position(|t| qid_of(t) == qid) {
            self.manual.remove(i);
        } else {
            let i = self.queue.iter().position(|t| qid_of(t) == qid).or_else(|| self.queue.iter().position(|t| tid(t) == track_id));
            if let Some(i) = i {
                self.queue.remove(i);
                if let Some(o) = &mut self.original {
                    o.retain(|x| qid_of(x) != qid);
                }
            }
            self.autoplay_ids.remove(&track_id);
        }
        let (track, info) = match pending {
            Some(pn) if pn.track_id == track_id => (pn.track, pn.info),
            _ => {
                let h = self.h.clone();
                let (info, track) = tokio::join!(
                    playback::get_stream_info(h.state::<AppState>(), track_id, self.use_track_gain),
                    metadata::get_track(h.state::<AppState>(), track_id)
                );
                match (info, track) {
                    (Ok(i), Ok(t)) => (t, serde_json::to_value(i).unwrap_or(Value::Null)),
                    _ => {
                        log::warn!("[rune] track-advanced stash miss for {track_id}; could not resolve");
                        return;
                    }
                }
            }
        };
        // advanceToTrack (UPA:623-662)
        self.generation += 1;
        let t = normalize(track);
        let mut stamped = self.ensure_qid(t);
        if let Some(prev) = self.current.clone() {
            self.push_history(prev);
        }
        stamped["_playingFrom"] = self.playback_source.clone().unwrap_or(Value::Null);
        stamped["_contextFrom"] = self.context_source.clone().unwrap_or(Value::Null);
        self.set_current(Some(stamped.clone()));
        self.stream_info = Some(info);
        self.playing = !self.user_paused;
        self.fail_count = 0;
        self.mark_loading(false);
        self.notify_started(&stamped, false).await;
    }

    // -------------------------------------------------------------- §4.2 previous

    fn play_previous(&mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let at = pos().interpolated();
            if at > 3.0 {
                self.seek_to(0.0).await;
                return;
            }
            self.fail_count = 0;
            let _ = playback::stop_track(self.state()).await;
            self.pending_next = None;
            if !self.history.is_empty() {
                let snapshot = (self.current.clone(), self.queue.clone(), self.original.clone(), self.manual.clone(), self.playback_source.clone(), self.context_source.clone(), self.history.clone());
                let mut prev = self.history.pop().unwrap();
                if let Some(mut cur) = self.current.clone() {
                    cur["_source"] = self.playback_source.as_ref().map(source_meta).unwrap_or(Value::Null);
                    self.manual.insert(0, cur);
                }
                if let Some(pf) = prev.get("_playingFrom").cloned() {
                    self.playback_source = if pf.is_null() { None } else { Some(pf) };
                    self.context_source = prev.get("_contextFrom").cloned().filter(|v| !v.is_null());
                }
                prev["_playingFrom"] = self.playback_source.clone().unwrap_or(Value::Null);
                prev["_contextFrom"] = self.context_source.clone().unwrap_or(Value::Null);
                self.mark_loading(true);
                self.set_current(Some(prev.clone()));
                match self.invoke_play(&prev).await {
                    Ok(info) => {
                        self.stream_info = Some(info);
                        self.playing = true;
                        self.mark_loading(false);
                        self.notify_started(&prev, true).await;
                    }
                    Err(e) => {
                        let (c, q, o, m, ps, cs, hi) = snapshot;
                        self.set_current(c);
                        (self.queue, self.original, self.manual, self.playback_source, self.context_source, self.history) = (q, o, m, ps, cs, hi);
                        self.playing = false;
                        self.mark_loading(false);
                        match classify(&e) {
                            Fail::RateLimited(a) => self.toast(format!("Too many requests — try again in {}s", a.unwrap_or(5))),
                            Fail::Unplayable => self.toast("Track unavailable"),
                            Fail::Network => self.toast_error("Network error — check your internet connection"),
                            _ => self.toast_error(format!("Playback failed: {e}")),
                        }
                    }
                }
                return;
            }
            // History empty: source fallback, else restart (§4.2 step 5, quirk reproduced:
            // stop_track above already ran, so this seek targets a stopped pipeline).
            if let (Some(src), Some(cur)) = (self.playback_source.clone(), self.current.clone()) {
                let tracks = src["tracks"].as_array().cloned().unwrap_or_default();
                if let Some(idx) = tracks.iter().position(|t| tid(t) == tid(&cur)) {
                    let mut p = idx as isize - 1;
                    while p >= 0 && unavailable(&tracks[p as usize]) {
                        p -= 1;
                    }
                    if idx > 0 && p >= 0 {
                        let prev = self.stamp(tracks[p as usize].clone());
                        let snapshot = (self.current.clone(), self.queue.clone(), self.original.clone(), self.manual.clone());
                        if !self.manual.is_empty() {
                            let mut c = cur.clone();
                            c["_source"] = source_meta(&src);
                            self.manual.insert(0, c);
                        } else {
                            self.queue.insert(0, cur.clone());
                            if let Some(o) = &mut self.original {
                                let my = idx;
                                let at = o.iter().position(|x| tracks.iter().position(|t| tid(t) == tid(x)).is_some_and(|k| k > my)).unwrap_or(o.len());
                                o.insert(at, cur.clone());
                            }
                        }
                        self.mark_loading(true);
                        self.set_current(Some(prev.clone()));
                        match self.invoke_play(&prev).await {
                            Ok(info) => {
                                self.stream_info = Some(info);
                                self.playing = true;
                                self.mark_loading(false);
                                self.notify_started(&prev, true).await;
                            }
                            Err(e) => {
                                let (c, q, o, m) = snapshot;
                                self.set_current(c);
                                (self.queue, self.original, self.manual) = (q, o, m);
                                self.playing = false;
                                self.mark_loading(false);
                                self.toast_error(format!("Playback failed: {e}"));
                            }
                        }
                        return;
                    }
                }
            }
            if self.current.is_some() {
                self.seek_to(0.0).await;
            }
        })
    }

    // -------------------------------------------------------------- §5, §6

    fn toggle_shuffle(&mut self) {
        if !self.prefs.shuffle {
            self.original = Some(self.queue.clone());
            self.queue = fisher_yates(std::mem::take(&mut self.queue));
            self.prefs.shuffle = true;
        } else {
            if let Some(o) = self.original.take() {
                let present: HashSet<String> = self.queue.iter().map(qid_of).collect();
                self.queue = o.into_iter().filter(|t| present.contains(&qid_of(t))).collect();
            }
            self.prefs.shuffle = false;
        }
    }

    fn current_is_video(&self) -> bool {
        self.current.as_ref().is_some_and(is_video)
    }

    async fn pause(&mut self) {
        self.user_paused = true;
        if self.current_is_video() {
            crate::video::pause();
            self.playing = false;
            return;
        }
        if playback::pause_track(self.state()).await.is_ok() {
            self.playing = false;
        }
    }

    async fn resume(&mut self) {
        self.user_paused = false;
        let Some(cur) = self.current.clone() else { return };
        // A video still open resumes in place (play after its end starts it over); one that was
        // closed or restored from the last session starts again.
        if is_video(&cur) && crate::video::is_open() {
            crate::video::play();
            self.playing = true;
            return;
        }
        let finished = is_video(&cur) || playback::is_track_finished(self.state()).await.unwrap_or(true);
        let res = if finished {
            self.mark_loading(true);
            match self.invoke_play(&cur).await {
                Ok(info) => {
                    self.stream_info = Some(info);
                    self.notify_seek(0.0);
                    self.notify_started(&cur, true).await;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            playback::resume_track(self.state()).await
        };
        match res {
            Ok(()) => self.playing = true,
            Err(e) => {
                self.playing = false;
                self.mark_loading(false);
                match classify(&e) {
                    Fail::RateLimited(a) => self.toast(format!("Too many requests — try again in {}s", a.unwrap_or(5))),
                    Fail::Unplayable => self.toast("Track unavailable"),
                    Fail::Network => self.toast_error("Network error — check your internet connection"),
                    _ => self.toast_error(format!("Playback failed: {e}")),
                }
            }
        }
    }

    async fn toggle_play(&mut self) {
        if self.playing { self.pause().await } else { self.resume().await }
    }

    async fn seek_to(&mut self, secs: f64) {
        if self.current_is_video() {
            crate::video::seek(secs);
            self.notify_seek(secs);
            return;
        }
        if playback::seek_track(self.state(), secs as f32).await.is_ok() {
            self.notify_seek(secs);
        }
    }

    /// rampVolume (UPA:696-713): 12 steps over 300 ms, each applied to the backend.
    async fn ramp_volume(&mut self, from: f32, to: f32) {
        if (to - from).abs() < 1e-4 {
            return;
        }
        for i in 1..=12 {
            let v = from + (to - from) * i as f32 / 12.0;
            self.prefs.volume = v;
            let _ = playback::set_volume(self.state(), v);
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn set_bit_perfect(&mut self, on: bool) {
        if on == self.bit_perfect {
            return;
        }
        if on {
            let norm = utility::get_volume_normalization(self.state());
            self.prefs.bit_perfect_prev = Some((self.prefs.volume, norm));
            let from = self.prefs.volume;
            self.ramp_volume(from, 1.0).await;
            let _ = utility::set_volume_normalization(self.state(), false);
            if utility::set_bit_perfect(self.state(), true).is_ok() {
                self.bit_perfect = true;
            }
        } else {
            if utility::set_bit_perfect(self.state(), false).is_ok() {
                self.bit_perfect = false;
            }
            if let Some((vol, norm)) = self.prefs.bit_perfect_prev.take() {
                let from = self.prefs.volume;
                self.ramp_volume(from, vol).await;
                let _ = utility::set_volume_normalization(self.state(), norm);
            }
        }
    }

    fn set_volume(&mut self, level: f32) {
        // §6.6: ignored entirely under bit-perfect (no video in this slice).
        if self.bit_perfect {
            return;
        }
        self.prefs.volume = level.clamp(0.0, 1.0);
        let _ = playback::set_volume(self.state(), self.prefs.volume);
        crate::video::set_volume(self.prefs.volume);
    }

    // -------------------------------------------------------------- §3.3 gapless

    async fn gapless_refresh(&mut self) {
        let enabled = self.gapless_supported && self.gapless_setting && !self.exclusive && !self.bit_perfect && self.current.is_some() && !self.current_is_video();
        let next = if !enabled || self.prefs.repeat == 2 {
            None
        } else {
            let head = self.manual.first().or(self.queue.first()).cloned();
            head.filter(|h| {
                !unavailable(h) && !is_video(h)
                    && match h.get("_source").filter(|s| !s.is_null()) {
                        Some(s) => self.playback_source.as_ref().map(|p| p["id"] == s["id"]).unwrap_or(false),
                        None => true,
                    }
            })
        };
        let Some(next) = next else {
            self.pending_next = None;
            let _ = playback::clear_next_track(self.state());
            return;
        };
        let (id, q) = (tid(&next), qid_of(&next));
        if self.pending_next.as_ref().is_some_and(|p| p.track_id == id && p.qid == q) {
            return;
        }
        if let Some((fid, fq, until)) = &self.gapless_failed {
            if *fid == id && *fq == q && Instant::now() < *until {
                return;
            }
        }
        let h = self.h.clone();
        match playback::set_next_track(h.state::<AppState>(), id, q.clone(), self.use_track_gain).await {
            Ok(info) => {
                self.gapless_failed = None;
                self.pending_next = Some(PendingNext { track_id: id, qid: q, track: next, info: serde_json::to_value(info).unwrap_or(Value::Null) });
            }
            Err(e) => {
                log::debug!("[rune] set_next_track failed: {e}");
                self.gapless_failed = Some((id, q, Instant::now() + GAPLESS_FAILURE_MEMO));
            }
        }
    }

    // -------------------------------------------------------------- §8.4 position polling

    async fn poll_position(&mut self, reschedule: bool) {
        let pgen = pos().generation;
        if self.current_is_video() {
            // The pipeline's own clock; the bar's clock holds still while the video buffers.
            if let Some(v) = crate::video::position() {
                let mut p = pos();
                if p.generation == pgen {
                    p.last = v;
                    p.at = Some(Instant::now());
                    p.loading = matches!(crate::video::state(), Some(crate::video::State::Loading));
                }
            }
            if reschedule && self.playing {
                self.later(VIDEO_POLL_EVERY, Msg::PositionPoll(self.poll_seq));
            }
            return;
        }
        if let Ok(v) = playback::get_playback_position(self.state()).await {
            let mut p = pos();
            if p.generation == pgen {
                let interp = p.interpolated();
                let settling = p.reset_at.is_some_and(|t| t.elapsed() < SETTLE_WINDOW);
                if !(settling && v as f64 > interp + 2.0) {
                    p.last = v as f64;
                    p.at = Some(Instant::now());
                }
            }
        }
        if reschedule && self.playing {
            self.later(POLL_EVERY, Msg::PositionPoll(self.poll_seq));
        }
    }

    // -------------------------------------------------------------- §1.5 snapshot

    fn snapshot_json(&self) -> String {
        let strip = |t: &Value| {
            let mut t = t.clone();
            if let Some(o) = t.as_object_mut() {
                o.remove("_playingFrom");
                o.remove("_contextFrom");
            }
            t
        };
        let list = |v: &[Value]| v.iter().map(strip).collect::<Vec<_>>();
        let hist_start = self.history.len().saturating_sub(HISTORY_CAP);
        json!({
            "currentTrack": self.current.as_ref().map(strip),
            "queue": list(&self.queue),
            "history": list(&self.history[hist_start..]),
            "manualQueue": list(&self.manual),
            "originalQueue": self.original.as_ref().map(|o| list(o)),
            "playbackSource": self.playback_source,
            "contextSource": self.context_source,
        })
        .to_string()
    }

    fn save_snapshot(&mut self) {
        PENDING_SNAPSHOT.lock().unwrap().take();
        let s = self.snapshot_json();
        if let Err(e) = playback::save_playback_queue(self.state(), s) {
            log::warn!("[rune] save_playback_queue: {e}");
        }
    }

    fn restore_snapshot(&mut self) {
        let Ok(Some(s)) = playback::load_playback_queue(self.state()) else { return };
        let Ok(v) = serde_json::from_str::<Value>(&s) else { return };
        let clean = |t: &Value| -> Option<Value> {
            t["id"].as_u64()?;
            let mut t = t.clone();
            if let Some(o) = t.as_object_mut() {
                o.remove("_playingFrom");
                o.remove("_contextFrom");
            }
            Some(t)
        };
        let list = |k: &str| v[k].as_array().map(|a| a.iter().filter_map(clean).collect::<Vec<_>>());
        if let Some(c) = clean(&v["currentTrack"]) {
            self.current = Some(c);
        }
        if let Some(q) = list("queue") {
            self.queue = q;
        }
        if let Some(h) = list("history") {
            let n = h.len().saturating_sub(HISTORY_CAP);
            self.history = h[n..].to_vec();
        }
        if let Some(m) = list("manualQueue") {
            self.manual = m;
        }
        self.original = list("originalQueue");
        self.playback_source = v["playbackSource"].as_object().map(|_| v["playbackSource"].clone());
        self.context_source = v["contextSource"].as_object().map(|_| v["contextSource"].clone());
        // Advance the qid counter past every restored q<N> and stamp the unstamped.
        let all = self.current.iter().chain(&self.queue).chain(&self.history).chain(&self.manual).chain(self.original.iter().flatten());
        let max = all.filter_map(|t| t["_qid"].as_str()?.strip_prefix('q')?.parse::<u64>().ok()).max().unwrap_or(0);
        self.qid_counter = self.qid_counter.max(max);
        let q = std::mem::take(&mut self.queue);
        self.queue = q.into_iter().map(|t| self.ensure_qid(t)).collect();
        let m = std::mem::take(&mut self.manual);
        self.manual = m.into_iter().map(|t| self.ensure_qid(t)).collect();
    }

    // -------------------------------------------------------------- outputs (§8)

    async fn sync(&mut self) {
        let mut want = self.pubd.clone();

        // Position clock follows isPlaying (§8.4).
        if self.pubd.playing != Some(self.playing) {
            let mut p = pos();
            if self.playing {
                p.at = Some(Instant::now());
                p.playing = true;
            } else {
                let i = p.interpolated();
                p.last = i;
                p.at = Some(Instant::now());
                p.playing = false;
            }
            drop(p);
            if self.playing {
                self.poll_seq += 1;
                self.later(Duration::ZERO, Msg::PositionPoll(self.poll_seq));
            }
        }

        // MPRIS metadata (+ status without position) on current/streamInfo change (§8.1).
        if let Some(cur) = self.current.clone() {
            let key = (tid(&cur), qid_of(&cur), quality_text(&self.stream_info), false);
            if self.pubd.meta.as_ref() != Some(&key) {
                let md = playback::MprisMetadata {
                    track_id: tid(&cur),
                    title: display_title(&cur),
                    artist: artist_mpris(&cur),
                    album: album_title(&cur).unwrap_or_default(),
                    art_url: art_url(&cur),
                    duration_secs: duration(&cur) as f64,
                    url: format!("tidal://{}/{}", if is_video(&cur) { "video" } else { "track" }, tid(&cur)),
                    quality_text: key.2.clone(),
                    album_artist: cur["artist"]["name"].as_str().map(str::to_string),
                    track_number: cur["trackNumber"].as_u64().map(|n| n as u32),
                    disc_number: cur["volumeNumber"].as_u64().map(|n| n as u32),
                    content_created: cur["album"]["releaseDate"].as_str().map(str::to_string),
                    user_rating: Some(0.0),
                };
                let _ = playback::update_mpris_metadata(self.state(), md);
                let _ = playback::update_mpris_playback_status(self.state(), self.playing, None);
                want.meta = Some(key);
            }
        }
        if self.pubd.playing != Some(self.playing) {
            let at = pos().interpolated();
            let _ = playback::update_mpris_playback_status(self.state(), self.playing, Some(at));
            want.playing = Some(self.playing);
        }
        if self.pubd.shuffle != Some(self.prefs.shuffle) {
            let _ = playback::update_mpris_shuffle(self.state(), self.prefs.shuffle);
            want.shuffle = Some(self.prefs.shuffle);
        }
        if self.pubd.repeat != Some(self.prefs.repeat) {
            let _ = playback::update_mpris_loop_status(self.state(), self.prefs.repeat);
            want.repeat = Some(self.prefs.repeat);
        }

        // Tray tooltip on current change.
        let tip = self.current.as_ref().map(|t| format!("{} — {}", display_title(t), artist_names(t).join(", "))).unwrap_or_else(|| "Sone".into());
        if self.pubd.tooltip.as_ref() != Some(&tip) {
            let h = self.h.clone();
            let t2 = tip.clone();
            sone_host_shim::async_runtime::spawn(async move {
                let _ = utility::update_tray_tooltip(h, t2).await;
            });
            want.tooltip = Some(tip);
        }

        // MCP mirror on current/isPlaying/queue change (§8.5). positionSeconds stays 0, as
        // in the React build (its position source was never wired); see PLAYBACK_SPEC §8.5.
        let up = self.upcoming();
        let mcp_key = format!("{:?}|{}|{:?}", self.current.as_ref().map(qid_of), self.playing, up.iter().map(qid_of).collect::<Vec<_>>());
        if self.pubd.mcp.as_ref() != Some(&mcp_key) {
            let np = self.current.as_ref().map(|t| NowPlayingSnapshot {
                track_id: Some(tid(t)),
                title: title(t),
                artist: artist_join(t),
                album: album_title(t),
                duration_seconds: duration(t),
                position_seconds: 0,
                is_playing: self.playing,
            });
            let q: Vec<QueueTrackSnapshot> = up.iter().take(50).map(|t| QueueTrackSnapshot { id: tid(t), title: title(t), artist: artist_join(t) }).collect();
            let _ = mcp_cmd::mcp_publish_state(self.state(), np, Some(q)).await;
            want.mcp = Some(mcp_key);
        }

        // Gapless refresh, debounced 250 ms, on manual/queue/current/repeat/shuffle change.
        let gk = format!("{:?}|{:?}|{}|{}", self.current.as_ref().map(qid_of), up.iter().map(qid_of).collect::<Vec<_>>(), self.prefs.repeat, self.prefs.shuffle);
        if self.pubd.gapless_key.as_ref() != Some(&gk) {
            self.gapless_seq += 1;
            self.later(GAPLESS_DEBOUNCE, Msg::GaplessDue(self.gapless_seq));
            want.gapless_key = Some(gk);
        }

        // Queue snapshot, debounced 2 s (§1.5), only after restore finished.
        if self.restored {
            let sk = self.snapshot_json();
            if self.pubd.snapshot_key.as_ref() != Some(&sk) {
                if self.pubd.snapshot_key.is_some() {
                    self.save_seq += 1;
                    self.later(SAVE_DEBOUNCE, Msg::SaveDue(self.save_seq));
                    *PENDING_SNAPSHOT.lock().unwrap() = Some(sk.clone());
                }
                want.snapshot_key = Some(sk);
            }
        }

        // Preferences (the React localStorage keys).
        let pk = serde_json::to_string(&self.prefs).unwrap_or_default();
        if self.pubd.prefs_key.as_ref() != Some(&pk) {
            if self.pubd.prefs_key.is_some() {
                if let Some(p) = prefs_path() {
                    let _ = std::fs::write(p, &pk);
                }
            }
            want.prefs_key = Some(pk);
        }

        self.pubd = want;
        self.publish_view();
    }

    fn publish_view(&mut self) {
        let v = View {
            track_id: self.current.as_ref().map(tid),
            title: self.current.as_ref().map(display_title).unwrap_or_default(),
            artist: self.current.as_ref().map(artist_join).unwrap_or_default(),
            album: self.current.as_ref().and_then(album_title).unwrap_or_default(),
            playing: self.playing,
            position: pos().interpolated(),
            duration: self.current.as_ref().map(|t| duration(t) as f64).unwrap_or(0.0),
            shuffle: self.prefs.shuffle,
            repeat: Repeat::from_u8(self.prefs.repeat),
            volume: if self.bit_perfect { 1.0 } else { self.prefs.volume },
            quality: Some(quality_text(&self.stream_info)).filter(|s| !s.is_empty()),
            current: self.current.as_ref().map(qitem),
            queue: self.upcoming().iter().map(qitem).collect(),
            playing_from: self.playback_source.as_ref().and_then(|s| s["name"].as_str()).map(str::to_string),
            history: self.history.iter().rev().take(50).map(qitem).collect(),
            bit_perfect: self.bit_perfect,
            exclusive: self.exclusive,
            signed_in: self.signed_in,
            login: self.login.clone(),
            track_mix: self.current.as_ref().and_then(|t| t["mixes"]["TRACK_MIX"].as_str()).map(str::to_string),
            artist_id: self.current.as_ref().and_then(|t| t["artist"]["id"].as_u64().or(t["artists"][0]["id"].as_u64())),
            // With the stream gone (track ended), the React badge shows the track's tier.
            badge: badge_text(&self.stream_info).or_else(|| self.current.as_ref().and_then(|t| tier(t["audioQuality"].as_str()).map(str::to_string))),
            cover: self.current.as_ref().and_then(|t| if is_video(t) { t["imageId"].as_str() } else { t["album"]["cover"].as_str() }).filter(|c| !c.is_empty()).map(str::to_string),
            video: self.current.as_ref().filter(|t| is_video(t)).map(|t| VideoInfo {
                id: tid(t),
                title: display_title(t),
                artist: artist_join(t),
                image: t["imageId"].as_str().filter(|c| !c.is_empty()).map(str::to_string),
                explicit: t["explicit"].as_bool().unwrap_or(false),
            }),
            video_session: self.video_session,
            video_quality: self.video_quality.clone(),
            video_open: crate::video::is_open(),
        };
        // Signal windows only when something they draw changed. The controller republishes on
        // every backend event (position syncs among them), and each signal is an animated
        // commit; the clock itself advances through `versions()`' ticks. A position that jumps
        // from where the clock would be by now (a seek, a restart) counts as a change.
        static LAST: Mutex<Option<(f64, std::time::Instant, bool)>> = Mutex::new(None);
        let mut last = LAST.lock().unwrap();
        let expected = last.map(|(p, at, playing)| if playing { p + at.elapsed().as_secs_f64() } else { p });
        let jumped = expected.is_none_or(|e| (v.position - e).abs() > 1.0);
        *last = Some((v.position, std::time::Instant::now(), v.playing));
        let mut cur = VIEW.get_or_init(Default::default).lock().unwrap();
        let changed = jumped || View { position: 0.0, ..cur.clone() } != View { position: 0.0, ..v.clone() };
        *cur = v;
        if changed {
            VERSION.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// The tracks after `i`, then the ones before it (playFromSource's wrap-around rest).
fn wrap_after(tracks: &[Value], i: usize) -> Vec<Value> {
    tracks[i + 1..].iter().chain(&tracks[..i]).cloned().collect()
}

fn fisher_yates(mut v: Vec<Value>) -> Vec<Value> {
    for i in (1..v.len()).rev() {
        let j = rand::random::<usize>() % (i + 1);
        v.swap(i, j);
    }
    v
}

async fn join_all<F: std::future::Future>(futs: impl IntoIterator<Item = F>) -> Vec<F::Output> {
    let mut out = vec![];
    let handles: Vec<_> = futs.into_iter().collect();
    // Sequential await of already-created futures; get_track is I/O bound and the fake/real
    // API answers quickly. Order is preserved, which is what matters (§2.5).
    for f in handles {
        out.push(f.await);
    }
    out
}

/// The player bar's quality badge, e.g. "16-BIT 44.1kHz FLAC LOSSLESS" (bit depth, rate with
/// one decimal when fractional, codec, then the TIDAL tier as the React badge shows it).
fn badge_text(info: &Option<Value>) -> Option<String> {
    let i = info.as_ref()?;
    let mut parts = vec![];
    if let Some(b) = i["bitDepth"].as_u64() {
        parts.push(format!("{b}-BIT"));
    }
    if let Some(r) = i["sampleRate"].as_u64() {
        let k = r as f64 / 1000.0;
        parts.push(if k.fract() == 0.0 { format!("{}kHz", k as u64) } else { format!("{k:.1}kHz") });
    }
    if let Some(c) = i["codec"].as_str() {
        parts.push(c.to_uppercase());
    }
    if let Some(t) = tier(i["audioQuality"].as_str()) {
        parts.push(t.to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn tier(q: Option<&str>) -> Option<&str> {
    match q? {
        "HI_RES_LOSSLESS" | "HI_RES" => Some("HI-RES LOSSLESS"),
        "LOSSLESS" => Some("LOSSLESS"),
        "HIGH" => Some("HIGH"),
        "" => None,
        other => Some(other),
    }
}

/// The authorization code in what the user pasted after the browser login: the `code` query
/// parameter of the redirect address, or the bare code itself (React handleSubmitEmbeddedUrl).
fn pkce_code(pasted: &str) -> Option<String> {
    let p = pasted.trim();
    if let Some((_, query)) = p.split_once('?') {
        let query = query.split('#').next().unwrap_or("");
        return query.split('&').find_map(|kv| kv.strip_prefix("code=")).filter(|c| !c.is_empty()).map(percent_decode);
    }
    (p.len() > 10 && !p.contains(' ') && !p.contains("://")).then(|| p.to_string())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()).ok_or(()) {
                Ok(v) => {
                    out.push(v);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod pkce_tests {
    use super::pkce_code;

    #[test]
    fn finds_the_code_in_a_pasted_redirect() {
        let u = "https://tidal.com/android/login/auth?code=abc_DEF-123%3D&state=na";
        assert_eq!(pkce_code(u).as_deref(), Some("abc_DEF-123="));
        assert_eq!(pkce_code("  https://tidal.com/android/login/auth?state=x&code=xyz123#frag \n").as_deref(), Some("xyz123"));
        assert_eq!(pkce_code("eyJhbGciOiJIUzI1NiJ9").as_deref(), Some("eyJhbGciOiJIUzI1NiJ9"));
        assert_eq!(pkce_code("https://tidal.com/android/login/auth?state=x"), None);
        assert_eq!(pkce_code("https://tidal.com/oops"), None);
        assert_eq!(pkce_code("short"), None);
    }
}

#[cfg(test)]
mod play_from_tests {
    use super::{tid, wrap_after};
    use serde_json::json;

    #[test]
    fn the_rest_follows_the_clicked_track_and_wraps() {
        let all: Vec<_> = (1..=5).map(|i| json!({"id": i})).collect();
        let ids = |v: Vec<serde_json::Value>| v.iter().map(tid).collect::<Vec<_>>();
        assert_eq!(ids(wrap_after(&all, 2)), [4, 5, 1, 2]);
        assert_eq!(ids(wrap_after(&all, 0)), [2, 3, 4, 5]);
        assert_eq!(ids(wrap_after(&all, 4)), [1, 2, 3, 4]);
    }
}
