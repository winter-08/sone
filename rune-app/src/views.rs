//! SONE's screens in Rune: window chrome, header (navigation + search), sidebar (library),
//! and the Home / Search / Album / Artist / Playlist pages.
//!
//! Accessible names follow the React build wherever the verifier's goldens show them, because
//! the verifier compares what's on screen through AT-SPI ("button: Favorite album", the
//! "Results for …" heading, …). Visual style is Afterglow (ui.rs), not a copy of SONE's.

use crate::controller::{self, Cmd, View};
use crate::data::{self, Key};
use crate::glyph::{G, glyph};
use crate::ui::{self, caps, t, white};
use crate::App;
use rune::{El, ScrollSpec, TextEdit, Tx, col, el, oklch, row, spacer, text, text_input};
use serde_json::{Value, json};

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

pub struct Nav {
    pub page: Page,
    pub back: Vec<Page>,
    pub fwd: Vec<Page>,
    pub search: TextEdit,
    pub lib: LibTab,
    pub tab: SearchTab,
    pub filter: TextEdit,
    pub queue_open: bool,
    /// When the debounced suggestions fetch for the current search text is due.
    pub suggest_due: Option<std::time::Instant>,
    pub menu_open: bool,
    pub settings_tab: SettingsTab,
    pub drawer: Option<Drawer>,
    pub login_mode: LoginMode,
    /// Settings is a modal over the current page (React SettingsModal).
    pub settings_open: bool,
    pub dialog: Option<Dialog>,
    /// Editable settings fields, filled from the saved settings when Settings opens.
    pub discord_status: TextEdit,
    pub lb_token: TextEdit,
    pub overlay_host: TextEdit,
    pub proxy_user: TextEdit,
    /// Debounced saves (React's 500 ms timers in DiscordTab / NetworkTab).
    pub discord_due: Option<std::time::Instant>,
    pub proxy_due: Option<std::time::Instant>,
    pub lb_status: Option<String>,
    /// The open context menu (crate::interact), and the create-playlist dialog.
    pub ctx: Option<crate::interact::Ctx>,
    pub create: Option<crate::interact::Create>,
    /// Synced lyrics: the active line, and whether the user scrolled away from it.
    pub lyric_line: Option<usize>,
    pub lyrics_scrolled: bool,
    /// The search dropdown is open (typing opens it; a pick, Enter, Escape or a click
    /// elsewhere closes it), and the entry ↑/↓ highlighted.
    pub search_open: bool,
    pub search_sel: Option<usize>,
    /// The music video player (video_ui.rs).
    pub video: crate::video_ui::Player,
    /// Home: the feed tab ("static" is For you) and how many further pages of it to show.
    pub home_tab: String,
    pub home_pages: usize,
    /// The page content's width inside its padding (measured; Home sizes grids and cards by it).
    pub content_w: f32,
    /// The sign-in screen's PKCE field: the redirect URL pasted back from the browser.
    pub pkce_paste: TextEdit,
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
        Nav { page: Page::Home, back: vec![], fwd: vec![], search: TextEdit::new(""), lib: LibTab::Playlists, tab: SearchTab::All, filter: TextEdit::new(""), queue_open: false, suggest_due: None, menu_open: false, settings_tab: SettingsTab::Playback, drawer: None, login_mode: LoginMode::Pkce, settings_open: false, dialog: None,
            discord_status: TextEdit::new(""), lb_token: TextEdit::new(""), overlay_host: TextEdit::new(""), proxy_user: TextEdit::new(""),
            discord_due: None, proxy_due: None, lb_status: None, ctx: None, create: None, lyric_line: None, lyrics_scrolled: false, search_open: false, search_sel: None,
            video: Default::default(), home_tab: "static".into(), home_pages: 0, content_w: 1000.0, pkce_paste: TextEdit::new("") }
    }
}

pub fn go(s: &mut App, p: Page) {
    if s.nav.page != p {
        let old = std::mem::replace(&mut s.nav.page, p);
        s.nav.back.push(old);
        s.nav.fwd.clear();
    }
    if !matches!(s.nav.page, Page::Search(_)) {
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

fn button<S: 'static>(key: &str, label: &str, g: Option<G>, primary: bool) -> El {
    let (bg, fg) = if primary { (white(0.97), oklch(0.2, 0.02, 270.0)) } else { (white(0.1), white(0.92)) };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(label)
        .focusable(true)
        .focus_ring(2.0, 2.0, white(0.6))
        .h(36.0)
        .px(14.0)
        .gap(8.0)
        .radius(18.0)
        .items_center()
        .fill(bg)
        .hover(if primary { white(0.9) } else { white(0.16) })
        .press_scale(0.96)
        .cursor(rune::Cursor::Pointer);
    if let Some(g) = g {
        b = b.child(glyph(g, 15.0, fg));
    }
    b.child(text(label.to_string()).font(t(13.0, 600.0)).color(fg).a11y_hidden())
}

/// A page-header action (AlbumView, PlaylistView, ArtistPage): a 22 pt icon over an 11 pt
/// caption. `label` is the accessible name (React's title), `caption` what shows under it.
fn header_btn(key: &str, label: &str, caption: &str, g: G, on: bool) -> El {
    col().key(key.to_string()).role("button").label(label).focusable(true).focus_ring(2.0, 2.0, white(0.6))
        .w(52.0).py(4.0).gap(3.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .child(glyph(g, 22.0, if on { accent() } else { white(0.8) }))
        .child(text(caption.to_string()).font(t(11.0, 500.0)).color(white(0.6)).a11y_hidden())
}

fn icon<S: 'static>(key: &str, label: &str, g: G) -> El {
    ui::icon_button::<S>(key, label, g, 18.0, white(0.75))
}

pub(crate) fn heading(label: &str, size: f32) -> El {
    text(label.to_string()).font(t(size, 650.0)).color(white(0.95)).role("heading").label(label)
}

/// A clickable catalogue card: art, a heading with its name, a subtitle. The card itself is the
/// press target (AT-SPI clicks reach it through the heading, as with the React cards).
/// The React accent (`text-th-accent`) for loved hearts and "in library" states.
pub(crate) fn accent() -> rune::Color {
    oklch(0.75, 0.13, 250.0)
}

/// A clickable catalogue card: art, a heading with its name, a subtitle. The card itself is the
/// press target (AT-SPI clicks reach it through the heading, as with the React cards).
///
/// With its `item` (MediaCard.tsx): right-click opens the media menu at the pointer; hovering
/// shows a play button (bottom-left, accent, 40 pt) and a heart (bottom-right, 32 pt), or for an
/// artist a centred 48 pt play button. Like React's, those buttons carry no accessible name.
fn card(key: String, title: &str, sub: &str, seed: u64, round: bool, img: Option<String>, item: Option<Value>, go_to: Page) -> El {
    card_w(148.0, key, title, sub, seed, round, img, item, go_to)
}

/// [`card`] `w` points wide (Home sizes its cards to the row, as React's card-scroll does).
pub(crate) fn card_w(w: f32, key: String, title: &str, sub: &str, seed: u64, round: bool, img: Option<String>, item: Option<Value>, go_to: Page) -> El {
    let a = w - 16.0;
    use crate::interact::{self, Anchor, Fav, Media, Say};
    let h = ui::hue(Some(seed));
    let mut art = if round { ui::with_image(ui::art(h, a, seed, 0.0), img, a, a / 2.0) } else { ui::with_image(ui::tile(h, a, seed, 10.0), img, a, 10.0) };
    let kind = item.as_ref().and_then(Media::of);
    if let (Some(it), Some(kind)) = (item.clone(), kind) {
        let play_it = it.clone();
        if kind == Media::Artist {
            art = art.child(el().absolute(a / 2.0 - 24.0, a / 2.0 - 24.0).key(format!("{key}-play")).role("button").size(48.0, 48.0).radius(24.0).fill(rune::oklcha(0.0, 0.0, 0.0, 0.45))
                .items_center().justify_center().group_opacity(0.0, 1.0).cursor(rune::Cursor::Pointer)
                .on_press(move |_: &mut App| interact::play_media(&play_it, kind))
                .child(glyph(G::Play, 28.0, white(1.0))));
        } else {
            let fav_kind = match kind { Media::Album => Fav::Album, Media::Playlist => Fav::Playlist, Media::Mix => Fav::Mix, Media::Artist => Fav::Artist };
            let d = data::data(&it);
            let id = if kind == Media::Playlist { d["uuid"].as_str().unwrap_or("").to_string() } else { d["id"].as_str().map(str::to_string).unwrap_or_else(|| d["id"].to_string()) };
            let on = interact::is_fav(fav_kind, &id);
            let label = data::name(&it);
            art = art
                .child(el().absolute(10.0, a - 50.0).key(format!("{key}-play")).role("button").size(40.0, 40.0).radius(20.0).fill(accent())
                    .items_center().justify_center().group_opacity(0.0, 1.0).press_scale(0.94).cursor(rune::Cursor::Pointer)
                    .on_press(move |_: &mut App| interact::play_media(&play_it, kind))
                    .child(glyph(G::Play, 20.0, oklch(0.18, 0.02, 270.0))))
                .child(el().absolute(a - 42.0, a - 42.0).key(format!("{key}-fav")).role("button").size(32.0, 32.0).radius(16.0).fill(rune::oklcha(0.0, 0.0, 0.0, 0.5))
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
        .cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| go(s, go_to.clone()));
    if let Some(it) = item.filter(|_| kind.is_some()) {
        c = c.on_secondary_press(move |s: &mut App, p, _| interact::open_media(s, it.clone(), Anchor::Point(p.x, p.y)));
    }
    c.child(art)
        .child(heading(title, 13.0).truncate().on_press(move |s: &mut App| go(s, to.clone())))
        .child(text(sub.to_string()).font(t(11.0, 400.0)).color(white(0.55)).truncate())
}

/// A track row with the row actions the React rows expose.
fn track_row(key: String, n: usize, tr: &Value, on_play: impl Fn(&mut App) + 'static) -> El {
    track_row_with(key, n, tr, true, None, on_play)
}

/// A track row (TrackList.tsx). A single click plays it (there's no double-click play in React
/// either); hovering swaps the number for a play icon; right-click opens the track menu at the
/// pointer. Its actions, after the time: More options (shown on hover; the same menu, below it),
/// Add to playlist (the playlist picker, below it) and the heart (toggled optimistically, filled
/// in the accent when loved). `pl` places it in the user's own playlist, for "Remove from
/// playlist".
fn track_row_with(key: String, n: usize, tr: &Value, actions: bool, pl: Option<crate::interact::InPlaylist>, on_play: impl Fn(&mut App) + 'static) -> El {
    use crate::interact::{self, Anchor, Fav, Say};
    let d = data::data(tr);
    let title = d["title"].as_str().unwrap_or("").to_string();
    let dur = d["duration"].as_u64().unwrap_or(0);
    let id = d["id"].as_u64().unwrap_or(0);
    // The React TrackList: the playing row shows the bouncing bars in place of its number, and
    // the current track's number and title take the accent colour.
    let (current_id, playing) = controller::now_playing();
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
        .hover(white(0.05))
        .hover_group()
        .cursor(rune::Cursor::Pointer)
        .on_press(on_play)
        .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, menu_tr.clone(), menu_pl.clone(), Anchor::Point(p.x, p.y)))
        .child(number)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(text(title.clone()).font(t(14.0, 500.0)).color(if active { accent_c } else { white(0.95) }).truncate())
                .child(text(data::artist_of(tr)).font(t(12.0, 400.0)).color(white(0.55)).truncate()),
        )
        .child(text(ui::fmt_time(dur as f64)).font(ui::tnum(t(12.0, 400.0))).color(white(0.55)).w(48.0));
    if actions {
        let loved = interact::is_fav(Fav::Track, &id.to_string());
        let (more_key, add_key) = (format!("{key}-more"), format!("{key}-add"));
        let (t1, t2, mk, ak) = (tr.clone(), tr.clone(), more_key.clone(), add_key.clone());
        r = r
            .child(icon::<App>(&more_key, "More options", G::More).group_opacity(0.0, 1.0)
                .on_press(move |s: &mut App| interact::open_track(s, t1.clone(), pl.clone(), Anchor::Below(mk.clone()))))
            .child(icon::<App>(&add_key, "Add to playlist", G::Plus).on_press(move |s: &mut App| interact::open_add_below(s, t2.clone(), ak.clone())))
            .child(ui::icon_button::<App>(&format!("{key}-fav"), if loved { "Remove from favorites" } else { "Add to favorites" }, if loved { G::HeartFilled } else { G::Heart }, 18.0, if loved { accent() } else { white(0.75) })
                .on_press(move |_: &mut App| interact::toggle_fav(Fav::Track, id.to_string(), title.clone(), Say::Nothing)));
    }
    r
}

fn grid(cards: Vec<El>) -> El {
    let mut r = row().w_full().gap(8.0).flex_wrap();
    for c in cards {
        r = r.child(c);
    }
    r
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

fn cards_for(prefix: &str, items: &[Value]) -> Vec<El> {
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

pub(crate) fn loading() -> El {
    text("Loading…").font(t(13.0, 500.0)).color(white(0.45))
}

// ------------------------------------------------------------------ chrome

pub fn window_buttons() -> El {
    row()
        .gap(2.0)
        .child(ui::icon_button::<App>("win-min", "Minimize", G::Minus, 14.0, white(0.6)).on_press_tx(|_: &mut App, tx: &mut Tx| tx.minimize()))
        .child(ui::icon_button::<App>("win-max", "Maximize", G::Square, 12.0, white(0.6)).on_press_tx(|_: &mut App, tx: &mut Tx| tx.toggle_maximized()))
        .child(ui::icon_button::<App>("win-close", "Close", G::Close, 14.0, white(0.6)).on_press_tx(|_: &mut App, tx: &mut Tx| tx.close_window()))
}

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
                s.nav.tab = SearchTab::All;
                go(s, Page::Search(q));
            }
        });
    let bar = row().key("search-bar").w(360.0).h(40.0).px(14.0).gap(8.0).radius(20.0).items_center().fill(white(0.08)).child(glyph(G::Search, 16.0, white(0.55))).child(field);
    row()
        .key("header")
        .w_full()
        .h(60.0)
        .px(20.0)
        .gap(10.0)
        .items_center()
        .child(icon::<App>("back", "Go back", G::Back).on_press(|s: &mut App| {
            if let Some(p) = s.nav.back.pop() {
                let cur = std::mem::replace(&mut s.nav.page, p);
                s.nav.fwd.push(cur);
            }
        }))
        .child(icon::<App>("forward", "Go forward", G::Forward).on_press(|s: &mut App| {
            if let Some(p) = s.nav.fwd.pop() {
                let cur = std::mem::replace(&mut s.nav.page, p);
                s.nav.back.push(cur);
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
        .child(window_buttons())
}

/// The suggestions dropdown under the search field while typing (React SearchBar).
/// One entry of the search dropdown, in display order.
#[derive(Clone, Debug)]
enum Entry {
    /// A text suggestion: fills the search bar and searches.
    Text(String),
    /// A direct hit: an album, artist or playlist page.
    Hit(String, Page),
    /// A direct hit on a track: plays it.
    Track(String, u64),
    ViewAll,
}

fn dropdown_entries(q: &str) -> Vec<Entry> {
    let Some(v) = data::peek(&Key::Suggest(q.to_string())) else { return vec![] };
    let mut out: Vec<Entry> = v["textSuggestions"].as_array().cloned().unwrap_or_default().iter()
        .filter_map(|x| x["query"].as_str().map(|q| Entry::Text(q.to_string()))).collect();
    for hit in v["directHits"].as_array().cloned().unwrap_or_default() {
        let title = hit["title"].as_str().or(hit["name"].as_str()).unwrap_or("").to_string();
        let page = match hit["hitType"].as_str().unwrap_or("") {
            "TRACKS" => {
                out.push(Entry::Track(title, hit["id"].as_u64().unwrap_or(0)));
                continue;
            }
            "ALBUMS" => hit["id"].as_u64().map(Page::Album),
            "ARTISTS" => hit["id"].as_u64().map(Page::Artist),
            "PLAYLISTS" => hit["uuid"].as_str().map(|u| Page::Playlist(u.to_string())),
            _ => None,
        };
        if let Some(p) = page {
            out.push(Entry::Hit(title, p));
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
            s.nav.tab = SearchTab::All;
            go(s, Page::Search(q));
        }
        Entry::Hit(_, p) => go(s, p),
        Entry::Track(_, id) => controller::send_play_ids(vec![id]),
        Entry::ViewAll => {
            let q = s.nav.search.text.trim().to_string();
            s.nav.tab = SearchTab::All;
            go(s, Page::Search(q));
        }
    }
}

/// Keys in the search field: ↑/↓ move through the dropdown, Tab completes the text to the
/// highlighted (or first) suggestion, Enter opens the highlighted entry (or searches, through
/// `on_submit`), Escape closes the dropdown (SearchBar.tsx has Enter and Escape; arrows and Tab
/// go further).
fn search_key(s: &mut App, ev: &rune::KeyEvent, _tx: &mut Tx) -> bool {
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
                Entry::Text(t) | Entry::Hit(t, _) | Entry::Track(t, _) => Some(t.clone()),
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
        let base = |key: String, label: &str, h: f32| row().key(key).role("button").label(label).h(h).px(10.0).gap(10.0).radius(8.0).items_center()
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
            Entry::Track(t, id) => {
                hit_n += 1;
                let (tk, mk) = (format!("hit-{}", hit_n - 1), format!("hit-{}-more", hit_n - 1));
                let track = crate::interact::json_of_track(*id, t, "");
                row().key(tk).h(40.0).px(10.0).gap(10.0).radius(8.0).items_center().fill(if on { white(0.1) } else { rune::Color::TRANSPARENT })
                    .child(row().role("button").label(t.as_str()).grow(1.0).items_center().cursor(rune::Cursor::Pointer).preserve_focus_on_press(true)
                        .on_press(move |s: &mut App| pick(s, ec.clone()))
                        .child(text(t.clone()).font(t_(13.0, 600.0)).color(white(0.95)).a11y_hidden()))
                    .child(icon::<App>(&mk, "More options", G::More).preserve_focus_on_press(true)
                        .on_press(move |s: &mut App| crate::interact::open_track(s, track.clone(), None, crate::interact::Anchor::Below(mk.clone()))))
            }
            Entry::Hit(t, _) => {
                hit_n += 1;
                row().key(format!("hit-{}", hit_n - 1)).role("group").h(40.0).px(10.0).radius(8.0).items_center().cursor(rune::Cursor::Pointer)
                    .fill(if on { white(0.1) } else { rune::Color::TRANSPARENT }).hover(white(0.08)).preserve_focus_on_press(true)
                    .on_press(move |s: &mut App| pick(s, ec.clone())).child(heading(t, 13.0))
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
        col().key("suggest").w(420.0).anchored_fit("search-bar", rune::Side::Bottom, rune::Align::End, 8.0)
            .radius(14.0).fill(oklch(0.2, 0.02, 270.0)).shadow(8.0, 24.0, rune::oklcha(0.0, 0.0, 0.0, 0.4)).clip_rounded(14.0)
            .child(col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list)),
    )
}

fn t_(size: f32, weight: f32) -> rune::TextStyle {
    t(size, weight)
}

pub fn sidebar(s: &App) -> El {
    let nav_btn = |key: &str, label: &str, g: G, page: Page| {
        let on = s.nav.page == page;
        row().key(key.to_string()).role("button").label(label).h(40.0).px(12.0).gap(12.0).radius(10.0).items_center()
            .fill(if on { white(0.08) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| go(s, page.clone()))
            .child(glyph(g, 18.0, white(if on { 0.95 } else { 0.7 })))
            .child(text(label.to_string()).font(t(14.0, 600.0)).color(white(if on { 0.95 } else { 0.75 })).a11y_hidden())
    };
    let tab = |key: &str, label: &str, which: LibTab| {
        let on = s.nav.lib == which;
        row().key(key.to_string()).role("button").label(label).h(28.0).px(10.0).radius(14.0).items_center()
            .fill(if on { white(0.16) } else { white(0.06) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                s.nav.lib = which;
            })
            .child(text(label.to_string()).font(t(12.0, 600.0)).color(white(0.9)).a11y_hidden())
    };
    let sort_label = if s.nav.lib == LibTab::Playlists { "Sort by: Last updated" } else { "Sort by: Date added" };
    let mut list = col().w_full().gap(2.0);
    match s.nav.lib {
        LibTab::Playlists => {
            list = list.child(lib_row("lib-loved", "Loved Tracks Collection", "Loved Tracks", "Collection", 9, None, None, Some(Page::Loved)));
            if let Some(f) = data::ready(Key::Folders) {
                for (i, it) in f["items"].as_array().cloned().unwrap_or_default().iter().enumerate() {
                    let d = &it["data"];
                    let title = d["title"].as_str().or(it["name"].as_str()).unwrap_or("").to_string();
                    let n = d["numberOfTracks"].as_u64().unwrap_or(0);
                    let sub = format!("You · {n} Tracks");
                    let uuid = d["uuid"].as_str().unwrap_or("").to_string();
                    // The React row's accessible name is its cover's alt text, title and subtitle.
                    let label = format!("{title} {title} {sub}");
                    list = list.child(lib_row(&format!("lib-pl-{i}"), &label, &title, &sub, i as u64 + 20, crate::art::item_url(it, 40.0), Some(json!({"type": "PLAYLIST", "data": d.clone()})), Some(Page::Playlist(uuid))));
                }
            }
        }
        LibTab::Albums => list = list.child(lib_list(Key::FavAlbums)),
        LibTab::Artists => list = list.child(lib_list(Key::FavArtists)),
        LibTab::Mixes => list = list.child(lib_list(Key::FavMixes)),
    }
    col()
        .key("sidebar")
        .w(280.0)
        .h_full()
        .padding(12.0)
        .gap(6.0)
        .fill(oklch(0.13, 0.02, 270.0))
        .child(nav_btn("nav-home", "Home", G::Home, Page::Home))
        .child(nav_btn("nav-explore", "Explore", G::Compass, Page::Explore))
        .child(nav_btn("nav-feed", "Feed", G::Bell, Page::Feed))
        .child(el().h(10.0))
        .child(
            row().w_full().items_center().gap(6.0)
                .child(row().key("lib").role("button").label("Your Library").h(34.0).px(8.0).gap(10.0).items_center().cursor(rune::Cursor::Pointer)
                    .child(glyph(G::Library, 18.0, white(0.75))).child(text("Your Library").font(t(14.0, 650.0)).color(white(0.9)).a11y_hidden()))
                .child(spacer())
                .child(icon::<App>("lib-sort", sort_label, G::Sort))
                .child(row().key("lib-all").role("button").label("Show all").h(28.0).px(8.0).items_center().cursor(rune::Cursor::Pointer)
                    .child(text("Show all").font(t(12.0, 600.0)).color(white(0.6)).a11y_hidden())),
        )
        .child(row().gap(6.0).child(tab("tab-pl", "Playlists", LibTab::Playlists)).child(tab("tab-al", "Albums", LibTab::Albums)).child(tab("tab-ar", "Artists", LibTab::Artists)).child(tab("tab-mx", "Mixes", LibTab::Mixes)))
        .child(ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(list), "lib-scroll".into()))
}

/// Loved Tracks' artwork: not an image from TIDAL but React's fixed gradient (#450af5 → #8e2de2
/// → #00d2ff, to the bottom right) with a white heart.
pub(crate) fn loved_art(size: f32, radius: f32, heart: f32) -> El {
    let c = |h: &str| rune::color::hex(h).unwrap_or(white(0.5));
    el().size(size, size).radius(radius).clip_rounded(radius).items_center().justify_center().a11y_hidden()
        .linear_gradient(std::f32::consts::PI * 0.75, [(0.0, c("#450af5")), (0.5, c("#8e2de2")), (1.0, c("#00d2ff"))])
        .child(glyph(G::HeartFilled, heart, white(1.0)))
}

fn lib_row(key: &str, label: &str, title: &str, sub: &str, seed: u64, img: Option<String>, menu: Option<Value>, page: Option<Page>) -> El {
    let art = if key == "lib-loved" { loved_art(40.0, 6.0, 15.0) } else { ui::with_image(ui::tile(ui::hue(Some(seed)), 40.0, seed, 6.0), img, 40.0, 6.0) };
    let mut r = row().key(key.to_string()).role("button").label(label).w_full().h(52.0).px(8.0).gap(10.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
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

fn lib_list(key: Key) -> El {
    let Some(v) = data::ready(key.clone()) else { return loading() };
    let mut c = col().w_full().gap(2.0);
    for (i, it) in v["items"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let name = data::name(it);
        let page = item_page(it).or_else(|| data::data(it)["id"].as_u64().map(Page::Album));
        c = c.child(lib_row(&format!("lib-{key:?}-{i}"), &name, &name, &data::artist_of(it), i as u64 + 40, crate::art::item_url(it, 40.0), (key != Key::FavAlbums).then(|| it.clone()), page));
    }
    c
}

/// Player-bar extras the React bar has around the transport (React PlayerBar).
/// After the bar's title (PlayerBar's LikeButton and ContextMenuButton): the heart, filled in
/// the accent when the track is loved (no name and no toast, as React's), and More options,
/// the track menu below it.
pub fn bar_track_actions(v: &View) -> El {
    let Some(c) = &v.current else { return el() };
    use crate::interact::{self, Anchor, Fav, Say};
    // A video's heart is a video favourite (PlayerBar FavoriteButton); it has no track menu.
    if let Some(vi) = &v.video {
        let loved = interact::is_fav(Fav::Video, &vi.id.to_string());
        let (id, title) = (vi.id.to_string(), vi.title.clone());
        return el().key("bar-love").role("button").size(28.0, 28.0).radius(14.0).items_center().justify_center()
            .hover(white(0.06)).press_scale(0.9).cursor(rune::Cursor::Pointer)
            .on_press(move |_: &mut App| interact::toggle_fav(Fav::Video, id.clone(), title.clone(), Say::Nothing))
            .child(glyph(if loved { G::HeartFilled } else { G::Heart }, 16.0, if loved { accent() } else { white(0.45) }));
    }
    let loved = interact::is_fav(Fav::Track, &c.id.to_string());
    let (id, title) = (c.id.to_string(), c.title.clone());
    let cur = interact::json_of_track(c.id, &c.title, &c.artist);
    row().gap(2.0).items_center()
        .child(el().key("bar-love").role("button").size(28.0, 28.0).radius(14.0).items_center().justify_center()
            .hover(white(0.06)).press_scale(0.9).cursor(rune::Cursor::Pointer)
            .on_press(move |_: &mut App| interact::toggle_fav(Fav::Track, id.clone(), title.clone(), Say::Nothing))
            .child(glyph(if loved { G::HeartFilled } else { G::Heart }, 16.0, if loved { accent() } else { white(0.45) })))
        .child(ui::icon_button::<App>("more", "More options", G::More, 16.0, white(0.45))
            .on_press(move |s: &mut App| interact::open_track(s, cur.clone(), None, Anchor::Below("more".into()))))
}

pub fn bar_extras(v: &View) -> El {
    let mut r = row().gap(4.0).items_center();
    if v.current.is_some() {
        if let Some(src) = &v.playing_from {
            r = r.child(row().key("playing-from").role("button").label(src.clone()).h(28.0).px(8.0).items_center().cursor(rune::Cursor::Pointer)
                .child(text(src.clone()).font(t(11.0, 600.0)).color(white(0.6)).truncate().a11y_hidden()));
        }
        if let Some(b) = &v.badge {
            r = r.child(row().key("badge").role("button").label(b.clone()).h(22.0).px(8.0).radius(6.0).items_center().fill(white(0.1))
                .child(text(b.clone()).font(t(10.0, 650.0).tracking(0.6)).color(white(0.8)).a11y_hidden()));
        }
    }
    r = r.child(icon::<App>("lyrics", "Lyrics", G::Mic).on_press(|s: &mut App| toggle_drawer(s, Drawer::Lyrics)))
        .child(icon::<App>("queue", "Play queue", G::List).on_press(|s: &mut App| toggle_drawer(s, Drawer::Queue)));
    if v.current.is_some() {
        r = r            .child(if crate::mini_open() {
                icon::<App>("mini", "Close miniplayer", G::PopOut).on_press(|_: &mut App| crate::set_mini(false))
            } else {
                icon::<App>("mini", "Open miniplayer", G::PopOut).on_press(|_: &mut App| crate::set_mini(true))
            })
            .child({
                let full = icon::<App>("full", "Fullscreen player", G::Maximize);
                // PlayerBar MaximizeButton: for a video, the video player, maximised; a closed
                // (or restored) video starts again first.
                if v.video.is_some() {
                    let open = v.video_open;
                    full.on_press_tx(move |s: &mut App, tx: &mut Tx| {
                        if !open {
                            controller::send(Cmd::TogglePlay);
                        }
                        s.nav.video.expanded = true;
                        if !s.nav.video.fullscreen {
                            s.nav.video.fullscreen = true;
                            tx.toggle_maximized();
                        }
                    })
                } else {
                    full
                }
            });
    }
    r
}

// ------------------------------------------------------------------ pages

pub fn page(s: &App, v: &View) -> El {
    let body = match &s.nav.page {
        Page::Home => crate::home::home(s),
        Page::Explore => explore(),
        Page::ExploreSub(path, title) => explore_sub(path, title),
        Page::Mix(id, title, sub) => mix_page(id, title, sub, v),
        Page::Loved => loved_page(s, v),
        Page::Feed => feed(),
        Page::Search(q) => search_page(s, q),
        Page::Album(id) => album(*id, v),
        Page::Artist(id) => artist(*id, v),
        Page::Playlist(u) => playlist(s, u, v),
        Page::Settings => settings_page(s),
    };
    // `.cache()`: without it Rune culls content outside the viewport, from paint and from the
    // accessibility tree, so screen readers (and the verifier) would miss everything below the fold.
    // React's PageContainer: content at most 1520 pt wide, centred, so a wide window doesn't
    // stretch rows across the whole screen. Its width is measured for Home's grids and rows.
    let content = col().w_full().max_w(1520.0).px(24.0).py(24.0).gap(18.0).cache()
        .on_drawn_layout_tx(|s: &mut App, r, tx| {
            let w = (r[2] - 48.0).max(200.0);
            if (s.nav.content_w - w).abs() >= 1.0 { s.nav.content_w = w } else { tx.live_only() }
        })
        .child(body);
    ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(row().w_full().justify_center().child(content)), format!("page-{:?}", s.nav.page))
}

fn play_label(v: &View, source_name: &str) -> &'static str {
    if v.playing_from.as_deref() == Some(source_name) && v.current.is_some() {
        if v.playing { "Pause" } else { "Resume" }
    } else {
        "Play"
    }
}

fn hero(title: &str, title_heading: bool, sub: &str, seed: u64, round: bool, img: Option<String>, actions: El) -> El {
    hero_with(title, title_heading, text(sub.to_string()).font(t(13.0, 500.0)).color(white(0.6)), seed, round, img, actions)
}

fn hero_with(title: &str, title_heading: bool, sub: El, seed: u64, round: bool, img: Option<String>, actions: El) -> El {
    let h = ui::hue(Some(seed));
    let art = if round { ui::with_image(ui::art(h, 180.0, seed, 0.0), img, 180.0, 90.0) } else { ui::with_image(ui::tile(h, 180.0, seed, 14.0), img, 180.0, 14.0) };
    let title_el = if title_heading { heading(title, 30.0) } else { text(title.to_string()).font(t(30.0, 650.0)).color(white(0.95)) };
    row().w_full().gap(24.0).items_end()
        .child(art)
        .child(col().gap(10.0).min_w(0.0).child(title_el).child(sub).child(actions))
}

/// "Artist, Artist · 2013" under an album's title, each artist a link to their page, as the
/// React header's TrackArtists (there clickable spans; here also reachable by keyboard and
/// exposed as links).
fn album_byline(a: &Value) -> El {
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

fn play_buttons(v: &View, src_name: &str, play: Cmd, shuffle: Cmd) -> El {
    let label = play_label(v, src_name);
    let toggle = label != "Play";
    row().gap(8.0).items_center()
        .child(button::<App>("hero-play", label, Some(if label == "Pause" { G::Pause } else { G::Play }), true).on_press(move |_: &mut App| {
            if toggle { controller::send(Cmd::TogglePlay) } else { controller::send(play.clone()) }
        }))
        .child(button::<App>("hero-shuffle", "Shuffle", Some(G::Shuffle), false).on_press(move |_: &mut App| controller::send(shuffle.clone())))
}

fn album(id: u64, v: &View) -> El {
    let Some(p) = data::ready(Key::Album(id)) else { return loading() };
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
    let mut rows = col().w_full().gap(2.0);
    for (i, tr) in tracks.iter().enumerate() {
        let (all, src, idx) = (tracks.clone(), json!({"type": "album", "id": id, "name": title}), i);
        rows = rows.child(track_row(format!("al-t{i}"), i + 1, tr, move |_: &mut App| {
            let mut ordered = all[idx..].to_vec();
            ordered.extend_from_slice(&all[..idx]);
            controller::send(Cmd::PlayCollection { tracks: ordered, source: src.clone(), album_mode: true, shuffle: false });
        }));
    }
    c = c.child(rows);
    // TIDAL's editorial review, when the album has one (tidal.com shows it under the tracks).
    if let Some(review) = page["review"]["text"].as_str().filter(|r| !r.trim().is_empty()) {
        let mut r = col().w_full().max_w(820.0).gap(10.0).child(heading("Review", 20.0)).child(crate::bio::view(review, 14.0, 0.72));
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
            c = c.child(heading(sec["title"].as_str().unwrap_or(""), 20.0)).child(grid(cards));
        }
    }
    c
}

fn artist(id: u64, v: &View) -> El {
    let Some(p) = data::ready(Key::Artist(id)) else { return loading() };
    let a = data::data(&p["item"]);
    let name = a["name"].as_str().unwrap_or("").to_string();
    let play = Cmd::PlayArtist { id, name: name.clone(), shuffle: false };
    let shuf = Cmd::PlayArtist { id, name: name.clone(), shuffle: true };
    let actions = row().gap(8.0).items_center()
        .child(play_buttons(v, &name, play, shuf))
        .child({
            // ArtistPage: Plus/"Follow", or Check/"Following" in the accent
            let (on, aid, l) = (crate::interact::is_fav(crate::interact::Fav::Artist, &id.to_string()), id.to_string(), name.clone());
            header_btn("artist-follow", if on { "Following" } else { "Follow" }, if on { "Following" } else { "Follow" }, if on { G::Check } else { G::Plus }, on)
                .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Artist, aid.clone(), l.clone(), crate::interact::Say::Nothing))
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
            .child(row().key("bio-more").role("button").label("Read more").cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| s.nav.dialog = Some(Dialog::Bio(id)))
                .child(text("Read more").font(t(12.0, 650.0)).color(white(0.85)).a11y_hidden())));
    }
    for (si, sec) in p["items"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let title = sec["title"].as_str().unwrap_or("");
        let items = sec["items"].as_array().cloned().unwrap_or_default();
        c = c.child(heading(title, 20.0));
        let tracks: Vec<Value> = items.iter().filter(|it| data::kind(it) == "TRACK").map(|it| data::data(it).clone()).collect();
        if !tracks.is_empty() {
            let mut rows = col().w_full().gap(2.0);
            for (i, tr) in tracks.iter().enumerate() {
                let tid = tr["id"].as_u64().unwrap_or(0);
                rows = rows.child(track_row(format!("ar-s{si}-t{i}"), i + 1, tr, move |_: &mut App| controller::send_play_ids(vec![tid])));
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

fn playlist(s: &App, uuid: &str, v: &View) -> El {
    let Some(p) = data::ready(Key::Playlist(uuid.to_string())) else { return loading() };
    let d = &p["details"];
    let title = d["title"].as_str().unwrap_or("").to_string();
    let tracks: Vec<Value> = p["tracks"]["items"].as_array().cloned().unwrap_or_default();
    let source = json!({"type": "playlist", "id": uuid, "name": title});
    let play = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: false, shuffle: false };
    let shuf = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: false, shuffle: true };
    // PlaylistView: your own playlist has Edit and Private/Public; anyone else's, the heart.
    let me = data::user_id();
    let own = d["creator"]["id"].as_u64().is_some_and(|c| c == me) || d["creator"]["id"].is_null();
    let mut actions = row().gap(8.0).items_center().child(play_buttons(v, &title, play, shuf));
    if own {
        let public = d["accessType"].as_str() == Some("PUBLIC"); // PlaylistView: resolvedAccessType === "PUBLIC"
        actions = actions
            .child(header_btn("pl-edit", "Edit playlist", "Edit", G::Pencil, false))
            .child(header_btn("pl-public", if public { "Make playlist private" } else { "Make playlist public" }, if public { "Public" } else { "Private" }, if public { G::Unlock } else { G::Lock }, public));
    } else {
        let (on, u, l) = (crate::interact::is_fav(crate::interact::Fav::Playlist, uuid), uuid.to_string(), title.clone());
        actions = actions.child(header_btn("pl-fav", if on { "Remove from favorites" } else { "Add to favorites" }, if on { "Added" } else { "Add" }, if on { G::HeartFilled } else { G::Heart }, on)
            .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Playlist, u.clone(), l.clone(), crate::interact::Say::Nothing)));
    }
    let share_uuid = uuid.to_string();
    let item = json!({"type": "PLAYLIST", "data": d.clone()});
    actions = actions
        .child(header_btn("pl-share", "Share playlist", "Share", G::Share, false).on_press_tx(move |_: &mut App, tx: &mut Tx| {
            tx.copy_rich(rune::ClipboardContent::new(format!("https://tidal.com/playlist/{share_uuid}")));
            crate::interact::toast("Copied share link to clipboard".into(), crate::interact::Kind::Success);
        }))
        .child(header_btn("pl-more", "More options", "More", G::More, false)
            .on_press(move |s: &mut App| crate::interact::open_media(s, item.clone(), crate::interact::Anchor::Below("pl-more".into()))));
    let n = d["numberOfTracks"].as_u64().unwrap_or(tracks.len() as u64);
    let creator = d["creator"]["name"].as_str().unwrap_or("You");
    let filter = text_input(&s.nav.filter, t(13.0, 500.0).lh(18.0))
        .key("pl-filter")
        .label("Filter playlist on title, artist or album")
        .single_line()
        .w(320.0)
        .color(white(0.95))
        .caret(white(0.95), white(0.25))
        .placeholder("Filter", white(0.4))
        .on_edit(|s: &mut App| &mut s.nav.filter);
    let needle = s.nav.filter.text.to_lowercase();
    let mut rows = col().w_full().gap(2.0);
    for (i, tr) in tracks.iter().enumerate() {
        if !needle.is_empty() && !format!("{} {}", data::name(tr), data::artist_of(tr)).to_lowercase().contains(&needle) {
            continue;
        }
        let (all, src, idx) = (tracks.clone(), source.clone(), i);
        let in_pl = own.then(|| crate::interact::InPlaylist { uuid: uuid.to_string(), index: i });
        rows = rows.child(track_row_with(format!("pl-t{i}"), i + 1, data::data(tr), true, in_pl, move |_: &mut App| {
            let ordered = all[idx..].iter().chain(all[..idx].iter()).cloned().collect();
            controller::send(Cmd::PlayCollection { tracks: ordered, source: src.clone(), album_mode: false, shuffle: false });
        }));
    }
    col().w_full().gap(16.0)
        .child(hero(&title, true, &format!("{creator} · {n} tracks"), 30, false, crate::art::item_url(d, 180.0), actions))
        .child(row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).child(filter))
        .child(rows)
}

fn search_page(s: &App, q: &str) -> El {
    let Some(v) = data::ready(Key::Search(q.to_string())) else { return loading() };
    let tab = |key: &str, label: &str, which: SearchTab| {
        let on = s.nav.tab == which;
        row().key(key.to_string()).role("button").label(label).h(30.0).px(12.0).radius(15.0).items_center()
            .fill(if on { white(0.9) } else { white(0.08) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| s.nav.tab = which)
            .child(text(label.to_string()).font(t(12.0, 600.0)).color(if on { oklch(0.2, 0.02, 270.0) } else { white(0.9) }).a11y_hidden())
    };
    let tabs = row().gap(6.0).flex_wrap()
        .child(tab("st-all", "All Results", SearchTab::All))
        .child(tab("st-top", "Top Hits", SearchTab::TopHits))
        .child(tab("st-tracks", "Tracks", SearchTab::Tracks))
        .child(tab("st-pl", "Playlists", SearchTab::Playlists))
        .child(tab("st-al", "Albums", SearchTab::Albums))
        .child(tab("st-ar", "Artists", SearchTab::Artists))
        .child(tab("st-vid", "Videos", SearchTab::Videos));
    let tracks: Vec<Value> = v["tracks"].as_array().cloned().unwrap_or_default();
    let typed = |k: &str, arr: &str| -> Vec<Value> { v[arr].as_array().cloned().unwrap_or_default().into_iter().map(|d| json!({"type": k, "data": d})).collect() };
    let (albums, artists, playlists) = (typed("ALBUM", "albums"), typed("ARTIST", "artists"), typed("PLAYLIST", "playlists"));
    let videos: Vec<Value> = v["videos"]["items"].as_array().or(v["videos"].as_array()).cloned().unwrap_or_default();
    let video_source = json!({"type": "search", "id": q, "name": format!("Search: {q}")});
    let track_rows = |limit: usize| {
        let mut rows = col().w_full().gap(2.0);
        for (i, tr) in tracks.iter().take(limit).enumerate() {
            let id = tr["id"].as_u64().unwrap_or(0);
            rows = rows.child(track_row(format!("sr-t{i}"), i + 1, tr, move |_: &mut App| controller::send_play_ids(vec![id])));
        }
        rows
    };
    let section = |title: &str, body: El| col().w_full().gap(10.0).child(row().items_center().child(heading(title, 18.0)).child(spacer()).child(
        row().role("button").label("VIEW ALL").h(26.0).px(8.0).items_center().cursor(rune::Cursor::Pointer).child(caps("View all", 0.6).a11y_hidden()))).child(body);
    let body = match s.nav.tab {
        SearchTab::All | SearchTab::TopHits => col().w_full().gap(22.0)
            .child(section("Tracks", track_rows(5)))
            .child(section("Albums", grid(cards_for("sr-al", &albums))))
            .child(section("Artists", grid(cards_for("sr-ar", &artists))))
            .child(section("Playlists", grid(cards_for("sr-pl", &playlists))))
            .child(if videos.is_empty() { el() } else { section("Videos", grid(crate::video_ui::video_cards("sr-vid", &videos[..videos.len().min(4)], video_source.clone()))) }),
        SearchTab::Tracks => track_rows(usize::MAX),
        SearchTab::Albums => grid(cards_for("sr-al", &albums)),
        SearchTab::Artists => grid(cards_for("sr-ar", &artists)),
        SearchTab::Playlists => grid(cards_for("sr-pl", &playlists)),
        SearchTab::Videos if !videos.is_empty() => grid(crate::video_ui::video_cards("sr-vid", &videos, video_source.clone())),
        SearchTab::Videos => text("No videos").font(t(13.0, 500.0)).color(white(0.5)),
    };
    col().w_full().gap(16.0).child(heading(&format!("Results for \"{q}\""), 26.0)).child(tabs).child(body)
}

// ------------------------------------------------------------------ account menu, settings, queue

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsTab {
    Playback,
    General,
    Network,
    Scrobbling,
    Discord,
    Mcp,
    Overlay,
    Themes,
    Utilities,
}

fn menu_item(key: &str, label: &str, checked: Option<bool>, on: impl Fn(&mut App) + 'static) -> El {
    // Toggles (checked is Some) keep the menu open; actions close it.
    let closes = checked.is_none();
    let mut r = row().key(key.to_string()).role("button").label(label).w_full().h(36.0).px(12.0).gap(10.0).radius(8.0).items_center()
        .hover(white(0.08)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            if closes {
                s.nav.menu_open = false;
            }
            on(s)
        })
        .child(text(label.to_string()).font(t(13.0, 500.0)).color(white(0.9)).a11y_hidden());
    if let Some(c) = checked {
        r = r.child(spacer()).child(text(if c { "On" } else { "Off" }).font(t(11.0, 600.0)).color(white(0.5)).a11y_hidden());
    }
    r
}

/// The Account button's menu (React AccountMenu).
pub fn account_menu(s: &App, v: &View) -> Option<El> {
    if !s.nav.menu_open {
        return None;
    }
    let name = data::ready(Key::Profile).and_then(|p| p.get(0).and_then(|n| n.as_str()).map(str::to_string)).unwrap_or_default();
    let (bp, ex) = (v.bit_perfect, v.exclusive);
    let mut m = col().key("account-menu").w(260.0).padding(8.0).gap(2.0).radius(14.0).fill(oklch(0.2, 0.02, 270.0)).shadow(8.0, 24.0, rune::oklcha(0.0, 0.0, 0.0, 0.45));
    if !name.is_empty() {
        m = m.child(menu_item("am-profile", &name, None, |_: &mut App| {}));
    }
    m = m
        .child(menu_item("am-exclusive", "Exclusive output", Some(ex), move |_: &mut App| controller::send(Cmd::SetExclusive(!ex))))
        .child(menu_item("am-bitperfect", "Bit-perfect", Some(bp), move |_: &mut App| controller::send(Cmd::SetBitPerfect(!bp))))
        .child(menu_item("am-settings", "Settings", None, |s: &mut App| {
            s.nav.settings_tab = SettingsTab::Playback;
            s.nav.settings_open = true;
            load_setting_fields(s);
        }))
        .child(menu_item("am-shortcuts", "Shortcuts", None, |s: &mut App| s.nav.dialog = Some(Dialog::Shortcuts)))
        .child(menu_item("am-about", "About", None, |s: &mut App| s.nav.dialog = Some(Dialog::About)))
        .child(menu_item("am-logout", "Log out", None, |_: &mut App| {}));
    // A modal layer over the window: the page is inert while the menu is open, and a press
    // anywhere outside the menu closes it.
    Some(el().key("am-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .modal(false, |s: &mut App| s.nav.menu_open = false)
        .on_press(|s: &mut App| s.nav.menu_open = false).on_secondary_press(|s: &mut App, _, _| s.nav.menu_open = false)
        .child(row().pin(None, Some(56.0), Some(120.0), None).child(m.role("menu").label("Account").on_press(|_: &mut App| {}))))
}

fn choice(key: &str, label: &str, selected: bool, on: impl Fn(&mut App) + 'static) -> El {
    row().key(key.to_string()).role("button").label(label).w_full().h(48.0).px(14.0).radius(10.0).items_center()
        .fill(if selected { white(0.12) } else { white(0.05) }).hover(white(0.1)).cursor(rune::Cursor::Pointer)
        .on_press(on)
        .child(text(label.to_string()).font(t(13.0, 500.0)).color(white(0.9)).a11y_hidden())
}

fn field(s: &App, key: &str, label: &str) -> El {
    // A field that isn't wired to a setting yet (still real and named).
    let _ = s;
    let edit = rune::TextEdit::new("");
    row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).w(320.0)
        .child(text_input(&edit, t(13.0, 500.0).lh(18.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95)).placeholder(label, white(0.4)))
}

/// A settings field bound to one of the Nav edits; `changed` runs on every edit.
fn bound_field(edit: &TextEdit, key: &str, label: &str, get: fn(&mut App) -> &mut TextEdit, changed: fn(&mut App)) -> El {
    row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).w(320.0)
        .child(text_input(edit, t(13.0, 500.0).lh(18.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95)).placeholder(label, white(0.4))
            .on_edit(get)
            .on_input_change(move |s: &mut App, _| changed(s)))
}

/// Fill the editable fields from what's saved (as the React tabs do on mount).
pub fn load_setting_fields(s: &mut App) {
    use sone_host_shim::Manager;
    let st = crate::host().state::<tauri_app_lib::AppState>();
    if let Some(set) = st.load_settings() {
        s.nav.discord_status = TextEdit::new(&set.discord_status_text);
        s.nav.overlay_host = TextEdit::new(&set.overlay_host);
        s.nav.proxy_user = TextEdit::new(set.proxy.username.as_deref().unwrap_or(""));
    }
}

/// Run the debounced saves that are due (called from App::poll).
pub fn flush_settings(s: &mut App) {
    use sone_host_shim::Manager;
    let now = std::time::Instant::now();
    if s.nav.discord_due.is_some_and(|t| now >= t) {
        s.nav.discord_due = None;
        let text = s.nav.discord_status.text.clone();
        let _ = tauri_app_lib::commands::utility::set_discord_status_text(crate::host().state::<tauri_app_lib::AppState>(), text);
    }
    if s.nav.proxy_due.is_some_and(|t| now >= t) {
        s.nav.proxy_due = None;
        let st = crate::host().state::<tauri_app_lib::AppState>();
        let mut p = tauri_app_lib::commands::utility::get_proxy_settings(st);
        let user = s.nav.proxy_user.text.trim().to_string();
        p.username = (!user.is_empty()).then_some(user);
        // React's shouldSubmitProxy: half-typed enabled settings are withheld.
        if !p.enabled || (!p.host.trim().is_empty() && p.port > 0) {
            let h = crate::host().clone();
            sone_host_shim::async_runtime::spawn(async move {
                let _ = tauri_app_lib::commands::utility::set_proxy_settings(h.state::<tauri_app_lib::AppState>(), p).await;
            });
        }
    }
}

pub(crate) fn open_url(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// The Keyboard Shortcuts and About dialogs (React ShortcutsModal / AboutModal).
pub fn dialog(s: &App) -> Option<El> {
    let d = s.nav.dialog?;
    let body = match d {
        Dialog::Shortcuts => {
            let mut rows = col().w_full().gap(2.0);
            for (label, keys) in [("Play / Pause", "Space"), ("Next track", "Ctrl+Right"), ("Previous track", "Ctrl+Left"), ("Volume up", "Ctrl+Up"),
                ("Volume down", "Ctrl+Down"), ("Mute / Unmute", "Ctrl+M"), ("Like / Unlike current track", "Ctrl+L"), ("Shuffle on / off", "Ctrl+S"),
                ("Repeat off / all / one", "Ctrl+R"), ("Focus search bar", "Ctrl+K"), ("Refresh app data", "Ctrl+Shift+R"), ("Zoom in", "Ctrl+="),
                ("Zoom out", "Ctrl+-"), ("Reset zoom to 100%", "Ctrl+0"), ("Toggle exclusive output", "Ctrl+E"), ("Toggle bit-perfect mode", "Ctrl+B"),
                ("Show keyboard shortcuts", "Ctrl+/"), ("Close window", "Ctrl+W"), ("Quit SONE", "Ctrl+Q")] {
                rows = rows.child(row().w_full().h(30.0).items_center()
                    .child(text(label.to_string()).font(t(13.0, 500.0)).color(white(0.85)))
                    .child(spacer())
                    .child(text(keys.to_string()).font(t(12.0, 600.0)).color(white(0.6))));
            }
            col().w_full().gap(14.0)
                .child(heading("Keyboard Shortcuts", 22.0))
                .child(rows)
                .child(button::<App>("sc-restore", "Restore defaults", None, false))
        }
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
            col().w_full().gap(16.0)
                .child(row().gap(12.0).items_center().child(avatar)
                    .child(col().gap(2.0).child(heading(&name, 16.0)).child(text("Biography").font(t(13.0, 400.0)).color(white(0.5)))))
                .child(col().key("bio-scroll").w_full().max_h(520.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
        }
        Dialog::About => {
            let link = |key: &str, label: &str, url: &'static str| button::<App>(key, label, None, false).on_press(move |_: &mut App| open_url(url));
            let version = concat!("v", env!("CARGO_PKG_VERSION"));
            col().w_full().gap(12.0).items_center()
                .child(heading(crate::TITLE, 30.0))
                .child(link("ab-version", version, "https://github.com/lullabyX/sone/releases"))
                .child(text("The native desktop client for TIDAL on Linux.").font(t(13.0, 500.0)).color(white(0.6)))
                .child(row().gap(8.0)
                    .child(link("ab-github", "GitHub", "https://github.com/lullabyX/sone"))
                    .child(link("ab-issues", "Issues", "https://github.com/lullabyX/sone/issues"))
                    .child(link("ab-license", "GPL-3.0", "https://github.com/lullabyX/sone/blob/master/LICENSE")))
                .child(link("ab-author", "@lullabyX", "https://github.com/lullabyX"))
                .child(link("ab-patreon", "Support development on Patreon", "https://patreon.com/lullabyX"))
        }
    };
    let wide = matches!(d, Dialog::Bio(_));
    let panel = col().key("dialog").w(if wide { 640.0 } else { 520.0 }).padding(24.0).radius(18.0).fill(oklch(0.17, 0.02, 270.0)).shadow(10.0, 40.0, rune::oklcha(0.0, 0.0, 0.0, 0.5))
        .child(row().w_full().child(spacer()).child(ui::icon_button::<App>("dialog-close", "Close dialog", G::Close, 14.0, white(0.6)).on_press(|s: &mut App| s.nav.dialog = None)))
        .child(body);
    Some(row().key("dialog-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).items_center().justify_center().fill(rune::oklcha(0.0, 0.0, 0.0, 0.45))
        .modal(false, |s: &mut App| s.nav.dialog = None)
        .on_press(|s: &mut App| s.nav.dialog = None)
        .child(panel.role("dialog").label(match d { Dialog::Shortcuts => "Keyboard shortcuts", Dialog::About => "About SONE", Dialog::Bio(_) => "Biography" }).on_press(|_: &mut App| {})))
}

/// The settings modal, when open.
pub fn settings_modal(s: &App) -> Option<El> {
    if !s.nav.settings_open {
        return None;
    }
    let panel = col().key("settings-modal").w(860.0).h(620.0).padding(24.0).radius(18.0).fill(oklch(0.17, 0.02, 270.0)).shadow(10.0, 40.0, rune::oklcha(0.0, 0.0, 0.0, 0.5))
        .child(ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(settings_page(s)), "settings-scroll".into()));
    // modal: the page behind is inert; a press on the scrim (outside the panel) closes it
    Some(row().key("settings-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).items_center().justify_center().fill(rune::oklcha(0.0, 0.0, 0.0, 0.45))
        .modal(false, |s: &mut App| s.nav.settings_open = false)
        .on_press(|s: &mut App| s.nav.settings_open = false)
        .child(panel.role("dialog").label("Settings").on_press(|_: &mut App| {})))
}

pub fn settings_page(s: &App) -> El {
    let tab = |key: &str, label: &str, which: SettingsTab| {
        let on = s.nav.settings_tab == which;
        row().key(key.to_string()).role("button").label(label).h(34.0).px(12.0).radius(10.0).items_center().w_full()
            .fill(if on { white(0.1) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| s.nav.settings_tab = which)
            .child(text(label.to_string()).font(t(13.0, 600.0)).color(white(0.85)).a11y_hidden())
    };
    let tabs = col().w(200.0).gap(2.0)
        .child(tab("set-playback", "Playback", SettingsTab::Playback))
        .child(tab("set-general", "General", SettingsTab::General))
        .child(tab("set-network", "Network", SettingsTab::Network))
        .child(tab("set-scrobbling", "Scrobbling", SettingsTab::Scrobbling))
        .child(tab("set-discord", "Discord", SettingsTab::Discord))
        .child(tab("set-mcp", "MCP", SettingsTab::Mcp))
        .child(tab("set-overlay", "OBS Overlay", SettingsTab::Overlay))
        .child(tab("set-themes", "Themes", SettingsTab::Themes))
        .child(tab("set-utilities", "Utilities", SettingsTab::Utilities));
    let quality = controller::max_quality();
    let body = match s.nav.settings_tab {
        SettingsTab::Playback => {
            let q = |key: &str, label: &str, tier: &'static str| choice(key, label, quality == tier, move |_: &mut App| controller::send(Cmd::SetMaxQuality(tier.to_string())));
            col().w_full().gap(8.0)
                .child(caps("Streaming quality", 0.5))
                .child(q("q-hires", "HI-RES LOSSLESS FLAC · up to 24-bit / 192 kHz", "HI_RES_LOSSLESS"))
                .child(q("q-lossless", "LOSSLESS FLAC · 16-bit / 44.1 kHz", "LOSSLESS"))
                .child(q("q-high", "HIGH AAC · up to 320 kbps", "HIGH"))
        }
        SettingsTab::General => col().w_full().gap(8.0)
            .child(caps("Title bar", 0.5))
            .child(choice("tb-custom", "Custom SONE's own in-app title bar, themed to match.", true, |_: &mut App| {}))
            .child(choice("tb-hidden", "Hidden Frameless — no title bar at all.", false, |_: &mut App| {}))
            .child(choice("tb-system", "System Native OS title bar and window controls.", false, |_: &mut App| {})),
        SettingsTab::Network => col().w_full().gap(10.0)
            .child(caps("Proxy", 0.5))
            .child(button::<App>("px-http", "HTTP", None, false))
            .child(field(s, "px-host", "host"))
            .child(field(s, "px-port", "port"))
            .child(bound_field(&s.nav.proxy_user, "px-user", "Username", |s: &mut App| &mut s.nav.proxy_user,
                |s: &mut App| s.nav.proxy_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(500))))
            .child(button::<App>("px-test", "Test connection", None, false)),
        SettingsTab::Scrobbling => {
            use sone_host_shim::Manager;
            let connected = crate::host().state::<tauri_app_lib::AppState>().load_settings()
                .and_then(|x| x.scrobble.listenbrainz.map(|c| c.username));
            let mut lb = if let Some(user) = connected {
                col().w_full().gap(10.0).child(caps("ListenBrainz", 0.5))
                    .child(text(format!("Connected as {user}")).font(t(13.0, 500.0)).color(white(0.8)))
                    .child(button::<App>("lb-disconnect", "Disconnect", None, false).on_press(|_: &mut App| {
                        let h = crate::host().clone();
                        sone_host_shim::async_runtime::spawn(async move {
                            let _ = tauri_app_lib::commands::scrobble::disconnect_provider(h.state::<tauri_app_lib::AppState>(), "listenbrainz".into()).await;
                            controller::bump();
                        });
                    }))
            } else { col().w_full().gap(10.0)
                .child(caps("ListenBrainz", 0.5))
                .child(bound_field(&s.nav.lb_token, "lb-token", "User token", |s: &mut App| &mut s.nav.lb_token, |_: &mut App| {}))
                .child(row().role("button").label("listenbrainz.org/settings").cursor(rune::Cursor::Pointer).on_press(|_: &mut App| open_url("https://listenbrainz.org/settings/"))
                    .child(text("listenbrainz.org/settings").font(t(12.0, 500.0)).color(white(0.6)).a11y_hidden()))
                .child(button::<App>("lb-connect", "Connect", None, true).on_press(|s: &mut App| {
                    use sone_host_shim::Manager;
                    let token = s.nav.lb_token.text.trim().to_string();
                    if token.is_empty() {
                        return;
                    }
                    let h = crate::host().clone();
                    sone_host_shim::async_runtime::spawn(async move {
                        let _ = tauri_app_lib::commands::scrobble::connect_listenbrainz(h.state::<tauri_app_lib::AppState>(), token).await;
                        controller::bump(); // saved settings changed: redraw the tab
                    });
                }))
            };
            if let Some(st) = &s.nav.lb_status {
                lb = lb.child(text(st.clone()).font(t(12.0, 500.0)).color(white(0.6)));
            }
            // Last.fm and Libre.fm come first, each with its own Connect (browser sign-in).
            col().w_full().gap(18.0)
                .child(col().gap(8.0).child(caps("Last.fm", 0.5)).child(button::<App>("lf-connect", "Connect", None, false)))
                .child(col().gap(8.0).child(caps("Libre.fm", 0.5)).child(button::<App>("lbfm-connect", "Connect", None, false)))
                .child(lb)
        }
        SettingsTab::Discord => {
            let chip = |key: &str, label: &str| row().key(key.to_string()).role("button").label(label).h(26.0).px(10.0).radius(13.0).items_center().fill(white(0.08)).cursor(rune::Cursor::Pointer)
                .child(text(label.to_string()).font(t(11.0, 600.0)).color(white(0.85)).a11y_hidden());
            col().w_full().gap(10.0)
                .child(caps("Status text", 0.5))
                .child(bound_field(&s.nav.discord_status, "dc-status", "{track} by {artist} on {album}", |s: &mut App| &mut s.nav.discord_status,
                    |s: &mut App| s.nav.discord_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(500))))
                .child(row().gap(6.0).child(chip("dc-track", "{track}")).child(chip("dc-artist", "{artist}")).child(chip("dc-album", "{album}")))
        }
        SettingsTab::Mcp => col().w_full().gap(10.0)
            .child(caps("MCP server", 0.5))
            .child(row().gap(8.0)
                .child(button::<App>("mcp-reveal", "Reveal token", None, false))
                .child(button::<App>("mcp-copy", "Copy", None, false))
                .child(button::<App>("mcp-regen", "Regenerate token", None, false))),
        SettingsTab::Overlay => col().w_full().gap(10.0)
            .child(caps("Overlay server", 0.5))
            .child(bound_field(&s.nav.overlay_host, "ov-host", "127.0.0.1", |s: &mut App| &mut s.nav.overlay_host, |_: &mut App| {}))
            .child(button::<App>("ov-apply", "Apply", None, true).on_press(|s: &mut App| {
                // As OverlayTab's applyHost: always overlay_set_host, which validates, saves, and
                // restarts the server only if it's running.
                use sone_host_shim::Manager;
                let host = s.nav.overlay_host.text.trim().to_string();
                if !host.is_empty() {
                    let h = crate::host().clone();
                    sone_host_shim::async_runtime::spawn(async move {
                        let _ = tauri_app_lib::commands::overlay::overlay_set_host(h.state::<tauri_app_lib::AppState>(), host).await;
                    });
                }
            })),
        SettingsTab::Themes => {
            let mut presets = row().w_full().gap(8.0).flex_wrap();
            for (i, name) in ["Midnight Cyan", "Violet Night", "Cyberpunk", "Noir", "Ocean", "Forest", "Ember", "Rose", "Copper", "Sakura", "Blossom", "Meadow", "Snowfall", "Daylight", "Paper"].iter().enumerate() {
                presets = presets.child(row().key(format!("th-{i}")).role("button").label(*name).h(34.0).px(12.0).radius(10.0).items_center().fill(white(0.07)).cursor(rune::Cursor::Pointer)
                    .child(text(name.to_string()).font(t(12.0, 600.0)).color(white(0.85)).a11y_hidden()));
            }
            col().w_full().gap(12.0)
                .child(heading("PREVIEW", 11.0))
                .child(el().w_full().h(60.0).radius(12.0).fill(ui::base(270.0)))
                .child(heading("PRESETS", 11.0))
                .child(presets)
                .child(heading("CUSTOM", 11.0))
                .child(row().gap(8.0)
                    .child(button::<App>("th-accent", "Pick accent color", None, false))
                    .child(button::<App>("th-bg", "Pick background color", None, false))
                    .child(button::<App>("th-reset", "Reset to default", None, false)))
        }
        SettingsTab::Utilities => col().w_full().gap(10.0)
            .child(caps("Cache and logs", 0.5))
            .child(row().gap(8.0)
                .child(button::<App>("ut-open", "Open folder", None, false))
                .child(button::<App>("ut-refresh", "Refresh", None, false))),
    };
    col().w_full().gap(18.0).child(heading("Settings", 28.0)).child(row().w_full().gap(28.0).items_start().child(tabs).child(col().grow(1.0).min_w(0.0).child(body)))
}

/// The play queue panel (React QueuePanel).
pub fn queue_panel(v: &View) -> El {
    let mut c = col().w_full().gap(6.0);
    if let Some(cur) = &v.current {
        c = c.child(heading("NOW PLAYING", 11.0))
            .child(row().w_full().gap(10.0).items_center().padding(6.0)
                .child(ui::with_image(ui::tile(ui::hue(Some(cur.id)), 44.0, cur.id, 6.0), crate::art::cover(v.cover.as_deref(), 44.0), 44.0, 6.0))
                .child(col().grow(1.0).min_w(0.0).child(heading(&cur.title, 14.0)).child(text(cur.artist.clone()).font(t(12.0, 400.0)).color(white(0.55))))
                .child({
                    // the drawer's heart toasts briefly (NowPlayingDrawer)
                    let (on, tid, l) = (crate::interact::is_fav(crate::interact::Fav::Track, &cur.id.to_string()), cur.id.to_string(), cur.title.clone());
                    ui::icon_button::<App>("qp-love", if on { "Remove from Loved tracks" } else { "Add to Loved tracks" }, if on { G::HeartFilled } else { G::Heart }, 15.0, if on { accent() } else { white(0.75) })
                        .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Track, tid.clone(), l.clone(), crate::interact::Say::LovedShort))
                }));
    }
    if !v.queue.is_empty() {
        c = c.child(row().w_full().items_center().pt(10.0).child(heading("NEXT UP", 11.0)).child(spacer())
            .child(row().key("qp-clear").role("button").label("Clear").h(26.0).px(10.0).radius(13.0).items_center().fill(white(0.08)).cursor(rune::Cursor::Pointer)
                .on_press(|_: &mut App| controller::send(Cmd::ClearQueue)).child(text("Clear").font(t(11.0, 600.0)).color(white(0.8)).a11y_hidden())));
        for (i, item) in v.queue.iter().enumerate() {
            let id = item.id;
            c = c.child(row().key(format!("qp-{i}-{id}")).role("presentation").w_full().h(44.0).px(6.0).gap(10.0).items_center().radius(8.0).hover(white(0.05)).cursor(rune::Cursor::Pointer)
                .on_press(move |_: &mut App| controller::send(Cmd::JumpTo(id)))
                .child(col().grow(1.0).min_w(0.0).child(text(item.title.clone()).font(t(13.0, 500.0)).color(white(0.9)).truncate()).child(text(item.artist.clone()).font(t(11.0, 400.0)).color(white(0.5)).truncate()))
                .child(icon::<App>(&format!("qp-{i}-rm"), "Remove", G::Close).on_press(move |_: &mut App| controller::send(Cmd::RemoveAt(i)))));
        }
    }
    if !v.history.is_empty() {
        c = c.child(row().pt(10.0).child(heading("HISTORY", 11.0)));
        for (i, item) in v.history.iter().enumerate() {
            c = c.child(row().key(format!("qh-{i}")).role("presentation").w_full().h(40.0).px(6.0).items_center()
                .child(text(item.title.clone()).font(t(12.0, 500.0)).color(white(0.6)).truncate()));
        }
    }
    c
}

fn toggle_drawer(s: &mut App, tab: Drawer) {
    s.nav.drawer = if s.nav.drawer == Some(tab) { None } else { Some(tab) };
}

/// The right-hand drawer (React NowPlayingDrawer): queue, suggestions, lyrics, credits.
pub fn drawer(s: &App, v: &View) -> Option<El> {
    let tab = s.nav.drawer?;
    let t_btn = |key: &str, label: &str, which: Drawer| {
        let on = tab == which;
        row().key(key.to_string()).role("button").label(label).h(30.0).px(10.0).radius(15.0).items_center()
            .fill(if on { white(0.14) } else { white(0.05) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| s.nav.drawer = Some(which))
            .child(text(label.to_string()).font(t(11.0, 600.0)).color(white(0.85)).a11y_hidden())
    };
    let tabs = row().gap(6.0).flex_wrap()
        .child(t_btn("dr-queue", "Play queue", Drawer::Queue))
        .child(t_btn("dr-suggested", "Suggested tracks", Drawer::Suggested))
        .child(t_btn("dr-lyrics", "Lyrics", Drawer::Lyrics))
        .child(t_btn("dr-credits", "Credits", Drawer::Credits));
    let body = match tab {
        Drawer::Queue => queue_panel(v),
        Drawer::Lyrics => lyrics(s, v),
        Drawer::Suggested => suggested(v),
        Drawer::Credits => credits(v),
    };
    let mut head = col().w_full().gap(10.0).child(tabs);
    if let Some(cur) = &v.current {
        head = head.child(heading(&cur.title, 16.0));
    }
    let mut d = col().key("drawer").w(360.0).h_full().padding(16.0).gap(12.0).fill(oklch(0.14, 0.02, 270.0))
        .child(head)
        .child(ui::smooth_wheel::<App>(col().grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false }).child(body), "drawer-scroll".into()));
    if tab == Drawer::Lyrics && s.nav.lyrics_scrolled && v.current.as_ref().and_then(|c| synced_lyrics(c.id)).is_some() {
        // LyricsTab: the floating "Sync lyrics" button once the user scrolls away
        d = d.child(row().key("ly-sync").role("button").label("Sync lyrics").pin(None, None, Some(32.0), Some(16.0)).h(36.0).px(16.0).gap(8.0).radius(18.0).items_center()
            .fill(accent()).shadow(6.0, 18.0, rune::oklcha(0.0, 0.0, 0.0, 0.4)).cursor(rune::Cursor::Pointer).press_scale(0.95)
            .on_press_tx(|s: &mut App, tx: &mut Tx| {
                s.nav.lyrics_scrolled = false;
                if let Some(i) = s.nav.lyric_line {
                    tx.scroll_center(format!("ly-{i}"));
                }
            })
            .child(glyph(G::Mic, 14.0, oklch(0.18, 0.02, 270.0)))
            .child(text("Sync lyrics").font(t(12.0, 700.0)).color(oklch(0.18, 0.02, 270.0)).a11y_hidden()));
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
    let Some(cur) = &v.current else { return text("Nothing playing").font(t(13.0, 500.0)).color(white(0.5)) };
    let mut c = col().w_full().gap(10.0).child(heading(&cur.title, 18.0));
    match data::want(Key::Lyrics(cur.id)) {
        data::Load::Ready(l) => {
            let provider = l["lyricsProvider"].as_str().map(str::to_string);
            if let Some(lines) = synced_lyrics(cur.id) {
                let active = s.nav.lyric_line;
                let duration = v.duration.max(1.0);
                let mut col_lines = col().w_full().gap(12.0).py(40.0)
                    .on_wheel(|s: &mut App, _, _| {
                        s.nav.lyrics_scrolled = true;
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
                                s.nav.lyrics_scrolled = false;
                                controller::send(Cmd::SeekFraction((at / duration).clamp(0.0, 1.0) as f32));
                            }),
                    );
                }
                c = c.child(col_lines.child(el().h(160.0)));
            } else {
                let plain = l["lyrics"].as_str().unwrap_or("").trim().to_string();
                if plain.is_empty() {
                    c = c.child(text("No lyrics available for this track").font(t(13.0, 500.0)).color(white(0.5)));
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
        data::Load::Failed(_) => c = c.child(text("No lyrics available for this track").font(t(13.0, 500.0)).color(white(0.5))),
        data::Load::Loading => c = c.child(loading()),
    }
    c
}

/// Explore (React ExplorePage): pill rows per category with View all, plus the icon grid.
fn explore() -> El {
    let Some(v) = data::ready(Key::Page("pages/explore".into())) else { return loading() };
    let mut c = col().w_full().gap(20.0).child(heading("Explore", 28.0));
    for (si, sec) in v["sections"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let title = sec["title"].as_str().unwrap_or("").to_string();
        let mut head = row().w_full().items_center();
        // The icon grid (PAGE_LINKS, "Shortcuts") shows no heading in the React page.
        if !title.is_empty() && sec["sectionType"].as_str() != Some("PAGE_LINKS") {
            head = head.child(heading(&title, 18.0));
        }
        if let Some(path) = sec["apiPath"].as_str().filter(|p| !p.is_empty()) {
            let (path, t2) = (path.to_string(), title.clone());
            head = head.child(spacer()).child(row().key(format!("ex-all-{si}")).role("button").label("View all").h(26.0).px(8.0).items_center().cursor(rune::Cursor::Pointer)
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

fn pill_link(key: String, it: &Value) -> El {
    let label = it["title"].as_str().or(it["name"].as_str()).unwrap_or("").to_string();
    let path = it["apiPath"].as_str().unwrap_or("").to_string();
    let t2 = label.clone();
    row().key(key).role("button").label(label.clone()).h(38.0).px(16.0).radius(19.0).items_center().fill(white(0.08)).hover(white(0.14)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| {
            if !path.is_empty() {
                go(s, Page::ExploreSub(path.clone(), t2.clone()))
            }
        })
        .child(text(label).font(t(13.0, 600.0)).color(white(0.9)).a11y_hidden())
}

/// An Explore sub-page: a genre/mood page (catalogue rows) or a view-all page (more links).
fn explore_sub(path: &str, title: &str) -> El {
    let Some(v) = data::ready(Key::Page(path.to_string())) else { return loading() };
    let mut c = col().w_full().gap(20.0).child(heading(title, 28.0));
    for (si, sec) in v["sections"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let st = sec["title"].as_str().unwrap_or("").to_string();
        let as_kind = sec["sectionType"].as_str().and_then(|t| t.strip_suffix("_LIST")).map(str::to_string);
        let items: Vec<Value> = sec["items"].as_array().cloned().unwrap_or_default().into_iter()
            .map(|it| match &as_kind { Some(k) if it.get("_itemType").is_none() => json!({"type": k, "data": it}), _ => it })
            .collect();
        let cards = cards_for(&format!("es-{si}"), &items);
        let mut block = col().w_full().gap(10.0);
        if !st.is_empty() && st != title {
            block = block.child(heading(&st, 18.0));
        }
        if !cards.is_empty() {
            block = block.child(grid(cards));
        } else if items.iter().any(|it| data::kind(it) == "TRACK") {
            let mut rows = col().w_full().gap(2.0);
            for (i, tr) in items.iter().enumerate() {
                let id = data::data(tr)["id"].as_u64().unwrap_or(0);
                rows = rows.child(track_row_with(format!("es-{si}-t{i}"), i + 1, data::data(tr), false, None, move |_: &mut App| controller::send_play_ids(vec![id])));
            }
            block = block.child(rows);
        } else {
            let mut pills = row().w_full().gap(8.0).flex_wrap();
            for (i, it) in items.iter().enumerate() {
                pills = pills.child(pill_link(format!("es-{si}-{i}"), it));
            }
            block = block.child(pills);
        }
        c = c.child(block);
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
    let Some(v) = data::ready(Key::Feed) else { return loading() };
    let today = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86400) as i64;
    let group = |date: &str| match days(date).map(|d| today - d) {
        Some(0) => "Today",
        Some(1..=6) => "This week",
        _ => "Older",
    };
    let mut c = col().w_full().gap(14.0).child(heading("Feed", 28.0));
    let mut last = "";
    for (i, it) in v["items"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let g = group(it["occurredAt"].as_str().unwrap_or(""));
        if g != last {
            c = c.child(heading(g, 14.0));
            last = g;
        }
        let item = &it["item"];
        let title = item["title"].as_str().unwrap_or("").to_string();
        let kind = it["kind"].as_str().unwrap_or("").to_string();
        let sub = data::artist_of(item);
        let id = item["id"].clone();
        let (t2, k2) = (title.clone(), kind.clone());
        c = c.child(row().key(format!("feed-{i}")).w_full().h(64.0).px(10.0).gap(12.0).items_center().radius(10.0).hover(white(0.05))
            .child(ui::with_image(ui::tile(ui::hue(Some(i as u64 + 60)), 48.0, i as u64 + 60, 8.0), crate::art::item_url(item, 48.0), 48.0, 8.0))
            .child(col().grow(1.0).min_w(0.0).child(text(title.clone()).font(t(14.0, 600.0)).color(white(0.95)).truncate()).child(text(sub).font(t(12.0, 400.0)).color(white(0.55)).truncate()))
            .child(icon::<App>(&format!("feed-{i}-play"), &format!("Play {title}"), G::Play).on_press(move |_: &mut App| play_feed_item(&k2, id.clone(), &t2)))
            .child({
                let (mk, menu_item) = (format!("feed-{i}-more"), json!({"type": kind.to_uppercase(), "data": item.clone()}));
                icon::<App>(&mk, &format!("More options for {title}"), G::More)
                    .on_press(move |s: &mut App| crate::interact::open_media(s, menu_item.clone(), crate::interact::Anchor::Below(mk.clone())))
            }));
    }
    c
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
                if let Ok(p) = data::get(Key::Playlist(uuid.clone())).await {
                    let tracks = p["tracks"]["items"].as_array().cloned().unwrap_or_default();
                    controller::send(Cmd::PlayCollection { tracks, source: json!({"type": "playlist", "id": uuid, "name": title}), album_mode: false, shuffle: false });
                }
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
    let mut c = col().key("login").w(420.0).gap(14.0).items_center()
        .child(row().items_end().child(text(&crate::TITLE[..1]).font(t(56.0, 700.0)).color(white(0.95)).a11y_hidden()).child(heading(&crate::TITLE[1..], 56.0)));
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
                c = c.child(text(e.clone()).font(t(12.0, 500.0)).color(oklch(0.75, 0.15, 25.0)).wrap());
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
        state => {
            let mode = s.nav.login_mode;
            let mtab = |key: &str, label: &str, which: LoginMode| {
                let on = mode == which;
                row().key(key.to_string()).role("button").label(label).h(32.0).px(14.0).radius(16.0).items_center()
                    .fill(if on { white(0.16) } else { white(0.05) }).cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut App| s.nav.login_mode = which)
                    .child(text(label.to_string()).font(t(12.0, 600.0)).color(white(0.9)).a11y_hidden())
            };
            c = c.child(row().gap(8.0).child(mtab("lm-pkce", "PKCE", LoginMode::Pkce)).child(mtab("lm-code", "Login Code", LoginMode::Code)));
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
                c = c.child(text(e.clone()).font(t(12.0, 500.0)).color(oklch(0.75, 0.15, 25.0)).wrap());
            }
            c = c.child(row().key("login-custom").role("button").label("Use custom credentials").cursor(rune::Cursor::Pointer)
                .child(text("Use custom credentials").font(t(12.0, 500.0)).color(white(0.55)).a11y_hidden()));
        }
    }
    col().key("login-root").w_full().h_full().fill(ui::base(270.0))
        .child(row().w_full().px(12.0).py(8.0).child(spacer()).child(window_buttons()))
        .child(col().grow(1.0).w_full().items_center().justify_center().child(c))
}


/// Suggested tracks: the current track's radio (React SuggestedTab: getMixItems(TRACK_MIX)).
/// Loved tracks (React FavoritesView): Play and Shuffle, the filter, and the tracks, newest
/// first (its first page of 100). Unloving one there takes it out of the list.
fn loved_page(s: &App, v: &View) -> El {
    let Some(p) = data::ready(Key::LovedTracks) else { return loading() };
    let tracks: Vec<Value> = p["items"].as_array().cloned().unwrap_or_default().into_iter()
        .filter(|t| crate::interact::is_fav(crate::interact::Fav::Track, &data::data(t)["id"].as_u64().unwrap_or(0).to_string()) || !crate::interact::favorites_loaded())
        .collect();
    let total = p["totalNumberOfItems"].as_u64().unwrap_or(tracks.len() as u64);
    let source = json!({"type": "favorites", "id": "favorites", "name": "Loved Tracks"});
    let play = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: false, shuffle: false };
    let shuf = Cmd::PlayCollection { tracks: tracks.clone(), source: source.clone(), album_mode: false, shuffle: true };
    let filter = text_input(&s.nav.filter, t(13.0, 500.0).lh(18.0))
        .key("loved-filter").label("Filter on title, artist or album").single_line().w(320.0)
        .color(white(0.95)).caret(white(0.95), white(0.25)).placeholder("Filter on title, artist or album", white(0.4))
        .on_edit(|s: &mut App| &mut s.nav.filter);
    let needle = s.nav.filter.text.to_lowercase();
    let mut rows = col().w_full().gap(2.0);
    for (i, tr) in tracks.iter().enumerate() {
        if !needle.is_empty() && !format!("{} {} {}", data::name(tr), data::artist_of(tr), data::data(tr)["album"]["title"].as_str().unwrap_or("")).to_lowercase().contains(&needle) {
            continue;
        }
        let (all, src, idx) = (tracks.clone(), source.clone(), i);
        rows = rows.child(track_row(format!("lv-t{i}"), i + 1, tr, move |_: &mut App| {
            let ordered = all[idx..].iter().chain(all[..idx].iter()).cloned().collect();
            controller::send(Cmd::PlayCollection { tracks: ordered, source: src.clone(), album_mode: false, shuffle: false });
        }));
    }
    let art = loved_art(180.0, 14.0, 64.0);
    let head = row().w_full().gap(24.0).items_end().child(art)
        .child(col().gap(10.0).min_w(0.0).child(heading("Loved Tracks", 30.0))
            .child(text(format!("{total} tracks")).font(t(13.0, 500.0)).color(white(0.6)))
            .child(play_buttons(v, "Loved Tracks", play, shuf)));
    col().w_full().gap(16.0)
        .child(head)
        .child(row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).child(filter))
        .child(rows)
}

/// A mix or radio (React MixPage): its tracks, with Play, Shuffle, Add, Share and More.
fn mix_page(id: &str, title: &str, sub: &str, v: &View) -> El {
    let Some(m) = data::ready(Key::Mix(id.to_string())) else { return loading() };
    let tracks: Vec<Value> = m["tracks"].as_array().or(m["items"].as_array()).cloned().unwrap_or_default();
    let source = json!({"type": "mix", "id": id, "name": title});
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
    let mut rows = col().w_full().gap(2.0);
    for (i, tr) in tracks.iter().enumerate() {
        let (all, src, idx) = (tracks.clone(), source.clone(), i);
        rows = rows.child(track_row(format!("mx-t{i}"), i + 1, tr, move |_: &mut App| {
            let ordered = all[idx..].iter().chain(all[..idx].iter()).cloned().collect();
            controller::send(Cmd::PlayCollection { tracks: ordered, source: src.clone(), album_mode: false, shuffle: false });
        }));
    }
    let seed = id.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    col().w_full().gap(16.0)
        .child(hero(title, true, sub, seed, false, tracks.first().and_then(|t| crate::art::item_url(t, 180.0)), actions))
        .child(rows)
}

fn suggested(v: &View) -> El {
    let Some(mix) = &v.track_mix else { return text("No suggestions for this track.").font(t(13.0, 500.0)).color(white(0.5)) };
    let Some(m) = data::ready(Key::Mix(mix.clone())) else { return loading() };
    let mut c = col().w_full().gap(2.0);
    for (i, tr) in m["tracks"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let id = tr["id"].as_u64().unwrap_or(0);
        // NowPlayingDrawer: a suggested row's click adds it to the queue
        let qt = data::name(tr);
        let (on, l) = (crate::interact::is_fav(crate::interact::Fav::Track, &id.to_string()), qt.clone());
        c = c.child(row().w_full().items_center()
            .child(col().grow(1.0).min_w(0.0).child(track_row_with(format!("sg-{i}"), i + 1, tr, false, None, move |_: &mut App| {
                controller::send(Cmd::QueueIds(vec![id]));
                crate::interact::toast(format!("Added \"{qt}\" to queue"), crate::interact::Kind::Success);
            })))
            .child(ui::icon_button::<App>(&format!("sg-{i}-love"), if on { "Remove from Loved tracks" } else { "Add to Loved tracks" }, if on { G::HeartFilled } else { G::Heart }, 15.0, if on { accent() } else { white(0.75) })
                .on_press(move |_: &mut App| crate::interact::toggle_fav(crate::interact::Fav::Track, id.to_string(), l.clone(), crate::interact::Say::LovedShort)))
            .child(icon::<App>(&format!("sg-{i}-queue"), "Add to queue", G::ListPlus).on_press(move |_: &mut App| controller::send(Cmd::QueueIds(vec![id])))));
    }
    c
}

/// Credits: the track's contributors by role, then the main artist's bio (React CreditsTab).
fn credits(v: &View) -> El {
    let Some(cur) = &v.current else { return text("Nothing playing").font(t(13.0, 500.0)).color(white(0.5)) };
    let mut c = col().w_full().gap(10.0);
    match data::ready(Key::Credits(cur.id)) {
        Some(cr) => {
            for credit in cr.as_array().cloned().unwrap_or_default() {
                let role = credit["type"].as_str().or(credit["creditType"].as_str()).unwrap_or("").to_string();
                let names: Vec<String> = credit["contributors"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x["name"].as_str().map(str::to_string)).collect();
                c = c.child(col().gap(2.0).child(caps(&role, 0.5)).child(text(names.join(", ")).font(t(13.0, 500.0)).color(white(0.9))));
            }
        }
        None => c = c.child(loading()),
    }
    if let Some(aid) = v.artist_id {
        c = c.child(heading("Bio", 16.0));
        if let Some(b) = data::ready(Key::ArtistBio(aid)) {
            let bio = b["text"].as_str().or(b.as_str()).unwrap_or("");
            c = c.child(crate::bio::view(bio, 13.0, 0.7));
        }
    }
    c
}
