//! Microphone recording for push-to-talk (ADR 0010).
//!
//! The stream is open only between a key press and release. Captured samples
//! are converted to mono 16 kHz 16-bit WAV in memory, which is the format the
//! speech model was tested with (finding 0002). Nothing is written to disk.

use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// The most recent recording, held in memory only.
#[derive(Default)]
pub struct LatestRecording(pub Mutex<Option<Recording>>);

/// The recording in progress, if any.
#[derive(Default)]
pub struct ActiveRecorder(pub Mutex<Option<Recorder>>);

#[derive(Debug, Clone)]
pub struct Recording {
    pub duration_ms: u64,
    pub wav: Vec<u8>,
    /// Time from key press to key release.
    pub held_ms: u64,
    /// Time from key press until the stream reported it was running.
    pub ready_ms: u64,
    /// Time from key press until the first audio sample arrived, if any did.
    pub first_sample_ms: Option<u64>,
}

struct RawAudio {
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
    ready_at: Instant,
    first_sample_at: Option<Instant>,
}

/// Owns the microphone thread. Dropping the stream stops capture.
pub struct Recorder {
    stop: Sender<()>,
    handle: JoinHandle<Result<RawAudio, String>>,
    started_at: Instant,
}

impl Recorder {
    /// Opens the default input device and starts capturing. Returns once the
    /// stream is running, or with the reason it could not start.
    pub fn start() -> Result<Self, String> {
        let started_at = Instant::now();
        let (stop, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let handle = thread::spawn(move || capture_until_stopped(stop_rx, ready_tx));
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop,
                handle,
                started_at,
            }),
            Ok(Err(message)) => {
                let _ = handle.join();
                Err(message)
            }
            Err(_) => Err("microphone thread stopped before it started".to_string()),
        }
    }

    /// Stops capturing and returns the recording as 16 kHz mono WAV.
    pub fn finish(self) -> Result<Recording, String> {
        let released_at = Instant::now();
        let _ = self.stop.send(());
        let raw = self
            .handle
            .join()
            .map_err(|_| "microphone thread panicked".to_string())??;
        let mono = downmix_to_mono(&raw.samples, raw.channels);
        let resampled = resample_linear(&mono, raw.sample_rate, TARGET_SAMPLE_RATE);
        let duration_ms = (resampled.len() as u64 * 1000) / u64::from(TARGET_SAMPLE_RATE);
        let wav = encode_wav_16k_mono(&to_pcm16(&resampled))?;
        let since_start = |at: Instant| at.duration_since(self.started_at).as_millis() as u64;
        Ok(Recording {
            duration_ms,
            wav,
            held_ms: since_start(released_at),
            ready_ms: since_start(raw.ready_at),
            first_sample_ms: raw.first_sample_at.map(since_start),
        })
    }
}

fn capture_until_stopped(
    stop_rx: Receiver<()>,
    ready_tx: Sender<Result<(), String>>,
) -> Result<RawAudio, String> {
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(device) => device,
        None => {
            let message = "no microphone found".to_string();
            let _ = ready_tx.send(Err(message.clone()));
            return Err(message);
        }
    };
    let supported = match device.default_input_config() {
        Ok(config) => config,
        Err(error) => {
            let message = format!("could not read microphone settings: {error}");
            let _ = ready_tx.send(Err(message.clone()));
            return Err(message);
        }
    };

    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let first_sample_at = Arc::new(Mutex::new(None::<Instant>));
    let format = supported.sample_format();
    let config = supported.config();
    let stream = match format {
        SampleFormat::F32 => {
            build_stream::<f32>(&device, &config, samples.clone(), first_sample_at.clone())
        }
        SampleFormat::I16 => {
            build_stream::<i16>(&device, &config, samples.clone(), first_sample_at.clone())
        }
        SampleFormat::U16 => {
            build_stream::<u16>(&device, &config, samples.clone(), first_sample_at.clone())
        }
        other => Err(format!("unsupported microphone sample format: {other:?}")),
    };
    let stream = match stream.and_then(|stream| {
        stream
            .play()
            .map(|_| stream)
            .map_err(|error| format!("could not start microphone: {error}"))
    }) {
        Ok(stream) => stream,
        Err(message) => {
            let _ = ready_tx.send(Err(message.clone()));
            return Err(message);
        }
    };
    let ready_at = Instant::now();
    let _ = ready_tx.send(Ok(()));

    // Block until the hotkey is released, then drop the stream to close the microphone.
    let _ = stop_rx.recv();
    drop(stream);

    let samples = samples
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let first_sample_at = first_sample_at.lock().ok().and_then(|guard| *guard);
    Ok(RawAudio {
        samples,
        channels: config.channels,
        sample_rate: config.sample_rate.0,
        ready_at,
        first_sample_at,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    first_sample_at: Arc<Mutex<Option<Instant>>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if let Ok(mut first) = first_sample_at.lock()
                    && first.is_none()
                    && !data.is_empty()
                {
                    *first = Some(Instant::now());
                }
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend(data.iter().map(|sample| sample.to_sample::<f32>()));
                }
            },
            |error| eprintln!("microphone stream error: {error}"),
            None,
        )
        .map_err(|error| format!("could not open microphone: {error}"))
}

/// Looks up the default microphone and its settings without opening a stream,
/// so the audio subsystem is initialised before the first key press. The
/// microphone itself is not opened (ADR 0010).
pub fn warm_up() {
    let host = cpal::default_host();
    if let Some(device) = host.default_input_device() {
        let _ = device.default_input_config();
    }
}

/// Averages interleaved channels into one channel.
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = usize::from(channels.max(1));
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Linear-interpolation resampling. Adequate for speech, not for music.
pub fn resample_linear(samples: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if samples.is_empty() || from_hz == 0 || from_hz == to_hz {
        return samples.to_vec();
    }
    let ratio = f64::from(from_hz) / f64::from(to_hz);
    let out_len = ((samples.len() as f64) / ratio).floor() as usize;
    (0..out_len)
        .map(|index| {
            let position = index as f64 * ratio;
            let left = position.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let fraction = (position - left as f64) as f32;
            samples[left] * (1.0 - fraction) + samples[right] * fraction
        })
        .collect()
}

pub fn to_pcm16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16)
        .collect()
}

pub fn encode_wav_16k_mono(pcm: &[i16]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|error| format!("could not start WAV: {error}"))?;
        for sample in pcm {
            writer
                .write_sample(*sample)
                .map_err(|error| format!("could not write WAV: {error}"))?;
        }
        writer
            .finalize()
            .map_err(|error| format!("could not finish WAV: {error}"))?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_stereo_frames() {
        assert_eq!(downmix_to_mono(&[1.0, 0.0, -1.0, 1.0], 2), vec![0.5, 0.0]);
    }

    #[test]
    fn downmix_passes_mono_through() {
        assert_eq!(downmix_to_mono(&[0.25, -0.5], 1), vec![0.25, -0.5]);
    }

    #[test]
    fn resample_48k_to_16k_keeps_one_third_of_the_samples() {
        let input = vec![0.0_f32; 48_000];
        assert_eq!(resample_linear(&input, 48_000, 16_000).len(), 16_000);
    }

    #[test]
    fn resample_is_a_no_op_at_the_target_rate() {
        let input = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_linear(&input, 16_000, 16_000), input);
    }

    #[test]
    fn pcm_conversion_clamps_out_of_range_values() {
        assert_eq!(to_pcm16(&[2.0, -2.0, 0.0]), vec![i16::MAX, -i16::MAX, 0]);
    }

    #[test]
    fn wav_has_the_speech_model_format_and_samples() {
        let pcm = vec![0_i16, 1000, -1000];
        let bytes = encode_wav_16k_mono(&pcm).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        let reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        let spec = reader.spec();
        assert_eq!(
            (spec.channels, spec.sample_rate, spec.bits_per_sample),
            (1, 16_000, 16)
        );
        let decoded: Vec<i16> = reader.into_samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(decoded, pcm);
    }
}
