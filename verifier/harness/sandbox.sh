#!/usr/bin/env bash
# Run one build of SONE in a hermetic session and hand a driver command the same session.
#
#   sandbox.sh RUN_DIR SONE_BIN [--no-proxy] -- DRIVER_CMD...
#
# Inside:
#   - private D-Bus session bus; no Secret Service, so SONE's key falls back to sone.key
#   - AT-SPI bus + registry, headless sway (pixman), fake StatusNotifierWatcher (tray.py)
#   - private PipeWire + WirePlumber (hardware monitors off) with one null sink, plus
#     pipewire-pulse; ALSA device `sone_capture` = file plugin teeing into PipeWire (clocked)
#   - mitmdump running fake_tidal.py; nothing is forwarded to the internet
#   - everything in its own network namespace (loopback only) with host system bus, /run/user,
#     nscd, /run/pulse and /dev/snd hidden
# The driver gets SONE_PID, RUN, WAYLAND_DISPLAY and the same DBUS_SESSION_BUS_ADDRESS.
# Env knobs: SPIKE_SEED (JSON merged into settings.json), SPIKE_STRACE=1 (trace connect()),
#   SPIKE_GPU=1, SPIKE_REFRESH=Hz (see the compositor below), and for real-account runs:
#   SPIKE_NET=1 (the host's network instead of loopback and the fake TIDAL; audio, the system
#   bus and /dev/snd stay private) and SPIKE_PROFILE_DIR (a signed-in profile copied in: its
#   sone.key and encrypted settings.json, never written to the logs).
# Must run inside `nix-shell verifier/harness/shell.nix`.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
RUN="$(realpath -m "$1")"; SONE_BIN="$2"; shift 2
PROXY=1
[[ "${1:-}" == "--no-proxy" ]] && { PROXY=0; shift; }
if [[ -n "${SPIKE_NET:-}" ]]; then PROXY=0; fi
[[ "${1:-}" == "--" ]] && shift

if [[ -z "${SPIKE_INNER:-}" ]]; then
  rm -rf "$RUN"; mkdir -p "$RUN"/{home,config,cache,data,mitm,logs}
  # Unix socket paths max out at 108 bytes, so the runtime dir needs a short real path.
  ln -s "$(mktemp -d "${TMPDIR:-/tmp}/rt.XXXXXX")" "$RUN/runtime"
  chmod 700 "$RUN/runtime/"
  pass=()
  for v in PATH GI_TYPELIB_PATH GST_PLUGIN_SYSTEM_PATH_1_0 $(compgen -e | grep '^SPIKE_' || true); do
    pass+=("$v=${!v:-}")
  done
  # The whole session lives in its own network namespace (loopback only, so nothing can reach
  # the internet even by accident) with host-global sockets and sound hardware hidden:
  # the system bus, the real /run/user, nscd (it would resolve names on the host's network),
  # the system PulseAudio and /dev/snd.
  hide=(--tmpfs /run/user --tmpfs /dev/snd)
  for d in /run/dbus /run/pulse /run/nscd; do [[ -d $d ]] && hide+=(--tmpfs "$d"); done
  netns=(--unshare-net)
  if [[ -n "${SPIKE_NET:-}" ]]; then netns=(); fi
  exec bwrap --dev-bind / / "${netns[@]}" "${hide[@]}" --die-with-parent -- \
    env -i "${pass[@]}" SPIKE_INNER=1 \
    HOME="$RUN/home" XDG_CONFIG_HOME="$RUN/config" XDG_CACHE_HOME="$RUN/cache" \
    XDG_DATA_HOME="$RUN/data" XDG_RUNTIME_DIR="$(readlink -f "$RUN/runtime")" \
    XDG_DATA_DIRS="${SPIKE_XDG_DATA_DIRS:-}" LANG=C.UTF-8 \
    DBUS_SYSTEM_BUS_ADDRESS="unix:path=/nonexistent/system_bus" \
    dbus-run-session --config-file="$SPIKE_DBUS_CONF" -- "$0" "$RUN" "$SONE_BIN" $([[ $PROXY == 0 ]] && echo --no-proxy) -- "$@"
fi

cleanup() { kill $(jobs -p) 2>/dev/null || true; wait 2>/dev/null || true; }
trap cleanup EXIT
wait_for() { for _ in $(seq 100); do eval "$1" >/dev/null 2>&1 && return 0; sleep 0.1; done; echo "timeout: $1" >&2; return 1; }

# ---- accessibility bus; the registry is normally systemd-activated, so start it directly
# Accessibility is switched on the way a screen reader leaves it: the launcher's IsEnabled
# reads GSettings toolkit-accessibility. There's no dconf in here, so use the keyfile backend.
mkdir -p "$XDG_CONFIG_HOME/glib-2.0/settings"
printf '[org/gnome/desktop/interface]\ntoolkit-accessibility=true\n' > "$XDG_CONFIG_HOME/glib-2.0/settings/keyfile"
GSETTINGS_BACKEND=keyfile "$SPIKE_ATSPI_LAUNCHER" --launch-immediately > "$RUN/logs/atspi.log" 2>&1 &
sleep 0.5
"$(dirname "$SPIKE_ATSPI_LAUNCHER")/at-spi2-registryd" >> "$RUN/logs/atspi.log" 2>&1 &
# Announce an assistive technology, as a screen reader does. WebKitGTK exposes its tree
# regardless; AccessKit (Rune) only registers once accessibility is switched on.
for _ in $(seq 20); do
  dbus-send --session --dest=org.a11y.Bus --print-reply /org/a11y/bus \
    org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:IsEnabled variant:boolean:true \
    >> "$RUN/logs/atspi.log" 2>&1 && break
  sleep 0.1
done

# ---- tray host stand-in
python3 -I "$HERE/../channels/tray.py" watcher > "$RUN/logs/tray.log" 2>&1 &

# ---- headless compositor
# Display size (SPIKE_OUTPUT, verify.py --display). AccessKit (Rune) hides scroll content outside
# the viewport from AT-SPI and exposes no scroll action, so Rune builds are scored on a tall display.
# SPIKE_REFRESH (Hz) sets the output's refresh rate; SPIKE_GPU=1 renders on the machine's GPU
# (compositor, WebKit and Rune) instead of in software.
printf 'output HEADLESS-1 mode --custom %s@%sHz\ndefault_border none\n' "${SPIKE_OUTPUT:-1400x900}" "${SPIKE_REFRESH:-60}" > "$RUN/sway.conf"
if [[ -n "${SPIKE_GPU:-}" ]]; then wlr_renderer=vulkan; else wlr_renderer=pixman; fi
if [[ -n "${SPIKE_GPU:-}" ]]; then export WLR_RENDER_DRM_DEVICE=/dev/dri/renderD128; fi
WLR_BACKENDS=headless WLR_RENDERER=$wlr_renderer WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$RUN/sway.conf" > "$RUN/logs/sway.log" 2>&1 &
wait_for 'ls "$XDG_RUNTIME_DIR"/wayland-?'
export WAYLAND_DISPLAY="$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-? | head -1)")"
export SWAYSOCK="$(ls "$XDG_RUNTIME_DIR"/sway-ipc.* | head -1)"

# ---- private PipeWire: one null sink, no hardware, no logind/bluez
mkdir -p "$XDG_CONFIG_HOME/pipewire/pipewire.conf.d" "$XDG_CONFIG_HOME/wireplumber/wireplumber.conf.d"
cat > "$XDG_CONFIG_HOME/pipewire/pipewire.conf.d/10-verifier.conf" <<'EOF'
context.properties = {
  default.clock.rate = 48000
  default.clock.allowed-rates = [ 44100 48000 88200 96000 176400 192000 ]
}
context.objects = [
  { factory = adapter
    args = {
      factory.name = support.null-audio-sink
      node.name = verifier_sink
      node.description = "Verifier sink"
      media.class = Audio/Sink
      audio.position = [ FL FR ]
      priority.session = 2000
    }
  }
]
EOF
cat > "$XDG_CONFIG_HOME/wireplumber/wireplumber.conf.d/10-verifier.conf" <<'EOF'
wireplumber.profiles = {
  main = {
    monitor.alsa = disabled
    monitor.alsa-midi = disabled
    monitor.bluez = disabled
    monitor.bluez-midi = disabled
    monitor.v4l2 = disabled
    monitor.libcamera = disabled
    support.logind = disabled
  }
}
EOF
pipewire > "$RUN/logs/pipewire.log" 2>&1 &
wait_for '[[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]]'
wireplumber > "$RUN/logs/wireplumber.log" 2>&1 &
pipewire-pulse > "$RUN/logs/pipewire-pulse.log" 2>&1 &
wait_for '[[ -S "$XDG_RUNTIME_DIR/pulse/native" ]]'
wait_for 'pw-cli ls Node | grep -q verifier_sink'

# ---- ALSA: `sone_capture` writes every frame to audio.raw, then plays into PipeWire (clock)
cat > "$HOME/.asoundrc" <<EOF
pcm_type.pipewire { lib "$SPIKE_PW_ALSA_DIR/libasound_module_pcm_pipewire.so" }
ctl_type.pipewire { lib "$SPIKE_PW_ALSA_DIR/libasound_module_ctl_pipewire.so" }
pcm.pw { type pipewire }
ctl.pw { type pipewire }
pcm.!default pw
ctl.!default pw
pcm.sone_capture {
  type file
  slave.pcm "pw"
  file "$RUN/audio.raw"
  format "raw"
  truncate false
}
EOF

# ---- fake TIDAL behind mitmproxy
if [[ $PROXY == 1 ]]; then
  SPIKE_LOG="$RUN/logs/flows.jsonl" mitmdump --set termlog_verbosity=warn -s "$HERE/fake_tidal.py" \
    --set fixtures="$HERE/../fixtures" --set confdir="$RUN/mitm" --listen-port 18080 \
    --set connection_strategy=lazy > "$RUN/logs/mitm.log" 2>&1 &
  wait_for '(exec 3<>/dev/tcp/127.0.0.1/18080)'
  cat /etc/ssl/certs/ca-certificates.crt "$RUN/mitm/mitmproxy-ca-cert.pem" > "$RUN/ca-bundle.pem"
  export SSL_CERT_FILE="$RUN/ca-bundle.pem" NIX_SSL_CERT_FILE="$RUN/ca-bundle.pem" \
         CURL_CA_BUNDLE="$RUN/ca-bundle.pem"
fi

if [[ -n "${SPIKE_NET:-}" && -f /etc/ssl/certs/ca-certificates.crt ]]; then
  export SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt NIX_SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
fi

# ---- profile seed (encrypted settings.json + sone.key), before first launch
seed_log="$RUN/logs/seed.json"
if [[ -n "${SPIKE_PROFILE_DIR:-}" ]]; then
  mkdir -p "$XDG_CONFIG_HOME/sone"
  cp "$SPIKE_PROFILE_DIR/sone.key" "$SPIKE_PROFILE_DIR/settings.json" "$XDG_CONFIG_HOME/sone/"
  seed_log=/dev/null  # the decrypted settings hold real tokens
fi
if [[ -n "${SPIKE_SEED:-}" ]]; then
  python3 -I "$HERE/seed_profile.py" seed "$XDG_CONFIG_HOME/sone" "$SPIKE_SEED" > "$seed_log"
fi

# ---- no real apps: xdg-open / $BROWSER only record what the app asked to open
mkdir -p "$RUN/bin"
for b in xdg-open firefox chromium sensible-browser; do
  printf '#!/bin/sh
printf "%%s\n" "$*" >> "%s/logs/opened.txt"
' "$RUN" > "$RUN/bin/$b"
  chmod +x "$RUN/bin/$b"
done
export PATH="$RUN/bin:$PATH" BROWSER="$RUN/bin/xdg-open"

# ---- SONE
# Without SPIKE_GPU the compositor has no GPU: WebKit renders in software, Rune uses lavapipe.
export GDK_BACKEND=wayland RUST_LOG=info
if [[ -z "${SPIKE_GPU:-}" ]]; then
  export WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 SONE_SOFTWARE_VULKAN=1
fi
# Rune builds: open at the display size (Rune keeps a window's first bounds for AccessKit).
export SONE_WINDOW_SIZE="${SPIKE_OUTPUT:-1400x900}"
launch=("$SONE_BIN")
if [[ -n "${SPIKE_STRACE:-}" ]]; then
  launch=(strace -f -qq -e trace=connect -o "$RUN/logs/connect.log" "${launch[@]}")
fi
"${launch[@]}" > "$RUN/logs/sone.log" 2>&1 &
export SONE_PID=$! RUN
"$@"
