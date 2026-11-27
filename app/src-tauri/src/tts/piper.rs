//! Piper as a separate process: text on stdin, raw 16-bit mono PCM on stdout,
//! wrapped into an in-memory WAV. Nothing is written to disk.

use std::io::{Cursor, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::assets::PiperPaths;
use super::{Cancel, TextToSpeech};

/// Raw PCM above this is refused (about 18 minutes of 22 kHz audio).
const MAX_AUDIO_BYTES: usize = 50 * 1024 * 1024;

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

/// Generous for a CPU voice: model start-up plus time proportional to the
/// text, capped so a hung process is always killed.
fn synthesis_timeout(text_len: usize) -> Duration {
    let millis = 15_000 + 100 * text_len as u64;
    Duration::from_millis(millis.min(120_000))
}

/// Kills and reaps the child however the scope is left.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Reads to the end, keeping at most `limit` bytes; `overflow` is raised the
/// moment the stream has more than that.
fn drain(mut stream: impl Read, limit: usize, overflow: &AtomicBool) -> Vec<u8> {
    let mut data = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return data,
            Ok(n) => {
                if data.len() + n > limit {
                    overflow.store(true, Ordering::SeqCst);
                    return data;
                }
                data.extend_from_slice(&buffer[..n]);
            }
        }
    }
}

/// Runs `command` with `text` on stdin and returns its stdout. All three
/// pipes are serviced on their own threads so none can fill up and stall the
/// child. The child is killed and reaped when `cancel` is raised, when
/// `timeout` passes, when stdout exceeds `max_output` and on any early
/// return.
fn run_child(
    mut command: Command,
    text: &str,
    cancel: &Cancel,
    timeout: Duration,
    max_output: usize,
) -> Result<Vec<u8>, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = ChildGuard(
        command
            .spawn()
            .map_err(|e| format!("could not start Piper: {e}"))?,
    );
    let mut stdin = child.0.stdin.take().ok_or("no Piper stdin")?;
    let stdout = child.0.stdout.take().ok_or("no Piper stdout")?;
    let stderr = child.0.stderr.take().ok_or("no Piper stderr")?;

    let overflow = Arc::new(AtomicBool::new(false));
    let line = format!("{text}\n");
    let writer = std::thread::spawn(move || stdin.write_all(line.as_bytes()));
    let audio = {
        let overflow = Arc::clone(&overflow);
        std::thread::spawn(move || drain(stdout, max_output, &overflow))
    };
    let errors = std::thread::spawn(move || drain(stderr, 64 * 1024, &AtomicBool::new(false)));

    let started = Instant::now();
    let status = loop {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        if overflow.load(Ordering::SeqCst) {
            return Err("Piper produced too much audio".into());
        }
        if started.elapsed() > timeout {
            return Err(format!("Piper timed out after {timeout:?}"));
        }
        match child.0.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(format!("Piper failed: {e}")),
        }
    };
    let _ = writer.join();
    let stdout = audio.join().map_err(|_| "Piper output thread failed")?;
    let stderr = errors.join().unwrap_or_default();
    if overflow.load(Ordering::SeqCst) {
        return Err("Piper produced too much audio".into());
    }
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        let tail: String = stderr
            .chars()
            .rev()
            .take(300)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return Err(format!("Piper exited with {status}: {}", tail.trim()));
    }
    if stdout.is_empty() {
        return Err("Piper produced no audio".into());
    }
    Ok(stdout)
}

impl TextToSpeech for PiperEngine {
    fn name(&self) -> &'static str {
        "piper"
    }

    fn synthesize(&self, text: &str, cancel: &Cancel) -> Result<Vec<u8>, String> {
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
            );
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let pcm = run_child(
            command,
            &text,
            cancel,
            synthesis_timeout(text.len()),
            MAX_AUDIO_BYTES,
        )?;
        pcm16_to_wav(&pcm, rate)
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

    #[test]
    fn the_timeout_grows_with_the_text_and_is_capped() {
        assert_eq!(synthesis_timeout(0), Duration::from_secs(15));
        assert_eq!(synthesis_timeout(100), Duration::from_secs(25));
        assert_eq!(synthesis_timeout(1_000_000), Duration::from_secs(120));
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
            .synthesize(
                "Hello! I can see your screen. The save button is in the top left.",
                &Cancel::new(0),
            )
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

    /// Stand-in children for the process handling. Linux only: they use
    /// `sh`, `cat` and `/proc` to see whether a process is really gone.
    #[cfg(target_os = "linux")]
    mod process {
        use super::*;
        use std::path::{Path, PathBuf};
        use std::thread;

        fn tmp(name: &str) -> PathBuf {
            std::env::temp_dir().join(format!("kj-piper-{}-{name}", std::process::id()))
        }

        /// A child that records its pid, then hangs without reading stdin.
        fn stalled(pid_file: &Path) -> Command {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg(format!("echo $$ > '{}'; exec sleep 60", pid_file.display()));
            command
        }

        fn pid_of(pid_file: &Path) -> u32 {
            for _ in 0..200 {
                if let Ok(text) = std::fs::read_to_string(pid_file)
                    && let Ok(pid) = text.trim().parse()
                {
                    return pid;
                }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("child never wrote its pid");
        }

        /// A zombie still has a /proc entry, so this also proves it was reaped.
        fn gone(pid: u32) -> bool {
            !Path::new(&format!("/proc/{pid}")).exists()
        }

        #[test]
        fn cancelling_kills_and_reaps_a_stalled_child_promptly() {
            let pid_file = tmp("cancel");
            let cancel = Arc::new(Cancel::new(1));
            let worker = {
                let cancel = Arc::clone(&cancel);
                let command = stalled(&pid_file);
                thread::spawn(move || {
                    run_child(command, "hi", &cancel, Duration::from_secs(60), 1024)
                })
            };
            let pid = pid_of(&pid_file);
            let raised = Instant::now();
            cancel.cancel();
            let result = worker.join().unwrap();
            assert_eq!(result, Err("cancelled".to_string()));
            assert!(raised.elapsed() < Duration::from_secs(2));
            assert!(gone(pid));
            let _ = std::fs::remove_file(&pid_file);
        }

        #[test]
        fn rapid_replacement_does_not_accumulate_processes() {
            let mut workers = Vec::new();
            let mut pid_files = Vec::new();
            for i in 0..12 {
                let pid_file = tmp(&format!("rapid-{i}"));
                let cancel = Arc::new(Cancel::new(i));
                let command = stalled(&pid_file);
                let worker_cancel = Arc::clone(&cancel);
                workers.push((
                    cancel,
                    thread::spawn(move || {
                        run_child(command, "hi", &worker_cancel, Duration::from_secs(60), 1024)
                    }),
                ));
                pid_files.push(pid_file);
            }
            let pids: Vec<u32> = pid_files.iter().map(|f| pid_of(f)).collect();
            for (cancel, _) in &workers {
                cancel.cancel();
            }
            for (_, worker) in workers {
                assert!(worker.join().unwrap().is_err());
            }
            assert!(pids.iter().all(|pid| gone(*pid)));
            for file in pid_files {
                let _ = std::fs::remove_file(file);
            }
        }

        #[test]
        fn a_hung_child_is_killed_at_the_timeout() {
            let pid_file = tmp("timeout");
            let result = run_child(
                stalled(&pid_file),
                "hi",
                &Cancel::new(1),
                Duration::from_millis(300),
                1024,
            );
            assert!(result.unwrap_err().contains("timed out"));
            assert!(gone(pid_of(&pid_file)));
            let _ = std::fs::remove_file(&pid_file);
        }

        #[test]
        fn runaway_output_is_refused_and_the_child_killed() {
            let mut command = Command::new("sh");
            command.arg("-c").arg("exec yes");
            let started = Instant::now();
            let result = run_child(
                command,
                "hi",
                &Cancel::new(1),
                Duration::from_secs(30),
                1024 * 1024,
            );
            assert_eq!(result, Err("Piper produced too much audio".to_string()));
            assert!(started.elapsed() < Duration::from_secs(10));
        }

        #[test]
        fn a_large_text_does_not_deadlock_the_pipes() {
            // `cat` echoes stdin to stdout, so both pipes carry megabytes.
            let text = "a".repeat(4 * 1024 * 1024);
            let result = run_child(
                Command::new("cat"),
                &text,
                &Cancel::new(1),
                Duration::from_secs(30),
                MAX_AUDIO_BYTES,
            );
            assert_eq!(result.unwrap().len(), text.len() + 1);
        }

        #[test]
        fn a_failing_child_reports_its_stderr() {
            let mut command = Command::new("sh");
            command.arg("-c").arg("echo boom >&2; exit 3");
            let error = run_child(command, "hi", &Cancel::new(1), Duration::from_secs(5), 1024)
                .unwrap_err();
            assert!(error.contains("boom"), "{error}");
        }
    }
}
