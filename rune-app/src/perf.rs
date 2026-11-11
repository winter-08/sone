//! `SONE_PERF=1` (read once): every 5 s one line on the log and stderr with what the window
//! did in that interval, for measuring the motion spike (PLAYER_DESIGN.md, "Motion spike"):
//!
//! `[perf] open playing focused fx=reacting tap=on (10.8 levels/s) | frames 30.2/s (drawn 151, unchanged 0, skipped 0)
//! views 1.0/s polls 6.2/s | cpu 41.3% rss 512 MB`
//!
//! Frames are Rune's own `on_frame_encoded` reports for the main window: `Drawn` frames were
//! encoded and presented. Views count `view` runs (commits); polls count `poll` runs. CPU is the
//! process's utime + stime from /proc/self/stat (all threads, audio included) as a share of one
//! core. Unset, no thread runs and the view and poll counters are one relaxed load each; the frame
//! hook stays installed only while the spike is on, which needs it to notice a covered window.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const EVERY: Duration = Duration::from_secs(5);
/// /proc reports CPU time in USER_HZ ticks, 100 a second on every Linux ABI.
const USER_HZ: f64 = 100.0;

static VIEWS: AtomicU64 = AtomicU64::new(0);
static POLLS: AtomicU64 = AtomicU64::new(0);
static DRAWN: AtomicU64 = AtomicU64::new(0);
static UNCHANGED: AtomicU64 = AtomicU64::new(0);
static SKIPPED: AtomicU64 = AtomicU64::new(0);

pub fn on() -> bool {
    static ON: LazyLock<bool> = LazyLock::new(|| std::env::var("SONE_PERF").is_ok_and(|v| v.trim() == "1"));
    *ON
}

pub fn view_ran() {
    if on() {
        VIEWS.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn poll_ran() {
    if on() {
        POLLS.fetch_add(1, Ordering::Relaxed);
    }
}

/// The main window's frame hook, installed when perf or the spike needs it: counts frames, and
/// tells the spike when the window stops drawing because it is covered or minimised.
pub fn frame(f: &rune::FrameEncoded) {
    let counter = match f.outcome() {
        rune::FrameOutcome::Drawn => {
            crate::fx::set_occluded(false);
            &DRAWN
        }
        rune::FrameOutcome::Skipped => {
            crate::fx::set_occluded(true);
            &SKIPPED
        }
        _ => &UNCHANGED,
    };
    if on() {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// Starts the logging thread, when SONE_PERF=1.
pub fn start() {
    if !on() {
        return;
    }
    std::thread::Builder::new().name("sone-perf".into()).spawn(|| {
        let mut last = (Instant::now(), cpu_ticks(), [0u64; 5], tauri_app_lib::audio_level::seq());
        loop {
            std::thread::sleep(EVERY);
            let now = (Instant::now(), cpu_ticks(), [&DRAWN, &UNCHANGED, &SKIPPED, &VIEWS, &POLLS].map(|c| c.load(Ordering::Relaxed)), tauri_app_lib::audio_level::seq());
            let secs = now.0.duration_since(last.0).as_secs_f64();
            let d: Vec<u64> = (0..5).map(|i| now.2[i] - last.2[i]).collect();
            let cpu = (now.1 - last.1) as f64 / USER_HZ / secs * 100.0;
            let (_, playing) = crate::controller::clock();
            let line = format!(
                "[perf] {} {} {} fx={} tap={} ({:.1} levels/s) | frames {:.1}/s (drawn {}, unchanged {}, skipped {}) views {:.1}/s polls {:.1}/s | cpu {:.1}% rss {} MB",
                if crate::full_ui::covers_bar() { "open" } else { "closed" },
                if playing { "playing" } else { "paused" },
                if crate::fx::focused() { "focused" } else { "unfocused" },
                if !crate::fx::on() { "off" } else if crate::fx::running() { "reacting" } else { "on" },
                if tauri_app_lib::audio_level::enabled() { "on" } else { "off" },
                (now.3 - last.3) as f64 / secs,
                d[0] as f64 / secs, d[0], d[1], d[2], d[3] as f64 / secs, d[4] as f64 / secs, cpu, rss_mb(),
            );
            log::info!("{line}");
            eprintln!("{line}");
            last = now;
        }
    }).expect("spawn perf thread");
}

fn cpu_ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // Fields after the parenthesised name: utime and stime are the 12th and 13th.
    let rest = stat.rsplit_once(')').map_or("", |(_, r)| r);
    rest.split_whitespace().skip(11).take(2).filter_map(|v| v.parse::<u64>().ok()).sum()
}

fn rss_mb() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: u64 = statm.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    pages * 4096 / (1024 * 1024)
}
