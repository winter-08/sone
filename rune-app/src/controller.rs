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
//! The content filter lives in content.rs, MCP's paged sources (`play_source` /
//! `shuffle_source`) in sources.rs; autoplay radio is `autoplay_radio`.

use serde_json::{Value, json};
use sone_host_shim::{AppHandle, Manager};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
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
    /// Seek to a time in seconds (a lyric line's), whatever length the item says it has.
    SeekTo(f64),
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
    /// An artist page's track row (ArtistPage handlePlayTrack → usePlaySource startAt): play
    /// `tracks[index]` from the artist source, keeping the manual queue. For the top tracks
    /// (`top`, which page on in the original) the tracks before the click are not wrapped in
    /// unless shuffle is on.
    PlayArtistTrack { id: u64, name: String, tracks: Vec<Value>, index: usize, top: bool },
    /// playSingle (§2.3): play this one item with an empty context queue and no source; the
    /// manual queue is kept (useMediaPlay for a video, a deep link).
    PlaySingle(Value),
    /// playSingle for a track id (the search dropdown's play button).
    PlaySingleId(u64),
    /// The menus' Play next / Add to queue (addToQueue / playNextInQueue): these tracks (or
    /// videos) as the menu has them, each tagged with the collection it came from (`source`, the
    /// "Playing from" once it plays), at the front of the manual queue in order (`next`) or at
    /// its end. Blocked ones are left out, as the content filter says.
    QueueTracks { tracks: Vec<Value>, source: Option<Value>, next: bool },
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
    /// Sign-in with the user's own client credentials (React Login's "Use custom credentials"):
    /// a device code, PKCE in the browser (then CompletePkceLogin), or a session imported from
    /// pasted tokens.
    StartDeviceLoginWith { id: String, secret: String },
    StartPkceLoginWith { id: String, secret: String },
    ImportSession { id: String, secret: String, refresh: String, access: Option<String> },
    /// The video player's quality menu: re-resolve the stream at HIGH / MEDIUM / LOW and carry on
    /// from the same moment (VideoPlayer handleQualityChange).
    SetVideoQuality(String),
    /// The video overlay's close on a finished video (VideoPlayer onEnded → closeVideo).
    CloseVideo,
    /// A favourite changed (interact.rs): the sync after it re-pushes the MPRIS rating
    /// (AppInitializer re-pushes metadata on favoriteTrackIds).
    FavoritesChanged,
    /// Settings' Autoplay switch (React autoplayAtom): radio continues when the queue runs out.
    SetAutoplay(bool),
    /// Settings' content switches (React allowExplicitAtom / allowAiAtom). Turning one off with
    /// something playing or queued also clears playback (resetPlaybackForContentChange); the
    /// React screen asks first, so send it after the user confirmed.
    SetAllowExplicit(bool),
    SetAllowAi(bool),
    /// The queue panel, by queue item (`QItem::qid`): play it (playFromQueue), remove it, or
    /// drag it to index `to` of the upcoming list (setQueueTracks with `reorder`).
    PlayQueued(String),
    RemoveQueued(String),
    MoveQueued { qid: String, to: usize },
    /// A History row in the queue panel: play that track again (playTrack, retryOnRateLimit).
    PlayHistory(String),
    /// A suggested track's cover in the drawer: play just it (playTrack, the queue kept).
    PlayTrack(Value),
    /// A suggested track's row: append it to the manual queue tagged with its album as the
    /// source it plays from (addToQueue(track, albumSource)).
    QueueTrack(Value),
    /// Settings → Playback's gapless toggle: saved by the backend, then the prerolled next
    /// track is refreshed for it.
    SetGapless(bool),
    /// Proxy settings were handed to the backend, which drops the prerolled gapless track
    /// whatever it made of them: forget it and arm it again (React PROXY_SAVED_EVENT).
    ProxySaved,
    /// Log out (React useAuth.logout): the backend's logout, then every trace of the account
    /// and its playback is dropped and the sign-in screen shows.
    Logout,
    /// The saved sign-in was refused at launch and a token refresh failed too (AppInitializer):
    /// the login screen shows; the saved tokens and the queue stay, as in React.
    SessionExpired,
    /// A collection page's Play, Shuffle or track row (usePlaySource): the loader holds what the
    /// page has loaded and pages in the rest (sources.rs, paged.rs); it plays as the MCP sources
    /// do, from its `start_at` track if it has one, shuffled if `play_shuffle`.
    PlaySource(Box<crate::sources::Loader>),
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
    /// The queue item's identity (`_qid`); duplicates of one track differ only by it.
    pub qid: String,
    pub explicit: bool,
    pub ai: bool,
    /// Not streamable now (isTrackUnavailable); its row greys and refuses a press.
    pub unavailable: bool,
    pub album: String,
    pub album_id: Option<u64>,
    /// The first artist with an id, for the row's artist link.
    pub artist_id: Option<u64>,
    /// The album cover (a CDN uuid or URL), or a video's picture.
    pub cover: Option<String>,
    /// The track itself, trimmed to what menus and the views read.
    pub track: std::sync::Arc<Value>,
}

/// The "Playing from" collection, enough to open its page (PlayingFromLabel navigateToSource).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Source {
    pub kind: String,
    /// Numbers (albums, artists) and strings (playlists, mixes) as text.
    pub id: String,
    pub name: String,
    pub subtitle: Option<String>,
    pub image: Option<String>,
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
    /// The saved sign-in has been checked at launch, so `signed_in` is known.
    pub auth_known: bool,
    pub login: Login,
    /// Counts the sign-in commands the controller has taken, so the sign-in screen can tell
    /// "still starting" from "answered", even when the answer leaves `login` as it was.
    pub login_answers: u64,
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
    /// The playing-from collection (`playing_from` is its name), and the one the context queue
    /// came from while a manual item plays from its own (the queue panel's "Next up from").
    pub source: Option<Source>,
    pub context_source: Option<Source>,
    /// How many of `queue` are the manual "Next in queue" items (they come first).
    pub manual_len: usize,
    /// The preferences the views show or apply: Autoplay and the content filter.
    pub autoplay: bool,
    pub allow_explicit: bool,
    pub allow_ai: bool,
    /// The current stream as play_tidal_track reported it (bitDepth, sampleRate, codec,
    /// audioQuality), for the signal path panel; null between tracks.
    pub stream_info: Option<Value>,
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

/// Whether the saved sign-in has been checked at launch (so `signed_in` is known).
pub fn auth_known() -> bool {
    VIEW.get().map(|v| v.lock().unwrap().auth_known).unwrap_or(false)
}

pub fn now_playing() -> (Option<u64>, bool) {
    VIEW.get().map(|v| { let v = v.lock().unwrap(); (v.track_id, v.playing) }).unwrap_or((None, false))
}

/// What the OBS overlay shows (obs.rs): the current item (trimmed), its cover, whether it
/// plays, the clock and the stream's quality text, without cloning the whole view.
pub fn overlay_now() -> Option<(std::sync::Arc<Value>, Option<String>, bool, f64, String)> {
    let v = VIEW.get()?.lock().unwrap();
    let cur = v.current.as_ref()?;
    let pos = POSITION.get().map(|p| p.lock().unwrap().interpolated()).unwrap_or(0.0).min(v.duration.max(0.0));
    Some((cur.track.clone(), v.cover.clone(), v.playing, pos, quality_text(&v.stream_info)))
}

/// The streaming quality ceiling, read from the backend (it owns the setting).
pub fn max_quality() -> String {
    utility::get_max_quality(crate::host().state::<AppState>())
}

/// The main window, in [`set_clock_shown`].
pub const MAIN: u8 = 1;
/// The miniplayer window, in [`set_clock_shown`].
pub const MINI: u8 = 2;
/// The windows on screen that draw the playback clock ([`MAIN`], [`MINI`]).
static CLOCK_SHOWN: AtomicU8 = AtomicU8::new(MAIN);

/// Says whether `window` is on screen drawing the clock. The position poll (every 2 s, or
/// 0.5 s for a video) only keeps the drawn clock honest, so it stops while no window shows
/// it and runs again, at once, when one comes back. Playback, MPRIS and queue advance don't
/// need it.
pub fn set_clock_shown(window: u8, shown: bool) {
    let before = if shown { CLOCK_SHOWN.fetch_or(window, Ordering::Relaxed) } else { CLOCK_SHOWN.fetch_and(!window, Ordering::Relaxed) };
    if before == 0 && shown && let Some(tx) = TX.get() {
        let _ = tx.send(Msg::ClockShown);
    }
}

fn clock_shown() -> bool {
    CLOCK_SHOWN.load(Ordering::Relaxed) != 0
}

/// The seek asked for last, not yet taken by the controller. Seeks are latest-wins: a flushing
/// seek waits for the stream to refill after the one before (about 170 ms), and a dragged seek
/// bar asks for one every frame, so handling each in turn left seconds of seeks to work through.
static PENDING_SEEK: Mutex<Option<f32>> = Mutex::new(None);

pub fn send(cmd: Cmd) {
    match cmd {
        Cmd::Next => crate::fx::commanded(1.0),
        Cmd::Previous => crate::fx::commanded(-1.0),
        Cmd::SeekFraction(f) => {
            preview_seek(f);
            // One message stands for every seek until the controller takes the slot.
            if PENDING_SEEK.lock().unwrap().replace(f).is_some() {
                return;
            }
        }
        Cmd::SeekTo(secs) => preview_seek_to(secs.max(0.0)),
        _ => {}
    }
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
    /// A window that draws the clock is on screen again after none was (`set_clock_shown`).
    ClockShown,
    SeekCorrection(u64),
    RateLimitResume { pgen: u64, retry: Option<Value>, context_first: bool },
    SaveDue(u64),
    LoginPoll(u64, Duration),
    /// Tracks fetched off the actor for an id request (§2.5): the successful ones in request
    /// order, how many were asked, and whether the UI asked (it hears about failures; MCP doesn't).
    Fetched { tracks: Vec<Value>, action: String, asked: usize, seq: u64, from_ui: bool },
    /// MCP play_source / shuffle_source (§2.5): the source was found; start loading it.
    SourceBuilt { loader: Box<crate::sources::Loader>, shuffle: bool },
    /// Its start batch loaded (usePlaySource): play it. `restore` is the shuffle mode to put back
    /// if it doesn't start (shuffle_source turned shuffle on for it).
    SourceReady { loader: Box<crate::sources::Loader>, request: u64, epoch: u64, restore: Option<bool> },
    /// A later page of the source playing, for the queue of `epoch` (appendToQueue).
    SourcePage { tracks: Vec<Value>, epoch: u64, owner: (Value, Value), shuffle: bool },
    /// A playing video's own metadata (`fill_video_meta`), for the queue item `qid` of video `id`.
    VideoMeta { id: u64, qid: String, meta: Value },
}

/// The queue epoch (§9.8): bumped whenever the context queue is replaced wholesale, which stops a
/// source still paging into the old one. Shared with the paging tasks.
static EPOCH: AtomicU64 = AtomicU64::new(0);
fn bump_epoch() {
    EPOCH.fetch_add(1, Ordering::Relaxed);
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
    /// When the last seek was asked for, until the backend reports a position near the clock.
    /// Just after a flushing seek it can report 0 (or where it was): those reads are dropped.
    seek_at: Option<Instant>,
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

/// The clock jumps to a seek's target as it's asked for, so the seek bar and time follow the
/// pointer while the backend catches up. A new generation drops position reads already in
/// flight, which would pull the clock back to where playback was.
fn preview_seek(f: f32) {
    let dur = VIEW.get().map(|v| v.lock().unwrap().duration).unwrap_or(0.0);
    if dur > 0.0 {
        preview_seek_to(f as f64 * dur);
    }
}

/// The clock jumps to `secs` at once, before the controller seeks there.
fn preview_seek_to(secs: f64) {
    let Some(p) = POSITION.get() else { return };
    let mut p = p.lock().unwrap();
    p.last = secs;
    p.at = Some(Instant::now());
    p.generation += 1;
    p.seek_at = Some(Instant::now());
    drop(p);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// A seek bar being dragged: the clock shows the time under the pointer, nothing seeks yet.
pub fn scrub(f: f32) {
    preview_seek(f);
}

/// A drag that was cancelled: let the next position read put the clock back.
pub fn end_scrub() {
    if let Some(p) = POSITION.get() {
        p.lock().unwrap().seek_at = None;
    }
    if let Some(tx) = TX.get() {
        let _ = tx.send(Msg::SeekCorrection(u64::MAX));
    }
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
/// Fills what a video queue item lacks from the video's metadata (TidalVideo, camelCase):
/// its length, picture, title, artists and explicit flag. What the item has stays.
fn merge_video_meta(cur: &mut Value, meta: &Value) {
    let missing = |v: &Value| v.is_null() || v.as_str() == Some("") || v.as_u64() == Some(0) || v.as_array().is_some_and(|a| a.is_empty());
    for k in ["duration", "imageId", "title", "artist", "artists", "explicit"] {
        if missing(&cur[k]) && !missing(&meta[k]) {
            cur[k] = meta[k].clone();
        }
    }
}
/// Whether a video's length read now (`now`) differs enough from the one published to replace it.
fn len_moved(published: f64, now: f64) -> bool {
    (now - published).abs() >= 0.5
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
/// Whether the item is a favourite: a Loved track, or for a video a loved video (React pushes no
/// MPRIS metadata for a video at all; this port does, so it rates it by the video's heart).
fn loved(t: &Value) -> bool {
    crate::interact::is_fav(if is_video(t) { crate::interact::Fav::Video } else { crate::interact::Fav::Track }, &tid(t).to_string())
}

/// xesam:userRating as AppInitializer sends it: 1.0 for a favourite, else 0.0.
fn mpris_rating(loved: bool) -> f64 {
    if loved { 1.0 } else { 0.0 }
}

/// isTrackUnavailable (§1.4), the future streamStartDate included.
fn unavailable(t: &Value) -> bool {
    crate::content::unavailable(t)
}
fn qitem(t: &Value) -> QItem {
    let cover = if is_video(t) { t["imageId"].as_str() } else { t["album"]["cover"].as_str() };
    let artist_id = t["artist"]["id"].as_u64().or_else(|| t["artists"].as_array().and_then(|a| a.iter().find_map(|x| x["id"].as_u64())));
    // What menus and rows read; the runtime fields and the rest of the payload stay behind.
    let mut slim = serde_json::Map::new();
    for k in ["id", "title", "version", "duration", "artist", "artists", "album", "explicit", "ai", "mixes", "itemType", "imageId", "audioQuality", "streamReady", "allowStreaming", "streamStartDate", "trackNumber", "copyright", "isrc"] {
        if let Some(v) = t.get(k).filter(|v| !v.is_null()) {
            slim.insert(k.to_string(), v.clone());
        }
    }
    QItem {
        id: tid(t),
        title: display_title(t),
        artist: artist_join(t),
        duration: duration(t),
        qid: qid_of(t),
        explicit: t["explicit"].as_bool() == Some(true),
        ai: t["ai"].as_bool() == Some(true),
        unavailable: unavailable(t),
        album: album_title(t).unwrap_or_default(),
        album_id: t["album"]["id"].as_u64(),
        artist_id,
        cover: cover.filter(|c| !c.is_empty()).map(str::to_string),
        track: std::sync::Arc::new(Value::Object(slim)),
    }
}
fn source_of(s: &Value) -> Option<Source> {
    let kind = s["type"].as_str()?.to_string();
    let id = match &s["id"] {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    };
    let text = |k: &str| s[k].as_str().filter(|x| !x.is_empty()).map(str::to_string);
    Some(Source { kind, id, name: s["name"].as_str().unwrap_or("").to_string(), subtitle: text("subtitle"), image: text("image") })
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
    /// The content filter refused it (PlayResult "filtered"); its toast was shown.
    Filtered,
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

static LAST_ERROR: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// The last failed play's message and when it failed (the miniplayer shows it for a while).
pub fn last_error() -> Option<(String, Instant)> {
    LAST_ERROR.lock().unwrap().clone()
}

/// The toast for a failed play: the backend's own message, as React's extractPlaybackError
/// reads it from the serialized error (`{kind, message}`): a string message as it is, a
/// blocked proxy's reason, anything else (an API error's status and body) "Playback failed".
fn playback_message(e: &SoneError) -> String {
    if let SoneError::ProxyBlocked { reason } = e {
        return reason.clone();
    }
    serde_json::to_value(e).ok().and_then(|v| v["message"].as_str().map(str::to_string)).unwrap_or_else(|| "Playback failed".into())
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
    /// The content filter (sone.allowExplicit.v1, sone.allowAi.v1): both allowed by default.
    #[serde(default = "allowed")]
    allow_explicit: bool,
    #[serde(default = "allowed")]
    allow_ai: bool,
}
fn allowed() -> bool {
    true
}
impl Default for Prefs {
    fn default() -> Self {
        Prefs { volume: 1.0, shuffle: false, repeat: 0, autoplay: false, bit_perfect_prev: None, allow_explicit: true, allow_ai: true }
    }
}
fn prefs_path() -> Option<std::path::PathBuf> {
    tauri_app_lib::config_dir_for_env().map(|d| d.join("rune-ui.json"))
}

#[derive(Default, PartialEq, Clone)]
struct Published {
    meta: Option<(u64, String, String, bool, String)>, // track id, qid, quality text, favourite, what the item says (title, art, length)
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
    login_answers: u64,
    login_creds: Option<(String, String, String)>, // device code, client id, secret
    pkce: Option<(String, String)>, // code verifier, client unique key
    /// The user's own client id and secret, when the PKCE sign-in in progress uses them.
    pkce_custom: Option<(String, String)>,
    pubd: Published,
    video_session: u64,
    /// The playing video's length as last published (`cur_duration`).
    video_len: f64,
    video_quality: String,
    /// The upcoming list's qids and their rows, as last published.
    qitems: (Vec<String>, Vec<QItem>),
    /// usePlaySource's latestRequest: a newer source request supersedes one still loading.
    source_req: u64,
}

impl Player {
    fn new(h: AppHandle, tx: mpsc::UnboundedSender<Msg>) -> Self {
        let prefs: Prefs = prefs_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        crate::content::set_prefs(prefs.allow_explicit, prefs.allow_ai);
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
            login_answers: 0,
            login_creds: None,
            pkce: None,
            pkce_custom: None,
            pubd: Published::default(),
            video_session: 0,
            video_len: 0.0,
            video_quality: "HIGH".into(),
            qitems: Default::default(),
            source_req: 0,
        }
    }

    fn state(&self) -> sone_host_shim::State<'_, AppState> {
        self.h.state::<AppState>()
    }

    /// contentBlockReason under the current preferences (§1.3).
    fn blocked(&self, t: &Value) -> Option<crate::content::Block> {
        crate::content::reason_with(t, self.prefs.allow_explicit, self.prefs.allow_ai)
    }
    fn is_blocked(&self, t: &Value) -> bool {
        self.blocked(t).is_some()
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

    /// A play that failed (React's "playback-error" event): its toast, and the miniplayer's
    /// error pill.
    fn playback_error(&self, e: &SoneError) {
        let m = playback_message(e);
        *LAST_ERROR.lock().unwrap() = Some((m.clone(), Instant::now()));
        self.toast_error(m);
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
                // the legacy sign-in nudge, and a sign-in TIDAL refuses (notices.rs)
                crate::notices::after_saved_sign_in();
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
                Msg::Ui(c) => {
                    let login = matches!(c, Cmd::StartDeviceLogin | Cmd::CancelLogin | Cmd::StartPkceLogin | Cmd::CompletePkceLogin(_)
                        | Cmd::StartDeviceLoginWith { .. } | Cmd::StartPkceLoginWith { .. } | Cmd::ImportSession { .. } | Cmd::Logout);
                    self.on_ui(c).await;
                    // Counted once handled: the answer is published with the state it leaves.
                    if login {
                        self.login_answers += 1;
                    }
                }
                Msg::Event(n, p) => self.on_event(&n, &p).await,
                Msg::GaplessDue(seq) if seq == self.gapless_seq => self.gapless_refresh().await,
                Msg::GaplessDue(_) => {}
                Msg::GaplessRetry => self.gapless_refresh().await,
                Msg::PositionPoll(seq) if seq == self.poll_seq => self.poll_position(true).await,
                Msg::PositionPoll(_) => {}
                // A new chain, which retires one still pending.
                Msg::ClockShown if self.playing => {
                    self.poll_seq += 1;
                    self.poll_position(true).await;
                }
                Msg::ClockShown => {}
                // (u64::MAX: a cancelled scrub asks for one read whatever came before)
                Msg::SeekCorrection(seq) => {
                    if pos().correction == seq || seq == u64::MAX {
                        self.poll_position(false).await;
                    }
                }
                Msg::RateLimitResume { pgen, retry, context_first } => self.rate_limit_fire(pgen, retry, context_first).await,
                Msg::SaveDue(seq) if seq == self.save_seq => self.save_snapshot(),
                Msg::SaveDue(_) => {}
                Msg::LoginPoll(seq, every) => self.login_poll(seq, every).await,
                Msg::Fetched { tracks, action, asked, seq, from_ui } => self.on_fetched(tracks, &action, asked, seq, from_ui).await,
                Msg::SourceBuilt { loader, shuffle } => self.source_built(*loader, shuffle),
                Msg::SourceReady { loader, request, epoch, restore } => self.source_ready(*loader, request, epoch, restore).await,
                Msg::VideoMeta { id, qid, meta } => {
                    if let Some(cur) = self.current.as_mut().filter(|c| tid(c) == id && qid_of(c) == qid) {
                        merge_video_meta(cur, &meta);
                    }
                }
                Msg::SourcePage { tracks, epoch, owner, shuffle } => {
                    if EPOCH.load(Ordering::Relaxed) == epoch {
                        self.append_to_queue(tracks, shuffle, Some(owner));
                    }
                }
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

    /// What AppInitializer loads once signed in (AI:285-470), in the order the first screen
    /// needs it: Home and the playlist folders (the sidebar) first; the account's name and
    /// picture (UserProfile: the header's avatar, AI:446-450), the feed (for the bell's dot) and
    /// the favourite ids (interact.rs) behind them, as background work (sched.rs).
    fn bootstrap_library(&self) {
        use crate::data::{self, Key};
        crate::interact::load_favorites_once();
        for (k, bg) in [(Key::Home("static".into()), false), (Key::Folders, false), (Key::UserProfile, true), (Key::Profile, true), (Key::Feed, true)] {
            // (one that failed before this sign-in is asked again; one on its way or in is kept)
            if matches!(data::peek_load(&k), Some(data::Load::Failed(_))) {
                data::forget(&k);
            }
            if bg { data::want_bg(k) } else { data::want(k) };
        }
        // AppInitializer's first pages of favourite albums, artists and tracks, last: React kept
        // them in atoms; here nothing reads them (the library tabs and Loved Tracks ask for their
        // own, larger, pages), so they wait behind everything else.
        let h = self.h.clone();
        sone_host_shim::async_runtime::spawn(async move {
            use tauri_app_lib::commands::library;
            let Ok(uid) = data::uid().await else { return };
            let st = || h.state::<AppState>();
            let _ = tokio::join!(
                crate::sched::bg(None, library::get_favorite_albums(st(), h.clone(), uid, 0, 20, "DATE".into(), "DESC".into())),
                crate::sched::bg(None, library::get_favorite_artists(st(), h.clone(), uid, 0, 20, "DATE".into(), "DESC".into())),
                crate::sched::bg(None, library::get_favorite_tracks(st(), h.clone(), uid, 0, 50, "DATE".into(), "DESC".into()))
            );
        });
    }

    async fn start_device_login(&mut self) {
        let (id, secret) = match auth::get_default_credentials() {
            Ok(c) if !c.0.is_empty() => c,
            Ok(_) => {
                self.login = Login::Failed("No embedded credentials available.".into());
                return;
            }
            Err(e) => {
                self.login = Login::Failed(format!("No embedded credentials available. ({e})"));
                return;
            }
        };
        self.start_device_login_with(id, secret).await
    }

    async fn start_device_login_with(&mut self, id: String, secret: String) {
        self.login_seq += 1;
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
        self.pkce_custom = None;
        self.await_pkce(params);
    }

    /// PKCE with the user's own client (React handlePkceLogin): the secret is required.
    fn start_pkce_login_with(&mut self, id: String, secret: String) {
        self.login_seq += 1;
        let params = match auth::start_pkce_auth(id.clone()) {
            Ok(p) => serde_json::to_value(p).unwrap_or_default(),
            Err(e) => {
                self.login = Login::Failed(e.to_string());
                return;
            }
        };
        self.pkce_custom = Some((id, secret));
        self.await_pkce(params);
    }

    /// Open TIDAL's login page in the browser and wait for the redirect URL to be pasted back.
    fn await_pkce(&mut self, params: Value) {
        let field = |k: &str| params[k].as_str().unwrap_or_default().to_string();
        let url = field("authorizeUrl");
        self.pkce = Some((field("codeVerifier"), field("clientUniqueKey")));
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
        self.login = Login::PkceWaiting { url, error: None };
    }

    /// Token import (React handleTokenImport, after the paste was parsed).
    async fn import_session(&mut self, id: String, secret: String, refresh: String, access: Option<String>) {
        self.login_seq += 1;
        let h = self.h.clone();
        match auth::import_session(h.state::<AppState>(), h.clone(), id, secret, refresh, access).await {
            Ok(t) => {
                if t.user_id.is_none() {
                    let _ = auth::get_session_user_id(h.state::<AppState>()).await;
                }
                self.login = Login::Idle;
                self.signed_in = true;
                self.bootstrap_library();
            }
            Err(e) => self.login = Login::Failed(e.to_string()),
        }
    }

    async fn complete_pkce_login(&mut self, pasted: String) {
        let Login::PkceWaiting { url, .. } = self.login.clone() else { return };
        let Some((verifier, key)) = self.pkce.clone() else { return };
        let Some(code) = pkce_code(&pasted) else {
            self.login = Login::PkceWaiting { url, error: Some("Could not find authorization code in the URL.".into()) };
            return;
        };
        let done = match self.pkce_custom.clone() {
            Some((id, secret)) => auth::complete_pkce_auth(self.h.state::<AppState>(), self.h.clone(), code, verifier, key, id, secret).await,
            None => auth::complete_pkce_browser_login(self.h.clone(), code, verifier, key).await,
        };
        match done {
            Ok(_tokens) => {
                self.login = Login::Idle;
                self.pkce = None;
                self.pkce_custom = None;
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
            Cmd::SeekFraction(_) => {
                let Some(f) = PENDING_SEEK.lock().unwrap().take() else { return };
                if self.current.is_some() {
                    let s = f as f64 * self.cur_duration();
                    self.seek_to(s).await;
                }
            }
            Cmd::SeekTo(secs) => {
                if self.current.is_some() {
                    self.seek_to(secs.max(0.0)).await;
                }
            }
            Cmd::SetVolume(v) => self.set_volume(v),
            Cmd::ToggleShuffle => self.toggle_shuffle(),
            Cmd::CycleRepeat => self.prefs.repeat = (self.prefs.repeat + 1) % 3,
            Cmd::PlayCollection { tracks, source, album_mode, shuffle } => {
                // usePlaySource: playAllFromSource refused (nothing playable) → info toast.
                let why = self.refusal(&tracks);
                if !self.play_all_src(tracks, Some(source), album_mode, shuffle).await {
                    self.toast(why);
                }
            }
            Cmd::PlayFrom { tracks, index, source, album_mode } => self.play_from_src(tracks, index, source, album_mode).await,
            Cmd::PlayArtistTrack { id, name, tracks, index, top } => {
                let Some(track) = tracks.get(index).cloned() else { return };
                let rest = artist_rest(&tracks, index, top, self.prefs.shuffle);
                let source = json!({"type": "artist", "id": id, "name": name});
                // Artist sources keep the manual queue, like every track click.
                self.set_queue_src(rest, false, self.prefs.shuffle, Some(source), tracks, false);
                self.fail_count = 0;
                if self.play_track(track, true, false, false, true, true).await == Err(Fail::Unplayable) {
                    self.play_next(true, true).await;
                }
            }
            Cmd::PlaySingle(t) => self.play_single(t).await,
            Cmd::PlaySingleId(id) => self.fetch_tracks(vec![id], "play_single", true),
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
                let why = self.refusal(&tracks);
                if !self.play_all_src(tracks, Some(source), false, shuffle).await {
                    self.toast(why);
                }
            }
            Cmd::RemoveAt(i) => self.remove_from_queue(i),
            Cmd::ClearQueue => self.clear_queue(),
            // Both modes turn gapless off and on, and the backend drops an armed track when
            // exclusive turns on: re-arm at once (React's mode subscriptions, no debounce).
            Cmd::SetBitPerfect(on) => {
                self.set_bit_perfect(on).await;
                self.gapless_refresh().await;
            }
            Cmd::SetExclusive(on) => {
                if utility::set_exclusive_mode(self.state(), on).is_ok() {
                    self.exclusive = on;
                    if !on && self.bit_perfect {
                        self.set_bit_perfect(false).await;
                    }
                    self.gapless_refresh().await;
                }
            }
            Cmd::ProxySaved => {
                self.pending_next = None;
                self.gapless_refresh().await;
            }
            Cmd::StartDeviceLogin => self.start_device_login().await,
            Cmd::CancelLogin => {
                self.login_seq += 1;
                self.login = Login::Idle;
                self.login_creds = None;
                self.pkce = None;
                self.pkce_custom = None;
            }
            Cmd::StartPkceLogin => self.start_pkce_login(),
            Cmd::CompletePkceLogin(pasted) => self.complete_pkce_login(pasted).await,
            Cmd::StartDeviceLoginWith { id, secret } => self.start_device_login_with(id, secret).await,
            Cmd::StartPkceLoginWith { id, secret } => self.start_pkce_login_with(id, secret),
            Cmd::ImportSession { id, secret, refresh, access } => self.import_session(id, secret, refresh, access).await,
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
            Cmd::PlaySource(loader) => self.source_built(*loader, false),
            Cmd::CloseVideo => {
                crate::video::close();
                self.playing = false;
            }
            Cmd::FavoritesChanged => {}
            Cmd::SetMaxQuality(q) => {
                let _ = utility::set_max_quality(self.state(), q);
            }
            Cmd::SetGapless(on) => {
                if utility::set_gapless(self.state(), on).is_ok() {
                    self.gapless_setting = on;
                    self.gapless_refresh().await;
                }
            }
            Cmd::Logout => self.logout().await,
            Cmd::SessionExpired => self.signed_in = false,
            Cmd::QueueTracks { tracks, source, next } => {
                let stamped: Vec<Value> = tracks.into_iter().filter(|t| !self.is_blocked(t)).collect::<Vec<_>>().into_iter().map(|t| {
                    let mut s = self.stamp(normalize(t));
                    if let Some(src) = &source {
                        s["_source"] = src.clone();
                    }
                    s
                }).collect();
                if next {
                    self.manual.splice(0..0, stamped);
                } else {
                    self.manual.extend(stamped);
                }
            }
            Cmd::JumpTo(id) => {
                if let Some(i) = self.upcoming().iter().position(|t| tid(t) == id) {
                    self.play_from_queue(i).await;
                }
            }
            Cmd::SetAutoplay(on) => self.prefs.autoplay = on,
            Cmd::SetAllowExplicit(on) => self.set_content_prefs(Some(on), None).await,
            Cmd::SetAllowAi(on) => self.set_content_prefs(None, Some(on)).await,
            Cmd::PlayQueued(q) => {
                if let Some(i) = self.upcoming_index(&q) {
                    self.play_from_queue(i).await;
                }
            }
            Cmd::RemoveQueued(q) => {
                if let Some(i) = self.upcoming_index(&q) {
                    self.remove_from_queue(i);
                }
            }
            Cmd::MoveQueued { qid, to } => {
                if let Some(from) = self.upcoming_index(&qid) {
                    let (list, manual) = reorder(self.upcoming(), self.manual.len(), from, to);
                    let mut list = list.into_iter().map(normalize);
                    self.manual = list.by_ref().take(manual).collect();
                    self.queue = list.collect();
                }
            }
            Cmd::PlayHistory(q) => {
                if let Some(t) = self.history.iter().rev().find(|t| qid_of(t) == q).cloned() {
                    let _ = self.play_track(t, true, false, false, true, false).await;
                }
            }
            Cmd::PlayTrack(t) => {
                let _ = self.play_track(t, true, false, false, true, false).await;
            }
            Cmd::QueueTrack(t) => {
                if !self.is_blocked(&t) {
                    let mut s = self.stamp(normalize(t));
                    if s["album"]["id"].is_number() {
                        s["_source"] = json!({"type": "album", "id": s["album"]["id"], "name": s["album"]["title"], "image": s["album"]["cover"]});
                    }
                    self.manual.push(s);
                }
            }
        }
    }

    /// The index in the upcoming list (manual, then context) of the item with this `_qid`.
    fn upcoming_index(&self, qid: &str) -> Option<usize> {
        self.manual.iter().chain(self.queue.iter()).position(|t| qid_of(t) == qid)
    }

    /// The content switches (§1.3). Turning either off clears playback, as the React settings
    /// do after their confirmation (resetPlaybackForContentChange, §1.5).
    async fn set_content_prefs(&mut self, explicit: Option<bool>, ai: Option<bool>) {
        let off = explicit == Some(false) && self.prefs.allow_explicit || ai == Some(false) && self.prefs.allow_ai;
        self.prefs.allow_explicit = explicit.unwrap_or(self.prefs.allow_explicit);
        self.prefs.allow_ai = ai.unwrap_or(self.prefs.allow_ai);
        crate::content::set_prefs(self.prefs.allow_explicit, self.prefs.allow_ai);
        if !off {
            return;
        }
        bump_epoch();
        self.resume_seq_gen = None;
        crate::video::close();
        let _ = playback::stop_track(self.state()).await;
        self.pending_next = None;
        self.set_current(None);
        self.playing = false;
        self.stream_info = None;
        self.queue.clear();
        self.manual.clear();
        self.original = None;
        self.history.clear();
        self.playback_source = None;
        self.context_source = None;
        self.autoplay_ids.clear();
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
            "mcp:play-source" | "mcp:shuffle-source" => {
                let kind = p["sourceType"].as_str().unwrap_or("").to_string();
                let id = match &p["id"] {
                    Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                let (tx, h, shuffle) = (self.tx.clone(), self.h.clone(), name == "mcp:shuffle-source");
                sone_host_shim::async_runtime::spawn(async move {
                    match crate::sources::build(&h, &kind, &id).await {
                        Some(l) => {
                            let _ = tx.send(Msg::SourceBuilt { loader: Box::new(l), shuffle });
                        }
                        None => log::warn!("[rune] mcp play_source: no {kind} {id}"),
                    }
                });
            }
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
                // Relative to the clock when the backend can't say (just after a seek).
                let cur = match playback::get_playback_position(self.state()).await {
                    Ok(v) => v as f64,
                    Err(_) => pos().interpolated(),
                };
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
            // A track link plays here; the other links (album, artist, playlist, mix) open their
            // page, and before sign-in every link waits (shell.rs, React handleDeepLink).
            "mpris:open-uri" => {
                let url = p.as_str().unwrap_or("").to_string();
                match crate::shell::track_link(&url).filter(|_| self.signed_in) {
                    Some(id) => {
                        if let Ok(t) = metadata::get_track(self.state(), id).await {
                            self.play_single(t).await;
                        }
                    }
                    None => crate::shell::open_link(url),
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
            p.seek_at = Some(Instant::now());
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
            // Every video start is at High (startVideoSession's default), whichever path
            // started it: a play, Previous, a resume or a repeat.
            self.video_quality = "HIGH".into();
            self.start_video(tid(t), 0.0).await?;
            self.fill_video_meta(t);
            return Ok(Value::Null);
        }
        if crate::video::is_open() {
            crate::video::close();
        }
        self.invoke_play_track(tid(t)).await
    }

    /// The video's own metadata (startVideoSession's get_video_metadata), for what the queue
    /// item lacks: a video queued from a deep link, MCP or an old snapshot can come without
    /// its length, picture or artist. A failure costs nothing; the item stays as it was. Fetched
    /// off the actor, so the play is marked started without waiting for it; it lands as
    /// `Msg::VideoMeta`, and the publish after it carries it to the bar and MPRIS.
    fn fill_video_meta(&self, t: &Value) {
        let (id, qid, h, tx) = (tid(t), qid_of(t), self.h.clone(), self.tx.clone());
        sone_host_shim::async_runtime::spawn(async move {
            let Ok(meta) = playback::get_video_metadata(h.state::<AppState>(), id).await else { return };
            let Ok(meta) = serde_json::to_value(meta) else { return };
            let _ = tx.send(Msg::VideoMeta { id, qid, meta });
        });
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
        crate::video::open(&url, self.prefs.volume, at).map_err(SoneError::Audio)?;
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
        // Content filter first (§2.1 step 1): nothing else changes.
        if let Some(b) = self.blocked(&track) {
            self.toast(crate::content::message(b));
            return Err(Fail::Filtered);
        }
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
                    Fail::Network => if let Some(m) = crate::proxy_banner::network_error() { self.toast_error(m) },
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
                    Fail::Transient => self.playback_error(&e),
                    Fail::Filtered => {}
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
            p.seek_at = None;
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
        // (nor over a video that became current meanwhile: React's currentVideoAtom check)
        if self.resume_seq_gen != Some(pgen) || pgen != self.generation || self.user_paused || self.current_is_video() {
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
        bump_epoch();
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
        let tracks: Vec<Value> = tracks.into_iter().filter(|t| !self.is_blocked(t)).collect();
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
        let eligible: Vec<Value> = tracks.into_iter().filter(|t| !unavailable(t) && !self.is_blocked(t)).collect();
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

    /// What usePlaySource says when playAllFromSource refuses `tracks`: the first content-block
    /// reason among them, else "No playable tracks".
    fn refusal(&self, tracks: &[Value]) -> &'static str {
        tracks.iter().find_map(|t| self.blocked(t)).map_or("No playable tracks", crate::content::message)
    }

    async fn play_single(&mut self, t: Value) {
        if let Some(b) = self.blocked(&t) {
            self.toast(crate::content::message(b));
            return;
        }
        self.set_queue(vec![], false, false);
        let _ = self.play_track(t, true, false, false, true, false).await;
    }

    /// playFromSource (§2.3): play `tracks[index]`; the rest is what follows it, then what came
    /// before (wrap-around), shuffled when shuffle mode is on. The manual queue survives. A
    /// blocked track only says why; blocked ones elsewhere in the collection are left out.
    async fn play_from_src(&mut self, tracks: Vec<Value>, index: usize, source: Value, album_mode: bool) {
        let Some(track) = tracks.get(index).cloned() else { return };
        if let Some(b) = self.blocked(&track) {
            self.toast(crate::content::message(b));
            return;
        }
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
        if action == "play_now" || action == "play_single" {
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
        if (action == "play_now" || action == "play_single") && seq != self.fetch_seq {
            return;
        }
        // MCP stays silent on failures (§2.5); the UI says what it couldn't load (useMediaPlay).
        if from_ui {
            let missed = asked - tracks.len();
            if tracks.is_empty() {
                self.toast_error(if action == "play_now" || action == "play_single" { "Failed to play" } else { "Couldn't load tracks" });
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
            "play_single" => {
                if let Some(t) = tracks.into_iter().next() {
                    self.play_single(t).await;
                }
            }
            // addToQueue / playNextInQueue ignore blocked tracks (UPA:777-795).
            "queue" => {
                for t in tracks.into_iter().filter(|t| !self.is_blocked(t)).collect::<Vec<_>>() {
                    let s = self.stamp(normalize(t));
                    self.manual.push(s);
                }
            }
            "play_next" => {
                for t in tracks.into_iter().filter(|t| !self.is_blocked(t)).rev().collect::<Vec<_>>() {
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
        if let Some(b) = self.blocked(&t) {
            self.toast(crate::content::message(b));
            return;
        }
        self.fail_count = 0;
        self.remove_from_queue(i);
        let _ = self.play_track(t, true, false, false, true, false).await;
    }

    /// usePlaySource for an MCP source (§2.4-2.5): shuffle_source turns shuffle mode on first
    /// (directly, so the current queue isn't reshuffled); then the start batch loads off the
    /// actor, as of this request and queue epoch.
    fn source_built(&mut self, mut loader: crate::sources::Loader, shuffle: bool) {
        let restore = shuffle.then_some(self.prefs.shuffle);
        if shuffle {
            self.prefs.shuffle = true;
        }
        self.source_req += 1;
        let (request, epoch) = (self.source_req, EPOCH.load(Ordering::Relaxed));
        let (tx, h, ae, aa) = (self.tx.clone(), self.h.clone(), self.prefs.allow_explicit, self.prefs.allow_ai);
        sone_host_shim::async_runtime::spawn(async move {
            let playable = move |t: &Value| !unavailable(t) && crate::content::reason_with(t, ae, aa).is_none();
            loader.start(&h, &playable).await;
            let _ = tx.send(Msg::SourceReady { loader: Box::new(loader), request, epoch, restore });
        });
    }

    async fn source_ready(&mut self, mut loader: crate::sources::Loader, request: u64, epoch: u64, restore: Option<bool>) {
        let put_back = |p: &mut Self| {
            if let Some(s) = restore {
                p.prefs.shuffle = s;
            }
        };
        // A newer source, or any queue replacement, since it was asked for: drop it.
        if request != self.source_req || EPOCH.load(Ordering::Relaxed) != epoch {
            return put_back(self);
        }
        if loader.tracks.is_empty() {
            if loader.failed {
                self.toast_error("Couldn't load tracks");
            }
            return put_back(self);
        }
        let tracks = std::mem::take(&mut loader.tracks);
        // A page's Shuffle with shuffle mode off scatters the later pages too (shuffleAppend).
        let shuffle_append = loader.play_shuffle && !self.prefs.shuffle;
        match loader.start_at.and_then(|id| tracks.iter().position(|t| tid(t) == id)) {
            // A track row (usePlaySource startAt): from it; with more pages coming and shuffle
            // off, the tracks before it aren't wrapped in (noWrap).
            Some(i) => {
                let no_wrap = loader.has_more && i > 0 && !self.prefs.shuffle;
                let (list, index) = if no_wrap { (tracks[i..].to_vec(), 0) } else { (tracks, i) };
                let before = EPOCH.load(Ordering::Relaxed);
                self.play_from_src(list, index, loader.meta.clone(), loader.album_mode).await;
                if EPOCH.load(Ordering::Relaxed) == before {
                    return; // refused (a blocked track says why)
                }
            }
            None => {
                let why = self.refusal(&tracks);
                if !self.play_all_src(tracks, Some(loader.meta.clone()), loader.album_mode, loader.play_shuffle).await {
                    self.toast(why);
                    return put_back(self);
                }
            }
        }
        if !loader.has_more {
            return;
        }
        // The rest, page by page, for as long as the queue is the one this source started.
        let ours = epoch + 1;
        let owner = (loader.meta["type"].clone(), loader.meta["id"].clone());
        let (tx, h, ae, aa) = (self.tx.clone(), self.h.clone(), self.prefs.allow_explicit, self.prefs.allow_ai);
        sone_host_shim::async_runtime::spawn(async move {
            let playable = move |t: &Value| !unavailable(t) && crate::content::reason_with(t, ae, aa).is_none();
            while loader.has_more && EPOCH.load(Ordering::Relaxed) == ours {
                let fresh = match loader.next_page(&h, &playable).await {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("[rune] failed to load remaining {} tracks: {e}", owner.0);
                        return;
                    }
                };
                let tracks: Vec<Value> = fresh.into_iter().filter(|t| !unavailable(t)).collect();
                if !tracks.is_empty() && tx.send(Msg::SourcePage { tracks, epoch: ours, owner: owner.clone(), shuffle: shuffle_append }).is_err() {
                    return;
                }
            }
        });
    }

    /// appendToQueue (§2.3): a later page of the source playing. Into the owner's track list
    /// (for repeat-all and previous), and into the queue: at random places after its head when
    /// shuffled (and in order onto the unshuffled copy), else at the end.
    fn append_to_queue(&mut self, tracks: Vec<Value>, shuffle: bool, owner: Option<(Value, Value)>) {
        let fresh: Vec<Value> = tracks.into_iter().filter(|t| !self.is_blocked(t)).collect();
        if fresh.is_empty() {
            return;
        }
        let stamped: Vec<Value> = fresh.into_iter().map(|t| {
            let n = normalize(t);
            self.stamp(n)
        }).collect();
        let owns = |s: &Option<Value>| match (&owner, s) {
            (Some((k, i)), Some(s)) => s["type"] == *k && s["id"] == *i,
            (None, Some(_)) => true,
            _ => false,
        };
        let target = if owner.is_some() && owns(&self.context_source) { self.context_source.as_mut() } else if owns(&self.playback_source) { self.playback_source.as_mut() } else { None };
        if let Some(s) = target {
            if let Some(a) = s["tracks"].as_array_mut() {
                a.extend(stamped.iter().cloned());
            }
        }
        if self.prefs.shuffle || shuffle {
            if self.prefs.shuffle {
                if let Some(o) = &mut self.original {
                    o.extend(stamped.iter().cloned());
                }
            }
            for t in stamped {
                let at = if self.queue.is_empty() { 0 } else { 1 + rand::random::<usize>() % self.queue.len() };
                self.queue.insert(at, t);
            }
        } else {
            self.queue.extend(stamped);
        }
    }

    fn clear_queue(&mut self) {
        bump_epoch();
        self.resume_seq_gen = None;
        self.queue.clear();
        self.manual.clear();
        self.original = None;
        self.playback_source = None;
        self.context_source = None;
    }

    /// React useAuth.logout: playback intent is cleared first (so an in-flight track event is
    /// ignored), then the backend logs out (it stops playback, purges scrobblers, MCP and the
    /// session, and clears the caches), then everything the account left in the UI goes and the
    /// sign-in screen shows.
    async fn logout(&mut self) {
        self.generation += 1;
        self.playing = false;
        if self.current_is_video() {
            crate::video::close();
        }
        self.set_current(None);
        self.clear_queue();
        if let Err(e) = auth::logout(self.state()).await {
            log::warn!("[rune] logout: {e}");
        }
        crate::data::forget_user_id();
        self.history.clear();
        self.stream_info = None;
        self.user_paused = false;
        self.pending_next = None;
        self.gapless_failed = None;
        self.autoplay_ids.clear();
        self.fail_count = 0;
        self.login_seq += 1;
        self.login = Login::Idle;
        self.login_creds = None;
        self.pkce = None;
        self.pkce_custom = None;
        self.signed_in = false;
        // The saved queue goes too (React removes sone.playback-state.v1).
        self.save_snapshot();
        // (The window empties the catalogue and favourites when it sees the sign-out:
        // settings::flush_settings, on its own thread, between frames.)
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
                // A video still open just rewinds (React: currentTime = 0), keeping its pipeline
                // and session, so a minimised player stays minimised.
                if self.current_is_video() && crate::video::is_open() {
                    crate::video::seek(0.0);
                    crate::video::play();
                    self.playing = true;
                    self.notify_seek(0.0);
                    return;
                }
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
                                Fail::Network => if let Some(m) = crate::proxy_banner::network_error() { self.toast_error(m) },
                                _ => self.playback_error(&e),
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
            let skip_manual = context_first && self.queue.iter().any(|t| !unavailable(t) && !self.is_blocked(t));
            // Manual drain (step 7).
            if !skip_manual {
                while !self.manual.is_empty() {
                    let next = self.manual[0].clone();
                    if self.is_blocked(&next) {
                        self.manual.remove(0);
                        continue;
                    }
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
                if self.is_blocked(&next) {
                    continue;
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
                let raw: Vec<Value> = raw.into_iter().filter(|t| !unavailable(t) && !self.is_blocked(t)).collect();
                let all: Vec<Value> = raw.into_iter().map(|t| {
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
            if self.prefs.autoplay && self.autoplay_radio().await {
                return;
            }
            self.playing = false;
        })
    }

    /// Autoplay radio (§3.2 step 11, UPA:1284-1331): the queue ran out, so continue with the
    /// current track's radio (its TRACK_MIX), leaving out what was already played. The radio
    /// becomes the context queue; "Playing from" stays. Whether a track was started (or a retry
    /// scheduled); false falls through to stopping.
    ///
    /// React returns early without clearing isPlaying when the track has no radio, though the
    /// pipeline was already stopped (the bar keeps saying "playing"); here it stops cleanly.
    async fn autoplay_radio(&mut self) -> bool {
        let Some(cur) = self.current.clone() else { return false };
        let Some(mix) = cur["mixes"]["TRACK_MIX"].as_str().filter(|m| !m.is_empty()).map(str::to_string) else { return false };
        let h = self.h.clone();
        let items = match tauri_app_lib::commands::pages::get_mix_items(h.state::<AppState>(), mix).await {
            Ok(m) => serde_json::to_value(m).unwrap_or(Value::Null),
            Err(e) => {
                match classify(&e) {
                    Fail::Network => if let Some(m) = crate::proxy_banner::network_error() { self.toast_error(m) },
                    Fail::RateLimited(after) => {
                        self.schedule_rate_limit(after, None, false);
                        return true;
                    }
                    _ => log::warn!("[rune] autoplay radio: {e}"),
                }
                return false;
            }
        };
        let played: HashSet<u64> = self.history.iter().map(tid).chain([tid(&cur)]).collect();
        let fresh = radio_fresh(&items, &played, |t| self.is_blocked(t));
        let Some((next, rest)) = fresh.split_first() else { return false };
        self.autoplay_ids = rest.iter().map(tid).collect();
        bump_epoch();
        self.queue = rest.iter().map(|t| {
            let n = normalize(t.clone());
            self.stamp(n)
        }).collect();
        self.use_track_gain = true;
        match self.play_track(next.clone(), false, false, false, false, false).await {
            Err(Fail::Unplayable) => self.play_next(false, false).await,
            Err(Fail::RateLimited(_)) => {
                let n = self.stamp(normalize(next.clone()));
                self.queue.insert(0, n);
            }
            _ => {}
        }
        true
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
                            Fail::Network => if let Some(m) = crate::proxy_banner::network_error() { self.toast_error(m) },
                            _ => self.playback_error(&e),
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
                    while p >= 0 && (unavailable(&tracks[p as usize]) || self.is_blocked(&tracks[p as usize])) {
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
                                self.playback_error(&e);
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

    /// The current item's length in seconds: a video's from its stream once the pipeline knows
    /// it (the <video> element's duration), else the item's own.
    fn cur_duration(&mut self) -> f64 {
        let item = self.current.as_ref().map(|t| duration(t) as f64).unwrap_or(0.0);
        if self.current_is_video() {
            if let Some(d) = crate::video::duration().filter(|d| d.is_finite() && *d > 0.0) {
                // An HLS stream's length can move by fractions as segments arrive: only a change
                // of half a second or more counts, so a video poll doesn't republish the view
                // (and rebuild every pane reading it) for it.
                if !len_moved(self.video_len, d) {
                    return self.video_len;
                }
                self.video_len = d;
                return d;
            }
        }
        item
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
                    Fail::Network => if let Some(m) = crate::proxy_banner::network_error() { self.toast_error(m) },
                    _ => self.playback_error(&e),
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
        } else {
            // The clock may have jumped ahead to the target (preview_seek): read it back.
            pos().seek_at = None;
            self.poll_position(false).await;
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
            // a playing video's audio follows the level too, as it does for set_volume
            crate::video::set_volume(v);
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
                // The pipeline sat at unity (a video may have moved `prefs.volume`).
                let from = 1.0;
                self.ramp_volume(from, vol).await;
                let _ = utility::set_volume_normalization(self.state(), norm);
            }
        }
    }

    fn set_volume(&mut self, level: f32) {
        // §6.6: bit-perfect audio stays at unity, but a video's audio is lossy, so its volume
        // still works; the audio pipeline is never given a non-unity level while bit-perfect is
        // on (React usePlaybackActions setVolume).
        let video = self.current_is_video();
        if self.bit_perfect && !video {
            return;
        }
        self.prefs.volume = level.clamp(0.0, 1.0);
        if !self.bit_perfect {
            let _ = playback::set_volume(self.state(), self.prefs.volume);
        }
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
                !unavailable(h) && !is_video(h) && !self.is_blocked(h)
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
            if reschedule && self.playing && clock_shown() {
                self.later(VIDEO_POLL_EVERY, Msg::PositionPoll(self.poll_seq));
            }
            return;
        }
        if let Ok(v) = playback::get_playback_position(self.state()).await {
            let mut p = pos();
            if p.generation == pgen {
                let interp = p.interpolated();
                let settling = p.reset_at.is_some_and(|t| t.elapsed() < SETTLE_WINDOW);
                let seeking = p.seek_at.is_some_and(|t| t.elapsed() < SETTLE_WINDOW);
                let far = (v as f64 - interp).abs() > 2.0;
                if seeking && !far {
                    p.seek_at = None;
                }
                if !(settling && v as f64 > interp + 2.0) && !(seeking && far) {
                    p.last = v as f64;
                    p.at = Some(Instant::now());
                }
            }
        }
        if reschedule && self.playing && clock_shown() {
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
        // (blocked tracks are dropped under the current content preferences, AI:583-713)
        let (ae, aa) = (self.prefs.allow_explicit, self.prefs.allow_ai);
        let clean = |t: &Value| -> Option<Value> {
            t["id"].as_u64()?;
            if crate::content::reason_with(t, ae, aa).is_some() {
                return None;
            }
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
        let src_tracks = [&self.playback_source, &self.context_source].into_iter().flatten().filter_map(|s| s["tracks"].as_array()).flatten();
        let all = self.current.iter().chain(&self.queue).chain(&self.history).chain(&self.manual).chain(self.original.iter().flatten()).chain(src_tracks);
        let max = all.filter_map(|t| t["_qid"].as_str()?.strip_prefix('q')?.parse::<u64>().ok()).max().unwrap_or(0);
        self.qid_counter = self.qid_counter.max(max);
        let q = std::mem::take(&mut self.queue);
        self.queue = q.into_iter().map(|t| self.ensure_qid(t)).collect();
        let m = std::mem::take(&mut self.manual);
        self.manual = m.into_iter().map(|t| self.ensure_qid(t)).collect();
        // (and every other restored list, as React's restoreSnapshot does: an unshuffle or a
        // repeat-all refill from these must not bring back items without a qid)
        if let Some(o) = self.original.take() {
            self.original = Some(o.into_iter().map(|t| self.ensure_qid(t)).collect());
        }
        let h = std::mem::take(&mut self.history);
        self.history = h.into_iter().map(|t| self.ensure_qid(t)).collect();
        for which in [0, 1] {
            let src = if which == 0 { self.playback_source.take() } else { self.context_source.take() };
            let src = src.map(|mut s| {
                if let Some(a) = s.get_mut("tracks").and_then(Value::as_array_mut).map(std::mem::take) {
                    s["tracks"] = Value::Array(a.into_iter().map(|t| self.ensure_qid(t)).collect());
                }
                s
            });
            if which == 0 { self.playback_source = src } else { self.context_source = src }
        }
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
            // (a video's own metadata lands after it starts: its title, picture and length
            // republish it; the length is the one the bar shows, to the second)
            let secs = self.cur_duration().round();
            let said = format!("{}\u{1f}{}\u{1f}{}\u{1f}{secs}", display_title(&cur), artist_mpris(&cur), art_url(&cur));
            let key = (tid(&cur), qid_of(&cur), quality_text(&self.stream_info), loved(&cur), said);
            if self.pubd.meta.as_ref() != Some(&key) {
                let md = playback::MprisMetadata {
                    track_id: tid(&cur),
                    title: display_title(&cur),
                    artist: artist_mpris(&cur),
                    album: album_title(&cur).unwrap_or_default(),
                    art_url: art_url(&cur),
                    duration_secs: secs,
                    url: format!("tidal://{}/{}", if is_video(&cur) { "video" } else { "track" }, tid(&cur)),
                    quality_text: key.2.clone(),
                    album_artist: cur["artist"]["name"].as_str().map(str::to_string),
                    track_number: cur["trackNumber"].as_u64().map(|n| n as u32),
                    disc_number: cur["volumeNumber"].as_u64().map(|n| n as u32),
                    content_created: cur["album"]["releaseDate"].as_str().map(str::to_string),
                    user_rating: Some(mpris_rating(key.3)),
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
        let tip = self.current.as_ref().map(|t| format!("{} — {}", display_title(t), artist_join(t))).unwrap_or_else(|| "Sone".into());
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
        let duration = self.cur_duration();
        let v = View {
            track_id: self.current.as_ref().map(tid),
            title: self.current.as_ref().map(display_title).unwrap_or_default(),
            artist: self.current.as_ref().map(artist_join).unwrap_or_default(),
            album: self.current.as_ref().and_then(album_title).unwrap_or_default(),
            playing: self.playing,
            position: pos().interpolated(),
            duration,
            shuffle: self.prefs.shuffle,
            repeat: Repeat::from_u8(self.prefs.repeat),
            volume: if self.bit_perfect && !self.current_is_video() { 1.0 } else { self.prefs.volume },
            quality: Some(quality_text(&self.stream_info)).filter(|s| !s.is_empty()),
            current: self.current.as_ref().map(qitem),
            queue: {
                // Rows are rebuilt only when the upcoming items change (a qid names one item).
                let keys: Vec<String> = self.manual.iter().chain(&self.queue).map(qid_of).collect();
                if self.qitems.0 != keys {
                    self.qitems = (keys, self.manual.iter().chain(&self.queue).map(qitem).collect());
                }
                self.qitems.1.clone()
            },
            playing_from: self.playback_source.as_ref().and_then(|s| s["name"].as_str()).map(str::to_string),
            // The queue panel's History: the last ten, oldest first (QueueTab history.slice(-10)).
            history: self.history[self.history.len().saturating_sub(10)..].iter().map(qitem).collect(),
            bit_perfect: self.bit_perfect,
            exclusive: self.exclusive,
            signed_in: self.signed_in,
            auth_known: self.restored,
            login: self.login.clone(),
            login_answers: self.login_answers,
            track_mix: self.current.as_ref().and_then(|t| t["mixes"]["TRACK_MIX"].as_str()).map(str::to_string),
            artist_id: self.current.as_ref().and_then(|t| t["artist"]["id"].as_u64().or(t["artists"][0]["id"].as_u64())),
            // With the stream gone (track ended), the React badge shows the track's tier.
            badge: badge_text(&self.stream_info, self.current.as_ref().and_then(|t| t["audioQuality"].as_str())).or_else(|| self.current.as_ref().and_then(|t| tier(t["audioQuality"].as_str()).map(str::to_string))),
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
            source: self.playback_source.as_ref().and_then(source_of),
            context_source: self.context_source.as_ref().and_then(source_of),
            manual_len: self.manual.len(),
            autoplay: self.prefs.autoplay,
            allow_explicit: self.prefs.allow_explicit,
            allow_ai: self.prefs.allow_ai,
            stream_info: self.stream_info.clone().filter(|i| i.is_object()),
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

/// The queue panel's drag (setQueueTracks `reorder`): `list` (manual items first, `manual` of
/// them) with item `from` moved to index `to`, and the new manual count. An item dragged out of
/// the manual part joins the context queue, and one dragged into it becomes manual.
fn reorder<T>(mut list: Vec<T>, manual: usize, from: usize, to: usize) -> (Vec<T>, usize) {
    if from >= list.len() {
        return (list, manual);
    }
    let to = to.min(list.len() - 1);
    if from == to {
        return (list, manual);
    }
    let moved = list.remove(from);
    list.insert(to, moved);
    let manual = if from < manual && to >= manual {
        manual - 1
    } else if from >= manual && to < manual {
        manual + 1
    } else {
        manual
    };
    (list, manual)
}

/// The radio's tracks (`get_mix_items`' `tracks`) worth playing next: not played already
/// (`played`: history and the current track), not blocked, not unavailable; in mix order.
fn radio_fresh(items: &Value, played: &HashSet<u64>, blocked: impl Fn(&Value) -> bool) -> Vec<Value> {
    items["tracks"].as_array().map(|a| a.iter().filter(|t| tid(t) != 0 && !played.contains(&tid(t)) && !blocked(t) && !unavailable(t)).cloned().collect()).unwrap_or_default()
}

/// The tracks after `i`, then the ones before it (playFromSource's wrap-around rest).
fn wrap_after(tracks: &[Value], i: usize) -> Vec<Value> {
    tracks[i + 1..].iter().chain(&tracks[..i]).cloned().collect()
}

/// What follows an artist page's clicked track (usePlaySource with startAt): the top tracks page
/// on past what the page shows, so without shuffle the tracks before the click aren't wrapped in
/// (noWrap); any other list wraps as playFromSource does.
fn artist_rest(tracks: &[Value], i: usize, top: bool, shuffle: bool) -> Vec<Value> {
    if top && i > 0 && !shuffle { tracks[i + 1..].to_vec() } else { wrap_after(tracks, i) }
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
fn badge_text(info: &Option<Value>, track_tier: Option<&str>) -> Option<String> {
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
    // (the stream's tier, else the track's: `streamInfo?.audioQuality || track.audioQuality`)
    if let Some(t) = tier(i["audioQuality"].as_str().filter(|q| !q.is_empty()).or(track_tier)) {
        parts.push(t.to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn tier(q: Option<&str>) -> Option<&str> {
    match q? {
        "HI_RES_LOSSLESS" | "HI_RES" => Some("HI-RES LOSSLESS"),
        "LOSSLESS" => Some("LOSSLESS"),
        "" => None,
        // QualityBadge: anything else (HIGH, LOW, a tier it doesn't know) reads "HIGH"
        _ => Some("HIGH"),
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
mod queue_tests {
    use super::{radio_fresh, reorder};
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn a_drag_across_the_divider_changes_section() {
        // manual a b | context c d e
        let l = vec!['a', 'b', 'c', 'd', 'e'];
        assert_eq!(reorder(l.clone(), 2, 0, 2), (vec!['b', 'c', 'a', 'd', 'e'], 1));
        assert_eq!(reorder(l.clone(), 2, 4, 0), (vec!['e', 'a', 'b', 'c', 'd'], 3));
        assert_eq!(reorder(l.clone(), 2, 3, 2), (vec!['a', 'b', 'd', 'c', 'e'], 2));
        assert_eq!(reorder(l.clone(), 2, 1, 1), (l.clone(), 2));
        assert_eq!(reorder(l.clone(), 2, 0, 99), (vec!['b', 'c', 'd', 'e', 'a'], 1));
    }

    #[test]
    fn the_radio_leaves_out_what_was_played_and_what_is_blocked() {
        let mix = json!({"tracks": [{"id": 1}, {"id": 2, "explicit": true}, {"id": 3, "streamReady": false}, {"id": 4}, {"id": 5}]});
        let played: HashSet<u64> = [1, 5].into_iter().collect();
        let fresh = radio_fresh(&mix, &played, |t| t["explicit"].as_bool() == Some(true));
        assert_eq!(fresh.iter().map(|t| t["id"].as_u64().unwrap()).collect::<Vec<_>>(), [4]);
        assert!(radio_fresh(&json!({}), &played, |_| false).is_empty());
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

    #[test]
    fn an_artists_top_tracks_dont_wrap_unless_shuffled() {
        use super::artist_rest;
        let all: Vec<_> = (1..=5).map(|i| json!({"id": i})).collect();
        let ids = |v: Vec<serde_json::Value>| v.iter().map(tid).collect::<Vec<_>>();
        assert_eq!(ids(artist_rest(&all, 2, true, false)), [4, 5]);
        assert_eq!(ids(artist_rest(&all, 0, true, false)), [2, 3, 4, 5]);
        assert_eq!(ids(artist_rest(&all, 2, true, true)), [4, 5, 1, 2]);
        assert_eq!(ids(artist_rest(&all, 2, false, false)), [4, 5, 1, 2]);
    }
}

#[cfg(test)]
mod video_item_tests {
    use super::unavailable;
    use serde_json::json;

    #[test]
    fn a_video_in_a_playlist_is_never_stream_gated() {
        assert!(!unavailable(&json!({"id": 9, "itemType": "video", "streamReady": false, "allowStreaming": false})));
        assert!(unavailable(&json!({"id": 9, "itemType": "track", "streamReady": false})));
    }
}

#[cfg(test)]
mod mpris_tests {
    #[test]
    fn the_rating_is_the_favourite_flag() {
        assert_eq!(super::mpris_rating(true), 1.0);
        assert_eq!(super::mpris_rating(false), 0.0);
    }

    #[test]
    fn a_video_length_follows_only_real_changes() {
        assert!(super::len_moved(0.0, 241.3));
        assert!(!super::len_moved(241.3, 241.5));
        assert!(!super::len_moved(241.3, 240.9));
        assert!(super::len_moved(241.3, 243.0));
    }
}

#[cfg(test)]
mod playback_parity_tests {
    use super::*;

    #[test]
    fn video_metadata_fills_only_what_the_item_lacks() {
        let mut cur = json!({"id": 7, "itemType": "video", "title": "Mine", "duration": 0, "imageId": null, "_qid": "q3"});
        let meta = json!({"id": 7, "title": "Theirs", "duration": 241, "imageId": "a-b", "artist": {"id": 1, "name": "X"}, "artists": [{"id": 1, "name": "X"}]});
        merge_video_meta(&mut cur, &meta);
        assert_eq!(cur["title"], "Mine");
        assert_eq!(cur["duration"], 241);
        assert_eq!(cur["imageId"], "a-b");
        assert_eq!(cur["artist"]["name"], "X");
        assert_eq!(cur["_qid"], "q3");
    }

    #[test]
    fn a_failed_play_toasts_the_backend_message() {
        assert_eq!(playback_message(&SoneError::Audio("device busy".into())), "device busy");
        assert_eq!(playback_message(&SoneError::Network("timed out".into())), "timed out");
        assert_eq!(playback_message(&SoneError::Api { status: 500, body: "{}".into() }), "Playback failed");
        assert_eq!(playback_message(&SoneError::NotAuthenticated), "Playback failed");
        assert_eq!(playback_message(&SoneError::ProxyBlocked { reason: "Proxy is off".into() }), "Proxy is off");
    }

    #[test]
    fn the_tray_names_an_unknown_artist() {
        assert_eq!(artist_join(&json!({"title": "T"})), "Unknown Artist");
        assert_eq!(artist_join(&json!({"artists": [{"name": "A"}, {"name": "B"}]})), "A, B");
    }
}
