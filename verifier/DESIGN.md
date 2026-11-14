# External verifier for the Rune port

How we prove that a Rune build of SONE does everything the Tauri/React build does, without
trusting the port's own code or its own tests.

- **Reference:** the current Tauri build (upstream tag, e.g. `v0.23.0`, commit `a4accb7`).
- **Candidate:** the Rune build.
- **Verifier:** a separate program that launches either build in the same sandbox, drives it
  through boundaries both builds share, records what comes out, and compares.

## Principles

1. **Black box.** The verifier never links, imports or calls app code. It sees only what any
   user or neighbouring program could see: the network, D-Bus, the audio device, files on disk,
   the accessibility tree, the MCP server and input events.
2. **The reference is the oracle.** Expected behaviour is recorded from the reference build, not
   written by hand. A scenario is valid only once it passes, deterministically, on the reference.
3. **Same scenario, both builds.** Every scenario runs unchanged against either build. Anything
   build-specific lives in a thin adapter, and that adapter stays small.
4. **Semantic, not pixel, parity.** Rune will look different. We compare what the app *does* and
   *exposes* (calls, state, audio, labelled controls), not how it is drawn.
5. **Coverage has a denominator.** `inventory.py` extracts the reference's surface from source.
   The port is done when every item is covered by a passing scenario or carries a written waiver.
6. **Verify the verifier.** Before any Rune code exists, the suite must pass on the reference,
   and it must fail on deliberately broken reference builds (mutation checks).

## What the reference exposes (from `inventory.py`, v0.23.0)

| Surface | Count | Notes |
|---|---|---|
| Tauri commands registered | 182 | 159 invoked by the frontend. The other 23 are dead code, MCP-only or debug. |
| Rust → UI events | 24 | `mpris:*`, `tray:*`, `pkce-login-*`, `track-finished`, `signal-path-changed`… |
| UI-side listeners | 46 | Includes `mcp:*` (MCP actions are executed by the frontend) |
| Views (`ViewTarget`) | 15 | home, album, playlist, artist, artistTracks, mix, favorites, search, explore, explorePage, feed, viewAll, libraryViewAll, profile, profilePlaylists |
| Keyboard shortcuts | 19 | `src/lib/shortcuts.ts` |
| Frontend-persisted keys | 8 | `sone.volume.v1`, `sone.shuffle.v1`… stored in **WebKitGTK localStorage** |
| `settings.json` fields | 27 | `~/.config/sone/settings.json` |
| MCP tools | 34 | Streamable HTTP on `127.0.0.1:<port>/<token>/mcp` |
| External hosts | ~12 real | TIDAL (api, auth, openapi, ec, login, resources), Last.fm, Libre.fm, ListenBrainz, MusicBrainz, GitHub |
| Unit tests | 588 vitest, 402 cargo | The vitest cases describe behaviour being moved from TS to Rust |

The most important finding: **much of the core logic lives in TypeScript**. The queue, shuffle,
repeat, autoplay and gapless prefetch live in `usePlaybackActions.ts` (~1.9k lines),
`usePlaySource.ts` and `useGaplessPrefetch.ts`, and auth flow logic is in `useAuth.ts`. The Rust
backend is kept, but this TS logic gets rewritten. That is where regressions will come from, so
it gets the most verifier attention.

## Sandbox

Each run gets a fresh, hermetic environment. Both builds run in the identical environment.

| Concern | Mechanism |
|---|---|
| Profile | Temp `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_DATA_HOME`, `XDG_RUNTIME_DIR` (`dirs` honours these, so `settings.json` and the cache stay isolated) |
| Session bus | `dbus-run-session`, private to the run. MPRIS, the tray (StatusNotifierItem) and AT-SPI all live on it |
| Keyring | None by default: with no Secret Service on the private bus, SONE falls back to `sone.key`, which lets the verifier seed and read profiles. A throwaway gnome-keyring is started only for scenarios that test keyring storage itself |
| Display | Headless Wayland compositor (`sway` with `WLR_BACKENDS=headless`, or `weston --backend=headless`), plus an Xvfb variant for X11 paths |
| Accessibility | `at-spi2-core` bus launched inside the session |
| Network | Every request goes through a **record/replay proxy** (mitmproxy addon). The proxy is set through `settings.json`'s `proxy` field. reqwest 0.11 uses native-tls (OpenSSL), so the proxy CA is trusted through `SSL_CERT_FILE`. Nothing reaches the real internet in verify mode |
| Audio | Private PipeWire with a null sink provides the clock and the shared-mode output (through pipewire-pulse). `~/.asoundrc` defines `sone_capture`: a `file`-plugin PCM that writes raw frames to disk and plays them into PipeWire. That device is selected as the exclusive device. No root or `snd-aloop` needed |
| Isolation | The whole session runs in its own network namespace (loopback only). Host system bus, `/run/user`, nscd, system PulseAudio and `/dev/snd` are hidden |
| Discord | A fake Discord IPC server on `$XDG_RUNTIME_DIR/discord-ipc-0` records the handshake and `SET_ACTIVITY` payloads |
| Clock | Real time, with tolerances (see Comparison). The Rune build's headless virtual clock is not used for parity runs, since the reference has no equivalent |

### TIDAL fixtures

- **Cassettes:** API responses recorded once from a real test account, then sanitized: tokens,
  user IDs, emails and signed URLs replaced with stable fakes. Cassettes are never committed with
  secrets. The sanitizer runs in CI and fails on high-entropy strings.
- **Streams:** the replay proxy rewrites playback-info manifests to point at a local fixture
  server. That server holds synthetic FLAC files (tone sweeps and known-pattern noise at 44.1/16,
  96/24 and 192/24) whose decoded PCM checksums are known in advance. This makes audio output
  bit-comparable.
- **Auth:** the device-code and PKCE flows are replayed from cassettes. A scenario can also start
  from a pre-seeded keyring plus `settings.json` (an already-logged-in user).

## Observation channels

Each channel turns one boundary into a normalized, timestamped event stream.

| Channel | Observes | Drives |
|---|---|---|
| **MCP** | `get_now_playing`, `get_queue`, favorites, playlists | 34 tools: play, skip, seek, shuffle, repeat, volume, queue edits, playlist CRUD |
| **MPRIS** (D-Bus) | Metadata, PlaybackStatus, Position, LoopStatus, Shuffle, Volume, signals | Play/Pause/Next/Previous/Seek/SetPosition/OpenUri |
| **Network** | API calls: method, host, path, normalized query and body, and order where it matters | — |
| **Audio** | PCM bytes, sample rate, bit depth, channel count, gaps, underruns | — |
| **Accessibility** (AT-SPI) | Per-view tree of (role, name, state) | Activate controls by role and name, type text, focus |
| **Input** | — | Key chords for the 19 shortcuts (`wtype`, or `xdotool` on Xvfb) |
| **Tray** (SNI D-Bus) | Menu items and states | Menu activation |
| **Discord IPC** | Activity payloads | — |
| **Filesystem** | `settings.json` contents, cache layout | Seeded profiles |
| **Keyring** | Stored items and attributes | Seeded tokens |
| **Process** | Startup time, RSS, CPU, exit code, stderr panics | Launch, signal, kill |

MCP deserves a note. In the reference, an MCP action goes Rust → `mcp:*` event → TS playback logic
→ state mirrored back to Rust. So driving through MCP exercises exactly the TS logic being
ported, through an interface the candidate must keep anyway. It is the strongest single oracle
we have.

## Verification layers

### L0 Coverage gate

`inventory.py --json` produces the denominator. `coverage.toml` maps every inventory item to the
scenarios that cover it, or to a waiver with a reason and an owner. CI fails when:

- an item has no scenario and no waiver
- a scenario covering an item fails on the candidate
- a new upstream release adds surface (`inventory.py --diff old.json`) that isn't mapped yet

The last check matters because SONE ships weekly. The port is chasing a moving target.

### L1 Backend contract

The Rust backend is kept, so its 402 cargo tests keep running on the candidate tree unchanged.
Every reference command a user can reach must still be reachable in the candidate. For example,
`toggle_bit_perfect` must be triggerable from the UI or a shortcut. Reachability is proven by an
L2 or L3 scenario, not by a code grep.

### L2 Behavioural parity: the core

Scenario scripts drive the app through MCP, MPRIS, the accessibility tree and input. Every
channel is recorded. **Record mode** runs a scenario on the reference and stores the normalized
trace as golden. **Verify mode** runs it on the candidate and diffs against the golden.
**Differential mode** runs both builds live, side by side, for exploratory checks.

Scenario families (each maps back to inventory items):

- **Playback and queue:** play album, playlist, mix or radio. Next, previous, seek, end of queue.
  Shuffle and repeat combinations. Autoplay when the queue runs out. Queue edits while playing.
  Duplicate handling when adding to a playlist. This is the biggest family, mirroring
  `usePlaybackActions.test.tsx` and `usePlaySource.test.tsx`.
- **Audio paths:** shared vs exclusive ALSA. Bit-perfect on/off. Quality cap (`max_quality`).
  Gapless transitions, checked for zero inserted silence between fixture tracks. Volume
  normalization. Resampling events.
- **Library:** favorite or unfavorite tracks, albums, artists, mixes and playlists. Playlist CRUD
  and folders. Every change is checked on the network channel (the right API call) and through
  MCP read-back.
- **Navigation:** every one of the 15 views reachable, with the expected labelled content present
  (L3). Back/forward and scroll restoration.
- **Search:** queries, tabs, search history persistence.
- **Integrations:** MPRIS round-trips. Tray actions. Scrobbles at the listen threshold (Last.fm,
  Libre.fm, ListenBrainz request bodies). TIDAL play reports to `ec.tidal.com`. Discord
  activity. Update check against a stubbed GitHub release.
- **Settings:** each of the 27 `settings.json` fields toggled through the UI, with the resulting
  file and behaviour checked.
- **Auth:** device-code login, PKCE login and its cancel and error paths, logout clearing tokens
  and state.
- **Proxy:** the app's own proxy settings (HTTP and SOCKS), the fail-closed paths, and
  `probe_proxy_reachability`.
- **Miniplayer and windows:** open and close the miniplayer, state sync, always-on-top,
  minimize to tray.
- **Shortcuts:** all 19, by key chord.

**Comparison rules** (golden vs candidate):

- Event *sequences* must match after normalization. Timestamps are dropped. Ordering is enforced
  only within a channel, and across channels only where the scenario declares a causal edge
  (for example, `skip_next` then a new MPRIS track ID then an audio boundary).
- Numeric tolerances: position ±1 s, volume ±1%, startup timing excluded from parity.
- Network: compare the set of API calls per step, with volatile fields (timestamps, nonces,
  session IDs) masked by declared rules. Image and CDN fetches (`resources.tidal.com`) are
  excluded, because the reference loads them from WebKit and the candidate from Rust.
- Audio: exact PCM equality in bit-perfect and exclusive mode. Format, continuity and
  no-underrun checks in shared mode.
- Each scenario runs 3 times. Any flake counts as a failure of the scenario, not a pass.

### L3 UI semantic parity

Both builds export the same accessibility protocol on Linux: WebKitGTK through its AT-SPI
bridge, and Rune through AccessKit's `accesskit_unix` (the `accessibility` cargo feature must be
enabled). One AT-SPI driver can therefore inspect and drive both.

- **Per-view inventory:** for each view, snapshot the normalized set of (role, name) for
  landmarks, buttons, inputs, list items and headings. Diff reference vs candidate. Missing
  controls are failures. Extra controls are reported for review.
- **Interaction:** L2 scenarios that need the UI use semantic selectors ("button named Shuffle",
  "row containing <track title>"), never coordinates or CSS selectors.
- **Side effect:** the Rune app must label its controls properly, which also makes it usable
  with Orca. The reference has to pass too, so poorly labelled reference controls are fixed in
  the adapter's selector map, not in the scenarios.

Fallback, if WebKitGTK's AT-SPI tree proves unusable (spike 1): drive the reference through
WebDriver (`tauri-driver` + `WebKitWebDriver`) and read the candidate through Rune's `--tree`
dump. Both then feed one semantic adapter that maps to the same (role, name) model.

### L4 Visual review (not a gate)

Pixel parity is a non-goal. Visuals are used for:

- a side-by-side gallery per view and theme (reference screenshot vs Rune `--snap`) for human
  sign-off
- theme checks: sampled colours on known elements match the active theme file
- golden regression images for the *candidate*, once a view is signed off

### L5 Non-functional budgets

Measured in the sandbox on the same machine, candidate vs reference:

- cold start to interactive home view
- RSS at idle and after scrolling a 10k-track playlist
- idle CPU, and CPU while playing
- frame time while scrolling (Rune `--trace`, and compositor frame callbacks for the reference)
- zero audio underruns over a 30-minute gapless run

The goal is for the candidate to be no worse than the reference on any of these, and better on
memory.

### L6 Migration

Existing users upgrade in place, so we test that too. Start the reference on a fresh profile, run
a setup scenario (log in, set volume, shuffle, repeat, a theme, search history, a proxy), quit,
then launch the **candidate on the same profile**. Everything must survive:

- `settings.json` and keyring tokens: shared, so they should just work
- **the 8 `sone.*` keys stored in WebKitGTK localStorage**, which a Rune app cannot see. The
  candidate needs a one-time migration that reads WebKit's localStorage SQLite under the old
  data directory, or users silently lose volume, shuffle, repeat, AI/explicit filters and search
  history. This is a known porting task the verifier pins down
- theme files and cache

Also check the reverse direction for downgrades: the candidate must not write anything that
breaks the reference on the same profile.

## Waivers (expected, written down)

| Item | Reason |
|---|---|
| Music video playback (`video` atoms, HLS) | Rune `Player` is macOS-only on Linux today. Tracked upstream in anomalyco/rune#30, #100, #112. Revisit when one lands |
| WebKit-specific env handling (`WEBKIT_DISABLE_DMABUF_RENDERER`, `SOUP_FORCE_HTTP*`) | Not applicable without WebKit |
| `greet`, `debug_home_page_raw` and other dead or debug commands | Not user-reachable |
| CDN image request parity | Different fetch path by design |

Every waiver names an owner and an expiry condition. Waivers live in `coverage.toml` and show up
in the report.

## Definition of done

The port is accepted when, on the same release of upstream:

1. L0: coverage is 100% of inventory items (scenarios or waivers), with no unmapped surface from
   the latest upstream diff.
2. L1: cargo tests are green on the candidate tree.
3. L2: every scenario matches its golden on the candidate, 3 runs each, with no flakes.
4. L3: per-view semantic diffs are empty, or reviewed and waived.
5. L5: all budgets are met.
6. L6: the migration suite passes in both directions.
7. L4: a human has signed off the visual gallery.

Ported TS behaviour also gets internal Rust tests. Each of the 588 vitest cases should map to a
Rust test or a waiver. That is tracked alongside, but it is not the verifier's evidence: the
verifier only trusts what it observes from outside.

## Layout and tooling

```
verifier/
  DESIGN.md            this file
  verify.py            CLI: record / check / run, parallel sandboxes, report
  inventory.py         surface extractor
  harness/
    shell.nix          pinned tools (nix-shell verifier/harness/shell.nix)
    sandbox.sh         one hermetic session per run (see "Sandbox completion")
    runner.py          executes a scenario inside the sandbox, writes trace.json
    compare.py         golden vs trace, with the tolerance rules
    fake_tidal.py      mitmproxy addon: fake TIDAL, fixture server, request log
    seed_profile.py    write/read SONE's encrypted settings.json
    make_fixtures.sh   regenerate fixtures (gen_noise.py: seeded-noise WAVs)
  channels/            mcp.py, mpris.py, net.py, audio.py, tray.py, atspi.py
  scenarios/           *.toml step scripts
  goldens/             normalized traces recorded from the reference (committed)
  fixtures/            tracks.json (committed); audio, DASH, artwork (generated)
  runs/                per-run sandboxes, logs, traces (ignored)
  coverage.toml        (next) inventory item -> scenarios | waiver
```

The verifier is written in **Python**, deliberately not Rust. Being a different language from the
candidate keeps it independent. Python also has mature mitmproxy, AT-SPI (`pyatspi`), D-Bus
(`dbus-python`) and numpy support. The pinned shell provides the compositor, PipeWire, at-spi2-core,
mitmproxy, bwrap and ffmpeg. Builds are passed with `--bin`, so the verifier works on any build:

```bash
nix-shell verifier/harness/shell.nix --run 'python3 -I verifier/verify.py check --bin /path/to/sone'
```

A scenario (see `harness/runner.py` for every key):

```toml
name = "skip-next"
covers = ["mcp_tools:skip_next"]
tracks = [1005, 1006, 1007]            # fixtures the audio channel looks for

[[step]]
mcp = { tool = "play_tracks", args = { track_ids = [1005, 1006, 1007], action = "play_now" } }
[[step]]
wait_for = { "mpris.status" = "Playing", timeout = 10 }
[[step]]
label = "skip to B"
mcp = { tool = "skip_next" }
```

After every step the runner records the MPRIS snapshot, MCP now-playing and queue, and the API
calls the step caused. The whole run adds the tray menu, the accessibility control inventory and
the audio analysis.

## Spike results (2026-11-08, reference v0.23.0)

All four approaches work. The spike code has since become `harness/` and `channels/` (see
"Harness v0"). The spike-era fixtures were tones; they were replaced by seeded noise.

| Spike | Result | What it showed |
|---|---|---|
| Profile seeding (unplanned) | **Go** | `settings.json` is AES-256-GCM (`SONE`, 0x01, 12-byte nonce, ciphertext). With no Secret Service on the bus, the key falls back to the raw 32-byte `sone.key`. The verifier can write a logged-in profile (fake tokens, MCP on, proxy, exclusive device) before first launch. **No real account and no login flow needed for most scenarios.** Cassettes are only needed for realistic catalogue data |
| 1. AT-SPI | **Go, with gaps** | WebKitGTK 2.54 exposes a usable tree (about 75 nodes on Explore). Nav buttons, library tabs, search and the queue button have roles and names. `doAction("press")` on "Explore" navigated. **Gap:** the player bar's icon buttons (likely prev, play/pause, next, shuffle, repeat, like) and the volume slider are unnamed. Either give them `aria-label`s upstream (a good accessibility fix anyway) or pin them by structure in the reference adapter's selector map. The sandbox must start `at-spi2-registryd` itself, because there's no systemd to activate it |
| 2. Proxy | **Go** (gaps resolved below) | The seeded `proxy` setting routes all reqwest and GStreamer traffic through mitmproxy, trusted through `SSL_CERT_FILE`. mitmproxy must run with `connection_strategy=lazy`, or it contacts real upstreams to fetch certificates. strace saw exactly two TCP connects, both to the proxy. **Hermeticity gaps:** the app also opens the host's **system D-Bus** and **PulseAudio** sockets, so the harness must hide both (bwrap or a mount namespace, plus a private PipeWire). WebKit-loaded images were not exercised, because the fixtures have no artwork |
| 3. Exclusive ALSA | **Go for bytes, no-go for timing** (timing resolved below) | With `exclusive_device = "sone_capture"` (ALSA `file` plugin) and bit-perfect on, the 96 kHz/24-bit fixture came out **bit-exact** in an `S24_32LE` container, with only silence around it. But the `null` slave has no clock: playback finishes instantly, the writer floods silence (436 MB for a 10 s track), and MCP reports `isPlaying=false`, position 0. **Timing scenarios need a clocked slave.** Use a PipeWire null sink in the sandbox through `pipewire-alsa`, which also fixes the PulseAudio gap, or `snd-aloop` on CI hosts that allow kernel modules |
| 4. Manifest rewrite | **Go** | Returning a `vnd.tidal.bts` manifest whose URL points at `fixtures.sone.test` worked. GStreamer (souphttpsrc/curlhttpsrc) fetched it through the proxy and played it. MCP showed the right now-playing and queue. The gapless prefetch of the next track appeared in the network log. **Untested:** DASH manifests (`dash+xml`), which real hi-res streams use. Add a DASH fixture (an MPD plus segments) next |

Other findings:

- **MCP drives and observes playback end to end.** `play_tracks`, then `get_now_playing` and
  `get_queue`, then the network log. This makes MCP the backbone channel for L2.
- **The tray channel needs a fake `org.kde.StatusNotifierWatcher`** on the private bus.
  Without one, the app logs that the tray failed and no tray menu is exposed.
- **Expected sandbox noise:** deep-link registration fails (`update-desktop-database` is
  missing).
- **The upstream Nix package was broken at v0.23.0:** its `pnpmDeps.hash` was stale after the
  vite bump. It's fixed locally in `nix/package.nix`. Worth an upstream PR.

## Sandbox completion (2026-11-08)

The gaps the spikes found are closed. `sandbox.sh` now provides:

| Piece | How | Verified |
|---|---|---|
| Clocked audio | Private PipeWire, WirePlumber (all hardware, bluez and logind monitors off) and pipewire-pulse, with one `support.null-audio-sink`. ALSA `sone_capture` is the `file` plugin teeing into the PipeWire ALSA plugin, so frames are captured *and* paced by a real clock | The 10 s track plays for 10 s. The capture is 1.78 MB (not 436 MB) and bit-exact (`S16LE`, 44.1 kHz). MPRIS `Position` advances in real time: 1.83, 3.84, 5.80, 7.81 s, then `Paused`/0 at end of queue |
| Host isolation | The whole session runs in `bwrap --unshare-net` (loopback only), with tmpfs over `/run/user`, `/run/dbus`, `/run/pulse`, `/run/nscd` and `/dev/snd`, and `DBUS_SYSTEM_BUS_ADDRESS` pointed nowhere | strace: every connect to a hidden socket (system bus, nscd) fails with `ENOENT`. TCP only reaches `127.0.0.1`. There is no route to the internet even if something ignored the proxy. Shared-mode audio goes to the private pipewire-pulse |
| Tray | `tray.py watcher` serves `org.kde.StatusNotifierWatcher`. `tray.py menu` and `tray.py click LABEL` read and drive the item's dbusmenu | Menu read back: Show / Play / Pause / Next Track / Previous Track / Quit |
| DASH | `make_fixtures.sh` stream-copies the 96 kHz/24-bit FLAC into fMP4 segments (ffmpeg) and rewrites the MPD to absolute URLs, because SONE hands the MPD to dashdemux as a `data:` URI. Track 1004 serves it as `application/dash+xml` | dashdemux fetched `init.mp4` and segments 1–3 through the proxy. Exclusive bit-perfect output is **bit-exact** (`S24_32LE`) against the FLAC's PCM. The UI shows "24-BIT 96kHz FLAC, HI-RES LOSSLESS" |
| Artwork | Every `resources.tidal.com/images/...` request gets `cover.jpg` | Three sizes requested through the proxy. The cover renders in the now-playing bar, so the webview trusts the proxy CA through `SSL_CERT_FILE` (glib-networking on Nix honours it) |

New findings:

- **MCP's `positionSeconds` stays 0** throughout playback in the reference: the state mirror
  doesn't track position. **Use MPRIS `Position` for timing assertions.** Whether the Rune
  port should fix this or preserve it is a decision to record as a waiver or a bug.
- **Images are not only loaded by WebKit:** the artwork requests carried no User-Agent. The
  earlier assumption that "the reference loads images from WebKit and the candidate from Rust"
  needs checking before the image-exclusion rule in L2 is finalised.
- **Stream fixtures by track ID:** 1001 BTS 44.1/16, 1002 BTS 96/24, 1003 BTS 192/24, 1004
  DASH 96/24. Every one decodes to a known PCM checksum (`fixtures/pcm.sha256`).

## Harness v0 (2026-11-08)

`verify.py` runs scenarios in parallel sandboxes. `record` stores a golden only when N runs of
the reference agree with each other; `check` compares a build against the goldens.

**Status:** 18 scenarios. 15 have goldens and **pass 3 of 3 fresh runs against the reference**
(45 runs, no flakes). 3 are quarantined (see below).

| Family | Scenarios |
|---|---|
| Playback | play-single, queue-advance, skip-next, skip-previous-shared, pause-resume-shared, seek-shared |
| Queue | queue-ops (append, play-next, remove, clear) |
| Modes | repeat-one, repeat-all, shuffle-toggle |
| Integrations | mpris-control, tray-control |
| Audio path | dash-hires, rate-switch (44.1 → 96 → 192 kHz), volume-not-bitperfect |

**The audio channel** (`channels/audio.py`) maps the raw capture back onto fixtures. All fixtures
are seeded noise, so any 64-frame window identifies one track and one frame. The output is an
ordered list of segments (track, frame range, full or partial, silence before), which shows
skips, cuts, repeats, resampling and gaps in one structure. The ALSA file plugin runs with
`truncate false`, which writes one file per PCM open (`audio.raw.0001`, …), so reopens don't
erase earlier tracks.

**Comparator tolerances:** position ±1.5 s, volume ±0.02, partial-play edges ±1.0 s, inter-track
silence ±0.25 s. The lossless container (S16/S24_32/S32) is not compared; the sample rate is.
Network calls are compared as a per-step set, with artwork and the update check excluded.
Startup traffic is kept out of step 0 by waiting for 2 s of network quiet before the first step.

**Sensitivity:** the comparator catches behaviour changes. Running `volume-not-bitperfect` with
`--profile '{"bit_perfect": true}'` fails on volume (1.0, expected 0.5) and on audio (bit-exact,
expected scaled). App-level mutation builds are still to do (build order step 3).

**Reference behaviour the goldens now pin down** (decide per item whether the port preserves
it or fixes it; a fix needs a waiver):

- **Exclusive mode is not gapless:** there are 100 ms of silence between same-format tracks
  (each track reopens the PCM). Rate changes insert 23 ms (44.1 → 96 kHz) and 50 ms
  (96 → 192 kHz).
- **Bit-perfect ignores volume:** `set_volume` leaves MPRIS volume at 1.0 and output bit-exact.
- **Bit-perfect appears to force exclusive output:** with `exclusive_mode = false` and
  `bit_perfect = true`, audio still reaches the exclusive device.
- **Volume is owned by the frontend:** `settings.json`'s `volume` is overridden by the
  frontend's `sone.volume.v1` in WebKit storage. That's another reason the L6 migration matters.
- **MCP `positionSeconds` is always 0;** use MPRIS for position.
- **Previous early in a track goes to the previous track** (skip-previous-shared, tray-control).

**Quarantined, for one shared reason:** in exclusive mode, anything that pauses or seeks
(`seek`, `pause-resume`, `skip-previous`'s late restart) is nondeterministic in the sandbox.
Playback stalls with position frozen and only silence written, or the track restarts from
frame 0 on resume, or it continues correctly. The same scenarios in shared mode are
deterministic and correct (resume continues from 1.52 → 2.55 s). So this is either the app's
exclusive ALSA writer or the PipeWire ALSA plugin underneath it. **Next step:** reproduce on a
real DAC (`exclusive_device = "hw:…"`), outside the sandbox. If real hardware behaves, the
sandbox needs a different clocked slave. If it doesn't, it's an upstream bug worth reporting.

## Port slice 1: playback core on Rune (2026-11-08)

The first candidate exists (`rune-app/`, see its README). SONE's backend runs unchanged behind
`src-tauri`'s new `host` feature, with `tauri::` aliased to `src-tauri/host-shim`. The React
playback logic is re-implemented in `rune-app/src/controller.rs` against
`rune-app/PLAYBACK_SPEC.md`, and the UI is the Afterglow kit from `~/dev/music-player`. The
Tauri build (`tauri-ui`, the default feature) still compiles; `run()` now calls the shared
`setup_core()`.

**Result: the candidate passes every non-quarantined golden on behaviour (L2), 3 of 3 runs**
(`check --runs 3 --ignore controls,unnamed_controls`). That covers MPRIS and MCP state, the API
calls at each step, and bit-exact audio, including DASH hi-res, rate switching, gapless timing
and repeat. L3 (the accessibility inventory) fails as expected: SONE's sidebar, library, search
and account views aren't ported. The candidate's own controls are all named (no unnamed
buttons, unlike the reference's player bar).

Harness changes this slice needed, applied to both builds:

- **Accessibility switched on** (keyfile GSettings, `toolkit-accessibility=true`): AccessKit
  registers only for an active assistive technology. The reference still passes all goldens in
  full under this.
- **Software Vulkan** (`SONE_SOFTWARE_VULKAN=1` → lavapipe via the candidate's wrapper): the
  headless compositor has no GPU.
- **`--ignore KEYS`** to judge a layer at a time.
- **`mpris-control` split:** MPRIS Pause/Play moved to `mpris-pause-play-shared`. In exclusive
  mode, the audio after a resume flaked on the *candidate* the same way the quarantined
  scenarios flake (skipped ~0.37 s in one run, silence in the other). That's below the
  controller, so it's the same open question as the quarantine.

## Port slice 2: views, by hill climbing (2026-11-08)

**Validator.** Three pieces turned the verifier into something to climb:

- **Synthetic catalog** (`fixtures/catalog/`, generator `gen.py`, see its README). The fake TIDAL
  serves Home, Search (every tab and suggestions), Album, Artist, the sidebar playlists and the
  Playlist page from deterministic JSON. Artist 7 "Fixture Artist", artist 8 "Mock Ensemble",
  album 77 "Fixture Album" (1001–1004), album 78 "Queue Suite" (1005–1007), playlists
  "Fixture Mixtape" and "Synthetic Selections". `/v1/tracks/{id}` agrees with the catalog. All
  goldens were re-recorded on it.
- **View scenarios** with `views = true`: the runner snapshots the visible named controls after
  every step. New step forms: `ui = { type = TEXT, into = NAME }` (AT-SPI focus, then real
  keystrokes with `wtype`), `key = "Return"` or `"ctrl+a"` (a persistent virtual keyboard,
  `channels/keyboard.py`), and `nth` to tell repeated names apart. Presses
  fall back to the nearest actionable ancestor, so cards are clicked through their heading.
  Scenarios: view-home, view-search (dropdown, results, every tab), view-album, view-artist,
  view-playlist (each also plays the collection), view-library.
- **`verify.py score`.** Per scenario it reports controls recall/precision, net recall,
  behaviour and audio. It takes the worst of `--runs`, compares against a saved baseline
  (`--baseline`, `--save` into `verifier/scores/`), and lists the reference controls most often
  missing and the most common extra controls. `waivers.toml` holds written-down intentional
  differences; so far that's the candidate naming controls the reference leaves unnamed.

**The climb** (`verifier/scores/rune-*.json`, overall score):

| Step | Change | Overall |
|---|---|---|
| baseline | playback-only Rune build | 0.417 |
| 1 | app shell: sidebar + library, header + search with suggestions, window buttons, Home/Search/Album/Artist/Playlist pages, player-bar extras, collection play in the controller | 0.879 |
| 2 | read the backend's flattened Home items (`_itemType`) | 0.939 |
| 3 | tall display for Rune builds (see below) | 0.940 |
| 4 | cards and rows presentational (heading carries the press), inner text of labelled controls hidden, no player-bar landmark, 300 ms suggestion debounce, artist Play falls back to the page's top tracks | 0.978 |
| 5 | open the Rune window at display size (see below) | 0.986 |
| 6 | album-page modules typed by module, playlist title as heading | **1.000** |

After step 6, a strict run of all three runs first exposed a scoring blind spot. Net counted only
*missing* requests, so per-keystroke suggestion fetches (the dropdown fetched on render) scored
1.00. Net is now an F1, and the dropdown only peeks at the cache. **Final strict check** (`check`, behaviour and
accessibility inventory, nothing ignored, 3 runs each, `--display 1400x2000`): **21 of 22
scenarios pass every run (65 of 66 runs).** The exception is `repeat-one`, 1 run in 3: the
candidate's repeat replay sometimes produces audio that the reference's never did inside the
window, which looks like the same exclusive-mode replay/reopen nondeterminism as the quarantine.

**Findings about Rune/AccessKit that the verifier surfaced:**

- **AccessKit hides scroll content outside the viewport** from AT-SPI, keeping only the first
  sibling beyond the edge, and exposes no scroll action over AT-SPI (only `click`). WebKit
  exposes the whole page. So Rune builds are scored on a 1400×2000 display (`--display`),
  while the reference stays at 1400×900 where its goldens were recorded. A tall WebKit window
  made the reference's timing-sensitive React logic flaky under parallel load. Long lists will
  need a real scroll sweep.
- **Rune keeps a window's first bounds for AccessKit** after the compositor resizes it, so
  content outside the original 1280×800 was clipped from the tree even on the tall display.
  It's worth reporting upstream. Workaround: `SONE_WINDOW_SIZE` opens the window at display size.
- **A pressable container becomes a `button`** named from all its text, unless its role is
  `presentation`. Visible text inside a labelled control shows up as an extra `label` node
  unless it's `a11y_hidden`.

## Port slice 3: settings, sign-in, queue, lyrics, miniplayer, Explore, Feed (2026-11-08)

**Validator additions:**
- **Catalog:** Explore (pill rows, an icon grid and 11 sub-pages), Feed (three activities, a
  mix, the seen PUT), synced lyrics for 1001/1005 (404 for the rest), credits, and the
  MusicBrainz ISRC lookup. The device-code sign-in is now stateful: the first poll answers
  `authorization_pending`, the next one succeeds. See catalog/README.md.
- **Harness:**
  - `settings = [keys]` records the decrypted `settings.json` values after each step.
  - `logged_out = true` starts without tokens.
  - The AT-SPI channel searches every accessible app of the process (for second windows).
  - `xdg-open`, `$BROWSER`, firefox and chromium are stubbed inside the sandbox to record the URL
    (`final.opened`). The reference was really launching Firefox during sign-in.
  - Each step waits for 0.4 s of network quiet before cutting its request slice, which removes a
    step-boundary race.
- **Scoring:** net is an F1, so extra requests now cost.
- **Scenarios:** view-queue, view-account, view-settings (every tab, plus the quality change
  persisted), view-miniplayer, signin (login screen → code → Home, tokens persisted, the
  approval URL opened), view-lyrics, view-explore, view-feed.

**Port:**
- Account menu: toggles stay open, actions close it.
- Settings as a modal with all nine tabs; quality writes through `set_max_quality`.
- Bit-perfect and exclusive toggles follow `setBitPerfect` (save, ramp, restore).
- A right-hand drawer with Play queue / Suggested tracks / Lyrics / Credits tabs, with synced
  lyric lines.
- The miniplayer as a second Rune window.
- Explore and genre/view-all pages: modules typed by module type; no heading on `PAGE_LINKS`.
- Feed: grouped by age, Play / More options per row, marked seen on open.
- Login screen: PKCE / Login Code modes, device code with the approval page opened.
- The post-sign-in bootstrap makes the same calls AppInitializer makes. It uses the id-list
  commands, so the library tabs still fetch when opened.

Scores: step 7 (first build with the new screens) 0.987 → step 9 0.998 across 30 scenarios
(`verifier/scores/rune-step9.json`). The per-step quiet wait shifts every timed state, so all
30 goldens were re-recorded with it (3 of 3 runs each). **Final strict check** (behaviour and
accessibility inventory, nothing ignored, 3 runs, 1400×2000): **29 of 30 scenarios pass every
run.** The exception is `repeat-one`, which fails 2 runs in 3. The Rune build's repeat replay
produces audio inside the window, and the reference's replay never does, in any recorded run.
Both call the same `play_tidal_track` in exclusive mode, so this sits with the exclusive-mode
replay/reopen question. It is not settled whether the reference stalls or the candidate is too
quick.

**Not covered:** the Credits and Suggested tracks tabs' content, settings fields' editing
(present and named, not wired), the PKCE login itself (it needs the embedded webview, which the
Rune build can't host; the system-browser flow is the alternative), Shortcuts and About dialogs,
music videos, and text content of lyrics (the inventory compares controls, not text).

## Port slice 4: drawer tabs, settings editing, dialogs (2026-11-08)

- **Catalog:** every track carries `mixes.TRACK_MIX`, with a fixture track-radio mix; ListenBrainz
  `validate-token` answers for user "fixture". All goldens were re-recorded.
- **Harness:**
  - key chords (`key = "ctrl+a"`) and `select_all` on typing;
  - credential-like query values (`api_key`, `api_sig`, `token`, …) are redacted in the network
    channel and never reach goldens;
  - the unnamed-control count tolerates ±2 (popovers);
  - `view-drawer-tabs` pauses first, so the current track can't change mid-scenario.
- **Scenarios:** view-drawer-tabs (Suggested tracks from the radio, Credits with bio),
  view-shortcuts, view-about (a link opens the repository URL), settings-edit (Discord status text,
  ListenBrainz token + Connect → connected state, overlay host + Apply, proxy username; values
  checked in `settings.json`).
- **Port:**
  - The drawer header shows the current track on every tab.
  - Suggested tracks come from the track radio, with Add to queue / Add to Loved tracks.
  - Credits shows contributors by role plus the artist bio.
  - Shortcuts and About are dialogs; About's links open the same URLs.
  - Settings fields are bound and saved with the React debounces: Discord 500 ms; proxy 500 ms,
    withheld while half-typed. ListenBrainz connect/disconnect redraws from the saved state.
    Overlay Apply acts only while the overlay is enabled, as in React.
  - Waiver: the dialogs' close button is named (it's unnamed in the reference).
- **Finding: `repeat-one` is nondeterministic in the reference itself.** Recording three runs
  gave one replay stalled at 0 and two playing on, which is exactly the "candidate flake" seen
  before. It is now quarantined with the other exclusive-mode replays, and
  `repeat-one-shared` covers the logic.

**Strict check:** all 34 non-quarantined scenarios pass every run (3 runs, 1400×2000). Four are
quarantined, all exclusive-mode pause/seek/replay.

## Benchmark: release builds (2026-11-08)

`verifier/bench.py` runs both release builds in the verifier's sandbox (fake TIDAL, seeded
logged-in profile, virtual audio, headless sway at 1400x900, software rendering: WebKit's
software path, Rune on lavapipe). Five alternating runs per build, medians. `bench/driver.py`
measures the whole process tree from /proc, so WebKit's web and network processes count.
Results: `verifier/bench/results/2026-11-08.{md,json}`.

| Metric | SONE (Tauri/WebKit) | SONE on Rune |
|---|---|---|
| Startup, Home content shown | 2.35 s | 2.39 s |
| Memory idle on Home (PSS) | 363 MB, 3 processes | 206 MB, 1 process |
| Memory while playing (PSS) | 491 MB | 322 MB |
| CPU idle | 0.7 % | 0 % |
| CPU while playing | 36.6 % | 27.2 % |
| Open album / artist / search | 0.51 / 0.43 / 0.20 s | 0.13 / 0.14 / 0.14 s |
| Main binary | 43.3 MB (plus system WebKitGTK) | 75.1 MB |

Finding: the first run showed Rune at **320 % of a core while playing**. Counting
`wl_surface.commit` with `WAYLAND_DEBUG` showed the main window presenting at 50 fps whenever a
track played (0 fps idle). Bisecting the now-playing bar pinned it to the seek slider: its fill's
`w_frac` changed on every poll and its layout spring never settled. The first fix took the
springs off (the seek fill, the eq bars, the cover rotation), which brought it to ~1.6 fps and
27 % CPU; the GPU work below restores those animations properly. Lesson for the port: anything
that changes on a timer must not re-run motion, and a frame counter belongs in the budget checks.

## Benchmark: GPU at 120 Hz, with frame pacing (2026-11-08)

Rune is a GPU framework, and the software numbers above mostly measure lavapipe. The sandbox
now takes `SPIKE_GPU=1` (sway's Vulkan renderer on the render node; WebKit and Rune on the
machine's NVIDIA GPU; `/dev/snd` stays hidden) and `SPIKE_REFRESH` (the headless output's
refresh rate). `bench.py --gpu --refresh 120` runs any number of builds (`--build
"LABEL=BIN,VAR=VALUE"`), each run in two passes: `bench/driver.py` (memory, CPU, latency) and
`bench/frames.py` (presented frames from the `WAYLAND_DEBUG` log: idle, playing, playing with the
animated bars on screen, a scripted wheel scroll, opening and closing a dialog). The wheel is a
persistent wlr virtual pointer (`bench/wheel.py`, pywayland); one-shot tools lose the axis events.

Two references, because SONE itself sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` when it sees NVIDIA
("to avoid WebKitGTK GBM allocation failure"): **SONE as it ships** on this machine (WebKit's
GPU renderer off), and **SONE with it forced on** (`WEBKIT_DISABLE_DMABUF_RENDERER=0`, which
works here). Five alternating runs, medians. Results: `bench/results/2026-11-08-gpu-120hz.{md,json}`.

| Metric | SONE as shipped | SONE, WebKit GPU forced | SONE on Rune |
|---|---|---|---|
| Startup, Home content shown | 2.29 s | 2.22 s | 2.08 s |
| Memory idle (PSS) | 350 MB, 3 processes | 489 MB | 127 MB, 1 process |
| Memory while playing (PSS) | 479 MB | 582 MB | 164 MB |
| CPU idle | 0.5 % | 0.4 % | 0.2 % |
| CPU while playing (includes starting playback) | 16.3 % | 39.4 % | 8.3 % |
| CPU while playing, bars animating | 6.1 % | 22.6 % | 4.1 % |
| Open album / artist / search | 0.52 / 0.43 / 0.20 s | 0.12 / 0.13 / 0.13 s | 0.03 / 0.10 / 0.11 s |
| Frames/s while playing (nothing animating) | 2 | 125 | 1.2 |
| Bars animating: frames/s, worst gap | 65, 21 ms | 125, 9 ms | 123, 10 ms |
| Wheel scroll: frames/s, worst gap | 47, 43 ms | 83, 20 ms | 115, 10 ms |
| Dialog open/close: frames/s, worst gap | 20, 75 ms | 50, 64 ms | 65, 9.5 ms |
| Main binary | 43 MB (plus system WebKitGTK) | | 75 MB (not a target yet) |

At 120 Hz a frame is 8.3 ms, so a worst gap near 10 ms means no dropped frames: Rune holds the
display rate in every animation and draws nothing it doesn't need (WebKit with its GPU renderer
presents 125 fps the whole time a track plays). Fixed-size fixtures: a 10 s track, short pages.

Animations, done the Rune way (`rune-app/src/ui.rs`):
- **Playing bars** (SONE's CSS `barBounce`): renderer-owned `Sequence` scale beats on the same
  curve, each bar a wide block scaled inside a 3 pt clip. No rebuilds, layout or wakeups. A
  `Cycle` opacity loop was tried first: it wakes on a timer and landed every other 120 Hz frame.
  The album, playlist, artist and search rows also gained SONE's indicator (bars when playing,
  accent number and title for the current track); only the queue drawer had it.
- **Seek fill**: glides on a short spring when a seek, a click or a new track moves it; the
  clock's once-a-second ticks land with `Tx::instant()`. The 50 fps loop came from the controller
  republishing its view on every backend event (position syncs): each publish counted as a state
  change, so its commit animated. It now signals only when the drawn view differs or the position
  jumps, and clock ticks are counted apart (`controller::versions()`).
- **Wheel scrolling**: Rune applies a notched wheel's line steps at once (40 pt a notch) and
  leaves smoothing to the app (`WheelEvent::precise`). `smooth_wheel` glides each notch with
  `Tx::scroll_to_with`, which keeps a running glide's speed, so a spin is one continuous motion.
- **Cover rotation**: never used (every caller passes 0); nothing to restore.

Still to measure from L5: a 10k-track playlist, a 30-minute gapless underrun run, and a
real-account run (network, user signs in, virtual audio only). The verifier itself still runs
in software at 60 Hz (34/34 pass with these changes).

## Benchmark: a real TIDAL account (2026-11-08)

The same GPU, 120 Hz benchmark against the real catalogue. `SPIKE_NET=1` gives the sandbox the
host's network (no fake TIDAL, no proxy) while audio, the system bus and `/dev/snd` stay private:
playback goes bit-perfect into the virtual capture device, never to a real card.
`bench/signin_real.py` runs SONE's device-code sign-in once (the account owner approves the link
in their own browser; nothing types a password) and saves the profile (`sone.key` + encrypted
`settings.json`); `SPIKE_PROFILE_DIR` copies it into every run and keeps the decrypted settings
out of the logs. `bench.py --real-profile DIR [--album TITLE]`; `bench/targets.py` searches the
album over MCP, then the drivers reach it through the UI: search from Home (timed to the first
result), the Albums tab (on "All", a playlist of the same name can come first), the album
(timed to its Shuffle button, present whether or not it's a favourite). No artist page: a real
"Daft Punk" is also an album title, and Rune's album page doesn't link the artist yet.

To stay ordinary account activity: one app session at a time on the same signed-in device, 45 s
between launches, one search, one album and a few seconds of two tracks per pass, three runs of
SONE as shipped and Rune (12 launches in ~25 minutes). Results:
`bench/results/2026-11-08-gpu-120hz-real.{md,json}` (album: Random Access Memories).

| Metric | SONE as shipped | SONE on Rune |
|---|---|---|
| Startup, Home content shown | 4.42 s | 2.16 s |
| Memory idle / playing (PSS) | 380 / 547 MB | 165 / 229 MB |
| CPU idle | 0.6 % | 0.2 % |
| CPU playing, on Home | 4.1 % | 1.3 % |
| CPU playing, album page (bars animating) | 40.7 % at 64 fps | 11.4 % at 123 fps |
| Search to first result / open album | 0.85 / 0.98 s | 0.60 / 0.40 s |
| Wheel scroll: frames/s, worst gap | 54, 35 ms | 115, 11 ms |
| Dialog open/close: frames/s, worst gap | 43, 44 ms | 64, 10 ms |

Found on the way:
- Fixed: the search suggestions dropdown was pinned at a fixed spot near the left; it's now
  `anchored_fit` under the search field, right-aligned, 420 pt wide, as SearchBar.tsx.
- Fixed: Rune's album page showed the artist as plain text; each artist is now a link to their
  page (SONE's TrackArtists spans are clickable but not accessible; waived as an extra control).
- Not a Rune bug: Ctrl+A looked broken in the search field, but the harness's wtype chords never
  arrived unpaced (no wl_keyboard events at all), and paced ones reached Rune (which selects
  all) but not WebKit. Unpaced single keys (`key = "Escape"`) were dropped too. Fixed:
  `channels/atspi.key` now presses keys on a persistent zwp_virtual_keyboard_v1
  (`channels/keyboard.py`) with the full default keymap and real Control_L presses, and both
  apps select all. Re-recorded goldens: settings-edit (the overlay host is now replaced,
  127.0.0.1 → 127.0.0.2, where the append used to leave it unchanged) and view-account (Escape
  now closes the account menu). The bench clears the search field with Ctrl+A, and its
  `modal` window now includes the dialog closing (before, Escape never arrived).
- Rune exposes only on-screen content to AT-SPI (AccessKit), so at 1400x900 a results page's
  lower sections can't be found by name (the verifier uses a 1400x2000 display for this).

## Rune upgrade and parity waves (2026-11-13 and 14)

Earlier sections say videos, settings and most views aren't ported; that is history. As of
`rune-port` at `29a3fe1`:

- **Rune pin** moved from `58eeef5` to `6c3cb8a`. The view is five regions over tracked state,
  track lists are virtual, and `bench/states.py` measures fixed states (player closed/open,
  paused, unfocused, covered, hidden, a large track list) from `SONE_PERF` lines. Results are in
  `bench/results/states-2026-11-13-*`: the ~100 % CPU spin with the window covered went away with
  the pin, a 3000-row list that used to pass 3 GB holds near 500 MB, and scrolling no longer runs
  the view per wheel notch. A playing track list still redraws the window at 60 fps (the playing
  row's bars).
- **Parity** against the React source was audited statically three times: before the work, after
  the first wave (at `2d1bc21`) and after the fix wave (at `9bd8d26`), then the last findings were
  fixed. Counts at the last audit, at parity of examined: playback core 91/103, now-playing
  88/105, videos 48/63, collections ~113/~125, menus ~64/76, discovery ~61/77; the shell and
  settings lists were closed except what `rune-app/README.md` lists under "Not done". Every backend
  command the React frontend invokes is called or replaced.
- **Harness changes:** `--ignore` now applies to the start-up state as it does to steps
  (`harness/compare.py`, tests in `harness/test_compare.py`), which let Home gain named controls
  and the player bar become a named landmark; `fake_tidal.py` gained opt-in fixtures
  (`SPIKE_BENCH_BIG`, `SPIKE_FOLDERS`, `SPIKE_HOME_EXTRA`, `SPIKE_HOME_TABS`, `SPIKE_COLLECTIONS`,
  `SPIKE_ARTIST_VIEWALL`, `SPIKE_RECS`, `SPIKE_PROFILE_ARTIST`, `SPIKE_PLAYLIST_VIDEO`); one waiver
  was added (`button: Dismiss`, the legacy-sign-in toast's close button).
- **Still true:** Rune builds need the 1400x2000 display; L3 (the full accessibility inventory,
  without `--ignore`) fails on control names that differ from the reference; `verify.py run`
  crashes after a scenario with `audio = false` (`audio.summary`, KeyError 'segments').

## Tauri removed (2026-11-14)

The Rune app replaces the Tauri app on this branch; `main` keeps the Tauri build.

- **Removed:** the React frontend, its tooling, the Tauri packaging and CI, the `tauri-ui`
  feature and every Tauri, WebKit and GTK dependency, the stand-in crate `host-shim` (now the
  module `core/src/host.rs`), and the code only a webview window could reach (the embedded
  sign-in window, the webview proxy credentials, the Wayland idle-inhibit layer that needed the
  webview's surface). `src-tauri` is now `core`, the crate `sone-core`.
- **Found while removing it:** the Tauri entry point settled the proxy environment and set
  `SOUP_FORCE_HTTP1` before starting; the Rune app never did either. Both now run from
  `core/src/launch.rs`, called first in `main()`, under the same source guards.
- **The reference is frozen.** The goldens were recorded from the Tauri build and can no longer
  be re-recorded from this branch. They are the regression baseline for the Rune app; a golden
  that has to change is changed by hand, with the reason in the commit.
- **Checked after the removal:** backend tests 414 pass (391 unit, 23 integration and source
  guards), app tests 109 pass.

## Build order

1. ~~Spikes~~, ~~sandbox completion~~, ~~harness v0~~, ~~port slice 1 (playback core)~~,
   ~~port slice 2 (views)~~ and ~~port slice 3~~ (done, see above).
2. Settle the exclusive-mode pause/seek/resume question (now seen on both builds) without real
   hardware: `snd-aloop` as the clocked slave (`sudo modprobe snd-aloop`, silent). Then
   un-quarantine.
3. Mutation checks on the reference (break shuffle, drop a scrobble, skip gapless prefetch). The
   suite must catch each one.
4. Expand coverage on the reference (library, search, views, settings, auth, scrobbling,
   miniplayer, migration) and port each area as its scenarios land. Gate per area: L2 and L3
   pass on the candidate.
