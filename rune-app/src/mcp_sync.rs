//! Library changes an MCP client made (React useMcpBridge's favourite and playlist listeners):
//! the hearts, the sidebar, the library lists and an open playlist page follow at once, as they
//! do for a change made in the app. Called for every backend event, from whichever thread emits.

use crate::data::{self, Key};
use crate::interact::{self, Fav};
use crate::paged::{self, PKey};
use serde_json::Value;
use sone_host_shim::Manager;
use std::time::Duration;
use tauri_app_lib::AppState;

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
        Change::Favorite { kind, id, on } => {
            interact::set_fav(kind, id.clone(), on);
            match kind {
                Fav::Track => paged::forget_where(|k| matches!(k, PKey::Loved { .. })),
                Fav::Album if on => {
                    // React fetches the album to put it at the top of the library at once.
                    let num = id.parse().unwrap_or(0);
                    sone_host_shim::async_runtime::spawn(async move {
                        let st = crate::host().state::<AppState>();
                        let _ = tauri_app_lib::commands::pages::get_album_detail(st, num).await;
                        data::forget(&Key::FavAlbums);
                    });
                }
                Fav::Album => data::forget(&Key::FavAlbums),
                Fav::Artist if on => {
                    let num = id.parse().unwrap_or(0);
                    sone_host_shim::async_runtime::spawn(async move {
                        let st = crate::host().state::<AppState>();
                        let _ = tauri_app_lib::commands::pages::get_artist_detail(st, num).await;
                        data::forget(&Key::FavArtists);
                    });
                }
                Fav::Artist => data::forget(&Key::FavArtists),
                _ => {}
            }
        }
        Change::Playlists { uuid, again_later } => {
            if let Some(u) = uuid {
                data::forget(&Key::Playlist(u));
            }
            forget_playlists();
            if again_later {
                sone_host_shim::async_runtime::spawn(async {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    forget_playlists();
                });
            }
        }
    }
}

/// Every list of the user's playlists: the sidebar and library, the pickers.
fn forget_playlists() {
    for k in [Key::Folders, Key::AllPlaylists, Key::FlattenedPlaylists] {
        data::forget(&k);
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
