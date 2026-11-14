# Tools for the verifier spikes. Usage: nix-shell verifier/harness/shell.nix --run '...'
# nixpkgs is the revision ../../flake.lock pins (mitmproxy fails its tests on the floating one).
{ pkgs ? import (builtins.getFlake "github:NixOS/nixpkgs/${(builtins.fromJSON (builtins.readFile ../../flake.lock)).nodes.nixpkgs.locked.rev}") { } }:
let
  py = pkgs.python3.withPackages (p: [
    p.mitmproxy
    p.cryptography
    p.pyatspi
    p.pygobject3
    p.dbus-python
    p.numpy
    p.pywayland
  ]);
in
pkgs.mkShell {
  # bench/wheel.py generates its virtual-pointer bindings from this XML
  SPIKE_WLR_PROTOCOLS = "${pkgs.wlr-protocols}/share/wlr-protocols";
  # bench/cover.py (a plain xdg-shell window that covers or unfocuses the app) uses xdg-shell
  SPIKE_WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";
  # channels/keyboard.py compiles its virtual keyboard's keymap with this
  SPIKE_XKBCOMMON = "${pkgs.libxkbcommon}/lib/libxkbcommon.so.0";
  packages = [
    py
    pkgs.at-spi2-core
    pkgs.sway
    pkgs.wtype
    pkgs.dbus
    pkgs.flac
    pkgs.sox
    pkgs.alsa-utils
    pkgs.gst_all_1.gstreamer
    pkgs.strace
    pkgs.jq
    pkgs.curl
    pkgs.grim
    pkgs.pipewire
    pkgs.wireplumber
    pkgs.bubblewrap
    pkgs.ffmpeg-headless
  ];
  SPIKE_PW_ALSA_DIR = "${pkgs.pipewire}/lib/alsa-lib";
  SPIKE_DBUS_CONF = "${pkgs.dbus}/share/dbus-1/session.conf";
  SPIKE_ATSPI_LAUNCHER = "${pkgs.at-spi2-core}/libexec/at-spi-bus-launcher";
  SPIKE_XDG_DATA_DIRS = "${pkgs.at-spi2-core}/share:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}";
  GST_PLUGIN_SYSTEM_PATH_1_0 = pkgs.lib.makeSearchPath "lib/gstreamer-1.0" [
    pkgs.gst_all_1.gstreamer
    pkgs.gst_all_1.gst-plugins-base
    pkgs.gst_all_1.gst-plugins-good
  ];
  GI_TYPELIB_PATH = pkgs.lib.makeSearchPath "lib/girepository-1.0" [
    pkgs.at-spi2-core
    pkgs.glib
    pkgs.gobject-introspection
  ];
}
