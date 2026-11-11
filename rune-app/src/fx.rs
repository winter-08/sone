//! The full-screen player's one-shot motion (PLAYER_DESIGN.md, "Motion spike"): the cover's open
//! spring, the pause shrink, the directional track change, the heart pop and the bar's rise. Each
//! plays for a few hundred ms after an event and draws nothing at rest. Nothing here touches the
//! normal view.
//!
//! `SONE_FX=0` (read once) turns all of it off and the player draws as it did before.
//!
//! The tunable numbers are the consts below; full_ui reads them.

use rune::{Beat, Curve, Easing, Spring, SpringPair, Transition};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{LazyLock, Mutex};
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

// ---- switches

pub fn on() -> bool {
    static ON: LazyLock<bool> = LazyLock::new(|| std::env::var("SONE_FX").map_or(true, |v| v.trim() != "0"));
    *ON
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

/// Playback's state as poll last saw it while the player was open: 0 unknown, 1 paused, 2 playing.
static SEEN_PLAYING: AtomicU8 = AtomicU8::new(0);

/// From the window's poll: whether the player is open and playing. Returns true when a space
/// press just toggled playback under the open player, so poll lands that change without motion.
pub fn follow(open: bool, playing: bool) -> bool {
    if !on() {
        return false;
    }
    let now = if playing { 2 } else { 1 };
    let was = SEEN_PLAYING.swap(if open { now } else { 0 }, Ordering::Relaxed);
    open && was != 0 && was != now && keyed()
}
