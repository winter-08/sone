//! The proxy notice bar (React ProxyNoticeBanner): across the top of the window, on the login
//! screen and in the app, when the proxy is blocked (nothing was sent) or unreachable (requests
//! go out and nothing comes back). It offers Turn off proxy everywhere and, inside the app, Open
//! network settings and a dismissal that lasts as long as the same condition is observed.
//!
//! The status is read at start, after every proxy save, and every 15 s while a proxy is
//! configured; while it is unreachable each poll first sends one probe through the live client,
//! the only thing that can see it recover. Off, it never polls.

use crate::glyph::{G, glyph};
use crate::ui::{t, white};
use crate::views::SettingsTab;
use crate::{App, theme};
use rune::{El, col, el, row, text};
use serde_json::Value;
use sone_core::host::Manager;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use sone_core::AppState;
use sone_core::commands::utility;
use tokio::sync::Notify;

const POLL: Duration = Duration::from_secs(15);

#[derive(Default)]
struct St {
    status: Option<Value>,
    dismissed: Option<String>,
    busy: bool,
    error: Option<String>,
}

static ST: LazyLock<Mutex<St>> = LazyLock::new(Default::default);
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);

/// After Log out (React unmounts Layout's banner with the shell): a dismissal and an error
/// from the last session are gone. The proxy's status stays; it is the device's, not the
/// account's.
pub fn reset_after_logout() {
    let mut s = ST.lock().unwrap();
    s.dismissed = None;
    s.error = None;
}

fn st() -> sone_core::host::State<'static, AppState> {
    crate::host().state::<AppState>()
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Tone {
    Blocked,
    Unreachable,
}

#[derive(Debug, PartialEq)]
pub struct Notice {
    pub tone: Tone,
    pub headline: String,
    pub detail: String,
}

/// proxyNotice: what to say about `status`, or nothing for `off` and `active`.
pub fn notice(status: &Value) -> Option<Notice> {
    match status["state"].as_str()? {
        "blocked" => {
            let reason = status["reason"].as_str().unwrap_or("").trim();
            Some(Notice {
                tone: Tone::Blocked,
                headline: "Proxy blocked — nothing can connect.".into(),
                detail: if reason.is_empty() { "The proxy settings are unusable".into() } else { reason.into() },
            })
        }
        "unreachable" => {
            let endpoint = status["endpoint"].as_str().unwrap_or("").trim();
            Some(Notice {
                tone: Tone::Unreachable,
                headline: if endpoint.is_empty() { "SONE isn't getting a reply through the proxy.".into() } else { format!("SONE isn't getting a reply through the proxy at {endpoint}.") },
                detail: "Requests are going out and nothing is coming back. The proxy may be unreachable, it may be refusing the connection — a rejected password or a blocked destination look the same from here — or this machine may be offline.".into(),
            })
        }
        _ => None,
    }
}

/// noticeKey: what a dismissal is pinned to (the state and what it named).
pub fn notice_key(status: &Value) -> Option<String> {
    let n = notice(status)?;
    let named = status["endpoint"].as_str().or(status["reason"].as_str()).unwrap_or("").trim();
    Some(format!("{}:{named}", if n.tone == Tone::Blocked { "blocked" } else { "unreachable" }))
}

/// Starts the status poll (once, at start).
pub fn start() {
    sone_core::host::async_runtime::spawn(async {
        loop {
            read_status();
            let (configured, unreachable) = {
                let s = ST.lock().unwrap();
                let state = s.status.as_ref().and_then(|v| v["state"].as_str()).unwrap_or("off").to_string();
                (state != "off", state == "unreachable")
            };
            if configured {
                tokio::select! {
                    _ = tokio::time::sleep(POLL) => {
                        if unreachable {
                            let _ = utility::probe_proxy_reachability(st()).await;
                        }
                    }
                    _ = WAKE.notified() => {}
                }
            } else {
                WAKE.notified().await;
            }
        }
    });
}

/// Read the status again now (a proxy save may have changed it: React's PROXY_STATUS_EVENT).
pub fn refresh() {
    WAKE.notify_one();
}

fn read_status() {
    let Ok(v) = serde_json::to_value(utility::get_proxy_status(st())) else { return };
    let mut s = ST.lock().unwrap();
    if s.status.as_ref() == Some(&v) {
        return;
    }
    // A dismissal is forgotten as soon as the condition it hid changes.
    if s.dismissed.is_some() && s.dismissed != notice_key(&v) {
        s.dismissed = None;
    }
    s.status = Some(v);
    drop(s);
    crate::controller::bump();
}

/// Whether a proxy is on, for the network error toast's wording.
pub fn proxy_enabled() -> bool {
    utility::get_proxy_settings(st()).enabled
}

/// React checkNetworkError + AppInitializer's handler: the network error toast's text, at most
/// once every 30 s (None inside the cooldown), naming the proxy when one is on.
pub fn network_error() -> Option<&'static str> {
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap();
    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(30)) {
        return None;
    }
    *last = Some(std::time::Instant::now());
    Some(if proxy_enabled() { "Network error — check your proxy settings" } else { "Network error — check your internet connection" })
}

/// A catalogue fetch's error, passed through: a network failure shows the network error toast
/// (React's `cached` fetcher runs checkNetworkError on every failure).
pub fn checked(e: String) -> String {
    if e.starts_with("Network error:")
        && let Some(m) = network_error()
    {
        crate::interact::toast(m.into(), crate::interact::Kind::Error);
    }
    e
}

fn disable_proxy() {
    {
        let mut s = ST.lock().unwrap();
        if s.busy {
            return;
        }
        s.busy = true;
        s.error = None;
    }
    crate::controller::bump();
    sone_core::host::async_runtime::spawn(async {
        // The saved host, port and credentials stay: the user turns the proxy off, nothing more.
        let mut p = utility::get_proxy_settings(st());
        p.enabled = false;
        let r = utility::set_proxy_settings(st(), p).await;
        {
            let mut s = ST.lock().unwrap();
            s.busy = false;
            match r {
                Ok(()) => crate::settings::proxy_turned_off(),
                Err(e) => {
                    let e = e.to_string();
                    s.error = Some(if e.trim().is_empty() { "Could not turn the proxy off".into() } else { e });
                }
            }
        }
        crate::controller::bump();
        refresh();
    });
}

/// `content` under the bar, when there is one to show. `in_app`: Settings is reachable (the
/// shell), so the bar offers it and can be dismissed; on the login screen it can't.
pub fn wrap(content: El, in_app: bool) -> El {
    match bar(in_app) {
        Some(b) => col().w_full().h_full().child(b).child(col().grow(1.0).min_h(0.0).w_full().child(content)),
        None => content,
    }
}

fn bar(in_app: bool) -> Option<El> {
    let s = ST.lock().unwrap();
    let status = s.status.as_ref()?;
    let n = notice(status)?;
    let key = notice_key(status);
    if in_app && s.dismissed.is_some() && s.dismissed == key {
        return None;
    }
    let accent = theme::color_of(if n.tone == Tone::Blocked { "#ff6666" } else { "#e8a33d" });
    let button = |k: &str, label: &str| {
        theme::focusable(row().key(k.to_string()).role("button").label(label)).px(10.0).py(4.0).radius(6.0).border(1.0, accent.alpha(0.4)).hover(accent.alpha(0.15)).cursor(rune::Cursor::Pointer)
            .child(text(label.to_string()).font(t(11.5, 600.0)).color(white(0.95)).a11y_hidden())
    };
    let mut buttons = row().gap(8.0).items_center();
    if in_app {
        buttons = buttons.child(button("proxy-open-settings", "Open network settings").on_press(|s: &mut App| {
            s.nav.settings_tab = SettingsTab::Network;
            s.nav.settings_open = true;
            crate::views::load_setting_fields(s);
        }));
    }
    let off = button("proxy-turn-off", if s.busy { "Turning off…" } else { "Turn off proxy" }).busy(s.busy).on_press(|_: &mut App| disable_proxy());
    buttons = buttons.child(off);
    if in_app {
        let why = if n.tone == Tone::Blocked { "Dismiss — shown again if the reason changes" } else { "Dismiss — shown again the next time this happens" };
        let k = key.clone();
        buttons = buttons.child(
            theme::focusable(row().key("proxy-dismiss").role("button").label("Dismiss")).help(why).px(8.0).py(4.0).radius(6.0).border(1.0, accent.alpha(0.4)).hover(accent.alpha(0.15)).cursor(rune::Cursor::Pointer)
                .child(glyph(G::Close, 11.0, white(0.95)))
                .on_press(move |_: &mut App| {
                    ST.lock().unwrap().dismissed = k.clone();
                    crate::controller::bump();
                }),
        );
    }
    let mut words = row().grow(1.0).min_w(0.0).gap(10.0).items_center()
        .child(el().w(8.0).h(8.0).radius(4.0).fill(accent))
        .child(text(n.headline).font(t(11.5, 600.0)).color(accent).wrap().min_w(0.0))
        .child(text(n.detail).font(t(11.5, 400.0)).color(white(0.6)).wrap().min_w(0.0).grow(1.0));
    if let Some(e) = &s.error {
        words = words.child(text(e.clone()).font(t(11.5, 400.0)).color(white(0.6)).wrap().min_w(0.0));
    }
    Some(row().key("proxy-notice").role("alert").w_full().px(16.0).py(10.0).gap(10.0).items_center().fill(accent.alpha(0.1)).border_sides([0.0, 0.0, 1.0, 0.0], accent.alpha(0.25))
        .child(words)
        .child(buttons))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn says_what_react_says() {
        assert_eq!(notice(&json!({"state": "off"})), None);
        assert_eq!(notice(&json!({"state": "active", "degraded": []})), None);
        let b = notice(&json!({"state": "blocked", "reason": "  "})).unwrap();
        assert_eq!((b.tone, b.detail.as_str()), (Tone::Blocked, "The proxy settings are unusable"));
        let u = notice(&json!({"state": "unreachable", "endpoint": "h:8080"})).unwrap();
        assert_eq!(u.headline, "SONE isn't getting a reply through the proxy at h:8080.");
    }

    #[test]
    fn a_dismissal_is_pinned_to_the_condition() {
        assert_eq!(notice_key(&json!({"state": "unreachable", "endpoint": "h:1"})).as_deref(), Some("unreachable:h:1"));
        assert_eq!(notice_key(&json!({"state": "blocked", "reason": "bad host "})).as_deref(), Some("blocked:bad host"));
        assert_eq!(notice_key(&json!({"state": "off"})), None);
    }
}
