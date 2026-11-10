//! Design tokens: the colours, radii and type sizes the views share, and the one surface every
//! floating layer is drawn on. Plain values on purpose: there is one (dark) theme.
//!
//! Text and fills on the dark ground are `ui::white(alpha)`; `TEXT` and `TEXT_MUTED` below are
//! the two text levels new code should pick from.

use crate::ui::white;
use rune::{Color, El, oklch, oklcha};

/// The hue of the app's tinted darks.
const HUE: f32 = 270.0;

// ---- the ambient tint
//
// Every panel (sidebar, page, drawer, player bar, the floating layers) is one hue at one low
// chroma, differing only in lightness, so the window reads as a single surface. The hue follows
// what's playing (`ui::hue` of the track, which also tints its placeholder art) and is the app's
// own violet with nothing playing; `main::view` sets it once per frame, before building.

/// The chroma every surface shares: enough to read as a hue, not as a colour.
const TINT: f32 = 0.025;

thread_local! {
    static AMBIENT: std::cell::Cell<f32> = const { std::cell::Cell::new(HUE) };
    /// Backdrop blurs [`overlay_surface`] may still hand out this frame (see [`take_glass`]).
    static GLASS_LEFT: std::cell::Cell<usize> = const { std::cell::Cell::new(rune::MAX_BACKDROP_BLURS) };
}

/// Starts a window's frame: sets the hue the surfaces below are drawn in and gives the frame
/// its full budget of glass surfaces. Every window's view calls it first, so neither the hue
/// nor the budget carries over from another window's frame.
pub fn set_ambient(hue: f32) {
    AMBIENT.with(|a| a.set(hue));
    GLASS_LEFT.with(|g| g.set(rune::MAX_BACKDROP_BLURS));
}

/// A surface at lightness `l` in hue `h`.
pub fn surface(l: f32, h: f32) -> Color {
    oklch(l, TINT, h)
}

fn ambient(l: f32) -> Color {
    surface(l, AMBIENT.with(|a| a.get()))
}

// ---- surfaces, darkest to lightest

/// The library sidebar.
pub fn sidebar_bg() -> Color {
    ambient(0.13)
}
/// The right-hand drawer.
pub fn drawer_bg() -> Color {
    ambient(0.14)
}
/// The window ground the page sits on (`ui::base` in the ambient hue).
pub fn ground() -> Color {
    ambient(0.155)
}
/// The player bar, lighter at its leading edge.
pub fn bar_bg() -> [(f32, Color); 2] {
    [(0.0, ambient(0.205)), (1.0, ambient(0.175))]
}
/// Menus, the search dropdown, toasts and dialogs (see [`overlay_surface`]).
pub fn overlay_bg() -> Color {
    ambient(0.2)
}
/// Ink on a light fill: the primary pill, a selected chip, the accent play buttons.
pub fn ink() -> Color {
    oklch(0.18, 0.02, HUE)
}
/// Black at `a`, for scrims, shadows and the dark discs behind icons over artwork.
pub fn black(a: f32) -> Color {
    oklcha(0.0, 0.0, 0.0, a)
}
/// What a dialog darkens the window with.
pub fn scrim() -> Color {
    black(0.5)
}

// ---- roles

/// The React accent (`text-th-accent`): loved hearts, "in library", selection marks.
pub fn accent() -> Color {
    oklch(0.75, 0.13, 250.0)
}
/// Destructive actions and error text.
pub fn danger() -> Color {
    oklch(0.66, 0.19, 25.0)
}
/// The success toast's dot.
pub fn success() -> Color {
    oklch(0.72, 0.13, 155.0)
}

// ---- text alphas (over `white`)

/// Titles and primary labels.
pub const TEXT: f32 = 0.95;
/// Secondary lines, empty states.
pub const TEXT_MUTED: f32 = 0.5;

// ---- radii

/// Every floating layer. What sits inside one takes [`inset_radius`] of its inset, so the
/// corners run concentric (menu rows: 14 - the menu's padding).
pub const RADIUS_OVERLAY: f32 = 14.0;

// ---- type scale (points)

/// A page's title: Search results, Explore, Feed, Settings, and the album/artist/playlist hero.
pub const PAGE_TITLE: f32 = 28.0;
/// A section inside a page (Home rows, album and artist modules, search sections).
pub const SECTION: f32 = 20.0;
/// A dialog's title.
pub const DIALOG_TITLE: f32 = 20.0;

// ---- shared treatments

// ---- the overlay material
//
// Only the layer that floats above the content is glass: menus, submenus, the search dropdown,
// the video quality menu, toasts and dialogs. The page, sidebar, drawer and player bar stay
// opaque. The glass is the frosted ("regular") kind, since every overlay is text: a strong
// blur of what's behind it under a dark tint in the ambient hue, dense enough that body text
// keeps 4.5:1 over a blurred white backdrop, with a top-lit hairline and a soft shadow so it
// reads as a plane lifted off the page. What sits on a glass surface (rows, hover and selected
// fills, fields, pills) uses plain translucent fills, never another blur, and a submenu is its
// own surface beside its menu, not inside it.

/// What kind of floating layer a surface is; the glass gets denser as the surface gets larger
/// and more text-heavy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlay {
    /// A one-line toast: small and transient, the clearest glass.
    Toast,
    /// A menu, submenu or the search dropdown: short labels, transient.
    Menu,
    /// A modal dialog over the scrim: a reading surface, the most opaque.
    Dialog,
}

/// The glass for each kind: (blur sigma in points, tint lightness, tint alpha).
///
/// The alphas come from the worst backdrop, a blurred pure white (linear luminance 1; blending
/// is linear): the backdrop's share `1 - a` sets the surface's luminance, and body text has to
/// keep 4.5:1 over it. Over white, toast text (white at 0.92) keeps 5.4:1 and menu labels
/// (0.78 and up) 5.3:1; a menu's muted captions (0.5) keep 4.5:1 up to a backdrop of 0.72 and
/// 3.8:1 at white. A dialog also sits over the half-black scrim, so its backdrop is at most
/// 0.5: even its muted text keeps 6:1. Over the app's dark page every kind is above 14:1.
fn glass(kind: Overlay) -> (f32, f32, f32) {
    match kind {
        Overlay::Toast => (16.0, 0.17, 0.88),
        Overlay::Menu => (24.0, 0.17, 0.90),
        Overlay::Dialog => (32.0, 0.18, 0.93),
    }
}

/// Whether the overlays are glass at all. `SONE_GLASS=0` (read once) draws the opaque surface
/// instead, the app's Reduce Transparency.
pub fn glass_enabled() -> bool {
    static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("SONE_GLASS").map_or(true, |v| v.trim() != "0"));
    *ON
}

/// Takes `n` of this frame's backdrop blurs, all or none. Rune blurs only the first
/// `MAX_BACKDROP_BLURS` blurred elements a frame draws and draws the rest as their bare fill,
/// which for glass would be a see-through tint; so a surface that doesn't get one here is drawn
/// opaque instead. Overlays are added to the root in the order they're built, so handing them
/// out in build order matches the order Rune counts them in (the page under them has none).
fn take_glass(n: usize) -> bool {
    glass_enabled()
        && GLASS_LEFT.with(|g| {
            let ok = g.get() >= n;
            if ok {
                g.set(g.get() - n);
            }
            ok
        })
}

/// The opaque overlay surface: what `SONE_GLASS=0` and a surface past the blur budget draw.
fn opaque_surface(e: El) -> El {
    e.radius(RADIUS_OVERLAY).fill(overlay_bg()).border(1.0, white(0.08)).shadow(10.0, 30.0, black(0.5))
}

fn glass_surface(e: El, kind: Overlay) -> El {
    let (sigma, l, a) = glass(kind);
    let hue = AMBIENT.with(|h| h.get());
    e.radius(RADIUS_OVERLAY)
        .backdrop_blur(sigma)
        .fill(oklcha(l, TINT, hue, a))
        // a faint sheen falling off from the top, and a hairline lit from above
        .gradient(white(0.035), white(0.0))
        .border_gradient(1.0, white(0.24), white(0.06))
        .shadow(10.0, 30.0, black(0.45))
        .shadow(1.0, 3.0, black(0.25))
}

fn surface_as(e: El, kind: Overlay, glass: bool) -> El {
    if glass { glass_surface(e, kind) } else { opaque_surface(e) }
}

/// The background of anything that floats over the window: material, corners, hairline and
/// shadow. Menus, submenus, the search dropdown, dialogs and the video quality menu all go
/// through here, toasts through [`overlay_surfaces`], so changing the material is a change to
/// this one place. Glass while the frame's blur budget lasts, opaque after it.
pub fn overlay_surface(e: El, kind: Overlay) -> El {
    surface_as(e, kind, take_glass(1))
}

/// For `n` surfaces seen together (the toast stack): all glass if the budget covers all of
/// them, else all opaque, so a stack never mixes the two.
pub fn overlay_surfaces(kind: Overlay, n: usize) -> impl Fn(El) -> El {
    let glass = take_glass(n);
    move |e| surface_as(e, kind, glass)
}

/// The corner radius of something inset `padding` points inside an overlay surface, so its
/// corners run concentric with the surface's.
pub const fn inset_radius(padding: f32) -> f32 {
    RADIUS_OVERLAY - padding
}

/// A keyboard-reachable control with the app's focus ring, drawn outside it (buttons, pills,
/// chips, tabs).
pub fn focusable(e: El) -> El {
    e.focusable(true).focus_ring(2.0, 2.0, white(0.6))
}

/// [`focusable`] for a full-width row or menu item: the ring sits just inside its edge, so a
/// scroll view or menu around it doesn't clip it.
pub fn focusable_row(e: El) -> El {
    e.focusable(true).focus_ring(2.0, -2.0, white(0.6))
}
