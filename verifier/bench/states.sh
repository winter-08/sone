#!/usr/bin/env bash
# Run bench/states.py (or, with --exec, any command) inside the verifier's nix shell, pinned to
# the nixpkgs revision the handoff uses: on the floating nixpkgs the shell's mitmproxy fails its
# tests and the shell does not build.
#
#   verifier/bench/states.sh --build base=rune-app/target/baseline/release/sone-rune-wrapped [--gpu] ...
#   verifier/bench/states.sh --exec python3 -c 'print(1)'
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PIN='import (builtins.getFlake "github:NixOS/nixpkgs/f9a75f41be3a74eab55d3a7679d34ad6cc0da977") {}'
if [[ "${1:-}" == "--exec" ]]; then
  shift
  cmd="$(printf '%q ' "$@")"
else
  cmd="python3 -I $(printf '%q' "$ROOT/verifier/bench/states.py") $(printf '%q ' "$@")"
fi
cd "$ROOT"
exec nix-shell "$ROOT/verifier/harness/shell.nix" --arg pkgs "$PIN" --run "$cmd"
