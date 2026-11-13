//! Home (React Home.tsx + HomeSection.tsx): TIDAL's feed tabs ("For you", "Staff Picks", …),
//! the quick-access grid, and the feed's sections, loading the next page of sections as the
//! end of the page comes into view (React's infinite-scroll sentinel).
//!
//! Section layouts follow React's:
//! - `SHORTCUT_LIST`: the quick-access grid, Loved Tracks first, then up to seven shortcuts,
//!   three columns at 1024 pt and up, two from 640.
//! - `COMPACT_GRID_CARD` (and "Recently played"): compact rows, three per column, filled down
//!   then across, as many 288 pt columns as fit, up to three (useGridColumns).
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

pub fn home(s: &App) -> El {
    let slug = (*s.nav.home_tab).clone();
    let first = match views::ready_or(Key::Home(slug.clone())) { Ok(v) => v, Err(e) => return e };
    let home = &first["home"];
    let w = *s.nav.content_w;
    // Rows and grids are sized to the measured width; until the first layout measures it, drawing
    // them at a guess made every card jump to its real size on launch.
    if w <= 0.0 {
        return col().w_full();
    }
    let mut c = col().w_full().gap(32.0);

    let tabs = home["tabs"].as_array().cloned().unwrap_or_default();
    if !tabs.is_empty() {
        let mut bar = row().gap(8.0).flex_wrap().role("tablist").focus_group();
        for tab in &tabs {
            let name = tab["name"].as_str().unwrap_or("").to_string();
            let tslug = tab["tabType"].as_str().unwrap_or("").to_lowercase();
            let on = tslug == slug;
            bar = bar.child(theme::focusable(row().key(format!("home-tab-{tslug}")).role("tab").label(name.clone())).focus_selected(on).h(36.0).px(16.0).radius(18.0).items_center()
                .fill(if on { white(0.95) } else { white(0.08) }).hover(if on { white(0.95) } else { white(0.14) }).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    if *s.nav.home_tab != tslug {
                        *s.nav.home_tab = tslug.clone();
                        *s.nav.home_pages = 0;
                    }
                })
                .child(text(name).font(t(14.0, 700.0)).color(if on { theme::ink() } else { white(0.92) }).a11y_hidden()));
        }
        c = c.child(bar);
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
            .on_visible(move |s: &mut App, on, _| if on && *s.nav.home_pages == n { *s.nav.home_pages += 1 }));
    }
    c
}

fn section(si: usize, sec: &Value, w: f32) -> Option<El> {
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
    if kind == "COMPACT_GRID_CARD" || title == "Recently played" {
        return Some(col().w_full().gap(14.0).child(header(si, &title, view_all, false)).child(compact_grid(si, &title, &items, w)));
    }
    let cards = card_row(si, &title, &items, w)?;
    Some(col().w_full().gap(14.0).child(header(si, &title, view_all, true)).child(cards))
}

fn header(si: usize, title: &str, view_all: Option<String>, arrows: bool) -> El {
    let mut r = row().w_full().items_center().gap(8.0);
    if !title.is_empty() {
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
            .on_press(move |s: &mut App| go(s, Page::ExploreSub(path.clone(), t2.clone())))
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
    let tile = |key: String, label: String, art: El| {
        theme::focusable_row(row().key(key).role("presentation")).w(tw).h(56.0).radius(4.0).clip_rounded(4.0).items_center().gap(12.0)
            .fill(white(0.05)).hover(white(0.1)).cursor(rune::Cursor::Pointer)
            .child(art)
            .child(text(label).font(t(13.0, 700.0)).color(white(0.95)).truncate().shrink().min_w(0.0))
    };
    let mut grid = row().w_full().gap(gap).flex_wrap()
        .child(tile("home-sc-loved".into(), "Loved Tracks".into(), views::loved_art(56.0, 0.0, 22.0)).on_press(|s: &mut App| go(s, Page::Loved)));
    for (i, it) in items.iter().filter(|it| data::kind(it) != "TASK").take(7).enumerate() {
        let it2 = it.clone();
        let mut t = tile(format!("home-sc-{i}"), data::name(it), thumb(it, 56.0, 0.0)).on_press(move |s: &mut App| open_item(s, &it2));
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

/// Compact rows, three to a column, filled down then across (useGridColumns + fitRows).
fn compact_grid(si: usize, title: &str, items: &[Value], w: f32) -> El {
    const ROWS: usize = 3;
    const MIN_W: f32 = 288.0;
    const GAP: f32 = 24.0;
    let cols = (((w + GAP) / (MIN_W + GAP)).floor() as usize).clamp(1, 3);
    let cw = ((w - (cols as f32 - 1.0) * GAP) / cols as f32).floor();
    let tracks = section_tracks(items);
    let mut grid = row().w_full().gap(GAP).items_start();
    for c in 0..cols {
        let mut column = col().w(cw).gap(4.0);
        for (r, it) in items.iter().enumerate().skip(c * ROWS).take(ROWS) {
            let d = data::data(it);
            let kind = data::kind(it);
            let sub = match kind.as_str() {
                "ARTIST" => "Artist".to_string(),
                _ if is_my_tracks(it) => "Collection".to_string(),
                _ => data::artist_of(it),
            };
            let name = data::name(it);
            let mut line = theme::focusable_row(row().key(format!("home-{si}-c{r}")).role("button").label(name.clone())).w_full().h(56.0).px(8.0).gap(12.0).radius(6.0).items_center()
                .hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .child(thumb(it, 40.0, 4.0))
                .child(col().shrink().min_w(0.0).gap(2.0)
                    .child(text(name.clone()).font(t(14.0, 500.0)).color(white(0.95)).truncate().a11y_hidden())
                    .child(text(sub).font(t(12.0, 400.0)).color(white(0.55)).truncate().a11y_hidden()));
            if kind == "TRACK" {
                let (id, tr, ttl) = (d["id"].as_u64().unwrap_or(0), tracks.clone(), title.to_string());
                let track = d.clone();
                line = line.on_press(move |_: &mut App| play_from(&ttl, &tr, id))
                    .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, track.clone(), None, Anchor::Point(p.x, p.y)));
            } else {
                let it2 = it.clone();
                line = line.on_press(move |s: &mut App| open_item(s, &it2));
                if interact::Media::of(it).is_some() {
                    let it3 = it.clone();
                    line = line.on_secondary_press(move |s: &mut App, p, _| interact::open_media(s, it3.clone(), Anchor::Point(p.x, p.y)));
                }
            }
            column = column.child(line);
        }
        grid = grid.child(column);
    }
    grid
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
    let videos: Vec<Value> = items.iter().filter(|it| data::kind(it) == "VIDEO").cloned().collect();
    let mut n = 0;
    for (i, it) in items.iter().enumerate() {
        let ck = format!("home-{si}-{i}");
        let kind = data::kind(it);
        let card = if kind == "MAGAZINE" {
            promo_card(ck, it, cw)
        } else if kind == "TRACK" {
            Some(track_card(ck, it, cw, title, &tracks))
        } else if kind == "VIDEO" {
            let idx = videos.iter().position(|v| v == it).unwrap_or(0);
            let list: Vec<Value> = videos.iter().map(|v| data::data(v).clone()).collect();
            Some(crate::video_ui::video_card(ck, data::data(it), &list, idx, json!({"type": "home-section", "id": title, "name": title})))
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
    theme::focusable_row(col().key(key).role("presentation")).w(cw).padding(8.0).gap(6.0).radius(12.0).hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .on_press(move |_: &mut App| play_from(&ttl, &tr, id))
        .on_secondary_press(move |s: &mut App, p, _| interact::open_track(s, d.clone(), None, Anchor::Point(p.x, p.y)))
        .child(ui::with_image(ui::tile(ui::hue(Some(id)), a, id, 10.0), crate::art::item_url(it, a), a, 10.0))
        .child(text(data::name(it)).font(t(13.0, 600.0)).color(white(0.9)).truncate())
        .child(text(data::artist_of(it)).font(t(11.0, 400.0)).color(white(0.55)).truncate())
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
    let header = d["header"].as_str().unwrap_or("").to_string();
    Some(theme::focusable_row(col().key(key).role("presentation")).w(cw).padding(8.0).gap(6.0).radius(12.0).hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| open(s))
        .child(art)
        .child(text(header).font(t(10.0, 700.0).tracking(0.6)).color(white(0.5)).truncate())
        .child(views::heading(&title, 13.0).truncate())
        .child(text(sub).font(t(11.0, 400.0)).color(white(0.55)).truncate()))
}
