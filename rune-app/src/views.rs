//! SONE's screens in Rune: window chrome, header (navigation + search), sidebar (library),
//! and the Home / Search / Album / Artist / Playlist pages.
//!
//! Accessible names follow the React build wherever the verifier's goldens show them, because
//! the verifier compares what's on screen through AT-SPI ("button: Favorite album", the
//! "Results for …" heading, …). Visual style is Afterglow (ui.rs), not a copy of SONE's.

use crate::controller::{self, Cmd, View};
use crate::data::{self, Key};
use crate::glyph::{G, glyph};
use crate::theme::{self, accent};
use crate::ui::{self, caps, t, white};
use crate::App;
pub use crate::settings::{SettingsTab, flush_settings, load_setting_fields, settings_modal, settings_page};
use rune::{El, ScrollSpec, TextEdit, Tracked, Tx, col, el, oklch, row, spacer, text, text_input};
use serde_json::{Value, json};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Home,
    Explore,
    Feed,
    Search(String),
    Album(u64),
    Artist(u64),
    Playlist(String),
    Settings,
    /// An Explore sub-page by api path, with the title it was opened under.
    ExploreSub(String, String),
    /// A mix or radio (React MixPage): id, title, subtitle.
    Mix(String, String, String),
    /// Loved tracks (React FavoritesView).
    Loved,
    /// The signed-in user's profile (React ProfilePage), and all its public playlists.
    Profile,
    ProfilePlaylists,
    /// An artist's popular tracks (React ArtistTracksPage): artist id and name.
    ArtistTracks(u64, String),
    /// A section's whole list (React ViewAllPage): title, api path, and the artist whose page
    /// it came from (paged through get_artist_view_all) or none (one get_page_section).
    ViewAll(String, String, Option<u64>),
    /// Your library's Show all (React LibraryViewAll): the kind, and for playlists the folder
    /// (id and name) or none for the top level.
    Library(crate::paged::Lib, Option<(String, String)>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drawer {
    Queue,
    Suggested,
    Lyrics,
    Credits,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoginMode {
    Pkce,
    Code,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LibTab {
    Playlists,
    Albums,
    Artists,
    Mixes,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SearchTab {
    All,
    TopHits,
    Tracks,
    Playlists,
    Albums,
    Artists,
    Videos,
}

/// The window's UI state. What the panes read is `Tracked`, so their regions (main::view)
/// build again when it's written, and only then; assign to it only when the value changes. The
/// search and filter fields aren't: Rune borrows a field's `TextEdit` mutably on every commit,
/// which would count as a write, so they're inputs of the regions that show them instead (a
/// `TextEdit` hashes its text, selection, composition and focus).
pub struct Nav {
    pub page: Tracked<Page>,
    pub back: Tracked<Vec<Page>>,
    pub fwd: Tracked<Vec<Page>>,
    pub search: TextEdit,
    pub lib: Tracked<LibTab>,
    pub tab: Tracked<SearchTab>,
    pub filter: TextEdit,
    pub queue_open: bool,
    /// When the debounced suggestions fetch for the current search text is due.
    pub suggest_due: Option<std::time::Instant>,
    pub menu_open: bool,
    pub settings_tab: SettingsTab,
    pub drawer: Tracked<Option<Drawer>>,
    pub login_mode: LoginMode,
    /// Settings is a modal over the current page (React SettingsModal).
    pub settings_open: bool,
    pub dialog: Option<Dialog>,
    /// The Settings modal's fields and what its commands reported (settings.rs), filled from
    /// the saved settings when Settings opens.
    pub set: crate::settings::Form,
    /// The open context menu (crate::interact), and the create-playlist dialog.
    pub ctx: Option<crate::interact::Ctx>,
    pub create: Option<crate::interact::Create>,
    /// Synced lyrics: the active line, and whether the user scrolled away from it.
    pub lyric_line: Tracked<Option<usize>>,
    pub lyrics_scrolled: Tracked<bool>,
    /// The search dropdown is open (typing opens it; a pick, Enter, Escape or a click
    /// elsewhere closes it), and the entry ↑/↓ highlighted.
    pub search_open: bool,
    pub search_sel: Option<usize>,
    /// The music video player (video_ui.rs).
    pub video: crate::video_ui::Player,
    /// The full-screen now-playing player (full_ui.rs).
    pub full: crate::full_ui::Full,
    /// Home: the feed tab ("static" is For you) and how many further pages of it to show.
    pub home_tab: Tracked<String>,
    pub home_pages: Tracked<usize>,
    /// The page content's width inside its padding (measured; Home sizes grids and cards by it).
    pub content_w: Tracked<f32>,
    /// The sign-in screen's PKCE field: the redirect URL pasted back from the browser.
    pub pkce_paste: TextEdit,
    /// Collections and the library: sorts, tabs, folder and playlist dialogs (coll.rs).
    pub coll: crate::coll::Coll,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dialog {
    Shortcuts,
    About,
    /// An artist's whole biography (ArtistPage's bio modal).
    Bio(u64),
}

impl Default for Nav {
    fn default() -> Self {
        Nav { page: Page::Home.into(), back: Tracked::default(), fwd: Tracked::default(), search: TextEdit::new(""), lib: LibTab::Playlists.into(), tab: SearchTab::All.into(), filter: TextEdit::new(""), queue_open: false, suggest_due: None, menu_open: false, settings_tab: SettingsTab::Playback, drawer: None.into(), login_mode: LoginMode::Pkce, settings_open: false, dialog: None,
            set: Default::default(), ctx: None, create: None, lyric_line: None.into(), lyrics_scrolled: false.into(), search_open: false, search_sel: None,
            video: Default::default(), full: Default::default(), home_tab: String::from("static").into(), home_pages: 0.into(), content_w: 0.0.into(), pkce_paste: TextEdit::new(""), coll: Default::default() }
    }
}

pub fn go(s: &mut App, p: Page) {
    if *s.nav.page != p {
        let old = std::mem::replace(&mut *s.nav.page, p);
        s.nav.back.push(old);
        s.nav.fwd.clear();
        // One filter field serves every playlist and Loved Tracks; it belongs to the page it
        // was typed on.
        s.nav.filter = TextEdit::new("");
    }
    if !matches!(*s.nav.page, Page::Search(_)) {
        s.nav.search.focused = false;
    }
    if s.nav.page == Page::Feed {
        let h = crate::host().clone();
        sone_host_shim::async_runtime::spawn(async move {
            use sone_host_shim::Manager;
            let _ = tauri_app_lib::commands::feed::mark_feed_seen(h.state::<tauri_app_lib::AppState>(), data::user_id()).await;
        });
    }
}

// ------------------------------------------------------------------ small pieces

/// [`ui::pill`], primary or secondary.
pub(crate) fn button<S: 'static>(key: &str, label: &str, g: Option<G>, primary: bool) -> El {
    ui::pill(key, label, g, if primary { ui::Pill::Primary } else { ui::Pill::Secondary })
}

/// A page-header action (AlbumView, PlaylistView, ArtistPage): a 22 pt icon over an 11 pt
/// caption. `label` is the accessible name (React's title), `caption` what shows under it.
pub(crate) fn header_btn(key: &str, label: &str, caption: &str, g: G, on: bool) -> El {
    theme::focusable(col().key(key.to_string()).role("button").label(label))
        .w(52.0).py(4.0).gap(3.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer).help(label)
        .child(glyph(g, 22.0, if on { accent() } else { white(0.8) }))
        .child(text(caption.to_string()).font(t(11.0, 500.0)).color(white(0.6)).a11y_hidden())
}

pub(crate) fn icon<S: 'static>(key: &str, label: &str, g: G) -> El {
    ui::icon_button::<S>(key, label, g, 18.0, white(0.75))
}

pub(crate) fn heading(label: &str, size: f32) -> El {
    text(label.to_string()).font(t(size, 650.0)).color(white(0.95)).role("heading").label(label)
}

/// A clickable catalogue card: art, a heading with its name, a subtitle. The card itself is the
/// press target (AT-SPI clicks reach it through the heading, as with the React cards).
///
/// With its `item` (MediaCard.tsx): right-click opens the media menu at the pointer; hovering
/// shows a play button (bottom-left, accent, 40 pt) and a heart (bottom-right, 32 pt), or for an
/// artist a centred 48 pt play button ("Play <name>", "Add <name> to library"; React's are
/// unnamed). Tab reaches the card through its heading, and the ring goes round the whole card.
pub(crate) fn card(key: String, title: &str, sub: &str, seed: u64, round: bool, img: Option<String>, item: Option<Value>, go_to: Page) -> El {
    card_w(148.0, key, title, sub, seed, round, img, item, go_to)
}

/// [`card`] `w` points wide (Home sizes its cards to the row, as React's card-scroll does).
pub(crate) fn card_w(w: f32, key: String, title: &str, sub: &str, seed: u64, round: bool, img: Option<String>, item: Option<Value>, go_to: Page) -> El {
    let a = w - 16.0;
    use crate::interact::{self, Anchor, Fav, Media, Say};
    let h = ui::hue(Some(seed));
    let mut art = if round { ui::with_image(ui::art(h, a, seed, 0.0), img, a, a / 2.0) } else { ui::with_image(ui::tile(h, a, seed, 10.0), img, a, 10.0) };
    let kind = item.as_ref().and_then(Media::of);
    let card_flags = item.as_ref().map(|it| {
        let d = data::data(it);
        (d["explicit"].as_bool() == Some(true), d["ai"].as_bool() == Some(true))
    });
    let ai_item = card_flags.is_some_and(|f| f.1);
    if let (Some(it), Some(kind)) = (item.clone(), kind) {
        let play_it = it.clone();
        let play_label = format!("Play {}", data::name(&it));
        if kind == Media::Artist {
            art = art.child(el().absolute(a / 2.0 - 24.0, a / 2.0 - 24.0).key(format!("{key}-play")).role("button").label(play_label).size(48.0, 48.0).radius(24.0).fill(theme::black(0.45))
                .items_center().justify_center().group_opacity(0.0, 1.0).cursor(rune::Cursor::Pointer)
                .on_press(move |_: &mut App| if !(ai_item && !crate::content::prefs().1) { interact::play_media(&play_it, kind) })
                .child(glyph(G::Play, 28.0, white(1.0))));
        } else {
            let fav_kind = match kind { Media::Album => Fav::Album, Media::Playlist => Fav::Playlist, Media::Mix => Fav::Mix, Media::Artist => Fav::Artist };
            let d = data::data(&it);
            let id = if kind == Media::Playlist { d["uuid"].as_str().unwrap_or("").to_string() } else { d["id"].as_str().map(str::to_string).unwrap_or_else(|| d["id"].to_string()) };
            let on = interact::is_fav(fav_kind, &id);
            let label = data::name(&it);
            let fav_label = if on { format!("Remove {label} from library") } else { format!("Add {label} to library") };
            art = art
                .child(el().absolute(10.0, a - 50.0).key(format!("{key}-play")).role("button").label(play_label).size(40.0, 40.0).radius(20.0).fill(accent())
                    .items_center().justify_center().group_opacity(0.0, 1.0).press_scale(0.94).cursor(rune::Cursor::Pointer)
                    .on_press(move |_: &mut App| if !(ai_item && !crate::content::prefs().1) { interact::play_media(&play_it, kind) })
                    .child(glyph(G::Play, 20.0, theme::ink())))
                .child(el().absolute(a - 42.0, a - 42.0).key(format!("{key}-fav")).role("button").label(fav_label).size(32.0, 32.0).radius(16.0).fill(theme::black(0.5))
                    .items_center().justify_center().group_opacity(0.0, 1.0).cursor(rune::Cursor::Pointer)
                    .on_press(move |_: &mut App| interact::toggle_fav(fav_kind, id.clone(), label.clone(), Say::Nothing))
                    .child(glyph(if on { G::HeartFilled } else { G::Heart }, 16.0, if on { accent() } else { white(0.95) })));
        }
    }
    // The card adds no accessible node of its own (as in the React build, where only its
    // heading is exposed); the heading carries the press so assistive tech can open it.
    let to = go_to.clone();
    let mut c = col()
        .key(key)
        .role("presentation")
        .w(w)
        .gap(6.0)
        .padding(8.0)
        .radius(12.0)
        .hover(white(0.06))
        .hover_group()
        .focus_within_ring(2.0, -2.0, white(0.6))
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| go(s, go_to.clone()));
    if let Some(it) = item.filter(|_| kind.is_some()) {
        c = c.on_secondary_press(move |s: &mut App, p, _| interact::open_media(s, it.clone(), Anchor::Point(p.x, p.y)));
    }
    // MediaCard: the explicit and AI badges under the art; with AI content off an AI item dims
    // and its play button does nothing.
    let heading_el = heading(title, 13.0).truncate().focusable(true).no_focus_ring().on_press(move |s: &mut App| go(s, to.clone()));
    let (flag_e, flag_ai) = card_flags.unwrap_or((false, false));
    let head = match crate::content::badges(flag_e, flag_ai) {
        Some(b) => row().w_full().min_w(0.0).gap(6.0).items_center().child(heading_el.shrink()).child(b),
        None => heading_el,
    };
    if flag_ai && !crate::content::prefs().1 {
        c = c.opacity(0.5);
    }
    c.child(art)
        .child(head)
        .child(text(sub.to_string()).font(t(11.0, 400.0)).color(white(0.55)).truncate())
}

thread_local! {
    /// The track playing and whether it plays, as the last view build read it (`set_now_playing`).
    static NOW_PLAYING: std::cell::Cell<Option<(Option<u64>, bool)>> = const { std::cell::Cell::new(None) };
}

/// What track rows mark as playing until the next view build: the reading the panes' regions
/// were keyed by, so rows built later in the commit (or as the list scrolls) agree with them.
pub fn set_now_playing(now: (Option<u64>, bool)) {
    NOW_PLAYING.set(Some(now));
}

/// A track row with the row actions the React rows expose.
pub(crate) fn track_row(key: String, n: usize, tr: &Value, on_play: impl Fn(&mut App) + 'static) -> El {
    track_row_with(key, n, tr, true, None, on_play)
}

/// A track row (TrackList.tsx). A single click plays it (there's no double-click play in React
/// either); hovering swaps the number for a play icon; right-click opens the track menu at the
/// pointer. Its actions, after the time: More options (shown on hover; the same menu, below it),
/// Add to playlist (the playlist picker, below it) and the heart (toggled optimistically, filled
/// in the accent when loved). `pl` places it in the user's own playlist, for "Remove from
/// playlist".
pub(crate) fn track_row_with(key: String, n: usize, tr: &Value, actions: bool, pl: Option<crate::interact::InPlaylist>, on_play: impl Fn(&mut App) + 'static) -> El {
    use crate::interact::{self, Anchor, Fav, Say};
    let d = data::data(tr);
    let title = d["title"].as_str().unwrap_or("").to_string();
    let dur = d["duration"].as_u64().unwrap_or(0);
    let id = d["id"].as_u64().unwrap_or(0);
    // The React TrackList: the playing row shows the bouncing bars in place of its number, and
    // the current track's number and title take the accent colour.
    let (current_id, playing) = NOW_PLAYING.get().unwrap_or_else(controller::now_playing);
    let active = current_id.is_some() && d["id"].as_u64() == current_id;
    let accent_c = oklch(0.82, 0.11, ui::hue(current_id));
    let number: El = if active && playing {
        el().w(22.0).child(ui::eq(true, &key))
    } else {
        el().w(22.0).h(20.0).justify_center()
            .child(text(format!("{n}")).font(ui::tnum(t(12.0, 500.0))).color(if active { accent_c } else { white(0.45) }).group_opacity(1.0, 0.0))
            .child(el().absolute(2.0, 3.0).group_opacity(0.0, 1.0).child(glyph(G::Play, 14.0, white(0.92))))
    };
    let (menu_tr, menu_pl) = (tr.clone(), pl.clone());
    // TrackList: a track the content filter blocks dims and does nothing (no play, no menu); an
    // unavailable one (unless it's the one playing) dims and says so.
    let blocked = crate::content::blocked(tr);
    let unavailable = !active && d["itemType"].as_str() != Some("video") && crate::content::unavailable(d);
    let mut title_row = row().min_w(0.0).gap(6.0).items_center()
        .child(text(title.clone()).font(t(14.0, 500.0)).color(if active { accent_c } else { white(0.95) }).truncate().shrink());
    // TrackList: a music video in a playlist says so before its other badges
    if d["itemType"].as_str() == Some("video") {
        title_row = title_row.child(crate::content::video_badge());
    }
    if let Some(b) = crate::content::badges_for(tr) {
        title_row = title_row.child(b);
    }
    // Rows aren't accessible nodes in the React build either; their action buttons are.
    let mut r = row()
        .key(key.clone())
        .role("presentation")
        .w_full()
        .h(48.0)
        .px(12.0)
        .gap(12.0)
        .radius(8.0)
        .items_center()
        .hover_group();
    r = if blocked {
        r.opacity(0.4)
    } else if unavailable {
        r.opacity(0.4).on_press(|_: &mut App| interact::toast("Track unavailable".into(), interact::Kind::Info))
            .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, menu_tr.clone(), menu_pl.clone(), Anchor::Point(p.x, p.y)))
    } else {
        r.hover(white(0.05)).cursor(rune::Cursor::Pointer).on_press(on_play)
            .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, menu_tr.clone(), menu_pl.clone(), Anchor::Point(p.x, p.y)))
    };
    let mut r = r
        .child(number)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(title_row)
                .child(text(data::artist_of(tr)).font(t(12.0, 400.0)).color(white(0.55)).truncate()),
        );
    // TrackList's album column, a link to the album (not on the album's own page).
    if let (Some(album_id), Some(album)) = (d["album"]["id"].as_u64(), d["album"]["title"].as_str().filter(|a| !a.is_empty()))
        && actions
        && !key.starts_with("al-t")
    {
        r = r.child(row().w(220.0).shrink().min_w(0.0).child(
            text(album.to_string()).font(t(12.0, 400.0)).color(white(0.55)).truncate()
                .key(format!("{key}-album")).role("link").label(album).focusable(true).focus_ring(2.0, 2.0, white(0.6))
                .cursor(rune::Cursor::Pointer).on_press(move |s: &mut App| go(s, Page::Album(album_id)))));
    }
    r = r.child(text(ui::fmt_time(dur as f64)).font(ui::tnum(t(12.0, 400.0))).color(white(0.55)).w(48.0));
    if actions {
        // TrackList: a video's heart (a playlist can hold videos) favourites the video.
        let fav = if d["itemType"].as_str() == Some("video") { Fav::Video } else { Fav::Track };
        let loved = interact::is_fav(fav, &id.to_string());
        let (more_key, add_key) = (format!("{key}-more"), format!("{key}-add"));
        let (t1, t2, mk, ak) = (tr.clone(), tr.clone(), more_key.clone(), add_key.clone());
        r = r
            // Only the glyph fades with the row's hover (the button has no hover of its own, so
            // the row is its group): the focus ring stays whole, and the glyph rests faint rather
            // than gone, so Tab never lands on an invisible button. (Rune has no focus-driven
            // opacity to reveal it on focus alone.)
            .child(theme::focusable(el().key(more_key.clone()).role("button").label("More options")).size(36.0, 36.0).radius(18.0).items_center().justify_center()
                .press(white(0.14)).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| interact::open_track(s, t1.clone(), pl.clone(), Anchor::Below(mk.clone())))
                .child(glyph(G::More, 18.0, white(0.75)).group_opacity(0.3, 1.0)))
            .child(icon::<App>(&add_key, "Add to playlist", G::Plus).on_press(move |s: &mut App| interact::open_add_below(s, t2.clone(), ak.clone())))
            .child(ui::icon_button::<App>(&format!("{key}-fav"), if loved { "Remove from favorites" } else { "Add to favorites" }, if loved { G::HeartFilled } else { G::Heart }, 18.0, if loved { accent() } else { white(0.75) })
                .on_press(move |_: &mut App| interact::toggle_fav(fav, id.to_string(), title.clone(), Say::Nothing)));
    }
    r
}

pub(crate) fn grid(cards: Vec<El>) -> El {
    let mut r = row().w_full().gap(8.0).flex_wrap();
    for c in cards {
        r = r.child(c);
    }
    r
}

/// A track list whose rows are built only while they're in or near view (`El::virtual_rows`:
/// the scroll viewport's rows and up to two viewports either side), so a list of thousands
/// costs what its visible rows do. It lists the tracks at `shown` (indices into `tracks`, a
/// filter's matches) in order; track `i`'s row is keyed `{prefix}{i}`, as it always was, and
/// `row(tracks, i)` builds it. Its rows land at once, as rows of a plain column did: a growth
/// stack's rows would grow in and fold out on a spring.
pub(crate) fn track_list<P: std::ops::Deref<Target = Vec<Value>> + 'static>(prefix: &'static str, tracks: P, shown: Vec<usize>, row: impl Fn(&P, usize) -> El + 'static) -> El {
    let shown = Rc::new(shown);
    // The keys change only with the rows shown, so layout needn't ask for every key each commit.
    let keys_version = {
        use std::hash::{BuildHasher, BuildHasherDefault};
        BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(&*shown)
    };
    let keys = shown.clone();
    col().key(format!("{prefix}rows")).w_full().gap(2.0).growth(rune::GrowthSpec::default()).instant_changes()
        .virtual_rows(shown.len(), 48.0, move |_: &App, k| format!("{prefix}{}", keys[k]).into(), move |_: &App, k| row(&tracks, shown[k]))
        .virtual_rows_version(keys_version)
}

pub(crate) fn item_page(item: &Value) -> Option<Page> {
    let d = data::data(item);
    match data::kind(item).as_str() {
        "ALBUM" => d["id"].as_u64().map(Page::Album),
        "ARTIST" => d["id"].as_u64().map(Page::Artist),
        "PLAYLIST" => d["uuid"].as_str().map(|u| Page::Playlist(u.to_string())),
        // MixPage, titled and subtitled as the card is
        "MIX" => d["id"].as_str().or(d["mixId"].as_str()).map(|id| Page::Mix(id.to_string(), data::name(item), data::artist_of(item))),
        _ => None,
    }
}

pub(crate) fn cards_for(prefix: &str, items: &[Value]) -> Vec<El> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| {
            let page = item_page(it)?;
            let round = data::kind(it) == "ARTIST";
            let seed = data::data(it)["id"].as_u64().unwrap_or(i as u64 + 1);
            Some(card(format!("{prefix}-{i}"), &data::name(it), &data::artist_of(it), seed, round, crate::art::item_url(it, 132.0), Some(it.clone()), page))
        })
        .collect()
}

thread_local! {
    /// A `loading()` placeholder was built since `took_loading` last looked.
    static LOADING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn loading() -> El {
    LOADING.set(true);
    text("Loading…").font(t(13.0, 500.0)).color(white(0.45))
}

/// Whether a `loading()` placeholder was built since the last call: a pane that built its
/// content between two calls is still loading, and tells assistive technology so (`El::busy`).
pub(crate) fn took_loading() -> bool {
    LOADING.replace(false)
}

/// A page's data, or what shows until it's there: `loading()`, or, when the fetch failed, a
/// short error with Retry (which forgets the entry, so the redraw fetches it again).
pub(crate) fn ready_or(key: Key) -> Result<Value, El> {
    match data::want(key.clone()) {
        data::Load::Ready(v) => Ok(v),
        data::Load::Loading => Err(loading()),
        data::Load::Failed(_) => Err(col().gap(10.0).items_start()
            .child(text("Couldn't load this").font(t(13.0, 500.0)).color(white(0.6)))
            .child(button::<App>(&format!("retry-{key:?}"), "Retry", None, false).on_press(move |_: &mut App| data::forget(&key)))),
    }
}

// ------------------------------------------------------------------ chrome

/// How the window's width shapes the shell. `main::view` reads it off the root's laid-out
/// width, so the view re-runs only when a resize crosses one of these thresholds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shell {
    pub sidebar_w: f32,
    /// The drawer takes a column of its own; otherwise it slides over the page from the right.
    pub drawer_docked: bool,
    pub drawer_w: f32,
    /// Room in the player bar for the "playing from" source and the quality badge.
    pub bar_source: bool,
    pub bar_badge: bool,
}

impl Shell {
    pub fn for_width(w: f32) -> Shell {
        // Below the window's 900 pt minimum (a tiling window manager can force that) the
        // sidebar narrows; the page always keeps at least ~520 pt beside a docked drawer.
        let sidebar_w = if w >= 860.0 { 280.0 } else { 232.0 };
        let drawer_docked = w >= sidebar_w + 360.0 + 520.0;
        let drawer_w = if drawer_docked { 360.0 } else { (w - sidebar_w - 48.0).clamp(240.0, 360.0) };
        Shell { sidebar_w, drawer_docked, drawer_w, bar_source: w >= 1300.0, bar_badge: w >= 1150.0 }
    }
}

// (The window buttons are the title bar's: shell::title_bar.)

pub fn header(s: &App) -> El {
    let field = text_input(&s.nav.search, t(14.0, 500.0).lh(20.0))
        .key("search")
        .label("Search")
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .color(white(0.97))
        .caret(white(0.95), white(0.25))
        .placeholder("Search", white(0.4))
        .on_edit(|s: &mut App| &mut s.nav.search)
        .on_input_change(|s: &mut App, _| {
            // SearchBar debounces suggestions by 300 ms (SearchBar.tsx:112-125).
            s.nav.suggest_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(300));
            s.nav.search_open = true;
            s.nav.search_sel = None;
        })
        .on_input_key(|s: &mut App, ev, tx| search_key(s, ev, tx))
        .on_submit(|s: &mut App, _| {
            let q = s.nav.search.text.trim().to_string();
            if !q.is_empty() {
                s.nav.search_open = false;
                *s.nav.tab = SearchTab::All;
                go(s, Page::Search(q));
            }
        });
    // 360 pt wide when there's room; in a narrow window it gives way before anything else does.
    let bar = row().key("search-bar").role("search").w(360.0).shrink().min_w(140.0).h(40.0).px(14.0).gap(8.0).radius(20.0).items_center().fill(white(0.08)).child(glyph(G::Search, 16.0, white(0.55))).child(field);
    // Back and Forward, dimmed and inert with no history that way.
    let step = |key: &str, label: &str, g: G, on: bool, f: fn(&mut App)| {
        let b = ui::icon_button::<App>(key, label, g, 18.0, white(if on { 0.75 } else { 0.25 }));
        if on { b.on_press(f) } else { ui::unavailable(b) }
    };
    row()
        .key("header")
        .role("banner")
        .w_full()
        .h(60.0)
        .px(20.0)
        .gap(10.0)
        .items_center()
        .child(step("back", "Go back", G::Back, !s.nav.back.is_empty(), |s| {
            if let Some(p) = s.nav.back.pop() {
                let cur = std::mem::replace(&mut *s.nav.page, p);
                s.nav.fwd.push(cur);
                s.nav.filter = TextEdit::new("");
            }
        }))
        .child(step("forward", "Go forward", G::Forward, !s.nav.fwd.is_empty(), |s| {
            if let Some(p) = s.nav.fwd.pop() {
                let cur = std::mem::replace(&mut *s.nav.page, p);
                s.nav.back.push(cur);
                s.nav.filter = TextEdit::new("");
            }
        }))
        .child(spacer())
        .child(bar)
        .child(icon::<App>("account", "Account", G::User).on_press(|s: &mut App| {
            s.nav.menu_open = !s.nav.menu_open;
            if s.nav.menu_open {
                data::want(Key::Profile);
            }
        }))
}

/// The suggestions dropdown under the search field while typing (React SearchBar).
/// One entry of the search dropdown, in display order.
#[derive(Clone, Debug)]
enum Entry {
    /// A text suggestion: fills the search bar and searches.
    Text(String),
    /// A direct hit: an album, artist or playlist page.
    Hit(String, Page, HitLook),
    /// A direct hit on a track: plays it.
    Track(String, u64, HitLook),
    ViewAll,
}

/// How a direct hit shows in the dropdown: what it is ("Album · <artist>"), its picture, and
/// whether that's round (an artist).
#[derive(Clone, Debug)]
struct HitLook {
    sub: String,
    img: Option<String>,
    round: bool,
    seed: u64,
}

fn dropdown_entries(q: &str) -> Vec<Entry> {
    let Some(v) = data::peek(&Key::Suggest(q.to_string())) else { return vec![] };
    let mut out: Vec<Entry> = v["textSuggestions"].as_array().cloned().unwrap_or_default().iter()
        .filter_map(|x| x["query"].as_str().map(|q| Entry::Text(q.to_string()))).collect();
    for hit in v["directHits"].as_array().cloned().unwrap_or_default() {
        let title = hit["title"].as_str().or(hit["name"].as_str()).unwrap_or("").to_string();
        let kind = hit["hitType"].as_str().unwrap_or("");
        let artist = hit["artistName"].as_str().or(hit["track"]["artists"][0]["name"].as_str()).unwrap_or("");
        let with_artist = |k: &str| if artist.is_empty() { k.to_string() } else { format!("{k} · {artist}") };
        let look = HitLook {
            sub: match kind { "TRACKS" => with_artist("Track"), "ALBUMS" => with_artist("Album"), "ARTISTS" => "Artist".into(), _ => "Playlist".into() },
            img: crate::art::item_url(&hit, 36.0).or_else(|| crate::art::cover(hit["albumCover"].as_str(), 36.0)),
            round: kind == "ARTISTS",
            seed: hit["id"].as_u64().unwrap_or(title.len() as u64 + 1),
        };
        let page = match kind {
            "TRACKS" => {
                out.push(Entry::Track(title, hit["id"].as_u64().unwrap_or(0), look));
                continue;
            }
            "ALBUMS" => hit["id"].as_u64().map(Page::Album),
            "ARTISTS" => hit["id"].as_u64().map(Page::Artist),
            "PLAYLISTS" => hit["uuid"].as_str().map(|u| Page::Playlist(u.to_string())),
            _ => None,
        };
        if let Some(p) = page {
            out.push(Entry::Hit(title, p, look));
        }
    }
    out.push(Entry::ViewAll);
    out
}

/// Set the search bar to `text`, caret at the end, keeping focus if it had it.
fn set_search_text(s: &mut App, text: &str, focused: bool) {
    let mut e = TextEdit::new(text);
    e.focused = focused;
    s.nav.search = e;
}

fn pick(s: &mut App, e: Entry) {
    s.nav.search_open = false;
    s.nav.search_sel = None;
    match e {
        Entry::Text(q) => {
            set_search_text(s, &q, false);
            *s.nav.tab = SearchTab::All;
            go(s, Page::Search(q));
        }
        Entry::Hit(_, p, _) => go(s, p),
        // SearchBar's play button: playSingle, which keeps the manual queue.
        Entry::Track(_, id, _) => controller::send(Cmd::PlaySingleId(id)),
        Entry::ViewAll => {
            let q = s.nav.search.text.trim().to_string();
            *s.nav.tab = SearchTab::All;
            go(s, Page::Search(q));
        }
    }
}

/// Keys in the search field: ↑/↓ move through the dropdown, Tab completes the text to the
/// highlighted (or first) suggestion, Enter opens the highlighted entry (or searches, through
/// `on_submit`), Escape closes the dropdown (SearchBar.tsx has Enter and Escape; arrows and Tab
/// go further).
fn search_key(s: &mut App, ev: &rune::KeyEvent, tx: &mut Tx) -> bool {
    // bound Ctrl and Alt shortcuts run from the search field too (keys.rs)
    if crate::keys::field_key(s, ev, tx) {
        return true;
    }
    let q = s.nav.search.text.trim().to_string();
    let entries = if s.nav.search_open && !q.is_empty() { dropdown_entries(&q) } else { vec![] };
    match &ev.key {
        rune::Key::Down | rune::Key::Up if !entries.is_empty() => {
            let n = entries.len();
            s.nav.search_sel = Some(match (s.nav.search_sel, ev.key == rune::Key::Down) {
                (None, true) => 0,
                (None, false) => n - 1,
                (Some(i), true) => (i + 1) % n,
                (Some(i), false) => (i + n - 1) % n,
            });
            true
        }
        rune::Key::Tab if !ev.mods.shift => {
            let target = s.nav.search_sel.and_then(|i| entries.get(i)).and_then(|e| match e {
                Entry::Text(t) | Entry::Hit(t, ..) | Entry::Track(t, ..) => Some(t.clone()),
                Entry::ViewAll => None,
            }).or_else(|| entries.iter().find_map(|e| if let Entry::Text(t) = e { Some(t.clone()) } else { None }));
            match target {
                Some(t) if t != s.nav.search.text => {
                    set_search_text(s, &t, true);
                    s.nav.search_sel = None;
                    s.nav.search_open = true;
                    s.nav.suggest_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(300));
                    true
                }
                _ => false,
            }
        }
        rune::Key::Enter => match s.nav.search_sel.and_then(|i| entries.get(i)).cloned() {
            Some(e) => {
                pick(s, e);
                true
            }
            None => false,
        },
        rune::Key::Escape if s.nav.search_open => {
            s.nav.search_open = false;
            s.nav.search_sel = None;
            true
        }
        _ => false,
    }
}

pub fn dropdown(s: &App) -> Option<El> {
    let q = s.nav.search.text.trim().to_string();
    // Open while typing, and while a press on it is landing (its rows keep the field's focus);
    // closed by a pick, Enter, Escape, or focus leaving the field.
    if q.is_empty() || !s.nav.search_open || !s.nav.search.focused {
        return None;
    }
    // Peek only: the debounce in App::poll decides when to fetch (rendering mid-typing must not).
    data::peek(&Key::Suggest(q.clone()))?;
    let entries = dropdown_entries(&q);
    let sel = s.nav.search_sel;
    let mut list = col().w_full().padding(8.0).gap(2.0);
    let mut hit_n = 0;
    let mut sug_n = 0;
    for (i, e) in entries.iter().enumerate() {
        let on = Some(i) == sel;
        let ec = e.clone();
        let base = |key: String, label: &str, h: f32| row().key(key).role("button").label(label).h(h).px(10.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center()
            .fill(if on { white(0.1) } else { rune::Color::TRANSPARENT }).hover(white(0.08)).cursor(rune::Cursor::Pointer)
            .preserve_focus_on_press(true);
        let el_ = match e {
            Entry::Text(t) => {
                sug_n += 1;
                base(format!("sug-{}", sug_n - 1), t, 34.0)
                    .on_press(move |s: &mut App| pick(s, ec.clone()))
                    .child(glyph(G::Search, 14.0, white(0.5)))
                    .child(text(t.clone()).font(t_(13.0, 500.0)).color(white(0.9)).a11y_hidden())
            }
            Entry::Track(t, id, look) => {
                hit_n += 1;
                let (tk, mk) = (format!("hit-{}", hit_n - 1), format!("hit-{}-more", hit_n - 1));
                let track = crate::interact::json_of_track(*id, t, "");
                row().key(tk).h(48.0).px(10.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center().fill(if on { white(0.1) } else { rune::Color::TRANSPARENT })
                    .child(row().role("button").label(t.as_str()).grow(1.0).min_w(0.0).gap(10.0).items_center().cursor(rune::Cursor::Pointer).preserve_focus_on_press(true)
                        .on_press(move |s: &mut App| pick(s, ec.clone()))
                        .child(hit_thumb(look))
                        .child(col().min_w(0.0).gap(2.0)
                            .child(text(t.clone()).font(t_(13.0, 600.0)).color(white(0.95)).truncate().a11y_hidden())
                            .child(hit_sub(look))))
                    .child(icon::<App>(&mk, "More options", G::More).preserve_focus_on_press(true)
                        .on_press(move |s: &mut App| crate::interact::open_track(s, track.clone(), None, crate::interact::Anchor::Below(mk.clone()))))
            }
            Entry::Hit(t, _, look) => {
                hit_n += 1;
                row().key(format!("hit-{}", hit_n - 1)).role("group").h(48.0).px(10.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center().cursor(rune::Cursor::Pointer)
                    .fill(if on { white(0.1) } else { rune::Color::TRANSPARENT }).hover(white(0.08)).preserve_focus_on_press(true)
                    .on_press(move |s: &mut App| pick(s, ec.clone()))
                    .child(hit_thumb(look))
                    .child(col().min_w(0.0).gap(2.0).child(heading(t, 13.0).truncate()).child(hit_sub(look)))
            }
            Entry::ViewAll => base("view-all".into(), "View all results", 36.0)
                .on_press(move |s: &mut App| pick(s, ec.clone()))
                .child(text("View all results").font(t_(13.0, 600.0)).color(white(0.85)).a11y_hidden()),
        };
        list = list.child(el_);
    }
    // SearchBar.tsx: 420 px wide, right-aligned 8 px under the search field, at most 70vh tall
    // and scrolling. `anchored_fit` sizes it to the room below (or above) the field.
    Some(
        theme::overlay_surface(col().key("suggest").w(420.0).anchored_fit("search-bar", rune::Side::Bottom, rune::Align::End, 8.0), theme::Overlay::Menu)
            .clip_rounded(theme::RADIUS_OVERLAY)
            .child(col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list)),
    )
}

/// A direct hit's picture in the dropdown: round for an artist. Decorative.
fn hit_thumb(l: &HitLook) -> El {
    let h = ui::hue(Some(l.seed));
    if l.round {
        ui::with_image(ui::art(h, 36.0, l.seed, 0.0), l.img.clone(), 36.0, 18.0)
    } else {
        ui::with_image(ui::tile(h, 36.0, l.seed, 4.0), l.img.clone(), 36.0, 4.0)
    }
}

/// A direct hit's kind and artist under its title. Decorative: the row keeps its name.
fn hit_sub(l: &HitLook) -> El {
    text(l.sub.clone()).font(t_(11.0, 400.0)).color(white(theme::TEXT_MUTED)).truncate().a11y_hidden()
}

fn t_(size: f32, weight: f32) -> rune::TextStyle {
    t(size, weight)
}

/// The collapsed sidebar's width: its icons and covers only.
pub const SIDEBAR_COLLAPSED: f32 = 64.0;

pub fn sidebar(s: &App, w: f32) -> El {
    let nav_btn = |key: &str, label: &str, g: G, page: Page| {
        let on = s.nav.page == page;
        theme::focusable_row(row().key(key.to_string()).role("button").label(label)).h(40.0).px(12.0).gap(12.0).radius(10.0).items_center()
            .fill(if on { white(0.08) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer).help(label)
            .on_press(move |s: &mut App| go(s, page.clone()))
            .child(glyph(g, 18.0, white(if on { 0.95 } else { 0.7 })))
            .child(text(label.to_string()).font(t(14.0, 600.0)).color(white(if on { 0.95 } else { 0.75 })).a11y_hidden())
    };
    let tab = |key: &str, label: &str, which: LibTab| {
        let on = s.nav.lib == which;
        theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(28.0).px(10.0).radius(14.0).items_center()
            .fill(if on { white(0.16) } else { white(0.06) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                *s.nav.lib = which;
            })
            .child(text(label.to_string()).font(t(12.0, 600.0)).color(white(0.9)).a11y_hidden())
    };
    // "Your Library" collapses the sidebar to its icons and expands it again (React Sidebar).
    let lib_toggle = |collapsed: bool| row().key("lib").role("button").label("Your Library").shrink().h(34.0).px(8.0).gap(10.0).items_center().cursor(rune::Cursor::Pointer)
        .hover(white(0.06)).radius(8.0).focusable(true)
        .on_press(|s: &mut App| {
            let c = !s.nav.coll.ui.collapsed;
            s.nav.coll.ui.collapsed = c;
        })
        .child(glyph(G::Library, 18.0, white(0.75)))
        .child(if collapsed { el() } else { text("Your Library").font(t(14.0, 650.0)).color(white(0.9)).truncate().a11y_hidden() });
    if w <= SIDEBAR_COLLAPSED {
        let icon_btn = |key: &str, label: &str, g: G, page: Page| {
            let on = s.nav.page == page;
            theme::focusable(el().key(key.to_string()).role("button").label(label)).size(40.0, 40.0).radius(10.0).items_center().justify_center()
                .fill(if on { white(0.08) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer).help(label)
                .on_press(move |s: &mut App| go(s, page.clone()))
                .child(glyph(g, 18.0, white(if on { 0.95 } else { 0.7 })))
        };
        return col().key("sidebar").role("navigation").w(w).h_full().py(12.0).px(8.0).gap(6.0).items_center().fill(theme::sidebar_bg())
            .child(icon_btn("nav-home", "Home", G::Home, Page::Home))
            .child(icon_btn("nav-explore", "Explore", G::Compass, Page::Explore))
            .child(icon_btn("nav-feed", "Feed", G::Bell, Page::Feed))
            .child(el().h(10.0))
            .child(lib_toggle(true))
            .child(ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(crate::library::sidebar_list(s)), "lib-scroll".into()).labelled_by("lib"));
    }
    let mut tabs = row().gap(6.0).focus_group().child(tab("tab-pl", "Playlists", LibTab::Playlists)).child(tab("tab-al", "Albums", LibTab::Albums)).child(tab("tab-ar", "Artists", LibTab::Artists)).child(tab("tab-mx", "Mixes", LibTab::Mixes));
    if w < 280.0 {
        tabs = tabs.flex_wrap(); // a narrowed sidebar (Shell): the chips take two lines
    }
    col()
        .key("sidebar")
        .role("navigation")
        .w(w)
        .h_full()
        .padding(12.0)
        .gap(6.0)
        .fill(theme::sidebar_bg())
        .child(nav_btn("nav-home", "Home", G::Home, Page::Home))
        .child(nav_btn("nav-explore", "Explore", G::Compass, Page::Explore))
        .child(nav_btn("nav-feed", "Feed", G::Bell, Page::Feed))
        .child(el().h(10.0))
        .child(
            row().w_full().items_center().gap(6.0)
                .child(lib_toggle(false))
                .child(spacer())
                .child(crate::library::sidebar_tools(s)),
        )
        .child(tabs)
        .child(ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(crate::library::sidebar_list(s)), "lib-scroll".into()).labelled_by("lib"))
}

/// Loved Tracks' artwork: not an image from TIDAL but React's fixed gradient (#450af5 → #8e2de2
/// → #00d2ff, to the bottom right) with a white heart.
pub(crate) fn loved_art(size: f32, radius: f32, heart: f32) -> El {
    let c = |h: &str| rune::color::hex(h).unwrap_or(white(0.5));
    el().size(size, size).radius(radius).clip_rounded(radius).items_center().justify_center().a11y_hidden()
        .linear_gradient(std::f32::consts::PI * 0.75, [(0.0, c("#450af5")), (0.5, c("#8e2de2")), (1.0, c("#00d2ff"))])
        .child(glyph(G::HeartFilled, heart, white(1.0)))
}

pub(crate) fn lib_row(key: &str, label: &str, title: &str, sub: &str, seed: u64, img: Option<String>, menu: Option<Value>, page: Option<Page>) -> El {
    let art = if key == "lib-loved" { loved_art(40.0, 6.0, 15.0) } else { ui::with_image(ui::tile(ui::hue(Some(seed)), 40.0, seed, 6.0), img, 40.0, 6.0) };
    let mut r = theme::focusable_row(row().key(key.to_string()).role("button").label(label)).w_full().h(52.0).px(8.0).gap(10.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer).help(title)
        .child(art)
        .child(col().min_w(0.0).a11y_hidden().child(text(title.to_string()).font(t(13.0, 600.0)).color(white(0.92)).truncate()).child(text(sub.to_string()).font(t(11.0, 400.0)).color(white(0.55)).truncate()));
    if let Some(p) = page {
        r = r.on_press(move |s: &mut App| go(s, p.clone()));
    }
    if let Some(it) = menu {
        r = r.on_secondary_press(move |s: &mut App, p, _| crate::interact::open_media(s, it.clone(), crate::interact::Anchor::Point(p.x, p.y)));
    }
    r
}


/// Player-bar extras the React bar has around the transport (React PlayerBar).
/// After the bar's title (PlayerBar's LikeButton and ContextMenuButton): the heart, filled in
/// the accent when the track is loved ("Like" / "Unlike"; React's has no name, and neither
/// toasts), and More options, the track menu below it.
pub fn bar_track_actions(v: &View) -> El {
    let Some(c) = &v.current else { return el() };
    use crate::interact::{self, Anchor, Fav, Say};
    let heart = |loved: bool| theme::focusable(el().key("bar-love").role("button").label(if loved { "Unlike" } else { "Like" }))
        .size(28.0, 28.0).radius(14.0).items_center().justify_center()
        .hover(white(0.06)).press_scale(0.9).cursor(rune::Cursor::Pointer)
        .child(glyph(if loved { G::HeartFilled } else { G::Heart }, 16.0, if loved { accent() } else { white(0.45) }));
    // A video's heart is a video favourite (PlayerBar FavoriteButton); it has no track menu.
    if let Some(vi) = &v.video {
        let loved = interact::is_fav(Fav::Video, &vi.id.to_string());
        let (id, title) = (vi.id.to_string(), vi.title.clone());
        return heart(loved).on_press(move |_: &mut App| interact::toggle_fav(Fav::Video, id.clone(), title.clone(), Say::Nothing));
    }
    let loved = interact::is_fav(Fav::Track, &c.id.to_string());
    let (id, title) = (c.id.to_string(), c.title.clone());
    let cur = interact::json_of_track(c.id, &c.title, &c.artist);
    row().gap(2.0).items_center()
        .child(heart(loved).on_press(move |_: &mut App| interact::toggle_fav(Fav::Track, id.clone(), title.clone(), Say::Nothing)))
        .child(ui::icon_button::<App>("more", "More options", G::More, 16.0, white(0.45))
            .on_press(move |s: &mut App| interact::open_track(s, cur.clone(), None, Anchor::Below("more".into()))))
}

/// The bar's title and artist (PlayerBar TrackInfoSection): the title opens its album and the
/// artist the artist's page, with the explicit and AI badges after the title. A video shows
/// its title, explicit badge and artist, unlinked.
pub fn bar_info(v: &View) -> El {
    let base = col().shrink().min_w(0.0).gap(2.0);
    let Some(cur) = &v.current else {
        return base.child(text("Nothing playing").font(t(13.0, 600.0)).color(white(0.95)).truncate());
    };
    let title = text(v.title.clone()).font(t(13.0, 600.0)).color(white(0.95)).truncate();
    let artist = text(v.artist.clone()).font(t(12.0, 400.0)).color(white(0.6)).truncate();
    let link = |e: El, key: &str, label: String, go_to: Page| {
        theme::focusable(el().key(key.to_string()).role("link").label(label)).shrink().min_w(0.0).radius(3.0).hover_group().cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| go(s, go_to.clone()))
            .child(e.group_color(white(1.0)).a11y_hidden())
    };
    let is_video = v.video.is_some();
    let mut head = row().gap(6.0).min_w(0.0).items_center();
    head = match cur.album_id.filter(|_| !is_video) {
        Some(id) => head.child(link(title, "bar-title", v.title.clone(), Page::Album(id))),
        None => head.child(title.shrink()),
    };
    if let Some(b) = crate::content::badges(cur.explicit, cur.ai && !is_video) {
        head = head.child(b);
    }
    let foot = match cur.artist_id.filter(|_| !is_video) {
        Some(id) => link(artist, "bar-artist", v.artist.clone(), Page::Artist(id)),
        None => artist,
    };
    base.child(head).child(foot)
}

/// `shell` says which of the source and the quality badge there's room for: in a narrow window
/// the source goes first, then the badge, so the transport and seek slider keep theirs.
pub fn bar_extras(v: &View, shell: Shell, drawer: Option<Drawer>) -> El {
    let mut r = row().gap(4.0).items_center();
    if v.current.is_some() {
        // PlayingFromLabel: "Playing from <name>", the name opening the collection's page.
        if let Some(src) = v.playing_from.as_ref().filter(|_| shell.bar_source) {
            let link = v.source.clone().filter(crate::queue_ui::navigable);
            let mut b = row().key("playing-from").role("button").label(src.clone()).shrink().max_w(220.0).h(28.0).px(8.0).gap(4.0).radius(6.0).items_center()
                .child(text("Playing from").font(t(10.0, 500.0)).color(white(0.4)).a11y_hidden())
                .child(text(src.clone()).font(t(11.0, 600.0)).color(white(0.6)).truncate().shrink().a11y_hidden());
            if let Some(source) = link {
                b = theme::focusable(b).hover(white(0.06)).cursor(rune::Cursor::Pointer).on_press(move |s: &mut App| crate::queue_ui::open_source(s, &source));
            }
            r = r.child(b);
        }
        // QualityBadge: opens the signal path panel ("Show signal path").
        if let Some(b) = v.badge.as_ref().filter(|_| shell.bar_badge) {
            r = r.child(theme::focusable(row().key("badge").role("button").label(b.clone())).h(22.0).px(8.0).radius(6.0).items_center().fill(white(0.1)).hover(white(0.16)).cursor(rune::Cursor::Pointer)
                .on_press(crate::signal_path::open)
                .child(text(b.clone()).font(t(10.0, 650.0).tracking(0.6)).color(white(0.8)).a11y_hidden()));
        }
    }
    // (the open drawer tab's button, and the miniplayer's while it's open, in the accent)
    let lit = |on: bool| if on { accent() } else { white(0.75) };
    r = r.child(ui::icon_button::<App>("lyrics", "Lyrics", G::Mic, 18.0, lit(drawer == Some(Drawer::Lyrics))).on_press(|s: &mut App| toggle_drawer(s, Drawer::Lyrics)))
        .child(ui::icon_button::<App>("queue", "Play queue", G::List, 18.0, lit(drawer == Some(Drawer::Queue))).on_press(|s: &mut App| toggle_drawer(s, Drawer::Queue)));
    if v.current.is_some() {
        r = r            .child(if crate::mini_open() {
                ui::icon_button::<App>("mini", "Close miniplayer", G::PopOut, 18.0, accent()).on_press(|_: &mut App| crate::set_mini(false))
            } else {
                icon::<App>("mini", "Open miniplayer", G::PopOut).on_press(|_: &mut App| crate::set_mini(true))
            });
    }
    // PlayerBar MaximizeButton, for a video: the video player, maximised; a closed (or
    // restored) video starts again first. A track's full-screen player opens from the bar's
    // cover instead (bar_cover_action).
    if v.video.is_some() {
        let open = v.video_open;
        r = r.child(icon::<App>("full", "Fullscreen player", G::Maximize).on_press_tx(move |s: &mut App, tx: &mut Tx| {
            if !open {
                controller::send(Cmd::TogglePlay);
            }
            s.nav.video.expanded = true;
            if !s.nav.video.fullscreen {
                s.nav.video.fullscreen = true;
                tx.toggle_maximized();
            }
        }));
    }
    r
}

/// What pressing the bar's cover does, with its accessible name: a minimised video's picture
/// opens the video player again (PlayerBar "Expand video"); a track's cover opens the
/// full-screen player, flying up to its place there (full_ui).
pub fn bar_cover_action(v: &View) -> Option<ui::CoverAction<App>> {
    if v.video.is_some() && !v.video_open {
        // a restored or closed video starts again, and its session opens the player
        Some(("Play video", |_: &mut App| controller::send(Cmd::TogglePlay)))
    } else if v.video.is_some() {
        Some(("Expand video", |s: &mut App| s.nav.video.expanded = true))
    } else {
        v.current.as_ref().map(|_| ("Open full-screen player", crate::full_ui::open as fn(&mut App)))
    }
}

// ------------------------------------------------------------------ pages

pub fn page(s: &App, v: &View) -> El {
    took_loading();
    let body = match &*s.nav.page {
        Page::Home => crate::home::home(s),
        Page::Explore => explore(),
        Page::ExploreSub(path, title) => explore_sub(s, path, title),
        Page::Mix(id, title, sub) => mix_page(id, title, sub, v),
        Page::Loved => crate::loved::page(s, v),
        Page::ArtistTracks(id, name) => crate::viewall::artist_tracks(s, *id, name, v),
        Page::ViewAll(title, path, artist) => crate::viewall::page(s, title, path, *artist),
        Page::Library(kind, folder) => crate::library::page(s, *kind, folder.as_ref()),
        Page::Feed => feed(),
        Page::Search(q) => search_page(s, q),
        Page::Album(id) => album(*id, v),
        Page::Artist(id) => artist(*id, v),
        Page::Playlist(u) => crate::playlist::page(s, u, v),
        Page::Settings => col().w_full().gap(18.0).child(heading("Settings", theme::PAGE_TITLE)).child(settings_page(s)),
        Page::Profile => crate::profile::page(s),
        Page::ProfilePlaylists => crate::profile::playlists_page(s),
    };
    // `.cache()`: without it Rune culls content outside the viewport, from paint and from the
    // accessibility tree, so screen readers (and the verifier) would miss everything below the fold.
    // React's PageContainer: content at most 1520 pt wide, centred, so a wide window doesn't
    // stretch rows across the whole screen. Its width is measured for Home's grids and rows.
    let content = col().w_full().max_w(1520.0).px(24.0).py(24.0).gap(18.0).cache()
        .on_drawn_layout_tx(|s: &mut App, r, tx| {
            let w = (r[2] - 48.0).max(200.0);
            if (*s.nav.content_w - w).abs() >= 1.0 { *s.nav.content_w = w } else { tx.live_only() }
        })
        .child(body);
    ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(row().w_full().justify_center().role("main").busy(took_loading()).child(content)), page_key(s))
}

/// The page's scroll viewport key, which is also its region's id (main::view).
pub fn page_key(s: &App) -> String {
    format!("page-{:?}", s.nav.page)
}

pub(crate) fn play_label(v: &View, source_name: &str) -> &'static str {
    if v.playing_from.as_deref() == Some(source_name) && v.current.is_some() {
        if v.playing { "Pause" } else { "Resume" }
    } else {
        "Play"
    }
}

pub(crate) fn hero(title: &str, title_heading: bool, sub: &str, seed: u64, round: bool, img: Option<String>, actions: El) -> El {
    hero_with(title, title_heading, text(sub.to_string()).font(t(13.0, 500.0)).color(white(0.6)), seed, round, img, actions)
}

pub(crate) fn hero_with(title: &str, title_heading: bool, sub: El, seed: u64, round: bool, img: Option<String>, actions: El) -> El {
    let h = ui::hue(Some(seed));
    let art = if round { ui::with_image(ui::art(h, 180.0, seed, 0.0), img, 180.0, 90.0) } else { ui::with_image(ui::tile(h, 180.0, seed, 14.0), img, 180.0, 14.0) };
    let title_el = if title_heading { heading(title, theme::PAGE_TITLE) } else { text(title.to_string()).font(t(theme::PAGE_TITLE, 650.0)).color(white(0.95)) };
    let title_el = title_el.truncate();
    row().w_full().gap(24.0).items_end()
        .child(art)
        .child(col().gap(10.0).min_w(0.0).child(title_el).child(sub).child(actions))
}

/// "Artist, Artist · 2013" under an album's title, each artist a link to their page, as the
/// React header's TrackArtists (there clickable spans; here also reachable by keyboard and
/// exposed as links).
pub(crate) fn album_byline(a: &Value) -> El {
    let d = data::data(a);
    let mut artists: Vec<(u64, String)> = d["artists"].as_array().map(|xs| {
        xs.iter().filter_map(|x| Some((x["id"].as_u64()?, x["name"].as_str()?.to_string()))).collect()
    }).unwrap_or_default();
    if artists.is_empty()
        && let (Some(id), Some(name)) = (d["artist"]["id"].as_u64(), d["artist"]["name"].as_str())
    {
        artists.push((id, name.to_string()));
    }
    let dim = |s: &str| text(s.to_string()).font(t(13.0, 500.0)).color(white(0.6));
    let mut r = row().items_center().min_w(0.0);
    for (i, (aid, name)) in artists.into_iter().enumerate() {
        if i > 0 {
            r = r.child(dim(", "));
        }
        r = r.child(
            text(name.clone()).font(t(13.0, 600.0)).color(white(0.85))
                .key(format!("album-artist-{i}")).role("link").label(name).focusable(true).focus_ring(2.0, 2.0, white(0.6))
                .cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| go(s, Page::Artist(aid))),
        );
    }
    let year = d["releaseDate"].as_str().unwrap_or("").get(..4).unwrap_or("").to_string();
    if !year.is_empty() {
        r = r.child(dim(&format!(" · {year}")));
    }
    r
}

pub(crate) fn play_buttons(v: &View, src_name: &str, play: Cmd, shuffle: Cmd) -> El {
    let label = play_label(v, src_name);
    let toggle = label != "Play";
    row().gap(8.0).items_center()
        .child(button::<App>("hero-play", label, Some(if label == "Pause" { G::Pause } else { G::Play }), true).on_press(move |_: &mut App| {
            if toggle { controller::send(Cmd::TogglePlay) } else { controller::send(play.clone()) }
        }))
        .child(button::<App>("hero-shuffle", "Shuffle", Some(G::Shuffle), false).on_press(move |_: &mut App| controller::send(shuffle.clone())))
}

fn album(id: u64, v: &View) -> El {
    let p = match ready_or(Key::Album(id)) { Ok(v) => v, Err(e) => return e };
    let page = &p["page"];
    let a = &page["album"];
    let title = a["title"].as_str().unwrap_or("").to_string();
    let tracks: Vec<Value> = page["tracks"].as_array().cloned().unwrap_or_default();
    let source = json!({"type": "album", "id": id, "name": title});
    let play = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: true, shuffle: false };
    let shuf = Cmd::PlayCollection { tracks: tracks.clone(), source, album_mode: true, shuffle: true };
    let actions = row().gap(8.0).items_center()
        .child(play_buttons(v, &title, play, shuf))
        .child({
            let (on, aid, l) = (crate::interact::is_fav(crate::interact::Fav::Album, &id.to_string()), id.to_string(), title.clone());
            header_btn("album-fav", "Favorite album", if on { "Added" } else { "Add" }, if on { G::HeartFilled } else { G::Heart }, on)
                .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Album, aid.clone(), l.clone(), crate::interact::Say::Nothing))
        })
        .child(header_btn("album-share", "Share album", "Share", G::Share, false).on_press_tx(move |_: &mut App, tx: &mut Tx| {
            tx.copy_rich(rune::ClipboardContent::new(format!("https://tidal.com/album/{id}")));
            crate::interact::toast("Copied share link to clipboard".into(), crate::interact::Kind::Success);
        }))
        .child({
            let item = json!({"type": "ALBUM", "data": a.clone()});
            header_btn("album-more", "More options", "More", G::More, false)
                .on_press(move |s: &mut App| crate::interact::open_media(s, item.clone(), crate::interact::Anchor::Below("album-more".into())))
        });
    let mut c = col().w_full().gap(18.0).child(hero_with(&title, true, album_byline(a), id, false, crate::art::item_url(a, 180.0), actions));
    let src = json!({"type": "album", "id": id, "name": title});
    let every = (0..tracks.len()).collect();
    let rows = track_list("al-t", Rc::new(tracks), every, move |tracks, i| {
        let (all, src) = (tracks.clone(), src.clone());
        track_row(format!("al-t{i}"), i + 1, &tracks[i], move |_: &mut App| {
            controller::send(Cmd::PlayFrom { tracks: all.to_vec(), index: i, source: src.clone(), album_mode: true });
        })
    });
    c = c.child(rows);
    // TIDAL's editorial review, when the album has one (tidal.com shows it under the tracks).
    if let Some(review) = page["review"]["text"].as_str().filter(|r| !r.trim().is_empty()) {
        let mut r = col().w_full().max_w(820.0).gap(10.0).child(heading("Review", theme::SECTION)).child(crate::bio::view(review, 14.0, 0.72));
        if let Some(src) = page["review"]["source"].as_str().filter(|x| !x.is_empty()) {
            r = r.child(text(format!("Review from {src}")).font(t(12.0, 400.0)).color(white(0.4)));
        }
        c = c.child(r);
    }
    for (si, sec) in page["sections"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        // Album-page modules hold bare objects whose own  is e.g. "EP"; the module's type
        // (ALBUM_LIST, ...) is what says what they are.
        let as_kind = sec["sectionType"].as_str().and_then(|t| t.strip_suffix("_LIST")).map(str::to_string);
        let items: Vec<Value> = sec["items"].as_array().cloned().unwrap_or_default().into_iter()
            .map(|it| match &as_kind { Some(k) if it.get("_itemType").is_none() => json!({"type": k, "data": it}), _ => it })
            .collect();
        let cards = cards_for(&format!("al-s{si}"), &items);
        if !cards.is_empty() {
            let title = sec["title"].as_str().unwrap_or("").to_string();
            let mut head = row().w_full().items_center().child(heading(&title, theme::SECTION));
            // AlbumView's CardScrollSection: View all opens the whole section (ViewAllPage).
            if let Some(path) = sec["apiPath"].as_str().filter(|p| !p.is_empty()) {
                let to = Page::ViewAll(title.clone(), path.to_string(), None);
                head = head.child(spacer()).child(theme::focusable(row().key(format!("al-s{si}-all")).role("button").label("View all")).h(26.0).px(8.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut App| go(s, to.clone()))
                    .child(caps("View all", 0.6).a11y_hidden()));
            }
            c = c.child(head).child(grid(cards));
        }
    }
    c
}

fn artist(id: u64, v: &View) -> El {
    let p = match ready_or(Key::Artist(id)) { Ok(v) => v, Err(e) => return e };
    let a = data::data(&p["item"]);
    let name = a["name"].as_str().unwrap_or("").to_string();
    // Play and Shuffle page through all of the artist's top tracks (the artist play source).
    // The page's own top tracks start it, as ArtistPage's `loaded`.
    let src = json!({"type": "artist", "id": id, "name": name});
    let preview: Vec<Value> = p["items"].as_array().into_iter().flatten()
        .map(|sec| sec["items"].as_array().cloned().unwrap_or_default().into_iter().filter(|it| data::kind(it) == "TRACK").map(|it| data::data(&it).clone()).collect::<Vec<_>>())
        .find(|ts| !ts.is_empty()).unwrap_or_default();
    let actions = row().gap(8.0).items_center()
        .child(crate::coll::play_pair(v, &name, move |shuffle| crate::paged::play_with(crate::paged::PKey::ArtistTracks(id), preview.clone(), src.clone(), shuffle, None, |_| true)))
        .child({
            // ArtistPage: Plus/"Follow", or Check/"Following" in the accent
            let (on, aid, l) = (crate::interact::is_fav(crate::interact::Fav::Artist, &id.to_string()), id.to_string(), name.clone());
            header_btn("artist-follow", if on { "Following" } else { "Follow" }, if on { "Following" } else { "Follow" }, if on { G::Check } else { G::Plus }, on)
                .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Artist, aid.clone(), l.clone(), crate::interact::Say::Nothing))
        })
        .child(match a["mixes"]["ARTIST_MIX"].as_str() {
            // ArtistPage's "Artist radio", when the artist has one.
            Some(mix) => {
                let to = Page::Mix(mix.to_string(), format!("{name} Radio"), name.clone());
                header_btn("artist-radio", "Artist radio", "Radio", G::Radio, false).on_press(move |s: &mut App| go(s, to.clone()))
            }
            None => el(),
        })
        .child(header_btn("artist-share", "Share", "Share", G::Share, false).on_press_tx(move |_: &mut App, tx: &mut Tx| {
            tx.copy_rich(rune::ClipboardContent::new(format!("https://tidal.com/artist/{id}")));
            crate::interact::toast("Copied share link to clipboard".into(), crate::interact::Kind::Success);
        }))
        .child({
            let item = json!({"type": "ARTIST", "data": a.clone()});
            header_btn("artist-more", "More", "More", G::More, false)
                .on_press(move |s: &mut App| crate::interact::open_media(s, item.clone(), crate::interact::Anchor::Below("artist-more".into())))
        });
    let fans = p["header"]["followersAmount"].as_u64().unwrap_or(0);
    let mut c = col().w_full().gap(18.0).child(hero(&name, true, &format!("{fans} fans"), id, true, crate::art::artist_picture(a, 180.0), actions));
    // ArtistPage: two clamped lines of the bio without its markup, and "Read more" for the
    // whole of it, links and all, in a dialog.
    if let Some(bio) = p["header"]["biography"]["text"].as_str().filter(|b| !b.trim().is_empty()) {
        c = c.child(col().gap(6.0).max_w(820.0).child(text(crate::bio::plain(bio)).font(t(13.0, 400.0)).color(white(0.6)).wrap().max_lines(2))
            .child(theme::focusable(row().key("bio-more").role("button").label("Read more")).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| s.nav.dialog = Some(Dialog::Bio(id)))
                .child(text("Read more").font(t(12.0, 650.0)).color(white(0.85)).a11y_hidden())));
    }
    let mut top_seen = false;
    for (si, sec) in p["items"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let title = sec["title"].as_str().unwrap_or("");
        let items = sec["items"].as_array().cloned().unwrap_or_default();
        // getArtistPage skips empty modules and the credits module.
        if items.is_empty() || items.first().is_some_and(|it| data::kind(it) == "TRACK_CREDIT") {
            continue;
        }
        let tracks: Vec<Value> = items.iter().filter(|it| data::kind(it) == "TRACK").map(|it| data::data(it).clone()).collect();
        // A section with more has View all: popular tracks for a track list (ArtistTracksPage),
        // the whole list otherwise (ViewAllPage through get_artist_view_all).
        let mut head = row().w_full().items_center().child(heading(title, theme::SECTION));
        if let Some(path) = sec["viewAll"].as_str().or(sec["showMore"]["apiPath"].as_str()).filter(|p| !p.is_empty()) {
            let to = if tracks.is_empty() { Page::ViewAll(title.to_string(), path.to_string(), Some(id)) } else { Page::ArtistTracks(id, name.clone()) };
            head = head.child(spacer()).child(theme::focusable(row().key(format!("ar-s{si}-all")).role("button").label("View all")).h(26.0).px(8.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| go(s, to.clone()))
                .child(caps("View all", 0.6).a11y_hidden()));
        }
        c = c.child(head);
        if !tracks.is_empty() {
            // ArtistPage handlePlayTrack: play from the artist source, the first track list being
            // the top tracks; the manual queue is kept.
            let top = !std::mem::replace(&mut top_seen, true);
            let mut rows = col().w_full().gap(2.0);
            // ArtistPage shows a track section's first ten.
            for (i, tr) in tracks.iter().take(10).enumerate() {
                let (all, an) = (tracks.clone(), name.clone());
                let tid = tr["id"].as_u64();
                rows = rows.child(track_row(format!("ar-s{si}-t{i}"), i + 1, tr, move |_: &mut App| {
                    if top {
                        // The top tracks: the artist source from the clicked one, the rest of
                        // the artist's top tracks paged in behind it.
                        let src = json!({"type": "artist", "id": id, "name": an});
                        crate::paged::play_with(crate::paged::PKey::ArtistTracks(id), all.clone(), src, false, tid, |_| true);
                    } else {
                        controller::send(Cmd::PlayArtistTrack { id, name: an.clone(), tracks: all.clone(), index: i, top });
                    }
                }));
            }
            c = c.child(rows);
        } else if items.iter().any(|it| data::kind(it) == "VIDEO") {
            // ArtistPage's video module: 16:9 cards that play through the queue.
            let vids: Vec<Value> = items.iter().filter(|it| data::kind(it) == "VIDEO").cloned().collect();
            c = c.child(grid(crate::video_ui::video_cards(&format!("ar-s{si}"), &vids, json!({"type": "artist", "id": id, "name": name}))));
        } else {
            c = c.child(grid(cards_for(&format!("ar-s{si}"), &items)));
        }
    }
    c
}

/// What a track list shows with nothing in it: `none` when it has no tracks at all, or that
/// the filter matched none of them.
pub(crate) fn tracks_empty(s: &App, none: &str) -> El {
    match s.nav.filter.text.trim() {
        "" => ui::empty(none),
        f => ui::empty(&format!("No tracks match \"{f}\"")),
    }.px(12.0)
}

fn search_page(s: &App, q: &str) -> El {
    let v = match ready_or(Key::Search(q.to_string())) { Ok(v) => v, Err(e) => return e };
    let tab = |key: &str, label: &str, which: SearchTab| {
        let on = s.nav.tab == which;
        theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(30.0).px(12.0).radius(15.0).items_center()
            .fill(if on { white(0.9) } else { white(0.08) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| *s.nav.tab = which)
            .child(text(label.to_string()).font(t(12.0, 600.0)).color(if on { theme::ink() } else { white(0.9) }).a11y_hidden())
    };
    let tabs = row().gap(6.0).flex_wrap().focus_group()
        .child(tab("st-all", "All Results", SearchTab::All))
        .child(tab("st-top", "Top Hits", SearchTab::TopHits))
        .child(tab("st-tracks", "Tracks", SearchTab::Tracks))
        .child(tab("st-vid", "Videos", SearchTab::Videos))
        .child(tab("st-pl", "Playlists", SearchTab::Playlists))
        .child(tab("st-al", "Albums", SearchTab::Albums))
        .child(tab("st-ar", "Artists", SearchTab::Artists));
    let tracks: Vec<Value> = v["tracks"].as_array().cloned().unwrap_or_default();
    let typed = |k: &str, arr: &str| -> Vec<Value> { v[arr].as_array().cloned().unwrap_or_default().into_iter().map(|d| json!({"type": k, "data": d})).collect() };
    let (albums, artists, playlists) = (typed("ALBUM", "albums"), typed("ARTIST", "artists"), typed("PLAYLIST", "playlists"));
    let videos: Vec<Value> = v["videos"]["items"].as_array().or(v["videos"].as_array()).cloned().unwrap_or_default();
    let video_source = json!({"type": "search", "id": q, "name": format!("Search: {q}")});
    let tracks = Rc::new(tracks);
    let track_rows = |limit: usize| {
        // SearchView handlePlayTrack: playFromSource over every track hit, keeping the manual queue.
        let src = json!({"type": "search", "id": q, "name": format!("Search: {q}")});
        track_list("sr-t", tracks.clone(), (0..tracks.len().min(limit)).collect(), move |tracks, i| {
            let (all, src) = (tracks.clone(), src.clone());
            track_row(format!("sr-t{i}"), i + 1, &tracks[i], move |_: &mut App| {
                controller::send(Cmd::PlayFrom { tracks: all.to_vec(), index: i, source: src.clone(), album_mode: false });
            })
        })
    };
    // SearchView's All tab: each kind's first few under its heading, its View all opening that
    // kind's tab; a kind with no hits is left out.
    let section = |title: &str, to: SearchTab, body: El| col().w_full().gap(10.0).child(row().items_center().child(heading(title, theme::SECTION)).child(spacer()).child(
        theme::focusable(row().key(format!("sr-all-{title}")).role("button").label("VIEW ALL")).h(26.0).px(8.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| *s.nav.tab = to)
            .child(caps("View all", 0.6).a11y_hidden()))).child(body);
    // A tab with no hits says so where its rows or cards would be.
    let or_none = |none: bool, body: El, what: &str| if none { ui::empty(&format!("No {what}")) } else { body };
    let first = |items: &[Value], n: usize| items[..items.len().min(n)].to_vec();
    let nothing = tracks.is_empty() && albums.is_empty() && artists.is_empty() && playlists.is_empty() && videos.is_empty();
    let body = match *s.nav.tab {
        SearchTab::All if nothing => col().gap(6.0).py(40.0).w_full().items_center()
            .child(text("No results found").font(t(17.0, 650.0)).color(white(0.9)))
            .child(ui::empty("Try a different search term")),
        SearchTab::All => {
            let mut c = col().w_full().gap(22.0);
            if !tracks.is_empty() {
                c = c.child(section("Tracks", SearchTab::Tracks, track_rows(8)));
            }
            if !videos.is_empty() {
                c = c.child(section("Videos", SearchTab::Videos, grid(crate::video_ui::video_cards("sr-vid", &videos[..videos.len().min(4)], video_source.clone()))));
            }
            if !playlists.is_empty() {
                c = c.child(section("Playlists", SearchTab::Playlists, grid(cards_for("sr-pl", &first(&playlists, 6)))));
            }
            if !albums.is_empty() {
                c = c.child(section("Albums", SearchTab::Albums, grid(cards_for("sr-al", &first(&albums, 6)))));
            }
            if !artists.is_empty() {
                c = c.child(section("Artists", SearchTab::Artists, grid(cards_for("sr-ar", &first(&artists, 6)))));
            }
            c
        }
        SearchTab::TopHits => top_hits(&v["topHits"]),
        SearchTab::Tracks => or_none(tracks.is_empty(), track_rows(usize::MAX), "tracks"),
        SearchTab::Albums => or_none(albums.is_empty(), grid(cards_for("sr-al", &albums)), "albums"),
        SearchTab::Artists => or_none(artists.is_empty(), grid(cards_for("sr-ar", &artists)), "artists"),
        SearchTab::Playlists => or_none(playlists.is_empty(), grid(cards_for("sr-pl", &playlists)), "playlists"),
        SearchTab::Videos => or_none(videos.is_empty(), grid(crate::video_ui::video_cards("sr-vid", &videos, video_source.clone())), "videos"),
    };
    col().w_full().gap(16.0).child(heading(&format!("Results for \"{q}\""), theme::PAGE_TITLE)).child(tabs).child(body)
}

/// The Top Hits tab (SearchView TopHitsList): every kind in one list, in the server's order. A
/// track's cover plays it and the rest of its row opens its album; an album, artist or
/// playlist opens its page; a video plays. Each has More options.
fn top_hits(hits: &Value) -> El {
    let hits = hits.as_array().cloned().unwrap_or_default();
    if hits.is_empty() {
        return col().gap(6.0).py(40.0).w_full().items_center()
            .child(text("No top hits").font(t(17.0, 650.0)).color(white(0.9)))
            .child(ui::empty("Try the other tabs for more results"));
    }
    let mut c = col().w_full().gap(2.0);
    for (i, hit) in hits.iter().enumerate() {
        let kind = hit["hitType"].as_str().unwrap_or("");
        let title = hit["title"].as_str().or(hit["name"].as_str()).unwrap_or("").to_string();
        let artist = hit["artistName"].as_str().or(hit["track"]["artists"][0]["name"].as_str()).or(hit["video"]["artists"][0]["name"].as_str()).unwrap_or("").to_string();
        let with = |k: &str| if artist.is_empty() { k.to_string() } else { format!("{k} · {artist}") };
        let sub = match kind { "TRACKS" => with("Track"), "VIDEOS" => with("Video"), "ALBUMS" => with("Album"), "ARTISTS" => "Artist".into(), _ => "Playlist".into() };
        let seed = hit["id"].as_u64().unwrap_or(i as u64 + 1);
        let img = crate::art::item_url(hit, 48.0).or_else(|| crate::art::cover(hit["albumCover"].as_str(), 48.0));
        let round = kind == "ARTISTS";
        let thumb = if round { ui::with_image(ui::art(ui::hue(Some(seed)), 48.0, seed, 0.0), img, 48.0, 24.0) } else { ui::with_image(ui::tile(ui::hue(Some(seed)), 48.0, seed, 6.0), img, 48.0, 6.0) };
        let key = format!("sr-top-{i}");
        let more_key = format!("{key}-more");
        let page = match kind {
            "ALBUMS" => hit["id"].as_u64().map(Page::Album),
            "ARTISTS" => hit["id"].as_u64().map(Page::Artist),
            "PLAYLISTS" => hit["uuid"].as_str().map(|u| Page::Playlist(u.to_string())),
            "TRACKS" => hit["albumId"].as_u64().or(hit["track"]["album"]["id"].as_u64()).map(Page::Album),
            _ => None,
        };
        let mut r = theme::focusable_row(row().key(key.clone()).role("button").label(title.as_str())).w_full().h(60.0).px(10.0).gap(12.0).radius(8.0).items_center().hover(white(0.05)).cursor(rune::Cursor::Pointer);
        let thumb = match kind {
            "TRACKS" => {
                let id = hit["id"].as_u64().unwrap_or(0);
                el().key(format!("{key}-play")).role("button").label(format!("Play {title}")).cursor(rune::Cursor::Pointer)
                    .on_press(move |_: &mut App| controller::send(Cmd::PlaySingleId(id))).child(thumb)
            }
            _ => thumb,
        };
        r = r.child(thumb)
            .child(col().grow(1.0).min_w(0.0).gap(2.0).a11y_hidden()
                .child(text(title.clone()).font(t(14.0, 600.0)).color(white(0.95)).truncate())
                .child(text(sub).font(t(12.0, 400.0)).color(white(0.55)).truncate()));
        if kind == "VIDEOS" {
            let v = if hit["video"].is_object() { hit["video"].clone() } else { hit.clone() };
            let tr = crate::video_ui::video_track(&v);
            r = r.on_press(move |_: &mut App| controller::send(Cmd::PlayCollection { tracks: vec![tr.clone()], source: json!({"type": "search", "id": "top-hits", "name": "Search"}), album_mode: false, shuffle: false }));
        } else if let Some(p) = page {
            r = r.on_press(move |s: &mut App| go(s, p.clone()));
        }
        let menu: Option<(bool, Value)> = match kind {
            "TRACKS" => Some((true, if hit["track"].is_object() { hit["track"].clone() } else { crate::interact::json_of_track(seed, &title, &artist) })),
            "ALBUMS" => Some((false, json!({"type": "ALBUM", "data": {"id": hit["id"], "title": title, "cover": hit["cover"]}}))),
            "ARTISTS" => Some((false, json!({"type": "ARTIST", "data": {"id": hit["id"], "name": title, "picture": hit["picture"]}}))),
            "PLAYLISTS" => Some((false, json!({"type": "PLAYLIST", "data": {"uuid": hit["uuid"], "title": title, "image": hit["image"]}}))),
            _ => None,
        };
        if let Some((track, item)) = menu {
            let mk = more_key.clone();
            r = r.child(icon::<App>(&more_key, "More options", G::More).on_press(move |s: &mut App| {
                if track {
                    crate::interact::open_track(s, item.clone(), None, crate::interact::Anchor::Below(mk.clone()))
                } else {
                    crate::interact::open_media(s, item.clone(), crate::interact::Anchor::Below(mk.clone()))
                }
            }));
        }
        c = c.child(r);
    }
    c
}

// ------------------------------------------------------------------ account menu, settings, queue

fn menu_item(key: &str, label: &str, checked: Option<bool>, on: impl Fn(&mut App) + 'static) -> El {
    // Toggles (checked is Some) keep the menu open; actions close it.
    let closes = checked.is_none();
    let mut r = theme::focusable_row(row().key(key.to_string()).role("button").label(label)).w_full().h(36.0).px(12.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center()
        .hover(white(0.08)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            if closes {
                s.nav.menu_open = false;
            }
            on(s)
        })
        .child(text(label.to_string()).font(t(13.0, 500.0)).color(white(0.9)).truncate().grow(1.0).min_w(0.0).a11y_hidden());
    // A toggle shows its state as a switch. (Its accessible state isn't set: `checked` would
    // make AT-SPI report this button as a toggle button, a different role.)
    if let Some(c) = checked {
        r = r.child(ui::switch_mark(c));
    }
    r
}

/// The account menu's first row: who is signed in, with their picture. It opens the profile
/// (UserMenu's navigateToProfile).
fn menu_profile(name: &str) -> El {
    let avatar = ui::with_image(el().size(30.0, 30.0).radius(15.0).fill(white(0.1)).clip_rounded(15.0).items_center().justify_center().child(glyph(G::User, 16.0, white(0.8))),
        crate::profile::avatar(), 30.0, 15.0);
    theme::focusable_row(row().key("am-profile").role("button").label(name)).w_full().h(48.0).px(12.0).gap(10.0).items_center()
        .radius(theme::inset_radius(8.0)).hover(white(0.08)).cursor(rune::Cursor::Pointer)
        .on_press(|s: &mut App| {
            s.nav.menu_open = false;
            go(s, Page::Profile);
        })
        .child(avatar)
        .child(text(name.to_string()).font(t(14.0, 650.0)).color(white(theme::TEXT)).truncate().grow(1.0).min_w(0.0).a11y_hidden())
}

/// The Account button's menu (React AccountMenu).
pub fn account_menu(s: &App, v: &View) -> Option<El> {
    if !s.nav.menu_open {
        return None;
    }
    let name = data::ready(Key::Profile).and_then(|p| p.get(0).and_then(|n| n.as_str()).map(str::to_string)).unwrap_or_default();
    let (bp, ex) = (v.bit_perfect, v.exclusive);
    let mut m = theme::overlay_surface(col().key("account-menu").w(260.0).padding(8.0).gap(2.0), theme::Overlay::Menu).focus_group();
    if !name.is_empty() {
        m = m.child(menu_profile(&name)).child(el().w_full().h(1.0).my(4.0).fill(white(0.08)));
    }
    m = m.child(menu_item("am-exclusive", "Exclusive output", Some(ex), move |s: &mut App| crate::settings::set_exclusive(s, !ex)));
    // With exclusive output on: the output device, and bit-perfect (UserMenu).
    if ex {
        if let Some(d) = crate::settings::device_picker(s) {
            m = m.child(d);
        }
        m = m.child(menu_item("am-bitperfect", "Bit-perfect", Some(bp), move |_: &mut App| crate::settings::set_bit_perfect(!bp)));
    }
    m = m
        .child(menu_item("am-settings", "Settings", None, |s: &mut App| {
            s.nav.settings_tab = SettingsTab::Playback;
            s.nav.settings_open = true;
            load_setting_fields(s);
        }))
        .child(menu_item("am-shortcuts", "Shortcuts", None, |s: &mut App| s.nav.dialog = Some(Dialog::Shortcuts)))
        .child(menu_item("am-about", "About", None, |s: &mut App| s.nav.dialog = Some(Dialog::About)))
        .child(menu_item("am-logout", "Log out", None, |_: &mut App| controller::send(Cmd::Logout)));
    // A modal layer over the window: the page is inert while the menu is open, and a press
    // anywhere outside the menu closes it.
    Some(el().key("am-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .modal(false, |s: &mut App| s.nav.menu_open = false)
        .on_press(|s: &mut App| s.nav.menu_open = false).on_secondary_press(|s: &mut App, _, _| s.nav.menu_open = false)
        .child(row().pin(None, Some(56.0), Some(120.0), None).child(m.role("menu").label("Account").on_press(|_: &mut App| {}))))
}

pub(crate) fn open_url(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// The Keyboard Shortcuts and About dialogs (React ShortcutsModal / AboutModal).
pub fn dialog(s: &App) -> Option<El> {
    let d = s.nav.dialog?;
    let (head, body) = match d {
        // The bindings in force, rebindable (keys.rs).
        Dialog::Shortcuts => (Some(ui::dialog_title("Keyboard Shortcuts")), crate::keys::dialog_body()),
        Dialog::Bio(id) => {
            let p = data::ready(Key::Artist(id))?;
            let a = data::data(&p["item"]);
            let name = a["name"].as_str().unwrap_or("").to_string();
            let bio = p["header"]["biography"]["text"].as_str().unwrap_or("");
            let source = p["header"]["biography"]["source"].as_str().filter(|x| !x.is_empty());
            let avatar = ui::with_image(ui::art(ui::hue(Some(id)), 44.0, id, 0.0), crate::art::artist_picture(a, 44.0), 44.0, 22.0);
            let mut body = col().w_full().gap(0.0).child(crate::bio::view(bio, 14.0, 0.72));
            if let Some(src) = source {
                body = body.child(row().pt(24.0).child(text(format!("Artist bio from {src}")).font(rune::TextStyle { italic: true, ..t(12.0, 400.0) }).color(white(0.4))));
            }
            // (The frame scrolls a long biography under the header.)
            (Some(row().gap(12.0).items_center().child(avatar)
                .child(col().gap(2.0).child(heading(&name, 16.0)).child(text("Biography").font(t(13.0, 400.0)).color(white(0.5))))), body)
        }
        Dialog::About => {
            let link = |key: &str, label: &str, url: &'static str| button::<App>(key, label, None, false).on_press(move |_: &mut App| open_url(url));
            let version = concat!("v", env!("CARGO_PKG_VERSION"));
            (None, col().w_full().gap(12.0).items_center()
                .child(heading(crate::TITLE, theme::PAGE_TITLE))
                .child(link("ab-version", version, "https://github.com/lullabyX/sone/releases"))
                .child(text("The native desktop client for TIDAL on Linux.").font(t(13.0, 500.0)).color(white(0.6)))
                .child(row().gap(8.0)
                    .child(link("ab-github", "GitHub", "https://github.com/lullabyX/sone"))
                    .child(link("ab-issues", "Issues", "https://github.com/lullabyX/sone/issues"))
                    .child(link("ab-license", "GPL-3.0", "https://github.com/lullabyX/sone/blob/master/LICENSE")))
                .child(link("ab-author", "@lullabyX", "https://github.com/lullabyX"))
                .child(link("ab-patreon", "Support development on Patreon", "https://patreon.com/lullabyX")))
        }
    };
    let label = match d { Dialog::Shortcuts => "Keyboard shortcuts", Dialog::About => "About SONE", Dialog::Bio(_) => "Biography" };
    let w = if matches!(d, Dialog::Bio(_)) { 640.0 } else { 520.0 };
    Some(ui::dialog_frame("dialog-layer", "dialog", label, ("dialog-close", "Close dialog"), w, None, head, body, |s: &mut App| {
        s.nav.dialog = None;
        crate::keys::stop_editing();
    }))
}

/// The play queue panel (React QueuePanel).
pub fn queue_panel(v: &View) -> El {
    crate::queue_ui::panel(v)
}

fn toggle_drawer(s: &mut App, tab: Drawer) {
    *s.nav.drawer = if *s.nav.drawer == Some(tab) { None } else { Some(tab) };
}

/// The right-hand drawer (React NowPlayingDrawer): queue, suggestions, lyrics, credits.
pub fn drawer(s: &App, v: &View, shell: Shell) -> Option<El> {
    let tab = (*s.nav.drawer)?;
    let t_btn = |key: &str, label: &str, which: Drawer| {
        let on = tab == which;
        theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(30.0).px(10.0).radius(15.0).items_center()
            .fill(if on { white(0.14) } else { white(0.05) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| *s.nav.drawer = Some(which))
            .child(text(label.to_string()).font(t(11.0, 600.0)).color(white(0.85)).a11y_hidden())
    };
    let tabs = row().gap(6.0).flex_wrap().focus_group()
        .child(t_btn("dr-queue", "Play queue", Drawer::Queue))
        .child(t_btn("dr-suggested", "Suggested tracks", Drawer::Suggested))
        .child(t_btn("dr-lyrics", "Lyrics", Drawer::Lyrics))
        .child(t_btn("dr-credits", "Credits", Drawer::Credits));
    took_loading();
    let body = match tab {
        Drawer::Queue => queue_panel(v),
        Drawer::Lyrics => lyrics(s, v),
        Drawer::Suggested => suggested(v),
        Drawer::Credits => credits(v),
    };
    // NowPlayingDrawer's corner: the full-screen player (or the video, maximised), and close.
    let is_video = v.video.is_some();
    let corner = row().gap(2.0).items_center()
        .child(icon::<App>("dr-full", if is_video { "Fullscreen video" } else { "Fullscreen player" }, G::Maximize).on_press_tx(move |s: &mut App, tx: &mut Tx| {
            if is_video {
                s.nav.video.expanded = true;
                if !s.nav.video.fullscreen {
                    s.nav.video.fullscreen = true;
                    tx.toggle_maximized();
                }
            } else if controller::now_playing().0.is_some() {
                crate::full_ui::open(s);
            }
        }))
        .child(icon::<App>("dr-close", "Close now playing panel", G::Close).on_press(|s: &mut App| *s.nav.drawer = None));
    let mut head = col().w_full().gap(10.0).child(row().w_full().gap(8.0).items_start().child(tabs.shrink()).child(spacer()).child(corner));
    if let Some(cur) = &v.current {
        let mut title = row().w_full().gap(8.0).items_center().child(heading(&cur.title, 16.0).truncate().shrink());
        if let Some(b) = crate::content::badges(cur.explicit, cur.ai) {
            title = title.child(b);
        }
        head = head.child(title);
    }
    // Docked, a column beside the page; in a narrow window (Shell), a sheet over the page's
    // right edge. Either way it starts under the header, which keeps the window controls.
    let d = col().key("drawer").role("complementary").label("Now playing panel").w(shell.drawer_w);
    let d = if shell.drawer_docked { d.h_full() } else { d.pin(None, Some(0.0), Some(0.0), Some(0.0)).border(1.0, white(0.06)).shadow(0.0, 32.0, theme::black(0.45)) };
    let mut d = d.padding(16.0).gap(12.0).fill(theme::drawer_bg())
        .child(head)
        .child(ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(body), "drawer-scroll".into())
            .labelled_by(match tab { Drawer::Queue => "dr-queue", Drawer::Suggested => "dr-suggested", Drawer::Lyrics => "dr-lyrics", Drawer::Credits => "dr-credits" })
            .busy(took_loading()));
    if tab == Drawer::Lyrics && *s.nav.lyrics_scrolled && v.current.as_ref().and_then(|c| synced_lyrics(c.id)).is_some() {
        // LyricsTab: the floating "Sync lyrics" button once the user scrolls away
        d = d.child(theme::focusable(row().key("ly-sync").role("button").label("Sync lyrics")).pin(None, None, Some(32.0), Some(16.0)).h(36.0).px(16.0).gap(8.0).radius(18.0).items_center()
            .fill(accent()).shadow(6.0, 18.0, theme::black(0.4)).cursor(rune::Cursor::Pointer).press_scale(0.95)
            .on_press_tx(|s: &mut App, tx: &mut Tx| {
                *s.nav.lyrics_scrolled = false;
                if let Some(i) = *s.nav.lyric_line {
                    tx.scroll_center(format!("ly-{i}"));
                }
            })
            .child(glyph(G::Mic, 14.0, theme::ink()))
            .child(text("Sync lyrics").font(t(12.0, 700.0)).color(theme::ink()).a11y_hidden()));
    }
    Some(d)
}

/// Synced (LRC) lines as (seconds, text); plain lyrics as untimed lines.

/// LRC lines as React's parseLrc reads them: every `[mm:ss.xx]` tag on a line is a time for its
/// text, lines without text are skipped, and the result is sorted by time.
fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = vec![];
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = vec![];
        while let Some(r) = rest.strip_prefix('[') {
            let Some((tag, after)) = r.split_once(']') else { break };
            let Some((m, sec)) = tag.split_once(':') else { break };
            let (Ok(m), Ok(sec)) = (m.trim().parse::<f64>(), sec.trim().replace(':', ".").parse::<f64>()) else { break };
            times.push(m * 60.0 + sec);
            rest = after.trim_start();
        }
        let text = rest.trim();
        if text.is_empty() {
            continue;
        }
        for at in times {
            out.push((at, text.to_string()));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

thread_local! {
    static LRC: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<(f64, String)>>)>> = const { std::cell::RefCell::new(None) };
}

/// A track's synced lyrics, parsed once (None until fetched, or when it has only plain lyrics).
pub fn synced_lyrics(track: u64) -> Option<std::rc::Rc<Vec<(f64, String)>>> {
    if let Some(hit) = LRC.with(|c| c.borrow().as_ref().filter(|(id, _)| *id == track).map(|(_, l)| l.clone())) {
        return (!hit.is_empty()).then_some(hit);
    }
    let v = data::peek(&Key::Lyrics(track))?;
    let lines = std::rc::Rc::new(v["subtitles"].as_str().map(parse_lrc).unwrap_or_default());
    LRC.with(|c| *c.borrow_mut() = Some((track, lines.clone())));
    (!lines.is_empty()).then_some(lines)
}

/// The line at `pos` (React findActiveLine) and when the next one starts.
pub fn lyric_at(lines: &[(f64, String)], pos: f64) -> (Option<usize>, Option<f64>) {
    let i = lines.iter().rposition(|(at, _)| pos >= *at);
    let next = lines.get(i.map_or(0, |i| i + 1)).map(|(at, _)| *at);
    (i, next)
}

/// The Lyrics tab (React LyricsTab). Synced lyrics: the active line grows (1.22x, bold, from
/// its left edge, over 0.5 s), past lines dim, the view keeps it centred (main::poll), clicking
/// a line seeks there, and scrolling away shows "Sync lyrics". Otherwise the plain text.
fn lyrics(s: &App, v: &View) -> El {
    let Some(cur) = &v.current else { return ui::empty("Nothing playing") };
    // (The track's title is the drawer's header, so it isn't repeated here.)
    let mut c = col().w_full().gap(10.0);
    match data::want(Key::Lyrics(cur.id)) {
        data::Load::Ready(l) => {
            let provider = l["lyricsProvider"].as_str().map(str::to_string);
            if let Some(lines) = synced_lyrics(cur.id) {
                let active = *s.nav.lyric_line;
                let duration = v.duration.max(1.0);
                let mut col_lines = col().w_full().gap(12.0).py(40.0)
                    .on_wheel(|s: &mut App, _, _| {
                        if !*s.nav.lyrics_scrolled {
                            *s.nav.lyrics_scrolled = true;
                        }
                        false
                    });
                for (i, (at, line)) in lines.iter().enumerate() {
                    let on = Some(i) == active;
                    let past = active.is_some_and(|a| i < a);
                    let at = *at;
                    let color = if on { white(0.97) } else if past { white(0.28) } else { white(0.45) };
                    col_lines = col_lines.child(
                        text(line.clone()).key(format!("ly-{i}")).font(t(20.0, if on { 700.0 } else { 500.0 }).lh(26.0)).color(color).wrap()
                            .origin(0.0, 0.5).scale(if on { 1.22 } else { 1.0 }).scale_motion(rune::Spring::new(0.5, 0.0))
                            .hover(white(0.0)).cursor(rune::Cursor::Pointer)
                            .on_press(move |s: &mut App| {
                                *s.nav.lyrics_scrolled = false;
                                controller::send(Cmd::SeekFraction((at / duration).clamp(0.0, 1.0) as f32));
                            }),
                    );
                }
                c = c.child(col_lines.child(el().h(160.0)));
            } else {
                let plain = l["lyrics"].as_str().unwrap_or("").trim().to_string();
                if plain.is_empty() {
                    c = c.child(ui::empty("No lyrics available for this track"));
                } else {
                    for line in plain.lines() {
                        c = c.child(text(line.to_string()).font(t(18.0, 500.0).lh(32.0)).color(white(0.5)).wrap());
                    }
                }
            }
            if let Some(p) = provider {
                c = c.child(text(format!("Lyrics provided by {p}")).font(t(11.0, 500.0)).color(white(0.4)));
            }
        }
        data::Load::Failed(_) => c = c.child(ui::empty("No lyrics available for this track")),
        data::Load::Loading => c = c.child(loading()),
    }
    c
}

/// Explore (React ExplorePage): pill rows per category with View all, plus the icon grid.
fn explore() -> El {
    let v = match ready_or(Key::Page("pages/explore".into())) { Ok(v) => v, Err(e) => return e };
    let mut c = col().w_full().gap(20.0).child(heading("Explore", theme::PAGE_TITLE));
    for (si, sec) in v["sections"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let title = sec["title"].as_str().unwrap_or("").to_string();
        let mut head = row().w_full().items_center();
        // The icon grid (PAGE_LINKS, "Shortcuts") shows no heading in the React page.
        if !title.is_empty() && sec["sectionType"].as_str() != Some("PAGE_LINKS") {
            head = head.child(heading(&title, theme::SECTION));
        }
        if let Some(path) = sec["apiPath"].as_str().filter(|p| !p.is_empty()) {
            let (path, t2) = (path.to_string(), title.clone());
            head = head.child(spacer()).child(theme::focusable(row().key(format!("ex-all-{si}")).role("button").label("View all")).h(26.0).px(8.0).items_center().cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| go(s, Page::ExploreSub(path.clone(), t2.clone())))
                .child(caps("View all", 0.6).a11y_hidden()));
        }
        let mut pills = row().w_full().gap(8.0).flex_wrap();
        for (i, it) in sec["items"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            pills = pills.child(pill_link(format!("ex-{si}-{i}"), it));
        }
        c = c.child(col().w_full().gap(10.0).child(head).child(pills));
    }
    c
}

pub(crate) fn pill_link(key: String, it: &Value) -> El {
    let label = it["title"].as_str().or(it["name"].as_str()).unwrap_or("").to_string();
    let path = it["apiPath"].as_str().unwrap_or("").to_string();
    let t2 = label.clone();
    theme::focusable(row().key(key).role("button").label(label.clone())).h(38.0).px(16.0).radius(19.0).items_center().fill(white(0.08)).hover(white(0.14)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            if !path.is_empty() {
                go(s, Page::ExploreSub(path.clone(), t2.clone()))
            }
        })
        .child(text(label).font(t(13.0, 600.0)).color(white(0.9)).a11y_hidden())
}

/// An Explore sub-page: a genre/mood page (catalogue rows) or a view-all page (more links).
///
/// As ExploreSubPage: a page of nothing but links (All Genres) is one grid of them; otherwise
/// each section is drawn as Home draws its sections (HomeSection): card rows with arrows and
/// View all, compact grids, promotions, tracks and videos; a links section among them as links.
fn explore_sub(s: &App, path: &str, title: &str) -> El {
    let v = match ready_or(Key::Page(path.to_string())) { Ok(v) => v, Err(e) => return e };
    let mut c = col().w_full().gap(28.0).child(heading(title, theme::PAGE_TITLE));
    let sections = v["sections"].as_array().cloned().unwrap_or_default();
    // isNavLinkSection
    let is_nav = |sec: &Value| {
        matches!(sec["sectionType"].as_str(), Some("PAGE_LINKS_CLOUD" | "PAGE_LINKS"))
            || sec["items"].as_array().and_then(|a| a.first()).is_some_and(|f| f.get("apiPath").is_some() && f.get("uuid").is_none() && f.get("id").is_none())
    };
    let links = |prefix: String, items: &[Value]| {
        let mut pills = row().w_full().gap(8.0).flex_wrap();
        for (i, it) in items.iter().enumerate() {
            pills = pills.child(pill_link(format!("{prefix}-{i}"), it));
        }
        pills
    };
    if sections.is_empty() {
        return c.child(ui::empty("No content found"));
    }
    if sections.iter().all(is_nav) {
        let all: Vec<Value> = sections.iter().flat_map(|s| s["items"].as_array().cloned().unwrap_or_default()).collect();
        return c.child(links("es-0".into(), &all));
    }
    let w = *s.nav.content_w;
    if w <= 0.0 {
        return c;
    }
    for (si, sec) in sections.iter().enumerate() {
        let items = sec["items"].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            continue;
        }
        let st = sec["title"].as_str().unwrap_or("").to_string();
        if is_nav(sec) {
            let mut block = col().w_full().gap(10.0);
            if !st.is_empty() && st != title {
                block = block.child(heading(&st, theme::SECTION));
            }
            c = c.child(block.child(links(format!("es-{si}"), &items)));
            continue;
        }
        // Page sections hold bare objects: give each its kind, promotions as Home's.
        let st_kind = sec["sectionType"].as_str().unwrap_or("");
        let as_kind = st_kind.strip_suffix("_LIST").map(str::to_string);
        let typed: Vec<Value> = items.iter().map(|it| {
            if st_kind.contains("PROMOTION") || data::data(it).get("artifactId").is_some() {
                json!({"type": "MAGAZINE", "data": data::data(it).clone()})
            } else {
                crate::viewall::typed(it, as_kind.as_deref())
            }
        }).collect();
        let shown = json!({"title": if st == title { String::new() } else { st }, "sectionType": st_kind, "items": typed, "apiPath": sec["apiPath"], "hasMore": sec["hasMore"]});
        if let Some(e) = crate::home::section(si, &shown, w) {
            c = c.child(e);
        }
    }
    c
}

/// Days since 1970-01-01 for a "YYYY-MM-DD..." date (Howard Hinnant's days_from_civil).
fn days(date: &str) -> Option<i64> {
    let y: i64 = date.get(0..4)?.parse().ok()?;
    let m: i64 = date.get(5..7)?.parse().ok()?;
    let d: i64 = date.get(8..10)?.parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

/// The Feed (React FeedPage): activities grouped by age, each with Play and More options.
fn feed() -> El {
    let v = match ready_or(Key::Feed) { Ok(v) => v, Err(e) => return e };
    let today = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86400) as i64;
    let now_months = month_index(today);
    let mut c = col().w_full().gap(14.0).child(heading("Feed", theme::PAGE_TITLE));
    let items = v["items"].as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        return c.child(ui::empty("Nothing here yet"));
    }
    // groupFeedByPeriod: by calendar month, in fixed order; undated entries are left out.
    for label in ["This month", "Last month", "Older"] {
        let group: Vec<(usize, &Value)> = items.iter().enumerate().filter(|(_, it)| {
            days(it["occurredAt"].as_str().unwrap_or("")).map(|d| feed_bucket(now_months - month_index(d))) == Some(label)
        }).collect();
        if group.is_empty() {
            continue;
        }
        c = c.child(heading(label, 14.0));
        for (i, it) in group {
            c = c.child(feed_row(i, it));
        }
    }
    c
}

/// Months since year 0 for a day count since 1970.
fn month_index(days: i64) -> i64 {
    // civil_from_days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    y * 12 + (m - 1)
}

fn feed_bucket(delta: i64) -> &'static str {
    match delta {
        ..=0 => "This month",
        1 => "Last month",
        _ => "Older",
    }
}

/// A feed entry (FeedRow): it opens the album or mix, plays from its Play, has More options
/// and right-click; an entry of a kind the app doesn't know is shown but inert.
fn feed_row(i: usize, it: &Value) -> El {
    let item = &it["item"];
    let title = data::name(item);
    let kind = it["kind"].as_str().unwrap_or("").to_string();
    // "Single by Artist", "EP by Artist" for releases.
    let sub = if kind == "album" {
        let artists = data::artist_of(item);
        let ty = item["type"].as_str().unwrap_or("");
        let label = if ty == "EP" { "EP".to_string() } else { ty.chars().next().map(|c| format!("{c}{}", ty[1..].to_lowercase())).unwrap_or_default() };
        if !label.is_empty() && !artists.is_empty() { format!("{label} by {artists}") } else { artists }
    } else {
        data::artist_of(item)
    };
    let id = item["id"].clone();
    let mut r = row().key(format!("feed-{i}")).w_full().h(64.0).px(10.0).gap(12.0).items_center().radius(10.0)
        .child(ui::with_image(ui::tile(ui::hue(Some(i as u64 + 60)), 48.0, i as u64 + 60, 8.0), crate::art::item_url(item, 48.0), 48.0, 8.0))
        .child(col().grow(1.0).min_w(0.0).child(text(title.clone()).font(t(14.0, 600.0)).color(white(0.95)).truncate()).child(text(sub.clone()).font(t(12.0, 400.0)).color(white(0.55)).truncate()));
    if kind != "album" && kind != "mix" {
        return r;
    }
    let open = match kind.as_str() {
        "album" => id.as_u64().map(Page::Album),
        _ => Some(Page::Mix(id.as_str().map(str::to_string).unwrap_or_else(|| id.to_string()), title.clone(), sub.clone())),
    };
    let menu_item = json!({"type": if kind == "mix" { "MIX" } else { "ALBUM" }, "data": item.clone()});
    let (t2, k2, m2) = (title.clone(), kind.clone(), menu_item.clone());
    r = r.hover(white(0.05)).cursor(rune::Cursor::Pointer)
        .on_secondary_press(move |s: &mut App, p, _| crate::interact::open_media(s, m2.clone(), crate::interact::Anchor::Point(p.x, p.y)));
    if let Some(p) = open {
        r = r.on_press(move |s: &mut App| go(s, p.clone()));
    }
    r.child(icon::<App>(&format!("feed-{i}-play"), &format!("Play {title}"), G::Play).on_press(move |_: &mut App| play_feed_item(&k2, id.clone(), &t2)))
        .child({
            let mk = format!("feed-{i}-more");
            icon::<App>(&mk, &format!("More options for {title}"), G::More)
                .on_press(move |s: &mut App| crate::interact::open_media(s, menu_item.clone(), crate::interact::Anchor::Below(mk.clone())))
        })
}

pub(crate) fn play_feed_item(kind: &str, id: Value, title: &str) {
    let (kind, title) = (kind.to_string(), title.to_string());
    sone_host_shim::async_runtime::spawn(async move {
        match kind.as_str() {
            "album" => {
                let Some(aid) = id.as_u64() else { return };
                if let Ok(p) = data::get(Key::Album(aid)).await {
                    let tracks = p["page"]["tracks"].as_array().cloned().unwrap_or_default();
                    controller::send(Cmd::PlayCollection { tracks, source: json!({"type": "album", "id": aid, "name": title}), album_mode: true, shuffle: false });
                }
            }
            "playlist" => {
                let Some(uuid) = id.as_str().map(str::to_string) else { return };
                // playableFromMedia: the whole playlist, paged in behind its first tracks.
                crate::paged::play(crate::paged::PKey::Playlist { uuid: uuid.clone(), order: None, dir: None }, json!({"type": "playlist", "id": uuid, "name": title}), false, None, |_| true);
            }
            _ => {
                let mid = id.as_str().map(str::to_string).unwrap_or_else(|| id.to_string());
                use sone_host_shim::Manager;
                let h = crate::host().clone();
                if let Ok(m) = tauri_app_lib::commands::pages::get_mix_items(h.state::<tauri_app_lib::AppState>(), mid.clone()).await {
                    let v = serde_json::to_value(m).unwrap_or(Value::Null);
                    let tracks = v["tracks"].as_array().or(v["items"].as_array()).cloned().unwrap_or_default();
                    controller::send(Cmd::PlayCollection { tracks, source: json!({"type": "mix", "id": mid, "name": title}), album_mode: false, shuffle: false });
                }
            }
        }
    });
}

/// The sign-in screen (React Login): PKCE or login-code mode, then the waiting state.
pub fn login(s: &App, v: &View) -> El {
    use controller::Login;
    let mut c = col().key("login").w(420.0).gap(14.0).items_center().child(wordmark());
    match &v.login {
        Login::PkceWaiting { url, error } => {
            let u = url.clone();
            let submit = |s: &mut App| controller::send(Cmd::CompletePkceLogin(s.nav.pkce_paste.text.clone()));
            let step = |n: &str, title: &str, sub: &str| {
                row().gap(12.0).w_full()
                    .child(col().w(28.0).h(28.0).radius(14.0).fill(white(0.12)).items_center().justify_center()
                        .child(text(n.to_string()).font(t(13.0, 700.0)).color(white(0.9)).a11y_hidden()))
                    .child(col().gap(4.0).grow(1.0)
                        .child(text(title.to_string()).font(t(14.0, 500.0)).color(white(0.95)))
                        .child(text(sub.to_string()).font(t(12.0, 400.0)).color(white(0.5)).wrap()))
            };
            let field = text_input(&s.nav.pkce_paste, t(13.0, 500.0).lh(18.0))
                .key("pkce-paste").label("Redirect URL").single_line().w_full()
                .color(white(0.95)).caret(white(0.95), white(0.25))
                .placeholder("https://tidal.com/android/login/auth?code=…", white(0.4))
                .on_edit(|s: &mut App| &mut s.nav.pkce_paste)
                .on_submit(move |s: &mut App, _| submit(s));
            c = c.child(col().w_full().gap(16.0).px(20.0).py(20.0).radius(12.0).fill(white(0.05))
                    .child(step("1", "Log in to TIDAL in your browser", "A browser window should have opened."))
                    .child(step("2", "Copy the redirect URL", "After login you'll see an \"Oops\" page. Copy the full URL."))
                    .child(step("3", "Paste it here", ""))
                    .child(field))
                .child(button::<App>("pkce-continue", "Continue", None, true).on_press(move |s: &mut App| submit(s)))
                .child(button::<App>("pkce-reopen", "Open the login page again", None, false).on_press(move |_: &mut App| {
                    let _ = std::process::Command::new("xdg-open").arg(&u).spawn();
                }))
                .child(button::<App>("login-cancel", "Cancel", None, false).on_press(|s: &mut App| {
                    s.nav.pkce_paste = TextEdit::new("");
                    controller::send(Cmd::CancelLogin)
                }));
            if let Some(e) = error {
                c = c.child(text(e.clone()).font(t(12.0, 500.0)).color(theme::danger()).wrap());
            }
        }
        Login::Waiting { code, uri } => {
            let u = uri.clone();
            let host_label = format!("Open {}", uri.split('/').take(3).collect::<Vec<_>>().join("/"));
            c = c.child(text("Enter this code at TIDAL:").font(t(14.0, 500.0)).color(white(0.7)))
                .child(text(code.clone()).font(t(34.0, 700.0).tracking(4.0)).color(white(0.97)))
                .child(button::<App>("login-open", &host_label, None, true).on_press(move |_: &mut App| {
                    let _ = std::process::Command::new("xdg-open").arg(&u).spawn();
                }))
                .child(button::<App>("login-cancel", "Cancel", None, false).on_press(|_: &mut App| controller::send(Cmd::CancelLogin)));
        }
        state if s.nav.set.custom_login.is_some() => {
            let failed = if let Login::Failed(e) = state { Some(e.as_str()) } else { None };
            c = c.w(480.0).child(crate::login::view(s, failed));
        }
        state => {
            let mode = s.nav.login_mode;
            let mtab = |key: &str, label: &str, which: LoginMode| {
                let on = mode == which;
                theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(32.0).px(14.0).radius(16.0).items_center()
                    .fill(if on { white(0.16) } else { white(0.05) }).cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut App| s.nav.login_mode = which)
                    .child(text(label.to_string()).font(t(12.0, 600.0)).color(white(0.9)).a11y_hidden())
            };
            c = c.child(row().gap(8.0).focus_group().child(mtab("lm-pkce", "PKCE", LoginMode::Pkce)).child(mtab("lm-code", "Login Code", LoginMode::Code)));
            c = match mode {
                LoginMode::Pkce => c
                    // React opens TIDAL's login in an embedded window first; with no web view
                    // here, both go through the browser (its "Open in browser instead").
                    .child(button::<App>("login-pkce", "Sign in with TIDAL", None, true).on_press(|s: &mut App| {
                        s.nav.pkce_paste = TextEdit::new("");
                        controller::send(Cmd::StartPkceLogin)
                    }))
                    .child(button::<App>("login-browser", "Open in browser instead", None, false).on_press(|s: &mut App| {
                        s.nav.pkce_paste = TextEdit::new("");
                        controller::send(Cmd::StartPkceLogin)
                    })),
                LoginMode::Code => c.child(button::<App>("login-code", "Get Login Code", None, true).on_press(|_: &mut App| controller::send(Cmd::StartDeviceLogin))),
            };
            if let Login::Failed(e) = state {
                c = c.child(text(e.clone()).font(t(12.0, 500.0)).color(theme::danger()).wrap());
                // React offers the advanced view when the embedded credentials look stale.
                if crate::login::credential_error(e) {
                    c = c.child(theme::focusable(row().key("login-creds-stale").role("button").label("Credentials may need updating — use custom credentials")).cursor(rune::Cursor::Pointer)
                        .on_press(crate::login::open)
                        .child(text("Credentials may need updating — use custom credentials").font(t(12.5, 500.0)).color(oklch(0.8, 0.13, 80.0)).a11y_hidden()));
                }
            }
            c = c.child(theme::focusable(row().key("login-custom").role("button").label("Use custom credentials")).cursor(rune::Cursor::Pointer).on_press(crate::login::open)
                .child(text("Use custom credentials").font(t(12.0, 500.0)).color(white(0.55)).a11y_hidden()));
        }
    }
    let mut frame = login_frame(c);
    if let Some(d) = crate::login::help_dialog(s) {
        frame = frame.child(d);
    }
    frame
}

/// The app's name, as the sign-in screen shows it.
fn wordmark() -> El {
    row().items_end().child(text(&crate::TITLE[..1]).font(t(56.0, 700.0)).color(white(0.95)).a11y_hidden()).child(heading(&crate::TITLE[1..], 56.0))
}

/// The sign-in screen's window: its background, `c` centred (the window buttons are the title
/// bar's, shell.rs).
fn login_frame(c: El) -> El {
    col().key("login-root").w_full().h_full().fill(theme::ground())
        .child(col().grow(1.0).w_full().items_center().justify_center().child(c))
}

/// What shows while the saved sign-in is checked at launch: the sign-in screen's frame and
/// name, with nothing to press.
pub fn splash() -> El {
    login_frame(col().key("login").w(420.0).items_center().child(wordmark()))
}


/// Suggested tracks: the current track's radio (React SuggestedTab: getMixItems(TRACK_MIX)).
/// A mix or radio (React MixPage): its tracks, with Play, Shuffle, Add, Share and More.
fn mix_page(id: &str, title: &str, sub: &str, v: &View) -> El {
    let m = match ready_or(Key::Mix(id.to_string())) { Ok(v) => v, Err(e) => return e };
    let tracks: Vec<Value> = m["tracks"].as_array().or(m["items"].as_array()).cloned().unwrap_or_default();
    // MixPage: what the link said, else what the mix says about itself; a track's radio plays
    // as a "radio" source.
    let radio = m["mixType"].as_str() == Some("TRACK_MIX");
    let fetched_title = m["title"].as_str().filter(|x| !x.is_empty()).unwrap_or(if radio { "Track Radio" } else { "Mix" }).to_string();
    let title = if title.is_empty() { fetched_title.as_str() } else { title };
    let sub = if sub.is_empty() { m["subtitle"].as_str().unwrap_or("") } else { sub };
    let source = json!({"type": if radio { "radio" } else { "mix" }, "id": id, "name": title});
    let play = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: false, shuffle: false };
    let shuf = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: false, shuffle: true };
    let (on, mid, l) = (crate::interact::is_fav(crate::interact::Fav::Mix, id), id.to_string(), title.to_string());
    let share_id = id.to_string();
    let item = json!({"type": "MIX", "data": {"id": id, "title": title}});
    let actions = row().gap(8.0).items_center()
        .child(play_buttons(v, title, play, shuf))
        .child(header_btn("mix-fav", if on { "Remove from favorites" } else { "Add to favorites" }, if on { "Added" } else { "Add" }, if on { G::HeartFilled } else { G::Heart }, on)
            .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Mix, mid.clone(), l.clone(), crate::interact::Say::Nothing)))
        .child(header_btn("mix-share", "Copy share link", "Share", G::Share, false).on_press_tx(move |_: &mut App, tx: &mut Tx| {
            tx.copy_rich(rune::ClipboardContent::new(format!("https://tidal.com/mix/{share_id}")));
            crate::interact::toast("Copied share link to clipboard".into(), crate::interact::Kind::Success);
        }))
        .child(header_btn("mix-more", "More options", "More", G::More, false)
            .on_press(move |s: &mut App| crate::interact::open_media(s, item.clone(), crate::interact::Anchor::Below("mix-more".into()))));
    let seed = id.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    let art = m["image"].as_str().filter(|u| u.starts_with("http")).map(str::to_string).or_else(|| tracks.first().and_then(|t| crate::art::item_url(t, 180.0)));
    // "MIX" / "TRACK RADIO" over the title; "N TRACKS (m:ss)" under the subtitle.
    let secs: u64 = tracks.iter().filter_map(|t| data::data(t)["duration"].as_u64()).sum();
    let (m_, s_) = (secs / 60, secs % 60);
    let count = format!("{} TRACK{}{}", tracks.len(), if tracks.len() == 1 { "" } else { "S" }, if secs > 0 { if secs >= 3600 { format!(" ({}:{:02}:{s_:02})", secs / 3600, m_ % 60) } else { format!(" ({m_}:{s_:02})") } } else { String::new() });
    let sub_el = col().gap(4.0).min_w(0.0)
        .child(text(if radio { "TRACK RADIO" } else { "MIX" }).font(t(11.0, 700.0).tracking(1.2)).color(white(0.55)))
        .child(text(sub.to_string()).font(t(13.0, 500.0)).color(white(0.6)).truncate())
        .child(text(count).font(t(11.0, 500.0).tracking(0.6)).color(white(0.45)));
    if tracks.is_empty() {
        return col().w_full().gap(16.0).child(hero_with(title, true, sub_el, seed, false, art, actions))
            .child(col().gap(6.0).child(text(if radio { "No radio tracks found" } else { "This mix is empty" }).font(t(15.0, 650.0)).color(white(0.9))));
    }
    let every = (0..tracks.len()).collect();
    let rows = track_list("mx-t", Rc::new(tracks), every, move |tracks, i| {
        let (all, src) = (tracks.clone(), source.clone());
        track_row(format!("mx-t{i}"), i + 1, &tracks[i], move |_: &mut App| {
            controller::send(Cmd::PlayFrom { tracks: all.to_vec(), index: i, source: src.clone(), album_mode: false });
        })
    });
    col().w_full().gap(16.0)
        .child(hero_with(title, true, sub_el, seed, false, art, actions))
        .child(rows)
}

fn suggested(v: &View) -> El {
    crate::drawer_tabs::suggested(v)
}

/// Credits: what is playing, the contributors by role, then the main artist's bio (React CreditsTab).
fn credits(v: &View) -> El {
    crate::drawer_tabs::credits(v)
}

#[cfg(test)]
mod feed_tests {
    use super::{days, feed_bucket, month_index};

    #[test]
    fn feed_groups_by_calendar_month() {
        let m = |d: &str| month_index(days(d).unwrap());
        assert_eq!(m("2024-01-15"), 2024 * 12);
        assert_eq!(m("2023-12-31"), 2023 * 12 + 11);
        // A December entry seen in January is last month, not older.
        assert_eq!(feed_bucket(m("2024-01-02") - m("2023-12-31")), "Last month");
        assert_eq!(feed_bucket(m("2024-03-01") - m("2024-03-31")), "This month");
        assert_eq!(feed_bucket(m("2024-03-01") - m("2023-12-01")), "Older");
    }
}
