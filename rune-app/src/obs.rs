//! The OBS overlay's now-playing feed (React useOverlayBridge): while the overlay server runs,
//! the current track goes to `overlay_publish_state` when it changes (a new track, play/pause,
//! the stream's quality, a seek) and once a second while it plays, so the overlay page's
//! `state.track` follows playback. It runs on the backend's runtime and reads the controller's
//! snapshot directly, so it costs the window nothing: no version bump, no rebuild.

use serde_json::Value;
use sone_host_shim::Manager;
use std::time::{Duration, Instant};
use tauri_app_lib::AppState;
use tauri_app_lib::overlay::OverlayTrackState;

/// How often the feed looks at playback while the overlay runs (a change lands within this),
/// and how often it checks whether the overlay was switched on while it doesn't.
const LOOK_EVERY: Duration = Duration::from_millis(250);
const IDLE_EVERY: Duration = Duration::from_secs(1);
/// React's position push while playing.
const PUSH_EVERY: Duration = Duration::from_secs(1);

pub fn start() {
    sone_host_shim::async_runtime::spawn(async move {
        let mut feed = Feed::default();
        loop {
            let st = crate::host().state::<AppState>();
            let enabled = st.overlay_handle.lock().await.is_some();
            if !enabled {
                feed.last = None;
                tokio::time::sleep(IDLE_EVERY).await;
                continue;
            }
            let now = controller_state();
            if let Some(track) = feed.due(now, Instant::now()) {
                let _ = tauri_app_lib::commands::overlay::overlay_publish_state(st, track).await;
            }
            tokio::time::sleep(LOOK_EVERY).await;
        }
    });
}

fn controller_state() -> Option<OverlayTrackState> {
    let (t, cover, playing, pos, quality) = crate::controller::overlay_now()?;
    Some(track_state(&t, cover.as_deref(), playing, pos, quality))
}

/// useOverlayBridge's trackState for `t`.
pub fn track_state(t: &Value, cover: Option<&str>, playing: bool, pos: f64, quality: String) -> OverlayTrackState {
    let names: Vec<&str> = t["artists"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect()).unwrap_or_default();
    let artist = if names.is_empty() { t["artist"]["name"].as_str().filter(|s| !s.is_empty()).unwrap_or("Unknown Artist").to_string() } else { names.join(", ") };
    OverlayTrackState {
        title: t["title"].as_str().unwrap_or("").to_string(),
        artist,
        album: t["album"]["title"].as_str().map(str::to_string),
        cover_url: cover.map(cover_url),
        is_playing: playing,
        position_seconds: pos,
        duration_seconds: t["duration"].as_f64().unwrap_or(0.0),
        quality,
    }
}

/// getTidalImageUrl(cover, 640).
fn cover_url(uuid: &str) -> String {
    if uuid.starts_with("http") {
        return uuid.to_string();
    }
    format!("https://resources.tidal.com/images/{}/640x640.jpg", uuid.replace('-', "/"))
}

/// What was published last, to tell a change (published at once) from a tick.
#[derive(Default)]
struct Feed {
    /// The state published last and when; None until the overlay is (again) on.
    last: Option<(Option<OverlayTrackState>, Instant)>,
}

impl Feed {
    /// What to publish now, if anything: `Some(state)` to send (`Some(None)` hides the overlay).
    fn due(&mut self, now: Option<OverlayTrackState>, at: Instant) -> Option<Option<OverlayTrackState>> {
        let send = match &self.last {
            None => true,
            Some((was, when)) => match (was, &now) {
                (None, None) => false,
                (Some(a), Some(b)) => {
                    let same = a.title == b.title && a.artist == b.artist && a.album == b.album && a.cover_url == b.cover_url && a.is_playing == b.is_playing && a.duration_seconds == b.duration_seconds && a.quality == b.quality;
                    // where the clock would be by now if nothing happened: further off is a seek
                    let expected = if a.is_playing { a.position_seconds + at.duration_since(*when).as_secs_f64() } else { a.position_seconds };
                    let jumped = (b.position_seconds - expected).abs() > 1.0;
                    !same || jumped || (b.is_playing && at.duration_since(*when) >= PUSH_EVERY)
                }
                _ => true,
            },
        };
        if !send {
            return None;
        }
        self.last = Some((now.clone(), at));
        Some(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn st(pos: f64, playing: bool) -> Option<OverlayTrackState> {
        Some(track_state(&json!({"title": "T", "artists": [{"name": "A"}, {"name": "B"}], "album": {"title": "Al"}, "duration": 200}), Some("ab-cd"), playing, pos, "24-BIT 96KHZ FLAC".into()))
    }

    #[test]
    fn the_state_reads_like_react() {
        let s = st(3.0, true).unwrap();
        assert_eq!(s.artist, "A, B");
        assert_eq!(s.album.as_deref(), Some("Al"));
        assert_eq!(s.cover_url.as_deref(), Some("https://resources.tidal.com/images/ab/cd/640x640.jpg"));
        assert_eq!(s.duration_seconds, 200.0);
        let v = track_state(&json!({"title": "T", "artist": {"name": ""}}), None, false, 0.0, String::new());
        assert_eq!(v.artist, "Unknown Artist");
        assert_eq!(v.cover_url, None);
    }

    #[test]
    fn publishes_changes_at_once_and_ticks_each_second_while_playing() {
        let mut f = Feed::default();
        let t0 = Instant::now();
        assert!(f.due(st(10.0, true), t0).is_some(), "switching on publishes");
        assert!(f.due(st(10.25, true), t0 + Duration::from_millis(250)).is_none());
        assert!(f.due(st(11.0, true), t0 + Duration::from_secs(1)).is_some(), "a second later");
        assert!(f.due(st(11.25, false), t0 + Duration::from_millis(1250)).is_some(), "pause at once");
        assert!(f.due(st(11.25, false), t0 + Duration::from_secs(5)).is_none(), "paused: nothing");
        assert!(f.due(st(60.0, false), t0 + Duration::from_millis(5250)).is_some(), "a paused seek");
        assert!(f.due(None, t0 + Duration::from_secs(6)).is_some(), "nothing playing hides it");
        assert!(f.due(None, t0 + Duration::from_secs(9)).is_none());
    }
}
