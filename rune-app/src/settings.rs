//! The Settings modal (React SettingsSheet and its tabs). It reads the backend's saved
//! settings directly, so it isn't a region: main::view builds it every frame while it's open.
//!
//! Every control writes through the backend command the React tab invokes. What React keeps
//! in localStorage (autoplay, the content switches, the hidden title bar) goes to the
//! controller (Cmd::SetAutoplay, SetAllowExplicit, SetAllowAi) and the title bar to the window
//! shell (shell::set_title_bar_mode). Async commands report back through
//! [`RESULTS`], which the poll applies ([`flush_settings`]).

use crate::App;
use crate::controller::{self, Cmd};
use crate::glyph::G;
use crate::theme;
use crate::ui::{self, caps, t, white};
use crate::views::{heading, open_url};
use rune::{El, TextEdit, Tx, col, el, row, spacer, text, text_input};
use serde_json::Value;
use sone_host_shim::Manager;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri_app_lib::AppState;
use tauri_app_lib::commands::{mcp, overlay, scrobble, utility};

/// [`ui::pill`], primary or secondary.
fn button<S: 'static>(key: &str, label: &str, g: Option<G>, primary: bool) -> El {
    ui::pill(key, label, g, if primary { ui::Pill::Primary } else { ui::Pill::Secondary })
}

fn st() -> sone_host_shim::State<'static, AppState> {
    crate::host().state::<AppState>()
}

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

// ------------------------------------------------------------------ state

/// Which content switch is waiting on the "Turn off … content?" confirmation.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Content {
    Explicit,
    Ai,
}

/// Last.fm / Libre.fm's browser sign-in (ScrobbleTab AudioscrobblerState).
#[derive(Clone, Debug, Default, PartialEq)]
enum AuthStep {
    #[default]
    Idle,
    /// Asking the provider for a request token, before the browser opens.
    Waiting,
    /// The browser is open on the provider's page; "I've authorized" completes it.
    Authorized(String),
    Submitting(String),
}

#[derive(Clone, Debug, Default)]
struct Auth {
    step: AuthStep,
    error: Option<String>,
}

/// One scrobbling provider as the backend reports it (get_scrobble_status).
#[derive(Clone, Debug, Default)]
struct Provider {
    name: String,
    connected: bool,
    username: Option<String>,
}

/// The proxy banner (NetworkTab): idle "Not tested", testing, ok or err with the reason.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Banner {
    #[default]
    Idle,
    Testing,
    Ok,
    Err,
}

/// The MCP or overlay server as the backend reports it (…_get_connection_info).
#[derive(Clone, Debug, Default)]
struct Server {
    enabled: bool,
    url: Option<String>,
    port: Option<u16>,
    host: String,
}

impl Server {
    fn from(v: Value) -> Server {
        Server {
            enabled: v["enabled"].as_bool().unwrap_or(false),
            url: v["url"].as_str().map(str::to_string),
            port: v["port"].as_u64().map(|p| p as u16),
            host: v["host"].as_str().unwrap_or("127.0.0.1").to_string(),
        }
    }
}

/// The Settings modal's own state: its fields, and what the async commands reported.
#[derive(Default)]
pub struct Form {
    // Discord
    discord_status: TextEdit,
    discord_due: Option<Instant>,
    discord_rpc: bool,
    // Scrobbling
    lb_token: TextEdit,
    lb_busy: bool,
    lb_error: Option<String>,
    providers: Vec<Provider>,
    queue_size: usize,
    scrobble_loading: bool,
    report_plays: bool,
    lastfm: Auth,
    librefm: Auth,
    // Network
    proxy: tauri_app_lib::ProxySettings,
    proxy_host: TextEdit,
    proxy_port: TextEdit,
    proxy_user: TextEdit,
    proxy_pass: TextEdit,
    proxy_due: Option<Instant>,
    banner: Banner,
    banner_msg: String,
    // MCP
    mcp: Server,
    mcp_on: bool,
    mcp_busy: bool,
    mcp_revealed: bool,
    mcp_error: Option<String>,
    mcp_regen_error: Option<String>,
    mcp_confirm: bool,
    // OBS overlay
    ov: Server,
    ov_on: bool,
    ov_busy: bool,
    ov_host: TextEdit,
    ov_port: TextEdit,
    ov_host_error: Option<String>,
    ov_port_error: Option<String>,
    ov_error: Option<String>,
    // Utilities
    logging: bool,
    // General
    minimize_to_tray: bool,
    // Playback
    pending_off: Option<Content>,
    /// "Copied" on a copy button, until this instant (the React 1.5 s flag), by button key.
    copied: Option<(&'static str, Instant)>,
    /// The account menu's output device list is open.
    device_open: bool,
    /// The profile editor (profile.rs), while open.
    pub profile_edit: Option<crate::profile::Edit>,
    /// The sign-in screen's custom credentials view (login.rs), while shown.
    pub custom_login: Option<crate::login::Custom>,
}

type Apply = Box<dyn FnOnce(&mut Form) + Send>;

/// Results of async commands, applied to the form by the poll.
static RESULTS: Mutex<Vec<Apply>> = Mutex::new(Vec::new());

/// Runs `fut` on the backend's runtime and hands its output to `apply` on the window's thread.
fn run<T: Send + 'static>(fut: impl std::future::Future<Output = T> + Send + 'static, apply: impl FnOnce(&mut Form, T) + Send + 'static) {
    sone_host_shim::async_runtime::spawn(async move {
        let v = fut.await;
        RESULTS.lock().unwrap().push(Box::new(move |f: &mut Form| apply(f, v)));
        controller::bump();
    });
}

fn later(after: Duration) -> Option<Instant> {
    Some(Instant::now() + after)
}

/// Whether a debounced save is waiting (the poll keeps waking until it's written).
pub fn due(s: &App) -> bool {
    s.nav.set.discord_due.is_some() || s.nav.set.proxy_due.is_some() || !RESULTS.lock().unwrap().is_empty()
}

/// Fill the form from what's saved and ask the servers how they are (as the React tabs do on
/// mount). Called whenever Settings opens.
pub fn load_setting_fields(s: &mut App) {
    let f = &mut s.nav.set;
    let saved = st().load_settings().unwrap_or_default();
    f.discord_status = TextEdit::new(&saved.discord_status_text);
    f.discord_due = None;
    f.discord_rpc = utility::get_discord_rpc(st());
    f.report_plays = utility::get_report_plays(st());
    f.logging = utility::get_enable_logging();
    f.minimize_to_tray = utility::get_minimize_to_tray(st());
    f.lb_token = TextEdit::new("");
    f.lb_busy = false;
    f.lb_error = None;
    f.lastfm = Auth::default();
    f.librefm = Auth::default();
    f.proxy = saved.proxy.clone();
    f.proxy_host = TextEdit::new(&saved.proxy.host);
    f.proxy_port = TextEdit::new(&if saved.proxy.port > 0 { saved.proxy.port.to_string() } else { String::new() });
    f.proxy_user = TextEdit::new(saved.proxy.username.as_deref().unwrap_or(""));
    f.proxy_pass = TextEdit::new(saved.proxy.password.as_deref().unwrap_or(""));
    f.proxy_due = None;
    f.banner = Banner::Idle;
    f.banner_msg = "Not tested".into();
    f.mcp_revealed = false;
    f.mcp_confirm = false;
    f.mcp_error = None;
    f.mcp_regen_error = None;
    f.mcp_on = saved.mcp_enabled;
    f.ov_on = saved.overlay_enabled;
    f.ov_host = TextEdit::new(&saved.overlay_host);
    f.ov_port = TextEdit::new(&saved.overlay_port.to_string());
    f.ov_host_error = None;
    f.ov_port_error = None;
    f.ov_error = None;
    f.pending_off = None;
    f.copied = None;
    refresh_scrobble(f, true);
    run(async { mcp::mcp_get_connection_info(st()).await.ok().and_then(|i| serde_json::to_value(i).ok()) }, |f, v| {
        if let Some(v) = v {
            f.mcp = Server::from(v);
            f.mcp_on = f.mcp.enabled;
        }
    });
    run(async { overlay::overlay_get_connection_info(st()).await.ok().and_then(|i| serde_json::to_value(i).ok()) }, |f, v| {
        if let Some(v) = v {
            f.ov = Server::from(v);
            f.ov_on = f.ov.enabled;
            f.ov_host = TextEdit::new(&f.ov.host);
            f.ov_port = TextEdit::new(&f.ov.port.unwrap_or(5578).to_string());
        }
    });
}

/// ScrobbleTab fetchStatus: the providers and the retry queue.
fn refresh_scrobble(f: &mut Form, first: bool) {
    if first {
        f.scrobble_loading = true;
    }
    run(
        async {
            let p = scrobble::get_scrobble_status(st()).await.ok().and_then(|v| serde_json::to_value(v).ok());
            let q = scrobble::get_scrobble_queue_size(st()).await.ok();
            (p, q)
        },
        |f, (p, q)| {
            if let Some(Value::Array(list)) = p {
                f.providers = list.iter().map(|v| Provider {
                    name: v["name"].as_str().unwrap_or("").to_string(),
                    connected: v["connected"].as_bool().unwrap_or(false),
                    username: v["username"].as_str().map(str::to_string),
                }).collect();
            }
            if let Some(q) = q {
                f.queue_size = q;
            }
            f.scrobble_loading = false;
        },
    );
}

/// Whether the account was signed in at the last poll: a sign-out (Log out) resets the window.
static WAS_SIGNED_IN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// After Log out (React useAuth.logout): back to Home with no history, every menu, dialog and
/// field of the account closed or emptied, so the next sign-in starts clean.
fn reset_after_logout(s: &mut App) {
    use crate::views::{Nav, Page};
    // Here rather than in the controller: emptying the catalogue while a frame is being built
    // would change what that frame's regions read under them.
    crate::data::clear();
    crate::interact::reset_favorites();
    let fresh = Nav::default();
    let n = &mut s.nav;
    if *rune::Tracked::untracked(&n.page) != Page::Home {
        *n.page = Page::Home;
    }
    if !rune::Tracked::untracked(&n.back).is_empty() {
        n.back.clear();
    }
    if !rune::Tracked::untracked(&n.fwd).is_empty() {
        n.fwd.clear();
    }
    n.search = fresh.search;
    n.filter = fresh.filter;
    n.search_open = false;
    n.search_sel = None;
    n.menu_open = false;
    n.settings_open = false;
    n.dialog = None;
    n.ctx = None;
    n.create = None;
    n.pkce_paste = fresh.pkce_paste;
    n.set = Form::default();
}

/// Run the debounced saves that are due and apply async results (called from App::poll).
pub fn flush_settings(s: &mut App) {
    let signed_in = controller::signed_in();
    if WAS_SIGNED_IN.swap(signed_in, std::sync::atomic::Ordering::Relaxed) && !signed_in {
        reset_after_logout(s);
    }
    let results: Vec<Apply> = std::mem::take(&mut *RESULTS.lock().unwrap());
    for r in results {
        r(&mut s.nav.set);
    }
    crate::profile::apply_results(s);
    // Without embedded credentials the sign-in screen can only offer the custom ones.
    if !signed_in && s.nav.set.custom_login.is_none() && controller::auth_known() && !crate::login::has_defaults() {
        crate::login::open(s);
    }
    let f = &mut s.nav.set;
    let now = Instant::now();
    if f.copied.is_some_and(|(_, until)| now >= until) {
        f.copied = None;
    }
    if f.discord_due.is_some_and(|t| now >= t) {
        f.discord_due = None;
        let _ = utility::set_discord_status_text(st(), f.discord_status.text.clone());
    }
    if f.proxy_due.is_some_and(|t| now >= t) {
        f.proxy_due = None;
        let p = f.proxy.clone();
        // shouldSubmitProxy: half-typed enabled settings are withheld (the backend fails
        // closed, so `port: 0` would leave the app with no HTTP client until it's finished).
        if should_submit_proxy(&p) {
            // submitProxy: a refusal says why in the banner (the settings still reached disk).
            run(async move { utility::set_proxy_settings(st(), p).await.map_err(|e| e.to_string()) }, |f, r| {
                if let Err(e) = r {
                    f.banner = Banner::Err;
                    f.banner_msg = describe(&e, "Could not apply proxy settings");
                }
            });
        }
    }
}

fn should_submit_proxy(p: &tauri_app_lib::ProxySettings) -> bool {
    !p.enabled || (!p.host.trim().is_empty() && p.port > 0)
}

/// A readable error, or `fallback` when it says nothing (proxySubmit describe).
fn describe(e: &str, fallback: &str) -> String {
    let e = e.trim();
    if e.is_empty() { fallback.to_string() } else { e.to_string() }
}

// ------------------------------------------------------------------ the account menu's audio rows

/// The output devices exclusive mode can use (list_audio_devices), once listed.
static DEVICES: Mutex<Option<Vec<(String, String)>>> = Mutex::new(None);
static DEVICES_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// UserMenu's device effect: list the devices, and with none chosen yet, choose the first.
fn load_devices() {
    if DEVICES_ASKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    sone_host_shim::async_runtime::spawn_blocking(|| {
        let list: Vec<(String, String)> = utility::list_audio_devices(st()).ok().and_then(|d| serde_json::to_value(d).ok())
            .and_then(|v| v.as_array().cloned()).unwrap_or_default().iter()
            .filter_map(|d| Some((d["id"].as_str()?.to_string(), d["name"].as_str().unwrap_or("").to_string()))).collect();
        if utility::get_exclusive_device(st()).is_none()
            && let Some((id, _)) = list.first()
        {
            let _ = utility::set_exclusive_device(st(), id.clone());
        }
        *DEVICES.lock().unwrap() = Some(list);
        controller::bump();
    });
}

/// Exclusive output on or off, from the account menu (turning it off also turns bit-perfect
/// off, through the controller), with UserMenu's toast.
pub fn set_exclusive(s: &mut App, on: bool) {
    controller::send(Cmd::SetExclusive(on));
    s.nav.set.device_open = false;
    if on {
        DEVICES_ASKED.store(false, std::sync::atomic::Ordering::Relaxed);
        load_devices();
    }
    crate::interact::toast(if on { "Exclusive output on — takes effect next track" } else { "Exclusive output off — takes effect next track" }.into(), crate::interact::Kind::Info);
}

pub fn set_bit_perfect(on: bool) {
    controller::send(Cmd::SetBitPerfect(on));
    crate::interact::toast(if on { "Bit-perfect on — takes effect next track" } else { "Bit-perfect off — takes effect next track" }.into(), crate::interact::Kind::Info);
}

/// The output device picker under Exclusive output (UserMenu): the chosen device, opening to
/// the list. Nothing while the list is loading or empty.
pub fn device_picker(s: &App) -> Option<El> {
    let list = DEVICES.lock().unwrap().clone();
    let Some(list) = list else {
        load_devices();
        return None;
    };
    if list.is_empty() {
        return None;
    }
    let chosen = utility::get_exclusive_device(st());
    let name = list.iter().find(|(id, _)| Some(id) == chosen.as_ref()).map(|(_, n)| n.clone()).unwrap_or_else(|| "Select device".into());
    let open = s.nav.set.device_open;
    let mut c = col().w_full().pl(28.0).pr(4.0).py(2.0).gap(2.0)
        .child(theme::focusable_row(row().key("am-device").role("button").label(format!("Output device: {name}"))).w_full().h(30.0).px(10.0).gap(8.0).radius(7.0).items_center()
            .fill(white(0.05)).border(1.0, white(0.08)).hover(white(0.09)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| s.nav.set.device_open = !open)
            .child(text(name).font(t(12.0, 500.0)).color(white(0.8)).truncate().grow(1.0).min_w(0.0).a11y_hidden())
            .child(crate::glyph::glyph(if open { G::ChevronUp } else { G::ChevronDown }, 12.0, white(0.6))));
    if open {
        let mut items = col().w_full().padding(4.0).gap(1.0).radius(8.0).fill(white(0.04)).max_h(160.0).scroll(rune::ScrollSpec { stick_bottom: false });
        for (i, (id, label)) in list.into_iter().enumerate() {
            let on = Some(&id) == chosen.as_ref();
            items = items.child(theme::focusable_row(row().key(format!("am-device-{i}")).role("button").label(label.clone())).w_full().h(28.0).px(8.0).radius(6.0).items_center()
                .fill(if on { theme::accent().alpha(0.1) } else { white(0.0) }).hover(white(0.07)).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    s.nav.set.device_open = false;
                    if let Err(e) = utility::set_exclusive_device(st(), id.clone()) {
                        crate::interact::toast(format!("Couldn't switch device: {e}"), crate::interact::Kind::Error);
                    }
                })
                .child(text(label).font(t(12.0, 500.0)).color(if on { theme::accent() } else { white(0.8) }).truncate().a11y_hidden()));
        }
        c = c.child(items);
    }
    Some(c)
}

// ------------------------------------------------------------------ building blocks

/// The settings modal, when open.
pub fn settings_modal(s: &App) -> Option<El> {
    if !s.nav.settings_open {
        return None;
    }
    let head = heading("Settings", theme::PAGE_TITLE);
    let frame = ui::dialog_frame("settings-layer", "settings-modal", "Settings", ("settings-close", "Close settings"), 860.0, Some(680.0), Some(head), settings_page(s), |s: &mut App| s.nav.settings_open = false);
    // The content-off confirmation sits over the sheet (ContentOffDialog).
    Some(match content_off_dialog(s) {
        Some(d) => el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(frame).child(d),
        None => frame,
    })
}

/// A section's caption (the React tabs' small uppercase labels).
fn section(label: &str) -> El {
    caps(label, 0.45).pt(6.0)
}

/// A group of rows on a slightly lighter card (the React `rounded-[14px] bg-th-surface` lists).
fn card(rows: Vec<El>) -> El {
    let mut c = col().w_full().radius(14.0).fill(white(0.04)).border(1.0, white(0.06)).clip_rounded(14.0);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            c = c.child(el().w_full().h(1.0).fill(white(0.06)));
        }
        c = c.child(r);
    }
    c
}

/// A setting's title and explanation, left of its control (SettingRow).
fn row_text(title: &str, subtitle: &str) -> El {
    let mut c = col().grow(1.0).min_w(0.0).gap(2.0).child(text(title.to_string()).font(t(13.5, 600.0)).color(white(0.92)).a11y_hidden());
    if !subtitle.is_empty() {
        c = c.child(text(subtitle.to_string()).font(t(11.5, 400.0)).color(white(0.5)).wrap().a11y_hidden());
    }
    c
}

/// A switch row: the whole row is the switch, named by its title and described by its
/// explanation. `disabled` greys it out and says why on hover.
fn switch_row(key: &str, title: &str, subtitle: &str, on: bool, disabled: Option<&str>, press: impl Fn(&mut App) + 'static) -> El {
    let mut r = row().key(key.to_string()).role("switch").label(title).checked(on).w_full().px(16.0).py(12.0).gap(14.0).items_center()
        .child(row_text(title, subtitle))
        .child(ui::switch_mark(on));
    if !subtitle.is_empty() {
        r = r.description(subtitle);
    }
    match disabled {
        Some(why) => r.disabled(true).opacity(0.45).help(why),
        None => theme::focusable_row(r).hover(white(0.03)).cursor(rune::Cursor::Pointer).on_press(press),
    }
}

/// A row with a title, explanation and any control on its right (SettingRow with children).
fn setting_row(title: &str, subtitle: &str, control: El) -> El {
    row().w_full().px(16.0).py(12.0).gap(14.0).items_center().child(row_text(title, subtitle)).child(control)
}

/// One option of a settings choice, the chosen one marked with a filled radio. (Its accessible
/// state isn't set: `checked` would make AT-SPI report this button as a toggle button.)
fn choice(key: &str, label: &str, selected: bool, on: impl Fn(&mut App) + 'static) -> El {
    theme::focusable_row(row().key(key.to_string()).role("button").label(label)).focus_selected(selected).w_full().h(48.0).px(14.0).gap(12.0).radius(10.0).items_center()
        .fill(if selected { white(0.12) } else { white(0.05) }).hover(white(0.1)).cursor(rune::Cursor::Pointer)
        .on_press(on)
        .child(ui::radio_mark(selected))
        .child(text(label.to_string()).font(t(13.0, 500.0)).color(white(0.9)).a11y_hidden())
}

/// A text field bound to one of the form's edits; `changed` runs on every edit.
fn field(edit: &TextEdit, key: &str, label: &str, placeholder: &str, w: Option<f32>, get: fn(&mut App) -> &mut TextEdit, changed: fn(&mut App)) -> El {
    let input = text_input(edit, t(13.0, 500.0).lh(18.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95)).caret(white(0.95), white(0.25))
        .placeholder(placeholder, white(0.4))
        .on_edit(get)
        .on_input_change(move |s: &mut App, _| changed(s));
    let r = row().h(38.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).border(1.0, white(0.06)).child(input);
    match w {
        Some(w) => r.w(w),
        None => r.grow(1.0).min_w(0.0),
    }
}

/// A small outlined button (the React tabs' secondary `border rounded-md` buttons).
fn small_button(key: &str, label: &str, enabled: bool) -> El {
    let b = row().key(key.to_string()).role("button").label(label).h(30.0).px(12.0).radius(8.0).items_center().border(1.0, white(0.12))
        .child(text(label.to_string()).font(t(12.0, 600.0)).color(white(if enabled { 0.85 } else { 0.4 })).a11y_hidden());
    // Not marked disabled: its handler checks instead. A control whose disabled state comes and
    // goes with typing kept its first accessible actions in testing, so it couldn't be pressed.
    let b = theme::focusable(b).cursor(rune::Cursor::Pointer);
    if enabled { b.hover(white(0.06)) } else { b.opacity(0.5) }
}

/// A copy button that says "Copied" for 1.5 s after a press (the React copy flags).
fn copy_button(f: &Form, key: &'static str, label: &str, content: String) -> El {
    let shown = if f.copied.is_some_and(|(k, _)| k == key) { "Copied" } else { label };
    small_button(key, shown, true).on_press_tx(move |s: &mut App, tx: &mut Tx| {
        tx.copy_rich(rune::ClipboardContent::new(content.clone()));
        s.nav.set.copied = Some((key, Instant::now() + Duration::from_millis(1500)));
    })
}

/// One line of muted help text.
fn note(s: &str) -> El {
    text(s.to_string()).font(t(11.0, 400.0)).color(white(0.5)).wrap()
}

fn error_line(s: &str) -> El {
    text(s.to_string()).font(t(11.5, 500.0)).color(theme::danger()).wrap()
}

/// A server's status banner (McpTab / OverlayTab): a dot, the address, and Running, Stopped
/// or Server error.
fn status_banner(addr: &str, state: (&str, Option<rune::Color>)) -> El {
    let (word, tone) = state;
    let dot = tone.unwrap_or(white(0.35));
    row().w_full().px(13.0).py(11.0).gap(10.0).radius(11.0).items_center()
        .fill(match tone { Some(c) => c.alpha(0.1), None => white(0.04) }).border(1.0, match tone { Some(c) => c.alpha(0.22), None => white(0.06) })
        .child(el().size(8.0, 8.0).radius(4.0).fill(dot))
        .child(text(addr.to_string()).font(ui::tnum(t(12.0, 500.0))).color(white(0.7)).truncate().min_w(0.0))
        .child(spacer())
        .child(text(word.to_string()).font(t(11.0, 650.0)).color(tone.unwrap_or(white(0.5))))
}

fn mono(s: &str) -> El {
    text(s.to_string()).font(ui::tnum(t(12.0, 500.0))).color(white(0.85))
}

// ------------------------------------------------------------------ the page

/// The settings tabs and the chosen tab's options, under a "Settings" title that the caller
/// draws. The tab column stays in view while the options scroll.
pub fn settings_page(s: &App) -> El {
    let tab = |key: &str, label: &str, which: SettingsTab| {
        let on = s.nav.settings_tab == which;
        theme::focusable_row(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(34.0).px(12.0).radius(10.0).items_center().w_full()
            .fill(if on { white(0.1) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| s.nav.settings_tab = which)
            .child(text(label.to_string()).font(t(13.0, 600.0)).color(if on { theme::accent() } else { white(0.85) }).a11y_hidden())
    };
    // SettingsSheet's groups.
    let group = |label: &str| caps(label, 0.35).px(12.0).pt(10.0).pb(2.0);
    let tabs = col().w(200.0).gap(2.0).focus_group()
        .child(group("Experience"))
        .child(tab("set-playback", "Playback", SettingsTab::Playback))
        .child(tab("set-themes", "Themes", SettingsTab::Themes))
        .child(group("Integrations"))
        .child(tab("set-scrobbling", "Scrobbling", SettingsTab::Scrobbling))
        .child(tab("set-discord", "Discord", SettingsTab::Discord))
        .child(tab("set-mcp", "MCP", SettingsTab::Mcp))
        .child(tab("set-overlay", "OBS Overlay", SettingsTab::Overlay))
        .child(group("System"))
        .child(tab("set-general", "General", SettingsTab::General))
        .child(tab("set-network", "Network", SettingsTab::Network))
        .child(tab("set-utilities", "Utilities", SettingsTab::Utilities));
    let body = match s.nav.settings_tab {
        SettingsTab::Playback => playback_tab(s),
        SettingsTab::General => general_tab(s),
        SettingsTab::Network => network_tab(s),
        SettingsTab::Scrobbling => scrobble_tab(s),
        SettingsTab::Discord => discord_tab(s),
        SettingsTab::Mcp => mcp_tab(s),
        SettingsTab::Overlay => overlay_tab(s),
        SettingsTab::Themes => themes_tab(s),
        SettingsTab::Utilities => utilities_tab(s),
    };
    row().w_full().gap(28.0).items_start().child(tabs.sticky_top(0.0)).child(col().grow(1.0).min_w(0.0).max_w(600.0).gap(10.0).child(body))
}

// ------------------------------------------------------------------ Playback

fn playback_tab(s: &App) -> El {
    let quality = controller::max_quality();
    let q = |key: &str, label: &str, tier: &'static str| choice(key, label, quality == tier, move |_: &mut App| controller::send(Cmd::SetMaxQuality(tier.to_string())));
    let v = &*s.ctl;
    let (bp, ex) = (v.bit_perfect, v.exclusive);
    let gapless_supported = utility::get_gapless_supported();
    let gapless = utility::get_gapless(st());
    let gapless_off = !gapless_supported || ex || bp;
    let gapless_why = if !gapless_supported { "Requires GStreamer 1.24 or newer" } else { "Available in normal mode only" };
    let norm = utility::get_volume_normalization(st());
    let has_playback = v.current.is_some() || !v.queue.is_empty();
    let content = move |which: Content, on: bool| {
        move |s: &mut App| {
            if !on {
                controller::send(content_cmd(which, true));
            } else if has_playback {
                s.nav.set.pending_off = Some(which);
            } else {
                controller::send(content_cmd(which, false));
            }
        }
    };
    let autoplay = v.autoplay;
    col().w_full().gap(10.0)
        .child(col().key("q-group").role("group").labelled_by("q-caption").w_full().gap(8.0).focus_group()
            .child(caps("Streaming quality", 0.5).key("q-caption"))
            .child(q("q-hires", "HI-RES LOSSLESS FLAC · up to 24-bit / 192 kHz", "HI_RES_LOSSLESS"))
            .child(q("q-lossless", "LOSSLESS FLAC · 16-bit / 44.1 kHz", "LOSSLESS"))
            .child(q("q-high", "HIGH AAC · up to 320 kbps", "HIGH")))
        .child(note("Caps the quality requested from Tidal. Playback steps down automatically when a track isn't available at this tier."))
        .child(section("Playback"))
        .child(card(vec![
            switch_row("pb-autoplay", "Autoplay", "Play similar tracks when the queue ends", autoplay, None, move |_: &mut App| controller::send(Cmd::SetAutoplay(!autoplay))),
            switch_row("pb-gapless", "Gapless playback", "Seamless transitions between continuous tracks (normal mode)", gapless && !gapless_off, gapless_off.then_some(gapless_why),
                move |_: &mut App| controller::send(Cmd::SetGapless(!gapless))),
            switch_row("pb-normalize", "Normalize volume", if bp { "Disabled while bit-perfect output is on" } else { "Even out volume differences between tracks" }, norm,
                bp.then_some("Disabled in bit-perfect mode"), move |_: &mut App| {
                    let _ = utility::set_volume_normalization(st(), !norm);
                }),
            switch_row("pb-explicit", "Allow explicit content", "Allow playing tracks marked as explicit", v.allow_explicit, None, content(Content::Explicit, v.allow_explicit)),
            switch_row("pb-ai", "Allow AI content", "Allow playing tracks labeled as AI-generated", v.allow_ai, None, content(Content::Ai, v.allow_ai)),
        ]))
}

/// "Turn off explicit content?" (ContentOffDialog): turning a content switch off with
/// something queued clears the queue, so it asks first.
fn content_off_dialog(s: &App) -> Option<El> {
    let which = s.nav.set.pending_off?;
    let label = match which { Content::Explicit => "explicit", Content::Ai => "AI" };
    let close = |s: &mut App| s.nav.set.pending_off = None;
    let body = col().w_full().gap(20.0)
        .child(text("Turning this off clears your current queue.").font(t(13.0, 400.0)).color(white(0.75)))
        .child(row().w_full().gap(10.0).child(spacer())
            .child(button::<App>("co-cancel", "Not now", None, false).on_press(close))
            .child(button::<App>("co-confirm", "Yes, turn off", None, true).on_press(move |s: &mut App| {
                s.nav.set.pending_off = None;
                controller::send(content_cmd(which, false));
            })));
    let title = format!("Turn off {label} content?");
    Some(ui::dialog_frame("content-off-layer", "content-off", &title, ("content-off-close", "Close dialog"), 400.0, None, Some(ui::dialog_title(&title)), body, close))
}

// ------------------------------------------------------------------ General

/// The content switches' command: the controller saves the switch and, turning one off,
/// clears playback (the UI asked first when something was queued).
fn content_cmd(which: Content, on: bool) -> Cmd {
    match which { Content::Explicit => Cmd::SetAllowExplicit(on), Content::Ai => Cmd::SetAllowAi(on) }
}

fn general_tab(s: &App) -> El {
    use crate::shell::TitleBarMode as TitleBar;
    let mode = crate::shell::title_bar_mode();
    let tray = s.nav.set.minimize_to_tray;
    let tb = |key: &str, label: &str, m: TitleBar| choice(key, label, mode == m, move |_: &mut App| crate::shell::set_title_bar_mode(m));
    col().w_full().gap(10.0)
        .child(col().key("tb-group").role("group").labelled_by("tb-caption").w_full().gap(8.0).focus_group()
            .child(caps("Title bar", 0.5).key("tb-caption"))
            .child(tb("tb-system", "System Native OS title bar and window controls.", TitleBar::System))
            .child(tb("tb-custom", "Custom SONE's own in-app title bar, themed to match.", TitleBar::Custom))
            .child(tb("tb-hidden", "Hidden Frameless — no title bar at all.", TitleBar::Hidden)))
        .child(section("Window"))
        .child(card(vec![switch_row("gn-tray", "Close to tray", "Minimize to system tray instead of quitting", tray, None, move |s: &mut App| {
            s.nav.set.minimize_to_tray = !tray;
            let _ = utility::set_minimize_to_tray(st(), !tray);
        })]))
}

// ------------------------------------------------------------------ Network

/// NetworkTab updateProxy: the edit lands at once, the save 500 ms later, and the banner goes
/// back to "Not tested".
fn proxy_changed(s: &mut App) {
    let f = &mut s.nav.set;
    f.proxy.host = f.proxy_host.text.clone();
    f.proxy.port = f.proxy_port.text.trim().parse().unwrap_or(0);
    let user = f.proxy_user.text.clone();
    f.proxy.username = (!user.is_empty()).then_some(user);
    let pass = f.proxy_pass.text.clone();
    f.proxy.password = (!pass.is_empty()).then_some(pass);
    f.banner = Banner::Idle;
    f.banner_msg = "Not tested".into();
    f.proxy_due = later(Duration::from_millis(500));
}

fn network_tab(s: &App) -> El {
    let f = &s.nav.set;
    let p = &f.proxy;
    let enabled = p.enabled;
    let mut c = col().w_full().gap(10.0)
        .child(section("Network"))
        .child(card(vec![switch_row("px-enabled", "Proxy", "Route all Tidal traffic through a proxy server", enabled, None, move |s: &mut App| {
            s.nav.set.proxy.enabled = !enabled;
            proxy_changed(s);
        })]));
    // The banner outlives the fields: turning the proxy off always submits (it's the way out
    // of a bad one), so its refusal must show with the switch already off.
    if enabled || f.banner == Banner::Err {
        let tone = match f.banner { Banner::Ok => Some(theme::accent()), Banner::Err => Some(theme::danger()), _ => None };
        let proto = match p.proxy_type { tauri_app_lib::ProxyType::Http => "http", tauri_app_lib::ProxyType::Socks5 => "socks5" };
        let endpoint = format!("{proto}://{}:{}", if p.host.is_empty() { "host" } else { &p.host }, if p.port > 0 { p.port.to_string() } else { "port".into() });
        let mut banner = row().key("px-banner").w_full().px(13.0).py(11.0).gap(10.0).radius(11.0).items_center()
            .fill(match tone { Some(c) => c.alpha(0.1), None => white(0.04) }).border(1.0, match tone { Some(c) => c.alpha(0.22), None => white(0.06) })
            .child(el().size(8.0, 8.0).radius(4.0).fill(tone.unwrap_or(white(0.35))));
        if enabled {
            banner = banner.child(text(endpoint).font(ui::tnum(t(12.0, 500.0))).color(white(0.7)).truncate().min_w(0.0).shrink());
        }
        banner = banner.child(spacer()).child(text(f.banner_msg.clone()).font(t(11.0, 650.0)).color(tone.unwrap_or(white(0.5))).wrap().min_w(0.0).shrink());
        c = c.child(banner);
    }
    if enabled {
        let label = match p.proxy_type { tauri_app_lib::ProxyType::Http => "HTTP", tauri_app_lib::ProxyType::Socks5 => "SOCKS5" };
        let socks = p.proxy_type == tauri_app_lib::ProxyType::Socks5;
        let protocol = theme::focusable(row().key("px-protocol").role("button").label(label)).h(38.0).px(12.0).gap(6.0).radius(10.0).items_center()
            .fill(theme::accent().alpha(0.1)).hover(theme::accent().alpha(0.16)).cursor(rune::Cursor::Pointer)
            .help("Switch between HTTP and SOCKS5")
            .on_press(move |s: &mut App| {
                s.nav.set.proxy.proxy_type = if socks { tauri_app_lib::ProxyType::Http } else { tauri_app_lib::ProxyType::Socks5 };
                proxy_changed(s);
            })
            .child(text(label.to_string()).font(t(11.5, 700.0)).color(theme::accent()).a11y_hidden())
            .child(crate::glyph::glyph(G::ChevronDown, 11.0, theme::accent()));
        let testing = f.banner == Banner::Testing;
        let can_test = !testing && !p.host.is_empty() && p.port > 0;
        let test = small_button("px-test", if testing { "Testing…" } else { "Test connection" }, can_test);
        let test = test.on_press(|s: &mut App| {
                let f = &mut s.nav.set;
                if f.banner == Banner::Testing || f.proxy.host.is_empty() || f.proxy.port == 0 {
                    return;
                }
                f.banner = Banner::Testing;
                f.banner_msg = "Testing…".into();
                let p = f.proxy.clone();
                run(async move { utility::test_proxy_connection(p, st()).await }, |f, r| match r {
                    Ok(msg) => {
                        f.banner = Banner::Ok;
                        f.banner_msg = if msg.is_empty() { "Connected".into() } else { msg };
                    }
                    Err(e) => {
                        f.banner = Banner::Err;
                        f.banner_msg = describe(&e, "Connection failed — check host, port, and credentials");
                    }
                });
            });
        c = c.child(caps("Proxy address", 0.45).pt(6.0))
            .child(row().w_full().gap(8.0).items_center()
                .child(protocol)
                .child(field(&f.proxy_host, "px-host", "host", "host", None, |s: &mut App| &mut s.nav.set.proxy_host, proxy_changed))
                .child(text(":").font(t(13.0, 500.0)).color(white(0.4)))
                .child(field(&f.proxy_port, "px-port", "port", "port", Some(90.0), |s: &mut App| &mut s.nav.set.proxy_port, |s: &mut App| {
                    // type="number": digits only
                    let digits: String = s.nav.set.proxy_port.text.chars().filter(char::is_ascii_digit).collect();
                    if digits != s.nav.set.proxy_port.text {
                        s.nav.set.proxy_port = TextEdit::new(&digits);
                    }
                    proxy_changed(s)
                })))
            .child(caps("Authentication · optional", 0.45).pt(6.0))
            .child(row().w_full().gap(9.0)
                .child(field(&f.proxy_user, "px-user", "Username", "Username", None, |s: &mut App| &mut s.nav.set.proxy_user, proxy_changed))
                .child({
                    let input = text_input(&f.proxy_pass, t(13.0, 500.0).lh(18.0)).key("px-pass").label("Password").single_line().secure().grow(1.0).color(white(0.95)).caret(white(0.95), white(0.25))
                        .placeholder("Password", white(0.4))
                        .on_edit(|s: &mut App| &mut s.nav.set.proxy_pass)
                        .on_input_change(|s: &mut App, _| proxy_changed(s));
                    row().h(38.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).border(1.0, white(0.06)).grow(1.0).min_w(0.0).child(input)
                }))
            .child(row().pt(6.0).child(test));
    }
    c
}

// ------------------------------------------------------------------ Scrobbling

fn provider<'a>(f: &'a Form, name: &str) -> Option<&'a Provider> {
    f.providers.iter().find(|p| p.name == name)
}

/// ScrobbleTab handleAudioscrobblerConnect: ask for a request token, open the provider's page.
fn audioscrobbler_connect(s: &mut App, which: &'static str) {
    let auth = if which == "lastfm" { &mut s.nav.set.lastfm } else { &mut s.nav.set.librefm };
    auth.step = AuthStep::Waiting;
    auth.error = None;
    run(
        async move {
            let r = if which == "lastfm" { scrobble::connect_lastfm(st()).await } else { scrobble::connect_librefm(st()).await };
            r.map(|a| (a.url, a.token)).map_err(|e| e.to_string())
        },
        move |f, r| {
            let auth = if which == "lastfm" { &mut f.lastfm } else { &mut f.librefm };
            match r {
                Ok((url, token)) => {
                    open_url(&url);
                    auth.step = AuthStep::Authorized(token);
                }
                Err(e) => {
                    auth.step = AuthStep::Idle;
                    auth.error = Some(describe(&e, "Failed to start auth flow"));
                }
            }
        },
    );
}

/// ScrobbleTab handleAudioscrobblerSubmit: exchange the authorized token for a session.
fn audioscrobbler_submit(s: &mut App, which: &'static str) {
    let auth = if which == "lastfm" { &mut s.nav.set.lastfm } else { &mut s.nav.set.librefm };
    let AuthStep::Authorized(token) = auth.step.clone() else { return };
    auth.step = AuthStep::Submitting(token.clone());
    auth.error = None;
    let tok = token.clone();
    run(async move { scrobble::complete_audioscrobbler_auth(st(), which.to_string(), tok).await.map_err(|e| e.to_string()) }, move |f, r| {
        let auth = if which == "lastfm" { &mut f.lastfm } else { &mut f.librefm };
        match r {
            Ok(_) => {
                *auth = Auth::default();
                refresh_scrobble(f, false);
            }
            Err(e) => {
                auth.step = AuthStep::Authorized(token);
                auth.error = Some(describe(&e, "Authentication failed"));
            }
        }
    });
}

fn disconnect(name: &'static str) -> impl Fn(&mut App) {
    move |_: &mut App| {
        run(async move { scrobble::disconnect_provider(st(), name.to_string()).await.is_ok() }, |f, _| refresh_scrobble(f, false));
    }
}

/// A provider row's left side: its name, and who it scrobbles as (or why not).
fn provider_head(display: &str, status: El, connected: bool) -> El {
    row().w_full().gap(12.0).items_center()
        .child(el().size(4.0, 30.0).radius(2.0).fill(if connected { theme::accent() } else { white(0.0) }))
        .child(col().grow(1.0).min_w(0.0).gap(2.0).child(text(display.to_string()).font(t(13.5, 700.0)).color(white(0.92))).child(status))
}

fn scrobbling_as(user: &str) -> El {
    row().gap(6.0).items_center()
        .child(el().size(7.0, 7.0).radius(3.5).fill(theme::accent()))
        .child(text(format!("Scrobbling as {user}")).font(t(11.5, 500.0)).color(white(0.7)))
}

fn audioscrobbler_row(f: &Form, name: &'static str, display: &str) -> El {
    let p = provider(f, name);
    let connected = p.is_some_and(|p| p.connected);
    let auth = if name == "lastfm" { &f.lastfm } else { &f.librefm };
    let key = |k: &str| format!("{}-{k}", if name == "lastfm" { "lf" } else { "lbfm" });
    let status = if connected {
        scrobbling_as(p.and_then(|p| p.username.as_deref()).unwrap_or(""))
    } else if auth.step == AuthStep::Waiting {
        text("Opening browser…").font(t(11.5, 500.0)).color(white(0.6))
    } else {
        text("Not connected").font(t(11.5, 400.0)).color(white(0.5))
    };
    let mut head = provider_head(display, status, connected);
    if connected {
        head = head.child(small_button(&key("disconnect"), "Disconnect", true).on_press(disconnect(name)));
    } else if auth.step == AuthStep::Idle {
        head = head.child(button::<App>(&key("connect"), "Connect", None, true).on_press(move |s: &mut App| audioscrobbler_connect(s, name)));
    }
    let mut c = col().w_full().px(16.0).py(14.0).gap(10.0).child(head);
    if !connected {
        match &auth.step {
            AuthStep::Authorized(_) | AuthStep::Submitting(_) => {
                let busy = matches!(auth.step, AuthStep::Submitting(_));
                let done = button::<App>(&key("authorized"), if busy { "Checking…" } else { "I've authorized" }, None, true);
                c = c.child(note("Authorize in the browser, then come back and click below."))
                    .child(row().child(done.opacity(if busy { 0.5 } else { 1.0 }).on_press(move |s: &mut App| audioscrobbler_submit(s, name))));
            }
            _ => {}
        }
        if let Some(e) = &auth.error {
            c = c.child(error_line(e));
        }
    }
    c
}

fn listenbrainz_row(f: &Form) -> El {
    let p = provider(f, "listenbrainz");
    let connected = p.is_some_and(|p| p.connected);
    let status = if connected { scrobbling_as(p.and_then(|p| p.username.as_deref()).unwrap_or("")) } else { text("Paste token to connect").font(t(11.5, 400.0)).color(white(0.5)) };
    let mut head = provider_head("ListenBrainz", status, connected);
    if connected {
        head = head.child(small_button("lb-disconnect", "Disconnect", true).on_press(disconnect("listenbrainz")));
    }
    let mut c = col().w_full().px(16.0).py(14.0).gap(10.0).child(head);
    if !connected {
        let can = !f.lb_busy && !f.lb_token.text.trim().is_empty();
        let connect = button::<App>("lb-connect", if f.lb_busy { "Connecting…" } else { "Connect" }, None, true);
        let connect = connect.opacity(if can { 1.0 } else { 0.5 }).on_press(|s: &mut App| {
                let token = s.nav.set.lb_token.text.trim().to_string();
                if s.nav.set.lb_busy || token.is_empty() {
                    return;
                }
                s.nav.set.lb_busy = true;
                s.nav.set.lb_error = None;
                run(async move { scrobble::connect_listenbrainz(st(), token).await.map_err(|e| e.to_string()) }, |f, r| {
                    f.lb_busy = false;
                    match r {
                        Ok(_) => {
                            f.lb_token = TextEdit::new("");
                            refresh_scrobble(f, false);
                        }
                        Err(e) => f.lb_error = Some(describe(&e, "Invalid token")),
                    }
                });
            });
        c = c.child(row().w_full().gap(8.0).items_center()
                .child(field(&f.lb_token, "lb-token", "User token", "User token", None, |s: &mut App| &mut s.nav.set.lb_token, |_: &mut App| {}))
                .child(connect))
            .child(row().gap(4.0).items_center()
                .child(text("Get your token from").font(t(11.0, 400.0)).color(white(0.5)))
                .child(theme::focusable(row().role("button").label("listenbrainz.org/settings")).cursor(rune::Cursor::Pointer).on_press(|_: &mut App| open_url("https://listenbrainz.org/settings/"))
                    .child(text("listenbrainz.org/settings").font(t(11.0, 600.0)).color(theme::accent()).a11y_hidden())));
        if let Some(e) = &f.lb_error {
            c = c.child(error_line(e));
        }
    }
    c
}

fn scrobble_tab(s: &App) -> El {
    let f = &s.nav.set;
    let report = f.report_plays;
    let mut c = col().w_full().gap(10.0)
        .child(section("TIDAL"))
        .child(card(vec![switch_row("sc-report", "Report plays to TIDAL", "Add plays to your TIDAL listening history", report, None, move |s: &mut App| {
            s.nav.set.report_plays = !report;
            run(async move { utility::set_report_plays(st(), !report).await.is_ok() }, move |f, ok| {
                if !ok {
                    f.report_plays = report;
                }
            });
        })]))
        .child(section("Scrobbling"));
    if f.scrobble_loading {
        c = c.child(row().w_full().h(80.0).items_center().justify_center().child(crate::views::loading()));
    } else {
        // Last.fm and Libre.fm first, each with its own Connect (browser sign-in), then ListenBrainz.
        c = c.child(card(vec![audioscrobbler_row(f, "lastfm", "Last.fm"), audioscrobbler_row(f, "librefm", "Libre.fm"), listenbrainz_row(f)]));
        if f.queue_size > 0 {
            c = c.child(row().w_full().pt(6.0).gap(8.0).items_center()
                .child(text(format!("Pending scrobbles: {}", f.queue_size)).font(t(11.0, 600.0)).color(white(0.7)))
                .child(spacer())
                .child(note("auto-retries on reconnect")));
        }
    }
    c
}

// ------------------------------------------------------------------ Discord

const SNAP_RELAY: &str = "mkdir -p $XDG_RUNTIME_DIR/snap.sone\nsocat UNIX-LISTEN:$XDG_RUNTIME_DIR/snap.sone/discord-ipc-0,fork \\\n      UNIX-CONNECT:$XDG_RUNTIME_DIR/discord-ipc-0";

fn discord_changed(s: &mut App) {
    s.nav.set.discord_due = later(Duration::from_millis(500));
}

fn discord_tab(s: &App) -> El {
    let f = &s.nav.set;
    let on = f.discord_rpc;
    let mut c = col().w_full().gap(10.0)
        .child(section("Discord"))
        .child(card(vec![switch_row("dc-rpc", "Discord Rich Presence", "Show what you're listening to on Discord", on, None, move |s: &mut App| {
            s.nav.set.discord_rpc = !on;
            if utility::set_discord_rpc(st(), !on).is_err() {
                s.nav.set.discord_rpc = on;
            }
        })]));
    if utility::is_snap() {
        c = c.child(col().w_full().padding(13.0).gap(8.0).radius(11.0).fill(oklch_warn().alpha(0.1)).border(1.0, oklch_warn().alpha(0.25))
            .child(text("Snap blocks the connection to Discord").font(t(12.0, 650.0)).color(white(0.92)))
            .child(note("Strict confinement denies access to Discord's socket. Run this relay before starting SONE, and leave it running:"))
            .child(col().w_full().padding(12.0).radius(10.0).fill(white(0.05)).child(text(SNAP_RELAY).font(ui::tnum(t(11.0, 500.0))).color(white(0.75))))
            .child(row().w_full().gap(8.0).items_center().child(note("Or install the Flatpak or a native package.")).child(spacer()).child(copy_button(f, "dc-relay-copy", "Copy", SNAP_RELAY.to_string()))));
    }
    if on {
        // React fills the status text through the caret; tags here go at the end of the text.
        let chip = |key: &'static str, tag: &'static str| theme::focusable(row().key(key).role("button").label(tag)).h(26.0).px(10.0).radius(7.0).items_center()
            .fill(theme::accent().alpha(0.1)).border(1.0, theme::accent().alpha(0.3)).hover(theme::accent().alpha(0.2)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                let next = format!("{}{tag}", s.nav.set.discord_status.text);
                s.nav.set.discord_status = TextEdit::new(&next);
                discord_changed(s);
            })
            .child(text(tag).font(ui::tnum(t(11.0, 600.0))).color(theme::accent()).a11y_hidden());
        let v = &*s.ctl;
        let (title, artist, album) = match &v.current {
            Some(cur) => (cur.title.clone(), cur.artist.clone(), v.album.clone()),
            None => ("High Hopes".to_string(), "Pink Floyd".to_string(), "The Division Bell".to_string()),
        };
        let quality = v.quality.clone().filter(|q| !q.is_empty()).unwrap_or_else(|| "24-BIT 192KHZ FLAC".into());
        let rendered = render_status(&f.discord_status.text, &title, &artist, &album);
        let user = crate::data::ready(crate::data::Key::Profile).and_then(|p| p.get(0).and_then(|n| n.as_str()).map(str::to_string)).unwrap_or_default();
        let art = ui::with_image(ui::tile(ui::hue(v.track_id), 60.0, v.track_id.unwrap_or(7), 8.0), if v.current.is_some() { crate::art::cover(v.cover.as_deref(), 60.0) } else { None }, 60.0, 8.0);
        c = c.child(caps("Status text", 0.45).pt(6.0))
            .child(field(&f.discord_status, "dc-status", "{track} by {artist} on {album}", "{track} by {artist} on {album}", None, |s: &mut App| &mut s.nav.set.discord_status, discord_changed))
            .child(row().gap(7.0).items_center().child(note("Customize with tags:")).child(chip("dc-track", "{track}")).child(chip("dc-artist", "{artist}")).child(chip("dc-album", "{album}")))
            .child(caps("In a server", 0.4).pt(10.0))
            .child(row().w_full().gap(11.0).padding(11.0).radius(11.0).fill(white(0.05)).items_center()
                .child(el().size(40.0, 40.0).radius(20.0).fill(theme::accent().alpha(0.6)))
                .child(col().min_w(0.0).gap(2.0).child(text(user).font(t(13.0, 700.0)).color(white(0.92))).child(text(rendered.clone()).font(t(12.0, 400.0)).color(white(0.6)).truncate())))
            .child(caps("On your profile", 0.4).pt(10.0))
            .child(col().w_full().gap(12.0).padding(14.0).radius(12.0).fill(white(0.05))
                .child(text(format!("Listening to {rendered}")).font(t(13.0, 500.0)).color(white(0.8)).wrap())
                .child(row().w_full().gap(12.0).child(art)
                    .child(col().grow(1.0).min_w(0.0).gap(2.0)
                        .child(text(title).font(t(13.5, 700.0)).color(white(0.92)).truncate())
                        .child(text(format!("by {artist}{}", if album.is_empty() { String::new() } else { format!(" on {album}") })).font(t(12.0, 400.0)).color(white(0.7)).truncate())
                        .child(text(quality).font(t(11.0, 400.0)).color(white(0.5)).truncate()))));
    }
    c
}

/// DiscordTab's rendered status: the tags filled in, or "TIDAL via SONE" when that's empty.
fn render_status(tpl: &str, title: &str, artist: &str, album: &str) -> String {
    let r = tpl.replace("{track}", title).replace("{artist}", artist).replace("{album}", album);
    let r = r.trim();
    if r.is_empty() { "TIDAL via SONE".into() } else { r.to_string() }
}

fn oklch_warn() -> rune::Color {
    rune::oklch(0.8, 0.15, 80.0)
}

// ------------------------------------------------------------------ MCP

fn mcp_apply(f: &mut Form, r: Result<Value, String>, regen: bool) {
    f.mcp_busy = false;
    match r {
        Ok(v) => {
            f.mcp = Server::from(v);
            f.mcp_on = f.mcp.enabled;
            f.mcp_revealed = false;
        }
        Err(e) => {
            if regen {
                f.mcp_regen_error = Some(describe(&e, "Failed to regenerate token"));
            } else {
                f.mcp_on = !f.mcp_on;
                f.mcp_error = Some(describe(&e, "Failed to update MCP server"));
            }
            // keep the last known state, or what the backend says now
            run(async { mcp::mcp_get_connection_info(st()).await.ok().and_then(|i| serde_json::to_value(i).ok()) }, |f, v| {
                if let Some(v) = v {
                    f.mcp = Server::from(v);
                    f.mcp_on = f.mcp.enabled;
                }
            });
        }
    }
}

fn mcp_tab(s: &App) -> El {
    let f = &s.nav.set;
    let on = f.mcp_on;
    let busy = f.mcp_busy;
    let toggle = switch_row("mcp-enabled", "MCP server", "Expose SONE to AI clients that support MCP", on, None, move |s: &mut App| {
        let f = &mut s.nav.set;
        if f.mcp_busy {
            return;
        }
        f.mcp_busy = true;
        f.mcp_on = !on;
        f.mcp_revealed = false;
        f.mcp_error = None;
        f.mcp_confirm = false;
        run(async move { mcp::mcp_set_enabled(st(), crate::host().clone(), !on).await.map_err(|e| e.to_string()).and_then(|i| serde_json::to_value(i).map_err(|e| e.to_string())) }, |f, r| mcp_apply(f, r, false));
    });
    let errored = on && f.mcp.url.is_none();
    let state = if errored { ("Server error", Some(theme::danger())) } else if on { ("Running", Some(theme::accent())) } else { ("Stopped", None) };
    let port = f.mcp.port.map(|p| p.to_string()).unwrap_or_else(|| st().load_settings().map(|x| x.mcp_port.to_string()).unwrap_or_else(|| "5577".into()));
    let mut c = col().w_full().gap(10.0)
        .child(section("MCP"))
        .child(card(vec![toggle]))
        .child(status_banner(&format!("127.0.0.1:{port}"), state));
    if let Some(e) = &f.mcp_error {
        c = c.child(error_line(e));
    }
    match (&f.mcp.url, on) {
        (Some(url), true) => {
            // http://127.0.0.1:{port}/{token}/mcp: the token is the first path segment
            let token = url.split("://").nth(1).and_then(|r| r.split('/').nth(1)).unwrap_or("").to_string();
            let masked = "•".repeat(if token.is_empty() { 12 } else { token.chars().count() });
            let shown = if token.is_empty() { url.clone() } else { url.replace(&token, if f.mcp_revealed { &token } else { &masked }) };
            let snippet = |u: &str| format!("{{\n  \"mcpServers\": {{\n    \"sone\": {{\n      \"type\": \"http\",\n      \"url\": \"{u}\"\n    }}\n  }}\n}}");
            let revealed = f.mcp_revealed;
            let line = |label: &str, value: El| row().w_full().py(10.0).gap(12.0).items_center().child(caps(label, 0.4).w(84.0)).child(value);
            let reveal = small_button("mcp-reveal", if revealed { "Hide token" } else { "Reveal token" }, true).on_press(move |s: &mut App| s.nav.set.mcp_revealed = !revealed);
            c = c.child(caps("Connection details", 0.45).pt(6.0))
                .child(col().w_full().px(13.0).radius(11.0).fill(white(0.04)).border(1.0, white(0.06))
                    .child(line("Transport", mono("Streamable HTTP")))
                    .child(line("URL", row().grow(1.0).min_w(0.0).gap(6.0).items_center().child(mono(&shown).truncate().grow(1.0).min_w(0.0)).child(reveal).child(copy_button(f, "mcp-copy", "Copy", url.clone()))))
                    .child(line("Port", mono(&port))))
                .child(row().w_full().pt(6.0).items_center().child(caps("Client config", 0.45)).child(spacer()).child(copy_button(f, "mcp-snippet-copy", "Copy", snippet(url))))
                .child(col().w_full().padding(12.0).radius(10.0).fill(white(0.04)).border(1.0, white(0.06)).child(text(snippet(&shown)).font(ui::tnum(t(11.5, 500.0))).color(white(0.75))))
                .child(note("Standard mcpServers format — drop into Claude, Cursor, Cline, Windsurf, or any MCP client."));
            if f.mcp_confirm {
                // React asks with window.confirm.
                c = c.child(col().key("mcp-confirm").role("group").label("Regenerate token?").w_full().padding(13.0).gap(10.0).radius(11.0).fill(white(0.05)).border(1.0, white(0.1))
                    .child(text("Regenerate token? Any connected MCP clients will need to reconnect with the new URL.").font(t(12.5, 500.0)).color(white(0.85)).wrap())
                    .child(row().gap(8.0)
                        .child(small_button("mcp-regen-cancel", "Cancel", true).on_press(|s: &mut App| s.nav.set.mcp_confirm = false))
                        .child(small_button("mcp-regen-ok", "Regenerate", true).on_press(|s: &mut App| {
                            let f = &mut s.nav.set;
                            f.mcp_confirm = false;
                            f.mcp_busy = true;
                            f.mcp_regen_error = None;
                            run(async { mcp::mcp_regenerate_token(st(), crate::host().clone()).await.map_err(|e| e.to_string()).and_then(|i| serde_json::to_value(i).map_err(|e| e.to_string())) }, |f, r| mcp_apply(f, r, true));
                        }))));
            } else {
                let regen = small_button("mcp-regen", "Regenerate token", !busy);
                c = c.child(row().w_full().pt(6.0).items_center()
                    .child(regen.on_press(|s: &mut App| s.nav.set.mcp_confirm = !s.nav.set.mcp_busy))
                    .child(spacer()).child(note("Disconnects connected clients")));
            }
            if let Some(e) = &f.mcp_regen_error {
                c = c.child(error_line(e));
            }
        }
        _ => {
            c = c.child(note(if on { "Couldn't reach the server — toggle it off and on to retry." } else { "Enable the server to get a connection URL." }));
        }
    }
    c
}

// ------------------------------------------------------------------ OBS overlay

fn ov_apply(f: &mut Form, r: Result<Value, String>, what: &str) {
    f.ov_busy = false;
    match r {
        Ok(v) => {
            f.ov = Server::from(v);
            f.ov_on = f.ov.enabled;
            match what {
                "host" => f.ov_host = TextEdit::new(&f.ov.host),
                "port" => f.ov_port = TextEdit::new(&f.ov.port.map(|p| p.to_string()).unwrap_or_else(|| f.ov_port.text.clone())),
                _ => {}
            }
        }
        Err(e) => {
            match what {
                "host" => f.ov_host_error = Some(describe(&e, "Invalid host address")),
                "port" => f.ov_port_error = Some(describe(&e, "Failed to apply port")),
                _ => {
                    f.ov_on = !f.ov_on;
                    f.ov_error = Some(describe(&e, "Failed to update overlay server"));
                }
            }
            run(async { overlay::overlay_get_connection_info(st()).await.ok().and_then(|i| serde_json::to_value(i).ok()) }, |f, v| {
                if let Some(v) = v {
                    f.ov = Server::from(v);
                    f.ov_on = f.ov.enabled;
                }
            });
        }
    }
}

/// OverlayTab applyHost: always overlay_set_host, which validates, saves, and restarts the
/// server only if it's running.
fn apply_host(s: &mut App) {
    let f = &mut s.nav.set;
    if f.ov_busy {
        return;
    }
    let host = f.ov_host.text.trim().to_string();
    if host.is_empty() {
        f.ov_host_error = Some("Host cannot be empty".into());
        return;
    }
    f.ov_host_error = None;
    f.ov_busy = true;
    run(async move { overlay::overlay_set_host(st(), host).await.map_err(|e| e.to_string()).and_then(|i| serde_json::to_value(i).map_err(|e| e.to_string())) }, |f, r| ov_apply(f, r, "host"));
}

fn apply_port(s: &mut App) {
    let f = &mut s.nav.set;
    if f.ov_busy {
        return;
    }
    let port = f.ov_port.text.trim().parse::<u32>().ok().filter(|p| (1024..=65535).contains(p));
    let Some(port) = port else {
        f.ov_port_error = Some("Port must be between 1024 and 65535".into());
        return;
    };
    f.ov_port_error = None;
    f.ov_busy = true;
    run(async move { overlay::overlay_set_port(st(), port as u16).await.map_err(|e| e.to_string()).and_then(|i| serde_json::to_value(i).map_err(|e| e.to_string())) }, |f, r| ov_apply(f, r, "port"));
}

fn overlay_tab(s: &App) -> El {
    let f = &s.nav.set;
    let on = f.ov_on;
    let busy = f.ov_busy;
    let toggle = switch_row("ov-enabled", "Browser source overlay", "Expose a local page for OBS/Streamlabs — add it as a Browser Source", on, None, move |s: &mut App| {
        let f = &mut s.nav.set;
        if f.ov_busy {
            return;
        }
        f.ov_busy = true;
        f.ov_on = !on;
        f.ov_error = None;
        run(async move { overlay::overlay_set_enabled(st(), !on).await.map_err(|e| e.to_string()).and_then(|i| serde_json::to_value(i).map_err(|e| e.to_string())) }, |f, r| ov_apply(f, r, "enabled"));
    });
    let errored = on && f.ov.url.is_none();
    let state = if errored { ("Server error", Some(theme::danger())) } else if on { ("Running", Some(theme::accent())) } else { ("Stopped", None) };
    let addr = format!("{}:{}/overlay", if f.ov.host.is_empty() { "127.0.0.1" } else { &f.ov.host }, f.ov.port.unwrap_or(5578));
    let mut c = col().w_full().gap(10.0)
        .child(section("OBS Overlay"))
        .child(card(vec![toggle]))
        .child(status_banner(&addr, state));
    if let Some(e) = &f.ov_error {
        c = c.child(error_line(e));
    }
    match (&f.ov.url, on) {
        (Some(url), true) => {
            c = c.child(caps("Browser source URL", 0.45).pt(6.0))
                .child(row().w_full().px(13.0).py(8.0).gap(12.0).radius(11.0).fill(white(0.04)).border(1.0, white(0.06)).items_center()
                    .child(mono(url).truncate().grow(1.0).min_w(0.0)).child(copy_button(f, "ov-copy", "Copy", url.clone())))
                .child(col().w_full().padding(12.0).radius(11.0).fill(white(0.04)).border(1.0, white(0.06))
                    .child(note("In OBS: Sources → + → Browser. Paste the URL above and set the size to 400 × 120.")));
        }
        _ => c = c.child(note(if on { "Couldn't start the server — try toggling it off and on." } else { "Enable the overlay to get a URL for OBS." })),
    }
    let host_apply = small_button("ov-apply", "Apply", !busy);
    let port_apply = small_button("ov-port-apply", "Apply", !busy);
    let host_field = text_input(&f.ov_host, t(13.0, 500.0).lh(18.0)).key("ov-host").label("127.0.0.1").single_line().grow(1.0).color(white(0.95)).caret(white(0.95), white(0.25))
        .placeholder("127.0.0.1", white(0.4)).on_edit(|s: &mut App| &mut s.nav.set.ov_host).on_submit(|s: &mut App, _| apply_host(s));
    let port_field = text_input(&f.ov_port, t(13.0, 500.0).lh(18.0)).key("ov-port").label("Port").single_line().grow(1.0).color(white(0.95)).caret(white(0.95), white(0.25))
        .on_edit(|s: &mut App| &mut s.nav.set.ov_port).on_submit(|s: &mut App, _| apply_port(s));
    let boxed = |e: El, w: Option<f32>| {
        let r = row().h(34.0).px(12.0).radius(8.0).items_center().fill(white(0.06)).border(1.0, white(0.06)).child(e);
        match w { Some(w) => r.w(w), None => r.grow(1.0).min_w(0.0) }
    };
    c = c.child(caps("Network interface", 0.45).pt(6.0))
        .child(row().w_full().gap(8.0).items_center().child(boxed(host_field, None)).child(host_apply.on_press(apply_host)));
    if let Some(e) = &f.ov_host_error {
        c = c.child(error_line(e));
    }
    c = c.child(note("127.0.0.1 = local only  ·  0.0.0.0 = all interfaces (accessible on LAN)"))
        .child(caps("Port", 0.45).pt(6.0))
        .child(row().gap(8.0).items_center().child(boxed(port_field, Some(110.0))).child(port_apply.on_press(apply_port)));
    if let Some(e) = &f.ov_port_error {
        c = c.child(error_line(e));
    }
    c.child(note("Default: 5578. Changing the port restarts the server."))
}

// ------------------------------------------------------------------ Themes

fn themes_tab(_s: &App) -> El {
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

// ------------------------------------------------------------------ Utilities

fn utilities_tab(s: &App) -> El {
    let f = &s.nav.set;
    let on = f.logging;
    let mut logs = vec![switch_row("ut-logging", "Write logs to disk", "Helps when reporting bugs. ~12 MB cap.", on, None, move |s: &mut App| {
        s.nav.set.logging = !on;
        let _ = utility::set_enable_logging(!on);
    })];
    if on {
        logs.push(col().w_full().px(16.0).pb(14.0).gap(7.0)
            .child(caps("Log folder", 0.45))
            .child(row().w_full().gap(8.0).items_center()
                .child(row().grow(1.0).min_w(0.0).px(11.0).py(8.0).radius(9.0).fill(white(0.05)).border(1.0, white(0.06)).child(mono("~/.config/sone/logs").truncate()))
                .child(small_button("ut-open", "Open folder", true).on_press(|_: &mut App| {
                    if let Err(e) = utility::open_log_folder() {
                        log::warn!("[rune] open_log_folder: {e}");
                    }
                }))));
    }
    col().w_full().gap(10.0)
        .child(section("Utilities"))
        .child(card(logs))
        .child(card(vec![setting_row("Refresh App", "Clear cache and reload the application", small_button("ut-refresh", "Refresh", true).on_press(|s: &mut App| {
            s.nav.settings_open = false;
            crate::shell::refresh_app(s);
        }))]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_typed_proxy_settings_are_withheld() {
        let mut p = tauri_app_lib::ProxySettings { enabled: true, host: "1".into(), port: 0, ..Default::default() };
        assert!(!should_submit_proxy(&p));
        p.port = 8080;
        assert!(should_submit_proxy(&p));
        p.host = "  ".into();
        assert!(!should_submit_proxy(&p));
        // turning it off is always allowed: it's the way out of a bad proxy
        p.enabled = false;
        assert!(should_submit_proxy(&p));
    }

    #[test]
    fn discord_status_renders_its_tags() {
        assert_eq!(render_status("{track} by {artist} on {album}", "T", "A", "B"), "T by A on B");
        assert_eq!(render_status("  ", "T", "A", "B"), "TIDAL via SONE");
        assert_eq!(render_status("{artist} - {track}", "T", "A", ""), "A - T");
    }
}
