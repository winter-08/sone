//! Read-only audio level tap for audio-reactive visuals.
//!
//! The playback paths call [`tap`] with the decoded PCM they are about to hand
//! on, before any volume or normalisation stage. It only reads the bytes, so the
//! bit-perfect path stays bit-exact. While disabled each call is a single atomic
//! load.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Set from `SONE_LEVEL_TAP=1` so the verifier and perf runs can exercise the
/// tap without a UI asking for it; `set_enabled(false)` can't turn it off.
static FORCED: AtomicBool = AtomicBool::new(false);
static RMS: AtomicU32 = AtomicU32::new(0);
static PEAK: AtomicU32 = AtomicU32::new(0);
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Bounds the per-buffer cost: larger buffers are decimated down to about this
/// many frames. It's a visual envelope, not a meter.
const MAX_FRAMES_PER_MEASURE: usize = 1024;

/// Ask for levels (the full-screen player is open) or stop. Off by default.
/// While off the tap costs nothing measurable.
pub fn set_enabled(on: bool) {
    let on = on || FORCED.load(Ordering::Relaxed);
    ENABLED.store(on, Ordering::Relaxed);
    if !on {
        reset();
    }
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// The most recent level: (rms, peak), each 0.0..=1.0 linear full-scale,
/// mono-summed, measured before any volume/normalisation stage.
/// (0.0, 0.0) while disabled, paused, or with no audio.
pub fn level() -> (f32, f32) {
    (
        f32::from_bits(RMS.load(Ordering::Relaxed)),
        f32::from_bits(PEAK.load(Ordering::Relaxed)),
    )
}

/// Monotonic count of level updates, so a caller can tell a new measurement
/// from a stale one.
pub fn seq() -> u64 {
    SEQ.load(Ordering::Relaxed)
}

/// Reads `SONE_LEVEL_TAP` once; called when the audio thread starts.
pub(crate) fn init_from_env() {
    if std::env::var("SONE_LEVEL_TAP").is_ok_and(|v| v == "1") {
        FORCED.store(true, Ordering::Relaxed);
        ENABLED.store(true, Ordering::Relaxed);
        log::info!("[audio-level] tap forced on by SONE_LEVEL_TAP=1");
    }
}

/// Drops the level to silence so visuals settle on pause, stop, flush or EOS.
pub(crate) fn reset() {
    if RMS.load(Ordering::Relaxed) == 0 && PEAK.load(Ordering::Relaxed) == 0 {
        return;
    }
    publish(0.0, 0.0);
}

fn publish(rms: f32, peak: f32) {
    RMS.store(rms.to_bits(), Ordering::Relaxed);
    PEAK.store(peak.to_bits(), Ordering::Relaxed);
    SEQ.fetch_add(1, Ordering::Relaxed);
}

/// Measures interleaved PCM in a GStreamer format (`"S16LE"` etc.) and
/// publishes it, if the tap is enabled. Unknown formats are ignored.
#[inline]
pub(crate) fn tap(data: &[u8], gst_format: &str, channels: u32) {
    if !enabled() {
        return;
    }
    let Some(fmt) = SampleFormat::from_gst(gst_format) else { return };
    if let Some((rms, peak)) = measure(data, fmt, channels as usize) {
        publish(rms, peak);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SampleFormat {
    S16,
    S24,
    S24In32,
    S32,
    F32,
}

impl SampleFormat {
    pub(crate) fn from_gst(s: &str) -> Option<Self> {
        Some(match s {
            "S16LE" => Self::S16,
            "S24LE" => Self::S24,
            "S24_32LE" => Self::S24In32,
            "S32LE" => Self::S32,
            "F32LE" => Self::F32,
            _ => return None,
        })
    }

    fn bytes(self) -> usize {
        match self {
            Self::S16 => 2,
            Self::S24 => 3,
            Self::S24In32 | Self::S32 | Self::F32 => 4,
        }
    }

    #[inline(always)]
    fn sample(self, b: &[u8]) -> f32 {
        match self {
            Self::S16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
            Self::S24 => {
                let v = (i32::from_le_bytes([0, b[0], b[1], b[2]])) >> 8;
                v as f32 / 8_388_608.0
            }
            // The 24 valid bits sit in the low three bytes; sign-extend from bit 23
            // so a stray pad byte can't skew the level.
            Self::S24In32 => {
                let v = (i32::from_le_bytes([b[0], b[1], b[2], b[3]]) << 8) >> 8;
                v as f32 / 8_388_608.0
            }
            Self::S32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            Self::F32 => {
                let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if v.is_finite() { v } else { 0.0 }
            }
        }
    }
}

/// (rms, peak) of the mono sum, each clamped to 0.0..=1.0. `None` for an empty
/// or malformed buffer, so the previous level stands.
pub(crate) fn measure(data: &[u8], fmt: SampleFormat, channels: usize) -> Option<(f32, f32)> {
    if channels == 0 {
        return None;
    }
    let frame_bytes = fmt.bytes() * channels;
    let frames = data.len() / frame_bytes;
    if frames == 0 {
        return None;
    }
    let stride = frames.div_ceil(MAX_FRAMES_PER_MEASURE);
    let inv_ch = 1.0 / channels as f32;
    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f32;
    let mut n = 0u32;
    for frame in data.chunks_exact(frame_bytes).step_by(stride) {
        let mut mono = 0.0f32;
        for s in frame.chunks_exact(fmt.bytes()) {
            mono += fmt.sample(s);
        }
        mono *= inv_ch;
        sum_sq += (mono * mono) as f64;
        peak = peak.max(mono.abs());
        n += 1;
    }
    let rms = (sum_sq / n as f64).sqrt() as f32;
    Some((rms.min(1.0), peak.min(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [SampleFormat; 5] = [
        SampleFormat::S16,
        SampleFormat::S24,
        SampleFormat::S24In32,
        SampleFormat::S32,
        SampleFormat::F32,
    ];

    /// Encodes `x` in -1.0..=1.0 the way a decoder would.
    fn encode(fmt: SampleFormat, x: f64, out: &mut Vec<u8>) {
        match fmt {
            SampleFormat::S16 => {
                out.extend_from_slice(&((x * 32_767.0).round() as i16).to_le_bytes())
            }
            SampleFormat::S24 => {
                out.extend_from_slice(&((x * 8_388_607.0).round() as i32).to_le_bytes()[..3])
            }
            SampleFormat::S24In32 => {
                out.extend_from_slice(&((x * 8_388_607.0).round() as i32).to_le_bytes())
            }
            SampleFormat::S32 => {
                out.extend_from_slice(&((x * 2_147_483_647.0).round() as i32).to_le_bytes())
            }
            SampleFormat::F32 => out.extend_from_slice(&(x as f32).to_le_bytes()),
        }
    }

    fn signal(fmt: SampleFormat, channels: usize, frames: usize, f: impl Fn(usize) -> f64) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..frames {
            for _ in 0..channels {
                encode(fmt, f(i), &mut out);
            }
        }
        out
    }

    #[test]
    fn silence_is_zero() {
        for fmt in ALL {
            let data = signal(fmt, 2, 1024, |_| 0.0);
            assert_eq!(measure(&data, fmt, 2), Some((0.0, 0.0)), "{fmt:?}");
        }
    }

    #[test]
    fn full_scale_square_is_one() {
        for fmt in ALL {
            let data = signal(fmt, 2, 1024, |i| if (i / 32) % 2 == 0 { 1.0 } else { -1.0 });
            let (rms, peak) = measure(&data, fmt, 2).unwrap();
            assert!((rms - 1.0).abs() < 1e-3, "{fmt:?} rms {rms}");
            assert!((peak - 1.0).abs() < 1e-3, "{fmt:?} peak {peak}");
        }
    }

    #[test]
    fn full_scale_sine_rms_is_root_half() {
        for fmt in ALL {
            let data = signal(fmt, 2, 1000, |i| (i as f64 * std::f64::consts::TAU / 100.0).sin());
            let (rms, peak) = measure(&data, fmt, 2).unwrap();
            assert!((rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 2e-3, "{fmt:?} rms {rms}");
            assert!((peak - 1.0).abs() < 2e-3, "{fmt:?} peak {peak}");
        }
    }

    #[test]
    fn negative_full_scale_peaks_at_one() {
        for fmt in [SampleFormat::S16, SampleFormat::S24, SampleFormat::S24In32, SampleFormat::S32] {
            let min = match fmt {
                SampleFormat::S16 => (i16::MIN as i32).to_le_bytes()[..2].to_vec(),
                SampleFormat::S24 => (-8_388_608i32).to_le_bytes()[..3].to_vec(),
                SampleFormat::S24In32 => (-8_388_608i32).to_le_bytes().to_vec(),
                _ => i32::MIN.to_le_bytes().to_vec(),
            };
            let (rms, peak) = measure(&min, fmt, 1).unwrap();
            assert_eq!((rms, peak), (1.0, 1.0), "{fmt:?}");
        }
    }

    #[test]
    fn opposite_channels_cancel_in_the_mono_sum() {
        let mut data = Vec::new();
        for _ in 0..256 {
            encode(SampleFormat::S16, 0.5, &mut data);
            encode(SampleFormat::S16, -0.5, &mut data);
        }
        let (rms, peak) = measure(&data, SampleFormat::S16, 2).unwrap();
        assert!(rms < 1e-4 && peak < 1e-4);
    }

    #[test]
    fn half_scale_and_decimated_large_buffer() {
        let data = signal(SampleFormat::S32, 2, 48_000, |i| if i % 2 == 0 { 0.5 } else { -0.5 });
        let (rms, peak) = measure(&data, SampleFormat::S32, 2).unwrap();
        assert!((rms - 0.5).abs() < 1e-3 && (peak - 0.5).abs() < 1e-3);
    }

    #[test]
    fn non_finite_float_is_ignored() {
        let mut data = Vec::new();
        data.extend_from_slice(&f32::NAN.to_le_bytes());
        data.extend_from_slice(&f32::INFINITY.to_le_bytes());
        assert_eq!(measure(&data, SampleFormat::F32, 1), Some((0.0, 0.0)));
    }

    #[test]
    fn empty_or_partial_buffers_leave_the_level_alone() {
        assert_eq!(measure(&[], SampleFormat::S16, 2), None);
        assert_eq!(measure(&[0, 0, 0], SampleFormat::S16, 2), None);
        assert_eq!(measure(&[0, 0], SampleFormat::S16, 0), None);
    }

    #[test]
    fn unknown_formats_are_not_measured() {
        assert_eq!(SampleFormat::from_gst("U8"), None);
        assert_eq!(SampleFormat::from_gst("S24_32LE"), Some(SampleFormat::S24In32));
    }
}
