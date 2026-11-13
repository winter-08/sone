//! SONE's item interactions, as the React UI has them (TrackContextMenu, MediaContextMenu,
//! AddToPlaylistMenu, MoveToFolderMenu, CreatePlaylistModal, useFavorites, ToastContext):
//! - context menus for tracks and for albums, playlists, artists and mixes, opened by right-click
//!   at the pointer or by a "More options" button (anchored below it, right-aligned), with the
//!   Add to playlist and Move to folder submenus beside the item;
//! - favourites (Loved tracks, library albums, playlists and mixes, followed artists), loaded once
//!   and toggled optimistically, rolled back if the server refuses;
//! - share links (copied to the clipboard) and toasts.
//!
//! Labels, icons, order, conditions and toast texts follow the React components; where this port
//! differs it says so.
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use rune::{El, Side, TextEdit, Tx, col, el, row, spacer, text, text_input};
use serde_json::{Value, json};
use sone_host_shim::Manager;
use tauri_app_lib::AppState;
use tauri_app_lib::commands::library;

use crate::App;
use crate::controller::{self, Cmd};
use crate::data::{self, Key};
use crate::glyph::{G, glyph};
use crate::theme;
use crate::ui::{self, t, white};
use crate::views::{self, Page};

static VERSION: AtomicU64 = AtomicU64::new(0);

/// Bumps when favourites or toasts change (main::poll redraws).
pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

fn bump() {
    VERSION.fetch_add(1, Ordering::Relaxed);
}

fn state() -> sone_host_shim::State<'static, AppState> {
    crate::host().state::<AppState>()
}

/// React clips titles in toasts: over 30 characters, the first 28 and an ellipsis.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() > max + 2 { format!("{}…", s.chars().take(max).collect::<String>()) } else { s.to_string() }
}

// ------------------------------------------------------------------ favourites

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fav {
    Track,
    Album,
    Playlist,
    Artist,
    Mix,
    Video,
}

#[derive(Default)]
struct Favs {
    tracks: HashSet<String>,
    albums: HashSet<String>,
    playlists: HashSet<String>,
    artists: HashSet<String>,
    mixes: HashSet<String>,
    videos: HashSet<String>,
}

impl Favs {
    fn set(&mut self, k: Fav) -> &mut HashSet<String> {
        match k {
            Fav::Track => &mut self.tracks,
            Fav::Album => &mut self.albums,
            Fav::Playlist => &mut self.playlists,
            Fav::Artist => &mut self.artists,
            Fav::Mix => &mut self.mixes,
            Fav::Video => &mut self.videos,
        }
    }
}

static FAVS: LazyLock<Mutex<Favs>> = LazyLock::new(Default::default);
static FAVS_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Load the account's favourite ids once (the React atoms behind every heart).
pub fn load_favorites_once() {
    if FAVS_REQUESTED.swap(true, Ordering::Relaxed) {
        return;
    }
    // AppInitializer: the unified favourite-ids endpoint (tracks, albums, artists, playlists),
    // then mixes and videos, which it doesn't cover.
    sone_host_shim::async_runtime::spawn(async move {
        let u = data::user_id();
        let s = state;
        let (all, mx, videos) = tokio::join!(
            library::get_all_favorite_ids(s(), u),
            library::get_favorite_mix_ids(s()),
            library::get_favorite_video_ids(s(), u)
        );
        let set = |v: &Value| -> HashSet<String> {
            v.as_array().map(|a| a.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect()).unwrap_or_default()
        };
        let mut f = FAVS.lock().unwrap();
        if let Ok(v) = all.map(|a| serde_json::to_value(a).unwrap_or(Value::Null)) {
            f.tracks = set(&v["tracks"]);
            f.albums = set(&v["albums"]);
            f.artists = set(&v["artists"]);
            f.playlists = set(&v["playlists"]);
        }
        if let Ok(v) = mx { f.mixes = v.into_iter().collect() }
        if let Ok(v) = videos { f.videos = v.into_iter().map(|id| id.to_string()).collect() }
        drop(f);
        FAVS_LOADED.store(true, Ordering::Relaxed);
        favs_changed();
    });
}

/// The favourite sets changed: hearts redraw, and the controller re-pushes the MPRIS rating.
fn favs_changed() {
    bump();
    controller::send(Cmd::FavoritesChanged);
}

pub fn favorites_loaded() -> bool {
    FAVS_LOADED.load(Ordering::Relaxed)
}

/// Drop the account's favourite ids and recently used playlists (logging out); the next
/// sign-in loads its own.
pub fn reset_favorites() {
    *FAVS.lock().unwrap() = Favs::default();
    FAVS_LOADED.store(false, Ordering::Relaxed);
    FAVS_REQUESTED.store(false, Ordering::Relaxed);
    RECENT.lock().unwrap().clear();
    bump();
}
/// Fetch the favourite ids again at the next poll (Refresh: React reloads the app, which loads
/// them again). The hearts keep what they show until the new ids land.
pub fn reload_favorites() {
    FAVS_REQUESTED.store(false, Ordering::Relaxed);
}
static FAVS_LOADED: AtomicBool = AtomicBool::new(false);

/// A favourite changed outside the app (an MCP client): the heart shows it; nothing is sent.
pub fn set_fav(k: Fav, id: String, on: bool) {
    let changed = { let mut f = FAVS.lock().unwrap(); let set = f.set(k); if on { set.insert(id) } else { set.remove(&id) } };
    if changed { favs_changed(); }
}

pub fn is_fav(k: Fav, id: &str) -> bool {
    FAVS.lock().unwrap().set(k).contains(id)
}

/// Which toast a toggle shows (React shows none for hearts on rows, cards and headers).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Say {
    Nothing,
    /// TrackContextMenu: `Added "<t>" to Loved tracks`.
    Loved,
    /// The drawer's hearts: `Added to Loved tracks`.
    LovedShort,
    /// TrackContextMenu on a video in a playlist: `Added "<t>" to Loved videos`. (React's menu
    /// favourites a track with the video's id there; this port favourites the video.)
    LovedVideo,
    /// MediaContextMenu: `Added "<l>" to library`, or `Following "<l>"` for artists.
    Library,
}

/// Flip a favourite optimistically: the heart changes now, the server is told, and the change
/// is rolled back if it refuses (useFavorites.ts).
pub fn toggle_fav(k: Fav, id: String, label: String, say: Say) {
    let now_on = {
        let mut f = FAVS.lock().unwrap();
        let set = f.set(k);
        let on = !set.contains(&id);
        if on { set.insert(id.clone()); } else { set.remove(&id); }
        on
    };
    favs_changed();
    sone_host_shim::async_runtime::spawn(async move {
        let u = data::user_id();
        let num = id.parse::<u64>().unwrap_or(0);
        let s = state;
        let res = match (k, now_on) {
            (Fav::Track, true) => library::add_favorite_track(s(), u, num).await,
            (Fav::Track, false) => library::remove_favorite_track(s(), u, num).await,
            (Fav::Album, true) => library::add_favorite_album(s(), u, num).await,
            (Fav::Album, false) => library::remove_favorite_album(s(), u, num).await,
            (Fav::Playlist, true) => library::add_favorite_playlist(s(), u, id.clone()).await,
            (Fav::Playlist, false) => library::remove_favorite_playlist(s(), u, id.clone()).await,
            (Fav::Artist, true) => library::add_favorite_artist(s(), u, num).await,
            (Fav::Artist, false) => library::remove_favorite_artist(s(), u, num).await,
            (Fav::Mix, true) => library::add_favorite_mix(s(), id.clone()).await,
            (Fav::Mix, false) => library::remove_favorite_mix(s(), id.clone()).await,
            (Fav::Video, true) => library::add_favorite_video(s(), u, num).await,
            (Fav::Video, false) => library::remove_favorite_video(s(), u, num).await,
        };
        let l = clip(&label, 28);
        match res {
            Ok(()) => {
                refresh_lists(k, &id, now_on);
                match (say, k, now_on) {
                    (Say::Loved, _, true) => toast(format!("Added \"{l}\" to Loved tracks"), Kind::Success),
                    (Say::Loved, _, false) => toast(format!("Removed \"{l}\" from Loved tracks"), Kind::Success),
                    (Say::LovedShort, _, true) => toast("Added to Loved tracks".into(), Kind::Success),
                    (Say::LovedShort, _, false) => toast("Removed from Loved tracks".into(), Kind::Success),
                    (Say::LovedVideo, _, true) => toast(format!("Added \"{l}\" to Loved videos"), Kind::Success),
                    (Say::LovedVideo, _, false) => toast(format!("Removed \"{l}\" from Loved videos"), Kind::Success),
                    (Say::Library, Fav::Artist, true) => toast(format!("Following \"{l}\""), Kind::Success),
                    (Say::Library, Fav::Artist, false) => toast(format!("Unfollowed \"{l}\""), Kind::Success),
                    (Say::Library, _, true) => toast(format!("Added \"{l}\" to library"), Kind::Success),
                    (Say::Library, _, false) => toast(format!("Removed \"{l}\" from library"), Kind::Success),
                    _ => {}
                }
            }
            Err(e) => {
                log::warn!("[interact] favourite {k:?} {id}: {e}");
                let mut f = FAVS.lock().unwrap();
                let set = f.set(k);
                if now_on { set.remove(&id); } else { set.insert(id.clone()); }
                drop(f);
                favs_changed();
                match say {
                    Say::Loved | Say::LovedShort => toast("Failed to update Loved tracks".into(), Kind::Error),
                    Say::LovedVideo => toast("Failed to update Loved videos".into(), Kind::Error),
                    Say::Library => toast("Failed to update library".into(), Kind::Error),
                    Say::Nothing => {}
                }
            }
        }
    });
}

/// The list a favourite of kind `k` shows in, after the server took a change to `id` (React adds
/// to and removes from its caches, useFavorites.ts):
/// - Loved Tracks, Loved Videos and the library's albums, artists and mixes hide what was
///   removed already (they filter by the favourite ids), so only an addition fetches them again,
///   and only when a loaded list doesn't hold it yet;
/// - the playlist lists (folders) don't filter, so a playlist added or removed fetches them again.
/// Only that kind's lists start over: the others keep their place.
fn refresh_lists(k: Fav, id: &str, now_on: bool) {
    use crate::paged::{self, Lib, PKey};
    fn lib(l: Lib) -> impl Fn(&PKey) -> bool {
        move |key| matches!(key, PKey::Library { kind, .. } if *kind == l)
    }
    let loved = |key: &PKey| matches!(key, PKey::Loved { .. });
    let videos = |key: &PKey| *key == PKey::LovedVideos;
    match k {
        Fav::Track if now_on && !paged::all_hold(loved, id) => paged::forget_where(loved),
        Fav::Video if now_on && !paged::all_hold(videos, id) => paged::forget_where(videos),
        Fav::Album if now_on && !paged::all_hold(lib(Lib::Albums), id) => data::forget(&Key::FavAlbums),
        Fav::Artist if now_on && !paged::all_hold(lib(Lib::Artists), id) => data::forget(&Key::FavArtists),
        Fav::Mix if now_on && !paged::all_hold(lib(Lib::Mixes), id) => data::forget(&Key::FavMixes),
        Fav::Playlist if !(now_on && paged::all_hold(lib(Lib::Playlists), id)) => data::forget(&Key::Folders),
        _ => {}
    }
}

// ------------------------------------------------------------------ toasts

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Success,
    Error,
    Info,
}

#[derive(Clone)]
enum ToastAction {
    /// Re-add tracks the server skipped as duplicates ("Add anyway").
    AddAgain { playlist: String, label: String, ids: Vec<u64>, requested: usize },
    /// Open a web page (the update toast's "View release").
    OpenUrl(String),
}

struct Toast {
    id: u64,
    msg: String,
    kind: Kind,
    until: Instant,
    action: Option<(String, ToastAction)>,
}

static TOASTS: LazyLock<Mutex<Vec<Toast>>> = LazyLock::new(Default::default);
static NEXT_TOAST: AtomicU64 = AtomicU64::new(1);

pub fn toast(msg: String, kind: Kind) {
    push_toast(msg, kind, Duration::from_millis(3000), None);
}

/// A toast that lasts `life`, with a button (label, url) opening a web page if given (the
/// update toast's "View release").
pub fn toast_long(msg: String, kind: Kind, life: Duration, link: Option<(&str, String)>) {
    push_toast(msg, kind, life, link.map(|(label, url)| (label.to_string(), ToastAction::OpenUrl(url))));
}

fn push_toast(msg: String, kind: Kind, life: Duration, action: Option<(String, ToastAction)>) {
    let id = NEXT_TOAST.fetch_add(1, Ordering::Relaxed);
    // A toast raised while the window is away starts its time when the window comes back.
    let from = TOASTS_HELD.lock().unwrap().unwrap_or_else(Instant::now);
    TOASTS.lock().unwrap().push(Toast { id, msg, kind, until: from + life, action });
    bump();
}

/// Since when the main window has been off screen, while it is: the toasts' time stands still
/// meanwhile, as Sonner holds its toasts while `document.hidden`, so none expires unseen.
static TOASTS_HELD: Mutex<Option<Instant>> = Mutex::new(None);

/// The main window went off screen (`held`) or came back: holds the toasts' time while it's
/// away, and gives each back what it had left when it returns.
pub fn hold_toasts(held: bool) {
    let mut since = TOASTS_HELD.lock().unwrap();
    if held {
        since.get_or_insert_with(Instant::now);
    } else if let Some(at) = since.take() {
        let away = at.elapsed();
        for t in TOASTS.lock().unwrap().iter_mut() {
            t.until += away;
        }
    }
}

/// Has a screen reader say the new track ("Title by Artist", politely) once per track change:
/// not on every poll, not when repeat starts the same track again, and not for the track a
/// launch restores (nothing changed for the listener). Costs one lock per poll otherwise.
pub fn announce_track(tx: &mut Tx) {
    // None until the restored session is known; then the track announced last.
    static SAID: Mutex<Option<Option<u64>>> = Mutex::new(None);
    let (id, _) = controller::now_playing();
    let mut said = SAID.lock().unwrap();
    if *said == Some(id) {
        return;
    }
    let v = controller::view();
    if !v.auth_known {
        return;
    }
    let restored = said.is_none();
    *said = Some(id);
    if restored || id.is_none() || v.title.is_empty() {
        return;
    }
    let words = if v.artist.is_empty() { v.title } else { format!("{} by {}", v.title, v.artist) };
    tx.announce(words, rune::LiveRegion::Polite);
}

/// Drop expired toasts; how long until the next one expires (for the poll's wake). Nothing
/// expires while [`hold_toasts`] holds them.
pub fn toast_tick() -> Option<Duration> {
    if TOASTS_HELD.lock().unwrap().is_some() {
        return None;
    }
    let mut v = TOASTS.lock().unwrap();
    let before = v.len();
    let now = Instant::now();
    v.retain(|t| t.until > now);
    if v.len() != before {
        bump();
    }
    v.iter().map(|t| t.until - now).min()
}

fn dismiss_toast(id: u64) {
    TOASTS.lock().unwrap().retain(|t| t.id != id);
    bump();
}

/// The toast stack, bottom-centre above the player bar (ToastContext: newest at the bottom).
pub fn toast_layer() -> Option<El> {
    let v = TOASTS.lock().unwrap();
    if v.is_empty() {
        return None;
    }
    let mut stack = col().gap(8.0).items_center();
    // Toasts are built last, so an open menu or dialog has its glass first; the stack is glass
    // only if what's left of the frame's blurs covers every toast, else it's all opaque.
    let surface = theme::overlay_surfaces(theme::Overlay::Toast, v.len());
    for toast in v.iter() {
        let (dot_bg, dot) = match toast.kind {
            Kind::Success => (theme::success(), G::Check),
            Kind::Error => (theme::danger(), G::Close),
            Kind::Info => (white(0.25), G::Check),
        };
        let id = toast.id;
        let mut r = surface(row().key(format!("toast-{id}")).role("status").label(&toast.msg).gap(10.0).px(14.0).h(40.0).items_center())
            .child(el().size(18.0, 18.0).radius(9.0).fill(dot_bg).items_center().justify_center().child(glyph(dot, 12.0, white(0.98))))
            .child(text(toast.msg.clone()).font(t(13.0, 500.0)).color(white(0.92)).a11y_hidden());
        if let Some((label, action)) = &toast.action {
            let a = action.clone();
            r = r.child(
                theme::focusable(row().key(format!("toast-{id}-act")).role("button").label(label.as_str())).px(6.0).h(26.0).radius(theme::inset_radius(7.0)).items_center().hover(white(0.08)).cursor(rune::Cursor::Pointer)
                    .on_press(move |_: &mut App| {
                        dismiss_toast(id);
                        run_toast_action(a.clone());
                    })
                    .child(text(label.clone()).font(t(13.0, 650.0)).color(theme::accent()).a11y_hidden()),
            );
        }
        r = r.child(
            theme::focusable(row().key(format!("toast-{id}-x")).role("button").label("Dismiss")).size(22.0, 22.0).radius(11.0).items_center().justify_center().hover(white(0.08)).cursor(rune::Cursor::Pointer)
                .on_press(move |_: &mut App| dismiss_toast(id))
                .child(glyph(G::Close, 14.0, white(0.55))),
        );
        stack = stack.child(r);
    }
    Some(row().pin(Some(0.0), None, Some(0.0), Some(104.0)).justify_center().child(stack))
}

fn run_toast_action(a: ToastAction) {
    match a {
        ToastAction::AddAgain { playlist, label, ids, requested } => add_to_playlist(playlist, label, ids, Some(requested)),
        ToastAction::OpenUrl(url) => crate::views::open_url(&url),
    }
}

// ------------------------------------------------------------------ menus

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Media {
    Album,
    Playlist,
    Artist,
    Mix,
}

impl Media {
    pub fn of(item: &Value) -> Option<Media> {
        match data::kind(item).as_str() {
            "ALBUM" => Some(Media::Album),
            "PLAYLIST" | "USER_PLAYLIST" => Some(Media::Playlist),
            "ARTIST" => Some(Media::Artist),
            "MIX" => Some(Media::Mix),
            _ => {
                let d = data::data(item);
                if d.get("uuid").is_some() { Some(Media::Playlist) } else if d["mixType"].is_string() { Some(Media::Mix) } else { None }
            }
        }
    }
}

/// A track's place in the user's own playlist, for "Remove from playlist".
#[derive(Clone, Debug)]
pub struct InPlaylist {
    pub uuid: String,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub enum Target {
    Track { track: Value, playlist: Option<InPlaylist> },
    Media { item: Value, kind: Media },
}

#[derive(Clone, Debug)]
pub enum Anchor {
    /// Right-click: the menu's top-left at the pointer, flipping up when it doesn't fit.
    Point(f32, f32),
    /// A "More options" button (its element key): below it, right-aligned, 4 pt away.
    Below(String),
}

#[derive(Clone, Debug)]
pub enum Sub {
    /// Beside the menu's "Add to playlist" item, or (`below`) under a track row's "+" button.
    AddToPlaylist { ids: Vec<u64>, all: bool, query: TextEdit, anchor: String, below: bool },
    /// Move to folder (library.rs): `all` shows every folder, found by `query`.
    MoveToFolder { uuid: String, label: String, all: bool, query: TextEdit },
}

#[derive(Clone, Debug)]
pub struct Ctx {
    pub target: Target,
    pub anchor: Anchor,
    pub sub: Option<Sub>,
    pub confirm_delete: bool,
    /// False when only the submenu shows (a row's "+" opens the playlist picker by itself).
    pub menu: bool,
    /// A submenu about to open or close on hover (see `tick_hover`).
    pub intent: Option<Intent>,
}

/// Which submenu an item opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Add,
    Move,
}

/// Hover intent, as desktop menus have it (React SONE opens submenus on click only; this port
/// also opens them on hover). Resting on a submenu item opens it after `OPEN_DELAY`; hovering
/// another item closes an open submenu after `CLOSE_DELAY`, unless the pointer reaches the
/// submenu first, so a diagonal move towards it doesn't close it on the way.
#[derive(Clone, Copy, Debug)]
pub enum Intent {
    Open(Which, Instant),
    Close(Instant),
}

const OPEN_DELAY: Duration = Duration::from_millis(150);
const CLOSE_DELAY: Duration = Duration::from_millis(300);

/// The create-playlist dialog (CreatePlaylistModal), with the tracks to add once it exists.
#[derive(Clone, Debug)]
pub struct Create {
    pub title: TextEdit,
    pub desc: TextEdit,
    pub public: bool,
    pub ids: Vec<u64>,
    pub saving: bool,
    /// The last save failed (shown in the dialog).
    pub error: Option<String>,
}

impl Create {
    /// A new, empty dialog that adds `ids` to the playlist it makes.
    pub fn new(ids: Vec<u64>) -> Create {
        Create { title: TextEdit::new(""), desc: TextEdit::new(""), public: false, ids, saving: false, error: None }
    }
}

/// Counts the menus opened, so an async result for one menu never lands in a later one.
static MENU_GEN: AtomicU64 = AtomicU64::new(0);

fn new_menu() -> u64 {
    MENU_GEN.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn open_track(s: &mut App, track: Value, playlist: Option<InPlaylist>, anchor: Anchor) {
    new_menu();
    s.nav.ctx = Some(Ctx { target: Target::Track { track, playlist }, anchor, sub: None, confirm_delete: false, menu: true, intent: None });
}

pub fn open_media(s: &mut App, item: Value, anchor: Anchor) {
    if let Some(kind) = Media::of(&item) {
        new_menu();
        s.nav.ctx = Some(Ctx { target: Target::Media { item, kind }, anchor, sub: None, confirm_delete: false, menu: true, intent: None });
    }
}

/// A track row's "+": the playlist picker alone, under the button (TrackList: no submenu offset).
pub fn open_add_below(s: &mut App, track: Value, button_key: String) {
    let id = track_id(&track);
    new_menu();
    s.nav.ctx = Some(Ctx {
        target: Target::Track { track, playlist: None },
        anchor: Anchor::Below(button_key.clone()),
        sub: Some(Sub::AddToPlaylist { ids: vec![id], all: false, query: TextEdit::new(""), anchor: button_key, below: true }),
        confirm_delete: false,
        menu: false,
        intent: None,
    });
    data::want(Key::AllPlaylists);
}

/// Escape: the submenu first, then the menu (dismissStack: a context menu is the top layer).
pub fn on_escape(s: &mut App) -> bool {
    if s.nav.create.is_some() {
        s.nav.create = None;
        return true;
    }
    match &mut s.nav.ctx {
        Some(c) if c.confirm_delete => c.confirm_delete = false,
        Some(c) if c.sub.is_some() && c.menu => c.sub = None,
        Some(_) => s.nav.ctx = None,
        None => return false,
    }
    true
}

fn close(s: &mut App) {
    s.nav.ctx = None;
}

fn mi(key: &str, label: &str, g: G, accent: bool, enabled: bool, on: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let (fg, ic) = if accent { (white(0.95), theme::accent()) } else { (white(0.78), white(0.5)) };
    let mut r = row().key(key.to_string()).role("button").label(label).w_full().radius(MENU_ROW_R).px(16.0).h(40.0).gap(12.0).items_center()
        .child(glyph(g, 18.0, ic))
        .child(text(label.to_string()).font(t(14.0, 500.0)).color(fg).truncate().a11y_hidden());
    if enabled {
        r = theme::focusable_row(r).hover(white(0.06)).cursor(rune::Cursor::Pointer).on_press_tx(on);
    } else {
        r = r.opacity(0.4);
    }
    r.on_hover_change(|s: &mut App, inside, tx| if inside { hover_plain(s, tx) })
}

/// The pointer is on an item that opens no submenu: close an open one, after the grace delay.
fn hover_plain(s: &mut App, tx: &mut Tx) {
    if let Some(c) = &mut s.nav.ctx
        && c.menu
        && c.sub.is_some()
    {
        c.intent = Some(Intent::Close(Instant::now() + CLOSE_DELAY));
        tx.wake_after(CLOSE_DELAY);
    }
}

fn mi_danger(key: &str, label: &str, on: impl Fn(&mut App, &mut Tx) + 'static) -> El {
    let red = theme::danger();
    theme::focusable_row(row().key(key.to_string()).role("button").label(label)).w_full().radius(MENU_ROW_R).px(16.0).h(40.0).gap(12.0).items_center()
        .hover(white(0.06)).cursor(rune::Cursor::Pointer).on_press_tx(on)
        .on_hover_change(|s: &mut App, inside, tx| if inside { hover_plain(s, tx) })
        .child(glyph(G::Trash, 18.0, red))
        .child(text(label.to_string()).font(t(14.0, 500.0)).color(red).a11y_hidden())
}

fn mi_sub(key: &str, label: &str, g: G, open: bool, which: Which) -> El {
    theme::focusable_row(row().key(key.to_string()).role("button").label(label)).w_full().radius(MENU_ROW_R).px(16.0).h(40.0).gap(12.0).items_center()
        .fill(if open { white(0.06) } else { rune::Color::TRANSPARENT }).hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            if let Some(c) = &mut s.nav.ctx {
                c.intent = None;
            }
            open_sub(s, which);
        })
        .on_hover_change(move |s: &mut App, inside, tx| {
            let Some(c) = &mut s.nav.ctx else { return };
            if inside {
                tx.wake_after(OPEN_DELAY);
            }
            let showing = matches!((&c.sub, which), (Some(Sub::AddToPlaylist { .. }), Which::Add) | (Some(Sub::MoveToFolder { .. }), Which::Move));
            c.intent = match (inside, showing, c.intent) {
                (true, true, _) => None,
                (true, false, _) => Some(Intent::Open(which, Instant::now() + OPEN_DELAY)),
                (false, _, Some(Intent::Open(w, _))) if w == which => None,
                (false, _, i) => i,
            };
        })
        .child(glyph(g, 18.0, white(0.5)))
        .child(text(label.to_string()).font(t(14.0, 500.0)).color(white(0.78)).a11y_hidden())
        .child(spacer())
        .child(glyph(G::ChevronRight, 16.0, white(0.5)))
}

fn sep() -> El {
    el().w_full().h(1.0).my(4.0).fill(white(0.08))
}

/// A menu's padding, and its rows' corner radius, concentric with the menu's corners.
const MENU_PAD: f32 = 4.0;
const MENU_ROW_R: f32 = theme::inset_radius(MENU_PAD);

fn panel(key: &str, w: f32) -> El {
    // a press on the menu's own padding isn't an outside click; Up and Down move between items
    theme::overlay_surface(col().key(key.to_string()).role("menu").w(w).padding(MENU_PAD), theme::Overlay::Menu)
        .focus_group()
        .on_press(|_: &mut App| {})
}

fn track_id(t: &Value) -> u64 {
    data::data(t)["id"].as_u64().unwrap_or(0)
}

fn title_of(v: &Value) -> String {
    data::name(v)
}

fn media_id(item: &Value, kind: Media) -> String {
    let d = data::data(item);
    match kind {
        Media::Playlist => d["uuid"].as_str().unwrap_or("").to_string(),
        Media::Mix => d["id"].as_str().map(str::to_string).unwrap_or_else(|| d["id"].to_string()),
        _ => d["id"].as_u64().map(|i| i.to_string()).unwrap_or_default(),
    }
}

fn share_url(kind: &str, id: &str) -> String {
    match kind {
        "track" => format!("https://tidal.com/track/{id}/u"),
        k => format!("https://tidal.com/{k}/{id}"),
    }
}

fn share(tx: &mut Tx, url: String) {
    tx.copy_rich(rune::ClipboardContent::new(url));
    toast("Copied share link to clipboard".into(), Kind::Success);
}

fn own_playlist(item: &Value) -> bool {
    let d = data::data(item);
    let me = data::user_id();
    me != 0 && d["creator"]["id"].as_u64() == Some(me)
}

/// The open menu, its submenu and dialogs, above everything (portalled, as MenuPortal).
pub fn overlay(s: &App) -> Vec<El> {
    let mut out = vec![];
    if let Some(c) = &s.nav.ctx {
        // One full-window layer holds the menu, its submenu and the confirmation. It's modal, so
        // the page behind is inert while a menu is open (no hover there); a press on the layer
        // itself is an outside click, which closes the menu (useContextMenu's outside mousedown).
        let mut layer = el().key("ctx-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .modal(false, |s: &mut App| { on_escape(s); })
            .on_press(|s: &mut App| close(s)).on_secondary_press(|s: &mut App, _, _| close(s));
        let mut push = |e: El| layer = std::mem::replace(&mut layer, el()).child(e);
        let (menu, sub) = match &c.target {
            Target::Track { track, playlist } => (track_menu(track, playlist.as_ref(), c), track_sub(s, c)),
            Target::Media { item, kind } => (media_menu(item, *kind, c), track_sub(s, c)),
        };
        if c.menu {
            match &c.anchor {
                Anchor::Point(x, y) => {
                    push(el().key("ctx-point").pin(Some(*x), Some(*y), None, None).size(1.0, 1.0));
                    push(menu.anchored("ctx-point", Side::Bottom, rune::Align::Start, 0.0));
                }
                Anchor::Below(key) => push(menu.anchored(key.clone(), Side::Bottom, rune::Align::End, 4.0)),
            }
        }
        if let Some(sub) = sub {
            push(sub);
        }
        if c.confirm_delete {
            push(confirm_delete(c));
        }
        out.push(layer);
    }
    if let Some(cr) = &s.nav.create {
        out.push(create_modal(cr));
    }
    out
}

fn track_menu(track: &Value, playlist: Option<&InPlaylist>, c: &Ctx) -> El {
    let id = track_id(track);
    let title = title_of(track);
    let d = data::data(track);
    // A playlist can hold music videos (`itemType: "video"`): they're never stream-gated, they
    // queue as the item itself (there's no track to fetch by that id), favourite as videos,
    // have no track radio and share as a video.
    let video = d["itemType"].as_str() == Some("video");
    let ok = video || (d["streamReady"].as_bool() != Some(false) && d["allowStreaming"].as_bool() != Some(false));
    let fav = if video { Fav::Video } else { Fav::Track };
    let loved = is_fav(fav, &id.to_string());
    let t1 = title.clone();
    let t2 = title.clone();
    let t3 = title.clone();
    let (item1, item2) = (d.clone(), d.clone());
    let love_label = match (video, loved) {
        (false, true) => "Remove from Loved tracks",
        (false, false) => "Add to Loved tracks",
        (true, true) => "Remove from Loved videos",
        (true, false) => "Add to Loved videos",
    };
    let mut m = panel("ctx-menu", 240.0)
        .child(mi("ctx-next", "Play next", G::ListEnd, false, ok, move |s, _| {
            controller::send(if video { Cmd::QueueItems { items: vec![item1.clone()], next: true } } else { Cmd::PlayNextIds(vec![id]) });
            toast(format!("\"{}\" will play next", clip(&t1, 28)), Kind::Success);
            close(s);
        }))
        .child(mi("ctx-queue", "Add to play queue", G::ListPlus, false, ok, move |s, _| {
            controller::send(if video { Cmd::QueueItems { items: vec![item2.clone()], next: false } } else { Cmd::QueueIds(vec![id]) });
            toast(format!("Added \"{}\" to queue", clip(&t2, 28)), Kind::Success);
            close(s);
        }))
        .child(sep())
        .child(mi_sub("ctx-add", "Add to playlist", G::List, matches!(c.sub, Some(Sub::AddToPlaylist { .. })), Which::Add))
        .child(mi("ctx-love", love_label, if loved { G::HeartFilled } else { G::Heart }, loved, true, move |s, _| {
            toggle_fav(fav, id.to_string(), t3.clone(), if video { Say::LovedVideo } else { Say::Loved });
            close(s);
        }));
    // Track radio: hidden when the track says it has none (React: `mixes` without TRACK_MIX),
    // and for videos.
    let mix = d["mixes"]["TRACK_MIX"].as_str().map(str::to_string);
    let has_mixes = d["mixes"].is_object();
    if !video && (mix.is_some() || !has_mixes) {
        let (t4, artist) = (title.clone(), data::artist_of(track));
        m = m.child(sep()).child(mi("ctx-radio", "Go to track radio", G::Radio, false, true, move |s, _| {
            close(s);
            match &mix {
                Some(mid) => go_radio(s, mid.clone(), &t4, &artist),
                None => {
                    // React fetches the track for its mixes first; this port asks the same.
                    let (t5, a5) = (t4.clone(), artist.clone());
                    sone_host_shim::async_runtime::spawn(async move {
                        let found = tauri_app_lib::commands::metadata::get_track(state(), id).await.ok()
                            .and_then(|t| t["mixes"]["TRACK_MIX"].as_str().map(str::to_string));
                        match found {
                            Some(mid) => {
                                *PENDING_NAV.lock().unwrap() = Some(Page::Mix(mid, format!("{t5} Radio"), format!("Based on {a5}")));
                                bump();
                            }
                            None => toast("Track radio unavailable".into(), Kind::Info),
                        }
                    });
                }
            }
        }));
    }
    m = m.child(sep()).child(mi("ctx-share", "Share", G::Link, false, true, move |s, tx| {
        share(tx, share_url(if video { "video" } else { "track" }, &id.to_string()));
        close(s);
    }));
    if let Some(p) = playlist {
        let (uuid, index, t6) = (p.uuid.clone(), p.index, title.clone());
        m = m.child(sep()).child(mi_danger("ctx-remove", "Remove from playlist", move |s, _| {
            close(s);
            let (uuid, t6) = (uuid.clone(), t6.clone());
            sone_host_shim::async_runtime::spawn(async move {
                match library::remove_track_from_playlist(state(), uuid.clone(), index as u32).await {
                    Ok(()) => {
                        data::forget(&Key::Playlist(uuid));
                        toast(format!("Removed \"{}\" from playlist", clip(&t6, 28)), Kind::Success);
                    }
                    Err(_) => toast("Failed to remove track".into(), Kind::Error),
                }
            });
        }));
    }
    m
}

static PENDING_NAV: LazyLock<Mutex<Option<Page>>> = LazyLock::new(Default::default);

/// A page an async action wants to show (track radio found after a fetch).
pub fn take_pending_nav() -> Option<Page> {
    PENDING_NAV.lock().unwrap().take()
}

fn go_radio(s: &mut App, mix: String, title: &str, artist: &str) {
    views::go(s, Page::Mix(mix, format!("{title} Radio"), format!("Based on {artist}")));
}

fn media_menu(item: &Value, kind: Media, c: &Ctx) -> El {
    let id = media_id(item, kind);
    let label = title_of(item);
    let it = item.clone();
    let (l1, l2, l3) = (label.clone(), label.clone(), label.clone());
    let (i1, i2, i3) = (it.clone(), it.clone(), it.clone());
    let mut m = panel("ctx-menu", 240.0)
        .child(mi("ctx-playnow", "Play now", G::Play, false, true, move |s, _| {
            play_media(&i1, kind);
            toast(format!("Now playing \"{}\"", clip(&l1, 28)), Kind::Success);
            close(s);
        }))
        .child(mi("ctx-next", "Play next", G::ListEnd, false, true, move |s, _| {
            let (item, l) = (i2.clone(), l2.clone());
            close(s);
            sone_host_shim::async_runtime::spawn(async move {
                match media_track_ids(&item, kind).await {
                    Some(ids) if !ids.is_empty() => {
                        controller::send(Cmd::PlayNextIds(ids));
                        toast(format!("\"{}\" will play next", clip(&l, 28)), Kind::Success);
                    }
                    _ => toast("Failed to play next".into(), Kind::Error),
                }
            });
        }))
        .child(mi("ctx-queue", "Add to play queue", G::ListPlus, false, true, move |s, _| {
            let (item, l) = (i3.clone(), l3.clone());
            close(s);
            sone_host_shim::async_runtime::spawn(async move {
                match media_track_ids(&item, kind).await {
                    Some(ids) if !ids.is_empty() => {
                        controller::send(Cmd::QueueIds(ids));
                        toast(format!("Added \"{}\" to queue", clip(&l, 28)), Kind::Success);
                    }
                    _ => toast("Failed to add to queue".into(), Kind::Error),
                }
            });
        }))
        .child(sep());
    // (a spinner in place of its icon while the item's tracks are fetched for it)
    let busy = fetching_ids();
    m = m.child(mi_sub("ctx-add", "Add to playlist", if busy { G::Loader } else { G::List }, matches!(c.sub, Some(Sub::AddToPlaylist { .. })), Which::Add).busy(busy));
    if kind == Media::Playlist {
        m = m.child(mi_sub("ctx-move", "Move to folder", G::FolderInput, matches!(c.sub, Some(Sub::MoveToFolder { .. })), Which::Move));
    }
    let fav_kind = match kind {
        Media::Album => Fav::Album,
        Media::Playlist => Fav::Playlist,
        Media::Artist => Fav::Artist,
        Media::Mix => Fav::Mix,
    };
    let on = is_fav(fav_kind, &id);
    let (lab, g) = match (kind, on) {
        (Media::Artist, false) => ("Follow artist", G::UserPlus),
        (Media::Artist, true) => ("Unfollow artist", G::UserCheck),
        (_, false) => ("Add to my library", G::Heart),
        (_, true) => ("Remove from my library", G::HeartFilled),
    };
    let (id5, l5) = (id.clone(), label.clone());
    m = m.child(sep()).child(mi("ctx-library", lab, g, on, true, move |s, _| {
        toggle_fav(fav_kind, id5.clone(), l5.clone(), Say::Library);
        close(s);
    }));
    let path = match kind {
        Media::Album => "album",
        Media::Playlist => "playlist",
        Media::Artist => "artist",
        Media::Mix => "mix",
    };
    let url = share_url(path, &id);
    m = m.child(sep()).child(mi("ctx-share", "Share", G::Link, false, true, move |s, tx| {
        share(tx, url.clone());
        close(s);
    }));
    if kind == Media::Playlist && own_playlist(item) {
        m = m.child(sep()).child(mi_danger("ctx-delete", "Delete playlist", |s, _| {
            if let Some(c) = &mut s.nav.ctx {
                c.confirm_delete = true;
            }
        }));
    }
    m
}

/// A media item's track ids for the Add to playlist submenu, fetched for the menu `menu_no`
/// (AddToPlaylistMenu's playlistTrackIds): `None` while the fetch runs, then the ids, which the
/// next poll opens the submenu with if that menu is still the one open.
#[derive(Default)]
struct SubIds {
    menu_no: u64,
    ids: Option<Vec<u64>>,
    /// The ids arrived and the submenu hasn't opened with them yet.
    pending: bool,
}

static SUB_IDS: LazyLock<Mutex<SubIds>> = LazyLock::new(Default::default);

/// The open menu's track ids are being fetched for its Add to playlist (its item shows a spinner).
fn fetching_ids() -> bool {
    let s = SUB_IDS.lock().unwrap();
    s.menu_no == MENU_GEN.load(Ordering::Relaxed) && s.ids.is_none()
}

/// Apply an async result to the open menu: the Add to playlist submenu once a media item's
/// track ids are in, or a page to go to.
pub fn apply_pending(s: &mut App) {
    let ids = {
        let mut sub = SUB_IDS.lock().unwrap();
        let ours = sub.menu_no == MENU_GEN.load(Ordering::Relaxed) && s.nav.ctx.as_ref().is_some_and(|c| matches!(c.target, Target::Media { .. }));
        match (std::mem::take(&mut sub.pending), &sub.ids) {
            (true, Some(ids)) if ours => Some(ids.clone()),
            _ => None,
        }
    };
    if let Some(ids) = ids {
        open_add(s, ids);
    }
    if let Some(p) = take_pending_nav() {
        views::go(s, p);
    }
}

fn open_add(s: &mut App, ids: Vec<u64>) {
    if let Some(c) = &mut s.nav.ctx {
        c.sub = Some(Sub::AddToPlaylist { ids, all: false, query: TextEdit::new(""), anchor: "ctx-add".into(), below: false });
    }
    data::want(Key::AllPlaylists);
}

pub fn play_media(item: &Value, kind: Media) {
    let d = data::data(item);
    let title = data::name(item);
    match kind {
        Media::Album => views::play_feed_item("album", d["id"].clone(), &title),
        Media::Playlist => views::play_feed_item("playlist", d["uuid"].clone(), &title),
        Media::Mix => views::play_feed_item("mix", d["id"].clone(), &title),
        Media::Artist => {
            if let Some(id) = d["id"].as_u64() {
                controller::send(Cmd::PlayArtist { id, name: title, shuffle: false });
            }
        }
    }
}

fn ids_in(v: &Value) -> Vec<u64> {
    v.as_array().map(|a| a.iter().filter_map(|t| {
        let x = data::data(t);
        x["id"].as_u64().or(t["item"]["id"].as_u64())
    }).collect()).unwrap_or_default()
}

/// All of a media item's track ids (React fetchMediaTracks).
async fn media_track_ids(item: &Value, kind: Media) -> Option<Vec<u64>> {
    let d = data::data(item);
    let v = match kind {
        Media::Album => data::get(Key::Album(d["id"].as_u64()?)).await.ok().map(|p| ids_in(&p["page"]["tracks"])),
        // Every page of it (getPlaylistTracks), not the first hundred.
        Media::Playlist => crate::paged::fetch_all(&crate::paged::PKey::Playlist { uuid: d["uuid"].as_str()?.to_string(), order: None, dir: None }).await.ok().map(|items| ids_in(&Value::Array(items))),
        Media::Artist => data::get(Key::ArtistTopTracks(d["id"].as_u64()?)).await.ok().map(|p| {
            let a = ids_in(&p["items"]);
            if a.is_empty() { ids_in(&p) } else { a }
        }),
        Media::Mix => {
            let mid = d["id"].as_str().map(str::to_string).unwrap_or_else(|| d["id"].to_string());
            data::get(Key::Mix(mid)).await.ok().map(|p| {
                let a = ids_in(&p["tracks"]);
                if a.is_empty() { ids_in(&p["items"]) } else { a }
            })
        }
    };
    v
}

// ------------------------------------------------------------------ submenus

static RECENT: LazyLock<Mutex<Vec<String>>> = LazyLock::new(Default::default);

/// A playlist just added to (or made) goes first among the picker's recent ones.
fn push_recent_playlist(uuid: &str) {
    let mut r = RECENT.lock().unwrap();
    r.retain(|u| u != uuid);
    r.insert(0, uuid.to_string());
    r.truncate(8);
}

fn track_sub(s: &App, c: &Ctx) -> Option<El> {
    let sub = match c.sub.as_ref()? {
        Sub::AddToPlaylist { ids, all, query, anchor, below } => add_menu(ids, *all, query, anchor, *below),
        Sub::MoveToFolder { uuid, label, all, query } => crate::library::move_menu(s, uuid, label, *all, query, "ctx-move"),
    };
    // reaching the submenu keeps it open
    Some(sub.on_hover_change(|s: &mut App, inside, _| {
        if inside && let Some(c) = &mut s.nav.ctx && matches!(c.intent, Some(Intent::Close(_))) {
            c.intent = None;
        }
    }))
}

/// Open the menu's submenu `which` for its target (a click, or hover intent).
fn open_sub(s: &mut App, which: Which) {
    let Some(c) = &mut s.nav.ctx else { return };
    match (which, c.target.clone()) {
        (Which::Add, Target::Track { track, .. }) => open_add(s, vec![track_id(&track)]),
        (Which::Add, Target::Media { item, kind }) => {
            // React fetches every track id first (once per menu), then opens the submenu.
            let menu_no = MENU_GEN.load(Ordering::Relaxed);
            {
                let mut sub = SUB_IDS.lock().unwrap();
                if sub.menu_no == menu_no {
                    // already fetched for this menu (or on its way)
                    if let Some(ids) = sub.ids.clone() {
                        drop(sub);
                        open_add(s, ids);
                    }
                    return;
                }
                *sub = SubIds { menu_no, ids: None, pending: false };
            }
            bump();
            sone_host_shim::async_runtime::spawn(async move {
                let ids = media_track_ids(&item, kind).await;
                let mut sub = SUB_IDS.lock().unwrap();
                if sub.menu_no != menu_no {
                    return;
                }
                match ids {
                    Some(ids) => *sub = SubIds { menu_no, ids: Some(ids), pending: true },
                    None => {
                        // nothing to open: let a later press try again
                        *sub = SubIds::default();
                        drop(sub);
                        toast("Failed to load tracks".into(), Kind::Error);
                    }
                }
                bump();
            });
        }
        (Which::Move, Target::Media { item, kind: Media::Playlist }) => {
            c.sub = Some(Sub::MoveToFolder { uuid: data::data(&item)["uuid"].as_str().unwrap_or("").to_string(), label: data::name(&item), all: false, query: TextEdit::new("") });
        }
        _ => {}
    }
}

/// Act on hover intent that's due; how soon the next is (for the poll's wake).
pub fn tick_hover(s: &mut App) -> Option<Duration> {
    let c = s.nav.ctx.as_mut()?;
    let now = Instant::now();
    match c.intent? {
        Intent::Open(which, at) if at <= now => {
            c.intent = None;
            open_sub(s, which);
            bump();
            None
        }
        Intent::Close(at) if at <= now => {
            c.intent = None;
            c.sub = None;
            bump();
            None
        }
        Intent::Open(_, at) | Intent::Close(at) => Some(at - now),
    }
}

fn playlists() -> Vec<Value> {
    data::peek(&Key::AllPlaylists).map(|v| v["items"].as_array().cloned().unwrap_or_default()).unwrap_or_default()
}

fn add_menu(ids: &[u64], all: bool, query: &TextEdit, anchor: &str, below: bool) -> El {
    let ids = ids.to_vec();
    let m = panel("ctx-sub", 320.0);
    let mut m = if below { m.anchored(anchor.to_string(), Side::Bottom, rune::Align::End, 6.0) } else { m.anchored(anchor.to_string(), Side::Right, rune::Align::Start, 6.0) };
    let pls = playlists();
    let row_for = |i: usize, p: &Value, detailed: bool| -> El {
        let d = data::data(p);
        let uuid = d["uuid"].as_str().unwrap_or("").to_string();
        let title = d["title"].as_str().unwrap_or("").to_string();
        let n = d["numberOfTracks"].as_u64().unwrap_or(0);
        let (u, l, ids) = (uuid.clone(), title.clone(), ids.clone());
        let mut r = theme::focusable_row(row().key(format!("atp-{i}")).role("button").label(title.as_str())).w_full().radius(MENU_ROW_R).px(14.0).h(if detailed { 56.0 } else { 38.0 }).gap(10.0).items_center()
            .hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                close(s);
                add_to_playlist(u.clone(), l.clone(), ids.clone(), None);
            });
        if detailed {
            r = r.child(crate::ui::with_image(crate::ui::tile(crate::ui::hue(Some(i as u64 + 20)), 40.0, i as u64 + 20, 6.0), crate::art::item_url(p, 40.0), 40.0, 6.0))
                .child(col().grow(1.0).min_w(0.0).a11y_hidden()
                    .child(text(title.clone()).font(t(14.0, 500.0)).color(white(0.9)).truncate())
                    .child(text(if n > 0 { format!("{n} tracks") } else { "Playlist".into() }).font(t(12.0, 400.0)).color(white(0.5))));
        } else {
            r = r.child(text(title.clone()).font(t(14.0, 500.0)).color(white(0.85)).truncate().grow(1.0).min_w(0.0).a11y_hidden());
        }
        r.child(glyph(G::Plus, 18.0, white(0.55)))
    };
    if !all {
        let ids_c = ids.clone();
        m = m.child(theme::focusable_row(row().key("atp-create").role("button").label("Create new playlist")).w_full().radius(MENU_ROW_R).px(14.0).h(48.0).gap(12.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    close(s);
                    s.nav.create = Some(Create::new(ids_c.clone()));
                })
                .child(el().size(32.0, 32.0).radius(16.0).fill(white(0.08)).items_center().justify_center().child(glyph(G::Plus, 18.0, white(0.85))))
                .child(text("Create new playlist").font(t(15.0, 500.0)).color(white(0.92)).a11y_hidden()))
            .child(theme::focusable_row(row().key("atp-all").role("button").label("Show all playlists")).w_full().radius(MENU_ROW_R).px(14.0).h(36.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut App| if let Some(Ctx { sub: Some(Sub::AddToPlaylist { all, .. }), .. }) = &mut s.nav.ctx { *all = true })
                .child(text("Show all playlists").font(t(13.0, 500.0)).color(white(0.55)).a11y_hidden()))
            .child(text("RECENT").font(t(11.0, 700.0)).color(white(0.45)).px(14.0).py(6.0));
        let recent = RECENT.lock().unwrap().clone();
        let mut shown: Vec<&Value> = recent.iter().filter_map(|u| pls.iter().find(|p| data::data(p)["uuid"].as_str() == Some(u.as_str()))).collect();
        if shown.is_empty() {
            shown = pls.iter().take(8).collect();
        }
        if pls.is_empty() && data::peek(&Key::AllPlaylists).is_none() {
            m = m.busy(true).child(row().px(14.0).h(36.0).items_center().child(glyph(G::Loader, 16.0, white(0.5))));
        }
        for (i, p) in shown.into_iter().take(8).enumerate() {
            m = m.child(row_for(i, p, false));
        }
    } else {
        let q = query.text.clone();
        m = m.child(theme::focusable_row(row().key("atp-back").role("button").label("← Back")).radius(MENU_ROW_R).px(14.0).h(32.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut App| if let Some(Ctx { sub: Some(Sub::AddToPlaylist { all, .. }), .. }) = &mut s.nav.ctx { *all = false })
                .child(text("← Back").font(t(13.0, 500.0)).color(white(0.6)).a11y_hidden()))
            .child(row().mx(12.0).my(4.0).h(34.0).px(10.0).gap(8.0).radius(8.0).items_center().fill(white(0.06))
                .child(glyph(G::Search, 14.0, white(0.5)))
                .child(text_input(query, t(13.0, 500.0).lh(18.0)).key("atp-find").label("Find a playlist").single_line().grow(1.0).color(white(0.92)).placeholder("Find a playlist", white(0.4))
                    .on_edit(|s: &mut App| match &mut s.nav.ctx {
                        Some(Ctx { sub: Some(Sub::AddToPlaylist { query, .. }), .. }) => query,
                        _ => unreachable!("the find field only shows with its submenu"),
                    })));
        let hits: Vec<&Value> = pls.iter().filter(|p| data::matches(&data::name(p), &q)).collect();
        if hits.is_empty() {
            m = m.child(ui::empty(if pls.is_empty() { "No playlists yet" } else { "No playlists found" }).px(14.0).py(10.0));
        }
        let mut list = col().w_full().max_h(320.0).scroll(rune::ScrollSpec { stick_bottom: false });
        for (i, p) in hits.into_iter().enumerate() {
            list = list.child(row_for(100 + i, p, true));
        }
        m = m.child(list);
    }
    m
}

/// Ids requested but not added, keeping repeats: one removal per added id
/// (AddToPlaylistMenu.skippedIds). The backend returns the ids it ADDED.
fn skipped_ids(requested: &[u64], added: &[u64]) -> Vec<u64> {
    let mut pending = added.to_vec();
    requested.iter().copied().filter(|id| match pending.iter().position(|p| p == id) {
        Some(i) => {
            pending.remove(i);
            false
        }
        None => true,
    }).collect()
}

fn tracks_label(n: usize) -> String {
    if n == 1 { "1 track".into() } else { format!("{n} tracks") }
}

/// The toast after a first (skip-duplicates) add: the message, and the action offered when some
/// ids were skipped as already there.
fn add_result_toast(requested: usize, added: usize, skipped: usize, l: &str) -> (String, Option<&'static str>) {
    if skipped == 0 {
        (if requested > 1 { format!("Added {requested} tracks to \"{l}\"") } else { format!("Added to \"{l}\"") }, None)
    } else if added == 0 {
        (format!("Already in \"{l}\""), Some("Add anyway"))
    } else {
        (format!("Added {} to \"{l}\" · {skipped} already there", tracks_label(added)), Some("Add them too"))
    }
}

/// The toast after "Add anyway": `requested` is the size of the first request.
fn add_again_toast(requested: usize, skipped: usize, l: &str) -> String {
    if requested > 1 { format!("Added {} again to \"{l}\"", tracks_label(skipped)) } else { format!("Added to \"{l}\" again") }
}

/// Add tracks to a playlist (AddToPlaylistMenu.handleAddToPlaylist): skip duplicates first; offer
/// to add the skipped ones anyway. `again` is the size of the first request when this is that
/// "Add anyway".
fn add_to_playlist(uuid: String, label: String, ids: Vec<u64>, again: Option<usize>) {
    sone_host_shim::async_runtime::spawn(async move {
        let dupes = if again.is_some() { tauri_app_lib::tidal_api::OnDupes::Add } else { tauri_app_lib::tidal_api::OnDupes::Skip };
        let l = clip(&label, 23);
        let res = library::add_tracks_to_playlist(state(), uuid.clone(), ids.clone(), dupes).await;
        if res.is_ok() {
            data::forget(&Key::Playlist(uuid.clone()));
            data::forget(&Key::AllPlaylists);
        }
        let failed = || toast(if ids.len() > 1 { "Failed to add tracks".into() } else { "Failed to add track".into() }, Kind::Error);
        if let Some(requested) = again {
            match res {
                Ok(_) => toast(add_again_toast(requested, ids.len(), &l), Kind::Success),
                Err(_) => failed(),
            }
            return;
        }
        match res {
            Ok(added) => {
                push_recent_playlist(&uuid);
                let skipped = skipped_ids(&ids, &added);
                match add_result_toast(ids.len(), added.len(), skipped.len(), &l) {
                    (msg, None) => toast(msg, Kind::Success),
                    (msg, Some(act)) => push_toast(msg, Kind::Info, Duration::from_millis(6000),
                        Some((act.into(), ToastAction::AddAgain { playlist: uuid, label, ids: skipped, requested: ids.len() }))),
                }
            }
            Err(_) => failed(),
        }
    });
}

// ------------------------------------------------------------------ dialogs

/// A dialog titled `title` (also its accessible name), in the app's one dialog frame.
fn dialog_frame(key: &str, title: &str, close_label: &str, body: El, close: fn(&mut App)) -> El {
    ui::dialog_frame(&format!("{key}-scrim"), key, title, (&format!("{key}-x"), close_label), 440.0, None, Some(ui::dialog_title(title)), body, close)
}

fn confirm_delete(c: &Ctx) -> El {
    let Target::Media { item, .. } = &c.target else { return el() };
    let title = data::name(item);
    let uuid = data::data(item)["uuid"].as_str().unwrap_or("").to_string();
    let body = col().gap(18.0)
        .child(text(format!("Are you sure you want to delete \"{title}\"? This can't be undone.")).key("del-msg").font(t(14.0, 400.0)).color(white(0.75)).wrap())
        .child(row().gap(10.0).child(spacer())
            .child(ui::pill("del-cancel", "Cancel", None, ui::Pill::Secondary).on_press(|s: &mut App| close(s)))
            .child(ui::pill("del-ok", "Delete", None, ui::Pill::Danger).described_by("del-msg").on_press(move |s: &mut App| {
                close(s);
                if s.nav.page == Page::Playlist(uuid.clone()) {
                    // React replaces the current history entry with Home
                    *s.nav.page = Page::Home;
                }
                let (u, l) = (uuid.clone(), title.clone());
                sone_host_shim::async_runtime::spawn(async move {
                    match library::delete_playlist(state(), data::user_id(), u).await {
                        Ok(()) => {
                            data::forget(&Key::Folders);
                            data::forget(&Key::AllPlaylists);
                            toast(format!("Deleted \"{}\"", clip(&l, 28)), Kind::Success);
                        }
                        Err(_) => toast("Failed to delete playlist".into(), Kind::Error),
                    }
                });
            })));
    dialog_frame("del-dialog", "Delete playlist?", "Close", body, |s| {
        if let Some(c) = &mut s.nav.ctx {
            c.confirm_delete = false;
        }
    })
}

fn create_modal(cr: &Create) -> El {
    let saving = cr.saving;
    let body = col().gap(14.0)
        .child(row().h(44.0).px(12.0).radius(10.0).items_center().fill(white(0.06))
            .child(text_input(&cr.title, t(14.0, 500.0).lh(20.0)).key("cp-title").label("Title").single_line().grow(1.0).color(white(0.95)).placeholder("Title", white(0.4))
                .on_edit(|s: &mut App| &mut s.nav.create.as_mut().expect("the field shows with its dialog").title)))
        .child(col().gap(4.0)
            .child(row().h(88.0).px(12.0).py(10.0).radius(10.0).fill(white(0.06))
                .child(text_input(&cr.desc, t(13.0, 400.0).lh(18.0)).key("cp-desc").label("Write a description").described_by("cp-desc-count").grow(1.0).color(white(0.9)).placeholder("Write a description", white(0.4))
                    .on_edit(|s: &mut App| &mut s.nav.create.as_mut().expect("the field shows with its dialog").desc)
                    .on_input_change(|s: &mut App, _| if let Some(c) = &mut s.nav.create { cap_desc(&mut c.desc) })))
            .child(text(format!("{}/{DESC_MAX} characters", cr.desc.text.chars().count().min(DESC_MAX))).key("cp-desc-count").font(t(11.0, 400.0)).color(white(0.45))))
        .child(crate::coll::error_line(&cr.error))
        .child(theme::focusable(row().key("cp-public").role("switch").label("Make it public")).described_by("cp-public-note").checked(cr.public).gap(10.0).items_center().cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| if let Some(c) = &mut s.nav.create { c.public = !c.public })
            .child(col().grow(1.0).a11y_hidden()
                .child(text("Make it public").font(t(14.0, 600.0)).color(white(0.92)))
                .child(text("Your playlist will be visible on your Profile and accessible by anyone.").key("cp-public-note").font(t(12.0, 400.0)).color(white(0.5)).wrap()))
            .child(ui::switch_mark(cr.public)))
        .child(row().child(spacer()).child(
            ui::pill("cp-save", "Save", saving.then_some(G::Loader), ui::Pill::Primary).busy(saving).on_press(|s: &mut App| save_playlist(s))));
    dialog_frame("cp-dialog", "Create playlist", "Close", body, |s| s.nav.create = None)
}

fn save_playlist(s: &mut App) {
    let Some(c) = &mut s.nav.create else { return };
    let title = c.title.text.trim().to_string();
    if title.is_empty() || c.saving {
        return;
    }
    c.saving = true;
    let save_no = CREATE_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let (desc, public, ids) = (c.desc.text.trim().to_string(), c.public, c.ids.clone());
    sone_host_shim::async_runtime::spawn(async move {
        let access = if public { "PUBLIC" } else { "UNLISTED" };
        match library::create_playlist(state(), title.clone(), desc, access.into()).await {
            Ok(p) => {
                let uuid = serde_json::to_value(&p).ok().and_then(|v| data::data(&v)["uuid"].as_str().map(str::to_string)).unwrap_or_default();
                if !ids.is_empty() && !uuid.is_empty() {
                    let _ = library::add_tracks_to_playlist(state(), uuid.clone(), ids, tauri_app_lib::tidal_api::OnDupes::Add).await;
                }
                // Made from a folder's menu: into that folder (library.rs).
                if !uuid.is_empty() {
                    crate::library::created_playlist(uuid.clone());
                }
                data::forget(&Key::Folders);
                data::forget(&Key::AllPlaylists);
                if !uuid.is_empty() {
                    push_recent_playlist(&uuid);
                }
                *CREATED.lock().unwrap() = Some((save_no, Ok(())));
                toast(format!("Created playlist \"{}\"", clip(&title, 28)), Kind::Success);
            }
            // CreatePlaylistModal says so in the dialog, which can save again.
            Err(_) => *CREATED.lock().unwrap() = Some((save_no, Err("Failed to create playlist".into()))),
        }
        bump();
    });
}

/// Counts the create-playlist saves; the result of save `n` is `CREATED`'s `(n, …)`.
static CREATE_GEN: AtomicU64 = AtomicU64::new(0);
static CREATED: LazyLock<Mutex<Option<(u64, Result<(), String>)>>> = LazyLock::new(Default::default);

/// What the create dialog's save came to (from the poll, which owns the state): it closes once
/// its playlist exists, or says it failed and can save again. Only the latest save's answer
/// counts, and only while its dialog is still saving.
pub fn settle_create(s: &mut App) {
    let Some((save_no, res)) = CREATED.lock().unwrap().take() else { return };
    let Some(c) = &mut s.nav.create else { return };
    if !c.saving || save_no != CREATE_GEN.load(Ordering::Relaxed) {
        return;
    }
    match res {
        Ok(()) => s.nav.create = None,
        Err(e) => {
            c.saving = false;
            c.error = Some(e);
        }
    }
}

/// CreatePlaylistModal's description limit: an edit past it is refused.
const DESC_MAX: usize = 500;

/// The description as an edit left it, held to [`DESC_MAX`] characters (the caret at the end).
fn cap_desc(e: &mut TextEdit) {
    if e.text.chars().count() > DESC_MAX {
        let focused = e.focused;
        *e = TextEdit::new(&e.text.chars().take(DESC_MAX).collect::<String>());
        e.focused = focused;
    }
}

pub fn json_of_track(id: u64, title: &str, artist: &str) -> Value {
    json!({"id": id, "title": title, "artist": {"name": artist}})
}

#[cfg(test)]
mod add_to_playlist_tests {
    use super::*;

    #[test]
    fn skipped_is_what_was_not_added() {
        assert_eq!(skipped_ids(&[1, 2, 3], &[1, 2, 3]), Vec::<u64>::new());
        assert_eq!(skipped_ids(&[1, 2, 3], &[2]), vec![1, 3]);
        assert_eq!(skipped_ids(&[7], &[]), vec![7]);
        // repeats: one removal per added id
        assert_eq!(skipped_ids(&[5, 5, 6], &[5, 6]), vec![5]);
    }

    #[test]
    fn toasts_follow_the_react_menu() {
        // everything added: a plain success with no action
        assert_eq!(add_result_toast(1, 1, 0, "Mix"), ("Added to \"Mix\"".into(), None));
        assert_eq!(add_result_toast(3, 3, 0, "Mix"), ("Added 3 tracks to \"Mix\"".into(), None));
        // the real duplicate case
        assert_eq!(add_result_toast(1, 0, 1, "Mix"), ("Already in \"Mix\"".into(), Some("Add anyway")));
        assert_eq!(add_result_toast(3, 2, 1, "Mix"), ("Added 2 tracks to \"Mix\" · 1 already there".into(), Some("Add them too")));
        assert_eq!(add_again_toast(1, 1, "Mix"), "Added to \"Mix\" again");
        assert_eq!(add_again_toast(3, 1, "Mix"), "Added 1 track again to \"Mix\"");
    }
}
