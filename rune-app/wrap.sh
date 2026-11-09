#!/usr/bin/env bash
# Write target/<profile>/sone-rune-wrapped: the binary with this shell's runtime environment
# baked in, so it runs under a clean environment (the verifier's sandbox uses `env -i`).
# Run inside `nix-shell rune-app/shell.nix`. SONE_SOFTWARE_VULKAN=1 at run time selects lavapipe.
set -euo pipefail
profile="${1:-debug}"
bin="$(cd "$(dirname "$0")" && pwd)/target/$profile/sone-rune"
[[ -x "$bin" ]] || { echo "build first: $bin missing" >&2; exit 1; }
out="$bin-wrapped"
cat > "$out" <<WRAP
#!/usr/bin/env bash
# /run/opengl-driver/lib last: NixOS's GPU driver libraries (libcuda, libnvcuvid for the NVDEC
# video decoder), found only there, so nothing the build ships is shadowed.
export LD_LIBRARY_PATH="$LD_LIBRARY_PATH\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}:/run/opengl-driver/lib"
export GST_PLUGIN_SYSTEM_PATH_1_0="$GST_PLUGIN_SYSTEM_PATH_1_0"
export GIO_EXTRA_MODULES="$GIO_EXTRA_MODULES"
if [[ -n "\${SONE_SOFTWARE_VULKAN:-}" ]]; then
  export VK_ICD_FILENAMES="$SONE_SW_VULKAN_ICD"
fi
exec "$bin" "\$@"
WRAP
chmod +x "$out"
echo "$out"
