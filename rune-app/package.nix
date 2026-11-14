# RONE: the Rune build of SONE as an installable app (`nix profile install .#rone`).
# Same runtime as shell.nix + wrap.sh, but built from the committed sources by Nix.
{
  lib,
  craneLib,
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
  # Shared by the two derivations below. crane (not buildRustPackage) so the ~480 dependency
  # crates build once, in `rone-deps`, and a change to this crate or the backend recompiles
  # only those two. It vendors each source into its own directory, which also copes with Rune
  # building its own tiny-skia-path 0.11.4 beside resvg's crates.io copy, and needs no hash:
  # everything is pinned by Cargo.lock.
  common = {
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
    # The crate is not at the source root.
    cargoToml = ./Cargo.toml;
    cargoLock = ./Cargo.lock;
    # Rune is the one git dependency. With its hash here it is fetched like any other source;
    # without, Nix would clone it with the user's own git at evaluation time. Update the hash
    # when the Rune rev in Cargo.toml changes (a wrong one fails with the right one in "got:").
    cargoVendorDir = craneLib.vendorCargoDeps {
      cargoLock = ./Cargo.lock;
      outputHashes = {
        "git+https://github.com/anomalyco/rune?rev=6c3cb8abf150e1c43b2f276a14568b2f2f504d72#6c3cb8abf150e1c43b2f276a14568b2f2f504d72" =
          "sha256-IJzxSA5Kc8UD+BZSwzBfQJ5igNhl2LDbSSUoffKs8zc=";
      };
      # Rune vendors published crates (vendor/winit and others) with the Cargo.toml.orig that
      # `cargo package` left in them, and crane packages each crate again, which refuses that
      # reserved name.
      overrideVendorGitCheckout = _: drv: drv.overrideAttrs (o: {
        postPatch = (o.postPatch or "") + ''
          find . -name Cargo.toml.orig -delete
        '';
      });
    };
    postUnpack = ''
      cd $sourceRoot/rune-app
      sourceRoot="."
    '';
    strictDeps = true;

    nativeBuildInputs = [ pkg-config makeWrapper copyDesktopItems ];
    buildInputs = runtime ++ gst ++ [ openssl glib ];

    # The tests drive the UI headless and need a GPU; the verifier covers them.
    doCheck = false;
  };
  cargoArtifacts = craneLib.buildDepsOnly common;
in
craneLib.buildPackage (common // {
  inherit cargoArtifacts;

  env = {
    SONE_RUNE_TITLE = "RONE";
    SONE_RUNE_APP_ID = appId;
  };

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
      # %u: a tidal:// link the desktop opens with it (a running instance takes it, shell.rs)
      exec = "rone %u";
      icon = appId;
      startupWMClass = appId;
      categories = [ "Audio" "AudioVideo" "Player" ];
      mimeTypes = [ "x-scheme-handler/tidal" ];
    })
  ];

  meta = {
    description = "SONE (TIDAL client) with a native Rune UI";
    license = lib.licenses.gpl3Only;
    mainProgram = "rone";
    platforms = [ "x86_64-linux" ];
  };
})
