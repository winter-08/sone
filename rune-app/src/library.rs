//! Your library: the sidebar's lists (React Sidebar), Show all and folder pages (React
//! LibraryViewAll), and playlist folders (useFolders, FolderContextMenu, MoveToFolderMenu):
//! open, rename, delete, a new playlist made inside one, and moving playlists in and out
//! (with a new folder made on the way).
//!
//! The lists page in 50 at a time as they scroll, sorted as the sort menu says (persisted, as
//! the React sort atoms are). After a folder change the lists are fetched again (the backend
//! drops its cached folder pages on every change) rather than patched optimistically as React's
//! atoms do; what shows is what the server has.

use rune::{El, Side, TextEdit, col, el, row, spacer, text, text_input};
use serde_json::{Value, json};
use sone_host_shim::Manager;
use tauri_app_lib::AppState;
use tauri_app_lib::commands::library;

use crate::App;
use crate::coll::{self, FolderDialog, FolderMenu, Sort, SortMenu};
use crate::data::{self, Key};
use crate::glyph::{G, glyph};
use crate::interact::{self, Anchor, Kind};
use crate::paged::{self, Lib, PKey};
use crate::theme;
use crate::ui::{self, t, white};
use crate::views::{self, LibTab, Page, go};

pub fn lib_of(t: LibTab) -> Lib {
    match t {
        LibTab::Playlists => Lib::Playlists,
        LibTab::Albums => Lib::Albums,
        LibTab::Artists => Lib::Artists,
        LibTab::Mixes => Lib::Mixes,
    }
}

pub fn key(kind: Lib, folder: &str, sort: &Sort) -> PKey {
    PKey::Library { kind, folder: folder.to_string(), order: sort.0.clone(), dir: sort.1.clone() }
}

pub fn set_lib_sort(s: &mut App, sort: Sort) {
    let k = lib_of(*s.nav.lib);
    s.nav.coll.ui.lib_sort.insert(coll::lib_name(k).into(), sort);
    coll::save(&s.nav.coll.ui);
}

/// Show all's sort: its own once changed there, else the library's.
pub fn view_sort(s: &App, k: Lib) -> Sort {
    match &s.nav.coll.ui.view_sort {
        Some((vk, sort)) if *vk == k => sort.clone(),
        _ => coll::lib_sort(&s.nav.coll.ui, k),
    }
}

pub fn set_view_sort(s: &mut App, sort: Sort) {
    if let Page::Library(k, _) = &*s.nav.page {
        let k = *k;
        s.nav.coll.ui.view_sort = Some((k, sort));
    }
}

/// A folder entry's id (its trn without "trn:folder:").
pub fn folder_id(it: &Value) -> String {
    it["trn"].as_str().unwrap_or("").trim_start_matches("trn:folder:").to_string()
}

pub fn is_folder(it: &Value) -> bool {
    it["itemType"].as_str() == Some("FOLDER")
}

/// folderSubtitle: "N playlists", or "Folder" when the server doesn't say.
fn folder_sub(it: &Value) -> String {
    match it["data"]["totalNumberOfItems"].as_u64().or(it["totalNumberOfItems"].as_u64()) {
        Some(n) => coll::count(n as usize, "playlist", "playlists"),
        None => "Folder".into(),
    }
}

/// A playlist's subtitle: "You" or its creator ("TIDAL" for editorial ones), and
/// playlistCountLabel.
fn playlist_sub(d: &Value) -> String {
    let me = data::user_id();
    let own = me == 0 || d["creator"]["id"].as_u64() == Some(me);
    let creator = if own { Some("You".to_string()) } else { d["creator"]["name"].as_str().filter(|n| !n.is_empty()).map(str::to_string).or_else(|| (d["creator"]["id"].as_u64() == Some(0)).then(|| "TIDAL".into())) };
    let count = if d["numberOfTracks"].is_null() { String::new() } else { count_label(d) };
    let sub = [creator.unwrap_or_default(), count].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
    if sub.is_empty() { "Playlist".into() } else { sub }
}

/// playlistCountLabel: "3 Videos · 12 Tracks", "1 Track", "0 Tracks".
pub fn count_label(d: &Value) -> String {
    let (tn, vn) = (d["numberOfTracks"].as_u64().unwrap_or(0), d["numberOfVideos"].as_u64().unwrap_or(0));
    let mut parts = vec![];
    if vn > 0 { parts.push(format!("{vn} Video{}", if vn == 1 { "" } else { "s" })) }
    if tn > 0 { parts.push(format!("{tn} Track{}", if tn == 1 { "" } else { "s" })) }
    if parts.is_empty() { parts.push("0 Tracks".into()) }
    parts.join(" · ")
}

/// A library item as a catalogue item ({type, data}) for cards, menus and pages.
pub fn as_item(kind: Lib, it: &Value) -> Value {
    match kind {
        Lib::Playlists => json!({"type": "PLAYLIST", "data": it["data"].clone()}),
        Lib::Albums => json!({"type": "ALBUM", "data": data::data(it).clone()}),
        Lib::Artists => json!({"type": "ARTIST", "data": data::data(it).clone()}),
        Lib::Mixes => json!({"type": "MIX", "data": data::data(it).clone()}),
    }
}

/// The library list's items still in the library (React filters by the favourite ids, so an
/// unloved album leaves at once).
fn still_in(kind: Lib, it: &Value) -> bool {
    if !interact::favorites_loaded() {
        return true;
    }
    let d = data::data(it);
    match kind {
        Lib::Playlists => true,
        Lib::Albums => interact::is_fav(interact::Fav::Album, &d["id"].to_string()),
        Lib::Artists => interact::is_fav(interact::Fav::Artist, &d["id"].to_string()),
        Lib::Mixes => interact::is_fav(interact::Fav::Mix, d["id"].as_str().unwrap_or("")),
    }
}

// ------------------------------------------------------------------ sidebar

/// The sidebar's sort button and Show all (React SortDropdown compact, "Show all").
pub fn sidebar_tools(s: &App) -> El {
    let k = lib_of(*s.nav.lib);
    let sort = coll::lib_sort(&s.nav.coll.ui, k);
    let label = format!("Sort by: {}", coll::sort_label(k, &sort));
    row().gap(2.0).items_center()
        .child(views::icon::<App>("lib-sort", &label, G::Sort).on_press(|s: &mut App| {
            s.nav.coll.ui.sort_menu = if s.nav.coll.ui.sort_menu == Some(SortMenu::Sidebar) { None } else { Some(SortMenu::Sidebar) };
        }))
        .child(theme::focusable(row().key("lib-all").role("button").label("Show all")).h(28.0).px(8.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                s.nav.coll.ui.view_sort = None;
                go(s, Page::Library(k, None));
            })
            .child(text("Show all").font(t(12.0, 600.0)).color(white(0.6)).a11y_hidden()))
}

/// The sidebar's list for the chosen chip.
pub fn sidebar_list(s: &App) -> El {
    let k = lib_of(*s.nav.lib);
    let sort = coll::lib_sort(&s.nav.coll.ui, k);
    let key = key(k, "root", &sort);
    // The sign-in bootstrap's folders are the default list's first page: the same request, so
    // while it is on its way the list waits for it rather than making it again.
    let mut bootstrap_coming = false;
    if k == Lib::Playlists && sort == coll::lib_sort(&coll::Ui::default(), Lib::Playlists) {
        match data::peek_load(&Key::Folders) {
            Some(data::Load::Ready(first)) => paged::seed(&key, &first),
            Some(data::Load::Loading) => bootstrap_coming = !paged::started(&key),
            _ => {}
        }
    }
    let mut list = col().w_full().gap(2.0);
    if k == Lib::Playlists {
        let on = *s.nav.page == Page::Loved;
        list = list.child(views::lib_row("lib-loved", "Loved Tracks Collection", "Loved Tracks", "Collection", 9, None, None, Some(Page::Loved), false).fill(if on { white(0.08) } else { white(0.0) }));
    }
    if bootstrap_coming {
        return list.child(crate::skeleton::lib_rows(5));
    }
    let snap = paged::get(&key);
    if snap.first_pending() {
        return list.child(crate::skeleton::lib_rows(5));
    }
    if snap.failed.is_some() && snap.len == 0 {
        let k2 = key.clone();
        return list.child(col().gap(8.0).px(8.0).py(8.0).items_start().child(ui::empty("Couldn't load your library"))
            .child(views::button::<App>("lib-retry", "Retry", None, false).on_press(move |_: &mut App| paged::retry(&k2))));
    }
    let items: Vec<&Value> = snap.shown().iter().filter(|it| still_in(k, it)).collect();
    if items.is_empty() && !snap.more {
        return match k {
            Lib::Playlists => list.child(col().w_full().gap(12.0).px(8.0).py(16.0).items_center()
                .child(ui::empty("Create your first playlist"))
                .child(ui::pill("lib-create", "Create playlist", None, ui::Pill::Primary).on_press(|s: &mut App| open_create(s, None)))),
            Lib::Albums => list.child(ui::empty("No favorite albums yet").px(8.0).py(12.0)),
            Lib::Artists => list.child(ui::empty("No followed artists yet").px(8.0).py(12.0)),
            Lib::Mixes => list.child(ui::empty("No favorite mixes yet").px(8.0).py(12.0)),
        };
    }
    let collapsed = s.nav.coll.ui.collapsed;
    if collapsed {
        // Only the covers, each still a button with the row's name.
        list = col().w_full().gap(4.0).items_center();
        if k == Lib::Playlists {
            list = list.child(compact_row("lib-loved", "Loved Tracks Collection", views::loved_art(40.0, 6.0, 15.0), Some(Page::Loved), None));
        }
    }
    for (i, it) in items.iter().enumerate() {
        list = list.child(if collapsed { sidebar_compact(k, i, it) } else { sidebar_row(s, k, i, it) });
    }
    if snap.more {
        // React's sentinel: the next 50 as the end of the list comes into view.
        let n = snap.len;
        let k2 = key.clone();
        list = list.child(el().key(format!("lib-more-{n}")).w_full().h(8.0).a11y_hidden().on_visible(move |_: &mut App, on, _| if on { paged::more(&k2) }));
    }
    if snap.loading && snap.len > 0 {
        list = list.child(crate::skeleton::lib_rows(2));
    }
    list
}

fn sidebar_row(s: &App, k: Lib, i: usize, it: &Value) -> El {
    match k {
        Lib::Playlists if is_folder(it) => {
            let (id, name) = (folder_id(it), it["name"].as_str().unwrap_or("").to_string());
            let sub = folder_sub(it);
            let on = matches!(&*s.nav.page, Page::Library(Lib::Playlists, Some((f, _))) if *f == id);
            let (id2, name2) = (id.clone(), name.clone());
            folder_row(&format!("lib-pl-{i}"), &format!("{name} {sub}"), &name, &sub, 40.0)
                .fill(if on { white(0.08) } else { white(0.0) })
                .on_press(move |s: &mut App| go(s, Page::Library(Lib::Playlists, Some((id.clone(), name.clone())))))
                .on_secondary_press(move |s: &mut App, p, _| open_folder_menu(s, id2.clone(), name2.clone(), Anchor::Point(p.x, p.y)))
        }
        Lib::Playlists => {
            let d = &it["data"];
            let title = d["title"].as_str().or(it["name"].as_str()).unwrap_or("").to_string();
            let sub = playlist_sub(d);
            let uuid = d["uuid"].as_str().unwrap_or("").to_string();
            let on = *s.nav.page == Page::Playlist(uuid.clone());
            // The React row's accessible name is its cover's alt text, title and subtitle.
            let label = format!("{title} {title} {sub}");
            views::lib_row(&format!("lib-pl-{i}"), &label, &title, &sub, i as u64 + 20, crate::art::item_url(it, 40.0), Some(as_item(k, it)), Some(Page::Playlist(uuid)), false)
                .fill(if on { white(0.08) } else { white(0.0) })
        }
        _ => {
            let item = as_item(k, it);
            let name = data::name(&item);
            let sub = match k {
                Lib::Artists => "Artist".to_string(),
                Lib::Mixes => Some(data::artist_of(&item)).filter(|x| !x.is_empty()).unwrap_or_else(|| "Mix".into()),
                _ => data::artist_of(&item),
            };
            let page = views::item_page(&item);
            let on = page.as_ref() == Some(&*s.nav.page);
            let img = if k == Lib::Artists { crate::art::artist_picture(data::data(&item), 40.0) } else { crate::art::item_url(&item, 40.0) };
            // Albums have no menu in the React sidebar; artists and mixes do.
            views::lib_row(&format!("lib-{k:?}-{i}"), &name, &name, &sub, i as u64 + 40, img, (k != Lib::Albums).then(|| item.clone()), page, k == Lib::Albums && data::data(&item)["ai"].as_bool() == Some(true))
                .fill(if on { white(0.08) } else { white(0.0) })
        }
    }
}

/// A collapsed sidebar's entry: the cover alone.
fn compact_row(key: &str, label: &str, art: El, page: Option<Page>, menu: Option<Box<dyn Fn(&mut App, f32, f32)>>) -> El {
    let mut r = theme::focusable(el().key(key.to_string()).role("button").label(label)).size(48.0, 48.0).radius(8.0).items_center().justify_center()
        .hover(white(0.06)).cursor(rune::Cursor::Pointer).help(label).child(art);
    if let Some(p) = page {
        r = r.on_press(move |s: &mut App| go(s, p.clone()));
    }
    if let Some(m) = menu {
        r = r.on_secondary_press(move |s: &mut App, p, _| m(s, p.x, p.y));
    }
    r
}

fn sidebar_compact(k: Lib, i: usize, it: &Value) -> El {
    if k == Lib::Playlists && is_folder(it) {
        let (id, name) = (folder_id(it), it["name"].as_str().unwrap_or("").to_string());
        let (id2, name2) = (id.clone(), name.clone());
        let art = el().size(40.0, 40.0).radius(6.0).fill(white(0.08)).items_center().justify_center().a11y_hidden().child(glyph(G::FolderOpen, 18.0, white(0.5)));
        return compact_row(&format!("lib-pl-{i}"), &name, art, Some(Page::Library(Lib::Playlists, Some((id, name.clone())))),
            Some(Box::new(move |s, x, y| open_folder_menu(s, id2.clone(), name2.clone(), Anchor::Point(x, y)))));
    }
    let item = as_item(k, it);
    let name = data::name(&item);
    let seed = i as u64 + 20;
    let round = k == Lib::Artists;
    let img = if round { crate::art::artist_picture(data::data(&item), 40.0) } else { crate::art::item_url(&item, 40.0) };
    let art = if round { ui::with_image(ui::art(ui::hue(Some(seed)), 40.0, seed, 0.0), img, 40.0, 20.0) } else { ui::with_image(ui::tile(ui::hue(Some(seed)), 40.0, seed, 6.0), img, 40.0, 6.0) };
    let key = if k == Lib::Playlists { format!("lib-pl-{i}") } else { format!("lib-{k:?}-{i}") };
    let menu: Option<Box<dyn Fn(&mut App, f32, f32)>> = (k != Lib::Albums).then(|| {
        let it2 = item.clone();
        Box::new(move |s: &mut App, x, y| interact::open_media(s, it2.clone(), Anchor::Point(x, y))) as Box<dyn Fn(&mut App, f32, f32)>
    });
    compact_row(&key, &name, art, views::item_page(&item), menu)
}

/// A folder's row: the folder icon on a tile, its name and count.
fn folder_row(key: &str, label: &str, name: &str, sub: &str, size: f32) -> El {
    theme::focusable_row(row().key(key.to_string()).role("button").label(label)).w_full().h(52.0).px(8.0).gap(10.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer).help(name)
        .child(el().size(size, size).radius(6.0).fill(white(0.08)).items_center().justify_center().a11y_hidden().child(glyph(G::FolderOpen, 18.0, white(0.5))))
        .child(col().min_w(0.0).a11y_hidden().child(text(name.to_string()).font(t(13.0, 600.0)).color(white(0.92)).truncate()).child(text(sub.to_string()).font(t(11.0, 400.0)).color(white(0.55)).truncate()))
}

// ------------------------------------------------------------------ Show all / folder page

fn config(k: Lib) -> (&'static str, &'static str) {
    match k {
        Lib::Playlists => ("Playlists", "Filter by title or creator"),
        Lib::Albums => ("Your favorite albums", "Filter by title or artist"),
        Lib::Artists => ("Artists you follow", "Filter by name"),
        Lib::Mixes => ("Mixes & Radios you liked", "Filter by title"),
    }
}

pub fn page(s: &App, k: Lib, folder: Option<&(String, String)>) -> El {
    let (title, placeholder) = config(k);
    let sort = view_sort(s, k);
    let fid = folder.map(|f| f.0.clone()).unwrap_or_else(|| "root".into());
    let key = key(k, &fid, &sort);
    let snap = paged::get(&key);
    let needle = s.nav.filter.text.trim().to_string();
    let filtering = !needle.is_empty();
    if filtering {
        paged::all(&key);
    }
    let mut head = row().w_full().gap(10.0).items_center().child(views::heading(folder.map(|f| f.1.as_str()).unwrap_or(title), theme::PAGE_TITLE).truncate());
    if let Some((id, name)) = folder.cloned() {
        head = head.child(views::icon::<App>("lv-folder-more", "More options", G::More).on_press(move |s: &mut App| open_folder_menu(s, id.clone(), name.clone(), Anchor::Below("lv-folder-more".into()))));
    }
    // Every item the filter can match: with playlists, those in folders too (React's flattened
    // list).
    let mut pool: Vec<Value> = snap.shown().iter().filter(|it| still_in(k, it)).cloned().collect();
    if filtering && k == Lib::Playlists {
        let have: std::collections::HashSet<String> = pool.iter().filter_map(|it| it["data"]["uuid"].as_str().map(str::to_string)).collect();
        if let data::Load::Ready(flat) = data::want(Key::FlattenedPlaylists) {
            pool.extend(flat.as_array().cloned().unwrap_or_default().into_iter()
                .filter(|it| it["itemType"].as_str() == Some("PLAYLIST") && !it["data"]["uuid"].as_str().is_some_and(|u| have.contains(u))));
        }
    }
    let hits: Vec<Value> = pool.into_iter().filter(|it| !filtering || match k {
        Lib::Playlists if is_folder(it) => data::matches(it["name"].as_str().unwrap_or(""), &needle),
        Lib::Playlists => {
            let d = &it["data"];
            [d["title"].as_str(), d["description"].as_str(), d["creator"]["name"].as_str()].iter().flatten().any(|x| data::matches(x, &needle))
        }
        Lib::Albums => data::matches(&format!("{} {}", data::name(it), data::artist_of(&as_item(k, it))), &needle),
        Lib::Artists => data::matches(&data::name(it), &needle),
        Lib::Mixes => data::matches(&format!("{} {}", data::name(it), data::data(it)["subTitle"].as_str().unwrap_or("")), &needle),
    }).collect();
    let n = if filtering { hits.len() } else if k == Lib::Playlists { snap.total.unwrap_or(snap.len) } else { snap.total.unwrap_or(snap.len) };
    let noun = match k { Lib::Playlists => ("playlist", "playlists"), Lib::Albums => ("album", "albums"), Lib::Artists => ("artist", "artists"), Lib::Mixes => ("mix", "mixes") };
    let sort_label = format!("{} {}", coll::sort_label(k, &sort), if sort.1 == "ASC" { "↑" } else { "↓" });
    let meta = row().gap(10.0).items_center().child(coll::dim(&coll::count(n, noun.0, noun.1)))
        .child(theme::focusable(row().key("lv-sort").role("button").label(format!("Sort by: {}", coll::sort_label(k, &sort)).as_str())).help(format!("Sort by: {}", coll::sort_label(k, &sort))).h(28.0).px(10.0).gap(6.0).radius(8.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| s.nav.coll.ui.sort_menu = if s.nav.coll.ui.sort_menu == Some(SortMenu::ViewAll) { None } else { Some(SortMenu::ViewAll) })
            .child(glyph(G::Sort, 13.0, white(0.55)))
            .child(text(sort_label).font(t(12.0, 500.0)).color(white(0.6)).a11y_hidden()));
    let filter = text_input(&s.nav.filter, t(13.0, 500.0).lh(18.0))
        .key("lv-filter").label(placeholder).single_line().w(320.0)
        .color(white(0.95)).caret(white(0.95), white(0.25)).placeholder(placeholder, white(0.4))
        .on_edit(|s: &mut App| &mut s.nav.filter);
    let mut c = col().w_full().gap(16.0).child(col().gap(6.0).child(head).child(meta))
        .child(row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).child(filter));
    if snap.first_pending() {
        return c.child(crate::skeleton::card_grid(18));
    }
    if snap.failed.is_some() && snap.len == 0 {
        let k2 = key.clone();
        return c.child(col().gap(10.0).items_start().child(ui::empty("Couldn't load this"))
            .child(views::button::<App>("lv-retry", "Retry", None, false).on_press(move |_: &mut App| paged::retry(&k2))));
    }
    if hits.is_empty() && !snap.loading {
        return c.child(ui::empty(&if filtering { format!("No {} match your search", coll::lib_name(k)) } else { format!("No {} yet", coll::lib_name(k)) }));
    }
    let mut cards = vec![];
    for (i, it) in hits.iter().enumerate() {
        let ck = format!("lv-{i}");
        if k == Lib::Playlists && is_folder(it) {
            cards.push(folder_card(ck, it));
            continue;
        }
        let item = as_item(k, it);
        let Some(page) = views::item_page(&item) else { continue };
        let d = data::data(&item);
        let sub = match k {
            Lib::Playlists => playlist_sub(d),
            Lib::Artists => "Artist".into(),
            _ => data::artist_of(&item),
        };
        let img = if k == Lib::Artists { crate::art::artist_picture(d, 132.0) } else { crate::art::item_url(&item, 132.0) };
        let seed = d["id"].as_u64().unwrap_or(i as u64 + 1);
        cards.push(views::card(ck, &data::name(&item), &sub, seed, k == Lib::Artists, img, Some(item.clone()), page));
    }
    c = c.child(views::grid(cards));
    if snap.more && !filtering {
        let n = snap.len;
        let k2 = key.clone();
        c = c.child(el().key(format!("lv-more-{n}")).w_full().h(8.0).a11y_hidden().on_visible(move |_: &mut App, on, _| if on { paged::more(&k2) }));
    }
    if snap.loading && snap.len > 0 {
        c = c.child(crate::skeleton::card_grid(6));
    }
    c
}

/// A folder as a card (LibraryViewAll): the folder icon, its name and count; it opens the
/// folder, right-click opens the folder menu.
fn folder_card(key: String, it: &Value) -> El {
    let (id, name) = (folder_id(it), it["name"].as_str().unwrap_or("").to_string());
    let sub = folder_sub(it);
    let (id2, name2, id3, name3) = (id.clone(), name.clone(), id.clone(), name.clone());
    col().key(key).role("presentation").w(148.0).gap(6.0).padding(8.0).radius(12.0).hover(white(0.06)).focus_within_ring(2.0, -2.0, white(0.6)).cursor(rune::Cursor::Pointer)
        .on_press(move |s: &mut App| go(s, Page::Library(Lib::Playlists, Some((id.clone(), name.clone())))))
        .on_secondary_press(move |s: &mut App, p, _| open_folder_menu(s, id2.clone(), name2.clone(), Anchor::Point(p.x, p.y)))
        .child(el().size(132.0, 132.0).radius(10.0).fill(white(0.08)).items_center().justify_center().a11y_hidden().child(glyph(G::FolderOpen, 32.0, white(0.45))))
        .child(views::heading(&name3, 13.0).truncate().focusable(true).no_focus_ring().on_press(move |s: &mut App| go(s, Page::Library(Lib::Playlists, Some((id3.clone(), name3.clone()))))))
        .child(text(sub).font(t(11.0, 400.0)).color(white(0.55)).truncate())
}

// ------------------------------------------------------------------ folders

pub fn open_folder_menu(s: &mut App, id: String, name: String, anchor: Anchor) {
    s.nav.coll.folder_menu = Some(FolderMenu { id, name, anchor });
}

/// The create-playlist dialog (interact.rs), and with `folder` the new playlist goes into it.
fn open_create(s: &mut App, folder: Option<String>) {
    s.nav.coll.create_in = folder;
    s.nav.create = Some(interact::Create::new(vec![]));
}

/// Every list a folder change can touch: the library's playlist lists and folders, and the
/// playlist pickers (after a folder change, an MCP client's playlist change).
pub fn refresh_folders() {
    data::forget(&Key::Folders);
    data::forget(&Key::AllPlaylists);
    data::forget(&Key::FlattenedPlaylists);
    data::forget(&Key::AllFolders);
    paged::forget_where(|k| matches!(k, PKey::Library { kind: Lib::Playlists, .. }));
}

/// The folder menu, the folder dialogs, and moving a just-created playlist into its folder.
pub fn overlay(s: &App) -> Vec<El> {
    let mut out = vec![];
    if let Some(m) = &s.nav.coll.folder_menu {
        let layer = el().key("fm-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .modal(false, |s: &mut App| s.nav.coll.folder_menu = None)
            .on_press(|s: &mut App| s.nav.coll.folder_menu = None).on_secondary_press(|s: &mut App, _, _| s.nav.coll.folder_menu = None);
        let item = |key: &str, label: &str, g: G, danger: bool, on: Box<dyn Fn(&mut App)>| {
            let c = if danger { theme::danger() } else { white(0.78) };
            theme::focusable_row(row().key(key.to_string()).role("button").label(label)).w_full().radius(theme::inset_radius(4.0)).px(16.0).h(40.0).gap(12.0).items_center()
                .hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| on(s))
                .child(glyph(g, 18.0, if danger { c } else { white(0.5) }))
                .child(text(label.to_string()).font(t(14.0, 500.0)).color(c).a11y_hidden())
        };
        let (id, name) = (m.id.clone(), m.name.clone());
        let (id2, name2, id3) = (id.clone(), name.clone(), id.clone());
        let menu = theme::overlay_surface(col().key("fm-menu").role("menu").label("Folder").w(240.0).padding(4.0), theme::Overlay::Menu).focus_group().on_press(|_: &mut App| {})
            .child(item("fm-rename", "Rename", G::Pencil, false, Box::new(move |s| {
                s.nav.coll.folder_menu = None;
                s.nav.coll.folder_dialog = Some(FolderDialog::Rename { id: id.clone(), name: name.clone(), field: { let mut f = TextEdit::new(&name); f.focused = true; f }, saving: false, error: None });
            })))
            .child(item("fm-create", "Create playlist", G::List, false, Box::new(move |s| {
                s.nav.coll.folder_menu = None;
                open_create(s, Some(id3.clone()));
            })))
            .child(el().w_full().h(1.0).my(4.0).fill(white(0.08)))
            .child(item("fm-delete", "Delete folder", G::Trash, true, Box::new(move |s| {
                s.nav.coll.folder_menu = None;
                s.nav.coll.folder_dialog = Some(FolderDialog::Delete { id: id2.clone(), name: name2.clone(), busy: false });
            })));
        let layer = match &m.anchor {
            Anchor::Point(x, y) => layer.child(el().key("fm-point").pin(Some(*x), Some(*y), None, None).size(1.0, 1.0)).child(menu.anchored("fm-point", Side::Bottom, rune::Align::Start, 0.0)),
            Anchor::Below(k) => layer.child(menu.anchored(k.clone(), Side::Bottom, rune::Align::End, 4.0)),
        };
        out.push(layer);
    }
    if let Some(d) = &s.nav.coll.folder_dialog {
        out.push(match d {
            FolderDialog::Rename { field, saving, error, name, .. } => {
                let can = !field.text.trim().is_empty() && field.text.trim() != name;
                let body = col().gap(14.0)
                    .child(coll::field(field, "rf-name", "Folder name", dialog_field, rename_folder))
                    .child(coll::error_line(error))
                    .child(row().child(spacer()).child(coll::save_button("rf-save", "Save", can, *saving, rename_folder)));
                ui::dialog_frame("rf-layer", "rf-dialog", "Rename folder", ("rf-x", "Close"), 520.0, None, Some(ui::dialog_title("Rename folder")), body, close_dialog)
            }
            FolderDialog::Create { field, saving, error, .. } => {
                let can = !field.text.trim().is_empty();
                let body = col().gap(14.0)
                    .child(coll::field(field, "cf-name", "Folder name", dialog_field, create_folder))
                    .child(coll::error_line(error))
                    .child(row().child(spacer()).child(coll::save_button("cf-save", "Save", can, *saving, create_folder)));
                ui::dialog_frame("cf-layer", "cf-dialog", "Create folder", ("cf-x", "Close"), 520.0, None, Some(ui::dialog_title("Create folder")), body, close_dialog)
            }
            FolderDialog::Delete { name, busy, .. } => {
                let body = col().gap(18.0)
                    .child(text(format!("Are you sure you want to delete \"{name}\"? This can't be undone.")).key("df-msg").font(t(14.0, 400.0)).color(white(0.75)).wrap())
                    .child(row().gap(10.0).child(spacer())
                        .child(ui::pill("df-cancel", "Cancel", None, ui::Pill::Secondary).on_press(close_dialog))
                        .child(ui::pill("df-ok", if *busy { "Deleting…" } else { "Delete" }, None, ui::Pill::Danger).described_by("df-msg").busy(*busy).on_press(delete_folder)));
                ui::dialog_frame("df-layer", "df-dialog", "Delete folder?", ("df-x", "Close"), 400.0, None, Some(ui::dialog_title("Delete folder?")), body, close_dialog)
            }
        });
    }
    out
}

fn close_dialog(s: &mut App) {
    s.nav.coll.folder_dialog = None;
}

fn dialog_field(s: &mut App) -> &mut TextEdit {
    match &mut s.nav.coll.folder_dialog {
        Some(FolderDialog::Rename { field, .. } | FolderDialog::Create { field, .. }) => field,
        _ => unreachable!("the field shows with its dialog"),
    }
}

fn st() -> sone_host_shim::State<'static, AppState> {
    crate::host().state::<AppState>()
}

/// Saves, or on failure says so in the dialog and lets it save again.
fn dialog_failed(msg: &'static str) {
    coll::later(move |s| match &mut s.nav.coll.folder_dialog {
        Some(FolderDialog::Rename { saving, error, .. } | FolderDialog::Create { saving, error, .. }) => {
            *saving = false;
            *error = Some(msg.into());
        }
        _ => {}
    });
}

fn rename_folder(s: &mut App) {
    let Some(FolderDialog::Rename { id, name, field, saving, error }) = &mut s.nav.coll.folder_dialog else { return };
    let new = field.text.trim().to_string();
    if new.is_empty() || new == *name || *saving {
        return;
    }
    *saving = true;
    *error = None;
    let id = id.clone();
    sone_host_shim::async_runtime::spawn(async move {
        match library::rename_playlist_folder(st(), format!("trn:folder:{id}"), new.clone()).await {
            Ok(()) => {
                refresh_folders();
                interact::toast(format!("Renamed to \"{new}\""), Kind::Success);
                coll::later(move |s| {
                    s.nav.coll.folder_dialog = None;
                    // The folder's own page takes the new name.
                    if let Page::Library(Lib::Playlists, Some((f, n))) = &mut *s.nav.page
                        && *f == id
                    {
                        *n = new;
                    }
                });
            }
            Err(e) => {
                log::warn!("[rune] rename folder: {e}");
                dialog_failed("Failed to rename folder");
            }
        }
    });
}

fn delete_folder(s: &mut App) {
    let Some(FolderDialog::Delete { id, name, busy }) = &mut s.nav.coll.folder_dialog else { return };
    if *busy {
        return;
    }
    *busy = true;
    let (id, name) = (id.clone(), name.clone());
    sone_host_shim::async_runtime::spawn(async move {
        let res = library::delete_playlist_folder(st(), format!("trn:folder:{id}")).await;
        let ok = res.is_ok();
        match res {
            Ok(()) => {
                refresh_folders();
                interact::toast(format!("Deleted \"{}\"", coll::clip(&name, 28)), Kind::Success);
            }
            Err(e) => {
                log::warn!("[rune] delete folder: {e}");
                interact::toast("Failed to delete folder".into(), Kind::Error);
            }
        }
        coll::later(move |s| {
            s.nav.coll.folder_dialog = None;
            // Deleting the folder you're in goes Home in its place (replaceView).
            if ok && matches!(&*s.nav.page, Page::Library(Lib::Playlists, Some((f, _))) if *f == id) {
                *s.nav.page = Page::Home;
            }
        });
    });
}

/// Move to folder's "Create new folder": the dialog that makes a folder with this playlist in it.
pub fn open_create_folder(s: &mut App, playlist: String, label: String) {
    s.nav.coll.folder_dialog = Some(FolderDialog::Create { playlist, label, field: { let mut f = TextEdit::new(""); f.focused = true; f }, saving: false, error: None });
}

fn create_folder(s: &mut App) {
    let Some(FolderDialog::Create { playlist, label, field, saving, error }) = &mut s.nav.coll.folder_dialog else { return };
    let name = field.text.trim().to_string();
    if name.is_empty() || *saving {
        return;
    }
    *saving = true;
    *error = None;
    let (playlist, label) = (playlist.clone(), label.clone());
    sone_host_shim::async_runtime::spawn(async move {
        match library::create_playlist_folder(st(), "root".into(), name.clone(), format!("trn:playlist:{playlist}")).await {
            Ok(v) => {
                // The new folder's id, from the answer when it says (useFolders.createFolder).
                let id = v["items"].as_array().or(v["data"]["items"].as_array()).and_then(|items| items.iter().find_map(|it| {
                    let d = if it["data"].is_object() { &it["data"] } else { it };
                    d["trn"].as_str().filter(|t| t.starts_with("trn:folder:") && d["name"].as_str() == Some(name.as_str())).map(|t| t.trim_start_matches("trn:folder:").to_string())
                }));
                if let Some(id) = id {
                    coll::later(move |s| push_recent(s, &id));
                }
                refresh_folders();
                interact::toast(format!("Moved \"{}\" to \"{}\"", coll::clip(&label, 23), coll::clip(&name, 23)), Kind::Success);
                coll::later(|s| s.nav.coll.folder_dialog = None);
            }
            Err(e) => {
                log::warn!("[rune] create folder: {e}");
                dialog_failed("Failed to create folder");
            }
        }
    });
}

fn push_recent(s: &mut App, id: &str) {
    let ui = &mut s.nav.coll.ui;
    ui.recent_folders.retain(|f| f != id);
    ui.recent_folders.insert(0, id.to_string());
    ui.recent_folders.truncate(8);
    coll::save(ui);
}

/// After the create-playlist dialog made a playlist, from a folder's "Create playlist": move it
/// into that folder (FolderContextMenu's onCreated). Called by interact.rs with the new uuid.
pub fn created_playlist(uuid: String) {
    coll::later(move |s| {
        let Some(folder) = s.nav.coll.create_in.take() else { return };
        sone_host_shim::async_runtime::spawn(async move {
            if library::move_playlist_to_folder(st(), folder, format!("trn:playlist:{uuid}")).await.is_err() {
                interact::toast("Playlist created but could not be moved to folder".into(), Kind::Error);
            }
            refresh_folders();
        });
    });
}

// ------------------------------------------------------------------ Move to folder

/// Every folder, by name (MoveToFolderMenu fetches every page of the top level's folders).
fn all_folders() -> Option<Vec<Value>> {
    let v = data::peek(&Key::AllFolders)?;
    let mut f: Vec<Value> = v.as_array().cloned().unwrap_or_default();
    f.sort_by_key(|x| x["name"].as_str().unwrap_or("").to_lowercase());
    Some(f)
}

/// Move to folder (MoveToFolderMenu): Create new folder, Show all folders (with Find), the
/// recent folders, and Playlists (the top level). `all`/`query` are the submenu's own state.
pub fn move_menu(s: &App, uuid: &str, label: &str, all: bool, query: &TextEdit, anchor: &str) -> El {
    data::want(Key::AllFolders);
    let mut m = theme::overlay_surface(col().key("ctx-sub").role("menu").w(320.0).padding(4.0), theme::Overlay::Menu).focus_group().on_press(|_: &mut App| {})
        .anchored(anchor.to_string(), Side::Right, rune::Align::Start, 6.0);
    let folders = all_folders();
    let row_for = |i: usize, f: &Value, detailed: bool| -> El {
        let (fid, fname) = (folder_id(f), f["name"].as_str().unwrap_or("").to_string());
        let (u, l, n2) = (uuid.to_string(), label.to_string(), fname.clone());
        let mut r = theme::focusable_row(row().key(format!("mtf-{i}")).role("button").label(fname.as_str())).w_full().radius(theme::inset_radius(4.0)).px(14.0).h(if detailed { 56.0 } else { 38.0 }).gap(10.0).items_center()
            .hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                s.nav.ctx = None;
                move_to(u.clone(), l.clone(), fid.clone(), n2.clone());
            });
        r = if detailed {
            r.child(el().size(40.0, 40.0).radius(6.0).fill(white(0.08)).items_center().justify_center().child(glyph(G::FolderOpen, 20.0, white(0.5))))
                .child(col().grow(1.0).min_w(0.0).a11y_hidden()
                    .child(text(fname.clone()).font(t(14.0, 500.0)).color(white(0.85)).truncate())
                    .child(text(folder_sub(f)).font(t(12.0, 400.0)).color(white(0.5))))
        } else {
            r.child(glyph(G::FolderOpen, 16.0, white(0.6))).child(text(fname).font(t(14.0, 500.0)).color(white(0.85)).truncate().a11y_hidden())
        };
        r
    };
    let set_all = |on: bool| move |s: &mut App| if let Some(interact::Ctx { sub: Some(interact::Sub::MoveToFolder { all, query, .. }), .. }) = &mut s.nav.ctx {
        *all = on;
        *query = TextEdit::new("");
    };
    if all {
        m = m.child(row().mx(12.0).my(6.0).h(34.0).px(10.0).gap(8.0).radius(8.0).items_center().fill(white(0.06))
                .child(glyph(G::Search, 14.0, white(0.5)))
                .child(text_input(query, t(13.0, 500.0).lh(18.0)).key("mtf-find").label("Find a folder").single_line().grow(1.0).color(white(0.92)).placeholder("Find a folder", white(0.4))
                    .on_edit(|s: &mut App| match &mut s.nav.ctx {
                        Some(interact::Ctx { sub: Some(interact::Sub::MoveToFolder { query, .. }), .. }) => query,
                        _ => unreachable!("the find field only shows with its submenu"),
                    })))
            .child(theme::focusable_row(row().key("mtf-back").role("button").label("← Back")).radius(theme::inset_radius(4.0)).px(14.0).h(32.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
                .on_press(set_all(false))
                .child(text("← Back").font(t(12.0, 500.0)).color(white(0.85)).a11y_hidden()));
        match &folders {
            None => m = m.busy(true).child(row().px(14.0).h(40.0).items_center().child(glyph(G::Loader, 16.0, white(0.5)))),
            Some(fs) => {
                let hits: Vec<&Value> = fs.iter().filter(|f| data::matches(f["name"].as_str().unwrap_or(""), &query.text)).collect();
                if hits.is_empty() {
                    m = m.child(ui::empty(if query.text.trim().is_empty() { "No folders yet" } else { "No folders found" }).px(14.0).py(10.0));
                }
                let mut list = col().w_full().max_h(300.0).scroll(rune::ScrollSpec { stick_bottom: false });
                for (i, f) in hits.into_iter().enumerate() {
                    list = list.child(row_for(100 + i, f, true));
                }
                m = m.child(list);
            }
        }
        return m;
    }
    let (u, l) = (uuid.to_string(), label.to_string());
    m = m.child(theme::focusable_row(row().key("mtf-create").role("button").label("Create new folder")).w_full().radius(theme::inset_radius(4.0)).px(14.0).h(48.0).gap(12.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                s.nav.ctx = None;
                open_create_folder(s, u.clone(), l.clone());
            })
            .child(el().size(32.0, 32.0).radius(16.0).fill(white(0.08)).items_center().justify_center().child(glyph(G::Plus, 18.0, white(0.85))))
            .child(text("Create new folder").font(t(15.0, 500.0)).color(white(0.92)).a11y_hidden()))
        .child(theme::focusable_row(row().key("mtf-all").role("button").label("Show all folders")).w_full().radius(theme::inset_radius(4.0)).px(14.0).h(36.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(set_all(true))
            .child(text("Show all folders").font(t(13.0, 500.0)).color(white(0.55)).a11y_hidden()));
    if let Some(fs) = &folders {
        let recent: Vec<&Value> = s.nav.coll.ui.recent_folders.iter().filter_map(|id| fs.iter().find(|f| folder_id(f) == *id)).collect();
        if !recent.is_empty() {
            m = m.child(text("RECENT").font(t(11.0, 700.0).tracking(1.2)).color(white(0.45)).px(14.0).py(6.0));
            for (i, f) in recent.into_iter().enumerate() {
                m = m.child(row_for(i, f, false));
            }
        }
    }
    let (u, l) = (uuid.to_string(), label.to_string());
    m.child(el().w_full().h(1.0).my(4.0).fill(white(0.08)))
        .child(theme::focusable_row(row().key("mtf-root").role("button").label("Playlists")).w_full().radius(theme::inset_radius(4.0)).px(14.0).h(38.0).gap(10.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                s.nav.ctx = None;
                move_to(u.clone(), l.clone(), "root".into(), String::new());
            })
            .child(glyph(G::FolderInput, 16.0, white(0.6)))
            .child(text("Playlists").font(t(14.0, 500.0)).color(white(0.85)).a11y_hidden()))
}

fn move_to(uuid: String, label: String, folder: String, folder_name: String) {
    sone_host_shim::async_runtime::spawn(async move {
        match library::move_playlist_to_folder(st(), folder.clone(), format!("trn:playlist:{uuid}")).await {
            Ok(()) => {
                refresh_folders();
                let l = coll::clip(&label, 23);
                if folder == "root" {
                    interact::toast(format!("Moved \"{l}\" to Playlists"), Kind::Success);
                } else {
                    interact::toast(format!("Moved \"{l}\" to \"{}\"", coll::clip(&folder_name, 23)), Kind::Success);
                    coll::later(move |s| push_recent(s, &folder));
                }
            }
            Err(e) => {
                log::warn!("[rune] move to folder: {e}");
                interact::toast("Failed to move playlist".into(), Kind::Error);
            }
        }
    });
}
