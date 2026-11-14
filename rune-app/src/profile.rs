//! The signed-in user's profile (React ProfilePage, ProfilePlaylistsPage, ProfileEditModal,
//! SocialMediaPanel, SocialLinks): the hero with name, handle, fans and bio, Edit profile and
//! Share, the public playlists, the social links, and the editor.
//!
//! The page is a region (main::view): it reads the profile through data.rs (`Key::UserProfile`),
//! whose version is among its inputs. The editor is a dialog over the window.

use crate::App;
use crate::data::{self, Key};
use crate::glyph::{G, glyph};
use crate::theme;
use crate::ui::{self, t, white};
use crate::views::{self, Page, heading, open_url};
use rune::{El, TextEdit, Tx, col, el, row, spacer, text, text_input};
use serde_json::Value;
use sone_core::host::Manager;
use std::sync::Mutex;
use sone_core::AppState;
use sone_core::commands::profile as cmd;

fn st() -> sone_core::host::State<'static, AppState> {
    crate::host().state::<AppState>()
}

const BIO_MAX: usize = 5000;
const WEBSITE: &str = "OFFICIAL_HOMEPAGE";
/// The social link types TIDAL takes, in the order the editor lists them (socialLinks.ts).
const SOCIALS: [(&str, &str); 5] = [("INSTAGRAM", "Instagram"), ("TIKTOK", "TikTok"), ("FACEBOOK", "Facebook"), ("TWITTER", "X"), ("SNAPCHAT", "Snapchat")];

// ------------------------------------------------------------------ the profile's pieces

/// pickProfileHeroImage: the smallest picture at least 640 wide, else the widest, else the first.
pub fn hero_image(files: &[Value]) -> Option<String> {
    let href = |f: &Value| f["href"].as_str().map(str::to_string);
    let width = |f: &Value| f["width"].as_u64();
    let first = files.first()?;
    let sized: Vec<&Value> = files.iter().filter(|f| width(f).is_some()).collect();
    if sized.is_empty() {
        return href(first);
    }
    if let Some(f) = sized.iter().filter(|f| width(f).unwrap_or(0) >= 640).min_by_key(|f| width(f)) {
        return href(f);
    }
    sized.iter().max_by_key(|f| width(f)).and_then(|f| href(f))
}

/// pickProfileAvatarHref: the smallest square picture (the list is widest first), else the
/// smallest of any shape.
pub fn avatar_image(files: &[Value]) -> Option<String> {
    let squares: Vec<&Value> = files.iter().filter(|f| f["width"].as_u64().is_some() && f["width"] == f["height"]).collect();
    let pool: Vec<&Value> = if squares.is_empty() { files.iter().collect() } else { squares };
    pool.last().and_then(|f| f["href"].as_str().map(str::to_string))
}

/// The signed-in user's picture for the header's Account button and the account menu (React
/// currentUserAvatarAtom), once the profile has loaded: the controller fetches it with the rest
/// of the account at sign-in, as AppInitializer does. Reading it never fetches (React's menu
/// sends nothing).
pub fn avatar() -> Option<String> {
    data::peek(&Key::UserProfile).and_then(|p| avatar_image(p["pictureFiles"].as_array().map(Vec::as_slice).unwrap_or(&[])))
}

/// The header's Account button (UserMenu): a 32 pt round button showing the user's picture,
/// or the person placeholder until it loads.
pub fn account_button(avatar: Option<String>) -> El {
    let face = ui::with_image(
        el().size(32.0, 32.0).radius(16.0).fill(white(0.1)).clip_rounded(16.0).items_center().justify_center().child(glyph(G::User, 16.0, white(0.75))),
        avatar, 32.0, 16.0);
    theme::focusable(el().key("account").role("button").label("Account"))
        .size(36.0, 36.0).radius(18.0).items_center().justify_center()
        .hover(white(0.08)).press(white(0.14)).press_scale(0.92).cursor(rune::Cursor::Pointer)
        .help("Account")
        .child(face)
}

/// splitLinks: the website, and the social links by type.
fn split_links(links: &[Value]) -> (String, Vec<(String, String)>) {
    let mut website = String::new();
    let mut socials = Vec::new();
    for l in links {
        let (ty, href) = (l["linkType"].as_str().unwrap_or(""), l["href"].as_str().unwrap_or(""));
        if ty == WEBSITE {
            website = href.to_string();
        } else if SOCIALS.iter().any(|(t, _)| *t == ty) {
            socials.push((ty.to_string(), href.to_string()));
        }
    }
    (website, socials)
}

/// assembleExternalLinks: the website first, then the socials in their fixed order, blanks left out.
fn assemble_links(website: &str, socials: &[(String, String)]) -> Vec<sone_core::tidal_api::ExternalLink> {
    let mut out = Vec::new();
    if !website.trim().is_empty() {
        out.push(sone_core::tidal_api::ExternalLink { href: website.trim().to_string(), link_type: WEBSITE.to_string() });
    }
    for (ty, _) in SOCIALS {
        if let Some((_, v)) = socials.iter().find(|(t, _)| t == ty)
            && !v.trim().is_empty()
        {
            out.push(sone_core::tidal_api::ExternalLink { href: v.trim().to_string(), link_type: ty.to_string() });
        }
    }
    out
}

/// "1.2K fans": the compact count ProfilePage shows.
fn fans(n: u64) -> String {
    let compact = if n >= 1_000_000 {
        trim_one(n as f64 / 1_000_000.0) + "M"
    } else if n >= 1_000 {
        trim_one(n as f64 / 1_000.0) + "K"
    } else {
        n.to_string()
    };
    format!("{compact} fan{}", if n == 1 { "" } else { "s" })
}

fn trim_one(x: f64) -> String {
    let s = format!("{:.1}", (x * 10.0).floor() / 10.0);
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

// ------------------------------------------------------------------ the page

/// The profile, or what shows instead: Not signed in, loading, not found, or the error.
pub fn page(s: &App) -> El {
    if data::user_id() == 0 {
        return message("Not signed in", "Sign in to view your profile.", false);
    }
    let p = match data::want(Key::UserProfile) {
        data::Load::Ready(v) => v,
        data::Load::Loading => return views::loading(),
        data::Load::Failed(e) if e.contains("404") => return message("Page not found", "This profile doesn't exist.", true),
        data::Load::Failed(e) => return message("Couldn't load profile", &e, true),
    };
    let name = p["name"].as_str().unwrap_or("").to_string();
    let artist_id = p["artistId"].as_u64();
    let files = p["pictureFiles"].as_array().cloned().unwrap_or_default();
    let mut meta = vec![];
    if let Some(h) = p["handle"].as_str().filter(|h| !h.is_empty()) {
        meta.push(format!("@{h}"));
    }
    if let Some(n) = p["fanCount"].as_u64() {
        meta.push(fans(n));
    }
    let bio = p["bio"].as_str().filter(|b| !b.is_empty());
    let mut info = col().gap(12.0).min_w(0.0).max_w(820.0).child(heading(&name, 56.0).wrap().max_lines(2));
    if !meta.is_empty() {
        info = info.child(text(meta.join(" · ")).font(t(14.0, 700.0)).color(white(0.7)));
    }
    if let Some(b) = bio {
        info = info.child(col().max_w(820.0).child(crate::bio::view(b, 14.0, 0.8)));
    } else if artist_id.is_some() {
        // shouldShowAddBio: only on an editable profile with no bio.
        let pc = p.clone();
        info = info.child(row().child(theme::focusable(row().key("pf-add-bio").role("button").label("Add bio")).h(30.0).px(16.0).radius(15.0).items_center()
            .border(1.0, white(0.5)).hover(white(0.08)).cursor(rune::Cursor::Pointer).on_press(move |s: &mut App| open_editor(s, &pc))
            .child(text("Add bio").font(t(13.0, 600.0)).color(white(0.92)).a11y_hidden())));
    }
    let mut actions = row().gap(28.0).pt(8.0);
    if let Some(aid) = artist_id {
        let pc = p.clone();
        actions = actions
            .child(action("pf-edit", "Edit profile", G::Pencil).on_press(move |s: &mut App| open_editor(s, &pc)))
            .child(action("pf-share", "Share", G::Share).on_press_tx(move |_: &mut App, tx: &mut Tx| {
                // The artist/profile share URL works whether or not a @handle is set.
                tx.copy_rich(rune::ClipboardContent::new(format!("https://tidal.com/artist/{aid}/u")));
                crate::interact::toast("Copied share link to clipboard".into(), crate::interact::Kind::Success);
            }));
    }
    // ProfilePage's hero: the photo tiled across the banner (the middle one whole, the rest at
    // 40%), fading into the page at the bottom, lightly at the top and most on the left, where
    // the name sits. (React also frosts the bottom with a blurred copy of the tiles; the fade
    // stands in for it.)
    let banner_h = 480.0;
    let mut hero = col().key("pf-hero").w_full().h(banner_h).radius(16.0).clip_rounded(16.0).fill(white(0.03)).justify_end();
    if let Some(url) = hero_image(&files) {
        let mut tiles = row().absolute(0.0, 0.0).w_full().h(banner_h).justify_center().a11y_hidden();
        for i in 0..7 {
            tiles = tiles.child(ui::with_image(el().size(banner_h, banner_h).opacity(if i == 3 { 1.0 } else { 0.4 }), Some(url.clone()), banner_h, 0.0));
        }
        let g = theme::ground();
        hero = hero.child(tiles)
            .child(el().absolute(0.0, 0.0).w_full().h(banner_h).linear_gradient(std::f32::consts::PI, [(0.0, g.alpha(0.5)), (0.5, g.alpha(0.0)), (1.0, g)]).a11y_hidden())
            .child(el().absolute(0.0, 0.0).w_full().h(banner_h).linear_gradient(std::f32::consts::FRAC_PI_2, [(0.0, g.alpha(0.85)), (0.5, g.alpha(0.0)), (1.0, g.alpha(0.55))]).a11y_hidden());
    }
    hero = hero.child(col().w_full().padding(28.0).gap(6.0).child(info).child(actions));
    let mut page = col().w_full().gap(28.0).child(hero);
    let playlists = p["publicPlaylists"].as_array().cloned().unwrap_or_default();
    if !playlists.is_empty() {
        // PlaylistsSection: every public playlist in a sideways-scrolling row, whole cards
        // across the width, with arrows that move it a page at a time.
        let w = (*s.nav.content_w).max(320.0);
        let cols = ((w + CARD_GAP) / (MIN_CARD + CARD_GAP)).floor().clamp(2.0, 8.0);
        let cw = ((w - (cols - 1.0) * CARD_GAP) / cols).floor();
        let mut cards = row().key(CAROUSEL).w_full().gap(CARD_GAP).items_start().scroll_x().on_scroll_offset(note_carousel);
        for (i, pl) in playlists.iter().enumerate() {
            cards = cards.child(playlist_card(&format!("pf-pl-{i}"), pl, &name, cw));
        }
        let (back, fwd) = carousel_ways();
        let arrow = |key: &'static str, label: &'static str, g: G, dir: f32, can: bool| {
            let b = theme::focusable(el().key(key).role("button").label(label)).size(32.0, 32.0).radius(16.0).items_center().justify_center();
            let b = if can {
                b.fill(white(0.08)).hover(white(0.14)).press_scale(0.92).cursor(rune::Cursor::Pointer).on_press_tx(move |_: &mut App, tx: &mut Tx| {
                    if let Some(p) = *CAROUSEL_AT.lock().unwrap() {
                        tx.scroll_to(CAROUSEL, (p.offset[0] + dir * (p.viewport[0] + CARD_GAP)).clamp(0.0, p.max[0].max(0.0)), 0.0);
                    }
                })
            } else {
                b.disabled(true)
            };
            b.child(glyph(g, 18.0, if can { white(0.9) } else { white(0.3) }))
        };
        page = page.child(col().w_full().gap(14.0)
            .child(row().w_full().gap(8.0).items_center().child(heading("Public playlists", theme::SECTION)).child(spacer())
                .child(arrow("pf-pl-prev", "Previous playlists", G::Back, -1.0, back))
                .child(arrow("pf-pl-next", "Next playlists", G::Forward, 1.0, fwd))
                .child(theme::focusable(row().key("pf-view-all").role("button").label("View all")).h(30.0).px(12.0).radius(15.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
                    .on_press(|s: &mut App| views::go(s, Page::ProfilePlaylists))
                    .child(text("View all").font(t(13.0, 700.0)).color(white(0.6)).a11y_hidden())))
            .child(cards));
    }
    let (_, socials) = split_links(p["externalLinks"].as_array().map(Vec::as_slice).unwrap_or(&[]));
    if !socials.is_empty() {
        let mut list = col().gap(10.0);
        for (ty, _) in SOCIALS {
            if let Some((_, href)) = socials.iter().find(|(t, _)| t == ty) {
                let label = SOCIALS.iter().find(|(t, _)| *t == ty).map(|(_, l)| *l).unwrap_or(ty);
                let url = href.clone();
                list = list.child(theme::focusable(row().key(format!("pf-social-{ty}")).role("link").label(label)).gap(10.0).items_center().cursor(rune::Cursor::Pointer).help(href.clone())
                    .on_press(move |_: &mut App| open_url(&url))
                    .child(crate::brand::social(ty).map_or_else(|| glyph(G::Link, 18.0, white(0.7)), |i| crate::brand::mark(i, 18.0, white(0.7))))
                    .child(text(label.to_lowercase()).font(t(14.0, 600.0)).color(white(0.8)).a11y_hidden()));
            }
        }
        page = page.child(col().gap(14.0).child(heading("Social", theme::SECTION)).child(list));
    }
    page
}

const CAROUSEL: &str = "pf-playlists";
const CARD_GAP: f32 = 16.0;
const MIN_CARD: f32 = 170.0;
/// Where the public playlists' row is scrolled, as its last layout reported.
static CAROUSEL_AT: Mutex<Option<rune::ScrollPos>> = Mutex::new(None);

fn ways(p: &rune::ScrollPos) -> (bool, bool) {
    (p.offset[0] > 10.0, p.offset[0] < p.max[0] - 10.0)
}

/// Which ways the row can move (React starts with right enabled, left not).
fn carousel_ways() -> (bool, bool) {
    CAROUSEL_AT.lock().unwrap().as_ref().map(ways).unwrap_or((false, true))
}

/// Changes whenever the row's arrows should change: an input of the page region only, so
/// the sidebar and the drawer don't rebuild for it.
static ARROWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn version() -> u64 {
    ARROWS.load(std::sync::atomic::Ordering::Relaxed)
}

/// After Log out: the next account's row starts at its beginning.
pub fn reset_after_logout() {
    if CAROUSEL_AT.lock().unwrap().take().is_some() {
        ARROWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The row scrolled or laid out: the page builds again only when an arrow's state changes.
fn note_carousel(p: rune::ScrollPos) {
    let was = CAROUSEL_AT.lock().unwrap().replace(p);
    if was.as_ref().map(ways) != Some(ways(&p)) {
        ARROWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// A header action: a 22 pt icon over its caption (ProfilePage HeaderAction).
fn action(key: &str, label: &str, g: G) -> El {
    theme::focusable(col().key(key.to_string()).role("button").label(label)).help(label).gap(6.0).items_center().padding(4.0).radius(8.0).hover(white(0.06)).cursor(rune::Cursor::Pointer)
        .child(crate::glyph::glyph(g, 22.0, white(0.8)))
        .child(text(label.to_string()).font(t(11.0, 600.0)).color(white(0.7)).a11y_hidden())
}

fn message(title: &str, body: &str, back: bool) -> El {
    let mut c = col().w_full().pt(80.0).gap(12.0).items_center()
        .child(crate::glyph::glyph(G::User, 48.0, white(0.3)))
        .child(heading(title, 18.0))
        .child(text(body.to_string()).font(t(13.0, 400.0)).color(white(0.55)).wrap().max_w(440.0));
    if back {
        c = c.child(ui::pill("pf-back", "Go back", None, ui::Pill::Primary).on_press(|s: &mut App| {
            if let Some(p) = s.nav.back.pop() {
                *s.nav.page = p;
            } else {
                views::go(s, Page::Home);
            }
        }));
    }
    c
}

/// A public playlist's card: it opens the playlist (no play button, as in React).
fn playlist_card(key: &str, pl: &Value, owner: &str, w: f32) -> El {
    let id = pl["id"].as_str().unwrap_or("").to_string();
    let title = pl["title"].as_str().unwrap_or("");
    let n = pl["numberOfTracks"].as_u64();
    let seed = id.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    let sub = match n { Some(n) => format!("{owner} · {n} tracks"), None => owner.to_string() };
    views::card_w(w, key.to_string(), title, &sub, seed, false, pl["coverUrl"].as_str().map(str::to_string), None, Page::Playlist(id))
}

/// The full grid of public playlists (ProfilePlaylistsPage): one page, as the profile returned it.
pub fn playlists_page(_s: &App) -> El {
    let p = match views::ready_or(Key::UserProfile) { Ok(v) => v, Err(e) => return e };
    let name = p["name"].as_str().unwrap_or("").to_string();
    let playlists = p["publicPlaylists"].as_array().cloned().unwrap_or_default();
    let count = playlists.len();
    let mut grid = row().w_full().gap(8.0).flex_wrap();
    for (i, pl) in playlists.iter().enumerate() {
        grid = grid.child(playlist_card(&format!("pfa-pl-{i}"), pl, &name, 160.0));
    }
    col().w_full().gap(16.0)
        .child(col().gap(4.0).child(heading("Public playlists", theme::PAGE_TITLE))
            .child(text(format!("{count} {}", if count == 1 { "playlist" } else { "playlists" })).font(t(14.0, 500.0)).color(white(0.55))))
        .child(if count == 0 { ui::empty("No public playlists") } else { grid })
}

// ------------------------------------------------------------------ the editor

/// ProfileEditModal's fields, as they were opened and as edited.
pub struct Edit {
    artist_id: Option<u64>,
    orig: Value,
    name: TextEdit,
    handle: TextEdit,
    website: TextEdit,
    bio: TextEdit,
    socials: Vec<TextEdit>,
    social_panel: bool,
    saving: bool,
    avatar: Option<String>,
    /// The chosen picture being framed (AvatarCropper), over the editor.
    crop: Option<Crop>,
}

impl Edit {
    /// Closes the cropper; whether it was open (Escape's innermost layer).
    pub fn close_crop(&mut self) -> bool {
        self.crop.take().is_some()
    }
}

/// AvatarCropper: the picture under a square frame, dragged to place it and zoomed 1–3×.
pub struct Crop {
    img: std::sync::Arc<image::RgbImage>,
    /// What the frame shows: the picture, at most 1024 px on a side.
    px: rune::Pixels,
    zoom: f32,
    /// The picture's offset from centred, in frame points.
    off: (f32, f32),
    grab: Option<(f32, f32)>,
    busy: bool,
}

/// The crop frame's side in points.
const FRAME: f32 = 440.0;

impl Crop {
    fn size(&self) -> (f32, f32) {
        (self.img.width() as f32, self.img.height() as f32)
    }
    /// Frame points per picture pixel: at zoom 1 the short side fills the frame.
    fn scale(&self) -> f32 {
        let (w, h) = self.size();
        self.zoom * FRAME / w.min(h)
    }
    /// `off` kept so the picture always covers the frame.
    fn clamped(&self, off: (f32, f32)) -> (f32, f32) {
        let (w, h) = self.size();
        let s = self.scale();
        let (mx, my) = (((w * s - FRAME) / 2.0).max(0.0), ((h * s - FRAME) / 2.0).max(0.0));
        (off.0.clamp(-mx, mx), off.1.clamp(-my, my))
    }
    /// The framed square in picture pixels: (x, y, side).
    fn area(&self) -> (u32, u32, u32) {
        let (w, h) = self.size();
        let s = self.scale();
        let (ox, oy) = ((FRAME - w * s) / 2.0 + self.off.0, (FRAME - h * s) / 2.0 + self.off.1);
        let side = (FRAME / s).round().max(1.0).min(w.min(h));
        let x = (-ox / s).round().clamp(0.0, w - side);
        let y = (-oy / s).round().clamp(0.0, h - side);
        (x as u32, y as u32, side as u32)
    }
}

fn open_editor(s: &mut App, p: &Value) {
    let (website, socials) = split_links(p["externalLinks"].as_array().map(Vec::as_slice).unwrap_or(&[]));
    let files = p["pictureFiles"].as_array().cloned().unwrap_or_default();
    s.nav.set.profile_edit = Some(Edit {
        artist_id: p["artistId"].as_u64(),
        orig: p.clone(),
        name: TextEdit::new(p["name"].as_str().unwrap_or("")),
        handle: TextEdit::new(p["handle"].as_str().unwrap_or("")),
        website: TextEdit::new(&website),
        bio: TextEdit::new(p["bio"].as_str().unwrap_or("")),
        socials: SOCIALS.iter().map(|(ty, _)| TextEdit::new(socials.iter().find(|(t, _)| t == ty).map(|(_, v)| v.as_str()).unwrap_or(""))).collect(),
        social_panel: false,
        saving: false,
        avatar: hero_image(&files),
        crop: None,
    });
}

type Done = Box<dyn FnOnce(&mut App) + Send>;
static DONE: Mutex<Vec<Done>> = Mutex::new(Vec::new());

/// Results of the editor's commands, applied by the poll (settings::flush_settings calls it).
pub fn apply_results(s: &mut App) {
    let done: Vec<Done> = std::mem::take(&mut *DONE.lock().unwrap());
    for d in done {
        d(s);
    }
}

fn later(fut: impl std::future::Future<Output = Result<(), String>> + Send + 'static, then: impl FnOnce(&mut App, Result<(), String>) + Send + 'static) {
    sone_core::host::async_runtime::spawn(async move {
        let r = fut.await;
        DONE.lock().unwrap().push(Box::new(move |s: &mut App| then(s, r)));
        crate::controller::bump();
    });
}

/// After an edit: the profile loads again (and the account menu's name and picture with it).
fn reload() {
    data::forget(&Key::UserProfile);
    data::forget(&Key::Profile);
}

fn toast_err(e: &str, fallback: &str) {
    let msg = if e.trim().is_empty() { fallback.to_string() } else { e.trim().to_string() };
    crate::interact::toast(msg, crate::interact::Kind::Error);
}

/// handleSave: name and handle (a dry run, then for real), the bio, then the links.
fn save(s: &mut App) {
    let Some(e) = &mut s.nav.set.profile_edit else { return };
    let Some(aid) = e.artist_id else { return };
    if e.saving {
        return;
    }
    e.saving = true;
    let name = e.name.text.trim().to_string();
    let handle = e.handle.text.trim().to_string();
    let name_changed = name != e.orig["name"].as_str().unwrap_or("");
    let handle_changed = handle != e.orig["handle"].as_str().unwrap_or("");
    let bio = e.bio.text.clone();
    let bio_changed = bio != e.orig["bio"].as_str().unwrap_or("");
    let socials: Vec<(String, String)> = SOCIALS.iter().zip(&e.socials).map(|((ty, _), v)| (ty.to_string(), v.text.clone())).collect();
    let links = assemble_links(&e.website.text, &socials);
    later(
        async move {
            let m = |n: bool, h: bool| (n.then(|| name.clone()), h.then(|| handle.clone()));
            if name_changed || handle_changed {
                let (n, h) = m(name_changed, handle_changed);
                cmd::update_profile_meta(st(), aid, n.clone(), h.clone(), true).await.map_err(|e| e.to_string())?;
                cmd::update_profile_meta(st(), aid, n, h, false).await.map_err(|e| e.to_string())?;
            }
            if bio_changed {
                // The biography is keyed by the artist id: PATCH /artistBiographies/{artistId}
                // creates or updates it.
                cmd::update_profile_bio(st(), aid.to_string(), bio).await.map_err(|e| e.to_string())?;
            }
            cmd::update_profile_links(st(), aid, links).await.map_err(|e| e.to_string())
        },
        |s, r| {
            if let Some(e) = &mut s.nav.set.profile_edit {
                e.saving = false;
            }
            match r {
                Ok(()) => {
                    crate::interact::toast("Profile updated".into(), crate::interact::Kind::Success);
                    s.nav.set.profile_edit = None;
                    reload();
                }
                Err(e) => toast_err(&e, "Failed to save profile"),
            }
        },
    );
}

fn delete_picture(s: &mut App) {
    let Some(aid) = s.nav.set.profile_edit.as_ref().and_then(|e| e.artist_id) else { return };
    later(async move { cmd::delete_profile_picture(st(), aid).await.map_err(|e| e.to_string()) }, |s, r| match r {
        Ok(()) => {
            crate::interact::toast("Profile picture removed".into(), crate::interact::Kind::Info);
            if let Some(e) = &mut s.nav.set.profile_edit {
                e.avatar = None;
            }
            reload();
        }
        Err(e) => toast_err(&e, "Failed to remove picture"),
    });
}

fn labelled_field(label: &str, input: El) -> El {
    col().w_full().gap(4.0)
        .child(text(label.to_string()).font(t(11.0, 600.0)).color(white(0.65)).a11y_hidden())
        .child(row().w_full().min_h(38.0).px(12.0).py(8.0).radius(8.0).fill(white(0.07)).items_center().child(input))
}

fn edit_input(edit: &TextEdit, key: &str, label: &str, get: fn(&mut App) -> &mut TextEdit) -> El {
    text_input(edit, t(14.0, 500.0).lh(19.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95)).caret(white(0.95), white(0.25))
        .placeholder(label, white(0.35)).on_edit(get)
}

macro_rules! edit_field {
    ($field:ident) => {
        |s: &mut App| &mut s.nav.set.profile_edit.as_mut().expect("editor open").$field
    };
}

/// The editor, with the cropper over it while a picture is being framed.
pub fn edit_dialog(s: &App) -> Option<El> {
    let ed = editor_dialog(s)?;
    match s.nav.set.profile_edit.as_ref().and_then(|e| e.crop.as_ref()) {
        Some(c) => Some(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(ed).child(crop_dialog(c))),
        None => Some(ed),
    }
}

/// The editor (ProfileEditModal), or its social media panel (SocialMediaPanel), when open.
fn editor_dialog(s: &App) -> Option<El> {
    let e = s.nav.set.profile_edit.as_ref()?;
    let close = |s: &mut App| s.nav.set.profile_edit = None;
    if e.social_panel {
        let mut body = col().w_full().gap(12.0).child(text("Your public profile will show links to social media accounts you add here.").font(t(12.0, 400.0)).color(white(0.6)).wrap());
        for (i, (ty, label)) in SOCIALS.iter().enumerate() {
            let input = text_input(&e.socials[i], t(14.0, 500.0).lh(19.0)).key(format!("pf-social-in-{ty}")).label(*label).single_line().grow(1.0).color(white(0.95)).caret(white(0.95), white(0.25))
                .placeholder(*label, white(0.35)).on_edit(move |s: &mut App| &mut s.nav.set.profile_edit.as_mut().expect("editor open").socials[i]);
            body = body.child(labelled_field(label, input));
        }
        let head = row().gap(8.0).items_center()
            .child(ui::icon_button::<App>("pf-social-back", "Back", G::Back, 18.0, white(0.7)).on_press(|s: &mut App| {
                if let Some(e) = &mut s.nav.set.profile_edit {
                    e.social_panel = false;
                }
            }))
            .child(ui::dialog_title("Social media"));
        return Some(ui::dialog_frame("profile-edit-layer", "profile-social", "Social media", ("pf-close", "Close"), 440.0, None, Some(head), body, close));
    }
    let editable = e.artist_id.is_some();
    let avatar = ui::with_image(el().size(64.0, 64.0).radius(32.0).fill(white(0.08)).clip_rounded(32.0), e.avatar.clone(), 64.0, 32.0);
    let bio_input = text_input(&e.bio, t(14.0, 500.0).lh(19.0)).key("pf-bio").label("Bio").w_full().min_h(80.0).color(white(0.95)).caret(white(0.95), white(0.25))
        .placeholder("Bio", white(0.35)).on_edit(edit_field!(bio))
        .on_input_change(|s: &mut App, _| {
            // maxLength
            if let Some(e) = &mut s.nav.set.profile_edit
                && e.bio.text.chars().count() > BIO_MAX
            {
                let cut: String = e.bio.text.chars().take(BIO_MAX).collect();
                e.bio = TextEdit::new(&cut);
            }
        });
    let remaining = BIO_MAX.saturating_sub(e.bio.text.chars().count());
    let link = |key: &str, label: &str, danger: bool| theme::focusable(row().key(key.to_string()).role("button").label(label)).cursor(rune::Cursor::Pointer)
        .child(text(label.to_string()).font(t(13.0, 500.0)).color(if danger { theme::danger() } else { theme::accent() }).a11y_hidden());
    let mut body = col().w_full().gap(16.0)
        .child(text("Information you add to your profile will be visible to everyone on and off TIDAL.").font(t(12.0, 400.0)).color(white(0.6)).wrap())
        .child(row().gap(16.0).items_center().child(avatar)
            .child(col().gap(4.0).child(text("Profile picture").font(t(13.0, 650.0)).color(white(0.92)))
                .child(row().gap(8.0).items_center()
                    .child(link("pf-pic-choose", "Choose profile picture", false).on_press_tx(choose_picture))
                    .child(text("·").font(t(13.0, 500.0)).color(white(0.4)))
                    .child(link("pf-pic-delete", "Delete", true).on_press(delete_picture)))))
        .child(labelled_field("Name", edit_input(&e.name, "pf-name", "Name", edit_field!(name))))
        .child(col().w_full().gap(4.0)
            .child(labelled_field("Username", edit_input(&e.handle, "pf-handle", "Username", edit_field!(handle))))
            .child(text("Use only the letters a-z, numbers 0-9 and underscores.").font(t(11.0, 400.0)).color(white(0.45))))
        .child(labelled_field("Link", edit_input(&e.website, "pf-link", "Link", edit_field!(website))))
        .child(col().w_full().gap(4.0)
            .child(labelled_field("Bio", if editable { bio_input } else { text(e.bio.text.clone()).font(t(14.0, 500.0)).color(white(0.5)) }))
            .child(text(if editable { format!("{remaining} characters remaining") } else { "Bio editing is unavailable for this profile.".to_string() }).font(t(11.0, 400.0)).color(white(0.45))))
        .child(theme::focusable_row(row().key("pf-socials").role("button").label("Social media")).w_full().h(44.0).px(12.0).radius(8.0).fill(white(0.07)).hover(white(0.1)).items_center().cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| {
                if let Some(e) = &mut s.nav.set.profile_edit {
                    e.social_panel = true;
                }
            })
            .child(text("Social media").font(t(14.0, 500.0)).color(white(0.92)).a11y_hidden()).child(spacer())
            .child(crate::glyph::glyph(G::ChevronRight, 18.0, white(0.5))));
    let save_btn = ui::pill("pf-save", if e.saving { "Saving…" } else { "Save" }, None, ui::Pill::Primary).opacity(if editable && !e.saving { 1.0 } else { 0.4 }).on_press(save);
    body = body.child(row().w_full().child(spacer()).child(save_btn));
    Some(ui::dialog_frame("profile-edit-layer", "profile-edit", "Edit profile", ("pf-close", "Close"), 460.0, None, Some(ui::dialog_title("Edit profile")), body, close))
}

/// "Choose profile picture": a file chooser, then the cropper (AvatarCropper) frames it.
fn choose_picture(s: &mut App, tx: &mut Tx) {
    if s.nav.set.profile_edit.as_ref().and_then(|e| e.artist_id).is_none() {
        return;
    }
    let options = rune::FileDialogOptions { title: "Choose profile picture".into(), ..Default::default() }.filter("Images", &["jpg", "jpeg", "png"]);
    tx.open_file(options, move |_: &mut App, result, _| match result {
        Ok(Some(path)) => {
            sone_core::host::async_runtime::spawn(async move {
                let loaded = tokio::task::spawn_blocking(move || load_for_crop(&path)).await.map_err(|e| e.to_string()).and_then(|r| r);
                DONE.lock().unwrap().push(Box::new(move |s: &mut App| match loaded {
                    Ok((img, px)) => {
                        if let Some(e) = &mut s.nav.set.profile_edit {
                            e.crop = Some(Crop { img: std::sync::Arc::new(img), px, zoom: 1.0, off: (0.0, 0.0), grab: None, busy: false });
                        }
                    }
                    Err(e) => toast_err(&e, "Couldn't read that picture"),
                }));
                crate::controller::bump();
            });
        }
        Ok(None) => {}
        Err(e) => toast_err(&e.to_string(), "Couldn't open a file chooser"),
    });
}

/// The picture at `path` decoded, and its preview for the frame (at most 1024 px a side).
fn load_for_crop(path: &std::path::Path) -> Result<(image::RgbImage, rune::Pixels), String> {
    let img = image::open(path).map_err(|e| format!("Couldn't read that picture: {e}"))?;
    let preview = if img.width().max(img.height()) > 1024 { img.thumbnail(1024, 1024) } else { img.clone() }.to_rgba8();
    let px = rune::Pixels::new();
    px.set(preview.width(), preview.height(), preview.into_raw());
    Ok((img.to_rgb8(), px))
}

/// getCroppedBlob: the framed square of `img`, at most 1280 px (clampOutputSize), as base64
/// JPEG.
fn crop_jpeg(img: &image::RgbImage, (x, y, side): (u32, u32, u32)) -> Result<String, String> {
    let out_side = side.min(1280);
    let square = image::imageops::crop_imm(img, x, y, side, side).to_image();
    let square = if out_side == side { square } else { image::imageops::resize(&square, out_side, out_side, image::imageops::FilterType::Lanczos3) };
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92).encode_image(&square).map_err(|e| e.to_string())?;
    Ok(base64_lite::encode(out.get_ref()))
}

/// Set photo: the framed square goes up as the profile picture.
fn set_photo(s: &mut App) {
    let Some(e) = &mut s.nav.set.profile_edit else { return };
    let (Some(aid), Some(c)) = (e.artist_id, e.crop.as_mut()) else { return };
    if c.busy {
        return;
    }
    c.busy = true;
    let (img, area) = (c.img.clone(), c.area());
    e.saving = true;
    later(
        async move {
            let b64 = tokio::task::spawn_blocking(move || crop_jpeg(&img, area)).await.map_err(|e| e.to_string())??;
            cmd::upload_profile_picture(st(), aid, b64).await.map_err(|e| e.to_string())
        },
        |s, r| {
            if let Some(e) = &mut s.nav.set.profile_edit {
                e.saving = false;
                e.crop = None;
            }
            match r {
                Ok(()) => {
                    crate::interact::toast("Picture uploaded — it may take a moment to appear".into(), crate::interact::Kind::Success);
                    reload();
                }
                Err(e) => toast_err(&e, "Failed to upload picture"),
            }
        },
    );
}

fn crop_of(s: &mut App) -> Option<&mut Crop> {
    s.nav.set.profile_edit.as_mut().and_then(|e| e.crop.as_mut())
}

/// The cropper dialog (AvatarCropper), over the editor.
fn crop_dialog(c: &Crop) -> El {
    let (w, h) = c.size();
    let sc = c.scale();
    let (iw, ih) = (w * sc, h * sc);
    let (x, y) = ((FRAME - iw) / 2.0 + c.off.0, (FRAME - ih) / 2.0 + c.off.1);
    let frame = el().key("crop-frame").size(FRAME, FRAME).radius(8.0).clip_rounded(8.0).fill(theme::black(1.0)).cursor(rune::Cursor::Move)
        .child(el().absolute(x, y).size(iw, ih).pixels_fit(&c.px, rune::SurfaceFit::Fill).a11y_hidden())
        .child(el().absolute(0.0, 0.0).size(FRAME, FRAME).radius(8.0).border(2.0, rune::Color::WHITE.alpha(0.85)).a11y_hidden())
        .on_drag(|s: &mut App, d: &mut rune::Drag, tx: &mut Tx| {
            let Some(c) = crop_of(s) else { return };
            if matches!(d.phase, rune::Phase::Up) {
                c.grab = None;
                return;
            }
            let start = *c.grab.get_or_insert(c.off);
            c.off = c.clamped((start.0 + d.dx, start.1 + d.dy));
            tx.instant();
        });
    let frac = (c.zoom - 1.0) / 2.0;
    let zoom = col().key("crop-zoom").role("slider").label("Zoom").range_value(c.zoom as f64, 1.0, 3.0).focusable(true).focus_ring(2.0, 2.0, white(0.6)).grow(1.0).h(24.0).justify_center().cursor(rune::Cursor::Pointer)
        .on_step(|s: &mut App, dir, _| {
            if let Some(c) = crop_of(s) {
                c.zoom = (c.zoom + dir as f32 * 0.1).clamp(1.0, 3.0);
                c.off = c.clamped(c.off);
            }
        })
        .on_drag(|s: &mut App, d: &mut rune::Drag, tx: &mut Tx| {
            if let Some(c) = crop_of(s) {
                c.zoom = 1.0 + 2.0 * (d.x / d.w).clamp(0.0, 1.0);
                c.off = c.clamped(c.off);
                tx.instant();
            }
        })
        .child(el().w_full().h(4.0).radius(2.0).fill(white(0.18)).child(el().absolute(0.0, 0.0).h(4.0).w_frac(frac).radius(2.0).fill(theme::accent())));
    let busy = c.busy;
    let body = col().w_full().gap(14.0)
        .child(text("TIDAL requires a square photo. Drag to reposition, then zoom to frame it.").font(t(12.0, 400.0)).color(white(0.6)).wrap())
        .child(frame)
        .child(row().w_full().gap(12.0).items_center().child(glyph(G::Search, 16.0, white(0.55))).child(zoom))
        .child(row().w_full().gap(8.0).child(spacer())
            .child(ui::pill("crop-cancel", "Cancel", None, ui::Pill::Secondary).on_press(|s: &mut App| {
                if let Some(e) = &mut s.nav.set.profile_edit {
                    e.crop = None;
                }
            }))
            .child(ui::pill("crop-set", if busy { "Working…" } else { "Set photo" }, None, ui::Pill::Primary).busy(busy).on_press(set_photo)));
    ui::dialog_frame("crop-layer", "crop-dialog", "Crop photo", ("crop-close", "Close"), FRAME + 48.0, None, Some(ui::dialog_title("Crop photo")), body, |s: &mut App| {
        if let Some(e) = &mut s.nav.set.profile_edit {
            e.crop = None;
        }
    })
}

/// Standard base64, for the upload (the backend decodes it).
mod base64_lite {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
            for i in 0..4 {
                if i <= chunk.len() {
                    s.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hero_and_avatar_pictures() {
        let files = vec![
            json!({"href": "w1280", "width": 1280, "height": 720}),
            json!({"href": "w640", "width": 640, "height": 640}),
            json!({"href": "w320", "width": 320, "height": 320}),
        ];
        assert_eq!(hero_image(&files).as_deref(), Some("w640"));
        assert_eq!(avatar_image(&files).as_deref(), Some("w320"));
        let small = vec![json!({"href": "a", "width": 300}), json!({"href": "b", "width": 500})];
        assert_eq!(hero_image(&small).as_deref(), Some("b"));
        assert_eq!(hero_image(&[json!({"href": "x"})]).as_deref(), Some("x"));
        assert_eq!(hero_image(&[]), None);
    }

    #[test]
    fn links_split_and_assemble() {
        let links = vec![json!({"href": "https://me.example", "linkType": "OFFICIAL_HOMEPAGE"}), json!({"href": "@me", "linkType": "TWITTER"}), json!({"href": "x", "linkType": "OTHER"})];
        let (web, socials) = split_links(&links);
        assert_eq!(web, "https://me.example");
        assert_eq!(socials, vec![("TWITTER".to_string(), "@me".to_string())]);
        let out = assemble_links(" https://me.example ", &[("TWITTER".into(), "@me".into()), ("INSTAGRAM".into(), "  ".into())]);
        let pairs: Vec<(String, String)> = out.into_iter().map(|l| (l.link_type, l.href)).collect();
        assert_eq!(pairs, vec![("OFFICIAL_HOMEPAGE".into(), "https://me.example".into()), ("TWITTER".into(), "@me".into())]);
    }

    #[test]
    fn fan_counts_are_compact() {
        assert_eq!(fans(1), "1 fan");
        assert_eq!(fans(999), "999 fans");
        assert_eq!(fans(1250), "1.2K fans");
        assert_eq!(fans(2_000_000), "2M fans");
    }

    fn crop(w: u32, h: u32) -> Crop {
        Crop { img: std::sync::Arc::new(image::RgbImage::from_pixel(w, h, image::Rgb([200, 40, 40]))), px: rune::Pixels::new(), zoom: 1.0, off: (0.0, 0.0), grab: None, busy: false }
    }

    #[test]
    fn the_frame_starts_on_the_centred_square_and_follows_pan_and_zoom() {
        let mut c = crop(200, 100);
        assert_eq!(c.area(), (50, 0, 100));
        // dragged right as far as it goes: the picture's left edge meets the frame's
        c.off = c.clamped((10_000.0, 0.0));
        assert_eq!(c.area(), (0, 0, 100));
        // zoomed 2x about the centre: half the side
        c.off = (0.0, 0.0);
        c.zoom = 2.0;
        assert_eq!(c.area(), (75, 25, 50));
        // a vertical drag on a picture that only just fills the frame at zoom 1 can't move it
        let mut d = crop(100, 100);
        assert_eq!(d.clamped((0.0, 50.0)), (0.0, 0.0));
        d.zoom = 3.0;
        assert!(d.clamped((0.0, 1000.0)).1 > 0.0);
    }

    #[test]
    fn the_framed_square_becomes_a_jpeg() {
        let img = image::RgbImage::from_pixel(300, 200, image::Rgb([200, 40, 40]));
        let b64 = crop_jpeg(&img, (50, 0, 200)).unwrap();
        // base64 of a JPEG's SOI marker (FF D8 FF)
        assert!(b64.starts_with("/9j/"), "{}", &b64[..8]);
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64_lite::encode(b""), "");
        assert_eq!(base64_lite::encode(b"f"), "Zg==");
        assert_eq!(base64_lite::encode(b"fo"), "Zm8=");
        assert_eq!(base64_lite::encode(b"foo"), "Zm9v");
        assert_eq!(base64_lite::encode(b"foobar"), "Zm9vYmFy");
    }
}
