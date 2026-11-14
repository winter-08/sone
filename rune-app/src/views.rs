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

thread_local! {
    /// The results tab each search was left on, so Back and Forward land on it again (React
    /// keeps the tab in the history entry's view state).
    static SEARCH_TABS: std::cell::RefCell<std::collections::HashMap<String, SearchTab>> = Default::default();
}

/// After Back or Forward from `left`: remember the tab a search was left on, and give the
/// search landed on its own (All Results when it had none).
fn search_tab_left(s: &mut App, left: &Page) {
    if let Page::Search(q) = left {
        let tab = *s.nav.tab;
        SEARCH_TABS.with(|m| m.borrow_mut().insert(q.clone(), tab));
    }
    if let Page::Search(q) = &*s.nav.page {
        let tab = SEARCH_TABS.with(|m| m.borrow().get(q).copied()).unwrap_or(SearchTab::All);
        if *s.nav.tab != tab {
            *s.nav.tab = tab;
        }
    }
}

pub fn go(s: &mut App, p: Page) {
    if *s.nav.page != p {
        if let Page::Search(q) = &*s.nav.page {
            let tab = *s.nav.tab;
            SEARCH_TABS.with(|m| m.borrow_mut().insert(q.clone(), tab));
        }
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
        feed_opened();
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

/// What a track row shows besides its title, artists and time (TrackList's showCover,
/// showDateAdded and onAddToCurrentPlaylist).
#[derive(Clone)]
pub(crate) struct Look {
    /// The 40 pt album cover (a video's picture) before the title; not on an album's own page.
    pub cover: bool,
    /// The Date Added column (your own playlists, Loved Tracks).
    pub date: bool,
    /// More options, Add to playlist and the heart.
    pub actions: bool,
    /// "Add to this playlist" in place of Add to playlist (a playlist's recommended tracks).
    pub add_here: Option<Rc<dyn Fn(&mut App)>>,
}

impl Default for Look {
    fn default() -> Self {
        Look { cover: true, date: false, actions: true, add_here: None }
    }
}

/// A row's artists (TrackList's TrackArtists): each a link to the artist's page, comma
/// separated; "Unknown Artist" with none.
pub(crate) fn artist_links(key: &str, d: &Value, color: rune::Color) -> El {
    artist_links_in(key, d, t(12.0, 400.0), color)
}

/// [`artist_links`] in `font`: the player bar, the queue and Suggested rows use it too.
pub(crate) fn artist_links_in(key: &str, d: &Value, font: rune::TextStyle, color: rune::Color) -> El {
    let list: Vec<(u64, String)> = match d["artists"].as_array().filter(|a| !a.is_empty()) {
        Some(a) => a.iter().filter_map(|x| Some((x["id"].as_u64().unwrap_or(0), x["name"].as_str()?.to_string()))).collect(),
        None => d["artist"]["name"].as_str().map(|n| vec![(d["artist"]["id"].as_u64().unwrap_or(0), n.to_string())]).unwrap_or_default(),
    };
    if list.is_empty() {
        return text("Unknown Artist").font(font).color(color).truncate();
    }
    let n = list.len();
    let mut r = row().min_w(0.0).items_center().clip_rounded(0.0);
    for (i, (aid, name)) in list.into_iter().enumerate() {
        let shown = if i + 1 < n { format!("{name},") } else { name.clone() };
        let label = text(shown).font(font).color(color).truncate();
        let gap = if i + 1 < n { 4.0 } else { 0.0 };
        r = r.child(if aid == 0 {
            label.shrink().pr(gap)
        } else {
            // the name brightens under the pointer, as React's hover:text-primary
            theme::focusable(el().key(format!("{key}-ar{i}")).role("link").label(name)).shrink().min_w(0.0).mr(gap).radius(3.0).hover_group().cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| go(s, Page::Artist(aid)))
                .child(label.group_color(white(0.95)).a11y_hidden())
        });
    }
    r
}

/// A row's 40 pt cover: the album's, or a video's picture. Decorative; it fills in place when
/// the image lands (art.rs), so the list isn't built again for it. (A video says so in its
/// title's VIDEO badge.)
fn row_cover(d: &Value) -> El {
    let video = d["itemType"].as_str() == Some("video");
    let url = if video { crate::video_ui::image_id(d).and_then(|i| crate::art::cover(Some(i), 40.0)) } else { crate::art::cover(d["album"]["cover"].as_str(), 40.0) };
    ui::with_image(el().size(40.0, 40.0).radius(4.0).clip_rounded(4.0).fill(white(0.08)), url, 40.0, 4.0).a11y_hidden()
}

/// A track row (TrackList.tsx). A single click plays it (there's no double-click play in React
/// either); hovering swaps the number for a play icon; right-click opens the track menu at the
/// pointer. Its actions, after the time: More options (shown on hover; the same menu, below it),
/// Add to playlist (the playlist picker, below it) and the heart (toggled optimistically, filled
/// in the accent when loved). `pl` places it in the user's own playlist, for "Remove from
/// playlist".
pub(crate) fn track_row_with(key: String, n: usize, tr: &Value, actions: bool, pl: Option<crate::interact::InPlaylist>, on_play: impl Fn(&mut App) + 'static) -> El {
    track_row_look(key, n, tr, Look { actions, ..Look::default() }, pl, on_play)
}

/// [`track_row_with`] showing what `look` asks for.
pub(crate) fn track_row_look(key: String, n: usize, tr: &Value, look: Look, pl: Option<crate::interact::InPlaylist>, on_play: impl Fn(&mut App) + 'static) -> El {
    use crate::interact::{self, Anchor, Fav, Say};
    let actions = look.actions;
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
    let mut r = r.child(number);
    if look.cover {
        r = r.child(row_cover(d));
    }
    let mut r = r
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(title_row)
                .child(artist_links(&key, d, white(0.55))),
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
    // TrackList's Date Added column
    if look.date {
        let added = d["dateAdded"].as_str().map(|s| crate::formats::date_added(s, crate::formats::now())).unwrap_or_default();
        r = r.child(text(added).font(t(12.0, 400.0)).color(white(0.55)).truncate().w(104.0));
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
            .child(theme::focusable(el().key(more_key.clone()).role("button").label("More options")).help("More options").size(36.0, 36.0).radius(18.0).items_center().justify_center()
                .press(white(0.14)).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| interact::open_track(s, t1.clone(), pl.clone(), Anchor::Below(mk.clone())))
                .child(glyph(G::More, 18.0, white(0.75)).group_opacity(0.3, 1.0)))
            .child(match look.add_here.clone() {
                // a recommended track: straight into the playlist on show
                Some(add) => ui::icon_button::<App>(&add_key, "Add to this playlist", G::ListPlus, 18.0, white(0.75)).on_press(move |s: &mut App| add(s)),
                None => icon::<App>(&add_key, "Add to playlist", G::Plus).on_press(move |s: &mut App| interact::open_add_below(s, t2.clone(), ak.clone())),
            })
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
    track_list_keyed(format!("{prefix}rows"), prefix, tracks, shown, row)
}

/// [`track_list`] keyed `list_key`: several lists over one set of tracks (an album's volumes)
/// keep their rows' keys, `{prefix}{i}`, unique.
pub(crate) fn track_list_keyed<P: std::ops::Deref<Target = Vec<Value>> + 'static>(list_key: String, prefix: &'static str, tracks: P, shown: Vec<usize>, row: impl Fn(&P, usize) -> El + 'static) -> El {
    let shown = Rc::new(shown);
    // The keys change only with the rows shown, so layout needn't ask for every key each commit.
    let keys_version = {
        use std::hash::{BuildHasher, BuildHasherDefault};
        BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(&*shown)
    };
    let keys = shown.clone();
    col().key(list_key).w_full().gap(2.0).growth(rune::GrowthSpec::default()).instant_changes()
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
        "MIX" => d["id"].as_str().or(d["mixId"].as_str()).map(|id| {
            if let Some(url) = crate::art::item_url(item, 180.0) {
                hint_mix_image(id, url);
            }
            Page::Mix(id.to_string(), data::name(item), data::artist_of(item))
        }),
        _ => None,
    }
}

thread_local! {
    /// The picture a mix was opened under (its card's, or the artist's for Artist radio), as
    /// MixPage's mixInfo.image: the mix's own response often has none.
    static MIX_IMAGES: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
}

pub(crate) fn hint_mix_image(id: &str, url: String) {
    MIX_IMAGES.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 500 {
            m.clear();
        }
        m.insert(id.to_string(), url);
    });
}

fn mix_image(id: &str) -> Option<String> {
    MIX_IMAGES.with(|m| m.borrow().get(id).cloned())
}

pub(crate) fn cards_for(prefix: &str, items: &[Value]) -> Vec<El> {
    cards_with(prefix, items, data::artist_of)
}

/// [`cards_for`] with each card's subtitle from `sub`.
pub(crate) fn cards_with(prefix: &str, items: &[Value], sub: impl Fn(&Value) -> String) -> Vec<El> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| {
            let page = item_page(it)?;
            let round = data::kind(it) == "ARTIST";
            let seed = data::data(it)["id"].as_u64().unwrap_or(i as u64 + 1);
            Some(card(format!("{prefix}-{i}"), &data::name(it), &sub(it), seed, round, crate::art::item_url(it, 132.0), Some(it.clone()), page))
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

/// A loading placeholder of another shape (skeleton.rs) was built: the pane is busy too.
pub(crate) fn note_loading() {
    LOADING.set(true);
}

/// Whether a `loading()` placeholder was built since the last call: a pane that built its
/// content between two calls is still loading, and tells assistive technology so (`El::busy`).
pub(crate) fn took_loading() -> bool {
    LOADING.replace(false)
}

/// A page's data, or what shows until it's there: its skeleton (skeleton.rs), or, when the
/// fetch failed, a short error with Retry (which forgets the entry, so the redraw fetches it
/// again).
pub(crate) fn ready_or(key: Key) -> Result<Value, El> {
    match data::want(key.clone()) {
        data::Load::Ready(v) => Ok(v),
        data::Load::Loading => Err(crate::skeleton::page(&key)),
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
    /// A wide window's docked drawer also gets NowPlayingDrawer's cover column, this wide, on
    /// the tabs' left (0: none; the tabs' header then shows a small cover instead).
    pub drawer_cover: f32,
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
        // (in steps, so a resize re-runs the view only at the bands; the page keeps ~640 pt)
        let room = w - sidebar_w - 360.0 - 640.0;
        let drawer_cover = [460.0, 380.0, 300.0].into_iter().find(|c| room >= *c).unwrap_or(0.0);
        Shell { sidebar_w, drawer_docked, drawer_w, drawer_cover, bar_source: w >= 1300.0, bar_badge: w >= 1150.0 }
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
                remember_search(s, &q);
                go(s, Page::Search(q));
                *s.nav.tab = SearchTab::All;
            }
        });
    // 360 pt wide when there's room; in a narrow window it gives way before anything else does.
    let mut bar = row().key("search-bar").role("search").w(360.0).shrink().min_w(140.0).h(40.0).px(14.0).gap(8.0).radius(20.0).items_center().fill(white(0.08)).child(glyph(G::Search, 16.0, white(0.55))).child(field);
    // SearchBar's X: empties the field and keeps it focused
    if !s.nav.search.text.is_empty() {
        bar = bar.child(ui::icon_button::<App>("search-clear", "Clear search", G::Close, 14.0, white(0.5)).size(24.0, 24.0).preserve_focus_on_press(true)
            .on_press(|s: &mut App| {
                set_search_text(s, "", true);
                s.nav.search_open = false;
                s.nav.search_sel = None;
            }));
    }
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
                search_tab_left(s, &cur);
                s.nav.fwd.push(cur);
                s.nav.filter = TextEdit::new("");
                if *s.nav.page == Page::Feed {
                    feed_opened();
                }
            }
        }))
        .child(step("forward", "Go forward", G::Forward, !s.nav.fwd.is_empty(), |s| {
            if let Some(p) = s.nav.fwd.pop() {
                let cur = std::mem::replace(&mut *s.nav.page, p);
                search_tab_left(s, &cur);
                s.nav.back.push(cur);
                s.nav.filter = TextEdit::new("");
                if *s.nav.page == Page::Feed {
                    feed_opened();
                }
            }
        }))
        .child(spacer())
        .child(bar)
        .child(crate::profile::account_button(crate::profile::avatar()).on_press(|s: &mut App| {
            s.nav.menu_open = !s.nav.menu_open;
            if s.nav.menu_open {
                data::want(Key::Profile);
            }
        }))
}

/// The suggestions dropdown under the search field (React SearchBar).
/// One entry of the search dropdown, in display order.
#[derive(Clone, Debug)]
enum Entry {
    /// A recent search (the field is empty): searches it again; removable.
    Recent(String),
    /// A text suggestion: fills the search bar and searches. `true` for one from the server's
    /// search history (a clock, and removable).
    Text(String, bool),
    /// A direct hit: an album, artist or playlist page, with the item its menu acts on.
    Hit(String, Page, HitLook, Value),
    /// A direct hit on a track (the full track): its row opens the album, its picture plays it.
    Track(String, Value, HitLook),
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
    ai: bool,
}

/// How many recent searches are kept, and shown under an empty field (SearchBar MAX_HISTORY).
const HISTORY_KEEP: usize = 10;
const HISTORY_SHOWN: usize = 5;

/// `q` first in the recent searches (SearchBar addToHistory: one entry per query, whatever its
/// case), the oldest past ten dropped.
pub(crate) fn push_history(h: &mut Vec<String>, q: &str) {
    let q = q.trim();
    if q.is_empty() {
        return;
    }
    h.retain(|x| x.to_lowercase() != q.to_lowercase());
    h.insert(0, q.to_string());
    h.truncate(HISTORY_KEEP);
}

/// Records a search the user ran (Enter, a suggestion, a recent search, View all), saved with
/// the library's other preferences (sone.search-history).
fn remember_search(s: &mut App, q: &str) {
    let mut h = s.nav.coll.ui.search_history.clone();
    push_history(&mut h, q);
    if h != s.nav.coll.ui.search_history {
        s.nav.coll.ui.search_history = h;
        crate::coll::save(&s.nav.coll.ui);
    }
}

thread_local! {
    /// Server history suggestions removed this session: TIDAL keeps them, but the dropdown
    /// leaves them out from then on (React's Remove only dropped them from its own history).
    static HIDDEN: std::cell::RefCell<std::collections::HashSet<String>> = Default::default();
}

fn forget_search(s: &mut App, q: &str) {
    HIDDEN.with(|h| h.borrow_mut().insert(q.to_lowercase()));
    if s.nav.coll.ui.search_history.iter().any(|x| x == q) {
        s.nav.coll.ui.search_history.retain(|x| x != q);
        crate::coll::save(&s.nav.coll.ui);
    }
}

fn dropdown_entries(s: &App, q: &str) -> Vec<Entry> {
    if q.is_empty() {
        return s.nav.coll.ui.search_history.iter().take(HISTORY_SHOWN).map(|h| Entry::Recent(h.clone())).collect();
    }
    let Some(v) = data::peek(&Key::Suggest(q.to_string())) else { return vec![] };
    let hidden = |t: &str| HIDDEN.with(|h| h.borrow().contains(&t.to_lowercase()));
    let mut out: Vec<Entry> = v["textSuggestions"].as_array().cloned().unwrap_or_default().iter()
        .filter_map(|x| x["query"].as_str().filter(|t| !hidden(t)).map(|t| Entry::Text(t.to_string(), x["source"].as_str() == Some("history"))))
        .collect();
    let mut hits = 0;
    for hit in v["directHits"].as_array().cloned().unwrap_or_default() {
        let title = hit["title"].as_str().or(hit["name"].as_str()).unwrap_or("").to_string();
        let kind = hit["hitType"].as_str().unwrap_or("");
        let artist = hit["artistName"].as_str().or(hit["track"]["artists"][0]["name"].as_str()).unwrap_or("");
        let with_artist = |k: &str| format!("{k} · {}", if artist.is_empty() { "Unknown" } else { artist });
        let look = HitLook {
            sub: match kind {
                "TRACKS" => with_artist("Track"),
                "ALBUMS" => with_artist("Album"),
                "ARTISTS" => "Artist".into(),
                _ => match hit["numberOfTracks"].as_u64().filter(|n| *n > 0) {
                    Some(n) => format!("Playlist · {n} tracks"),
                    None => "Playlist".into(),
                },
            },
            img: crate::art::item_url(&hit, 48.0).or_else(|| crate::art::cover(hit["albumCover"].as_str(), 48.0)),
            round: kind == "ARTISTS",
            seed: hit["id"].as_u64().unwrap_or(title.len() as u64 + 1),
            ai: hit["ai"].as_bool() == Some(true),
        };
        let (page, item) = match kind {
            "TRACKS" => {
                let id = hit["id"].as_u64().unwrap_or(0);
                let mut track = if hit["track"].is_object() { hit["track"].clone() } else { crate::interact::json_of_track(id, &title, artist) };
                if track["album"]["id"].is_null()
                    && let Some(a) = hit["albumId"].as_u64()
                {
                    track["album"] = json!({"id": a, "title": hit["albumTitle"], "cover": hit["albumCover"]});
                }
                out.push(Entry::Track(title, track, look));
                hits += 1;
                continue;
            }
            "ALBUMS" => (hit["id"].as_u64().map(Page::Album), json!({"type": "ALBUM", "data": {"id": hit["id"], "title": title, "cover": hit["cover"], "artist": {"name": artist}}})),
            "ARTISTS" => (hit["id"].as_u64().map(Page::Artist), json!({"type": "ARTIST", "data": {"id": hit["id"], "name": title, "picture": hit["picture"]}})),
            "PLAYLISTS" => (hit["uuid"].as_str().map(|u| Page::Playlist(u.to_string())), json!({"type": "PLAYLIST", "data": {"uuid": hit["uuid"], "title": title, "image": hit["image"]}})),
            _ => (None, Value::Null),
        };
        if let Some(p) = page {
            out.push(Entry::Hit(title, p, look, item));
            hits += 1;
        }
    }
    // SearchBar's View all closes the direct hits; with none there's nothing to view.
    if hits > 0 {
        out.push(Entry::ViewAll);
    }
    out
}

/// Set the search bar to `text`, caret at the end, keeping focus if it had it.
fn set_search_text(s: &mut App, text: &str, focused: bool) {
    let mut e = TextEdit::new(text);
    e.focused = focused;
    s.nav.search = e;
}

fn search_for(s: &mut App, q: String) {
    set_search_text(s, &q, false);
    remember_search(s, &q);
    go(s, Page::Search(q));
    *s.nav.tab = SearchTab::All;
}

fn pick(s: &mut App, e: Entry) {
    s.nav.search_open = false;
    s.nav.search_sel = None;
    match e {
        Entry::Recent(q) | Entry::Text(q, _) => search_for(s, q),
        Entry::Hit(_, p, ..) => go(s, p),
        // SearchBar: a track's row opens its album (its picture plays it)
        Entry::Track(_, track, _) => {
            if let Some(a) = track["album"]["id"].as_u64() {
                go(s, Page::Album(a));
            }
        }
        Entry::ViewAll => {
            let q = s.nav.search.text.trim().to_string();
            remember_search(s, &q);
            go(s, Page::Search(q));
            *s.nav.tab = SearchTab::All;
        }
    }
}

/// Keys in the search field: ↑/↓ move through the dropdown, Tab completes the text to the
/// highlighted (or first) suggestion, Enter opens the highlighted entry (or searches, through
/// `on_submit`), Escape closes the dropdown and leaves the field (SearchBar.tsx has Enter and
/// Escape; arrows and Tab go further).
fn search_key(s: &mut App, ev: &rune::KeyEvent, tx: &mut Tx) -> bool {
    // bound Ctrl and Alt shortcuts run from the search field too (keys.rs)
    if crate::keys::field_key(s, ev, tx) {
        return true;
    }
    let q = s.nav.search.text.trim().to_string();
    let entries = if q.is_empty() || s.nav.search_open { dropdown_entries(s, &q) } else { vec![] };
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
                Entry::Recent(t) | Entry::Text(t, _) | Entry::Hit(t, ..) | Entry::Track(t, ..) => Some(t.clone()),
                Entry::ViewAll => None,
            }).or_else(|| entries.iter().find_map(|e| if let Entry::Text(t, _) = e { Some(t.clone()) } else { None }));
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
        // SearchBar: Escape closes the dropdown and blurs the field
        rune::Key::Escape if s.nav.search_open || q.is_empty() => {
            s.nav.search_open = false;
            s.nav.search_sel = None;
            s.nav.search.focused = false;
            true
        }
        _ => false,
    }
}

/// A dropdown row's remove button (SearchBar's X, "Remove").
fn remove_btn(key: String, q: &str) -> El {
    let q = q.to_string();
    ui::icon_button::<App>(&key, &format!("Remove {q} from search history"), G::Close, 14.0, white(0.5)).help("Remove").size(28.0, 28.0).preserve_focus_on_press(true)
        .on_press(move |s: &mut App| {
            forget_search(s, &q);
            s.nav.search_sel = None;
        })
}

pub fn dropdown(s: &App) -> Option<El> {
    let q = s.nav.search.text.trim().to_string();
    // Open while the field has focus: the recent searches while it's empty, the suggestions
    // while typing (until a pick, Enter or Escape closes them). Rows keep the field's focus.
    if !s.nav.search.focused || (!q.is_empty() && !s.nav.search_open) {
        return None;
    }
    let entries = dropdown_entries(s, &q);
    // Peek only: the debounce in App::poll decides when to fetch (rendering mid-typing must not).
    // A failed fetch has answered: "No results found", as React's catch empties the hits.
    let pending = !q.is_empty() && !matches!(data::peek_load(&Key::Suggest(q.clone())), Some(data::Load::Ready(_) | data::Load::Failed(_)));
    if q.is_empty() && entries.is_empty() {
        return None;
    }
    let sel = s.nav.search_sel;
    let mut list = col().w_full().padding(8.0).gap(2.0);
    if q.is_empty() {
        list = list.child(row().h(26.0).px(10.0).items_center().child(caps("Recent searches", 0.45)));
    }
    let mut hit_n = 0;
    let mut sug_n = 0;
    let mut has_hits = false;
    for (i, e) in entries.iter().enumerate() {
        let on = Some(i) == sel;
        let ec = e.clone();
        let fill = if on { white(0.1) } else { rune::Color::TRANSPARENT };
        let base = |key: String, label: &str, h: f32| row().key(key).role("button").label(label).h(h).px(10.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center()
            .fill(fill).hover(white(0.08)).cursor(rune::Cursor::Pointer)
            .preserve_focus_on_press(true);
        let el_ = match e {
            Entry::Recent(t) | Entry::Text(t, _) => {
                sug_n += 1;
                let recent = matches!(e, Entry::Recent(_)) || matches!(e, Entry::Text(_, true));
                let k = format!("sug-{}", sug_n - 1);
                let mut r = row().w_full().gap(4.0).items_center()
                    .child(base(k.clone(), t, 34.0).grow(1.0).min_w(0.0)
                        .on_press(move |s: &mut App| pick(s, ec.clone()))
                        .child(glyph(if recent { G::Clock } else { G::Search }, 14.0, white(0.5)))
                        .child(text(t.clone()).font(t_(13.0, 500.0)).color(white(0.9)).truncate().a11y_hidden()));
                if recent {
                    r = r.child(remove_btn(format!("{k}-x"), t));
                }
                r
            }
            Entry::Track(t, track, look) => {
                hit_n += 1;
                has_hits = true;
                let (tk, mk) = (format!("hit-{}", hit_n - 1), format!("hit-{}-more", hit_n - 1));
                let blocked = crate::content::blocked(track);
                let (t1, t2, t3) = (track.clone(), track.clone(), track.clone());
                // the picture plays it alone, keeping the queue (playSingle)
                let thumb = el().key(format!("{tk}-play")).role("button").label(format!("Play {t}")).help("Play").cursor(rune::Cursor::Pointer).hover_group().preserve_focus_on_press(true)
                    .on_press(move |s: &mut App| {
                        s.nav.search_open = false;
                        if !crate::content::blocked(&t1) {
                            controller::send(Cmd::PlaySingle(t1.clone()));
                        }
                    })
                    .child(hit_thumb(look))
                    .child(el().absolute(0.0, 0.0).size(48.0, 48.0).radius(4.0).fill(theme::black(0.4)).items_center().justify_center().group_opacity(0.0, 1.0)
                        .child(glyph(G::Play, 16.0, rune::color::hex("#ffffff").unwrap_or(white(1.0)))));
                // Only a track with an album opens something (React returns without albumId):
                // without one the title is plain text, not a button that does nothing.
                let opens = track["album"]["id"].as_u64().is_some();
                let title = text(t.clone()).font(t_(13.0, 600.0)).color(white(0.95)).truncate().shrink();
                let mut title_row = row().min_w(0.0).gap(6.0).items_center().child(if opens { title.a11y_hidden() } else { title });
                if let Some(b) = crate::content::badges_for(track) {
                    title_row = title_row.child(b);
                }
                // (the title is the row's button, beside the picture's and More's, not around them)
                let mut body = row().grow(1.0).min_w(0.0).h_full().items_center().preserve_focus_on_press(true)
                    .child(col().grow(1.0).min_w(0.0).gap(2.0).child(title_row).child(hit_sub(look)));
                if opens {
                    body = body.role("button").label(t.as_str()).cursor(rune::Cursor::Pointer).on_press(move |s: &mut App| pick(s, ec.clone()));
                }
                let r = row().key(tk).h(60.0).px(10.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center().fill(fill).hover(white(0.08))
                    .hover_group()
                    .on_secondary_press(move |s: &mut App, p, _| crate::interact::open_track(s, t2.clone(), None, crate::interact::Anchor::Point(p.x, p.y)))
                    .child(thumb)
                    .child(body)
                    .child(icon::<App>(&mk, "More options", G::More).preserve_focus_on_press(true)
                        .on_press(move |s: &mut App| crate::interact::open_track(s, t3.clone(), None, crate::interact::Anchor::Below(mk.clone()))));
                if blocked { r.opacity(0.5) } else { r }
            }
            Entry::Hit(t, _, look, item) => {
                hit_n += 1;
                has_hits = true;
                let k = format!("hit-{}", hit_n - 1);
                let mk = format!("{k}-more");
                let (i1, i2, mk2) = (item.clone(), item.clone(), mk.clone());
                let mut head = row().min_w(0.0).gap(6.0).items_center().child(heading(t, 13.0).truncate().shrink());
                if look.ai {
                    head = head.child(crate::content::ai_badge());
                }
                row().key(k).role("group").h(60.0).px(10.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center().cursor(rune::Cursor::Pointer)
                    .fill(fill).hover(white(0.08)).preserve_focus_on_press(true)
                    .on_press(move |s: &mut App| pick(s, ec.clone()))
                    .on_secondary_press(move |s: &mut App, p, _| crate::interact::open_media(s, i1.clone(), crate::interact::Anchor::Point(p.x, p.y)))
                    .child(hit_thumb(look))
                    .child(col().grow(1.0).min_w(0.0).gap(2.0).child(head).child(hit_sub(look)))
                    .child(icon::<App>(&mk, "More options", G::More).preserve_focus_on_press(true)
                        .on_press(move |s: &mut App| crate::interact::open_media(s, i2.clone(), crate::interact::Anchor::Below(mk2.clone()))))
            }
            Entry::ViewAll => base("view-all".into(), "View all results", 36.0).justify_center()
                .on_press(move |s: &mut App| pick(s, ec.clone()))
                .child(text("View all results").font(t_(12.0, 650.0)).color(accent()).a11y_hidden()),
        };
        list = list.child(el_);
    }
    // While the suggestions load, a spinner; then "No results found" without direct hits.
    if pending {
        list = list.child(row().w_full().h(56.0).justify_center().items_center().role("status").label("Searching").child(glyph(G::Loader, 18.0, accent())));
    } else if !q.is_empty() && !has_hits {
        list = list.child(row().w_full().h(56.0).justify_center().items_center().child(text("No results found").font(t_(13.0, 500.0)).color(white(0.45))));
    }
    // SearchBar.tsx: 420 px wide, right-aligned 8 px under the search field, at most 70vh tall
    // and scrolling. `anchored_fit` sizes it to the room below (or above) the field.
    Some(
        theme::overlay_surface(col().key("suggest").w(420.0).anchored_fit("search-bar", rune::Side::Bottom, rune::Align::End, 8.0), theme::Overlay::Menu)
            .clip_rounded(theme::RADIUS_OVERLAY)
            .child(col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list)),
    )
}

/// A direct hit's picture in the dropdown (48 pt): round for an artist. Decorative.
fn hit_thumb(l: &HitLook) -> El {
    let h = ui::hue(Some(l.seed));
    if l.round {
        ui::with_image(ui::art(h, 48.0, l.seed, 0.0), l.img.clone(), 48.0, 24.0)
    } else {
        ui::with_image(ui::tile(h, 48.0, l.seed, 4.0), l.img.clone(), 48.0, 4.0)
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

/// Whether the sidebar's `page` entry is lit on `cur`: Explore stays lit on its sub-pages (as
/// React's Sidebar treats explore and explorePage alike).
fn nav_on(cur: &Page, page: &Page) -> bool {
    cur == page || (*page == Page::Explore && matches!(cur, Page::ExploreSub(..)))
}

/// A sidebar entry's icon; the Feed's with React's unread dot while the feed has unseen items.
fn nav_glyph(g: G, on: bool) -> El {
    let icon = glyph(g, 18.0, white(if on { 0.95 } else { 0.7 }));
    if g != G::Bell || !feed_unseen() {
        return icon;
    }
    el().size(18.0, 18.0).child(icon)
        .child(el().absolute(12.0, -1.0).size(8.0, 8.0).radius(4.0).fill(accent()).border(2.0, theme::sidebar_bg()).a11y_hidden())
}

/// The feed has activities not seen yet (its unseenCount; opening the Feed clears it).
fn feed_unseen() -> bool {
    data::peek(&Key::Feed).is_some_and(|f| f["unseenCount"].as_u64().unwrap_or(0) > 0)
}

/// The Feed was opened, by any way there (a link, Back or Forward): FeedPage fetches the feed
/// again on every visit and marks it seen, which clears the unread dot at once. The feed shown
/// stays until the new one lands.
fn feed_opened() {
    if let Some(mut f) = data::peek(&Key::Feed) {
        if f["unseenCount"].as_u64().unwrap_or(0) > 0 {
            f["unseenCount"] = json!(0);
            data::replace(Key::Feed, f);
        }
    }
    let h = crate::host().clone();
    let again = data::peek(&Key::Feed).is_some();
    sone_host_shim::async_runtime::spawn(async move {
        use sone_host_shim::Manager;
        let st = || h.state::<tauri_app_lib::AppState>();
        // the feed first (from the backend's cache when it's fresh), then the seen mark, which
        // drops that cache for the next visit
        if again
            && let Ok(f) = tauri_app_lib::commands::feed::get_feed(st(), data::user_id()).await
            && let Ok(mut v) = serde_json::to_value(f)
        {
            v["unseenCount"] = json!(0);
            data::replace(Key::Feed, v);
        }
        let _ = tauri_app_lib::commands::feed::mark_feed_seen(st(), data::user_id()).await;
    });
}

pub fn sidebar(s: &App, w: f32) -> El {
    let nav_btn = |key: &str, label: &str, g: G, page: Page| {
        let on = nav_on(&s.nav.page, &page);
        theme::focusable_row(row().key(key.to_string()).role("button").label(label)).h(40.0).px(12.0).gap(12.0).radius(10.0).items_center()
            .fill(if on { white(0.08) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer).help(label)
            .on_press(move |s: &mut App| go(s, page.clone()))
            .child(nav_glyph(g, on))
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
            let on = nav_on(&s.nav.page, &page);
            theme::focusable(el().key(key.to_string()).role("button").label(label)).size(40.0, 40.0).radius(10.0).items_center().justify_center()
                .fill(if on { white(0.08) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer).help(label)
                .on_press(move |s: &mut App| go(s, page.clone()))
                .child(nav_glyph(g, on))
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
        // (white on the gradient in every theme)
        .child(glyph(G::HeartFilled, heart, c("#ffffff")))
}

pub(crate) fn lib_row(key: &str, label: &str, title: &str, sub: &str, seed: u64, img: Option<String>, menu: Option<Value>, page: Option<Page>, ai: bool) -> El {
    let art = if key == "lib-loved" { loved_art(40.0, 6.0, 15.0) } else { ui::with_image(ui::tile(ui::hue(Some(seed)), 40.0, seed, 6.0), img, 40.0, 6.0) };
    let mut r = theme::focusable_row(row().key(key.to_string()).role("button").label(label)).w_full().h(52.0).px(8.0).gap(10.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer).help(title)
        .child(art)
        .child(col().min_w(0.0).a11y_hidden()
            // (Sidebar: an AI album has the AI badge after its title)
            .child(row().min_w(0.0).gap(6.0).items_center().child(text(title.to_string()).font(t(13.0, 600.0)).color(white(0.92)).truncate().shrink()).child(crate::content::badges(false, ai).unwrap_or_else(el)))
            .child(text(sub.to_string()).font(t(11.0, 400.0)).color(white(0.55)).truncate()));
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
    // A video's heart is a video favourite (PlayerBar FavoriteButton); More options opens the
    // track menu for whatever is current, a video included (its items know it's a video).
    let (fav, id, title) = match &v.video {
        Some(vi) => (Fav::Video, vi.id.to_string(), vi.title.clone()),
        None => (Fav::Track, c.id.to_string(), c.title.clone()),
    };
    let loved = interact::is_fav(fav, &id);
    let cur = (*c.track).clone();
    row().gap(2.0).items_center()
        .child(heart(loved).on_press(move |_: &mut App| interact::toggle_fav(fav, id.clone(), title.clone(), Say::Nothing)))
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
    // TrackArtists: every artist is its own link
    let foot = if is_video {
        artist
    } else {
        artist_links_in("bar-artist", &cur.track, t(12.0, 400.0), white(0.6))
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
            r = r.child(quality_badge(b));
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
        r = r.child(icon::<App>("full", "Fullscreen player", G::Maximize).on_press_tx(move |s: &mut App, tx: &mut Tx| crate::video_ui::open_fullscreen(s, tx, open)));
    }
    r
}

/// QualityBadge: the stream's detail ("24-BIT 96kHz FLAC") small over the tier's pill, which is
/// the accent for Max, the accent at 70% for Lossless and a neutral fill otherwise. Its name is
/// the whole text; a press opens the signal path panel ("Show signal path").
fn quality_badge(b: &str) -> El {
    let (detail, tier) = ["HI-RES LOSSLESS", "LOSSLESS", "HIGH"].into_iter()
        .find_map(|t| b.strip_suffix(t).map(|d| (d.trim_end(), t)))
        .unwrap_or(("", b));
    let (fill, ink) = match tier {
        "HI-RES LOSSLESS" => (accent(), theme::ink()),
        "LOSSLESS" => (accent().alpha(0.7), theme::ink()),
        _ => (white(0.14), white(0.95)),
    };
    let pill = row().h(15.0).px(7.0).radius(4.0).items_center().fill(fill)
        .child(text(tier).font(t(9.0, 900.0).tracking(0.9)).color(ink));
    let mut c = col().items_end().gap(2.0);
    if !detail.is_empty() {
        c = c.child(text(detail.to_string()).font(t(9.0, 500.0).tracking(0.4)).color(white(0.45)).a11y_hidden());
    }
    theme::focusable(c.key("badge").role("button").label(b.to_string())).help("Show signal path").px(4.0).py(2.0).radius(4.0)
        .hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .on_press(crate::signal_path::open)
        .child(pill.a11y_hidden())
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
        Page::Explore => explore(s),
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

/// Whether what plays came from `source` (a play source's JSON): by its type and id, as
/// SourcePlayButton matches, not by its name (an artist and its popular tracks, or two
/// playlists of one title, share a name).
pub(crate) fn plays_from(v: &View, source: &Value) -> bool {
    v.current.is_some() && v.source.as_ref().is_some_and(|s| same_source(&s.kind, &s.id, source))
}

/// A source's kind and id (as text) against a play source's JSON.
fn same_source(kind: &str, id: &str, source: &Value) -> bool {
    let sid = match &source["id"] {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        x => x.to_string(),
    };
    source["type"].as_str() == Some(kind) && sid == id
}

pub(crate) fn play_label(v: &View, source: &Value) -> &'static str {
    if plays_from(v, source) {
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
    r
}

/// Under an album's title (AlbumView's header): the "ALBUM" eyebrow, the main artist's picture
/// beside the artists, "N TRACKS (h:mm:ss)", and the year with the AI and quality badges.
fn album_sub(a: &Value, tracks: &[Value]) -> El {
    let d = data::data(a);
    let mut by = row().gap(8.0).items_center().min_w(0.0);
    let pic = d["artists"][0]["picture"].as_str().or(d["artist"]["picture"].as_str()).filter(|p| !p.is_empty());
    if let Some(p) = pic {
        by = by.child(ui::with_image(el().size(24.0, 24.0).radius(12.0).clip_rounded(12.0).fill(white(0.08)), crate::art::artist_picture(&json!({"picture": p}), 24.0), 24.0, 12.0).a11y_hidden());
    }
    let mut c = col().gap(6.0).min_w(0.0)
        .child(text("ALBUM").font(t(11.0, 700.0).tracking(1.2)).color(white(0.55)))
        .child(by.child(album_byline(a)));
    let secs = d["duration"].as_u64().unwrap_or_else(|| tracks.iter().filter_map(|t| data::data(t)["duration"].as_u64()).sum());
    if let Some(n) = d["numberOfTracks"].as_u64() {
        c = c.child(text(crate::formats::track_count(n as usize, secs)).font(t(11.0, 500.0).tracking(0.6)).color(white(0.45)));
    } else if secs > 0 {
        c = c.child(text(format!("({})", crate::formats::total_duration(secs))).font(t(11.0, 500.0).tracking(0.6)).color(white(0.45)));
    }
    let year = d["releaseDate"].as_str().and_then(|r| r.get(..4)).filter(|y| !y.is_empty());
    let badge = crate::formats::quality_badge(d);
    let ai = d["ai"].as_bool() == Some(true);
    if year.is_some() || badge.is_some() || ai {
        let mut r = row().gap(8.0).items_center();
        if let Some(y) = year {
            r = r.child(text(y.to_string()).font(t(12.0, 500.0)).color(white(0.55)));
        }
        if ai {
            r = r.child(crate::content::ai_badge());
        }
        if let Some((label, tier)) = badge {
            let fill = match tier { 2 => accent().alpha(0.2), 1 => accent().alpha(0.15), _ => white(0.1) };
            r = r.child(el().h(18.0).px(8.0).radius(4.0).fill(fill).items_center().justify_center()
                .child(text(label).font(t(10.0, 650.0).tracking(0.6)).color(white(0.9))));
        }
        c = c.child(r);
    }
    c
}

/// Under an album's tracks (AlbumView's footer): "MAY 17, 2013 • 12 TRACKS (40:00)" and the
/// copyright.
fn album_footer(a: &Value, tracks: &[Value], copyright: Option<&str>) -> El {
    let d = data::data(a);
    let secs: u64 = tracks.iter().filter_map(|t| data::data(t)["duration"].as_u64()).sum();
    let mut line = crate::formats::track_count(tracks.len(), secs);
    if let Some(r) = d["releaseDate"].as_str().filter(|r| !r.is_empty()) {
        line = format!("{} \u{2022} {line}", crate::formats::release_long(r));
    }
    let mut c = col().w_full().gap(4.0).px(12.0).child(text(line).font(t(12.0, 500.0)).color(white(0.4)));
    if let Some(cr) = copyright.filter(|c| !c.trim().is_empty()) {
        c = c.child(text(cr.to_uppercase()).font(t(11.0, 500.0)).color(white(0.4)).wrap());
    }
    c
}

pub(crate) fn play_buttons(v: &View, source: &Value, play: Cmd, shuffle: Cmd) -> El {
    let label = play_label(v, source);
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
    let shuf = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: true, shuffle: true };
    let actions = row().gap(8.0).items_center()
        .child(play_buttons(v, &source, play, shuf))
        .child({
            let (on, aid, l) = (crate::interact::is_fav(crate::interact::Fav::Album, &id.to_string()), id.to_string(), title.clone());
            header_btn("album-fav", if on { "Unfavorite album" } else { "Favorite album" }, if on { "Added" } else { "Add" }, if on { G::HeartFilled } else { G::Heart }, on)
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
    let mut c = col().w_full().gap(18.0).child(hero_with(&title, true, album_sub(a, &tracks), id, false, crate::art::item_url(a, 180.0), actions));
    let footer = (!tracks.is_empty()).then(|| album_footer(a, &tracks, page["copyright"].as_str()));
    let src = json!({"type": "album", "id": id, "name": title});
    // A multi-disc album lists each volume under "Volume N" (AlbumView volumeGroups); a click
    // plays from that track in the whole album either way.
    let groups = crate::formats::volumes(&tracks).unwrap_or_else(|| vec![(0, (0..tracks.len()).collect())]);
    let tracks = Rc::new(tracks);
    for (vol, shown) in groups {
        let src = src.clone();
        let list_key = if vol == 0 { "al-trows".to_string() } else { format!("al-trows{vol}") };
        let rows = track_list_keyed(list_key, "al-t", tracks.clone(), shown, move |tracks, i| {
            let (all, src) = (tracks.clone(), src.clone());
            // TrackList numbers an album's rows by their track number
            let n = data::data(&tracks[i])["trackNumber"].as_u64().map(|n| n as usize).unwrap_or(i + 1);
            track_row_look(format!("al-t{i}"), n, &tracks[i], Look { cover: false, ..Look::default() }, None, move |_: &mut App| {
                controller::send(Cmd::PlayFrom { tracks: all.to_vec(), index: i, source: src.clone(), album_mode: true });
            })
        });
        if vol > 0 {
            c = c.child(heading(&format!("Volume {vol}"), 15.0).pt(if vol > 1 { 8.0 } else { 0.0 }));
        }
        c = c.child(rows);
    }
    if let Some(f) = footer {
        c = c.child(f);
    }
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
        .child(crate::coll::play_pair(v, &src.clone(), move |shuffle| crate::paged::play_with(crate::paged::PKey::ArtistTracks(id), preview.clone(), src.clone(), shuffle, None, |_| true)))
        .child({
            // ArtistPage: Plus/"Follow", or Check/"Following" in the accent
            let (on, aid, l) = (crate::interact::is_fav(crate::interact::Fav::Artist, &id.to_string()), id.to_string(), name.clone());
            header_btn("artist-follow", if on { "Following" } else { "Follow" }, if on { "Following" } else { "Follow" }, if on { G::Check } else { G::Plus }, on)
                .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Artist, aid.clone(), l.clone(), crate::interact::Say::Nothing))
        })
        .child(match a["mixes"]["ARTIST_MIX"].as_str() {
            // ArtistPage's "Artist radio", when the artist has one.
            Some(mix) => {
                if let Some(url) = crate::art::artist_picture(a, 180.0) {
                    hint_mix_image(mix, url);
                }
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
    // ArtistPage: "48.2K fans", and no line at all for none
    let fans = crate::formats::fans(p["header"]["followersAmount"].as_u64().unwrap_or(0)).unwrap_or_default();
    let mut c = col().w_full().gap(18.0).child(hero(&name, true, &fans, id, true, crate::art::artist_picture(a, 180.0), actions));
    // ArtistPage: two clamped lines of the bio without its markup, and "Read more" for the
    // whole of it, links and all, in a dialog.
    if let Some(bio) = p["header"]["biography"]["text"].as_str().filter(|b| !b.trim().is_empty()) {
        c = c.child(col().gap(6.0).max_w(820.0).child(text(crate::bio::plain(bio)).font(t(13.0, 400.0)).color(white(0.6)).wrap().max_lines(2))
            .child(theme::focusable(row().key("bio-more").role("button").label("Read more")).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| s.nav.dialog = Some(Dialog::Bio(id)))
                .child(text("Read more").font(t(12.0, 650.0)).color(white(0.85)).a11y_hidden())));
    }
    let mut top_seen = false;
    let mut shown_any = false;
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
            c = c.child(grid(cards_with(&format!("ar-s{si}"), &items, move |it| artist_card_sub(it, id))));
        }
        shown_any = true;
    }
    if !shown_any {
        // ArtistPage's empty state
        c = c.child(col().w_full().gap(6.0).py(40.0).items_center()
            .child(glyph(G::User, 40.0, white(0.25)))
            .child(text("No content available").font(t(17.0, 650.0)).color(white(0.9)))
            .child(ui::empty("This artist doesn't have any tracks or albums yet.")));
    }
    c
}

/// A card's subtitle on an artist's pages: an album's year and kind, or its artists when this
/// artist only appears on it (getArtistAlbumSubtitle); other items as everywhere.
pub(crate) fn artist_card_sub(it: &Value, artist: u64) -> String {
    if data::kind(it) == "ALBUM"
        && let Some(s) = crate::formats::artist_album_subtitle(data::data(it), artist)
    {
        return s;
    }
    data::artist_of(it)
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
                el().key(format!("{key}-play")).role("button").label(format!("Play {title}")).help("Play").cursor(rune::Cursor::Pointer)
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
        // (UserMenu draws Log out in red, with its icon)
        .child(theme::focusable_row(row().key("am-logout").role("button").label("Log out")).w_full().h(36.0).px(12.0).gap(10.0).radius(theme::inset_radius(8.0)).items_center()
            .hover(white(0.08)).cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| {
                s.nav.menu_open = false;
                controller::send(Cmd::Logout)
            })
            .child(glyph(G::LogOut, 16.0, theme::danger()))
            .child(text("Log out").font(t(13.0, 500.0)).color(theme::danger()).grow(1.0).a11y_hidden()));
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
                .child(link("ab-version", version, "https://github.com/lullabyX/sone/releases").help("View releases"))
                .child(text("The native desktop client for TIDAL on Linux.").font(t(13.0, 500.0)).color(white(0.6)))
                .child(row().gap(8.0)
                    .child(link("ab-github", "GitHub", "https://github.com/lullabyX/sone"))
                    .child(link("ab-issues", "Issues", "https://github.com/lullabyX/sone/issues"))
                    .child(link("ab-license", "GPL-3.0", "https://github.com/lullabyX/sone/blob/master/LICENSE").help("View license")))
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
    // (each with its icon, as React's TABS)
    let t_btn = |key: &str, label: &str, which: Drawer, g: G| {
        let on = tab == which;
        theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(30.0).px(10.0).gap(6.0).radius(15.0).items_center()
            .fill(if on { white(0.14) } else { white(0.05) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| *s.nav.drawer = Some(which))
            .child(glyph(g, 12.0, white(if on { 0.85 } else { 0.6 })).a11y_hidden())
            .child(text(label.to_string()).font(t(11.0, 600.0)).color(white(0.85)).a11y_hidden())
    };
    let tabs = row().gap(6.0).flex_wrap().focus_group()
        .child(t_btn("dr-queue", "Play queue", Drawer::Queue, G::List))
        .child(t_btn("dr-suggested", "Suggested tracks", Drawer::Suggested, G::Sparkles))
        .child(t_btn("dr-lyrics", "Lyrics", Drawer::Lyrics, G::Mic))
        .child(t_btn("dr-credits", "Credits", Drawer::Credits, G::Users));
    took_loading();
    let body = match tab {
        Drawer::Queue => queue_panel(v),
        Drawer::Lyrics => lyrics(s, v),
        Drawer::Suggested => suggested(v),
        Drawer::Credits => credits(v),
    };
    // NowPlayingDrawer's corner: the full-screen player (or the video, maximised), and close.
    let (is_video, video_open) = (v.video.is_some(), v.video_open);
    let corner = row().gap(2.0).items_center()
        .child(icon::<App>("dr-full", if is_video { "Fullscreen video" } else { "Fullscreen player" }, G::Maximize).on_press_tx(move |s: &mut App, tx: &mut Tx| {
            if is_video {
                crate::video_ui::open_fullscreen(s, tx, video_open);
            } else if controller::now_playing().0.is_some() {
                crate::full_ui::open(s);
            }
        }))
        .child(icon::<App>("dr-close", "Close now playing panel", G::Close).on_press(|s: &mut App| *s.nav.drawer = None));
    let mut head = col().w_full().gap(10.0).child(row().w_full().gap(8.0).items_start().child(tabs.shrink()).child(spacer()).child(corner));
    let wide = shell.drawer_docked && shell.drawer_cover > 0.0;
    let mut cover_col = None;
    if let Some(cur) = &v.current {
        let mut title = row().gap(8.0).items_center().min_w(0.0).child(heading(&cur.title, if wide { 22.0 } else { 16.0 }).truncate().shrink());
        if let Some(b) = crate::content::badges(cur.explicit, cur.ai) {
            title = title.child(b);
        }
        let artist = text(v.artist.clone()).font(t(if wide { 15.0 } else { 12.0 }, 400.0)).color(white(0.55)).truncate();
        if wide {
            // NowPlayingDrawer's left column: the cover as large as the column allows, the
            // title and artist centred under it.
            let side = shell.drawer_cover - 48.0;
            // (the Afterglow tile until the picture is in, as every cover)
            let art = ui::with_image(ui::tile(ui::hue(Some(cur.id)), side, cur.id, 8.0), crate::art::cover(cur.cover.as_deref(), side), side, 8.0)
                .key("dr-cover").shadow(0.0, 24.0, theme::black(0.35));
            cover_col = Some(col().w(shell.drawer_cover).h_full().min_h(0.0).padding(24.0).gap(20.0).items_center().justify_center().clip_rounded(0.0)
                .child(art)
                .child(col().w_full().gap(4.0).items_center().child(title.max_w(shell.drawer_cover - 48.0)).child(artist.max_w(shell.drawer_cover - 48.0))));
        } else {
            // the narrower panel: a small cover beside the title and artist
            let cover = ui::with_image(ui::tile(ui::hue(Some(cur.id)), 52.0, cur.id, 6.0), crate::art::cover(cur.cover.as_deref(), 52.0), 52.0, 6.0);
            head = head.child(row().w_full().gap(12.0).items_center().child(cover)
                .child(col().grow(1.0).min_w(0.0).gap(3.0).child(title).child(artist)));
        }
    }
    // Docked, a column beside the page; in a narrow window (Shell), a sheet over the page's
    // right edge. Either way it starts under the header, which keeps the window controls.
    // With the cover column, the landmark is the row holding both (below); the tabs are its
    // right column.
    // (the region's root carries its key, "drawer": a sheet's root is the layer holding it)
    let d = if wide { col().w(shell.drawer_w) } else { col().key(if shell.drawer_docked { "drawer" } else { "drawer-panel" }).role("complementary").label("Now playing panel").w(shell.drawer_w) };
    let d = if shell.drawer_docked { d.h_full() } else { d.pin(None, Some(0.0), Some(0.0), Some(0.0)).border(1.0, white(0.06)).shadow(0.0, 32.0, theme::black(0.45)) };
    // The album's colour washing down from the top (React's overlayGradient: vibrantColor at
    // 0.28, clear by 90%), else the cover's own most vivid colour.
    let tint = v.current.as_ref().and_then(|c| {
        c.track["album"]["vibrantColor"].as_str().and_then(theme::hex_oklch).map(|(l, c, h)| rune::oklch(l, c, h))
            .or_else(|| crate::art::cover_color(c.cover.as_deref()).filter(|k| k.c >= 0.03).map(|k| rune::oklch(k.l, k.c, k.h)))
    });
    let wash = move |e: El| match tint {
        Some(c) => e.linear_gradient(std::f32::consts::PI, [(0.0, c.alpha(0.28)), (0.9, c.alpha(0.0))]),
        None => e,
    };
    let mut scroller = col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(body);
    if tab == Drawer::Lyrics {
        scroller = scroller.on_scroll_offset(watch_lyrics_scroll);
    } else {
        LY_SCROLL.with(|c| c.set(None));
    }
    let d = if wide { d } else { wash(d.fill(theme::drawer_bg())) };
    let mut d = d.padding(16.0).gap(12.0)
        .child(head)
        .child(ui::smooth_wheel::<App>(scroller, "drawer-scroll".into())
            .labelled_by(match tab { Drawer::Queue => "dr-queue", Drawer::Suggested => "dr-suggested", Drawer::Lyrics => "dr-lyrics", Drawer::Credits => "dr-credits" })
            .busy(took_loading()));
    if tab == Drawer::Lyrics && *s.nav.lyrics_scrolled && v.current.as_ref().and_then(|c| synced_lyrics(c.id)).is_some() {
        // LyricsTab: the floating "Sync lyrics" button once the user scrolls away
        d = d.child(theme::focusable(row().key("ly-sync").role("button").label("Sync lyrics")).pin(None, None, Some(32.0), Some(16.0)).h(36.0).px(16.0).gap(8.0).radius(18.0).items_center()
            .fill(accent()).shadow(6.0, 18.0, theme::black(0.4)).cursor(rune::Cursor::Pointer).press_scale(0.95)
            .on_press_tx(|s: &mut App, tx: &mut Tx| {
                *s.nav.lyrics_scrolled = false;
                if let Some(i) = *s.nav.lyric_line {
                    lyrics_auto_scroll();
                    tx.scroll_center(format!("ly-{i}"));
                }
            })
            .child(glyph(G::Mic, 14.0, theme::ink()))
            .child(text("Sync lyrics").font(t(12.0, 700.0)).color(theme::ink()).a11y_hidden()));
    }
    // It slides in from the right as it opens (React's sheet slides up, 250 ms on
    // cubic-bezier(0.32, 0.72, 0, 1)): a one-shot transition, nothing at rest.
    let slide = || rune::Transition::offset(48.0, 0.0).combined(rune::Transition::opacity())
        .springs(rune::SpringPair::new(rune::Spring::new(0.25, 0.0), rune::Spring::new(0.18, 0.0)));
    if wide {
        let outer = row().key("drawer").role("complementary").label("Now playing panel").w(shell.drawer_cover + shell.drawer_w).h_full().fill(theme::drawer_bg());
        let outer = match cover_col {
            Some(cc) => outer.child(cc).child(el().w(1.0).h_full().fill(white(0.06)).a11y_hidden()),
            // (nothing playing: the tabs alone, at the full width)
            None => outer,
        };
        return Some(wash(outer).child(d.grow(1.0)).transition(slide()));
    }
    let d = d.transition(slide());
    if !shell.drawer_docked {
        // Over the page, a press beside the sheet closes it (React's backdrop).
        let scrim = el().key("drawer-scrim").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(theme::black(0.35)).a11y_hidden()
            .transition(rune::Transition::opacity().springs(rune::SpringPair::new(rune::Spring::new(0.2, 0.0), rune::Spring::new(0.2, 0.0))))
            .on_press(|s: &mut App| *s.nav.drawer = None);
        return Some(el().key("drawer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(scrim).child(d));
    }
    Some(d)
}

/// Synced (LRC) lines as (seconds, text); plain lyrics as untimed lines.

/// LRC lines as React's parseLrc reads them: every `[mm:ss.xx]` tag on a line is a time for its
/// text, lines without text are skipped, and the result is sorted by time.
fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = vec![];
    for raw in lrc.lines() {
        // Every time tag anywhere on the line is a time for it, and is cut from its text
        // (lrc.ts: /\[(\d{1,2}:\d{2}(?:[.:]\d{1,3})?)\]/g); other tags stay as text.
        let (mut text, mut times, mut rest) = (String::new(), vec![], raw.trim());
        while let Some(i) = rest.find('[') {
            text.push_str(&rest[..i]);
            match lrc_tag(&rest[i + 1..]) {
                Some((at, len)) => {
                    times.push(at);
                    rest = &rest[i + 1 + len..];
                }
                None => {
                    text.push('[');
                    rest = &rest[i + 1..];
                }
            }
        }
        text.push_str(rest);
        let text = text.trim();
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

/// One LRC time tag at the start of `s` (just after its `[`): `m:ss`, `mm:ss`, with an optional
/// `.x`/`:x` fraction of 1-3 digits read as lrc.ts's parseTimestamp reads it (one digit is
/// tenths, two hundredths, three milliseconds). Its time in seconds and its length with `]`.
fn lrc_tag(s: &str) -> Option<(f64, usize)> {
    let b = s.as_bytes();
    let digits = |from: usize, max: usize| b[from..].iter().take(max).take_while(|c| c.is_ascii_digit()).count();
    let m = digits(0, 2);
    if m == 0 || b.get(m) != Some(&b':') || digits(m + 1, 2) != 2 {
        return None;
    }
    let mut end = m + 3;
    let mut ms = 0.0;
    if matches!(b.get(end), Some(b'.' | b':')) {
        let f = digits(end + 1, 3);
        if f > 0 {
            let v: f64 = s[end + 1..end + 1 + f].parse().ok()?;
            ms = v * [100.0, 10.0, 1.0][f - 1];
            end += 1 + f;
        }
    }
    if b.get(end) != Some(&b']') {
        return None;
    }
    let mins: f64 = s[..m].parse().ok()?;
    let secs: f64 = s[m + 1..m + 3].parse().ok()?;
    Some((mins * 60.0 + secs + ms / 1000.0, end + 1))
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

thread_local! {
    /// The Lyrics tab's scroll offset and range as last reported (None: not shown since).
    static LY_SCROLL: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
    /// Until when a scroll is the app's own glide to the active line (LyricsTab isAutoScrolling).
    static LY_AUTO_UNTIL: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}
static LY_USER_SCROLLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The app is about to glide the Lyrics tab to a line: the scrolling that follows is its own.
pub fn lyrics_auto_scroll() {
    LY_AUTO_UNTIL.with(|c| c.set(Some(std::time::Instant::now() + std::time::Duration::from_millis(1000))));
}

/// The Lyrics tab's scroll offset, every frame it moves: a move the app didn't make (the
/// scrollbar, the keyboard; the wheel says so itself) is the user scrolling away (LyricsTab's
/// scroll listener). A changed range is the content resizing, not a scroll.
fn watch_lyrics_scroll(at: rune::ScrollPos) {
    let now = (at.offset[1], at.max[1]);
    let before = LY_SCROLL.with(|c| c.replace(Some(now)));
    let Some((off, max)) = before else { return };
    let auto = LY_AUTO_UNTIL.with(|c| c.get()).is_some_and(|t| std::time::Instant::now() < t);
    if !auto && (max - now.1).abs() < 0.5 && (off - now.0).abs() > 2.0 {
        LY_USER_SCROLLED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Whether the user scrolled the Lyrics tab since this was last asked.
pub fn take_lyrics_user_scroll() -> bool {
    LY_USER_SCROLLED.swap(false, std::sync::atomic::Ordering::Relaxed)
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
                    let mut words = text(line.clone()).key(format!("ly-{i}")).font(t(20.0, if on { 700.0 } else { 500.0 }).lh(26.0)).color(color).wrap()
                        .origin(0.0, 0.5).scale(if on { 1.22 } else { 1.0 }).scale_motion(rune::Spring::new(0.5, 0.0));
                    // (an inactive line brightens under the pointer: LyricsLine's hover colour)
                    if !on {
                        words = words.group_color(white(0.7));
                    }
                    col_lines = col_lines.child(
                        el().key(format!("lyw-{i}")).role("presentation").hover_group().cursor(rune::Cursor::Pointer)
                            .on_press(move |s: &mut App| {
                                *s.nav.lyrics_scrolled = false;
                                controller::send(Cmd::SeekTo(at));
                            })
                            .child(words),
                    );
                }
                c = c.child(col_lines.child(el().h(160.0)));
                // (React puts the provider under synced lyrics and plain ones, never under
                // "No lyrics available")
                if let Some(p) = provider {
                    c = c.child(text(format!("Lyrics provided by {p}")).font(t(11.0, 500.0)).color(white(0.4)));
                }
            } else {
                let plain = l["lyrics"].as_str().unwrap_or("").trim().to_string();
                if plain.is_empty() {
                    c = c.child(ui::empty("No lyrics available for this track"));
                } else {
                    // whitespace-pre-wrap: a blank line keeps its height, between verses
                    for line in plain.lines() {
                        c = c.child(if line.trim().is_empty() { el().h(32.0) } else { text(line.to_string()).font(t(18.0, 500.0).lh(32.0)).color(white(0.5)).wrap() });
                    }
                    if let Some(p) = provider {
                        c = c.child(text(format!("Lyrics provided by {p}")).font(t(11.0, 500.0)).color(white(0.4)).mt(20.0));
                    }
                }
            }
        }
        data::Load::Failed(_) => c = c.child(ui::empty("No lyrics available for this track")),
        data::Load::Loading => c = c.child(loading()),
    }
    c
}

/// Explore (React ExplorePage): pill rows per category with View all, plus the icon grid.
fn explore(s: &App) -> El {
    let v = match ready_or(Key::Page("pages/explore".into())) { Ok(v) => v, Err(e) => return e };
    let mut c = col().w_full().gap(20.0).child(heading("Explore", theme::PAGE_TITLE));
    let sections = v["sections"].as_array().cloned().unwrap_or_default();
    let has_items = |s: &Value| s["items"].as_array().is_some_and(|a| !a.is_empty());
    let has_icons = |s: &Value| s["items"].as_array().is_some_and(|a| a.iter().any(|i| i.get("icon").is_some_and(|x| !x.is_null())));
    let titled = |s: &Value| s["title"].as_str().is_some_and(|t| !t.is_empty());
    if sections.is_empty() {
        return c.child(ui::empty("No categories found"));
    }
    // ExplorePage draws only its link sections: the titled pill rows (PAGE_LINKS_CLOUD), then the
    // icon grid (the first PAGE_LINKS), then untitled rows without icons as pills.
    let pills: Vec<(usize, &Value)> = sections.iter().enumerate().filter(|(_, s)| has_items(s) && titled(s) && s["sectionType"].as_str() == Some("PAGE_LINKS_CLOUD")).collect();
    let icons = sections.iter().enumerate().find(|(_, s)| has_items(s) && s["sectionType"].as_str() == Some("PAGE_LINKS"));
    let untitled: Vec<(usize, &Value)> = sections.iter().enumerate().filter(|(_, s)| has_items(s) && !titled(s) && !has_icons(s)).collect();
    for (si, sec) in pills {
        c = c.child(explore_pills(si, sec));
    }
    if let Some((si, sec)) = icons {
        c = c.child(crate::home::explore_icon_grid(si, sec["items"].as_array().map(Vec::as_slice).unwrap_or(&[]), *s.nav.content_w));
    }
    for (si, sec) in untitled {
        c = c.child(explore_pills(si, sec));
    }
    c
}

/// One of Explore's pill rows (PillSection): its title, View all, and a pill per link.
fn explore_pills(si: usize, sec: &Value) -> El {
    let title = sec["title"].as_str().unwrap_or("").to_string();
    let mut head = row().w_full().items_center();
    if !title.is_empty() {
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
    col().w_full().gap(10.0).child(head).child(pills)
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
        if let Some(e) = crate::home::section(&format!("es-{path}"), si, &shown, w) {
            c = c.child(e);
        }
    }
    c
}

/// The UTC day (days since 1970-01-01) of a date or date-time as TIDAL writes it, its offset
/// counted (groupFeedByPeriod compares UTC months: a "…T23:30:00-02:00" is the next day).
fn days(date: &str) -> Option<i64> {
    crate::content::parse_date(date).map(|s| s.div_euclid(86400))
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
        .child(col().grow(1.0).min_w(0.0)
            // FeedRow: the AI badge after the title
            .child(row().min_w(0.0).gap(6.0).items_center().child(text(title.clone()).font(t(14.0, 600.0)).color(white(0.95)).truncate().shrink())
                .child(crate::content::badges(false, item["ai"].as_bool() == Some(true)).unwrap_or_else(el)))
            .child(text(sub.clone()).font(t(12.0, 400.0)).color(white(0.55)).truncate()));
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
                match data::get(Key::Album(aid)).await {
                    Ok(p) => {
                        let tracks = p["page"]["tracks"].as_array().cloned().unwrap_or_default();
                        controller::send(Cmd::PlayCollection { tracks, source: json!({"type": "album", "id": aid, "name": title}), album_mode: true, shuffle: false });
                    }
                    // useMediaPlay
                    Err(_) => crate::interact::toast("Failed to play".into(), crate::interact::Kind::Error),
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
                match tauri_app_lib::commands::pages::get_mix_items(h.state::<tauri_app_lib::AppState>(), mid.clone()).await {
                    Ok(m) => {
                        let v = serde_json::to_value(m).unwrap_or(Value::Null);
                        let tracks = v["tracks"].as_array().or(v["items"].as_array()).cloned().unwrap_or_default();
                        controller::send(Cmd::PlayCollection { tracks, source: json!({"type": "mix", "id": mid, "name": title}), album_mode: false, shuffle: false });
                    }
                    Err(_) => crate::interact::toast("Failed to play".into(), crate::interact::Kind::Error),
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
        Login::Waiting { code, uri } => c = c.child(crate::login::waiting(code, uri)),
        state if s.nav.set.custom_login.is_some() => {
            let failed = if let Login::Failed(e) = state { Some(e.as_str()) } else { None };
            c = c.w(480.0).child(crate::login::view(s, failed));
        }
        // (the simple view, login.rs: the method tabs when PKCE is built in, the error, status)
        state => c = c.child(crate::login::simple(s, state)),
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
        .child(play_buttons(v, &source, play, shuf))
        .child(header_btn("mix-fav", if on { "Remove from favorites" } else { "Add to favorites" }, if on { "Added" } else { "Add" }, if on { G::HeartFilled } else { G::Heart }, on)
            .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Mix, mid.clone(), l.clone(), crate::interact::Say::Nothing)))
        .child(header_btn("mix-share", "Copy share link", "Share", G::Share, false).on_press_tx(move |_: &mut App, tx: &mut Tx| {
            tx.copy_rich(rune::ClipboardContent::new(format!("https://tidal.com/mix/{share_id}")));
            crate::interact::toast("Copied share link to clipboard".into(), crate::interact::Kind::Success);
        }))
        .child(header_btn("mix-more", "More options", "More", G::More, false)
            .on_press(move |s: &mut App| crate::interact::open_media(s, item.clone(), crate::interact::Anchor::Below("mix-more".into()))));
    let seed = id.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    // MixPage: the picture it was opened under, else the mix's own, else its first track's.
    let art = mix_image(id).or_else(|| m["image"].as_str().filter(|u| u.starts_with("http")).map(str::to_string)).or_else(|| tracks.first().and_then(|t| crate::art::item_url(t, 180.0)));
    // "MIX" / "TRACK RADIO" over the title; "N TRACKS (m:ss)" under the subtitle.
    let secs: u64 = tracks.iter().filter_map(|t| data::data(t)["duration"].as_u64()).sum();
    let count = crate::formats::track_count(tracks.len(), secs);
    // A track radio names its seed artist (the first track's), with their picture, linked;
    // a mix shows its subtitle.
    let seed_artist = tracks.first().map(data::data).map(|d| if d["artists"][0].is_object() { &d["artists"][0] } else { &d["artist"] }).filter(|a| a["id"].as_u64().is_some());
    let line = match seed_artist.filter(|_| radio) {
        Some(a) => {
            let mut r = row().gap(8.0).items_center().min_w(0.0);
            if let Some(url) = crate::art::artist_picture(a, 24.0) {
                r = r.child(ui::with_image(el().size(24.0, 24.0).radius(12.0).clip_rounded(12.0).fill(white(0.08)), Some(url), 24.0, 12.0).a11y_hidden());
            }
            r.child(artist_links("mx-seed", &json!({"artist": a}), white(0.9)))
        }
        None => text(sub.to_string()).font(t(13.0, 500.0)).color(white(0.6)).truncate(),
    };
    let sub_el = col().gap(4.0).min_w(0.0)
        .child(text(if radio { "TRACK RADIO" } else { "MIX" }).font(t(11.0, 700.0).tracking(1.2)).color(white(0.55)))
        .child(line)
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
mod search_history_tests {
    use super::push_history;

    #[test]
    fn recent_searches_dedupe_by_case_and_keep_ten() {
        let mut h = vec!["Daft Punk".to_string(), "beatles".to_string()];
        push_history(&mut h, "  daft punk ");
        assert_eq!(h, ["daft punk", "beatles"]);
        push_history(&mut h, "");
        assert_eq!(h.len(), 2);
        for i in 0..12 {
            push_history(&mut h, &format!("q{i}"));
        }
        assert_eq!(h.len(), 10);
        assert_eq!(h[0], "q11");
    }
}

#[cfg(test)]
mod source_tests {
    use super::same_source;
    use serde_json::json;

    #[test]
    fn sources_match_by_type_and_id_not_name() {
        let artist = json!({"type": "artist", "id": 7, "name": "Band"});
        let popular = json!({"type": "artist-tracks", "id": 7, "name": "Band"});
        assert!(same_source("artist", "7", &artist));
        assert!(!same_source("artist", "7", &popular));
        // two playlists with one title
        assert!(!same_source("playlist", "a", &json!({"type": "playlist", "id": "b", "name": "Mix"})));
        assert!(same_source("playlist", "b", &json!({"type": "playlist", "id": "b", "name": "Other"})));
    }
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
        // the offset counts: these are 1 March and 31 January in UTC
        assert_eq!(m("2024-02-29T23:30:00.000-0100"), 2024 * 12 + 2);
        assert_eq!(m("2024-02-01T00:30:00+01:00"), 2024 * 12);
        assert_eq!(m("2024-02-01T00:00:00.000+0000"), 2024 * 12 + 1);
    }
}

#[cfg(test)]
mod lrc_tests {
    use super::parse_lrc;

    #[test]
    fn time_tags_count_anywhere_on_the_line() {
        let l = parse_lrc("[ar:Someone]\n[00:01.5]One\nTwo [00:03.25] more\n[00:02][00:04.123]Echo\n[00:05]\n[1:02:03]x [01:02.1234]y");
        let at: Vec<(f64, &str)> = l.iter().map(|(t, s)| (*t, s.as_str())).collect();
        assert_eq!(at.len(), 5);
        assert_eq!(at[0], (1.5, "One"));
        assert_eq!(at[1], (2.0, "Echo"));
        assert_eq!(at[2], (3.25, "Two  more"));
        assert!((at[3].0 - 4.123).abs() < 1e-9 && at[3].1 == "Echo");
        // a `:` fraction counts; a four-digit fraction is no tag and stays as text
        assert!((at[4].0 - 62.03).abs() < 1e-9 && at[4].1 == "x [01:02.1234]y");
    }
}
