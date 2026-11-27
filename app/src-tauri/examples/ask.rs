//! Runs the real model loop (local llama-server clients, prompt, parser)
//! against a mock screen without Tauri. Used for the end-to-end check.
//!
//! ```text
//! python3 examples/mock_screen.py /tmp/mock.png
//! KJ_PNG=/tmp/mock.png KJ_WAV=/tmp/question.wav KJ_DATA_DIR=/tmp/kj-data \
//!   cargo run --example ask
//! ```
//!
//! Without `KJ_WAV`, `KJ_TRANSCRIPT` stands in for speech recognition. Set
//! `KJ_ASR_URL` / `KJ_VLM_URL` to use servers that are already running;
//! otherwise `KJ_DATA_DIR` must hold `llama/` and `models/` (or use the
//! `KJ_LLAMA_SERVER` / `KJ_MODELS_DIR` overrides).

use std::path::PathBuf;
use std::time::Instant;

use kutty_jarvis_lib::assistant::{self, Input};
use kutty_jarvis_lib::models::SpeechToText;
use kutty_jarvis_lib::models::llama_server::LlamaServers;
use kutty_jarvis_lib::uia::{Element, ElementList, Rect, Role};

/// Matches the shapes drawn by `mock_screen.py`.
const MOCK_ELEMENTS: [(&str, Role, i32, i32, i32, i32); 5] = [
    ("Open", Role::Button, 40, 60, 120, 44),
    ("Save", Role::Button, 180, 60, 120, 44),
    ("Print", Role::Button, 320, 60, 120, 44),
    ("Search", Role::Edit, 700, 60, 360, 44),
    ("Settings", Role::Button, 1100, 60, 140, 44),
];

struct FixedTranscript(String);

impl SpeechToText for FixedTranscript {
    fn transcribe(&self, _wav: &[u8]) -> Result<String, String> {
        Ok(self.0.clone())
    }
}

fn mock_list() -> ElementList {
    let elements: Vec<Element> = MOCK_ELEMENTS
        .iter()
        .enumerate()
        .map(|(index, (name, role, x, y, w, h))| Element {
            id: index + 1,
            role: *role,
            name: (*name).to_string(),
            rect: Rect::new(*x, *y, *w, *h),
            automation_id: None,
        })
        .collect();
    ElementList {
        window_title: "Report - Editor".into(),
        process_id: 0,
        window_rect: Rect::new(0, 0, 1280, 720),
        candidates: elements.len(),
        elements,
        visited: 0,
        pruned_offscreen: 0,
        elapsed_ms: 0,
        capped: false,
        partial: false,
        stop_reason: None,
    }
}

fn main() {
    let env = |name: &str| std::env::var(name).ok();
    let data_dir = PathBuf::from(env("KJ_DATA_DIR").unwrap_or_else(|| ".".into()));
    let servers = LlamaServers::new(data_dir);

    let wav = env("KJ_WAV").map(|path| std::fs::read(&path).expect("cannot read KJ_WAV"));
    let screenshot = env("KJ_PNG").map(|path| std::fs::read(&path).expect("cannot read KJ_PNG"));
    let stt: std::sync::Arc<dyn SpeechToText> = match (&wav, env("KJ_TRANSCRIPT")) {
        (Some(_), _) => servers.speech_to_text(),
        (None, Some(text)) => std::sync::Arc::new(FixedTranscript(text)),
        (None, None) => panic!("set KJ_WAV or KJ_TRANSCRIPT"),
    };

    let input = Input {
        wav: wav.unwrap_or_default(),
        screenshot_png: screenshot,
        elements: Some(mock_list()),
    };
    let started = Instant::now();
    let result = assistant::run(stt.as_ref(), servers.assistant().as_ref(), input, |heard| {
        println!("heard:      {heard}");
    });
    let total = started.elapsed();
    servers.stop_all();

    match result {
        Ok(reply) => {
            println!("reply:      {}", reply.text);
            match &reply.target {
                Some(target) => println!(
                    "points at:  {} {} \"{}\" at {:?}",
                    target.id, target.role, target.name, target.rect
                ),
                None => println!("points at:  nothing"),
            }
            println!(
                "timings:    speech-to-text {:.1?}, answer {:.1?}, total {:.1?} (includes any server start)",
                reply.transcribe_time, reply.answer_time, total
            );
        }
        Err(failure) => {
            eprintln!("failed at {:?}: {}", failure.stage, failure.message);
            std::process::exit(1);
        }
    }
}
