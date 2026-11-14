//! Keyboard shortcuts: the table of actions and their default keys, the user's bindings and how
//! they're kept, dispatch from the main window's keys, and the Keyboard Shortcuts dialog where
//! they're rebound (React: lib/shortcuts.ts, hooks/useShortcuts.ts, the AppInitializer dispatch
//! and the UserMenu shortcuts modal).
//!
//! A combo names the physical key as the web's `KeyboardEvent.code` does ("KeyS", "ArrowRight",
//! "Equal"), which is what Rune's `Tx::physical_key` reports (winit's `KeyCode` spells its
//! variants the same way), so a binding saved here reads exactly as the React app's did, and a
//! shortcut stays on its key whatever the keyboard layout.

use crate::controller::{self, Cmd};
use crate::glyph::{G, glyph};
use crate::interact::{self, Fav, Kind, Say};
use crate::ui::{t, white};
use crate::views::Dialog;
use crate::{App, shell, theme};
use rune::{El, Key, KeyEvent, Tx, col, el, row, text};
use serde_json::{Map, Value, json};
use std::cell::Cell;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    PlayPause,
    NextTrack,
    PrevTrack,
    VolumeUp,
    VolumeDown,
    MuteToggle,
    LikeToggle,
    ToggleShuffle,
    ToggleRepeat,
    FocusSearch,
    RefreshData,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleExclusive,
    ToggleBitPerfect,
    ToggleShortcuts,
    CloseWindow,
    QuitApp,
}

/// A key and the modifiers held with it. `mod` is Ctrl (or the logo key), as the React combo's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    pub code: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

const fn c(code: &'static str, ctrl: bool, shift: bool, alt: bool) -> (&'static str, bool, bool, bool) {
    (code, ctrl, shift, alt)
}

pub struct Meta {
    pub id: Action,
    /// The id the React app stores the binding under.
    pub key: &'static str,
    pub label: &'static str,
    default: (&'static str, bool, bool, bool),
    /// False: a held key's auto-repeat doesn't run it again.
    pub repeatable: bool,
    /// Not rebindable: always on its default, shown read-only, its combo reserved.
    pub fixed: bool,
}

impl Meta {
    pub fn default_combo(&self) -> Combo {
        let (code, ctrl, shift, alt) = self.default;
        Combo { code: code.to_string(), ctrl, shift, alt }
    }
}

const fn m(id: Action, key: &'static str, label: &'static str, default: (&'static str, bool, bool, bool), repeatable: bool, fixed: bool) -> Meta {
    Meta { id, key, label, default, repeatable, fixed }
}

/// The React ACTION_REGISTRY, in its order (the dialog lists them so).
pub static REGISTRY: [Meta; 19] = [
    m(Action::PlayPause, "playPause", "Play / Pause", c("Space", false, false, false), true, false),
    m(Action::NextTrack, "nextTrack", "Next track", c("ArrowRight", true, false, false), false, false),
    m(Action::PrevTrack, "prevTrack", "Previous track", c("ArrowLeft", true, false, false), false, false),
    m(Action::VolumeUp, "volumeUp", "Volume up", c("ArrowUp", true, false, false), true, false),
    m(Action::VolumeDown, "volumeDown", "Volume down", c("ArrowDown", true, false, false), true, false),
    m(Action::MuteToggle, "muteToggle", "Mute / Unmute", c("KeyM", false, false, false), false, false),
    m(Action::LikeToggle, "likeToggle", "Like / Unlike current track", c("KeyL", false, false, false), false, false),
    m(Action::ToggleShuffle, "toggleShuffle", "Shuffle on / off", c("KeyS", false, false, true), false, false),
    m(Action::ToggleRepeat, "toggleRepeat", "Repeat off / all / one", c("KeyR", false, false, true), false, false),
    m(Action::FocusSearch, "focusSearch", "Focus search bar", c("KeyK", true, false, false), true, false),
    m(Action::RefreshData, "refreshData", "Refresh app data", c("KeyR", true, true, false), true, true),
    m(Action::ZoomIn, "zoomIn", "Zoom in", c("Equal", true, false, false), true, false),
    m(Action::ZoomOut, "zoomOut", "Zoom out", c("Minus", true, false, false), true, false),
    m(Action::ZoomReset, "zoomReset", "Reset zoom to 100%", c("Digit0", true, false, false), true, false),
    m(Action::ToggleExclusive, "toggleExclusive", "Toggle exclusive output", c("KeyE", true, false, false), false, false),
    m(Action::ToggleBitPerfect, "toggleBitPerfect", "Toggle bit-perfect mode", c("KeyB", true, false, false), false, false),
    m(Action::ToggleShortcuts, "toggleShortcuts", "Show keyboard shortcuts", c("Slash", false, true, false), true, false),
    m(Action::CloseWindow, "closeWindow", "Close window", c("KeyW", true, false, false), false, false),
    m(Action::QuitApp, "quitApp", "Quit SONE", c("KeyQ", true, false, false), false, false),
];

/// Keys the dismissal stack owns: listed so they stay discoverable, never bindable.
pub const FIXED_KEY_DOCS: [(&str, &str); 1] = [("Escape", "Dismiss topmost overlay")];

pub fn meta(id: Action) -> &'static Meta {
    REGISTRY.iter().find(|a| a.id == id).expect("every action is registered")
}

/// Combos no action may take: Ctrl+R (the webview's reload in the original), the fixed
/// actions' and Escape.
fn reserved() -> Vec<Combo> {
    let mut r = vec![Combo { code: "KeyR".into(), ctrl: true, shift: false, alt: false }];
    r.extend(REGISTRY.iter().filter(|a| a.fixed).map(Meta::default_combo));
    r.extend(FIXED_KEY_DOCS.iter().map(|(code, _)| Combo { code: (*code).into(), ctrl: false, shift: false, alt: false }));
    r
}

pub fn is_reserved(c: &Combo) -> bool {
    reserved().contains(c)
}

// ------------------------------------------------------------------ bindings

/// Each action's combo, None when unbound. Indexed as `REGISTRY`.
pub type Bindings = Vec<Option<Combo>>;

/// The localStorage key the React app keeps them under (`sone.shortcuts.v2`), here a key of the
/// shell's store (shell.rs), with the same JSON: `{ actionId: {code, mod, shift, alt} | null }`.
pub const STORAGE_KEY: &str = "sone.shortcuts.v2";

pub fn defaults() -> Bindings {
    REGISTRY.iter().map(|a| Some(a.default_combo())).collect()
}

fn combo_json(c: &Option<Combo>) -> Value {
    match c {
        Some(c) => json!({"code": c.code, "mod": c.ctrl, "shift": c.shift, "alt": c.alt}),
        None => Value::Null,
    }
}

fn combo_of(v: &Value) -> Option<Combo> {
    Some(Combo { code: v["code"].as_str()?.to_string(), ctrl: v["mod"].as_bool()?, shift: v["shift"].as_bool()?, alt: v["alt"].as_bool()? })
}

pub fn to_json(b: &Bindings) -> Value {
    Value::Object(REGISTRY.iter().zip(b).map(|(a, c)| (a.key.to_string(), combo_json(c))).collect::<Map<_, _>>())
}

/// What a stored map binds: its well-formed combos, an explicit `null` unbound, and for an id it
/// lacks (an action added since it was saved) the default, unless that combo is taken or
/// reserved (React `fillMissingBindings`). A fixed action always has its default.
pub fn from_json(stored: &Value) -> Bindings {
    let Some(obj) = stored.as_object() else { return defaults() };
    let mut claimed: Vec<Combo> = obj.values().filter_map(combo_of).collect();
    claimed.extend(REGISTRY.iter().filter(|a| a.fixed).map(Meta::default_combo));
    REGISTRY
        .iter()
        .map(|a| {
            if a.fixed {
                return Some(a.default_combo());
            }
            match obj.get(a.key) {
                Some(v) => combo_of(v),
                None => {
                    let d = a.default_combo();
                    if claimed.contains(&d) || is_reserved(&d) {
                        None
                    } else {
                        claimed.push(d.clone());
                        Some(d)
                    }
                }
            }
        })
        .collect()
}

static BINDINGS: Mutex<Option<Bindings>> = Mutex::new(None);

/// The bindings in force: what's saved, else the defaults.
pub fn bindings() -> Bindings {
    let mut b = BINDINGS.lock().unwrap();
    b.get_or_insert_with(|| {
        let stored = shell::store_get(STORAGE_KEY);
        let b = stored.as_ref().map(from_json).unwrap_or_else(defaults);
        // React writes the filled map back at startup, so a later default can't steal a key.
        if stored.is_some_and(|s| to_json(&b) != s) {
            shell::store_set(STORAGE_KEY, to_json(&b));
        }
        b
    })
    .clone()
}

fn set_bindings(b: Bindings) {
    shell::store_set(STORAGE_KEY, to_json(&b));
    *BINDINGS.lock().unwrap() = Some(b);
}

/// Bind `id` to `combo`, taking it from any other action that held it (the dialog's capture).
pub fn rebind(mut b: Bindings, id: Action, combo: Combo) -> Bindings {
    for (a, slot) in REGISTRY.iter().zip(b.iter_mut()) {
        if a.id != id && slot.as_ref() == Some(&combo) {
            *slot = None;
        }
    }
    if let Some(i) = REGISTRY.iter().position(|a| a.id == id) {
        b[i] = Some(combo);
    }
    b
}

/// The action `combo` runs, if any. A fixed action wins a collision, as its combo is reserved.
pub fn action_for(b: &Bindings, combo: &Combo) -> Option<Action> {
    let hit = |fixed: bool| REGISTRY.iter().zip(b).find(|(a, c)| a.fixed == fixed && c.as_ref() == Some(combo)).map(|(a, _)| a.id);
    hit(true).or_else(|| hit(false))
}

// ------------------------------------------------------------------ reading keys

const MODIFIER_CODES: [&str; 8] = ["ControlLeft", "ControlRight", "SuperLeft", "SuperRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight"];

/// The combo a key press makes, as the React `keyFromEvent` reads it; None for a modifier alone.
/// The physical key names it; without one (a synthetic key), it's derived from the logical key.
pub fn combo_from(e: &KeyEvent, tx: &Tx) -> Option<Combo> {
    let code = match tx.physical_key() {
        Some(k) => format!("{k:?}"),
        None => code_of(&e.key)?,
    };
    if MODIFIER_CODES.contains(&code.as_str()) || matches!(e.key, Key::Modifier { .. }) {
        return None;
    }
    let m = e.mods;
    Some(Combo { code, ctrl: m.ctrl || m.cmd || m.logo, shift: m.shift, alt: m.alt })
}

/// The `code` of a logical key on a US layout, for keys with no physical origin.
fn code_of(k: &Key) -> Option<String> {
    let ch = |c: char| -> Option<String> {
        Some(match c.to_ascii_lowercase() {
            ' ' => "Space".into(),
            l @ 'a'..='z' => format!("Key{}", l.to_ascii_uppercase()),
            d @ '0'..='9' => format!("Digit{d}"),
            '=' | '+' => "Equal".into(),
            '-' | '_' => "Minus".into(),
            '/' | '?' => "Slash".into(),
            '\\' | '|' => "Backslash".into(),
            '[' | '{' => "BracketLeft".into(),
            ']' | '}' => "BracketRight".into(),
            ',' | '<' => "Comma".into(),
            '.' | '>' => "Period".into(),
            ';' | ':' => "Semicolon".into(),
            '\'' | '"' => "Quote".into(),
            '`' | '~' => "Backquote".into(),
            _ => return None,
        })
    };
    Some(match k {
        Key::Text(t) => ch(t.chars().next()?)?,
        Key::Command(c) => ch(*c)?,
        Key::Enter => "Enter".into(),
        Key::Backspace => "Backspace".into(),
        Key::Delete => "Delete".into(),
        Key::Left => "ArrowLeft".into(),
        Key::Right => "ArrowRight".into(),
        Key::Up => "ArrowUp".into(),
        Key::Down => "ArrowDown".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PageUp".into(),
        Key::PageDown => "PageDown".into(),
        Key::Insert => "Insert".into(),
        Key::Escape => "Escape".into(),
        Key::Tab => "Tab".into(),
        Key::Function(n) => format!("F{n}"),
        _ => return None,
    })
}

fn code_display(code: &str) -> String {
    let named = match code {
        "Space" => "Space",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Escape" => "Esc",
        "Delete" => "Del",
        "PageUp" => "PgUp",
        "PageDown" => "PgDn",
        "Equal" => "=",
        "Minus" => "-",
        "Slash" => "/",
        "Backslash" => "\\",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Comma" => ",",
        "Period" => ".",
        "Semicolon" => ";",
        "Quote" => "'",
        "Backquote" => "`",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    if let Some(k) = code.strip_prefix("Key") {
        return k.into();
    }
    if let Some(d) = code.strip_prefix("Digit") {
        return d.into();
    }
    if let Some(n) = code.strip_prefix("Numpad") {
        return format!("Num{n}");
    }
    code.into()
}

/// "Ctrl + Alt + Shift + K", or "—" unbound (React `formatCombo`).
pub fn format(c: Option<&Combo>) -> String {
    let Some(c) = c else { return "—".into() };
    let mut parts: Vec<String> = Vec::new();
    if c.ctrl {
        parts.push("Ctrl".into());
    }
    if c.alt {
        parts.push("Alt".into());
    }
    if c.shift {
        parts.push("Shift".into());
    }
    parts.push(code_display(&c.code));
    parts.join(" + ")
}

// ------------------------------------------------------------------ dispatch

/// The main window's keys: the dialog's capture first, then the bound actions. True when one
/// took the key. Escape's layers stay main.rs's (they run before this, unless capturing).
pub fn on_key(s: &mut App, e: &KeyEvent, tx: &mut Tx) -> bool {
    if capture(s, e, tx) {
        return true;
    }
    let Some(combo) = combo_from(e, tx) else { return false };
    let Some(id) = action_for(&bindings(), &combo) else { return false };
    let meta = meta(id);
    if !meta.repeatable && tx.key_repeat() {
        return true;
    }
    // In a text field only Ctrl and Alt chords stay live (Alt chords type nothing).
    let typing = s.nav.search.focused || s.nav.filter.focused;
    if typing && !combo.ctrl && !combo.alt {
        return false;
    }
    run(s, id, tx);
    true
}

/// A key in the search field, before its editor sees it: Ctrl and Alt chords that are bound run
/// their action there too (the React handler listens on the window, so Ctrl+→ skips a track even
/// from the search box instead of moving the caret by a word).
pub fn field_key(s: &mut App, e: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(combo) = combo_from(e, tx).filter(|c| c.ctrl || c.alt) else { return false };
    let Some(id) = action_for(&bindings(), &combo) else { return false };
    if !meta(id).repeatable && tx.key_repeat() {
        return true;
    }
    run(s, id, tx);
    true
}

/// The volume Mute put away, for Unmute (React preMuteVolumeAtom, in memory only).
static PRE_MUTE: Mutex<f32> = Mutex::new(0.0);

pub fn run(s: &mut App, id: Action, tx: &mut Tx) {
    let v = controller::view();
    match id {
        Action::PlayPause => {
            crate::fx::key_toggled();
            controller::send(Cmd::TogglePlay);
        }
        Action::NextTrack => controller::send(Cmd::Next),
        Action::PrevTrack => controller::send(Cmd::Previous),
        Action::VolumeUp => controller::send(Cmd::SetVolume((v.volume + 0.1).min(1.0))),
        Action::VolumeDown => controller::send(Cmd::SetVolume((v.volume - 0.1).max(0.0))),
        Action::MuteToggle => {
            if v.volume > 0.0 {
                *PRE_MUTE.lock().unwrap() = v.volume;
                controller::send(Cmd::SetVolume(0.0));
            } else {
                let back = *PRE_MUTE.lock().unwrap();
                controller::send(Cmd::SetVolume(if back > 0.0 { back } else { 0.5 }));
            }
        }
        Action::LikeToggle => {
            // (React likes the item by its track id even when it's a video; a video's heart is
            // its video favourite, as the video player's heart is.)
            if let Some(cur) = &v.current {
                let kind = if v.video.is_some() { Fav::Video } else { Fav::Track };
                interact::toggle_fav(kind, cur.id.to_string(), cur.title.clone(), Say::Nothing);
            }
        }
        Action::ToggleShuffle => controller::send(Cmd::ToggleShuffle),
        Action::ToggleRepeat => controller::send(Cmd::CycleRepeat),
        Action::FocusSearch => {
            s.nav.search.focused = true;
            s.nav.search_open = !s.nav.search.text.trim().is_empty();
        }
        Action::RefreshData => shell::refresh_app(s),
        Action::ZoomIn => shell::zoom_step(1),
        Action::ZoomOut => shell::zoom_step(-1),
        Action::ZoomReset => shell::zoom_step(0),
        Action::ToggleExclusive => {
            if v.exclusive {
                controller::send(Cmd::SetExclusive(false));
                interact::toast("Exclusive output off — takes effect next track".into(), Kind::Info);
            } else {
                exclusive_on();
                interact::toast("Exclusive output on — takes effect next track".into(), Kind::Info);
            }
        }
        Action::ToggleBitPerfect => {
            if v.bit_perfect {
                controller::send(Cmd::SetBitPerfect(false));
                interact::toast("Bit-perfect off — takes effect next track".into(), Kind::Info);
            } else {
                if !v.exclusive {
                    exclusive_on();
                }
                controller::send(Cmd::SetBitPerfect(true));
                interact::toast("Bit-perfect on — takes effect next track".into(), Kind::Info);
            }
        }
        Action::ToggleShortcuts => {
            // The account menu closes too: left open it would sit over the dialog.
            s.nav.menu_open = false;
            s.nav.dialog = if s.nav.dialog == Some(Dialog::Shortcuts) { None } else { Some(Dialog::Shortcuts) };
            stop_editing();
        }
        Action::CloseWindow => tx.close_window(),
        Action::QuitApp => shell::quit(tx),
    }
}

/// Exclusive output on, choosing the first output device when none is chosen yet (React's
/// shortcut does the same).
fn exclusive_on() {
    controller::send(Cmd::SetExclusive(true));
    // (Off the UI thread: listing the devices may probe GStreamer when nothing is cached yet.)
    sone_core::host::async_runtime::spawn_blocking(|| {
        use sone_core::host::Manager;
        use sone_core::commands::utility;
        let h = crate::host();
        let st = h.state::<sone_core::AppState>();
        if st.exclusive_device.lock().unwrap().is_some() {
            return;
        }
        if let Ok(devices) = utility::list_audio_devices(h.state::<sone_core::AppState>()) {
            if let Some(first) = devices.first() {
                let _ = utility::set_exclusive_device(h.state::<sone_core::AppState>(), first.id.clone());
            }
        }
    });
}

// ------------------------------------------------------------------ the dialog

thread_local! {
    /// The row waiting for its new key, and whether the last key tried was reserved.
    static EDITING: Cell<Option<Action>> = const { Cell::new(None) };
    static RESERVED_HINT: Cell<bool> = const { Cell::new(false) };
}

pub fn editing() -> Option<Action> {
    EDITING.get()
}

pub fn stop_editing() {
    EDITING.set(None);
    RESERVED_HINT.set(false);
}

fn start_editing(id: Action) {
    RESERVED_HINT.set(false);
    EDITING.set(Some(id));
}

/// While a row waits for its key, every key goes to it: Escape cancels, a modifier alone keeps
/// waiting, a reserved combo says so, anything else becomes the binding (taken from whichever
/// action had it).
fn capture(s: &mut App, e: &KeyEvent, tx: &Tx) -> bool {
    let Some(id) = EDITING.get() else { return false };
    if s.nav.dialog != Some(Dialog::Shortcuts) {
        stop_editing();
        return false;
    }
    if matches!(e.key, Key::Escape) {
        stop_editing();
        return true;
    }
    let Some(combo) = combo_from(e, tx) else { return true };
    if is_reserved(&combo) {
        RESERVED_HINT.set(true);
        return true;
    }
    set_bindings(rebind(bindings(), id, combo));
    stop_editing();
    true
}

/// A key cap: the combo in a small mono box, or the capture prompt on the row being edited.
fn cap(label: &str, state: Cap) -> El {
    let (fill, border, ink) = match state {
        Cap::Idle => (white(0.06), white(0.1), white(0.6)),
        Cap::Waiting => (theme::accent().alpha(0.12), theme::accent().alpha(0.6), theme::accent()),
        Cap::Refused => (theme::danger().alpha(0.12), theme::danger().alpha(0.45), theme::danger()),
    };
    row().h(26.0).px(10.0).radius(6.0).items_center().fill(fill).border(1.0, border)
        .child(text(label.to_string()).font(t(12.0, 500.0).mono()).color(ink).a11y_hidden())
}

#[derive(Clone, Copy)]
enum Cap {
    Idle,
    Waiting,
    Refused,
}

/// The Keyboard Shortcuts dialog's body: every action with its current key (double-click a row,
/// or press Return on it, to give it a new one), the keys that can't be rebound dimmed, and
/// Restore defaults. Mounted by `views::dialog` for `Dialog::Shortcuts`.
pub fn dialog_body() -> El {
    let b = bindings();
    let mut rows = col().key("sc-rows").role("grid").label("Keyboard shortcuts").w_full().gap(2.0).focus_group();
    for (a, combo) in REGISTRY.iter().zip(&b) {
        let id = a.id;
        let editing = EDITING.get() == Some(id);
        let shown = if a.fixed { Some(a.default_combo()) } else { combo.clone() };
        let (label, state) = match (editing, RESERVED_HINT.get()) {
            (true, true) => ("Reserved — pick another".to_string(), Cap::Refused),
            (true, false) => ("Press a key…".to_string(), Cap::Waiting),
            _ => (format(shown.as_ref()), Cap::Idle),
        };
        // A row, not a button (the React rows are plain divs): named with its action and key.
        let mut r = row().key(format!("sc-{}", a.key)).role("row").label(format!("{}: {}", a.label, label))
            .w_full().h(36.0).px(8.0).radius(6.0).items_center()
            .child(text(a.label).font(t(13.0, 500.0)).color(white(0.75)).grow(1.0).min_w(0.0).truncate().a11y_hidden())
            .child(cap(&label, state));
        if a.fixed {
            r = r.opacity(0.6).help("Not rebindable");
        } else {
            r = theme::focusable_row(r).hover(white(0.05)).cursor(rune::Cursor::Pointer)
                .on_double_press(move |_: &mut App| start_editing(id))
                // Return on a focused row (the keyboard's double-click); keys while it waits go
                // to the capture before focus navigation or the dialog's Escape.
                .on_key(move |s: &mut App, e: &KeyEvent, tx: &mut Tx| {
                    if EDITING.get().is_some() {
                        return capture(s, e, tx);
                    }
                    if matches!(e.key, Key::Enter) && e.mods == rune::Mods::default() {
                        start_editing(id);
                        return true;
                    }
                    false
                });
        }
        rows = rows.child(r);
    }
    for (code, label) in FIXED_KEY_DOCS {
        let combo = Combo { code: code.into(), ctrl: false, shift: false, alt: false };
        let shown = format(Some(&combo));
        rows = rows.child(row().role("row").label(format!("{label}: {shown}")).w_full().h(36.0).px(8.0).items_center().opacity(0.6).help("Not rebindable")
            .child(text(label).font(t(13.0, 500.0)).color(white(0.75)).grow(1.0).a11y_hidden())
            .child(cap(&shown, Cap::Idle)));
    }
    let restore = crate::ui::pill("sc-restore", "Restore defaults", None, crate::ui::Pill::Secondary).on_press(|_: &mut App| {
        set_bindings(defaults());
        stop_editing();
    });
    let foot = row().w_full().pt(10.0).items_center().gap(12.0)
        .child(row().gap(6.0).items_center().grow(1.0).min_w(0.0)
            .child(glyph(G::Keyboard, 14.0, white(0.4)))
            .child(text("Double-click to edit · Esc to cancel").font(t(11.0, 500.0)).color(white(0.45)).a11y_hidden()))
        .child(restore);
    col().w_full().gap(8.0).child(rows).child(el().w_full().h(1.0).fill(white(0.08))).child(foot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(code: &str, ctrl: bool, shift: bool, alt: bool) -> Combo {
        Combo { code: code.into(), ctrl, shift, alt }
    }

    #[test]
    fn defaults_match_the_react_registry() {
        let b = defaults();
        let of = |id| b[REGISTRY.iter().position(|a| a.id == id).unwrap()].clone().unwrap();
        assert_eq!(format(Some(&of(Action::ToggleShuffle))), "Alt + S");
        assert_eq!(format(Some(&of(Action::ToggleRepeat))), "Alt + R");
        assert_eq!(format(Some(&of(Action::NextTrack))), "Ctrl + →");
        assert_eq!(format(Some(&of(Action::RefreshData))), "Ctrl + Shift + R");
        assert_eq!(format(Some(&of(Action::ToggleShortcuts))), "Shift + /");
        assert_eq!(format(Some(&of(Action::ZoomIn))), "Ctrl + =");
        assert_eq!(format(None), "—");
    }

    #[test]
    fn dispatch_finds_the_bound_action() {
        let b = defaults();
        assert_eq!(action_for(&b, &combo("KeyS", false, false, true)), Some(Action::ToggleShuffle));
        assert_eq!(action_for(&b, &combo("KeyS", true, false, false)), None);
        assert_eq!(action_for(&b, &combo("Space", false, false, false)), Some(Action::PlayPause));
    }

    #[test]
    fn rebinding_takes_the_combo_from_its_holder() {
        let b = rebind(defaults(), Action::ToggleShuffle, combo("KeyM", false, false, false));
        assert_eq!(action_for(&b, &combo("KeyM", false, false, false)), Some(Action::ToggleShuffle));
        let mute = REGISTRY.iter().position(|a| a.id == Action::MuteToggle).unwrap();
        assert_eq!(b[mute], None);
    }

    #[test]
    fn reserved_combos() {
        assert!(is_reserved(&combo("KeyR", true, false, false)));
        assert!(is_reserved(&combo("KeyR", true, true, false)));
        assert!(is_reserved(&combo("Escape", false, false, false)));
        assert!(!is_reserved(&combo("KeyR", false, false, true)));
    }

    #[test]
    fn stored_maps_round_trip_and_fill_missing_ids() {
        let b = rebind(defaults(), Action::LikeToggle, combo("KeyF", false, false, false));
        assert_eq!(from_json(&to_json(&b)), b);
        // An old map without toggleShuffle, whose Alt+S another action already took.
        let mut old = to_json(&defaults());
        let o = old.as_object_mut().unwrap();
        o.remove("toggleShuffle");
        o.remove("quitApp");
        o.insert("muteToggle".into(), json!({"code": "KeyS", "mod": false, "shift": false, "alt": true}));
        let filled = from_json(&old);
        let at = |id| REGISTRY.iter().position(|a| a.id == id).unwrap();
        assert_eq!(filled[at(Action::ToggleShuffle)], None);
        assert_eq!(filled[at(Action::QuitApp)], Some(combo("KeyQ", true, false, false)));
        // An explicit null stays unbound; a fixed action keeps its key whatever was stored.
        let mut nulled = to_json(&defaults());
        nulled["playPause"] = Value::Null;
        nulled["refreshData"] = json!({"code": "KeyX", "mod": false, "shift": false, "alt": false});
        let n = from_json(&nulled);
        assert_eq!(n[at(Action::PlayPause)], None);
        assert_eq!(n[at(Action::RefreshData)], Some(combo("KeyR", true, true, false)));
    }

    #[test]
    fn logical_keys_map_to_codes() {
        assert_eq!(code_of(&Key::Text(" ".into())).as_deref(), Some("Space"));
        assert_eq!(code_of(&Key::Command('k')).as_deref(), Some("KeyK"));
        assert_eq!(code_of(&Key::Text("?".into())).as_deref(), Some("Slash"));
        assert_eq!(code_of(&Key::Right).as_deref(), Some("ArrowRight"));
        assert_eq!(code_of(&Key::Command('=')).as_deref(), Some("Equal"));
    }
}
