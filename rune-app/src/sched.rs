//! Background requests: a light rule, now that the backend's TIDAL client sends requests
//! concurrently (at most `tidal_api::MAX_IN_FLIGHT` at once).
//!
//! What the user did not ask for yet (prefetching the Home tabs not showing, refreshing a stale
//! tab, the feed and its unread dot, the profile and avatar, favourite id lists, the update
//! check) goes through [`bg`], which lets at most [`BG_SLOTS`] such requests run at once, so a
//! start-up burst never takes every slot from the page on screen, a click or a play. Everything
//! else is asked for directly. A background request still waiting that the user turns out to
//! need (they open the tab it prefetches) can be [`promote`]d: it stops waiting and goes now.
//!
//! (An earlier version of this module ran a full foreground/background scheduler, needed while
//! the backend answered one request at a time. With concurrent requests it made no measurable
//! difference to what the user waits for: bench/latency.py, 2026-11-14.)

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Notify, Semaphore};

/// Background requests that may run at once (of the client's six).
pub const BG_SLOTS: usize = 2;

static SLOTS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(BG_SLOTS));
static PROMOTED: LazyLock<Notify> = LazyLock::new(Notify::new);

/// Set to let a waiting background request go at once (see [`bg`]).
#[derive(Clone, Default)]
pub struct Promote(Arc<AtomicBool>);

impl Promote {
    pub fn new() -> Promote {
        Promote::default()
    }
}

/// Lets a waiting background request go at once.
pub fn promote(p: &Promote) {
    if !p.0.swap(true, Ordering::Relaxed) {
        PROMOTED.notify_waiters();
    }
}

/// Runs `f` as background work: once one of the [`BG_SLOTS`] is free, or at once if `promote`
/// is set meanwhile. `SONE_SCHED=0` turns the rule off (timing runs compare with and without).
pub async fn bg<F: Future>(promote: Option<&Promote>, f: F) -> F::Output {
    static OFF: LazyLock<bool> = LazyLock::new(|| std::env::var("SONE_SCHED").is_ok_and(|v| v == "0"));
    if *OFF {
        return f.await;
    }
    run(&SLOTS, &PROMOTED, promote, f).await
}

async fn run<F: Future>(slots: &Semaphore, promoted: &Notify, promote: Option<&Promote>, f: F) -> F::Output {
    let _slot = loop {
        if promote.is_some_and(|p| p.0.load(Ordering::Relaxed)) {
            break None;
        }
        let woken = promoted.notified();
        tokio::pin!(woken);
        woken.as_mut().enable();
        if promote.is_some_and(|p| p.0.load(Ordering::Relaxed)) {
            break None;
        }
        tokio::select! {
            s = slots.acquire() => break s.ok(),
            _ = woken => {}
        }
    };
    f.await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap()
    }

    #[test]
    fn background_requests_never_take_more_than_their_slots() {
        rt().block_on(async {
            let slots = Arc::new(Semaphore::new(BG_SLOTS));
            let notify = Arc::new(Notify::new());
            let (running, max) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
            let mut hs = vec![];
            for _ in 0..8 {
                let (slots, notify, running, max) = (slots.clone(), notify.clone(), running.clone(), max.clone());
                hs.push(tokio::spawn(async move {
                    run(&slots, &notify, None, async {
                        let n = running.fetch_add(1, Ordering::SeqCst) + 1;
                        max.fetch_max(n, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        running.fetch_sub(1, Ordering::SeqCst);
                    }).await
                }));
            }
            for h in hs {
                h.await.unwrap();
            }
            assert_eq!(max.load(Ordering::SeqCst), BG_SLOTS);
        });
    }

    #[test]
    fn a_promoted_request_does_not_wait_for_a_slot() {
        rt().block_on(async {
            let slots = Arc::new(Semaphore::new(1));
            let notify = Arc::new(Notify::new());
            let log = Arc::new(Mutex::new(vec![]));
            // the one slot busy for 300 ms
            let (s1, n1, l1) = (slots.clone(), notify.clone(), log.clone());
            let busy = tokio::spawn(async move {
                run(&s1, &n1, None, async {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    l1.lock().unwrap().push("busy");
                }).await
            });
            tokio::time::sleep(Duration::from_millis(10)).await;
            let p = Promote::new();
            let (s2, n2, l2, p2) = (slots.clone(), notify.clone(), log.clone(), p.clone());
            let t = Instant::now();
            let tab = tokio::spawn(async move { run(&s2, &n2, Some(&p2), async { l2.lock().unwrap().push("tab") }).await });
            tokio::time::sleep(Duration::from_millis(20)).await;
            // what `promote` does, on this test's own notifier
            if !p.0.swap(true, Ordering::Relaxed) {
                notify.notify_waiters();
            }
            tab.await.unwrap();
            assert!(t.elapsed() < Duration::from_millis(200), "{:?}", t.elapsed());
            busy.await.unwrap();
            assert_eq!(*log.lock().unwrap(), ["tab", "busy"]);
        });
    }
}
