"""Audio channel: map a raw ALSA capture back onto the fixtures it came from.

The capture is whatever bytes SONE wrote to the `sone_capture` PCM, in whatever container it
chose. Fixtures are seeded noise, so any 64-frame window identifies one track and one frame.
The result is an ordered list of segments: which track, which frames, bit-exact or not, and
the silence between them. That one list answers "what played, in what order, was anything
skipped, cut, repeated, resampled or gapped".
"""
import json
import os
import re

import numpy as np

WINDOW = 64      # frames in a lookup key
STRIDE = 512     # index a key every STRIDE frames of each fixture
CHANNELS = 2


def _containers(samples: np.ndarray, bits: int) -> dict[str, tuple[bytes, int]]:
    """Every lossless container a `bits`-deep fixture may arrive in: name -> (bytes, frame bytes)."""
    s = samples.astype(np.int32)
    if bits == 16:
        return {"S16LE": (s.astype("<i2").tobytes(), 4),
                "S24_32LE": ((s << 8).astype("<i4").tobytes(), 8),
                "S32LE": ((s << 16).astype("<i4").tobytes(), 8)}
    return {"S24LE": (None, 6),  # filled below (packed 3-byte)
            "S24_32LE": (s.astype("<i4").tobytes(), 8),
            "S32LE": ((s << 8).astype("<i4").tobytes(), 8)}


def _load_pcm(path: str, bits: int) -> tuple[np.ndarray, bytes]:
    raw = open(path, "rb").read()
    if bits == 16:
        return np.frombuffer(raw, "<i2"), raw
    b = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
    v = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
    v = np.where(v & 0x800000, v - 0x1000000, v)
    return v, raw


class Index:
    def __init__(self, fixtures_dir: str, track_ids: list[int] | None = None):
        spec = json.load(open(os.path.join(fixtures_dir, "tracks.json")))
        self.tracks = {}   # (track, container) -> (bytes, frame_bytes, frames, rate)
        self.keys = {}     # window bytes -> (track, container, frame)
        self.widths = set()
        for k, v in spec.items():
            if not k.isdigit() or (track_ids and int(k) not in track_ids):
                continue
            tid = int(k)
            samples, raw = _load_pcm(os.path.join(fixtures_dir, v["pcm"]), v["bits"])

            for name, (data, fb) in _containers(samples, v["bits"]).items():
                data = raw if data is None else data
                frames = len(data) // fb
                self.tracks[(tid, name)] = (data, fb, frames, v["rate"])
                w = WINDOW * fb
                self.widths.add(w)
                for f in range(0, frames - WINDOW, STRIDE):
                    self.keys.setdefault(data[f * fb:f * fb + w], (tid, name, f))


def _extend(cap: bytes, c: int, fix: bytes, f: int, fb: int) -> tuple[int, int]:
    """Grow a match at capture byte c / fixture frame f in both directions, frame-aligned.
    Returns (capture start byte, fixture start frame, frames)."""
    # backwards
    back = 0
    while f - back > 0 and c - (back + 1) * fb >= 0 and \
            cap[c - (back + 1) * fb:c - back * fb] == fix[(f - back - 1) * fb:(f - back) * fb]:
        back += 1
    start_c, start_f = c - back * fb, f - back
    # forwards in chunks, then frame by frame inside the first mismatching chunk
    n = back
    pos_c, pos_f = c, f
    chunk = 4096
    while True:
        a = cap[pos_c:pos_c + chunk * fb]
        b = fix[pos_f * fb:(pos_f + chunk) * fb]
        m = min(len(a), len(b)) // fb
        if m == 0:
            break
        if a[:m * fb] == b[:m * fb]:
            pos_c += m * fb; pos_f += m; n += m
            if m < chunk:
                break
            continue
        for i in range(m):
            if a[i * fb:(i + 1) * fb] != b[i * fb:(i + 1) * fb]:
                break
            n += 1
        break
    return start_c, start_f, n


_NONZERO = re.compile(rb"[^\x00]")


def capture_files(capture_path: str) -> list[str]:
    """ALSA's file plugin (truncate=false) writes one file per PCM open: PATH, PATH.0001, …"""
    d, base = os.path.split(capture_path)
    if not os.path.isdir(d):
        return []
    names = [n for n in os.listdir(d) if n == base or re.fullmatch(re.escape(base) + r"\.\d{4}", n)]
    return [os.path.join(d, n) for n in sorted(names, key=lambda n: int(n.rsplit(".", 1)[1]) if n != base else -1)]


def analyze(capture_path: str, index: Index) -> dict:
    files = capture_files(capture_path)
    if not files:
        return {"captured_bytes": 0, "segments": []}
    # Opens are concatenated in order; every open starts on a frame boundary, so alignment holds.
    cap = b"".join(open(f, "rb").read() for f in files)
    segs, unknown, p = [], 0, 0
    last_end = 0
    while True:
        m = _NONZERO.search(cap, p)
        if not m:
            break
        p = m.start() & ~1  # 2-byte alignment covers every container's sample size
        hit = None
        scan_to = min(len(cap), p + STRIDE * 8 * 2)  # a key must appear within one stride
        q = p
        while q < scan_to and hit is None:
            for w in index.widths:
                k = cap[q:q + w]
                if len(k) == w and k in index.keys:
                    hit = (q, index.keys[k])
                    break
            q += 2
        if hit is None:
            # non-silent audio that is in no fixture: resampled, volume-scaled, dithered...
            unknown += scan_to - p
            p = scan_to
            continue
        q, (tid, cont, f) = hit
        data, fb, frames, rate = index.tracks[(tid, cont)]
        c0, f0, n = _extend(cap, q, data, f, fb)
        gap_bytes = c0 - last_end
        segs.append({
            "track": tid, "container": cont, "rate": rate,
            "start_frame": f0, "end_frame": f0 + n, "frames": frames,
            "full": f0 == 0 and f0 + n == frames,
            "silence_before_frames": max(0, gap_bytes // fb) if segs else None,
        })
        last_end = p = c0 + n * fb
    return {"captured_bytes": len(cap), "pcm_opens": sum(1 for f in files if os.path.getsize(f)),
            "unmatched_nonsilent_bytes": unknown, "segments": segs}


def summary(result: dict) -> list[str]:
    out = []
    for s in result["segments"]:
        span = "full" if s["full"] else f"{s['start_frame'] / s['rate']:.2f}-{s['end_frame'] / s['rate']:.2f}s"
        gap = "" if s["silence_before_frames"] is None else f" after {s['silence_before_frames'] / s['rate']:.3f}s silence"
        out.append(f"{s['track']} {span} [{s['container']}@{s['rate']}]{gap}")
    if result.get("unmatched_nonsilent_bytes"):
        out.append(f"unmatched non-silent bytes: {result['unmatched_nonsilent_bytes']}")
    return out


if __name__ == "__main__":
    import sys
    r = analyze(sys.argv[1], Index(sys.argv[2], [int(x) for x in sys.argv[3:]] or None))
    print("\n".join(summary(r)) or "no audio")
