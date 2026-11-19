# Build shell for the Rune frontend: SONE's backend deps (GStreamer, ALSA, D-Bus, OpenSSL) plus
# what Rune links or dlopens (Wayland/X11, xkbcommon, Vulkan/GL, fontconfig). No WebKit, no GTK.
# Usage: nix-shell rune-app/shell.nix --run 'cd rune-app && cargo build && ./wrap.sh'
#
# nixpkgs is the revision ../flake.lock pins, so the shell and `nix build .#sone` agree and the
# GStreamer patch below always applies.
{ pkgs ? import (builtins.getFlake "github:NixOS/nixpkgs/${(builtins.fromJSON (builtins.readFile ../flake.lock)).nodes.nixpkgs.locked.rev}") { } }:
let
  # hlsdemux2 (music videos) aborts the process when stopped mid-download in GStreamer 1.28.7:
  # its cleanup completes libsoup messages without the transfer thread's context. Two lines
  # fix it (patches/; upstream-able).
  good = pkgs.gst_all_1.gst-plugins-good.overrideAttrs (o: {
    patches = (o.patches or [ ]) ++ [ ./patches/gst-adaptivedemux2-stop-context.patch ];
  });
  gst = with pkgs.gst_all_1; [ gstreamer gst-plugins-base good gst-plugins-bad gst-libav ];
  runtime = with pkgs; [ wayland libxkbcommon vulkan-loader libGL libx11 libxcursor libxi libxrandr alsa-lib fontconfig freetype dbus ];
in
pkgs.mkShell {
  # lld links the binary; sccache shares compiled dependencies between checkouts and worktrees
  # (registry and git crates live under ~/.cargo, so their cache keys match everywhere; this
  # crate and the backend are path crates and stay incremental).
  packages = with pkgs; [ pkg-config llvmPackages.bintools sccache ];
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C link-arg=-fuse-ld=lld";
  RUSTC_WRAPPER = "sccache";
  SCCACHE_CACHE_SIZE = "40G";
  # The sccache server outlives this shell, and nix-shell deletes its TMPDIR on exit: start the
  # server here with a temp dir that stays, or later shells' compiles fail in a missing one.
  shellHook = ''
    TMPDIR="''${XDG_RUNTIME_DIR:-/tmp}" sccache --start-server </dev/null >/dev/null 2>&1 || true
  '';
  buildInputs = runtime ++ gst ++ (with pkgs; [ openssl glib wayland.dev libxkbcommon.dev alsa-lib.dev dbus.dev ]);
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtime;
  GST_PLUGIN_SYSTEM_PATH_1_0 = pkgs.lib.makeSearchPath "lib/gstreamer-1.0" gst;
  # TLS for GStreamer's souphttpsrc (what wrapGAppsHook gives the Tauri package).
  GIO_EXTRA_MODULES = "${pkgs.glib-networking}/lib/gio/modules";
  # Software Vulkan (lavapipe) for GPU-less runs, e.g. the verifier's headless compositor.
  SONE_SW_VULKAN_ICD = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
}
