#!/usr/bin/env python3
"""Write a deterministic stereo noise WAV: gen_noise.py OUT.wav RATE BITS SECONDS SEED

Seeded white noise at about -12 dBFS. Unlike a tone, every short window of noise is unique, so
the audio channel can say exactly which track and which frame any captured audio came from
(skips, seeks, partial plays, repeats). A different seed per track keeps tracks distinct.
"""
import random
import struct
import sys
import wave

out, rate, bits, secs, seed = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4]), int(sys.argv[5])
rng = random.Random(seed)
peak = (1 << (bits - 1)) // 4
frames = int(rate * secs)
width = bits // 8
buf = bytearray()
for _ in range(frames * 2):
    v = rng.randint(-peak, peak)
    buf += v.to_bytes(width, "little", signed=True)
with wave.open(out, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(width)
    w.setframerate(rate)
    w.writeframes(bytes(buf))
