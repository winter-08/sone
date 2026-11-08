# Build shell for the Rune frontend: SONE's backend deps (GStreamer, ALSA, D-Bus, OpenSSL) plus
# what Rune links or dlopens (Wayland/X11, xkbcommon, Vulkan/GL, fontconfig). No WebKit, no GTK.
# Usage: nix-shell rune-app/shell.nix --run 'cd rune-app && cargo build && ./wrap.sh'
{ pkgs ? import (builtins.getFlake "nixpkgs") { } }:
let
  gst = with pkgs.gst_all_1; [ gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav ];
  runtime = with pkgs; [ wayland libxkbcommon vulkan-loader libGL libx11 libxcursor libxi libxrandr alsa-lib fontconfig freetype dbus ];
in
pkgs.mkShell {
  packages = with pkgs; [ pkg-config ];
  buildInputs = runtime ++ gst ++ (with pkgs; [ openssl glib wayland.dev libxkbcommon.dev alsa-lib.dev dbus.dev ]);
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtime;
  GST_PLUGIN_SYSTEM_PATH_1_0 = pkgs.lib.makeSearchPath "lib/gstreamer-1.0" gst;
  # TLS for GStreamer's souphttpsrc (what wrapGAppsHook gives the Tauri package).
  GIO_EXTRA_MODULES = "${pkgs.glib-networking}/lib/gio/modules";
  # Software Vulkan (lavapipe) for GPU-less runs, e.g. the verifier's headless compositor.
  SONE_SW_VULKAN_ICD = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
}
