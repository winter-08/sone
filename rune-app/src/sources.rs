//! Whole sources played with paging (React usePlaySource and the pagers in
//! src/lib/trackSources.ts), as MCP `play_source` / `shuffle_source` build them
//! (useMcpBridge playableForSource, PLAYBACK_SPEC §2.4-2.5).
//!
//! A source loads its first pages until it has 50 playable tracks (or runs out), off the
//! controller's actor; the controller then plays it, and the rest arrives page by page, appended
//! to the queue for as long as that queue is still the one this source started (its epoch).

use serde_json::{Value, json};
use sone_host_shim::{AppHandle, Manager};
use std::collections::HashSet;
use tauri_app_lib::AppState;
use tauri_app_lib::commands::{library, metadata, pages};
use tauri_app_lib::error::SoneError;

const START_BATCH: usize = 50;
const MAX_PAGES: usize = 200;
const ARTIST_PAGE: u32 = 50;
const PLAYLIST_PAGE: u32 = 100;

/// Where further pages come from.
#[derive(Clone, Debug)]
pub enum Pager {
    Playlist(String),
    Artist(u64),
    /// A collection page's own pager (paged.rs): a sorted playlist, Loved Tracks or Videos, an
    /// artist's popular tracks or view-all list.
    Paged(crate::paged::PKey),
}

/// A source's tracks as they load (usePlaySource's `take` / `fetchNext` state).
#[derive(Clone, Debug)]
pub struct Loader {
    /// `{type, id, name, image?, subtitle?, mixType?}`: the "Playing from" meta.
    pub meta: Value,
    pub album_mode: bool,
    pager: Option<Pager>,
    dedupe: bool,
    seen: HashSet<u64>,
    pub tracks: Vec<Value>,
    playable: usize,
    offset: u32,
    pub has_more: bool,
    pages: usize,
    /// A page failed to load during the start batch.
    pub failed: bool,
    /// A page's Shuffle button (playAllFromSource with `shuffle`), rather than shuffle mode.
    pub play_shuffle: bool,
    /// A clicked track to start from (usePlaySource `startAt`): the start batch stops once
    /// it's in.
    pub start_at: Option<u64>,
    /// Items the source leaves out (usePlaySource `include`: tracks un-loved this session).
    include: fn(&Value) -> bool,
    /// The cursor a cursor-paged pager goes on from.
    cursor: Option<String>,
}

impl Loader {
    fn new(meta: Value, loaded: Vec<Value>, pager: Option<Pager>, dedupe: bool, album_mode: bool) -> Loader {
        let offset = loaded.len() as u32;
        Loader::with(meta, loaded, pager, offset, true, dedupe, album_mode, |_| true)
    }

    /// A source with `loaded` (what a page shows) going on from raw offset `offset` while
    /// `has_more`, as a page's PlayableSource is (`nextOffset`, `hasMore`, `include`).
    #[allow(clippy::too_many_arguments)]
    pub fn with(meta: Value, loaded: Vec<Value>, pager: Option<Pager>, offset: u32, has_more: bool, dedupe: bool, album_mode: bool, include: fn(&Value) -> bool) -> Loader {
        let has_more = has_more && pager.is_some();
        let mut l = Loader { meta, album_mode, pager, dedupe, seen: HashSet::new(), tracks: vec![], playable: 0, offset, has_more, pages: 0, failed: false, play_shuffle: false, start_at: None, include, cursor: None };
        l.take(loaded, &|_| true);
        l
    }

    fn has_start(&self) -> bool {
        self.start_at.is_some_and(|id| self.tracks.iter().any(|t| t["id"].as_u64() == Some(id)))
    }

    /// Adds `items` (skipping ids already seen when deduplicating); returns the new ones and
    /// how many new ids there were.
    fn take(&mut self, items: Vec<Value>, playable: &(dyn Fn(&Value) -> bool + Sync)) -> (Vec<Value>, usize) {
        let mut fresh = vec![];
        let mut new_ids = 0;
        for t in items {
            if self.dedupe {
                let id = t["id"].as_u64().unwrap_or(0);
                if !self.seen.insert(id) {
                    continue;
                }
                new_ids += 1;
            }
            if !(self.include)(&t) {
                continue;
            }
            if playable(&t) {
                self.playable += 1;
            }
            fresh.push(t);
        }
        self.tracks.extend(fresh.iter().cloned());
        (fresh, new_ids)
    }

    /// The next page (fetchNext): its new tracks.
    pub async fn next_page(&mut self, h: &AppHandle, playable: &(dyn Fn(&Value) -> bool + Sync)) -> Result<Vec<Value>, SoneError> {
        let Some(pager) = self.pager.clone() else {
            self.has_more = false;
            return Ok(vec![]);
        };
        let (items, more) = match &pager {
            Pager::Paged(key) => {
                let (items, _, cursor, more) = crate::paged::fetch_page(key, self.offset as usize, self.cursor.take()).await.map_err(SoneError::Parse)?;
                self.cursor = cursor;
                (items.iter().map(|v| crate::paged::playable_of(key, v)).collect(), more)
            }
            _ => page(h, &pager, self.offset).await?,
        };
        self.pages += 1;
        self.offset += items.len() as u32;
        let n = items.len();
        let (fresh, new_ids) = self.take(items, playable);
        self.has_more = more && n > 0 && self.pages < MAX_PAGES && (!self.dedupe || new_ids > 0);
        Ok(fresh)
    }

    /// The start batch: pages until 50 playable tracks or the end. A failure ends paging.
    pub async fn start(&mut self, h: &AppHandle, playable: &(dyn Fn(&Value) -> bool + Sync)) {
        while self.has_more && self.playable < START_BATCH && !self.has_start() {
            if let Err(e) = self.next_page(h, playable).await {
                log::warn!("[rune] failed to load {} tracks: {e}", self.meta["type"].as_str().unwrap_or("source"));
                self.has_more = false;
                self.failed = true;
            }
        }
    }
}

/// One page from `pager` at `offset`: its tracks and whether more follow.
async fn page(h: &AppHandle, pager: &Pager, offset: u32) -> Result<(Vec<Value>, bool), SoneError> {
    let st = || h.state::<AppState>();
    match pager {
        Pager::Playlist(id) => {
            let p = library::get_playlist_tracks_page(st(), h.clone(), id.clone(), offset, PLAYLIST_PAGE, None, None).await?;
            let more = offset as usize + p.items.len() < p.total_number_of_items as usize;
            let items = p.items.into_iter().filter_map(|t| serde_json::to_value(t).ok()).collect();
            Ok((items, more))
        }
        // (paged in by next_page, with its cursor)
        Pager::Paged(_) => Ok((vec![], false)),
        Pager::Artist(id) => {
            let raw = pages::get_artist_top_tracks_all(st(), h.clone(), *id, offset, ARTIST_PAGE).await?;
            let items: Vec<Value> = raw["items"].as_array().cloned().unwrap_or_default().into_iter()
                .map(|it| if it["data"].is_object() { it["data"].clone() } else { it })
                .collect();
            // v2 has no total: a full page means there's probably more.
            let more = items.len() >= ARTIST_PAGE as usize;
            Ok((items, more))
        }
    }
}

/// playableForSource: the source MCP names, with its first tracks (or its pager). None for an
/// unknown type, or when an album or mix can't be fetched (React throws there and logs it).
pub async fn build(h: &AppHandle, kind: &str, id: &str) -> Option<Loader> {
    let st = || h.state::<AppState>();
    match kind {
        "playlist" => {
            let meta = match metadata::get_playlist_details(st(), id.to_string()).await {
                Ok(d) => json!({"type": "playlist", "id": id, "name": d["title"], "image": d.get("squareImage").filter(|v| !v.is_null()).unwrap_or(&d["image"])}),
                Err(_) => json!({"type": "playlist", "id": id, "name": "Playlist"}),
            };
            Some(Loader::new(meta, vec![], Some(Pager::Playlist(id.to_string())), false, false))
        }
        "album" => {
            let aid = id.parse::<u64>().ok()?;
            let page = match pages::get_album_page(st(), h.clone(), aid).await {
                Ok(p) => serde_json::to_value(p).ok()?,
                Err(e) => {
                    log::warn!("[rune] mcp play_source album {id}: {e}");
                    return None;
                }
            };
            let p = &page["page"];
            let meta = json!({"type": "album", "id": aid, "name": p["album"]["title"], "image": p["album"]["cover"]});
            Some(Loader::new(meta, p["tracks"].as_array().cloned().unwrap_or_default(), None, false, true))
        }
        "artist" => {
            let aid = id.parse::<u64>().ok()?;
            let meta = match pages::get_artist_detail(st(), aid).await {
                Ok(d) => json!({"type": "artist", "id": aid, "name": d.name, "image": d.picture, "subtitle": "Top tracks"}),
                Err(_) => json!({"type": "artist", "id": aid, "name": "Artist"}),
            };
            Some(Loader::new(meta, vec![], Some(Pager::Artist(aid)), true, false))
        }
        "mix" => {
            let m = match pages::get_mix_items(st(), id.to_string()).await {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("[rune] mcp play_source mix {id}: {e}");
                    return None;
                }
            };
            let meta = json!({"type": "mix", "id": id, "name": m.title.clone().unwrap_or_else(|| "Mix".into()), "image": m.image, "mixType": m.mix_type});
            let tracks = m.tracks.into_iter().filter_map(|t| serde_json::to_value(t).ok()).collect();
            Some(Loader::new(meta, tracks, None, false, false))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_skips_seen_ids_and_counts_playable() {
        let mut l = Loader::new(json!({"type": "artist"}), vec![], None, true, false);
        let (fresh, new_ids) = l.take(vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 1})], &|t| t["id"] != 2);
        assert_eq!(fresh.len(), 2);
        assert_eq!(new_ids, 2);
        assert_eq!(l.playable, 1);
        let (fresh, new_ids) = l.take(vec![json!({"id": 2})], &|_| true);
        assert!(fresh.is_empty());
        assert_eq!(new_ids, 0);
    }

    #[test]
    fn loaded_sources_without_a_pager_are_complete() {
        let l = Loader::new(json!({"type": "album"}), vec![json!({"id": 1}), json!({"id": 1})], None, false, true);
        assert_eq!(l.tracks.len(), 2);
        assert!(!l.has_more);
        assert_eq!(l.offset, 2);
    }
}
