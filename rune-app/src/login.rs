//! Sign-in with the user's own client credentials (React Login's advanced view): API
//! credentials (saved ones pre-filled), then Device Code, PKCE or Token Import, and the
//! "What are Client ID and Client Secret?" help. views::login shows it in place of the simple
//! view when the user asks for it, or when no embedded credentials are built in.

use crate::App;
use crate::controller::{self, Cmd};
use crate::glyph::G;
use crate::theme;
use crate::ui::{self, t, white};
use rune::{El, TextEdit, Tx, col, el, row, spacer, text, text_input};
use sone_host_shim::Manager;
use tauri_app_lib::AppState;
use tauri_app_lib::commands::auth;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Method {
    #[default]
    Device,
    Pkce,
    Import,
}

/// The advanced view's fields.
#[derive(Default)]
pub struct Custom {
    client_id: TextEdit,
    client_secret: TextEdit,
    show_secret: bool,
    method: Method,
    curl: TextEdit,
    response: TextEdit,
    import_error: Option<String>,
    /// The token fields' masks as the user set them (None: masked once over 200 characters).
    curl_masked: Option<bool>,
    response_masked: Option<bool>,
    help: bool,
    /// The error shown for this view (the simple view's goes away when switching here).
    dismissed: bool,
}

impl Custom {
    /// Closes the help dialog; whether it was open.
    pub fn close_help(&mut self) -> bool {
        std::mem::replace(&mut self.help, false)
    }
}

/// Whether the build has embedded credentials, so the simple view can sign in at all (React
/// loadCreds: with neither the login-code nor the PKCE pair it opens on the advanced view).
pub fn has_defaults() -> bool {
    let code = auth::get_default_credentials().map(|c| !c.0.is_empty()).unwrap_or(false);
    code || auth::has_pkce_defaults().unwrap_or(false)
}

/// Open the advanced view with the saved credentials filled in.
pub fn open(s: &mut App) {
    let st = crate::host().state::<AppState>();
    let (id, secret) = auth::get_saved_credentials(st).unwrap_or_default();
    s.nav.set.custom_login = Some(Custom { client_id: TextEdit::new(&id), client_secret: TextEdit::new(&secret), ..Default::default() });
}

/// Whether a token field shows masked: as the user last set it, else once it holds more than
/// 200 characters (Login.tsx's auto-mask of long pastes).
fn mask_shown(explicit: Option<bool>, text: &str) -> bool {
    !text.is_empty() && explicit.unwrap_or(text.chars().count() > 200)
}

/// isCredentialRelatedError: an error that says the client credentials are wrong.
pub fn credential_error(e: &str) -> bool {
    let l = e.to_lowercase();
    l.contains("invalid_client") || l.contains("unauthorized") || (l.contains("client") && l.contains("not found"))
}

macro_rules! field_of {
    ($f:ident) => {
        |s: &mut App| &mut s.nav.set.custom_login.as_mut().expect("custom sign-in open").$f
    };
}

fn input(edit: &TextEdit, key: &str, label: &str, placeholder: &str, get: fn(&mut App) -> &mut TextEdit) -> El {
    text_input(edit, t(13.0, 500.0).lh(18.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95)).caret(white(0.95), white(0.25))
        .placeholder(placeholder, white(0.35)).on_edit(get)
}

fn boxed(e: El) -> El {
    row().w_full().h(40.0).px(12.0).gap(8.0).radius(8.0).items_center().fill(white(0.06)).border(1.0, white(0.08)).child(e)
}

fn small(s: &str, a: f32) -> El {
    text(s.to_string()).font(t(11.5, 400.0)).color(white(a)).wrap()
}

fn primary(key: &str, label: &str, enabled: bool) -> El {
    // Dimmed rather than disabled: its handler checks (see settings::small_button).
    ui::pill(key, label, None, ui::Pill::Primary).w_full().justify_center().opacity(if enabled { 1.0 } else { 0.4 })
}

/// A field's Paste button (React pasteTo): the clipboard's text replaces the field's.
fn paste(key: &str, get: fn(&mut App) -> &mut TextEdit) -> El {
    theme::focusable(row().key(key.to_string()).role("button").label("Paste")).h(24.0).px(8.0).radius(6.0).items_center().hover(white(0.08)).cursor(rune::Cursor::Pointer)
        .on_press_tx(move |_: &mut App, tx: &mut Tx| {
            tx.read_clipboard(move |s: &mut App, c: &rune::ClipboardContent, _| {
                let t = c.text().trim().to_string();
                if !t.is_empty() && s.nav.set.custom_login.is_some() {
                    *get(s) = TextEdit::new(&t);
                }
            });
        })
        .child(text("Paste").font(t(10.5, 600.0)).color(white(0.6)).a11y_hidden())
}

/// The advanced view. `failed` is the sign-in error to show, if any.
pub fn view(s: &App, failed: Option<&str>) -> El {
    let Some(c) = s.nav.set.custom_login.as_ref() else { return el() };
    let defaults = has_defaults();
    let id = c.client_id.text.trim().to_string();
    let has_secret = !c.client_secret.text.trim().is_empty();
    let import = c.method == Method::Import;
    let mut v = col().key("login-advanced").w_full().gap(14.0);
    if defaults {
        v = v.child(row().child(theme::focusable(row().key("login-back").role("button").label("Back")).gap(6.0).h(28.0).px(6.0).radius(6.0).items_center().hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| s.nav.set.custom_login = None)
            .child(crate::glyph::glyph(G::Back, 14.0, white(0.6)))
            .child(text("Back").font(t(13.0, 500.0)).color(white(0.6)).a11y_hidden())));
    }
    v = v.child(text("Connect your TIDAL account to start streaming").font(t(16.0, 500.0)).color(white(0.7)).wrap())
        .child(row().child(theme::focusable(row().key("login-help").role("button").label("What are Client ID and Client Secret?")).gap(6.0).h(26.0).px(4.0).items_center().cursor(rune::Cursor::Pointer)
            .on_press(|s: &mut App| {
                if let Some(c) = &mut s.nav.set.custom_login {
                    c.help = true;
                }
            })
            .child(crate::glyph::glyph(G::Info, 14.0, theme::accent()))
            .child(text("What are Client ID and Client Secret?").font(t(13.0, 500.0)).color(theme::accent()).a11y_hidden())));
    // API credentials (greyed out for Token Import, which reads them from the paste).
    let secret_input = {
        let i = input(&c.client_secret, if c.show_secret { "login-secret-shown" } else { "login-secret" }, "Client Secret", if import { "Auto-extracted from cURL" } else { "Leave blank for Lossless (CD quality)" }, field_of!(client_secret));
        if c.show_secret { i } else { i.secure() }
    };
    let show = c.show_secret;
    let mut creds = col().w_full().padding(18.0).gap(10.0).radius(12.0).fill(white(0.04)).border(1.0, white(0.07)).opacity(if import { 0.6 } else { 1.0 })
        .child(row().gap(8.0).items_center().child(crate::glyph::glyph(G::Lock, 15.0, theme::accent())).child(text("API Credentials").font(t(14.0, 600.0)).color(white(0.92))).child(spacer())
            .child(if import { small("Extracted from pasted data", 0.45) } else { el() }))
        .child(row().w_full().child(small("Client ID *", 0.6)).child(spacer()).child(small("Required", 0.4)))
        .child(boxed(input(&c.client_id, "login-client-id", "Client ID", if import { "Auto-extracted from cURL" } else { "Your TIDAL app Client ID" }, field_of!(client_id))))
        .child(row().w_full().child(small("Client Secret", 0.6)).child(spacer()).child(small("Optional — enables Hi-Res", 0.4)))
        .child(boxed(secret_input).child(theme::focusable(row().key("login-show-secret").role("button").label(if show { "Hide secret" } else { "Show secret" })).h(24.0).px(6.0).radius(6.0).items_center().cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                if let Some(c) = &mut s.nav.set.custom_login {
                    c.show_secret = !show;
                }
            })
            .child(text(if show { "Hide" } else { "Show" }).font(t(11.0, 600.0)).color(white(0.55)).a11y_hidden())));
    if !id.is_empty() && !import {
        let (tone, msg) = if has_secret {
            (theme::success(), "Hi-Res mode: Up to 24-bit/192kHz with automatic token refresh.")
        } else {
            (theme::accent(), "Lossless mode: CD quality (16-bit/44.1kHz). Add a Client Secret for Hi-Res.")
        };
        creds = creds.child(row().w_full().padding(10.0).radius(8.0).fill(tone.alpha(0.1)).border(1.0, tone.alpha(0.3)).child(text(msg).font(t(12.0, 500.0)).color(tone).wrap()));
    }
    creds = creds.child(small("Credentials are stored locally and only sent to TIDAL's auth servers.", 0.4));
    v = v.child(creds);
    // The three methods.
    let method = c.method;
    let tab = |key: &str, label: &str, m: Method| {
        let on = method == m;
        theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).grow(1.0).h(40.0).justify_center().items_center()
            .fill(if on { theme::accent().alpha(0.08) } else { white(0.0) }).hover(white(0.05)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| {
                if let Some(c) = &mut s.nav.set.custom_login {
                    c.method = m;
                    c.import_error = None;
                    c.dismissed = true;
                }
            })
            .child(text(label.to_string()).font(t(12.0, 600.0)).color(if on { theme::accent() } else { white(0.55) }).a11y_hidden())
    };
    let tabs = row().w_full().focus_group().child(tab("lc-device", "Device Code", Method::Device)).child(tab("lc-pkce", "PKCE", Method::Pkce)).child(tab("lc-import", "Token Import", Method::Import));
    let body = match method {
        Method::Device => col().w_full().gap(10.0)
            .child(small("Get a code, visit link.tidal.com, and enter it to log in. No redirect URLs needed.", 0.55))
            .child(text("Requires a native app Client ID (Android/desktop). Web player client IDs do not support this flow — use Token Import instead.").font(t(11.0, 500.0)).color(warn()).wrap())
            .child(primary("lc-device-go", "Get Login Code", !id.is_empty()).on_press(|s: &mut App| {
                let Some(c) = &mut s.nav.set.custom_login else { return };
                let (id, secret) = (c.client_id.text.trim().to_string(), c.client_secret.text.trim().to_string());
                c.dismissed = false;
                if id.is_empty() {
                    crate::interact::toast("Client ID is required.".into(), crate::interact::Kind::Error);
                    return;
                }
                controller::send(Cmd::StartDeviceLoginWith { id, secret });
            })),
        Method::Pkce => col().w_full().gap(10.0)
            .child(small("Opens TIDAL's login in your browser. After login, copy the redirect URL back here.", 0.55))
            .child(text("Requires both Client ID and Client Secret from a native app (Android redirect URI). Web player credentials will not work.").font(t(11.0, 500.0)).color(warn()).wrap())
            .child(primary("lc-pkce-go", "Login with TIDAL", !id.is_empty() && has_secret).on_press(|s: &mut App| {
                let Some(c) = &mut s.nav.set.custom_login else { return };
                let (id, secret) = (c.client_id.text.trim().to_string(), c.client_secret.text.trim().to_string());
                c.dismissed = false;
                if id.is_empty() || secret.is_empty() {
                    let msg = if id.is_empty() { "Client ID is required." } else { "Client Secret is required for PKCE." };
                    crate::interact::toast(msg.into(), crate::interact::Kind::Error);
                    return;
                }
                s.nav.pkce_paste = TextEdit::new("");
                controller::send(Cmd::StartPkceLoginWith { id, secret });
            })),
        Method::Import => {
            // React masks a field once a paste passes 200 characters (it holds tokens), with an
            // eye to show it again. (A field that starts masked needs its own key in Rune.)
            let area = |edit: &TextEdit, key: &str, label: &str, placeholder: &str, get: fn(&mut App) -> &mut TextEdit, mask: fn(&mut Custom) -> &mut Option<bool>, explicit: Option<bool>| {
                let masked = mask_shown(explicit, &edit.text);
                let field = text_input(edit, t(11.0, 500.0).lh(15.0)).key(if masked { format!("{key}-masked") } else { key.to_string() }).label(label).w_full().min_h(40.0).max_h(90.0).color(white(0.9)).caret(white(0.95), white(0.25))
                    .placeholder(placeholder, white(0.35)).on_edit(get).on_input_change(|s: &mut App, _| {
                        if let Some(c) = &mut s.nav.set.custom_login {
                            c.import_error = None;
                        }
                    });
                let field = if masked { field.secure() } else { field };
                let mut r = row().w_full().padding(8.0).gap(6.0).radius(8.0).fill(white(0.06)).border(1.0, white(0.08)).child(field);
                if !edit.text.is_empty() {
                    r = r.child(theme::focusable(row().key(format!("{key}-eye")).role("button").label(if masked { "Show" } else { "Hide" })).h(22.0).px(6.0).radius(6.0).items_center().self_start().cursor(rune::Cursor::Pointer)
                        .on_press(move |s: &mut App| {
                            if let Some(c) = &mut s.nav.set.custom_login {
                                *mask(c) = Some(!masked);
                            }
                        })
                        .child(text(if masked { "Show" } else { "Hide" }).font(t(10.5, 600.0)).color(white(0.5)).a11y_hidden()));
                }
                r
            };
            let can = !c.curl.text.trim().is_empty() || !c.response.text.trim().is_empty();
            let mut b = col().w_full().gap(10.0)
                .child(small("For web player client IDs or any client that doesn't support Device Code. Paste the cURL and response from your browser's Network tab.", 0.55))
                .child(row().w_full().items_center().child(small("cURL Command", 0.65)).child(spacer()).child(paste("lc-curl-paste", field_of!(curl))))
                .child(area(&c.curl, "lc-curl", "cURL Command", "Right-click token request → Copy as cURL", field_of!(curl), |c: &mut Custom| &mut c.curl_masked, c.curl_masked))
                .child(small("Provides Client ID.", 0.4))
                .child(row().w_full().items_center().child(small("Response Body", 0.65)).child(spacer()).child(paste("lc-response-paste", field_of!(response))))
                .child(area(&c.response, "lc-response", "Response Body", "Same request → Response tab → copy JSON", field_of!(response), |c: &mut Custom| &mut c.response_masked, c.response_masked))
                .child(small("Provides session tokens (access_token, refresh_token).", 0.4));
            if let Some(e) = &c.import_error {
                b = b.child(text(e.clone()).font(t(11.5, 500.0)).color(theme::danger()).wrap());
            }
            b.child(primary("lc-import-go", "Import Session", can).on_press(import_tokens))
        }
    };
    v = v.child(col().w_full().radius(12.0).fill(white(0.04)).border(1.0, white(0.07)).clip_rounded(12.0).child(tabs).child(el().w_full().h(1.0).fill(white(0.07))).child(col().w_full().padding(18.0).child(body)));
    if let Some(e) = failed.filter(|_| !c.dismissed) {
        v = v.child(col().w_full().padding(14.0).gap(8.0).radius(10.0).fill(theme::danger().alpha(0.12)).border(1.0, theme::danger().alpha(0.4))
            .child(text(e.to_string()).font(t(12.5, 500.0)).color(theme::danger()).wrap())
            .child(theme::focusable(row().key("login-dismiss").role("button").label("Dismiss")).justify_center().cursor(rune::Cursor::Pointer)
                .on_press(|s: &mut App| {
                    if let Some(c) = &mut s.nav.set.custom_login {
                        c.dismissed = true;
                    }
                })
                .child(text("Dismiss").font(t(12.0, 600.0)).color(theme::danger()).a11y_hidden())));
    }
    v
}

fn warn() -> rune::Color {
    rune::oklch(0.8, 0.13, 80.0)
}

/// React handleTokenImport: parse both pastes, then import the session they describe.
fn import_tokens(s: &mut App) {
    let Some(c) = &mut s.nav.set.custom_login else { return };
    let (curl, response) = (c.curl.text.trim().to_string(), c.response.text.trim().to_string());
    if curl.is_empty() && response.is_empty() {
        return;
    }
    c.dismissed = false;
    let parse = |text: &str| auth::parse_token_data(text.to_string()).ok().and_then(|p| serde_json::to_value(p).ok());
    let get = |v: &Option<serde_json::Value>, k: &str| v.as_ref().and_then(|v| v[k].as_str()).filter(|s| !s.is_empty()).map(str::to_string);
    let (mut id, mut secret, mut refresh, mut access) = (None, None, None, None);
    if !curl.is_empty() {
        let r = parse(&curl);
        id = get(&r, "clientId");
        secret = get(&r, "clientSecret");
        refresh = get(&r, "refreshToken");
        access = get(&r, "accessToken");
    }
    if !response.is_empty() {
        let r = parse(&response);
        access = get(&r, "accessToken").or(access);
        refresh = get(&r, "refreshToken").or(refresh);
        id = id.or(get(&r, "clientId"));
    }
    let Some(id) = id else {
        c.import_error = Some("Could not find a Client ID. Make sure you pasted the cURL command.".into());
        return;
    };
    if refresh.is_none() && access.is_none() {
        c.import_error = Some("No session tokens found. Make sure you pasted the Response body (JSON) from the token request.".into());
        return;
    }
    controller::send(Cmd::ImportSession { id, secret: secret.unwrap_or_default(), refresh: refresh.unwrap_or_default(), access });
}

/// The help dialog (React's Help Modal), when open.
pub fn help_dialog(s: &App) -> Option<El> {
    if !s.nav.set.custom_login.as_ref().is_some_and(|c| c.help) {
        return None;
    }
    let p = |s: &str| text(s.to_string()).font(t(13.0, 400.0)).color(white(0.7)).wrap();
    let b = |s: &str| text(s.to_string()).font(t(13.0, 650.0)).color(white(0.92));
    let body = col().w_full().gap(14.0)
        .child(p("They are OAuth application credentials used to connect to TIDAL's API. Official TIDAL apps (Android, iOS, desktop) have these built in."))
        .child(p("SONE requires credentials from a native TIDAL application (such as the Android or desktop client). Credentials from the TIDAL Developer Portal (developer.tidal.com) will not work — those are for TIDAL's public catalog API, which is a different system that does not support authentication or streaming."))
        .child(p("SONE does not provide or endorse any specific method for obtaining credentials. You may find guidance by searching online."))
        .child(b("Do I need both?"))
        .child(p("No. There are two login methods:"))
        .child(p("•  Device Code — works with Client ID alone (CD-quality lossless, 16-bit/44.1kHz). Adding Client Secret unlocks Hi-Res up to 24-bit/192kHz."))
        .child(p("•  PKCE — requires both Client ID and Client Secret. Supports Hi-Res up to 24-bit/192kHz."))
        .child(b("Are my credentials safe?"))
        .child(p("Client ID and Client Secret identify an application, not your personal account. Your TIDAL login is handled separately through TIDAL's standard OAuth 2.0 flow — the same mechanism used by all official TIDAL applications. Credentials are stored locally (encrypted at rest with AES-256-GCM) and only sent to TIDAL's authentication servers."))
        .child(el().w_full().h(1.0).fill(white(0.08)))
        .child(text("SONE is an independent, community-driven project. It is not affiliated with, endorsed by, or connected to TIDAL in any way. All content is streamed directly from TIDAL's service and requires a valid paid subscription. SONE is a streaming client only — it does not support offline downloads, and does not redistribute or circumvent protection of any content. All trademarks belong to their respective owners.").font(t(11.0, 400.0)).color(white(0.4)).wrap());
    let title = "What are Client ID and Client Secret?";
    Some(ui::dialog_frame("login-help-layer", "login-help", title, ("login-help-close", "Close dialog"), 520.0, None, Some(ui::dialog_title(title)), body, |s: &mut App| {
        if let Some(c) = &mut s.nav.set.custom_login {
            c.help = false;
        }
    }))
}

// ------------------------------------------------------------------ the simple view

/// Whether the build has the PKCE pair (has_pkce_defaults), read once: without it the simple
/// view has no method tabs and opens on Login Code (React loadCreds).
pub fn pkce_embedded() -> bool {
    static PKCE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PKCE.get_or_init(|| auth::has_pkce_defaults().unwrap_or(false))
}

/// The method the simple view shows: the chosen tab, or Login Code when PKCE isn't built in.
pub fn simple_mode(chosen: crate::views::LoginMode) -> crate::views::LoginMode {
    if pkce_embedded() { chosen } else { crate::views::LoginMode::Code }
}

/// A flow being started (React's status line with its spinner, "Starting device
/// authorization..."): the message, and the sign-in state it was pressed in, so it shows until
/// the controller's state moves on.
static STARTING: std::sync::Mutex<Option<(&'static str, controller::Login)>> = std::sync::Mutex::new(None);

/// Starts `cmd` from the simple view, with `status` under a spinner until it answers.
fn start(state: &controller::Login, status: &'static str, cmd: Cmd) {
    *STARTING.lock().unwrap() = Some((status, state.clone()));
    controller::send(cmd);
}

fn starting(state: &controller::Login) -> Option<&'static str> {
    STARTING.lock().unwrap().as_ref().filter(|(_, at)| at == state).map(|(m, _)| *m)
}

fn status_line(msg: &str) -> El {
    col().key("login-status").gap(12.0).py(18.0).items_center()
        .child(crate::glyph::glyph(G::Loader, 24.0, theme::accent()))
        .child(text(msg.to_string()).font(t(14.0, 400.0)).color(white(0.6)))
}

/// The simple view, idle (React Login's simple view): the method tabs when PKCE is built in, a
/// line saying what each method gives, the error with Dismiss, a status while a flow starts.
pub fn simple(s: &App, state: &controller::Login) -> El {
    use crate::views::LoginMode;
    let mode = simple_mode(s.nav.login_mode);
    let mut c = col().w_full().gap(14.0).items_center()
        .child(text("Connect your TIDAL account").font(t(17.0, 400.0)).color(white(0.6)));
    if pkce_embedded() {
        let mtab = |key: &str, label: &str, which: LoginMode| {
            let on = mode == which;
            theme::focusable(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(32.0).w(124.0).radius(16.0).items_center().justify_center()
                .fill(if on { theme::accent() } else { white(0.0) }).hover(if on { theme::accent() } else { white(0.06) }).cursor(rune::Cursor::Pointer)
                .on_press(move |s: &mut App| {
                    s.nav.login_mode = which;
                    if matches!(controller::view().login, controller::Login::Failed(_)) {
                        controller::send(Cmd::CancelLogin);
                    }
                })
                .child(text(label.to_string()).font(t(13.0, 500.0)).color(if on { theme::ink() } else { white(0.6) }).a11y_hidden())
        };
        c = c.child(row().gap(0.0).padding(4.0).radius(20.0).fill(white(0.05)).border(1.0, white(0.08)).focus_group()
            .child(mtab("lm-pkce", "PKCE", LoginMode::Pkce)).child(mtab("lm-code", "Login Code", LoginMode::Code)));
    }
    if let controller::Login::Failed(e) = state {
        let mut b = col().key("login-error").w_full().padding(14.0).gap(8.0).radius(10.0).fill(theme::danger().alpha(0.12)).border(1.0, theme::danger().alpha(0.4))
            .child(text(e.clone()).font(t(13.0, 400.0)).color(theme::danger()).wrap());
        if credential_error(e) {
            // React offers the advanced view when the embedded credentials look stale.
            b = b.child(theme::focusable(row().key("login-creds-stale").role("button").label("Credentials may need updating — use custom credentials")).gap(6.0).cursor(rune::Cursor::Pointer)
                .on_press(open)
                .child(crate::glyph::glyph(G::Info, 14.0, rune::oklch(0.8, 0.13, 80.0)))
                .child(text("Credentials may need updating — use custom credentials").font(t(12.5, 500.0)).color(rune::oklch(0.8, 0.13, 80.0)).a11y_hidden()));
        } else {
            b = b.child(theme::focusable(row().key("login-dismiss").role("button").label("Dismiss")).w_full().justify_center().cursor(rune::Cursor::Pointer)
                .on_press(|_: &mut App| controller::send(Cmd::CancelLogin))
                .child(text("Dismiss").font(t(12.5, 500.0)).color(theme::danger().alpha(0.85)).a11y_hidden()));
        }
        c = c.child(b);
    }
    if let Some(msg) = starting(state) {
        c = c.child(status_line(msg));
    } else {
        let st = state.clone();
        c = match mode {
            // TIDAL's login page opens in the user's browser: there is no web view here for
            // React's embedded login window, so the browser is the one PKCE flow. Both of
            // React's buttons stay (the sign-in golden pins them) and say what they do.
            LoginMode::Pkce => {
                let st2 = st.clone();
                c.child(small("Recommended. Unlocks Hi-Res / MAX quality up to 24-bit/192 kHz. Opens TIDAL's sign-in page in your browser.", 0.45).align(rune::Align::Center))
                    .child(primary("login-pkce", "Sign in with TIDAL", true).on_press(move |s: &mut App| {
                        s.nav.pkce_paste = TextEdit::new("");
                        start(&st, "Waiting for browser...", Cmd::StartPkceLogin)
                    }))
                    .child(theme::focusable(row().key("login-browser").role("button").label("Open in browser instead")).gap(6.0).h(22.0).items_center().cursor(rune::Cursor::Pointer)
                        .help("Signs in through your browser, as Sign in with TIDAL does here")
                        .on_press(move |s: &mut App| {
                            s.nav.pkce_paste = TextEdit::new("");
                            start(&st2, "Waiting for browser...", Cmd::StartPkceLogin)
                        })
                        .child(crate::glyph::glyph(G::External, 11.0, white(0.45)))
                        .child(text("Open in browser instead").font(t(12.0, 400.0)).color(white(0.45)).a11y_hidden()))
            }
            LoginMode::Code => c
                .child(small("Get a 6-character code and enter it on link.tidal.com.", 0.45).align(rune::Align::Center))
                .child(primary("login-code", "Get Login Code", true).on_press(move |_: &mut App| start(&st, "Starting device authorization...", Cmd::StartDeviceLogin)))
                .child(el().h(22.0)),
        };
    }
    c.child(el().w_full().h(1.0).fill(white(0.06)))
        .child(theme::focusable(row().key("login-custom").role("button").label("Use custom credentials")).gap(4.0).items_center().cursor(rune::Cursor::Pointer).on_press(open)
            .child(text("Use custom credentials").font(t(12.0, 500.0)).color(white(0.45)).a11y_hidden())
            .child(crate::glyph::glyph(G::Forward, 12.0, white(0.45))))
}

/// Waiting for the login code to be approved (React's device_pending step): the code, a QR
/// code of the approval link, the address and a button opening it, "Waiting for
/// authorization..." and Cancel.
pub fn waiting(code: &str, uri: &str) -> El {
    let u = uri.to_string();
    let base = uri.split('/').take(3).collect::<Vec<_>>().join("/");
    let mut card = col().w_full().padding(22.0).gap(12.0).radius(12.0).fill(white(0.04)).border(1.0, white(0.07))
        .child(text("Scan the QR code or enter this code:").font(t(14.0, 400.0)).color(white(0.6)))
        .child(row().w_full().justify_center().py(8.0).child(text(code.to_string()).font(t(34.0, 700.0).tracking(8.0)).color(white(0.97))));
    if let Some(px) = qr_pixels(uri) {
        card = card.child(row().w_full().justify_center().child(
            el().key("login-qr").size(184.0, 184.0).radius(12.0).fill(rune::Color::WHITE).padding(12.0)
                .child(el().size(160.0, 160.0).pixels_fit(&px, rune::SurfaceFit::Fill))));
    }
    card = card.child(row().w_full().justify_center().child(text(base.clone()).font(t(13.0, 400.0)).color(white(0.45))))
        .child(ui::pill("login-open", &format!("Open {base}"), Some(G::External), ui::Pill::Secondary).w_full().justify_center().on_press(move |_: &mut App| {
            let _ = std::process::Command::new("xdg-open").arg(&u).spawn();
        }));
    col().w_full().gap(16.0).items_center()
        .child(card)
        .child(row().gap(10.0).items_center()
            .child(crate::glyph::glyph(G::Loader, 16.0, white(0.45)))
            .child(text("Waiting for authorization...").font(t(13.0, 400.0)).color(white(0.45))))
        .child(theme::focusable(row().key("login-cancel").role("button").label("Cancel")).h(24.0).px(8.0).items_center().cursor(rune::Cursor::Pointer)
            .on_press(|_: &mut App| controller::send(Cmd::CancelLogin))
            .child(text("Cancel").font(t(12.0, 500.0)).color(white(0.45)).a11y_hidden()))
}

/// The QR code of `uri` as an image, 8 pixels a module (the white card around it is its margin).
fn qr_pixels(uri: &str) -> Option<rune::Pixels> {
    static CACHE: std::sync::Mutex<Option<(String, rune::Pixels)>> = std::sync::Mutex::new(None);
    let mut c = CACHE.lock().unwrap();
    if let Some((u, p)) = c.as_ref().filter(|(u, _)| u == uri) {
        let _ = u;
        return Some(p.clone());
    }
    let (side, rgba) = qr_rgba(uri, 8)?;
    let p = rune::Pixels::new();
    p.set(side, side, rgba);
    *c = Some((uri.to_string(), p.clone()));
    Some(p)
}

/// `uri` as a QR code (error correction M, as React's QRCodeSVG level="M"): its side in pixels
/// and the RGBA, black modules on white, `scale` pixels a module.
pub fn qr_rgba(uri: &str, scale: u32) -> Option<(u32, Vec<u8>)> {
    let qr = qrcodegen::QrCode::encode_text(uri, qrcodegen::QrCodeEcc::Medium).ok()?;
    let n = qr.size() as u32;
    let side = n * scale;
    let mut rgba = vec![255u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            if qr.get_module((x / scale) as i32, (y / scale) as i32) {
                let i = ((y * side + x) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
    }
    Some((side, rgba))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_qr_code_is_square_and_scaled() {
        let (side, rgba) = super::qr_rgba("https://link.tidal.com/ABCDE", 4).unwrap();
        assert_eq!(side % 4, 0);
        assert_eq!(rgba.len(), (side * side * 4) as usize);
        // a finder pattern's corner module is dark
        assert_eq!(&rgba[0..3], &[0, 0, 0]);
    }

    #[test]
    fn long_pastes_are_masked_until_shown() {
        let long = "x".repeat(201);
        assert!(!super::mask_shown(None, "short"));
        assert!(super::mask_shown(None, &long));
        assert!(!super::mask_shown(Some(false), &long));
        assert!(super::mask_shown(Some(true), "short"));
        assert!(!super::mask_shown(Some(true), ""));
    }

    #[test]
    fn credential_errors_are_recognised() {
        assert!(super::credential_error("Api error 401: {\"error\":\"invalid_client\"}"));
        assert!(super::credential_error("Client not found"));
        assert!(!super::credential_error("Network unreachable"));
    }
}
