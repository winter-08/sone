//! Library changes an MCP client made (React useMcpBridge's favourite and playlist listeners):
//! the hearts, the sidebar, the library lists and an open playlist page follow at once, as they
//! do for a change made in the app. Called for every backend event, from whichever thread emits.

use crate::data::{self, Key};
use crate::interact::{self, Fav};
use serde_json::Value;
use std::time::Duration;

/// What an MCP event asks the views to drop and set, apart from the fetches it starts.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// A favourite flipped: the heart's set, and whether the item was added.
    Favorite { kind: Fav, id: String, on: bool },
    /// The user's playlists changed (created, renamed, deleted, tracks added or removed); the
    /// playlist itself, if one is named, is fetched again.
    Playlists { uuid: Option<String>, again_later: bool },
}

pub fn parse(name: &str, payload: &str) -> Option<Change> {
    if !name.starts_with("mcp:") {
        return None;
    }
    let p: Value = serde_json::from_str(payload).ok()?;
    let uuid = || p["uuid"].as_str().map(str::to_string);
    Some(match name {
        "mcp:favorite-changed" => {
            let kind = match p["kind"].as_str()? {
                "track" => Fav::Track,
                "album" => Fav::Album,
                "artist" => Fav::Artist,
                _ => return None,
            };
            let id = match &p["id"] {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            Change::Favorite { kind, id, on: p["action"].as_str() == Some("add") }
        }
        // (React refreshes the sidebar again 3 s after a create or a track change, for the
        // cover TIDAL makes for it.)
        "mcp:playlist-created" => Change::Playlists { uuid: uuid(), again_later: true },
        "mcp:playlist-tracks-changed" => Change::Playlists { uuid: uuid(), again_later: true },
        "mcp:playlist-updated" | "mcp:playlist-deleted" => Change::Playlists { uuid: uuid(), again_later: false },
        _ => return None,
    })
}

pub fn on_event(name: &str, payload: &str) {
    let Some(change) = parse(name, payload) else { return };
    log::info!("[rune] mcp library change {change:?}");
    match change {
        // The heart, then the lists, exactly as for a heart pressed in the app
        // (interact::refresh_lists): an addition fetches the list of its kind again unless a
        // loaded one holds it already; a removal is hidden by the lists' favourite filter.
        Change::Favorite { kind, id, on } => {
            interact::set_fav(kind, id.clone(), on);
            interact::refresh_lists(kind, &id, on);
        }
        Change::Playlists { uuid, again_later } => {
            if let Some(u) = uuid {
                data::refresh(&Key::Playlist(u));
            }
            crate::library::refresh_folders();
            if again_later {
                sone_core::host::async_runtime::spawn(async {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    crate::library::refresh_folders();
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_backend_payloads() {
        assert_eq!(parse("mcp:favorite-changed", r#"{"kind":"track","id":42,"action":"add"}"#), Some(Change::Favorite { kind: Fav::Track, id: "42".into(), on: true }));
        assert_eq!(parse("mcp:favorite-changed", r#"{"kind":"artist","id":7,"action":"remove"}"#), Some(Change::Favorite { kind: Fav::Artist, id: "7".into(), on: false }));
        assert_eq!(parse("mcp:favorite-changed", r#"{"kind":"video","id":7,"action":"add"}"#), None);
        assert_eq!(parse("mcp:playlist-deleted", r#"{"uuid":"u-1"}"#), Some(Change::Playlists { uuid: Some("u-1".into()), again_later: false }));
        assert_eq!(parse("mcp:playlist-tracks-changed", r#"{"uuid":"u-1","delta":2}"#), Some(Change::Playlists { uuid: Some("u-1".into()), again_later: true }));
        assert_eq!(parse("mcp:pause", "null"), None);
        assert_eq!(parse("track-finished", "{}"), None);
    }
}
