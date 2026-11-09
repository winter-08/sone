# RONE: the Rune build of SONE as an installable app (`nix profile install .#rone`).
# Same runtime as shell.nix + wrap.sh, but built from the committed sources by Nix.
{
  lib,
  rustPlatform,
  makeWrapper,
  copyDesktopItems,
  makeDesktopItem,
  pkg-config,
  gst_all_1,
  glib,
  glib-networking,
  openssl,
  alsa-lib,
  dbus,
  wayland,
  libxkbcommon,
  vulkan-loader,
  libGL,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
  fontconfig,
  freetype,
  src,
}:

let
  # hlsdemux2 aborts when a music video closes mid-download in GStreamer 1.28.7 (see shell.nix).
  good = gst_all_1.gst-plugins-good.overrideAttrs (o: {
    patches = (o.patches or [ ]) ++ [ ./patches/gst-adaptivedemux2-stop-context.patch ];
  });
  gst = with gst_all_1; [ gstreamer gst-plugins-base good gst-plugins-bad gst-libav ];
  # Opened with dlopen by Rune (winit, wgpu) and the backend, so they go on LD_LIBRARY_PATH.
  runtime = [ wayland libxkbcommon vulkan-loader libGL libx11 libxcursor libxi libxrandr alsa-lib fontconfig freetype dbus ];
  appId = "rone";
  icons = src + "/src-tauri/icons";
in
rustPlatform.buildRustPackage {
  pname = "rone";
  version = (lib.importTOML ./Cargo.toml).package.version;

  # Only what the build reads: this crate and the SONE backend it links (no target/ dirs).
  src = lib.fileset.toSource {
    root = src;
    fileset = lib.fileset.difference
      (lib.fileset.unions [ (src + "/rune-app") (src + "/src-tauri") ])
      (lib.fileset.unions [
        (lib.fileset.maybeMissing (src + "/rune-app/target"))
        (lib.fileset.maybeMissing (src + "/src-tauri/target"))
      ]);
  };
  cargoRoot = "rune-app";
  buildAndTestSubdir = "rune-app";
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes."rune-0.0.1" = "sha256-6nHRqNXfO7Jld+U4ST0I6l2pwyH9gyF3Zz5WazACpgQ=";
  };

  nativeBuildInputs = [ pkg-config makeWrapper copyDesktopItems ];
  buildInputs = runtime ++ gst ++ [ openssl glib ];

  env = {
    SONE_RUNE_TITLE = "RONE";
    SONE_RUNE_APP_ID = appId;
  };

  # The tests drive the UI headless and need a GPU; the verifier covers them.
  doCheck = false;

  postInstall = ''
    mv $out/bin/sone-rune $out/bin/rone
    for s in 32x32 64x64 128x128; do
      install -Dm644 ${icons}/$s.png $out/share/icons/hicolor/$s/apps/${appId}.png
    done
    install -Dm644 ${icons}/128x128@2x.png $out/share/icons/hicolor/256x256/apps/${appId}.png
  '';

  # /run/opengl-driver/lib last: NixOS's GPU driver libraries, found only there.
  postFixup = ''
    wrapProgram $out/bin/rone \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtime} \
      --suffix LD_LIBRARY_PATH : /run/opengl-driver/lib \
      --set GST_PLUGIN_SYSTEM_PATH_1_0 ${lib.makeSearchPath "lib/gstreamer-1.0" (map lib.getLib gst)} \
      --prefix GIO_EXTRA_MODULES : ${glib-networking}/lib/gio/modules
  '';

  desktopItems = [
    (makeDesktopItem {
      name = appId;
      desktopName = "RONE";
      genericName = "TIDAL player";
      comment = "SONE with a native Rune interface";
      exec = "rone";
      icon = appId;
      startupWMClass = appId;
      categories = [ "Audio" "AudioVideo" "Player" ];
    })
  ];

  meta = {
    description = "SONE (TIDAL client) with a native Rune UI";
    license = lib.licenses.gpl3Only;
    mainProgram = "rone";
    platforms = [ "x86_64-linux" ];
  };
}
