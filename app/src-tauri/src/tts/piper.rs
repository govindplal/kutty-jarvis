//! Piper as a separate process: text on stdin, raw 16-bit mono PCM on stdout,
//! wrapped into an in-memory WAV. Nothing is written to disk.

use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

use super::TextToSpeech;
use super::assets::PiperPaths;

pub struct PiperEngine {
    paths: PiperPaths,
}

impl PiperEngine {
    pub fn new(paths: PiperPaths) -> Self {
        Self { paths }
    }

    fn sample_rate(&self) -> Result<u32, String> {
        let json = std::fs::read_to_string(&self.paths.config)
            .map_err(|e| format!("could not read voice config: {e}"))?;
        parse_sample_rate(&json)
    }
}

pub fn parse_sample_rate(json: &str) -> Result<u32, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("bad voice config: {e}"))?;
    value["audio"]["sample_rate"]
        .as_u64()
        .and_then(|rate| u32::try_from(rate).ok())
        .filter(|rate| *rate > 0)
        .ok_or_else(|| "voice config has no audio.sample_rate".to_string())
}

/// Piper reads one utterance per line.
pub fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn pcm16_to_wav(raw: &[u8], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for pair in raw.as_chunks::<2>().0 {
            writer
                .write_sample(i16::from_le_bytes([pair[0], pair[1]]))
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

impl TextToSpeech for PiperEngine {
    fn name(&self) -> &'static str {
        "piper"
    }

    fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let text = clean_text(text);
        if text.is_empty() {
            return Err("nothing to say".into());
        }
        let rate = self.sample_rate()?;
        let mut command = Command::new(&self.paths.executable);
        command
            .arg("--model")
            .arg(&self.paths.model)
            .arg("--config")
            .arg(&self.paths.config)
            .arg("--output_raw")
            .current_dir(
                self.paths
                    .executable
                    .parent()
                    .ok_or("bad Piper executable path")?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("could not start Piper: {e}"))?;
        let mut stdin = child.stdin.take().ok_or("no Piper stdin")?;
        // Written on its own thread so a long text cannot block on a full
        // stdout pipe.
        let writer = std::thread::spawn(move || writeln!(stdin, "{text}"));
        let output = child
            .wait_with_output()
            .map_err(|e| format!("Piper failed: {e}"))?;
        let _ = writer.join();
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: String = stderr
                .chars()
                .rev()
                .take(300)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return Err(format!(
                "Piper exited with {}: {}",
                output.status,
                tail.trim()
            ));
        }
        if output.stdout.is_empty() {
            return Err("Piper produced no audio".into());
        }
        pcm16_to_wav(&output.stdout, rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_sample_rate_from_the_voice_config() {
        assert_eq!(
            parse_sample_rate(r#"{"audio":{"sample_rate":22050}}"#),
            Ok(22050)
        );
        assert!(parse_sample_rate(r#"{"audio":{}}"#).is_err());
        assert!(parse_sample_rate("nope").is_err());
    }

    #[test]
    fn text_is_flattened_to_one_line() {
        assert_eq!(clean_text("  Hello,\n  world \t!\r\n"), "Hello, world !");
    }

    /// Downloads Piper and the voice (~90 MB) and synthesises a sentence.
    /// Run: `cargo test real_piper -- --ignored --nocapture`.
    #[test]
    #[ignore = "downloads about 90 MB"]
    fn real_piper_download_and_synthesis() {
        let dir = std::env::temp_dir().join(format!("kj-piper-real-{}", std::process::id()));
        let started = std::time::Instant::now();
        let paths = crate::tts::assets::install(&dir).expect("install");
        let download = started.elapsed();
        let engine = PiperEngine::new(paths);
        let started = std::time::Instant::now();
        let wav = engine
            .synthesize("Hello! I can see your screen. The save button is in the top left.")
            .expect("synthesize");
        let synth = started.elapsed();
        let decoded = crate::tts::playback::decode_wav(&wav).expect("valid wav");
        let seconds = decoded.samples.len() as f32 / decoded.sample_rate as f32;
        println!(
            "download+verify {download:?}; synth {synth:?}; {} Hz, {} ch, {seconds:.2} s audio, {} bytes",
            decoded.sample_rate,
            decoded.channels,
            wav.len()
        );
        assert!(seconds > 2.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raw_pcm_becomes_a_mono_wav() {
        let raw: Vec<u8> = [0i16, 1000, -1000, 32767]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let wav = pcm16_to_wav(&raw, 22050).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, 22050);
        assert_eq!(reader.spec().channels, 1);
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        assert_eq!(samples, [0, 1000, -1000, 32767]);
    }
}
