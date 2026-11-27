//! Plays a WAV through the default output device with cpal, converting its
//! rate and channel count to what the device wants. Stops promptly when the
//! utterance is cancelled.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use super::Cancel;
use crate::audio::{downmix_to_mono, resample_linear};

/// Decoded PCM, interleaved, as f32 in -1..1.
#[derive(Debug, PartialEq)]
pub struct Decoded {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn decode_wav(wav: &[u8]) -> Result<Decoded, String> {
    let mut reader =
        hound::WavReader::new(Cursor::new(wav)).map_err(|e| format!("bad WAV: {e}"))?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err("bad WAV: empty format".into());
    }
    let samples: Result<Vec<f32>, _> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader.samples::<f32>().collect(),
        (hound::SampleFormat::Int, bits @ 1..=32) => {
            let scale = (1u64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect()
        }
        (format, bits) => return Err(format!("unsupported WAV format: {format:?} {bits}-bit")),
    };
    Ok(Decoded {
        samples: samples.map_err(|e| format!("bad WAV: {e}"))?,
        sample_rate: spec.sample_rate,
        channels: spec.channels,
    })
}

/// Mono-izes, resamples to the device rate and spreads the signal over the
/// device's channels.
pub fn convert_for_device(decoded: &Decoded, out_rate: u32, out_channels: u16) -> Vec<f32> {
    let mono = downmix_to_mono(&decoded.samples, decoded.channels);
    let mono = resample_linear(&mono, decoded.sample_rate, out_rate);
    let channels = usize::from(out_channels.max(1));
    let mut out = Vec::with_capacity(mono.len() * channels);
    for sample in mono {
        out.extend(std::iter::repeat_n(sample, channels));
    }
    out
}

/// Why playback ended without finishing. `Setup` means no audio was ever
/// started, so another engine may still speak the reply.
#[derive(Debug, PartialEq)]
pub enum PlayError {
    Setup(String),
    Playing(String),
}

impl std::fmt::Display for PlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayError::Setup(e) | PlayError::Playing(e) => f.write_str(e),
        }
    }
}

/// Blocks until the WAV has played or `cancel` is raised. `on_start` runs
/// once the device is really playing, never when setup fails.
pub fn play_wav(wav: &[u8], cancel: &Cancel, on_start: impl FnOnce()) -> Result<(), PlayError> {
    let decoded = decode_wav(wav).map_err(PlayError::Setup)?;
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| PlayError::Setup("no audio output device".into()))?;
    let config = device
        .default_output_config()
        .map_err(|e| PlayError::Setup(format!("no output configuration: {e}")))?;
    let rate = config.sample_rate().0;
    let samples = Arc::new(convert_for_device(&decoded, rate, config.channels()));
    let position = Arc::new(AtomicUsize::new(0));

    let stream_config = config.config();
    let stream = build_for_format(
        config.sample_format(),
        &device,
        &stream_config,
        &samples,
        &position,
    )
    .map_err(PlayError::Setup)?;
    stream
        .play()
        .map_err(|e| PlayError::Setup(format!("could not start playback: {e}")))?;
    on_start();

    let audio_length = Duration::from_secs_f64(
        samples.len() as f64 / f64::from(config.channels()) / f64::from(rate),
    );
    let deadline = Instant::now() + audio_length + Duration::from_secs(5);
    while position.load(Ordering::Relaxed) < samples.len() {
        if cancel.is_cancelled() {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(PlayError::Playing("audio output stalled".into()));
        }
        sleep(Duration::from_millis(20));
    }
    // Let the device buffer drain before the stream is dropped.
    let drain_until = Instant::now() + Duration::from_millis(250);
    while Instant::now() < drain_until && !cancel.is_cancelled() {
        sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn build_for_format(
    format: SampleFormat,
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: &Arc<Vec<f32>>,
    position: &Arc<AtomicUsize>,
) -> Result<cpal::Stream, String> {
    match format {
        SampleFormat::F32 => build::<f32>(device, config, samples, position),
        SampleFormat::F64 => build::<f64>(device, config, samples, position),
        SampleFormat::I8 => build::<i8>(device, config, samples, position),
        SampleFormat::I16 => build::<i16>(device, config, samples, position),
        SampleFormat::I32 => build::<i32>(device, config, samples, position),
        SampleFormat::I64 => build::<i64>(device, config, samples, position),
        SampleFormat::U8 => build::<u8>(device, config, samples, position),
        SampleFormat::U16 => build::<u16>(device, config, samples, position),
        SampleFormat::U32 => build::<u32>(device, config, samples, position),
        SampleFormat::U64 => build::<u64>(device, config, samples, position),
        other => Err(format!("unsupported output format: {other:?}")),
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: &Arc<Vec<f32>>,
    position: &Arc<AtomicUsize>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let samples = Arc::clone(samples);
    let position = Arc::clone(position);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let start = position.load(Ordering::Relaxed);
                for (i, slot) in data.iter_mut().enumerate() {
                    *slot = samples
                        .get(start + i)
                        .map_or(T::EQUILIBRIUM, |s| T::from_sample(*s));
                }
                position.store(start + data.len(), Ordering::Relaxed);
            },
            |error| eprintln!("audio output error: {error}"),
            None,
        )
        .map_err(|e| format!("could not open the output device: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
        for s in samples {
            writer.write_sample(*s).unwrap();
        }
        writer.finalize().unwrap();
        cursor.into_inner()
    }

    #[test]
    fn decodes_16_bit_pcm_to_floats() {
        let decoded = decode_wav(&wav(1, 22050, &[0, 16384, -32768])).unwrap();
        assert_eq!(decoded.sample_rate, 22050);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.samples, [0.0, 0.5, -1.0]);
    }

    #[test]
    fn rejects_bytes_that_are_not_a_wav() {
        assert!(decode_wav(b"not a wav").is_err());
    }

    #[test]
    fn mono_is_spread_over_every_device_channel() {
        let decoded = Decoded {
            samples: vec![0.25, -0.25],
            sample_rate: 48_000,
            channels: 1,
        };
        assert_eq!(
            convert_for_device(&decoded, 48_000, 2),
            [0.25, 0.25, -0.25, -0.25]
        );
    }

    #[test]
    fn stereo_is_averaged_before_spreading() {
        let decoded = Decoded {
            samples: vec![1.0, 0.0],
            sample_rate: 48_000,
            channels: 2,
        };
        assert_eq!(convert_for_device(&decoded, 48_000, 2), [0.5, 0.5]);
    }

    #[test]
    fn resampling_scales_the_length_by_the_rate_ratio() {
        let decoded = Decoded {
            samples: vec![0.1; 22_050],
            sample_rate: 22_050,
            channels: 1,
        };
        let out = convert_for_device(&decoded, 44_100, 2);
        let frames = out.len() / 2;
        assert!((frames as i64 - 44_100).abs() <= 2, "{frames}");
    }
}
