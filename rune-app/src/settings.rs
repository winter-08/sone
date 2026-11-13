//! The Settings modal (React SettingsSheet and its tabs). It reads the backend's saved
//! settings directly, so it isn't a region: main::view builds it every frame while it's open.

use crate::App;
use crate::controller::{self, Cmd};
use crate::glyph::G;
use crate::theme;
use crate::ui::{self, caps, t, white};
use crate::views::{heading, open_url};
use rune::{El, TextEdit, col, el, row, text, text_input};

/// [`ui::pill`], primary or secondary.
fn button<S: 'static>(key: &str, label: &str, g: Option<G>, primary: bool) -> El {
    ui::pill(key, label, g, if primary { ui::Pill::Primary } else { ui::Pill::Secondary })
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

/// One option of a settings choice, the chosen one marked with a filled radio. (Its accessible
/// state isn't set: `checked` would make AT-SPI report this button as a toggle button.)
fn choice(key: &str, label: &str, selected: bool, on: impl Fn(&mut App) + 'static) -> El {
    theme::focusable_row(row().key(key.to_string()).role("button").label(label)).focus_selected(selected).w_full().h(48.0).px(14.0).gap(12.0).radius(10.0).items_center()
        .fill(if selected { white(0.12) } else { white(0.05) }).hover(white(0.1)).cursor(rune::Cursor::Pointer)
        .on_press(on)
        .child(ui::radio_mark(selected))
        .child(text(label.to_string()).font(t(13.0, 500.0)).color(white(0.9)).a11y_hidden())
}

fn field(s: &App, key: &str, label: &str) -> El {
    // A field that isn't wired to a setting yet (still real and named).
    let _ = s;
    let edit = rune::TextEdit::new("");
    row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).w(320.0)
        .child(text_input(&edit, t(13.0, 500.0).lh(18.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95)).placeholder(label, white(0.4)))
}

/// A settings field bound to one of the Nav edits; `changed` runs on every edit.
fn bound_field(edit: &TextEdit, key: &str, label: &str, get: fn(&mut App) -> &mut TextEdit, changed: fn(&mut App)) -> El {
    row().h(40.0).px(12.0).radius(10.0).items_center().fill(white(0.06)).w(320.0)
        .child(text_input(edit, t(13.0, 500.0).lh(18.0)).key(key.to_string()).label(label).single_line().grow(1.0).color(white(0.95)).placeholder(label, white(0.4))
            .on_edit(get)
            .on_input_change(move |s: &mut App, _| changed(s)))
}

/// Fill the editable fields from what's saved (as the React tabs do on mount).
pub fn load_setting_fields(s: &mut App) {
    use sone_host_shim::Manager;
    let st = crate::host().state::<tauri_app_lib::AppState>();
    if let Some(set) = st.load_settings() {
        s.nav.discord_status = TextEdit::new(&set.discord_status_text);
        s.nav.overlay_host = TextEdit::new(&set.overlay_host);
        s.nav.proxy_user = TextEdit::new(set.proxy.username.as_deref().unwrap_or(""));
    }
}

/// Run the debounced saves that are due (called from App::poll).
pub fn flush_settings(s: &mut App) {
    use sone_host_shim::Manager;
    let now = std::time::Instant::now();
    if s.nav.discord_due.is_some_and(|t| now >= t) {
        s.nav.discord_due = None;
        let text = s.nav.discord_status.text.clone();
        let _ = tauri_app_lib::commands::utility::set_discord_status_text(crate::host().state::<tauri_app_lib::AppState>(), text);
    }
    if s.nav.proxy_due.is_some_and(|t| now >= t) {
        s.nav.proxy_due = None;
        let st = crate::host().state::<tauri_app_lib::AppState>();
        let mut p = tauri_app_lib::commands::utility::get_proxy_settings(st);
        let user = s.nav.proxy_user.text.trim().to_string();
        p.username = (!user.is_empty()).then_some(user);
        // React's shouldSubmitProxy: half-typed enabled settings are withheld.
        if !p.enabled || (!p.host.trim().is_empty() && p.port > 0) {
            let h = crate::host().clone();
            sone_host_shim::async_runtime::spawn(async move {
                let _ = tauri_app_lib::commands::utility::set_proxy_settings(h.state::<tauri_app_lib::AppState>(), p).await;
            });
        }
    }
}

/// The settings modal, when open.
pub fn settings_modal(s: &App) -> Option<El> {
    if !s.nav.settings_open {
        return None;
    }
    let head = heading("Settings", theme::PAGE_TITLE);
    Some(ui::dialog_frame("settings-layer", "settings-modal", "Settings", ("settings-close", "Close settings"), 860.0, Some(620.0), Some(head), settings_page(s), |s: &mut App| s.nav.settings_open = false))
}

/// The settings tabs and the chosen tab's options, under a "Settings" title that the caller
/// draws. The tab column stays in view while the options scroll.
pub fn settings_page(s: &App) -> El {
    let tab = |key: &str, label: &str, which: SettingsTab| {
        let on = s.nav.settings_tab == which;
        theme::focusable_row(row().key(key.to_string()).role("button").label(label)).focus_selected(on).h(34.0).px(12.0).radius(10.0).items_center().w_full()
            .fill(if on { white(0.1) } else { white(0.0) }).hover(white(0.06)).cursor(rune::Cursor::Pointer)
            .on_press(move |s: &mut App| s.nav.settings_tab = which)
            .child(text(label.to_string()).font(t(13.0, 600.0)).color(white(0.85)).a11y_hidden())
    };
    let tabs = col().w(200.0).gap(2.0).focus_group()
        .child(tab("set-playback", "Playback", SettingsTab::Playback))
        .child(tab("set-general", "General", SettingsTab::General))
        .child(tab("set-network", "Network", SettingsTab::Network))
        .child(tab("set-scrobbling", "Scrobbling", SettingsTab::Scrobbling))
        .child(tab("set-discord", "Discord", SettingsTab::Discord))
        .child(tab("set-mcp", "MCP", SettingsTab::Mcp))
        .child(tab("set-overlay", "OBS Overlay", SettingsTab::Overlay))
        .child(tab("set-themes", "Themes", SettingsTab::Themes))
        .child(tab("set-utilities", "Utilities", SettingsTab::Utilities));
    let quality = controller::max_quality();
    let body = match s.nav.settings_tab {
        SettingsTab::Playback => {
            let q = |key: &str, label: &str, tier: &'static str| choice(key, label, quality == tier, move |_: &mut App| controller::send(Cmd::SetMaxQuality(tier.to_string())));
            col().key("q-group").role("group").labelled_by("q-caption").w_full().gap(8.0).focus_group()
                .child(caps("Streaming quality", 0.5).key("q-caption"))
                .child(q("q-hires", "HI-RES LOSSLESS FLAC · up to 24-bit / 192 kHz", "HI_RES_LOSSLESS"))
                .child(q("q-lossless", "LOSSLESS FLAC · 16-bit / 44.1 kHz", "LOSSLESS"))
                .child(q("q-high", "HIGH AAC · up to 320 kbps", "HIGH"))
        }
        SettingsTab::General => col().key("tb-group").role("group").labelled_by("tb-caption").w_full().gap(8.0).focus_group()
            .child(caps("Title bar", 0.5).key("tb-caption"))
            .child(choice("tb-custom", "Custom SONE's own in-app title bar, themed to match.", true, |_: &mut App| {}))
            .child(choice("tb-hidden", "Hidden Frameless — no title bar at all.", false, |_: &mut App| {}))
            .child(choice("tb-system", "System Native OS title bar and window controls.", false, |_: &mut App| {})),
        SettingsTab::Network => col().w_full().gap(10.0)
            .child(caps("Proxy", 0.5))
            .child(button::<App>("px-http", "HTTP", None, false))
            .child(field(s, "px-host", "host"))
            .child(field(s, "px-port", "port"))
            .child(bound_field(&s.nav.proxy_user, "px-user", "Username", |s: &mut App| &mut s.nav.proxy_user,
                |s: &mut App| s.nav.proxy_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(500))))
            .child(button::<App>("px-test", "Test connection", None, false)),
        SettingsTab::Scrobbling => {
            use sone_host_shim::Manager;
            let connected = crate::host().state::<tauri_app_lib::AppState>().load_settings()
                .and_then(|x| x.scrobble.listenbrainz.map(|c| c.username));
            let mut lb = if let Some(user) = connected {
                col().w_full().gap(10.0).child(caps("ListenBrainz", 0.5))
                    .child(text(format!("Connected as {user}")).font(t(13.0, 500.0)).color(white(0.8)))
                    .child(button::<App>("lb-disconnect", "Disconnect", None, false).on_press(|_: &mut App| {
                        let h = crate::host().clone();
                        sone_host_shim::async_runtime::spawn(async move {
                            let _ = tauri_app_lib::commands::scrobble::disconnect_provider(h.state::<tauri_app_lib::AppState>(), "listenbrainz".into()).await;
                            controller::bump();
                        });
                    }))
            } else { col().w_full().gap(10.0)
                .child(caps("ListenBrainz", 0.5))
                .child(bound_field(&s.nav.lb_token, "lb-token", "User token", |s: &mut App| &mut s.nav.lb_token, |_: &mut App| {}))
                .child(theme::focusable(row().role("button").label("listenbrainz.org/settings")).cursor(rune::Cursor::Pointer).on_press(|_: &mut App| open_url("https://listenbrainz.org/settings/"))
                    .child(text("listenbrainz.org/settings").font(t(12.0, 500.0)).color(white(0.6)).a11y_hidden()))
                .child(button::<App>("lb-connect", "Connect", None, true).on_press(|s: &mut App| {
                    use sone_host_shim::Manager;
                    let token = s.nav.lb_token.text.trim().to_string();
                    if token.is_empty() {
                        return;
                    }
                    let h = crate::host().clone();
                    sone_host_shim::async_runtime::spawn(async move {
                        let _ = tauri_app_lib::commands::scrobble::connect_listenbrainz(h.state::<tauri_app_lib::AppState>(), token).await;
                        controller::bump(); // saved settings changed: redraw the tab
                    });
                }))
            };
            if let Some(st) = &s.nav.lb_status {
                lb = lb.child(text(st.clone()).font(t(12.0, 500.0)).color(white(0.6)));
            }
            // Last.fm and Libre.fm come first, each with its own Connect (browser sign-in).
            col().w_full().gap(18.0)
                .child(col().gap(8.0).child(caps("Last.fm", 0.5)).child(button::<App>("lf-connect", "Connect", None, false)))
                .child(col().gap(8.0).child(caps("Libre.fm", 0.5)).child(button::<App>("lbfm-connect", "Connect", None, false)))
                .child(lb)
        }
        SettingsTab::Discord => {
            let chip = |key: &str, label: &str| row().key(key.to_string()).role("button").label(label).h(26.0).px(10.0).radius(13.0).items_center().fill(white(0.08)).cursor(rune::Cursor::Pointer)
                .child(text(label.to_string()).font(t(11.0, 600.0)).color(white(0.85)).a11y_hidden());
            col().w_full().gap(10.0)
                .child(caps("Status text", 0.5))
                .child(bound_field(&s.nav.discord_status, "dc-status", "{track} by {artist} on {album}", |s: &mut App| &mut s.nav.discord_status,
                    |s: &mut App| s.nav.discord_due = Some(std::time::Instant::now() + std::time::Duration::from_millis(500))))
                .child(row().gap(6.0).child(chip("dc-track", "{track}")).child(chip("dc-artist", "{artist}")).child(chip("dc-album", "{album}")))
        }
        SettingsTab::Mcp => col().w_full().gap(10.0)
            .child(caps("MCP server", 0.5))
            .child(row().gap(8.0)
                .child(button::<App>("mcp-reveal", "Reveal token", None, false))
                .child(button::<App>("mcp-copy", "Copy", None, false))
                .child(button::<App>("mcp-regen", "Regenerate token", None, false))),
        SettingsTab::Overlay => col().w_full().gap(10.0)
            .child(caps("Overlay server", 0.5))
            .child(bound_field(&s.nav.overlay_host, "ov-host", "127.0.0.1", |s: &mut App| &mut s.nav.overlay_host, |_: &mut App| {}))
            .child(button::<App>("ov-apply", "Apply", None, true).on_press(|s: &mut App| {
                // As OverlayTab's applyHost: always overlay_set_host, which validates, saves, and
                // restarts the server only if it's running.
                use sone_host_shim::Manager;
                let host = s.nav.overlay_host.text.trim().to_string();
                if !host.is_empty() {
                    let h = crate::host().clone();
                    sone_host_shim::async_runtime::spawn(async move {
                        let _ = tauri_app_lib::commands::overlay::overlay_set_host(h.state::<tauri_app_lib::AppState>(), host).await;
                    });
                }
            })),
        SettingsTab::Themes => {
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
        SettingsTab::Utilities => col().w_full().gap(10.0)
            .child(caps("Cache and logs", 0.5))
            .child(row().gap(8.0)
                .child(button::<App>("ut-open", "Open folder", None, false))
                .child(button::<App>("ut-refresh", "Refresh", None, false))),
    };
    row().w_full().gap(28.0).items_start().child(tabs.sticky_top(0.0)).child(col().grow(1.0).min_w(0.0).child(body))
}
