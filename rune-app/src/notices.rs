//! What AppInitializer tells the user at launch besides the shell: a newer release (the update
//! toast, at most three times per version), the legacy sign-in nudge, and a saved sign-in TIDAL
//! no longer accepts (the playlists' 401 survives a token refresh: back to the login screen).

use crate::controller::{self, Cmd};
use crate::data::{self, Key, Load};
use crate::interact::{self, Kind};
use serde_json::{Value, json};
use sone_core::host::Manager;
use std::time::Duration;
use sone_core::AppState;
use sone_core::commands::{auth, updates};

const SEEN_KEY: &str = "sone.updateToast.v1";
const MAX_UPDATE_TOAST_SHOWS: u64 = 3;

fn st() -> sone_core::host::State<'static, AppState> {
    crate::host().state::<AppState>()
}

/// shouldShowUpdateToast: whether to show the toast for `latest` (if `available`), and what to
/// remember as seen.
pub fn should_show(available: bool, latest: &str, seen: &Value) -> (bool, Value) {
    if !available {
        return (false, seen.clone());
    }
    let count = if seen["version"].as_str() == Some(latest) { seen["count"].as_u64().unwrap_or(0) } else { 0 };
    if count >= MAX_UPDATE_TOAST_SHOWS {
        return (false, json!({"version": latest, "count": count}));
    }
    (true, json!({"version": latest, "count": count + 1}))
}

/// The launch check for a newer GitHub release (silent on any failure).
pub fn check_for_update() {
    sone_core::host::async_runtime::spawn(async {
        let Ok(info) = crate::sched::bg(None, updates::check_for_update(st())).await else { return };
        let seen = crate::shell::store_get(SEEN_KEY).unwrap_or_else(|| json!({"version": "", "count": 0}));
        let (show, next) = should_show(info.available, &info.latest, &seen);
        if show {
            interact::toast_long(format!("Update available: SONE v{}", info.latest), Kind::Info, Duration::from_secs(30), Some(("View release", info.url.clone())));
        }
        if info.available {
            crate::shell::store_set(SEEN_KEY, next);
        }
    });
}

/// After a saved sign-in was loaded at launch: the legacy device-code nudge, and the
/// playlists' 401 check.
pub fn after_saved_sign_in() {
    sone_core::host::async_runtime::spawn(async {
        if auth::consume_legacy_auth_notice(st()).unwrap_or(false) {
            interact::toast_long("Old sign-in detection, logout and sign in again with PKCE for lossless quality".into(), Kind::Info, Duration::from_secs(10), None);
        }
        // AppInitializer: the first playlists load failing with a 401 tries one token refresh;
        // if that fails too, the app shows the login screen (the saved tokens stay on disk).
        for _ in 0..240 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            match data::want(Key::Folders) {
                Load::Loading => continue,
                Load::Ready(_) => return,
                Load::Failed(e) => {
                    if !is_auth_error(&e) {
                        return;
                    }
                    match auth::refresh_tidal_auth(st()).await {
                        Ok(_) => data::forget(&Key::Folders),
                        Err(err) => {
                            log::warn!("[rune] token refresh failed at launch: {err}");
                            controller::send(Cmd::SessionExpired);
                        }
                    }
                    return;
                }
            }
        }
    });
}

/// isAuthError: a 401 status, or a body mentioning 401.
fn is_auth_error(e: &str) -> bool {
    e.starts_with("API error (401)") || (e.starts_with("API error (") && e.contains("401"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_update_toast_shows_three_times_per_version() {
        let mut seen = json!({"version": "", "count": 0});
        for _ in 0..3 {
            let (show, next) = should_show(true, "1.2.0", &seen);
            assert!(show);
            seen = next;
        }
        assert_eq!(should_show(true, "1.2.0", &seen).0, false);
        assert_eq!(should_show(true, "1.3.0", &seen), (true, json!({"version": "1.3.0", "count": 1})));
        assert_eq!(should_show(false, "1.3.0", &seen).0, false);
    }

    #[test]
    fn auth_errors() {
        assert!(is_auth_error("API error (401): expired"));
        assert!(is_auth_error("API error (400): {\"status\":401}"));
        assert!(!is_auth_error("Network error: down"));
    }
}
