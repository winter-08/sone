//! Home (React Home.tsx + HomeSection.tsx): TIDAL's feed tabs ("For you", "Staff Picks", …),
//! the quick-access grid, and the feed's sections, loading the next page of sections as the
//! end of the page comes into view (React's infinite-scroll sentinel).
//!
//! Section layouts follow React's:
//! - `SHORTCUT_LIST`: the quick-access grid, Loved Tracks first, then up to seven shortcuts,
//!   three columns at 1024 pt and up, two from 640.
//! - `COMPACT_GRID_CARD` (and "Recently played"): compact rows, three per column, filled down
//!   then across, as many 288 pt columns as fit, up to three (useGridColumns); a row opens its
//!   item (a track, its album) and its cover's play button plays it.
//! - `TRACK_LIST` (unless its items are of mixed kinds): the same grid of track rows, which play
//!   the section from them; the title opens the album, the artists their pages.
//! - everything else: one row of cards that scrolls sideways, with previous/next arrows and
//!   "View all" (CardScrollSection), the cards sized so a whole number fit the width.
//!   `MAGAZINE` items (promotions) open what they point at, or the article in the browser.

use crate::controller::{self, Cmd};
use crate::data::{self, Key};
use crate::glyph::{G, glyph};
use crate::interact::{self, Anchor};
use crate::theme;
use crate::ui::{self, t, white};
use crate::views::{self, Page, go};
use crate::App;
use rune::{El, Tx, col, el, row, spacer, text};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const CARD_GAP: f32 = 16.0;
const MIN_CARD: f32 = 170.0;

thread_local! {
    /// Each card row's scroll position, for its arrows (set as it scrolls, without a rebuild).
    static ROWS: RefCell<HashMap<String, rune::ScrollPos>> = RefCell::new(HashMap::new());
    /// Bumped when a row's arrows change (it can now scroll a way it couldn't, or no longer
    /// can), so `main::poll` rebuilds the view; scrolling between the ends rebuilds nothing.
    static ARROWS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Which ways a row scrolled to `p` can still go: (back, forward).
fn ways(p: &rune::ScrollPos) -> (bool, bool) {
    (p.offset[0] > 0.5, p.offset[0] < p.max[0] - 0.5)
}

/// What [`ways`] says for the card row keyed `key`: neither, until it has been laid out.
fn can_scroll(key: &str) -> (bool, bool) {
    ROWS.with(|m| m.borrow().get(key).map(ways).unwrap_or((false, false)))
}

/// Changes whenever some card row's arrows should change.
pub fn version() -> u64 {
    ARROWS.with(|a| a.get())
}

/// The feed's tabs as the last Home page that listed them had them (React's `cachedTabs`): a tab
/// still loading, or one that failed, keeps the tab bar, so there's always a way back.
static TABS: std::sync::Mutex<Vec<Value>> = std::sync::Mutex::new(Vec::new());

/// How many further pages of each tab were shown, so a tab switched back to shows as much as it
/// did (React keeps each tab's sections, with the pages it loaded, in its tab cache).
static PAGES: std::sync::LazyLock<std::sync::Mutex<HashMap<String, usize>>> = std::sync::LazyLock::new(Default::default);

/// Switch Home to the tab `slug`, keeping the page count of the one it leaves.
fn switch_tab(s: &mut App, slug: &str) {
    if *s.nav.home_tab == slug {
        return;
    }
    let mut pages = PAGES.lock().unwrap();
    pages.insert((*s.nav.home_tab).clone(), *s.nav.home_pages);
    let n = pages.get(slug).copied().unwrap_or(0);
    drop(pages);
    *s.nav.home_tab = slug.to_string();
    if *s.nav.home_pages != n {
        *s.nav.home_pages = n;
    }
}

fn tab_bar(tabs: &[Value], slug: &str) -> El {
    let mut bar = row().gap(8.0).flex_wrap().role("tablist").focus_group();
    for tab in tabs {
        let name = tab["name"].as_str().unwrap_or("").to_string();
        let tslug = tab["tabType"].as_str().unwrap_or("").to_lowercase();
        let on = tslug == slug;
        bar = bar.child(theme::focusable(row().key(format!("home-tab-{tslug}")).role("tab").label(name.clone())).focus_selected(on).h(36.0).px(16.0).radius(18.0).items_center()
            .fill(if on { white(0.95) } else { white(0.08) }).hover(if on { white(0.95) } else { white(0.14) }).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| switch_tab(s, &tslug))
            .child(text(name).font(t(14.0, 700.0)).color(if on { theme::ink() } else { white(0.92) }).a11y_hidden()));
    }
    bar
}

pub fn home(s: &App) -> El {
    let slug = (*s.nav.home_tab).clone();
    let load = data::want(Key::Home(slug.clone()));
    if let data::Load::Ready(first) = &load {
        let tabs = first["home"]["tabs"].as_array().cloned().unwrap_or_default();
        if !tabs.is_empty() {
            *TABS.lock().unwrap() = tabs.clone();
            // Home.tsx: once the first tab is in, the others load behind it.
            for t in &tabs {
                let other = t["tabType"].as_str().unwrap_or("").to_lowercase();
                if !other.is_empty() && other != slug {
                    data::want(Key::Home(other));
                }
            }
        }
        refresh_if_stale(&slug, first);
    }
    let tabs = TABS.lock().unwrap().clone();
    let first = match load {
        data::Load::Ready(v) => v,
        // A tab still loading or that failed keeps the bar above it.
        data::Load::Loading if !tabs.is_empty() => return col().w_full().gap(32.0).child(tab_bar(&tabs, &slug)).child(views::loading()),
        data::Load::Loading => return views::loading(),
        data::Load::Failed(e) => return failed(&tabs, &slug, &e),
    };
    let home = &first["home"];
    let w = *s.nav.content_w;
    // Rows and grids are sized to the measured width; until the first layout measures it, drawing
    // them at a guess made every card jump to its real size on launch.
    if w <= 0.0 {
        return col().w_full();
    }
    let mut c = col().w_full().gap(32.0);
    if !tabs.is_empty() {
        c = c.child(tab_bar(&tabs, &slug));
    }

    // The first page's sections, then each further page asked for so far.
    let mut sections: Vec<Value> = home["sections"].as_array().cloned().unwrap_or_default();
    let mut cursor = home["cursor"].as_str().filter(|c| !c.is_empty()).map(str::to_string);
    let mut waiting = false;
    for _ in 0..*s.nav.home_pages {
        let Some(cur) = cursor.clone() else { break };
        match data::want(Key::HomeMore(slug.clone(), cur)) {
            data::Load::Ready(p) => {
                sections.extend(p["sections"].as_array().cloned().unwrap_or_default());
                cursor = p["cursor"].as_str().filter(|c| !c.is_empty()).map(str::to_string);
            }
            data::Load::Loading => {
                waiting = true;
                break;
            }
            // A page that failed ends the feed there (React stops paginating on an error).
            data::Load::Failed(_) => {
                cursor = None;
                break;
            }
        }
    }
    if sections.is_empty() {
        return c.child(ui::empty("Nothing to show here yet"));
    }
    for (si, sec) in sections.iter().enumerate() {
        if let Some(e) = section(si, sec, w) {
            c = c.child(e);
        }
    }
    if waiting {
        c = c.child(views::loading());
    } else if cursor.is_some() {
        // The sentinel: reaching the end asks for the next page.
        let n = *s.nav.home_pages;
        c = c.child(el().key(format!("home-more-{n}")).w_full().h(2.0).a11y_hidden()
            .on_visible(move |s: &mut App, on, _| if on && *s.nav.home_pages == n {
                *s.nav.home_pages += 1;
                PAGED.lock().unwrap().insert((*s.nav.home_tab).clone());
            }));
    }
    c
}

/// A tab that couldn't load (Home.tsx): the tab bar, "Couldn't load Home", why, and Retry.
/// The tabs that have shown a further page this session (React's `hasPaginated`): their first
/// page stays as it is, since the pages after it follow on from its cursor.
static PAGED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::LazyLock::new(Default::default);

/// The tabs whose stale first page has been refreshed (once a session each).
static REFRESHED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::LazyLock::new(Default::default);

/// Home.tsx: a first page the backend served stale from its cache shows at once, and the tab is
/// fetched fresh behind it (refresh_home_page); the fresh page takes its place unless the tab
/// has paged on meanwhile.
fn refresh_if_stale(slug: &str, first: &Value) {
    if first["isStale"].as_bool() != Some(true) || PAGED.lock().unwrap().contains(slug) || !REFRESHED.lock().unwrap().insert(slug.to_string()) {
        return;
    }
    let (slug, old_tabs) = (slug.to_string(), first["home"]["tabs"].clone());
    sone_host_shim::async_runtime::spawn(async move {
        use sone_host_shim::Manager;
        let h = crate::host().clone();
        match tauri_app_lib::commands::pages::refresh_home_page(h.state::<tauri_app_lib::AppState>(), Some(slug.clone())).await {
            Ok(fresh) => {
                let mut home = serde_json::to_value(fresh).unwrap_or(Value::Null);
                // (the tabs come with the cached page's header; keep them if this has none)
                if home["tabs"].as_array().is_none_or(|t| t.is_empty()) {
                    home["tabs"] = old_tabs;
                }
                if !PAGED.lock().unwrap().contains(&slug) {
                    data::replace(Key::Home(slug), json!({"home": home, "isStale": false}));
                }
            }
            Err(e) => log::warn!("[rune] home refresh failed: {e}"),
        }
    });
}

/// The window came back on screen: Home.tsx revalidates the tab showing when it was loaded more
/// than five minutes ago and hasn't paged on (its visibilitychange / focus handler). The page
/// shown stays until the new one lands.
pub fn revalidate(s: &App) {
    let slug = (*s.nav.home_tab).clone();
    let key = Key::Home(slug.clone());
    if *s.nav.page != Page::Home || PAGED.lock().unwrap().contains(&slug) || !data::age(&key).is_some_and(|a| a.as_secs() >= 5 * 60) {
        return;
    }
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if BUSY.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    sone_host_shim::async_runtime::spawn(async move {
        use sone_host_shim::Manager;
        let h = crate::host().clone();
        let got = tauri_app_lib::commands::pages::get_home_page(h.state::<tauri_app_lib::AppState>(), h.clone(), Some(slug.clone())).await;
        if let Ok(v) = got.map_err(|e| e.to_string()).and_then(|x| serde_json::to_value(x).map_err(|e| e.to_string()))
            && !PAGED.lock().unwrap().contains(&slug)
        {
            // a stale answer gets its refresh again
            REFRESHED.lock().unwrap().remove(&slug);
            data::replace(key, v);
        }
        BUSY.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}

fn failed(tabs: &[Value], slug: &str, why: &str) -> El {
    let mut c = col().w_full().gap(32.0);
    if !tabs.is_empty() {
        c = c.child(tab_bar(tabs, slug));
    }
    let key = Key::Home(slug.to_string());
    c.child(col().w_full().gap(6.0).py(60.0).items_center()
        .child(text("Couldn't load Home").font(t(14.0, 600.0)).color(white(0.92)))
        .child(text(why.to_string()).font(t(12.0, 400.0)).color(white(0.45)).wrap())
        .child(el().h(10.0))
        .child(views::button::<App>("home-retry", "Retry", None, false).on_press(move |_: &mut App| data::forget(&key))))
}


pub(crate) fn section(si: usize, sec: &Value, w: f32) -> Option<El> {
    let items = sec["items"].as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        return None;
    }
    let title = sec["title"].as_str().unwrap_or("").to_string();
    let kind = sec["sectionType"].as_str().unwrap_or("");
    if kind == "SHORTCUT_LIST" {
        return Some(shortcuts(&items, w));
    }
    let view_all = sec["apiPath"].as_str().filter(|p| !p.is_empty() && sec["hasMore"].as_bool() != Some(false)).map(str::to_string);
    let compact = kind == "COMPACT_GRID_CARD" || title == "Recently played";
    // HomeSection: the backend types a row from its first item, so a row of mixed kinds can come
    // as TRACK_LIST; it shows as cards then (a track list would hand albums to the player).
    let mixed = items.iter().map(data::kind).filter(|k| !k.is_empty()).collect::<std::collections::HashSet<_>>().len() > 1;
    if compact || (kind == "TRACK_LIST" && !mixed) {
        let grid = if compact { Grid::Compact } else { Grid::Tracks };
        return Some(col().w_full().gap(14.0).child(header(si, &title, None, view_all, false)).child(row_grid(si, &title, &items, w, grid)));
    }
    let cards = card_row(si, &title, &items, w)?;
    Some(col().w_full().gap(14.0).child(header(si, &title, sec.get("header").filter(|h| h.is_object()), view_all, true)).child(cards))
}

fn header(si: usize, title: &str, ctx: Option<&Value>, view_all: Option<String>, arrows: bool) -> El {
    let mut r = row().w_full().items_center().gap(8.0);
    if let Some(it) = ctx {
        // A row based on an item ("Because you liked …"): its picture, the section's title small,
        // the item's name big; it opens the item (a track, its album).
        let it2 = it.clone();
        let name = data::name(it);
        let d = data::data(it);
        let target = if data::kind(it) == "TRACK" { d["album"]["id"].as_u64().map(Page::Album) } else { views::item_page(it) };
        let mut c = theme::focusable_row(row().key(format!("home-ctx-{si}")).role("button").label(format!("{title} {name}").as_str())).gap(12.0).items_center().min_w(0.0).shrink().cursor(rune::Cursor::Pointer)
            .child(thumb(&it2, 48.0, 4.0))
            .child(col().min_w(0.0).a11y_hidden()
                .child(text(title.to_string()).font(t(13.0, 400.0)).color(white(0.55)).truncate())
                .child(row().min_w(0.0).gap(8.0).items_center()
                    .child(text(name).font(t(theme::SECTION, 700.0)).color(white(0.95)).truncate().shrink())
                    .child(crate::content::badges(false, d["ai"].as_bool() == Some(true)).unwrap_or_else(el))));
        if let Some(p) = target {
            c = c.on_press(move |s: &mut App| go(s, p.clone()));
        }
        r = r.child(c);
    } else if !title.is_empty() {
        r = r.child(views::heading(title, theme::SECTION));
    }
    r = r.child(spacer());
    if arrows {
        let key = format!("home-row-{si}");
        // Shown only while the row overflows, and inert (dimmed) at the end it can't move
        // toward, as React's are. A row that fits keeps them in the accessibility tree,
        // undrawn and disabled, so the inventory doesn't change with the window's width.
        let (back, fwd) = can_scroll(&key);
        // React's arrows carry no accessible name (lucide chevrons in bare buttons).
        for (g, dir, can) in [(G::Back, -1.0f32, back), (G::Forward, 1.0, fwd)] {
            let k = key.clone();
            let b = el().key(format!("{key}-{}", if dir < 0.0 { "prev" } else { "next" })).role("button").size(32.0, 32.0).radius(16.0).items_center().justify_center();
            let ink = if can { white(0.9) } else if back || fwd { white(0.3) } else { white(0.0) };
            let b = if !can {
                b.disabled(true)
            } else {
                b.fill(white(0.08)).hover(white(0.14)).press_scale(0.92).cursor(rune::Cursor::Pointer)
                    .on_press_tx(move |_: &mut App, tx: &mut Tx| {
                        // A page at a time: the row's width plus a gap, so cards land aligned.
                        if let Some(p) = ROWS.with(|m| m.borrow().get(&k).copied()) {
                            let x = (p.offset[0] + dir * (p.viewport[0] + CARD_GAP)).clamp(0.0, p.max[0].max(0.0));
                            tx.scroll_to(k.clone(), x, 0.0);
                        }
                    })
            };
            r = r.child(b.child(glyph(g, 18.0, ink)));
        }
    }
    if let Some(path) = view_all {
        let t2 = title.to_string();
        r = r.child(theme::focusable(row().key(format!("home-viewall-{si}")).role("button").label("View all")).h(30.0).px(12.0).radius(15.0).items_center()
            .hover(white(0.06)).cursor(rune::Cursor::Pointer)
            // HomeSection: navigateToViewAll, the section's whole list in one grid.
            .on_press(move |s: &mut App| go(s, Page::ViewAll(t2.clone(), path.clone(), None)))
            .child(text("View all").font(t(13.0, 700.0)).color(white(0.6)).a11y_hidden()));
    }
    r
}

fn is_my_tracks(it: &Value) -> bool {
    data::kind(it) == "DEEP_LINK" && data::name(it) == "My Tracks"
}

fn open_item(s: &mut App, it: &Value) {
    if is_my_tracks(it) {
        go(s, Page::Loved);
    } else if let Some(p) = views::item_page(it) {
        go(s, p);
    }
}

fn thumb(it: &Value, size: f32, radius: f32) -> El {
    if is_my_tracks(it) {
        return views::loved_art(size, radius, size * 0.4);
    }
    let d = data::data(it);
    let seed = d["id"].as_u64().unwrap_or(7);
    let h = ui::hue(Some(seed));
    let url = crate::art::item_url(it, size);
    if data::kind(it) == "ARTIST" {
        ui::with_image(ui::art(h, size, seed, 0.0).radius(size / 2.0).clip_rounded(size / 2.0), url, size, size / 2.0)
    } else {
        ui::with_image(ui::tile(h, size, seed, radius), url, size, radius)
    }
}

/// The quick-access grid: Loved Tracks, then the feed's shortcuts (React: up to seven; the
/// "Choose your favorites" task isn't one).
fn shortcuts(items: &[Value], w: f32) -> El {
    let cols = if w >= 1024.0 { 3.0 } else if w >= 640.0 { 2.0 } else { 1.0 };
    let gap = 12.0;
    let tw = ((w - (cols - 1.0) * gap) / cols).floor();
    // (Home.tsx: an AI item's tile has the AI badge after its title)
    let tile = |key: String, label: String, art: El, ai: bool| {
        theme::focusable_row(row().key(key).role("presentation")).w(tw).h(56.0).radius(4.0).clip_rounded(4.0).items_center().gap(12.0)
            .fill(white(0.05)).hover(white(0.1)).cursor(rune::Cursor::Pointer)
            .child(art)
            .child(text(label).font(t(13.0, 700.0)).color(white(0.95)).truncate().shrink().min_w(0.0))
            .child(crate::content::badges(false, ai).unwrap_or_else(el))
    };
    let mut grid = row().w_full().gap(gap).flex_wrap()
        .child(tile("home-sc-loved".into(), "Loved Tracks".into(), views::loved_art(56.0, 0.0, 22.0), false).on_press(|s: &mut App| go(s, Page::Loved)));
    for (i, it) in items.iter().filter(|it| data::kind(it) != "TASK" && !is_my_tracks(it)).take(7).enumerate() {
        let it2 = it.clone();
        let ai = data::data(it)["ai"].as_bool() == Some(true);
        let mut t = tile(format!("home-sc-{i}"), data::name(it), thumb(it, 56.0, 0.0), ai).on_press(move |s: &mut App| open_item(s, &it2));
        if interact::Media::of(it).is_some() {
            let it3 = it.clone();
            t = t.on_secondary_press(move |s: &mut App, p, _| interact::open_media(s, it3.clone(), Anchor::Point(p.x, p.y)));
        }
        grid = grid.child(t);
    }
    grid
}

/// Tracks from a section as a playable list (the My Tracks shortcut isn't one).
fn section_tracks(items: &[Value]) -> Vec<Value> {
    items.iter().filter(|it| data::kind(it) == "TRACK").map(|it| data::data(it).clone()).collect()
}

fn play_from(title: &str, tracks: &[Value], id: u64) {
    let idx = tracks.iter().position(|t| t["id"].as_u64() == Some(id)).unwrap_or(0);
    controller::send(Cmd::PlayFrom { tracks: tracks.to_vec(), index: idx, source: json!({"type": "home-section", "id": title, "name": title}), album_mode: false });
}

/// Which of HomeSection's row grids a section is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Grid {
    /// TRACK_LIST (TrackListSection): a track per row; the row plays the section from it, its
    /// title opens the album.
    Tracks,
    /// COMPACT_GRID_CARD and "Recently played" (CompactGridSection): anything per row; the row
    /// opens it (a track, its album) and its cover's play button plays it.
    Compact,
}

/// Rows three to a column, filled down then across, as many 288 pt columns as fit up to three,
/// and only as many items as those hold (useGridColumns + fitRows).
fn row_grid(si: usize, title: &str, items: &[Value], w: f32, grid: Grid) -> El {
    const ROWS: usize = 3;
    const MIN_W: f32 = 288.0;
    const GAP: f32 = 24.0;
    let cols = (((w + GAP) / (MIN_W + GAP)).floor() as usize).clamp(1, 3);
    let cw = ((w - (cols as f32 - 1.0) * GAP) / cols as f32).floor();
    let shown = &items[..items.len().min(cols * ROWS)];
    // What a track plays among: TrackListSection plays the section's tracks (every one),
    // CompactGridSection the tracks it shows.
    let tracks = section_tracks(if grid == Grid::Tracks { items } else { shown });
    let mut out = row().w_full().gap(GAP).items_start();
    for c in 0..cols {
        let mut column = col().w(cw).gap(4.0);
        for (r, it) in shown.iter().enumerate().skip(c * ROWS).take(ROWS) {
            column = column.child(grid_row(format!("home-{si}-c{r}"), it, title, &tracks, grid));
        }
        out = out.child(column);
    }
    out
}

/// One row of a [`row_grid`]: the cover (with a play button on hover), the title and its
/// badges, the artists or what the item is, and More options.
///
/// HomeSection's rows are bare elements with an unnamed More button; here the row is the press
/// target for the pointer, and its parts are what assistive technology and the keyboard reach:
/// "Play <title>" on the cover, the title as a link (to the album for a track), each artist as
/// a link, and "More options".
fn grid_row(key: String, it: &Value, title: &str, tracks: &[Value], grid: Grid) -> El {
    let d = data::data(it).clone();
    let kind = data::kind(it);
    let my = is_my_tracks(it);
    let is_track = !my && (kind == "TRACK" || (grid == Grid::Tracks && kind.is_empty()));
    let name = if my { "Loved Tracks".to_string() } else { data::name(it) };
    let id = d["id"].as_u64().unwrap_or(0);
    let album = d["album"]["id"].as_u64().filter(|_| is_track);
    // A track the content filter blocks (or, with AI content off, anything AI) dims; a blocked
    // track in a track list does nothing, in a compact grid it still opens its album.
    let restricted = !my && if is_track { crate::content::blocked(&d) } else { !crate::content::prefs().1 && d["ai"].as_bool() == Some(true) };
    let media = (!is_track && !my).then(|| interact::Media::of(it)).flatten();
    let can_play = !my && !restricted && (is_track || media.is_some());
    let section = title.to_string();
    let play: Rc<dyn Fn()> = {
        let (tr, ttl, item) = (tracks.to_vec(), section.clone(), it.clone());
        Rc::new(move || match media {
            Some(m) => interact::play_media(&item, m),
            None => play_from(&ttl, &tr, id),
        })
    };

    // the cover, and on hover the play button over it
    let mut art = el().size(40.0, 40.0).child(thumb(it, 40.0, 4.0));
    if can_play {
        let p = play.clone();
        art = art.child(theme::focusable(el().key(format!("{key}-play")).role("button").label(format!("Play {name}"))).absolute(0.0, 0.0).size(40.0, 40.0).radius(4.0)
            .fill(theme::black(0.4)).items_center().justify_center().group_opacity(0.0, 1.0).cursor(rune::Cursor::Pointer)
            .on_press(move |_: &mut App| p())
            .child(glyph(G::Play, 14.0, white(1.0))));
    }

    // the title: a link to its album for a track, else to the item (the row's own action)
    let opens: Option<Page> = if my { Some(Page::Loved) } else if is_track { album.map(Page::Album) } else { views::item_page(it) };
    let title_text = text(name.clone()).font(t(14.0, 500.0)).color(white(0.95)).truncate();
    let title_el = match opens.clone() {
        Some(p) => theme::focusable(el().key(format!("{key}-title")).role("link").label(name.clone())).shrink().min_w(0.0).radius(3.0).hover_group().cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| go(s, p.clone()))
            .child(title_text.group_color(white(1.0)).a11y_hidden()),
        None => title_text.shrink(),
    };
    let mut words = row().min_w(0.0).gap(6.0).items_center().child(title_el);
    if let Some(b) = crate::content::badges(is_track && d["explicit"].as_bool() == Some(true), !my && d["ai"].as_bool() == Some(true)) {
        words = words.child(b);
    }

    // under it: the artists (links) for a track, else what the item is
    let artists: Vec<(String, Option<u64>)> = if is_track {
        let list = d["artists"].as_array().cloned().filter(|a| !a.is_empty()).unwrap_or_else(|| if d["artist"].is_object() { vec![d["artist"].clone()] } else { vec![] });
        list.iter().filter_map(|a| a["name"].as_str().map(|n| (n.to_string(), a["id"].as_u64()))).collect()
    } else {
        vec![]
    };
    let sub: El = if !artists.is_empty() {
        let mut line = row().min_w(0.0).items_center();
        let last = artists.len() - 1;
        for (i, (an, aid)) in artists.into_iter().enumerate() {
            let label = text(an.clone()).font(t(12.0, 400.0)).color(white(0.55)).truncate();
            line = line.child(match aid {
                Some(aid) => theme::focusable(el().key(format!("{key}-ar{i}")).role("link").label(an)).shrink().min_w(0.0).radius(3.0).hover_group().cursor(rune::Cursor::Pointer)
                    .on_press(move |s: &mut App| go(s, Page::Artist(aid)))
                    .child(label.group_color(white(0.9)).a11y_hidden()),
                None => label.shrink(),
            });
            if i < last {
                line = line.child(text(", ").font(t(12.0, 400.0)).color(white(0.55)));
            }
        }
        line
    } else {
        let what = match kind.as_str() {
            _ if my => "Collection".to_string(),
            "ARTIST" => "Artist".to_string(),
            _ => data::artist_of(it),
        };
        text(what).font(t(12.0, 400.0)).color(white(0.55)).truncate()
    };

    let mut line = row().key(key.clone()).role("presentation").w_full().h(56.0).px(8.0).gap(12.0).radius(6.0).items_center().hover_group()
        .child(art)
        .child(col().grow(1.0).shrink().min_w(0.0).gap(2.0).child(words).child(sub));
    if !my {
        let mk = format!("{key}-more");
        let (track, item) = (d.clone(), it.clone());
        let more = theme::focusable(el().key(mk.clone()).role("button").label("More options")).size(32.0, 32.0).radius(16.0).items_center().justify_center()
            .press(white(0.14)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| match media {
                Some(_) => interact::open_media(s, item.clone(), Anchor::Below(mk.clone())),
                None => interact::open_track(s, track.clone(), None, Anchor::Below(mk.clone())),
            })
            .child(glyph(G::More, 16.0, white(0.75)).group_opacity(0.3, 1.0));
        // (a track list's blocked row has no menu; a media item without a menu kind neither)
        if !(is_track && restricted && grid == Grid::Tracks) && (is_track || media.is_some()) {
            line = line.child(more);
        }
    }
    if restricted {
        line = line.opacity(0.5);
    }
    // the row: a track list plays from it, a compact grid opens it
    let press: Option<Box<dyn Fn(&mut App)>> = match grid {
        Grid::Tracks if my => Some(Box::new(|s: &mut App| go(s, Page::Loved))),
        Grid::Tracks if restricted => None,
        Grid::Tracks => { let p = play.clone(); Some(Box::new(move |_: &mut App| p())) }
        Grid::Compact if is_track && restricted && album.is_none() => None,
        Grid::Compact => {
            let it2 = it.clone();
            Some(Box::new(move |s: &mut App| match album {
                Some(a) => go(s, Page::Album(a)),
                None => open_item(s, &it2),
            }))
        }
    };
    if let Some(f) = press {
        line = line.hover(white(0.06)).cursor(rune::Cursor::Pointer).on_press(move |s: &mut App| f(s));
    }
    let menu_ok = !my && !(is_track && restricted);
    if menu_ok && is_track {
        let track = d.clone();
        line = line.on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, track.clone(), None, Anchor::Point(p.x, p.y)));
    } else if menu_ok && media.is_some() {
        let item = it.clone();
        line = line.on_secondary_press(move |s: &mut App, p, _| interact::open_media(s, item.clone(), Anchor::Point(p.x, p.y)));
    }
    line
}

/// One sideways-scrolling row of cards, as many whole cards as fit across the width.
fn card_row(si: usize, title: &str, items: &[Value], w: f32) -> Option<El> {
    let cols = ((w + CARD_GAP) / (MIN_CARD + CARD_GAP)).floor().clamp(2.0, 8.0);
    let cw = ((w - (cols - 1.0) * CARD_GAP) / cols).floor();
    let key = format!("home-row-{si}");
    let k = key.clone();
    let mut r = row().key(key.clone()).w_full().gap(CARD_GAP).items_start().scroll_x()
        .on_scroll_offset(move |p| ROWS.with(|m| {
            let was = m.borrow_mut().insert(k.clone(), p);
            if was.as_ref().map(ways) != Some(ways(&p)) {
                ARROWS.with(|a| a.set(a.get().wrapping_add(1)));
            }
        }));
    let tracks = section_tracks(items);
    let mut n = 0;
    for (i, it) in items.iter().enumerate() {
        let ck = format!("home-{si}-{i}");
        let kind = data::kind(it);
        let card = if kind == "MAGAZINE" {
            promo_card(ck, it, cw)
        } else if kind == "TRACK" {
            Some(track_card(ck, it, cw, title, &tracks))
        } else if kind == "VIDEO" {
            // HomeSection plays a video card alone (useMediaPlay: playSingle), not the row's videos
            let d = data::data(it).clone();
            Some(crate::video_ui::video_card(ck, &d, std::slice::from_ref(&d), 0, Value::Null))
        } else if is_my_tracks(it) {
            // My Tracks shows as Loved Tracks (HomeSection's myTracks card).
            let a = cw - 16.0;
            Some(theme::focusable_row(col().key(ck).role("presentation")).w(cw).padding(8.0).gap(6.0).radius(12.0).hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut App| go(s, Page::Loved))
                .child(views::loved_art(a, 10.0, 40.0))
                .child(views::heading("Loved Tracks", 13.0).truncate())
                .child(text("Collection").font(t(11.0, 400.0)).color(white(0.55))))
        } else {
            views::item_page(it).map(|page| {
                let seed = data::data(it)["id"].as_u64().unwrap_or(i as u64 + 1);
                views::card_w(cw, ck, &data::name(it), &data::artist_of(it), seed, kind == "ARTIST", crate::art::item_url(it, cw - 16.0), Some(it.clone()), page)
            })
        };
        if let Some(cd) = card {
            r = r.child(cd);
            n += 1;
        }
    }
    (n > 0).then_some(r)
}

/// A track as a card: its cover, title and artists; pressing it plays the section's tracks
/// from it (React HomeSection track cards, which have no accessible name of their own).
fn track_card(key: String, it: &Value, cw: f32, title: &str, tracks: &[Value]) -> El {
    let d = data::data(it).clone();
    let id = d["id"].as_u64().unwrap_or(0);
    let a = cw - 16.0;
    let (tr, ttl) = (tracks.to_vec(), title.to_string());
    // (a blocked track dims and neither plays nor opens its menu; its badges follow the title)
    let blocked = crate::content::blocked(&d);
    let mut words = row().w_full().min_w(0.0).gap(6.0).items_center().child(text(data::name(it)).font(t(13.0, 600.0)).color(white(0.9)).truncate().shrink());
    if let Some(b) = crate::content::badges_for(&d) {
        words = words.child(b);
    }
    let card = theme::focusable_row(col().key(key).role("presentation")).w(cw).padding(8.0).gap(6.0).radius(12.0)
        .child(ui::with_image(ui::tile(ui::hue(Some(id)), a, id, 10.0), crate::art::item_url(it, a), a, 10.0))
        .child(words)
        .child(text(data::artist_of(it)).font(t(11.0, 400.0)).color(white(0.55)).truncate());
    if blocked {
        return card.opacity(0.5);
    }
    card.hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .on_press(move |_: &mut App| play_from(&ttl, &tr, id))
        .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, d.clone(), None, Anchor::Point(p.x, p.y)))
}

/// A promotion (`MAGAZINE`): its 550×400 image, header and subheader; it opens the playlist,
/// album or artist it names, plays a video, or opens an article in the browser.
fn promo_card(key: String, it: &Value, cw: f32) -> Option<El> {
    let d = data::data(it);
    let target = d["artifactId"].as_str()?.to_string();
    let ty = d["type"].as_str().unwrap_or("").to_string();
    let title = d["shortHeader"].as_str().or(d["header"].as_str()).unwrap_or("").to_string();
    let sub = d["shortSubHeader"].as_str().unwrap_or("").to_string();
    let image_id = d["imageId"].as_str().unwrap_or("").to_string();
    let open: Box<dyn Fn(&mut App)> = match ty.as_str() {
        "PLAYLIST" => Box::new(move |s: &mut App| go(s, Page::Playlist(target.clone()))),
        "ALBUM" => { let id = target.parse().ok()?; Box::new(move |s: &mut App| go(s, Page::Album(id))) }
        "ARTIST" => { let id = target.parse().ok()?; Box::new(move |s: &mut App| go(s, Page::Artist(id))) }
        "EXTURL" => Box::new(move |_: &mut App| views::open_url(&target)),
        "VIDEO" => {
            let v = json!({"id": target.parse::<u64>().ok()?, "title": title, "imageId": image_id});
            // HomeSection → useMediaPlay: a video plays alone through playSingle (manual queue kept).
            Box::new(move |_: &mut App| controller::send(Cmd::PlaySingle(crate::video_ui::video_track(&v))))
        }
        _ => return None,
    };
    let a = cw - 16.0;
    let h = (a * 8.0 / 11.0).floor();
    let seed = title.len() as u64 + 3;
    let mut art = el().size(a, h).radius(10.0).clip_rounded(10.0).fill(rune::oklch(0.22, 0.07, ui::hue(Some(seed)))).a11y_hidden();
    let url = d["imageURL"].as_str().map(str::to_string).or_else(|| crate::art::cover(Some(&image_id), a));
    if let Some(px) = url.and_then(|u| crate::art::pixels(&u, a)) {
        art = art.child(el().absolute(0.0, 0.0).size(a, h).pixels_fit(&px, rune::SurfaceFit::Fill));
    }
    // The eyebrow (the long header, in the accent) only when the card's title is the short one;
    // otherwise the header is the title (MediaCard).
    let header = d["shortHeader"].as_str().and(d["header"].as_str()).unwrap_or("").to_string();
    Some(theme::focusable_row(col().key(key).role("presentation")).w(cw).padding(8.0).gap(6.0).radius(12.0).hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| open(s))
        .child(art)
        .child(if header.is_empty() { el() } else { text(header).font(t(10.0, 700.0).tracking(0.6)).color(theme::accent()).truncate() })
        .child(views::heading(&title, 13.0).truncate())
        .child(text(sub).font(t(11.0, 400.0)).color(white(0.55)).truncate()))
}

// ------------------------------------------------------------------ Explore's icon grid

/// The lucide icons ExplorePage gives its shortcuts (lucide-react 0.563.0, drawn as icons.rs's).
mod shortcut_icons {
    use rune::Icon;
    macro_rules! icon {
        ($name:ident, $id:literal, $($el:literal),+) => {
            pub static $name: Icon = Icon { name: $id, view: 24.0, body: concat!($($el, r#" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#),+) };
        };
    }
    icon!(CALENDAR, "lucide-calendar", r#"<path d="M8 2v4""#, r#"<path d="M16 2v4""#, r#"<rect width="18" height="18" x="3" y="4" rx="2""#, r#"<path d="M3 10h18""#);
    icon!(TROPHY, "lucide-trophy", r#"<path d="M10 14.66v1.626a2 2 0 0 1-.976 1.696A5 5 0 0 0 7 21.978""#, r#"<path d="M14 14.66v1.626a2 2 0 0 0 .976 1.696A5 5 0 0 1 17 21.978""#, r#"<path d="M18 9h1.5a1 1 0 0 0 0-5H18""#, r#"<path d="M4 22h16""#, r#"<path d="M6 9a6 6 0 0 0 12 0V3a1 1 0 0 0-1-1H7a1 1 0 0 0-1 1z""#, r#"<path d="M6 9H4.5a1 1 0 0 1 0-5H6""#);
    icon!(SQUARE_PLAY, "lucide-square-play", r#"<rect width="18" height="18" x="3" y="3" rx="2""#, r#"<path d="M9 9.003a1 1 0 0 1 1.517-.859l4.997 2.997a1 1 0 0 1 0 1.718l-4.997 2.997A1 1 0 0 1 9 14.996z""#);
    icon!(CHART_COLUMN, "lucide-chart-no-axes-column", r#"<path d="M18 20V10""#, r#"<path d="M12 20V4""#, r#"<path d="M6 20v-6""#);
    icon!(SHIELD_CHECK, "lucide-shield-check", r#"<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z""#, r#"<path d="m9 12 2 2 4-4""#);
    icon!(STAR, "lucide-star", r#"<path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z""#);
}

/// Explore's shortcuts (IconGrid): three columns (two under 768 pt), each link with its icon:
/// ExplorePage's own for New, Top, Videos, HiRes, Clean Content, Staff Picks and Creator Hub,
/// else the server's picture, else a note.
pub(crate) fn explore_icon_grid(si: usize, items: &[Value], w: f32) -> El {
    let cols = if w >= 768.0 { 3.0 } else { 2.0 };
    let gap = 48.0;
    let cw = ((w.max(200.0) - (cols - 1.0) * gap) / cols).floor();
    let mut grid = row().w_full().pt(24.0).gap(gap).flex_wrap();
    for (i, it) in items.iter().enumerate() {
        let title = it["title"].as_str().or(it["name"].as_str()).unwrap_or("").to_string();
        let path = it["apiPath"].as_str().unwrap_or("").to_string();
        let ink = white(0.6);
        let lucide = |i: &'static rune::Icon| rune::icon(i, 24.0).color(ink);
        let icon: El = match title.as_str() {
            "New" => lucide(&shortcut_icons::CALENDAR),
            "Top" => lucide(&shortcut_icons::TROPHY),
            "Videos" => lucide(&shortcut_icons::SQUARE_PLAY),
            "HiRes" => lucide(&shortcut_icons::CHART_COLUMN),
            "Clean Content" => lucide(&shortcut_icons::SHIELD_CHECK),
            "Staff Picks" => lucide(&shortcut_icons::STAR),
            "Creator Hub" => glyph(G::Users, 24.0, ink),
            _ => match it["icon"].as_str().filter(|u| !u.is_empty()).and_then(|u| crate::art::pixels(u, 24.0)) {
                Some(px) => el().size(24.0, 24.0).opacity(0.7).pixels_fit(&px, rune::SurfaceFit::Fill),
                None => glyph(G::Music, 24.0, ink),
            },
        };
        let t2 = title.clone();
        grid = grid.child(theme::focusable_row(row().key(format!("ex-{si}-{i}")).role("button").label(title.clone())).w(cw).h(32.0).gap(16.0).items_center().radius(6.0).hover_group().cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| if !path.is_empty() { go(s, Page::ExploreSub(path.clone(), t2.clone())) })
            .child(icon.a11y_hidden().group_color(white(0.95)))
            .child(text(title).font(t(16.0, 500.0)).color(white(0.6)).group_color(white(0.95)).truncate().a11y_hidden()));
    }
    grid
}
