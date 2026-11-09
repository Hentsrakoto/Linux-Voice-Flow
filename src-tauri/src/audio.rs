#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn downmix_clips_and_rejects_nonfinite() {
        let b = normalize(&[2.0, 2.0, -2.0, -2.0], 2, 16000).unwrap();
        assert_eq!(b.samples, vec![32767, -32768]);
        assert!(normalize(&[f32::NAN], 1, 16000).is_err());
    }
    #[test]
    fn short_silence_preserves_duration() {
        for rate in [16000, 44100, 48000] {
            let b = normalize(&vec![0.0; rate as usize / 10], 1, rate).unwrap();
            assert_eq!(b.samples.len(), 1600);
            assert!(b.samples.iter().all(|s| *s == 0));
        }
    }
    #[test]
    fn rejects_empty_and_over_limit() {
        assert!(normalize(&[], 1, 16000).is_err());
        assert!(normalize(&vec![0.0; 960001], 1, 16000).is_err());
    }
    #[test]
    fn resampling_preserves_tone_and_filters_aliases() {
        for rate in [44100, 48000] {
            let tone = |hz: f32| {
                (0..rate)
                    .map(|i| 0.5 * (std::f32::consts::TAU * hz * i as f32 / rate as f32).sin())
                    .collect::<Vec<_>>()
            };
            let rms = |b: &AudioBuffer| {
                (b.samples[200..15800]
                    .iter()
                    .map(|s| (*s as f64 / 32768.0).powi(2))
                    .sum::<f64>()
                    / 15600.0)
                    .sqrt()
            };
            let pass = normalize(&tone(1000.0), 1, rate).unwrap();
            let stop = normalize(&tone(12000.0), 1, rate).unwrap();
            assert_eq!(pass.samples.len(), 16000);
            assert!((rms(&pass) - 0.35355).abs() < 0.005);
            assert!(rms(&stop) < 0.003, "alias rms {} at {}", rms(&stop), rate);
            let crossings = pass.samples[200..15800]
                .windows(2)
                .filter(|w| w[0] <= 0 && w[1] > 0)
                .count();
            assert!((crossings as i32 - 975).abs() <= 2);
        }
    }
    #[test]
    fn capture_overflow_and_disconnect_invalidate_entire_clip() {
        let mut capture = CaptureBuffer::new(8000, 2).unwrap();
        capture.push(&[0.25, 0.75], |s| s);
        let clip = capture.finish(8000).unwrap();
        assert_eq!(clip.samples.len(), 2);
        capture.failed = true;
        assert!(capture.finish(8000).is_err());
        let mut capture = CaptureBuffer::new(8000, 1).unwrap();
        capture.push(&vec![0.0; 480001], |s| s);
        assert!(capture.finish(8000).is_err());
        assert!(capture.samples.is_empty());
    }
}

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::{error::Error, fmt};
use zeroize::{Zeroize, Zeroizing};
pub const OUTPUT_RATE: u32 = 16_000;
pub const MAX_SECONDS: usize = 60;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioError {
    pub code: &'static str,
    pub message: String,
}
impl AudioError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl Error for AudioError {}
#[derive(Debug)]
pub struct AudioBuffer {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
}
impl Drop for AudioBuffer {
    fn drop(&mut self) {
        self.samples.zeroize();
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Microphone {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Normalize outside the realtime callback. Sinc filtering prevents aliases when downsampling.
pub fn normalize(input: &[f32], channels: usize, rate: u32) -> Result<AudioBuffer, AudioError> {
    if channels == 0
        || channels > 32
        || !(8000..=192000).contains(&rate)
        || input.is_empty()
        || !input.len().is_multiple_of(channels)
    {
        return Err(AudioError::new(
            "audio_format",
            "Invalid audio format or empty recording",
        ));
    }
    let frames = input.len() / channels;
    if frames > rate as usize * MAX_SECONDS {
        return Err(AudioError::new(
            "audio_limit",
            "Recording exceeds sixty seconds",
        ));
    }
    let mut mono = Zeroizing::new(Vec::with_capacity(frames));
    for frame in input.chunks_exact(channels) {
        if frame.iter().any(|s| !s.is_finite()) {
            mono.zeroize();
            return Err(AudioError::new("audio_format", "Nonfinite audio sample"));
        }
        mono.push(
            (frame.iter().map(|s| *s as f64).sum::<f64>() / channels as f64).clamp(-1.0, 1.0)
                as f32,
        );
    }
    let target_len =
        ((frames as u64 * OUTPUT_RATE as u64 + rate as u64 / 2) / rate as u64).max(1) as usize;
    let mut output = if rate == OUTPUT_RATE {
        mono.to_vec()
    } else {
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let mut resampler =
            SincFixedIn::<f32>::new(OUTPUT_RATE as f64 / rate as f64, 1.0, params, 1024, 1)
                .map_err(|_| AudioError::new("audio_conversion", "Cannot initialize resampler"))?;
        let delay = resampler.output_delay();
        let mut converted = Zeroizing::new(Vec::with_capacity(target_len + delay + 1024));
        // Zero padding flushes filter latency; trim latency and preserve the exact clip duration.
        let mut block = Zeroizing::new(vec![0.0; 1024]);
        let mut offset = 0;
        while converted.len() < target_len + delay {
            block.fill(0.0);
            let end = (offset + 1024).min(mono.len());
            if offset < mono.len() {
                block[..end - offset].copy_from_slice(&mono[offset..end]);
            }
            let mut result = resampler
                .process(&[block.as_slice()], None)
                .map_err(|_| AudioError::new("audio_conversion", "Resampling failed"))?;
            converted.extend_from_slice(&result[0]);
            result[0].zeroize();
            offset += 1024;
        }
        resampler.reset();
        block.zeroize();
        let kept = converted[delay..delay + target_len].to_vec();
        converted.zeroize();
        kept
    };
    mono.zeroize();
    let samples = output
        .iter()
        .map(|s| {
            let s = s.clamp(-1.0, 1.0);
            if s < 0.0 {
                (s * 32768.0).round() as i16
            } else {
                (s * 32767.0).round() as i16
            }
        })
        .collect();
    output.zeroize();
    Ok(AudioBuffer {
        samples,
        sample_rate: OUTPUT_RATE,
    })
}

/// Bounded mono capture state. Any missing or invalid input invalidates the whole session.
#[cfg(any(feature = "native-audio", test))]
pub(crate) struct CaptureBuffer {
    samples: Vec<f32>,
    capacity: usize,
    channels: usize,
    pub(crate) failed: bool,
}
#[cfg(any(feature = "native-audio", test))]
impl CaptureBuffer {
    pub(crate) fn new(rate: u32, channels: usize) -> Result<Self, AudioError> {
        if !(8000..=192000).contains(&rate) || channels == 0 || channels > 32 {
            return Err(AudioError::new(
                "audio_format",
                "Unsupported microphone configuration",
            ));
        }
        let capacity = rate as usize * MAX_SECONDS;
        let mut samples = Vec::new();
        samples.try_reserve_exact(capacity).map_err(|_| {
            AudioError::new("audio_memory", "Cannot allocate bounded capture buffer")
        })?;
        Ok(Self {
            samples,
            capacity,
            channels,
            failed: false,
        })
    }
    pub(crate) fn push<T: Copy>(&mut self, input: &[T], convert: impl Fn(T) -> f32) {
        if self.failed {
            return;
        }
        if !input.len().is_multiple_of(self.channels)
            || input.len() / self.channels > self.capacity - self.samples.len()
        {
            self.failed = true;
            return;
        }
        for frame in input.chunks_exact(self.channels) {
            let mut sum = 0.0f64;
            for sample in frame {
                let value = convert(*sample);
                if !value.is_finite() {
                    self.failed = true;
                    return;
                }
                sum += value as f64;
            }
            self.samples
                .push((sum / self.channels as f64).clamp(-1.0, 1.0) as f32);
        }
    }
    pub(crate) fn finish(&mut self, rate: u32) -> Result<AudioBuffer, AudioError> {
        let result = if self.failed {
            Err(AudioError::new(
                "audio_capture",
                "Incomplete microphone recording",
            ))
        } else {
            normalize(&self.samples, 1, rate)
        };
        self.samples.zeroize();
        self.samples.clear();
        result
    }
}
#[cfg(any(feature = "native-audio", test))]
impl Drop for CaptureBuffer {
    fn drop(&mut self) {
        self.samples.zeroize();
    }
}
#[cfg(feature = "native-audio")]
#[path = "audio_native.rs"]
mod native;
#[cfg(feature = "native-audio")]
pub use native::NativeAudioRecorder;
