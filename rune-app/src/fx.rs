//! The motion spike in the full-screen player (PLAYER_DESIGN.md, "Motion spike"): the cover's
//! open spring, the pause shrink, the directional track change, the heart pop, the bar's rise
//! and the audio-reactive cover and ground. Nothing here touches the normal view.
//!
//! `SONE_FX=0` (read once) turns all of it off and the player draws as it did before the spike.
//! `SONE_FX_UNFOCUSED=0` pauses the audio-reactive loop while the window isn't focused (by
//! default it keeps going: a full-screen player on a second monitor is usually unfocused).
//!
//! The tunable numbers are the consts below; full_ui reads them.

use rune::{Beat, Curve, Easing, Live, Spring, SpringPair, Transition};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::thread::Thread;
use std::time::{Duration, Instant};

// ---- tunables

/// The open flight's overshoot (the close stays critically damped, full_ui::CLOSE).
const OPEN_BOUNCE: f32 = 0.12;
/// The big cover while paused, and its shadow then: closer to the ground, softer.
pub const PAUSED_SCALE: f32 = 0.94;
pub const PAUSE: Spring = Spring::new(0.32, 0.0);
/// Play springs the cover back up, with a little bounce.
pub const RESUME: Spring = Spring::new(0.42, 0.15);
/// A track change pushes the cover this far (points) out one side and in from the other.
const PUSH: f32 = 32.0;
const PUSH_IN: Spring = Spring::new(0.3, 0.0);
const PUSH_OUT: Spring = Spring::new(0.22, 0.0);
/// The title and artist ride the same direction, a shorter way.
const NUDGE: f32 = 8.0;
/// How long a Next/Previous press decides the direction of the track change it causes.
const DIRECTION_TTL: Duration = Duration::from_secs(2);
/// The heart's pop on a like: 1 -> 1.25 -> 1 in 300 ms.
pub const HEART_POP: &[Beat] = &[
    Beat::new(0.0),
    Beat::new(0.11).scale(1.25).easing(Easing::EASE_OUT),
    Beat::new(0.3).easing(Easing::cubic(0.32, 0.72, 0.0, 1.0)),
];
/// The bar's auto-hiding groups rise this far as they fade in; their exit is opacity only.
pub const BAR_RISE: f32 = 7.0;
const RISE: Curve = Curve::tween(0.2, Easing::cubic(0.32, 0.72, 0.0, 1.0));
/// A space press that toggles playback lands its change without motion for this long.
const KEY_QUIET: Duration = Duration::from_millis(600);

/// The audio-reactive loop's rate. It sets Live values from its own thread, so each step is a
/// frame with no view or layout; frames stop when the values stop changing.
const REACT_HZ: f32 = 30.0;
/// At full loudness the cover grows this much, and the ground's scrim opens this fraction.
const REACT_SCALE: f32 = 0.02;
const REACT_SCRIM: f32 = 0.12;
/// The loudness envelope: fast attack, slower release (seconds, time constants). It runs every
/// step, so the tap's ~10 updates a second arrive as glides, not steps.
const ATTACK: f32 = 0.06;
const RELEASE: f32 = 0.25;
/// Loudness maps onto 0..1 over this many dB under a running reference that jumps up to loud
/// passages and falls back slowly, so a quiet master still moves and a loud one doesn't pin.
const RANGE_DB: f32 = 18.0;
const REF_FALL_DB_PER_S: f32 = 1.5;
const REF_FLOOR_DB: f32 = -42.0;
/// The tap measures each decoder buffer as it leaves for the sink (about 10 a second), so it
/// leads the sound by the sink's buffer: tens of ms through PipeWire, up to ~500 ms of ALSA ring
/// in exclusive mode. The loop reads it this late. True sync would need the device's delay
/// (`snd_pcm_delay`) with each level.
const SHARED_DELAY: f32 = 0.1;
const EXCLUSIVE_DELAY: f32 = 0.35;
/// Values are rounded to these steps, so near-silence publishes nothing and wakes no frame.
const SCALE_STEP: f32 = 1.0 / 4096.0;

// ---- switches

pub fn on() -> bool {
    static ON: LazyLock<bool> = LazyLock::new(|| std::env::var("SONE_FX").map_or(true, |v| v.trim() != "0"));
    *ON
}

fn unfocused_ok() -> bool {
    static OK: LazyLock<bool> = LazyLock::new(|| std::env::var("SONE_FX_UNFOCUSED").map_or(true, |v| v.trim() != "0"));
    *OK
}

/// The open flight: the nearest spring to the old 440 ms curve, with a small overshoot.
pub fn open_spring() -> Spring {
    Spring::new(0.44, if on() { OPEN_BOUNCE } else { 0.0 })
}

// ---- track direction and keyboard toggles

static DIRECTION: Mutex<Option<(f32, Instant)>> = Mutex::new(None);
static KEY_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// From controller::send: a Next (1) or Previous (-1) press, which the next track change follows.
pub fn commanded(dir: f32) {
    if on() {
        *DIRECTION.lock().unwrap() = Some((dir, Instant::now()));
    }
}

fn direction() -> Option<f32> {
    DIRECTION.lock().unwrap().filter(|(_, at)| at.elapsed() < DIRECTION_TTL).map(|(d, _)| d)
}

/// From on_key: space toggled playback, so the pause shrink it causes lands at once.
pub fn key_toggled() {
    if on() {
        *KEY_AT.lock().unwrap() = Some(Instant::now());
    }
}

fn keyed() -> bool {
    KEY_AT.lock().unwrap().is_some_and(|at| at.elapsed() < KEY_QUIET)
}

/// How the cover replaces the last track's: pushed out the side the queue moved away from and in
/// from the other, or (direction unknown) a cross-fade. The cover the player opened on arrives
/// with the open itself.
pub fn cover_change(first: bool) -> Transition {
    let enter = if first { Spring::NONE } else { PUSH_IN };
    match direction() {
        Some(d) => Transition::asymmetric(
            Transition::offset(PUSH * d, 0.0).combined(Transition::opacity()),
            Transition::offset(-PUSH * d, 0.0).combined(Transition::opacity()),
        ).springs(SpringPair::new(enter, PUSH_OUT)),
        None => Transition::opacity().springs(SpringPair::new(if first { Spring::NONE } else { Spring::new(0.32, 0.0) }, Spring::new(0.32, 0.0))),
    }
}

/// The title and artist: in with a small nudge the cover's way, out as a quick fade.
pub fn words_change(first: bool) -> Transition {
    let enter = Transition::offset(direction().map_or(0.0, |d| NUDGE * d), 0.0).combined(Transition::opacity());
    Transition::asymmetric(enter, Transition::opacity()).springs(SpringPair::new(if first { Spring::NONE } else { Spring::new(0.26, 0.0) }, Spring::new(0.14, 0.0)))
}

/// The bar's auto-hiding groups: risen into place while shown; dropped back only once faded.
pub fn bar_rise(e: rune::El, shown: bool) -> rune::El {
    use rune::AnimExt;
    if shown { e.offset(0.0, 0.0).offset_motion(RISE) } else { e.offset(0.0, BAR_RISE).offset_motion(Curve::tween(0.01, Easing::LINEAR).delay(0.26)) }
}

// ---- the audio-reactive loop

/// The cover's scale and the scrim's opacity, bound in full_ui and set by the loop's thread.
pub struct React {
    pub scale: Live<f32>,
    pub scrim: Live<f32>,
    thread: Thread,
}

static REACT: OnceLock<React> = OnceLock::new();
/// Whether the loop should run (player open, playing, focused or allowed unfocused, not
/// occluded, no reduced motion); the thread parks while it's false.
static RUN: AtomicBool = AtomicBool::new(false);
static OPEN: AtomicBool = AtomicBool::new(false);
static PLAYING: AtomicBool = AtomicBool::new(false);
static FOCUSED: AtomicBool = AtomicBool::new(true);
static OCCLUDED: AtomicBool = AtomicBool::new(false);
static REDUCED: AtomicBool = AtomicBool::new(false);
static EXCLUSIVE: AtomicBool = AtomicBool::new(false);
/// Playback's state as poll last saw it while the player was open: 0 unknown, 1 paused, 2 playing.
static SEEN_PLAYING: AtomicU8 = AtomicU8::new(0);

/// The loop's handles, its thread started the first time the player draws with the spike on.
pub fn react() -> &'static React {
    REACT.get_or_init(|| {
        let (scale, scrim) = (Live::new(1.0f32), Live::new(1.0f32));
        let (s, m) = (scale.clone(), scrim.clone());
        let thread = std::thread::Builder::new().name("sone-fx".into()).spawn(move || react_loop(&s, &m)).expect("spawn fx thread").thread().clone();
        React { scale, scrim, thread }
    })
}

/// From the window's poll: whether the player is open and playing. Returns true when a space
/// press just toggled playback under the open player, so poll lands that change without motion.
pub fn follow(open: bool, playing: bool) -> bool {
    if !on() {
        return false;
    }
    OPEN.store(open, Ordering::Relaxed);
    PLAYING.store(playing, Ordering::Relaxed);
    refresh();
    let now = if playing { 2 } else { 1 };
    let was = if open { SEEN_PLAYING.swap(now, Ordering::Relaxed) } else { SEEN_PLAYING.swap(0, Ordering::Relaxed) };
    open && was != 0 && was != now && keyed()
}

/// The root's window-focus handler.
pub fn set_focused(focused: bool) {
    FOCUSED.store(focused, Ordering::Relaxed);
    refresh();
}

/// From the frame hook: the window was covered or minimised (a frame skipped), or drew again.
pub fn set_occluded(occluded: bool) {
    if OCCLUDED.swap(occluded, Ordering::Relaxed) != occluded {
        refresh();
    }
}

/// From the view: the system's reduced-motion preference.
pub fn set_reduced(reduced: bool) {
    if REDUCED.swap(reduced, Ordering::Relaxed) != reduced {
        refresh();
    }
}

/// From the player's view: whether output is exclusive (a deeper device buffer to wait out).
pub fn set_exclusive(exclusive: bool) {
    EXCLUSIVE.store(exclusive, Ordering::Relaxed);
}

pub fn focused() -> bool {
    FOCUSED.load(Ordering::Relaxed)
}

fn refresh() {
    let run = on()
        && OPEN.load(Ordering::Relaxed)
        && PLAYING.load(Ordering::Relaxed)
        && !REDUCED.load(Ordering::Relaxed)
        && !OCCLUDED.load(Ordering::Relaxed)
        && (FOCUSED.load(Ordering::Relaxed) || unfocused_ok());
    if RUN.swap(run, Ordering::Relaxed) == run {
        return;
    }
    tauri_app_lib::audio_level::set_enabled(run);
    if run {
        react().thread.unpark();
    }
}

/// Loudness (linear RMS) to 0..1 against the running reference.
fn loudness(rms: f32, reference: &mut f32, dt: f32) -> f32 {
    let db = 20.0 * rms.max(1e-5).log10();
    *reference = if db > *reference { db } else { (*reference - REF_FALL_DB_PER_S * dt).max(REF_FLOOR_DB) };
    let x = ((db - (*reference - RANGE_DB)) / RANGE_DB).clamp(0.0, 1.0);
    x * x
}

/// Steps the envelope at REACT_HZ while RUN holds, then lets it fall back to rest and parks.
fn react_loop(scale: &Live<f32>, scrim: &Live<f32>) {
    let dt = 1.0 / REACT_HZ;
    let (mut env, mut reference) = (0.0f32, -18.0f32);
    // The last levels read, one a step, newest at the back: the delay line.
    let mut recent = std::collections::VecDeque::with_capacity((EXCLUSIVE_DELAY * REACT_HZ) as usize + 2);
    loop {
        let run = RUN.load(Ordering::Relaxed);
        if !run && env < 0.005 {
            env = 0.0;
            recent.clear();
            scale.set(1.0);
            scrim.set(1.0);
            std::thread::park();
            continue;
        }
        let delay = if EXCLUSIVE.load(Ordering::Relaxed) { EXCLUSIVE_DELAY } else { SHARED_DELAY };
        recent.push_back(if run { tauri_app_lib::audio_level::level().0 } else { 0.0 });
        while recent.len() > (delay * REACT_HZ).round() as usize + 1 {
            recent.pop_front();
        }
        let x = if run { loudness(recent[0], &mut reference, dt) } else { 0.0 };
        let tau = if x > env { ATTACK } else { RELEASE };
        env += (x - env) * (1.0 - (-dt / tau).exp());
        let q = |v: f32| (v / SCALE_STEP).round() * SCALE_STEP;
        scale.set(q(1.0 + REACT_SCALE * env));
        scrim.set(q(1.0 - REACT_SCRIM * env));
        std::thread::sleep(Duration::from_secs_f32(dt));
    }
}

/// For the perf line: whether the loop is running.
pub fn running() -> bool {
    RUN.load(Ordering::Relaxed)
}
