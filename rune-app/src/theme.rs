//! Design tokens: the colours, radii and type sizes the views share, and the one surface every
//! floating layer is drawn on, in the theme Settings → Themes picked (React's themes: a preset
//! or a custom accent and background, kept in `theme.json`; see [`set`] and [`init`]).
//!
//! Text and fills on the ground are `ui::white(alpha)`, the foreground: near white on a dark
//! theme, near black on a light one ([`fg_lightness`]). `TEXT` and `TEXT_MUTED` below are the two
//! text levels new code should pick from.

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
    /// This frame draws in the picked theme (the main window; see [`begin_themed`]).
    static THEMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// This frame's foreground is dark (a light theme, outside the players over artwork).
    static LIGHT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Backdrop blurs [`overlay_surface`] may still hand out this frame (see [`take_glass`]).
    static GLASS_LEFT: std::cell::Cell<usize> = const { std::cell::Cell::new(rune::MAX_BACKDROP_BLURS) };
}

/// Starts a window's frame: sets the hue the surfaces below are drawn in and gives the frame
/// its full budget of glass surfaces. Every window's view calls it first, so neither the hue
/// nor the budget carries over from another window's frame.
pub fn set_ambient(hue: f32) {
    AMBIENT.with(|a| a.set(hue));
    GLASS_LEFT.with(|g| g.set(rune::MAX_BACKDROP_BLURS));
    THEMED.with(|t| t.set(false));
    LIGHT.with(|t| t.set(false));
}

/// After [`set_ambient`], for the main window: its surfaces take the picked theme's background
/// (the default theme keeps the hue of what's playing) and, in a light theme, its foreground
/// turns dark.
pub fn begin_themed() {
    let p = palette();
    THEMED.with(|t| t.set(!p.ambient));
    LIGHT.with(|t| t.set(p.light));
}

/// Draws what `f` builds on a dark foreground whatever the theme: the full-screen and video
/// players sit over artwork and video, which stay dark.
pub fn dark<T>(f: impl FnOnce() -> T) -> T {
    let was = LIGHT.with(|t| t.replace(false));
    let out = f();
    LIGHT.with(|t| t.set(was));
    out
}

/// The foreground's lightness: `ui::white` is this at an alpha.
pub fn fg_lightness() -> f32 {
    if LIGHT.with(|t| t.get()) { 0.2 } else { 0.985 }
}

/// A surface at lightness `l` in hue `h`.
pub fn surface(l: f32, h: f32) -> Color {
    oklch(l, TINT, h)
}

fn ambient(l: f32) -> Color {
    if THEMED.with(|t| t.get()) {
        let p = palette();
        return oklch(surface_l(p.l, p.light, l), p.c, p.h);
    }
    surface(l, AMBIENT.with(|a| a.get()))
}

/// The lightness a surface drawn at `l` in the default dark ground (0.155) takes over a theme
/// background of lightness `bg`: as much lighter on a dark one, darker on a light one (React
/// deriveTheme's lightness shifts).
fn surface_l(bg: f32, light: bool, l: f32) -> f32 {
    if light { (bg - (l - 0.155).abs()).clamp(0.0, 1.0) } else { (bg + (l - 0.155)).clamp(0.0, 1.0) }
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
    if LIGHT.with(|t| t.get()) { oklch(0.98, 0.01, HUE) } else { oklch(0.18, 0.02, HUE) }
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
    palette().accent
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
    if THEMED.with(|t| t.get()) {
        let p = palette();
        let l = if p.light { (p.l + 0.02).min(0.99) } else { surface_l(p.l, false, l) };
        return e.radius(RADIUS_OVERLAY).backdrop_blur(sigma).fill(oklcha(l, p.c, p.h, a))
            .gradient(white(0.035), white(0.0)).border_gradient(1.0, white(0.24), white(0.06))
            .shadow(10.0, 30.0, black(0.45)).shadow(1.0, 3.0, black(0.25));
    }
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

// ------------------------------------------------------------------ themes (React lib/theme.ts)

/// A theme as React keeps it: a name ("Custom" or a preset's) and two `#RRGGBB` colours.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub accent: String,
    pub bg: String,
}

/// React's PRESET_THEMES, in its order; the first is the default.
pub const PRESETS: [(&str, &str, &str); 15] = [
    ("Violet Night", "#A855F7", "#130F1A"),
    ("Cyberpunk", "#FCE300", "#18180C"),
    ("Forest", "#22C55E", "#0E1410"),
    ("Ocean", "#3B82F6", "#0E1118"),
    ("Midnight Cyan", "#00FFFF", "#121212"),
    ("Sakura", "#F9A8D4", "#140F12"),
    ("Rose", "#F43F5E", "#140E0F"),
    ("Ember", "#F97316", "#151010"),
    ("Copper", "#E8915A", "#12100E"),
    ("Noir", "#FFFFFF", "#020202"),
    ("Daylight", "#2563EB", "#F5F3EE"),
    ("Snowfall", "#0891B2", "#F8FAFC"),
    ("Paper", "#7C3AED", "#FAF5FF"),
    ("Meadow", "#16A34A", "#F2F6EE"),
    ("Blossom", "#E11D48", "#FEF3F2"),
];

impl Theme {
    pub fn preset(i: usize) -> Theme {
        let (n, a, b) = PRESETS[i];
        Theme { name: n.into(), accent: a.into(), bg: b.into() }
    }
    pub fn default_theme() -> Theme {
        Theme::preset(0)
    }
    /// The preset with these colours, if one has them (ThemesTab's activePreset).
    pub fn matching_preset(&self) -> Option<usize> {
        PRESETS.iter().position(|(_, a, b)| a.eq_ignore_ascii_case(&self.accent) && b.eq_ignore_ascii_case(&self.bg))
    }
}

/// `#RGB` or `#RRGGBB` to uppercase `#RRGGBB` (normalizeHex).
pub fn normalize_hex(s: &str) -> Option<String> {
    let d = s.trim().strip_prefix('#')?;
    let d: String = match d.len() {
        3 => d.chars().flat_map(|c| [c, c]).collect(),
        6 => d.to_string(),
        _ => return None,
    };
    d.chars().all(|c| c.is_ascii_hexdigit()).then(|| format!("#{}", d.to_ascii_uppercase()))
}

/// sRGB channels 0..1 of a `#RRGGBB`.
pub fn rgb(hex: &str) -> Option<[f32; 3]> {
    let h = normalize_hex(hex)?;
    let ch = |i: usize| u8::from_str_radix(&h[1 + 2 * i..3 + 2 * i], 16).ok().map(|v| v as f32 / 255.0);
    Some([ch(0)?, ch(1)?, ch(2)?])
}

/// A `#RRGGBB` in OKLCH: (lightness, chroma, hue in degrees).
pub fn hex_oklch(hex: &str) -> Option<(f32, f32, f32)> {
    let [r, g, b] = rgb(hex)?;
    let lin = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    let (r, g, b) = (lin(r), lin(g), lin(b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let ll = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    Some((ll, a.hypot(bb), bb.atan2(a).to_degrees().rem_euclid(360.0)))
}

/// A colour of a theme, as Rune draws it.
pub fn color_of(hex: &str) -> Color {
    let (l, c, h) = hex_oklch(hex).unwrap_or((0.5, 0.0, 0.0));
    oklch(l, c, h)
}

/// React deriveTheme's isDark, negated: the background's HSL lightness at 50% or more.
pub fn is_light(bg: &str) -> bool {
    rgb(bg).map(|[r, g, b]| (r.max(g).max(b) + r.min(g).min(b)) / 2.0 >= 0.5).unwrap_or(false)
}

/// A surface of a theme that isn't the one in use (the presets' cards, the preview): the
/// background drawn at `l` as [`ambient`] draws the theme in use.
pub fn surface_of(bg: &str, l: f32) -> Color {
    let (bl, c, h) = hex_oklch(bg).unwrap_or((0.155, TINT, HUE));
    oklch(surface_l(bl, is_light(bg), l), c.min(0.06), h)
}

/// What the views draw with, worked out from the theme once.
#[derive(Clone, Copy)]
struct Palette {
    accent: Color,
    /// The background in OKLCH (its chroma kept low enough to read as a tint).
    l: f32,
    c: f32,
    h: f32,
    light: bool,
    /// The default theme: surfaces follow what's playing, as the app always has.
    ambient: bool,
}

fn palette_of(t: &Theme) -> Palette {
    let (l, c, h) = hex_oklch(&t.bg).unwrap_or((0.155, TINT, HUE));
    let default = Theme::default_theme();
    let ambient = t.accent.eq_ignore_ascii_case(&default.accent) && t.bg.eq_ignore_ascii_case(&default.bg);
    Palette { accent: color_of(&t.accent), l, c: c.min(0.06), h, light: is_light(&t.bg), ambient }
}

static CURRENT: std::sync::LazyLock<std::sync::RwLock<(Theme, Palette)>> = std::sync::LazyLock::new(|| {
    let t = Theme::default_theme();
    let p = palette_of(&t);
    std::sync::RwLock::new((t, p))
});
static VERSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn palette() -> Palette {
    CURRENT.read().unwrap().1
}

/// The theme in use.
pub fn current() -> Theme {
    CURRENT.read().unwrap().0.clone()
}

/// Bumped on every theme change; the main window's panes take it as an input.
pub fn version() -> u64 {
    VERSION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Use `t` from the next frame on (nothing saved). Whether it changed anything.
fn apply(t: Theme) -> bool {
    let mut cur = CURRENT.write().unwrap();
    if cur.0.accent.eq_ignore_ascii_case(&t.accent) && cur.0.bg.eq_ignore_ascii_case(&t.bg) && cur.0.name == t.name {
        return false;
    }
    let p = palette_of(&t);
    *cur = (t, p);
    drop(cur);
    VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    publish_overlay_css();
    crate::controller::bump();
    true
}

/// Settings → Themes: use `t` now. `save` writes it to `theme.json` too (React's write-through,
/// which Settings debounces while a colour is dragged).
pub fn set(t: Theme, save_now: bool) {
    if apply(t.clone()) && save_now {
        save(&t);
    }
}

/// Write the theme in use to `theme.json`.
pub fn save_current() {
    save(&current());
}

/// themeToFile: a preset's name when the theme is one (by name and colours), else "custom".
pub fn to_file(t: &Theme) -> tauri_app_lib::theme_config::ThemeFile {
    let accent = normalize_hex(&t.accent).unwrap_or_else(|| t.accent.to_uppercase());
    let bg = normalize_hex(&t.bg).unwrap_or_else(|| t.bg.to_uppercase());
    let preset = PRESETS.iter().any(|(n, a, b)| *n == t.name && a.eq_ignore_ascii_case(&accent) && b.eq_ignore_ascii_case(&bg));
    tauri_app_lib::theme_config::ThemeFile {
        version: Some(1),
        preset: if preset { t.name.clone() } else { "custom".into() },
        custom: tauri_app_lib::theme_config::ThemeCustom { accent, background: bg },
    }
}

/// resolveThemeFile: the file's preset, or its custom colours.
pub fn from_file(f: &tauri_app_lib::theme_config::ThemeFile) -> Option<Theme> {
    if f.version.is_some_and(|v| v != 1) {
        return None;
    }
    let accent = normalize_hex(&f.custom.accent)?;
    let bg = normalize_hex(&f.custom.background)?;
    if f.preset == "custom" {
        return Some(Theme { name: "Custom".into(), accent, bg });
    }
    PRESETS.iter().position(|(n, _, _)| *n == f.preset).map(Theme::preset)
}

fn save(t: &Theme) {
    if let Err(e) = tauri_app_lib::theme_config::theme_file_set(to_file(t)) {
        log::warn!("[theme] theme.json unavailable: {e}");
    }
}

/// At launch (React bootstrapThemeFile): the theme in `theme.json`; with no file, the default
/// is written there; an invalid file is left alone (the user may be editing it).
pub fn init() {
    match tauri_app_lib::theme_config::theme_file_get() {
        Ok(Some(f)) => {
            if let Some(t) = from_file(&f) {
                apply(t);
            }
        }
        Ok(None) => save(&current()),
        Err(e) => log::warn!("[theme] not using theme.json: {e}"),
    }
    publish_overlay_css();
}

/// The backend's watcher saw `theme.json` change (`theme-file-changed`): apply it, or, deleted
/// while running, write it back from the theme in use.
pub fn on_file_changed(payload: &str) {
    match serde_json::from_str::<Option<tauri_app_lib::theme_config::ThemeFile>>(payload) {
        Ok(Some(f)) => {
            if let Some(t) = from_file(&f) {
                apply(t);
            }
        }
        Ok(None) => save(&current()),
        Err(e) => log::warn!("[theme] theme-file-changed: {e}"),
    }
}

// ---- React deriveTheme's HSL arithmetic, for the OBS overlay's stylesheet

fn hex_hsl(hex: &str) -> (f32, f32, f32) {
    let [r, g, b] = rgb(hex).unwrap_or([0.0; 3]);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return (0.0, 0.0, l * 100.0);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h * 60.0, s * 100.0, l * 100.0)
}

fn hsl_hex(h: f32, s: f32, l: f32) -> String {
    let (s, l) = (s / 100.0, l / 100.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", to(r), to(g), to(b))
}

fn shift(hex: &str, d: f32) -> String {
    let (h, s, l) = hex_hsl(hex);
    hsl_hex(h, s, (l + d).clamp(0.0, 100.0))
}

/// The CSS variables the OBS overlay reads (useOverlayBridge OVERLAY_VARS), for `t`.
pub fn overlay_css(t: &Theme) -> String {
    let dark = !is_light(&t.bg);
    let dir = if dark { 1.0 } else { -1.0 };
    let (h, s, l) = hex_hsl(&t.accent);
    let vars = [
        ("--th-bg-base", t.bg.clone()),
        ("--th-bg-elevated", shift(&t.bg, 2.5 * dir)),
        ("--th-bg-inset", shift(&t.bg, 7.0 * dir)),
        ("--th-accent", t.accent.clone()),
        ("--th-accent-hover", hsl_hex(h, s, (l * 0.88).clamp(0.0, 100.0))),
        ("--th-on-accent", if dark { "#000000" } else { "#ffffff" }.into()),
        ("--th-text-primary", if dark { "#ffffff" } else { "#111111" }.into()),
        ("--th-text-secondary", if dark { "#b3b3b3" } else { "#444444" }.into()),
        ("--th-text-muted", if dark { "#a6a6a6" } else { "#666666" }.into()),
        ("--th-text-faint", "#666666".into()),
        ("--th-border-subtle", if dark { "rgba(255,255,255,0.06)" } else { "rgba(0,0,0,0.08)" }.into()),
        ("--th-slider-fill", if dark { "rgba(255,255,255,0.65)" } else { "rgba(0,0,0,0.45)" }.into()),
        ("--th-slider-track", if dark { "rgba(255,255,255,0.15)" } else { "rgba(0,0,0,0.15)" }.into()),
    ];
    let lines: Vec<String> = vars.iter().map(|(k, v)| format!("  {k}: {v};")).collect();
    format!(":root {{\n{}\n}}", lines.join("\n"))
}

/// Hand the overlay server the theme's stylesheet (React useOverlayBridge's theme effect).
fn publish_overlay_css() {
    let css = overlay_css(&current());
    sone_host_shim::async_runtime::spawn(async move {
        use sone_host_shim::Manager;
        let _ = tauri_app_lib::commands::overlay::overlay_publish_theme(crate::host().state::<tauri_app_lib::AppState>(), css).await;
    });
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn hex_is_normalised() {
        assert_eq!(normalize_hex("#abc").as_deref(), Some("#AABBCC"));
        assert_eq!(normalize_hex(" #a855f7 ").as_deref(), Some("#A855F7"));
        assert_eq!(normalize_hex("#12345"), None);
        assert_eq!(normalize_hex("123456"), None);
    }

    #[test]
    fn files_round_trip() {
        for (i, p) in PRESETS.iter().enumerate() {
            let t = Theme::preset(i);
            assert_eq!(from_file(&to_file(&t)), Some(t.clone()));
            assert_eq!(to_file(&t).preset, p.0);
        }
        let custom = Theme { name: "Custom".into(), accent: "#112233".into(), bg: "#010203".into() };
        assert_eq!(to_file(&custom).preset, "custom");
        assert_eq!(from_file(&to_file(&custom)), Some(custom));
        // a custom theme that lands on a preset's colours stays custom
        let lookalike = Theme { name: "Custom".into(), accent: "#A855F7".into(), bg: "#130F1A".into() };
        assert_eq!(to_file(&lookalike).preset, "custom");
    }

    #[test]
    fn light_and_dark_backgrounds() {
        assert!(!is_light("#130F1A"));
        assert!(is_light("#F5F3EE"));
        let (l, _, _) = hex_oklch("#FFFFFF").unwrap();
        assert!((l - 1.0).abs() < 0.01);
        let (l, c, _) = hex_oklch("#000000").unwrap();
        assert!(l.abs() < 0.01 && c < 0.01);
    }

    #[test]
    fn overlay_css_derives_like_react() {
        let css = overlay_css(&Theme::preset(0));
        assert!(css.contains("--th-bg-base: #130F1A;"));
        assert!(css.contains("--th-on-accent: #000000;"));
        assert_eq!(shift("#000000", 10.0), "#1a1a1a");
    }
}
