//! The order backend requests go out in.
//!
//! The backend holds one lock on its TIDAL client across every HTTP request, so requests run one
//! at a time, in the order they reach the lock (tokio's mutex is fair). Real TIDAL takes 0.15-2 s
//! a request, so whatever reaches the lock first decides what the user waits for. The order is
//! therefore chosen here, before a command is called:
//!
//! - **Foreground** (`fg`): what the page on screen needs, anything a click asked for, playback.
//!   It goes at once.
//! - **Background** (`bg`): prefetching other Home tabs, refreshing stale pages, the feed and its
//!   unread dot, favourite id lists, the profile and avatar, the update check. One at a time, in
//!   the order asked, and only while no foreground request is running or waiting and none has
//!   finished in the last [`GRACE`] (a page that lands often asks for its next piece at once).
//!   So at most one background request is ever ahead of a user action.
//!
//! A background request that the user turns out to need (they open the tab it prefetches) can
//! be [`Promote`]d: it stops waiting and goes as foreground.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// How long after the last foreground request ends before a background one may start.
pub const GRACE: Duration = Duration::from_millis(150);

#[derive(Default)]
struct Lanes {
    /// Foreground requests running (or waiting at the backend's lock).
    fg: usize,
    /// A background request is running.
    bg_busy: bool,
    /// Background requests waiting, oldest first.
    queue: VecDeque<u64>,
    next: u64,
    last_fg_end: Option<Instant>,
}

pub struct Sched {
    lanes: Mutex<Lanes>,
    notify: Notify,
    grace: Duration,
}

/// Set to let a waiting background request go as foreground (see [`Sched::bg`]).
#[derive(Clone, Default)]
pub struct Promote(Arc<AtomicBool>);

impl Promote {
    pub fn new() -> Promote {
        Promote::default()
    }
}

/// The app's scheduler.
pub static SCHED: LazyLock<Sched> = LazyLock::new(|| Sched::new(GRACE));

/// Holds the foreground lane busy while it lives (see [`Sched::fg_guard`]).
pub struct FgGuard<'a>(&'a Sched);

impl Drop for FgGuard<'_> {
    fn drop(&mut self) {
        let mut l = self.0.lanes.lock().unwrap();
        l.fg -= 1;
        if l.fg == 0 {
            l.last_fg_end = Some(Instant::now());
        }
        drop(l);
        self.0.notify.notify_waiters();
    }
}

/// The background lane's turn, or a place in its queue; released when dropped (also when the
/// request is cancelled while it waits).
struct BgTurn<'a> {
    s: &'a Sched,
    id: u64,
    running: bool,
}

impl Drop for BgTurn<'_> {
    fn drop(&mut self) {
        let mut l = self.s.lanes.lock().unwrap();
        if self.running {
            l.bg_busy = false;
        } else {
            l.queue.retain(|x| *x != self.id);
        }
        drop(l);
        self.s.notify.notify_waiters();
    }
}

impl Sched {
    pub fn new(grace: Duration) -> Sched {
        Sched { lanes: Mutex::new(Lanes::default()), notify: Notify::new(), grace }
    }

    /// Marks foreground work in progress until the guard drops: a user command the controller
    /// is handling, whose backend calls are awaited inside it.
    pub fn fg_guard(&self) -> FgGuard<'_> {
        self.lanes.lock().unwrap().fg += 1;
        FgGuard(self)
    }

    /// Runs `f` as foreground work.
    pub async fn fg<F: Future>(&self, f: F) -> F::Output {
        let _g = self.fg_guard();
        f.await
    }

    /// Runs `f` as background work: after the background requests asked before it, and only
    /// while the foreground lane is idle. If `promote` is set while it waits, it goes at once
    /// as foreground.
    pub async fn bg<F: Future>(&self, promote: Option<&Promote>, f: F) -> F::Output {
        let mut turn = {
            let mut l = self.lanes.lock().unwrap();
            l.next += 1;
            let id = l.next;
            l.queue.push_back(id);
            BgTurn { s: self, id, running: false }
        };
        loop {
            if promote.is_some_and(|p| p.0.load(Ordering::Relaxed)) {
                drop(turn);
                return self.fg(f).await;
            }
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let wait = {
                let mut l = self.lanes.lock().unwrap();
                if l.fg == 0 && !l.bg_busy && l.queue.front() == Some(&turn.id) {
                    match l.last_fg_end.map(|t| t.elapsed()).filter(|e| *e < self.grace) {
                        Some(e) => Some(self.grace - e),
                        None => {
                            l.queue.pop_front();
                            l.bg_busy = true;
                            turn.running = true;
                            None
                        }
                    }
                } else {
                    Some(Duration::from_secs(1))
                }
            };
            let Some(d) = wait else { break };
            tokio::select! {
                _ = notified => {}
                _ = tokio::time::sleep(d) => {}
            }
        }
        let out = f.await;
        drop(turn);
        out
    }

    /// Lets a waiting background request go as foreground.
    pub fn promote(&self, p: &Promote) {
        if !p.0.swap(true, Ordering::Relaxed) {
            self.notify.notify_waiters();
        }
    }

    #[cfg(test)]
    fn idle(&self) -> bool {
        let l = self.lanes.lock().unwrap();
        l.fg == 0 && !l.bg_busy && l.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap()
    }

    /// A pretend request: logs its start and end and holds for `ms`.
    async fn req(log: &Mutex<Vec<String>>, name: &str, ms: u64) {
        log.lock().unwrap().push(format!("{name}+"));
        tokio::time::sleep(Duration::from_millis(ms)).await;
        log.lock().unwrap().push(format!("{name}-"));
    }

    #[test]
    fn background_waits_for_foreground() {
        rt().block_on(async {
            let s = Arc::new(Sched::new(Duration::from_millis(30)));
            let log = Arc::new(Mutex::new(vec![]));
            let (s1, l1) = (s.clone(), log.clone());
            let fg = tokio::spawn(async move { s1.fg(req(&l1, "fg", 100)).await });
            tokio::time::sleep(Duration::from_millis(10)).await;
            let (s2, l2) = (s.clone(), log.clone());
            let bg = tokio::spawn(async move { s2.bg(None, req(&l2, "bg", 10)).await });
            fg.await.unwrap();
            bg.await.unwrap();
            assert_eq!(*log.lock().unwrap(), ["fg+", "fg-", "bg+", "bg-"]);
            assert!(s.idle());
        });
    }

    #[test]
    fn background_waits_out_the_grace_after_foreground() {
        rt().block_on(async {
            let s = Arc::new(Sched::new(Duration::from_millis(80)));
            s.fg(async {}).await;
            let t = Instant::now();
            s.bg(None, async {}).await;
            assert!(t.elapsed() >= Duration::from_millis(70), "{:?}", t.elapsed());
        });
    }

    #[test]
    fn one_background_request_at_a_time_in_order() {
        rt().block_on(async {
            let s = Arc::new(Sched::new(Duration::ZERO));
            let log = Arc::new(Mutex::new(vec![]));
            let running = Arc::new(AtomicUsize::new(0));
            let max = Arc::new(AtomicUsize::new(0));
            let mut hs = vec![];
            for i in 0..5 {
                let (s, log, running, max) = (s.clone(), log.clone(), running.clone(), max.clone());
                hs.push(tokio::spawn(async move {
                    s.bg(None, async {
                        let n = running.fetch_add(1, Ordering::SeqCst) + 1;
                        max.fetch_max(n, Ordering::SeqCst);
                        req(&log, &format!("b{i}"), 20).await;
                        running.fetch_sub(1, Ordering::SeqCst);
                    }).await
                }));
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            for h in hs {
                h.await.unwrap();
            }
            assert_eq!(max.load(Ordering::SeqCst), 1);
            let starts: Vec<String> = log.lock().unwrap().iter().filter(|e| e.ends_with('+')).cloned().collect();
            assert_eq!(starts, ["b0+", "b1+", "b2+", "b3+", "b4+"]);
        });
    }

    #[test]
    fn foreground_never_waits_for_queued_background() {
        rt().block_on(async {
            let s = Arc::new(Sched::new(Duration::ZERO));
            let log = Arc::new(Mutex::new(vec![]));
            // one background request running, three more queued
            let mut hs = vec![];
            for i in 0..4 {
                let (s, log) = (s.clone(), log.clone());
                hs.push(tokio::spawn(async move { s.bg(None, req(&log, &format!("b{i}"), 60)).await }));
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            let t = Instant::now();
            s.fg(req(&log, "click", 5)).await;
            // it ran at once, beside the one background request already running
            assert!(t.elapsed() < Duration::from_millis(40));
            for h in hs {
                h.await.unwrap();
            }
            let l = log.lock().unwrap().clone();
            let click = l.iter().position(|e| e == "click+").unwrap();
            assert_eq!(l[..click], ["b0+"], "{l:?}");
        });
    }

    #[test]
    fn a_promoted_background_request_goes_as_foreground() {
        rt().block_on(async {
            let s = Arc::new(Sched::new(Duration::ZERO));
            let log = Arc::new(Mutex::new(vec![]));
            let mut hs = vec![];
            for i in 0..3 {
                let (s, log) = (s.clone(), log.clone());
                hs.push(tokio::spawn(async move { s.bg(None, req(&log, &format!("b{i}"), 80)).await }));
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            let p = Promote::new();
            let (s2, l2, p2) = (s.clone(), log.clone(), p.clone());
            let tab = tokio::spawn(async move { s2.bg(Some(&p2), req(&l2, "tab", 5)).await });
            tokio::time::sleep(Duration::from_millis(10)).await;
            s.promote(&p);
            tab.await.unwrap();
            let l = log.lock().unwrap().clone();
            let at = l.iter().position(|e| e == "tab+").unwrap();
            assert_eq!(l[..at], ["b0+"], "{l:?}");
            for h in hs {
                h.await.unwrap();
            }
            assert!(s.idle());
        });
    }

    #[test]
    fn a_cancelled_background_request_leaves_the_queue() {
        rt().block_on(async {
            let s = Arc::new(Sched::new(Duration::ZERO));
            let g = s.fg_guard();
            let s2 = s.clone();
            let waiting = tokio::spawn(async move { s2.bg(None, async {}).await });
            tokio::time::sleep(Duration::from_millis(10)).await;
            waiting.abort();
            let _ = waiting.await;
            drop(g);
            // the next one isn't stuck behind it
            tokio::time::timeout(Duration::from_millis(500), s.bg(None, async {})).await.unwrap();
            assert!(s.idle());
        });
    }
}
