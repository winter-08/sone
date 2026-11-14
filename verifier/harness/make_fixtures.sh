#!/usr/bin/env bash
# Deterministic FLAC fixtures plus the exact PCM each one decodes to (little-endian, signed,
# interleaved), so captured audio can be compared byte for byte.
set -euo pipefail
D="$(cd "$(dirname "$0")/.." && pwd)/fixtures"; mkdir -p "$D"; cd "$D"
# Seeded noise, never tones: a tone repeats every few hundred frames, so a captured window
# couldn't be placed in its track. Names are historical; all are noise.
G="python3 -I $(dirname "$D")/harness/gen_noise.py"
$G tone_44k16.wav  44100  16 10 1001
$G noise_96k24.wav 96000  24 10 1002
$G tone_192k24.wav 192000 24 10 1003
# Short same-format tracks for queue scenarios.
$G qa_44k16.wav    44100  16 8  1005
$G qb_44k16.wav    44100  16 8  1006
$G qc_44k16.wav    44100  16 8  1007
for w in *.wav; do
  b="${w%.wav}"
  flac -s -f --best -o "$b.flac" "$w"
  flac -s -f -d --force-raw-format --endian=little --sign=signed -o "$b.pcm" "$b.flac"
  rm "$w"
done
sha256sum *.pcm > pcm.sha256
cat pcm.sha256

# DASH: the 96k/24 FLAC stream-copied into fMP4 segments, as TIDAL serves hi-res. SONE hands
# the MPD to GStreamer as a data: URI, so segment URLs in the manifest must be absolute.
rm -rf dash && mkdir -p dash/noise
ffmpeg -loglevel error -i noise_96k24.flac -c:a copy -f dash -seg_duration 4 \
  -use_template 1 -use_timeline 0 -init_seg_name 'init.mp4' -media_seg_name 'seg-$Number$.mp4' \
  dash/noise/manifest.mpd
sed -i -E 's#(initialization|media)="#\1="https://fixtures.sone.test/dash/noise/#g' dash/noise/manifest.mpd
ls dash/noise

# Artwork, served for every resources.tidal.com/images/... request.
ffmpeg -loglevel error -y -f lavfi -i 'color=c=0x2a9d8f:s=640x640,drawbox=x=160:y=160:w=320:h=320:color=0xe9c46a:t=fill' \
  -frames:v 1 cover.jpg

# Animated album cover (SPIKE_VIDEO_COVER=1 in fake_tidal.py): a 4 s loop with no sound track, in
# the two sizes TIDAL serves (resources.tidal.com/videos/<uuid>/{640x640,1280x1280}.mp4). An
# orange square crosses the teal, so screenshots tell it from the still (teal with a yellow
# square) and one frame from the next.
mkdir -p videocover
for n in 640 1280; do
  q=$((n / 4))
  ffmpeg -loglevel error -y -f lavfi -i "color=c=0x2a9d8f:s=${n}x${n}:r=25:d=4" \
    -f lavfi -i "color=c=0xe76f51:s=${q}x${q}:r=25:d=4" \
    -filter_complex "[0][1]overlay=x='(W-w)*t/4':y='(H-h)/2':shortest=1,format=yuv420p" \
    -an -c:v libx264 -preset veryfast -g 25 -movflags +faststart "videocover/${n}x${n}.mp4"
done
ls -l videocover
