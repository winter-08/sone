//! State shared by the collection pages and the library (playlist.rs, loved.rs, library.rs,
//! viewall.rs): track sorts per list, the library sorts, tabs, the open sort menu and the
//! playlist and folder dialogs; and the pieces those pages share (a sortable track list's
//! header, Play/Shuffle over a paged collection, the sort menu).
//!
//! The sorts persist as React's localStorage atoms do (sone.trackSortPrefs.v1, sone.*Sort.v1),
//! in `rune-library.json` beside the backend's settings; so do the recent folders.

use std::collections::BTreeMap;

use rune::{El, Side, TextEdit, Tracked, col, el, row, text};
use serde::{Deserialize, Serialize};

use crate::App;
use crate::glyph::{G, glyph};
use crate::paged::Lib;
use crate::theme::{self, accent};
use crate::ui::{self, t, white};
use crate::views;

/// A sort: the server's order key ("DATE", "NAME", …) and direction ("ASC" / "DESC").
pub type Sort = (String, String);

/// What the collection panes read (`Tracked`, so their regions rebuild when it changes).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ui {
    /// A track list's sort by list (a playlist's uuid, "__favorites__"), as trackSortPrefs.
    pub track_sort: BTreeMap<String, Sort>,
    /// The library's sort per kind (playlistSortAtom, albumSortAtom, …).
    pub lib_sort: BTreeMap<String, Sort>,
    /// Folders a playlist was moved to lately, newest first (sone.recent-folders.v1).
    pub recent_folders: Vec<String>,
    /// Searches run lately, newest first (SearchBar's sone.search-history).
    pub search_history: Vec<String>,
    /// The sidebar is collapsed to its icons (sidebarCollapsedAtom).
    #[serde(skip)]
    pub collapsed: bool,
    #[serde(skip)]
    pub loved_videos: bool,
    /// Show all's own sort, started from the library's when the page opens (React keeps it in
    /// the page's state).
    #[serde(skip)]
    pub view_sort: Option<(Lib, Sort)>,
    /// The open sort menu.
    #[serde(skip)]
    pub sort_menu: Option<SortMenu>,
    /// A playlist's recommended tracks: (playlist, page of 10 shown, api offset of the batch).
    #[serde(skip)]
    pub recs: Option<(String, usize, usize)>,
    /// Recommended tracks added to the playlist this visit (taken out of the list at once).
    #[serde(skip)]
    pub recs_added: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMenu {
    Sidebar,
    ViewAll,
}

/// The playlist editor (React EditPlaylistModal).
#[derive(Clone, Debug)]
pub struct Edit {
    pub uuid: String,
    pub title: TextEdit,
    pub desc: TextEdit,
    pub public: bool,
    pub saving: bool,
    pub error: Option<String>,
}

/// A folder dialog: rename, delete, create (moving a playlist into the new folder), or a new
/// playlist made inside a folder.
#[derive(Clone, Debug)]
pub enum FolderDialog {
    Rename { id: String, name: String, field: TextEdit, saving: bool, error: Option<String> },
    Delete { id: String, name: String, busy: bool },
    Create { playlist: String, label: String, field: TextEdit, saving: bool, error: Option<String> },
}

/// The folder menu (React FolderContextMenu), at the pointer or under the folder page's More.
#[derive(Clone, Debug)]
pub struct FolderMenu {
    pub id: String,
    pub name: String,
    pub anchor: crate::interact::Anchor,
}

#[derive(Default)]
pub struct Coll {
    pub ui: Tracked<Ui>,
    pub edit: Option<Edit>,
    /// A playlist's whole description (its "Read more"): title and text.
    pub description: Option<(String, String)>,
    pub folder_menu: Option<FolderMenu>,
    pub folder_dialog: Option<FolderDialog>,
    /// A new playlist to be moved into this folder once created (FolderContextMenu's Create
    /// playlist).
    pub create_in: Option<String>,
    loaded: bool,
}

fn prefs_path() -> Option<std::path::PathBuf> {
    tauri_app_lib::config_dir_for_env().map(|d| d.join("rune-library.json"))
}

impl Coll {
    /// Reads the saved sorts once.
    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Some(saved) = prefs_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str::<Ui>(&s).ok()) {
            *self.ui = saved;
        }
    }
}

/// Writes the persisted part of `ui`.
pub fn save(ui: &Ui) {
    if let Some(p) = prefs_path()
        && let Ok(s) = serde_json::to_string(ui)
    {
        let _ = std::fs::write(p, s);
    }
}

pub fn lib_name(k: Lib) -> &'static str {
    match k {
        Lib::Playlists => "playlists",
        Lib::Albums => "albums",
        Lib::Artists => "artists",
        Lib::Mixes => "mixes",
    }
}

/// The library's sort for `k` (playlists by last update, the rest by date added, newest first).
pub fn lib_sort(ui: &Ui, k: Lib) -> Sort {
    ui.lib_sort.get(lib_name(k)).cloned().unwrap_or_else(|| ((if k == Lib::Playlists { "DATE_UPDATED" } else { "DATE" }).into(), "DESC".into()))
}

/// SORT_OPTIONS: each kind's orders and their labels.
pub fn sort_options(k: Lib) -> &'static [(&'static str, &'static str)] {
    match k {
        Lib::Playlists => &[("DATE", "Date added"), ("DATE_UPDATED", "Last updated"), ("NAME", "Name")],
        Lib::Albums => &[("DATE", "Date added"), ("NAME", "Name"), ("ARTIST", "Artist"), ("RELEASE_DATE", "Release date")],
        Lib::Artists => &[("DATE", "Date added"), ("NAME", "Name")],
        Lib::Mixes => &[("DATE", "Date added"), ("NAME", "Name"), ("MIX_TYPE", "Type")],
    }
}

pub fn sort_label(k: Lib, s: &Sort) -> &'static str {
    sort_options(k).iter().find(|o| o.0 == s.0).map(|o| o.1).unwrap_or("Date added")
}

/// SortDropdown.handleSelect: the same order flips its direction; a new one starts descending
/// (by name, ascending).
pub fn next_sort(cur: &Sort, order: &str) -> Sort {
    if cur.0 == order {
        (order.into(), if cur.1 == "DESC" { "ASC".into() } else { "DESC".into() })
    } else {
        (order.into(), if order == "NAME" { "ASC".into() } else { "DESC".into() })
    }
}

/// The sort menu under `anchor` (React SortDropdown's list): each order, the current one in the
/// accent with its direction arrow.
pub fn sort_menu(k: Lib, cur: &Sort, anchor: &str, pick: fn(&mut App, Sort)) -> El {
    let mut m = theme::overlay_surface(col().key("sort-menu").role("menu").label("Sort by").w(180.0).padding(4.0), theme::Overlay::Menu)
        .focus_group().on_press(|_: &mut App| {})
        .anchored(anchor.to_string(), Side::Bottom, rune::Align::End, 4.0);
    for (i, (order, label)) in sort_options(k).iter().enumerate() {
        let on = cur.0 == *order;
        let next = next_sort(cur, order);
        let arrow = if on { if cur.1 == "ASC" { " ↑" } else { " ↓" } } else { "" };
        let name = format!("{label}{arrow}");
        m = m.child(theme::focusable_row(row().key(format!("sort-{i}")).role("button").label(name.as_str())).w_full().h(32.0).px(12.0).radius(theme::inset_radius(4.0)).items_center()
            .hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                s.nav.coll.ui.sort_menu = None;
                pick(s, next.clone());
            })
            .child(text(name.clone()).font(t(12.0, 500.0)).color(if on { accent() } else { white(0.75) }).a11y_hidden()));
    }
    m
}

/// Play and Shuffle for a collection whose tracks page in (usePlaySource over its pager).
/// `source` is the play source, for the Pause/Resume state as `views::play_buttons` has it.
pub fn play_pair(v: &crate::controller::View, source: &serde_json::Value, play: impl Fn(bool) + Clone + 'static) -> El {
    let label = views::play_label(v, source);
    let toggle = label != "Play";
    let p2 = play.clone();
    row().gap(8.0).items_center()
        .child(views::button::<App>("hero-play", label, Some(if label == "Pause" { G::Pause } else { G::Play }), true).on_press(move |_: &mut App| {
            if toggle { crate::controller::send(crate::controller::Cmd::TogglePlay) } else { play(false) }
        }))
        .child(views::button::<App>("hero-shuffle", "Shuffle", Some(G::Shuffle), false).on_press(move |_: &mut App| p2(true)))
}

/// A sortable track list's column header (TrackList's header row): Title, Artist, Album,
/// optionally Date Added, and Time, each a button that sorts by it (again to flip the
/// direction), the current one with its arrow.
pub fn sort_header(key: &str, cur: Option<&Sort>, date_added: bool, pick: impl Fn(&mut App, Option<Sort>) + Clone + 'static) -> El {
    let mut r = row().key(format!("{key}-head")).w_full().h(32.0).px(12.0).gap(12.0).items_center();
    let cols: &[(&str, &str)] = if date_added {
        &[("NAME", "Title"), ("ARTIST", "Artist"), ("ALBUM", "Album"), ("DATE", "Date Added"), ("LENGTH", "Time")]
    } else {
        &[("NAME", "Title"), ("ARTIST", "Artist"), ("ALBUM", "Album"), ("LENGTH", "Time")]
    };
    // The rows have no column per field here, so the header is a row of sort keys under a
    // caption rather than column titles over nothing.
    r = r.child(text("SORT BY").font(t(11.0, 650.0).tracking(0.8)).color(white(0.35)).a11y_hidden());
    for (order, label) in cols {
        let on = cur.is_some_and(|c| c.0 == *order);
        let arrow = match cur { Some(c) if on => if c.1 == "ASC" { " ↑" } else { " ↓" }, _ => "" };
        let name = format!("Sort by {label}{arrow}");
        let next = match cur {
            Some(c) if c.0 == *order => (order.to_string(), if c.1 == "ASC" { "DESC".into() } else { "ASC".into() }),
            _ => (order.to_string(), "ASC".to_string()),
        };
        let p = pick.clone();
        let cell = theme::focusable(row().key(format!("{key}-sort-{order}")).role("button").label(name.as_str())).h(26.0).px(6.0).radius(6.0).items_center()
            .hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| p(s, Some(next.clone())))
            .child(text(format!("{}{arrow}", label.to_uppercase())).font(t(11.0, 650.0).tracking(0.8)).color(if on { accent() } else { white(0.5) }).a11y_hidden());
        r = r.child(cell);
    }
    if let Some(_) = cur {
        let p = pick.clone();
        r = r.child(theme::focusable(row().key(format!("{key}-sort-clear")).role("button").label("Clear sort")).h(26.0).px(6.0).radius(6.0).items_center()
            .hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| p(s, None))
            .child(glyph(G::Close, 12.0, white(0.5))));
    }
    r
}

/// The collection dialogs and menus over the window (main::view): the playlist editor, a
/// description, the folder menu and folder dialogs, and the sort menus.
pub fn overlay(s: &App) -> Vec<El> {
    let mut out = vec![];
    if let Some(m) = s.nav.coll.ui.sort_menu {
        let layer = el().key("sort-layer").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .modal(false, |s: &mut App| s.nav.coll.ui.sort_menu = None)
            .on_press(|s: &mut App| s.nav.coll.ui.sort_menu = None).on_secondary_press(|s: &mut App, _, _| s.nav.coll.ui.sort_menu = None);
        let menu = match m {
            SortMenu::Sidebar => {
                let k = crate::library::lib_of(*s.nav.lib);
                sort_menu(k, &lib_sort(&s.nav.coll.ui, k), "lib-sort", |s, sort| crate::library::set_lib_sort(s, sort))
            }
            SortMenu::ViewAll => match &*s.nav.page {
                views::Page::Library(k, _) => sort_menu(*k, &crate::library::view_sort(s, *k), "lv-sort", |s, sort| crate::library::set_view_sort(s, sort)),
                _ => el(),
            },
        };
        out.push(layer.child(menu));
    }
    out.extend(crate::playlist::overlay(s));
    out.extend(crate::library::overlay(s));
    out
}

/// Escape closes the topmost collection layer.
pub fn on_escape(s: &mut App) -> bool {
    let c = &mut s.nav.coll;
    if c.folder_dialog.is_some() {
        c.folder_dialog = None;
    } else if c.folder_menu.is_some() {
        c.folder_menu = None;
    } else if c.edit.is_some() {
        c.edit = None;
    } else if c.description.is_some() {
        c.description = None;
    } else if c.ui.sort_menu.is_some() {
        c.ui.sort_menu = None;
    } else {
        return false;
    }
    true
}

type Later = Box<dyn FnOnce(&mut App) + Send>;
static LATER: std::sync::Mutex<Vec<Later>> = std::sync::Mutex::new(Vec::new());

/// Runs `f` on the window's state at the next poll (an async result closing a dialog, going
/// to a page), and has the window redraw.
pub fn later(f: impl FnOnce(&mut App) + Send + 'static) {
    LATER.lock().unwrap().push(Box::new(f));
    crate::data::bump_version();
}

/// Applies what async work left for the window (main::poll), and reads the saved sorts once.
pub fn poll(s: &mut App) {
    s.nav.coll.ensure_loaded();
    let due: Vec<Later> = std::mem::take(&mut *LATER.lock().unwrap());
    for f in due {
        f(s);
    }
    // A create-playlist dialog closed without creating leaves no folder waiting for it.
    if s.nav.create.is_none() && s.nav.coll.create_in.is_some() {
        s.nav.coll.create_in = None;
    }
}

/// A "N tracks"-style count with the right plural.
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// React's two-line ellipsis on long names in toasts.
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() > max + 2 { format!("{}…", s.chars().take(max).collect::<String>()) } else { s.to_string() }
}

pub fn dim(s: &str) -> El {
    text(s.to_string()).font(t(13.0, 500.0)).color(white(0.6))
}

/// A modal's text field (rename, create folder, playlist title).
pub fn field(edit: &TextEdit, key: &str, label: &str, get: fn(&mut App) -> &mut TextEdit, submit: fn(&mut App)) -> El {
    row().w_full().h(44.0).px(12.0).radius(10.0).items_center().fill(white(0.06))
        .child(rune::text_input(edit, t(14.0, 500.0).lh(20.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95))
            .caret(white(0.95), white(0.25)).placeholder(label, white(0.4))
            .on_edit(get).on_submit(move |s: &mut App, _| submit(s)))
}

pub fn error_line(e: &Option<String>) -> El {
    match e {
        Some(e) => text(e.clone()).font(t(13.0, 500.0)).color(theme::danger()),
        None => el(),
    }
}

/// Save, busy while saving, inert when it can't save.
pub fn save_button(key: &str, label: &str, can: bool, saving: bool, on: fn(&mut App)) -> El {
    let b = ui::pill(key, label, saving.then_some(G::Loader), ui::Pill::Primary).busy(saving);
    if can && !saving { b.on_press(on) } else { ui::unavailable(b).opacity(0.4) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_menu_flips_or_starts_fresh() {
        let cur: Sort = ("DATE_UPDATED".into(), "DESC".into());
        assert_eq!(next_sort(&cur, "DATE_UPDATED"), ("DATE_UPDATED".into(), "ASC".into()));
        assert_eq!(next_sort(&cur, "NAME"), ("NAME".into(), "ASC".into()));
        assert_eq!(next_sort(&cur, "DATE"), ("DATE".into(), "DESC".into()));
    }

    #[test]
    fn library_sorts_default_as_react() {
        let ui = Ui::default();
        assert_eq!(lib_sort(&ui, Lib::Playlists), ("DATE_UPDATED".into(), "DESC".into()));
        assert_eq!(lib_sort(&ui, Lib::Albums), ("DATE".into(), "DESC".into()));
        assert_eq!(sort_label(Lib::Albums, &("RELEASE_DATE".into(), "ASC".into())), "Release date");
    }
}
