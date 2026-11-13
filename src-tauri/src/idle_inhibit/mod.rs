//! Cross-display-server idle/screen-blanking inhibition.
//!
//! Probes the active display server and runs every applicable layer additively:
//! Wayland native (zwp_idle_inhibit), X11 native (screensaver+DPMS), D-Bus
//! (ScreenSaver/GNOME/login1), and the XDG portal as a last-resort fallback.

mod dbus;
#[cfg(feature = "tauri-ui")]
mod wayland;
mod x11;

/// The Wayland layer needs the webview's GTK surface. Without a webview (the `host` build)
/// there is none, so it never starts; the D-Bus and portal layers still run.
#[cfg(not(feature = "tauri-ui"))]
mod wayland {
    pub struct WaylandInhibitor;
    impl WaylandInhibitor {
        pub fn start(window: &tauri::WebviewWindow) -> Option<Self> {
            match *window {}
        }
        pub fn stop(self) {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    Wayland,
    X11,
    Unknown,
}

impl DisplayServer {
    /// Detect from the environment. Wayland wins if WAYLAND_DISPLAY is set
    /// (apps can have both set under Xwayland).
    pub fn detect() -> Self {
        Self::detect_from(
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            std::env::var_os("DISPLAY").is_some(),
        )
    }

    fn detect_from(wayland: bool, x11: bool) -> Self {
        if wayland {
            DisplayServer::Wayland
        } else if x11 {
            DisplayServer::X11
        } else {
            DisplayServer::Unknown
        }
    }
}

pub struct IdleInhibitor {
    active: bool,
    dbus: dbus::DbusInhibitor,
    x11: Option<x11::X11Inhibitor>,
    wayland: Option<wayland::WaylandInhibitor>,
}

impl IdleInhibitor {
    pub fn new() -> Self {
        Self {
            active: false,
            dbus: dbus::DbusInhibitor::new(),
            x11: None,
            wayland: None,
        }
    }

    /// Inhibit screen blanking + system sleep. Runs every applicable layer for
    /// the detected display server — no short-circuit.
    pub async fn inhibit(&mut self, window: &tauri::WebviewWindow) {
        self.inhibit_layers(Some(window)).await;
    }

    /// `inhibit` for a native UI with no webview window (the `host` build): every layer but the
    /// Wayland one, which needs the webview's GTK surface. The D-Bus and portal layers keep the
    /// screen on there.
    #[cfg(not(feature = "tauri-ui"))]
    pub async fn inhibit_without_window(&mut self) {
        self.inhibit_layers(None).await;
    }

    async fn inhibit_layers(&mut self, window: Option<&tauri::WebviewWindow>) {
        if self.active {
            return;
        }
        self.active = true;

        let server = DisplayServer::detect();
        log::info!("idle inhibit requested (display server: {server:?})");

        // Layer 1: display-server native.
        match server {
            DisplayServer::Wayland => {
                self.wayland = window.and_then(wayland::WaylandInhibitor::start);
            }
            DisplayServer::X11 => {
                self.x11 = x11::X11Inhibitor::start();
            }
            DisplayServer::Unknown => {}
        }

        // Layer 2: D-Bus session (KDE/GNOME screen-off) — always attempt; harmless
        // where unsupported. Returns whether a screen-keeping inhibition landed.
        let dbus_screen = self.dbus.inhibit_screen().await;

        // Layer 3: D-Bus system suspend (logind) — always attempt.
        self.dbus.inhibit_sleep().await;

        // Layer 4: portal fallback — only if NOTHING screen-keeping succeeded.
        let native_screen = self.wayland.is_some() || self.x11.is_some();
        if !native_screen && !dbus_screen {
            log::warn!("no native/D-Bus screen inhibition succeeded; trying portal");
            self.dbus.inhibit_portal().await;
        }
    }

    pub async fn uninhibit(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;

        if let Some(mut x) = self.x11.take() {
            x.stop();
        }
        if let Some(w) = self.wayland.take() {
            w.stop();
        }
        self.dbus.release().await;
        log::info!("idle inhibit released");
    }
}

#[cfg(test)]
mod tests {
    use super::DisplayServer;

    #[test]
    fn wayland_wins_when_both_set() {
        assert_eq!(DisplayServer::detect_from(true, true), DisplayServer::Wayland);
    }

    #[test]
    fn x11_when_only_display_set() {
        assert_eq!(DisplayServer::detect_from(false, true), DisplayServer::X11);
    }

    #[test]
    fn wayland_when_only_wayland_set() {
        assert_eq!(DisplayServer::detect_from(true, false), DisplayServer::Wayland);
    }

    #[test]
    fn unknown_when_neither_set() {
        assert_eq!(DisplayServer::detect_from(false, false), DisplayServer::Unknown);
    }
}
