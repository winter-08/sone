# SONE on Rune

This is a vibe-slopped port of [SONE](https://github.com/lullabyX/sone), the Linux client for
[TIDAL](https://tidal.com), to the [Rune](https://github.com/anomalyco/rune) UI toolkit. It exists
to see what Rune can do. It is an experiment, not a product: almost all of it was written by AI
agents, and it is not reviewed or tested to the standard of the original.

If you want a TIDAL client for Linux, use the original SONE. All credit for the app, its design
and its backend belongs to that project.

## What is here

- `rune-app/`: the app, a native Rune UI. [`rune-app/README.md`](rune-app/README.md) maps the code
  and lists what is not done.
- `core/`: SONE's backend (TIDAL client, audio engine, scrobbling, MPRIS, MCP and overlay
  servers) with Tauri removed.

The original Tauri and React app is not in this branch.

## Install

Nix only, on x86_64 Linux. There are no releases and no other packages.

```bash
nix profile install github:winter-08/sone#sone
```

To update:

```bash
nix profile upgrade sone
```

To build and run from a checkout, see [`rune-app/README.md`](rune-app/README.md).

You need an active TIDAL subscription.

## Disclaimer

This is not affiliated with, endorsed by, or connected to TIDAL, and it is not the SONE project.
Content is streamed from TIDAL's service and requires a paid subscription. There are no offline
downloads and nothing is redistributed. All trademarks belong to their owners.

## License

[GPL-3.0-only](LICENSE), as the original.
