//! `SONE_PERF=1` (read once): every 5 s one line on the log and stderr with what the window
//! did in that interval (PLAYER_DESIGN.md, "Motion spike"):
//!
//! `[perf] open playing focused fx=on | frames 4.2/s (drawn 21, unchanged 0, skipped 0)
//! views 1.0/s polls 6.2/s | cpu 41.3% rss 512 MB | regions 1.0/s`
//!
//! Frames are Rune's own `on_frame_encoded` reports for the main window: `Drawn` frames were
//! encoded and presented. Views count `view` runs (commits); polls count `poll` runs. CPU is the
//! process's utime + stime from /proc/self/stat (all threads, audio included) as a share of one
//! core. Focus comes from the shell's window-focus hook. Regions count the main window's pane
//! regions built again (`main::pane`; a reused one costs no build), not the debug builds'
//! re-checks of reused ones. Unset, no thread runs, neither the frame hook nor the focus handler
//! is installed, and the view, poll and region counters are one relaxed load each.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

const EVERY: Duration = Duration::from_secs(5);
/// /proc reports CPU time in USER_HZ ticks, 100 a second on every Linux ABI.
const USER_HZ: f64 = 100.0;

static VIEWS: AtomicU64 = AtomicU64::new(0);
static POLLS: AtomicU64 = AtomicU64::new(0);
static REGIONS: AtomicU64 = AtomicU64::new(0);
static DRAWN: AtomicU64 = AtomicU64::new(0);
static UNCHANGED: AtomicU64 = AtomicU64::new(0);
static SKIPPED: AtomicU64 = AtomicU64::new(0);
static FOCUSED: AtomicBool = AtomicBool::new(true);

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

pub fn region_built() {
    if on() {
        REGIONS.fetch_add(1, Ordering::Relaxed);
    }
}

/// Called from the shell's window-focus hook (`shell::on_focus`).
pub fn set_focused(focused: bool) {
    FOCUSED.store(focused, Ordering::Relaxed);
}

/// The main window's frame hook, installed only with SONE_PERF=1.
pub fn frame(f: &rune::FrameEncoded) {
    let counter = match f.outcome() {
        rune::FrameOutcome::Drawn => &DRAWN,
        rune::FrameOutcome::Skipped => &SKIPPED,
        _ => &UNCHANGED,
    };
    counter.fetch_add(1, Ordering::Relaxed);
}

/// Starts the logging thread, when SONE_PERF=1.
pub fn start() {
    if !on() {
        return;
    }
    std::thread::Builder::new().name("sone-perf".into()).spawn(|| {
        let counters = || [&DRAWN, &UNCHANGED, &SKIPPED, &VIEWS, &POLLS, &REGIONS].map(|c| c.load(Ordering::Relaxed));
        let mut last = (Instant::now(), cpu_ticks(), [0u64; 6]);
        loop {
            std::thread::sleep(EVERY);
            let now = (Instant::now(), cpu_ticks(), counters());
            let secs = now.0.duration_since(last.0).as_secs_f64();
            let d: Vec<u64> = (0..6).map(|i| now.2[i] - last.2[i]).collect();
            let cpu = (now.1 - last.1) as f64 / USER_HZ / secs * 100.0;
            let (_, playing) = crate::controller::clock();
            let line = format!(
                "[perf] {} {} {} fx={} | frames {:.1}/s (drawn {}, unchanged {}, skipped {}) views {:.1}/s polls {:.1}/s | cpu {:.1}% rss {} MB | regions {:.1}/s",
                if crate::full_ui::covers_bar() { "open" } else { "closed" },
                if playing { "playing" } else { "paused" },
                if FOCUSED.load(Ordering::Relaxed) { "focused" } else { "unfocused" },
                if crate::fx::on() { "on" } else { "off" },
                d[0] as f64 / secs, d[0], d[1], d[2], d[3] as f64 / secs, d[4] as f64 / secs, cpu, rss_mb(), d[5] as f64 / secs,
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
