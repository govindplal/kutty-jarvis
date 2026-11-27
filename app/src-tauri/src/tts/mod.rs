//! Spoken replies. The model loop calls [`speak`] with the reply text and
//! [`stop`] when a new hotkey press should cut the speech off.
//!
//! Piper (a separate process, downloaded on first use) is the default engine.
//! Until it is installed, Windows SAPI speaks instead. See ADR 0014.
//!
//! Events, all with an `engine` field (`"piper"` or `"sapi"`):
//! - `speech-started`: audio is about to play; `fallback` says why SAPI was
//!   used instead of Piper.
//! - `speech-finished`: `interrupted` is true when [`stop`] or a newer
//!   [`speak`] cut it short.
//! - `speech-failed`: `error` explains why no audio played.
//! - `speech-download`: `status` is `started`, `ready` or `failed`.

pub mod assets;
mod piper;
mod playback;
mod sapi;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use assets::PiperPaths;
use piper::PiperEngine;

/// An engine that turns text into a WAV; playback is shared. Implement this
/// for a cloud provider to make it interchangeable with Piper.
pub trait TextToSpeech: Send + Sync {
    fn name(&self) -> &'static str;
    fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
}

/// Raised to interrupt one utterance.
#[derive(Default)]
pub struct Cancel(AtomicBool);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One utterance at a time: a newer one cancels the current one and waits
/// for it to let go of the output device.
#[derive(Default)]
pub struct Controller {
    current: Mutex<Option<Arc<Cancel>>>,
    output: Mutex<()>,
}

impl Controller {
    /// Registers a new utterance, cancelling the previous one.
    pub fn begin(&self) -> Arc<Cancel> {
        let next = Arc::new(Cancel::default());
        if let Some(previous) = lock(&self.current).replace(Arc::clone(&next)) {
            previous.cancel();
        }
        next
    }

    pub fn stop(&self) {
        if let Some(current) = lock(&self.current).take() {
            current.cancel();
        }
    }

    /// Runs `work` with exclusive use of the output, unless the utterance was
    /// cancelled while waiting its turn.
    pub fn run<R>(&self, cancel: &Cancel, work: impl FnOnce() -> R) -> Option<R> {
        let _output = lock(&self.output);
        if cancel.is_cancelled() {
            return None;
        }
        Some(work())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PiperState {
    NotChecked,
    Downloading,
    Ready(PiperPaths),
    Failed(String),
}

/// What to use for the next utterance.
#[derive(Debug, PartialEq, Eq)]
enum Choice {
    Piper(PiperPaths),
    /// SAPI, with the reason Piper is not used.
    Sapi(String),
}

impl PiperState {
    /// Picks an engine and says whether a download should start now.
    /// `installed` is looked up lazily and only while no state is known.
    fn choose(
        &mut self,
        installed: impl FnOnce() -> Option<PiperPaths>,
        download_allowed: bool,
    ) -> (Choice, bool) {
        if matches!(self, PiperState::NotChecked)
            && let Some(paths) = installed()
        {
            *self = PiperState::Ready(paths);
        }
        match self {
            PiperState::Ready(paths) => (Choice::Piper(paths.clone()), false),
            PiperState::Downloading => (Choice::Sapi("piper is still downloading".into()), false),
            PiperState::Failed(error) => (
                Choice::Sapi(format!("piper download failed: {error}")),
                false,
            ),
            PiperState::NotChecked if download_allowed => {
                *self = PiperState::Downloading;
                (Choice::Sapi("piper is not installed yet".into()), true)
            }
            PiperState::NotChecked => (Choice::Sapi("piper downloads are disabled".into()), false),
        }
    }
}

/// Tauri-managed speech state.
pub struct Speech {
    controller: Controller,
    piper: Mutex<PiperState>,
    root: PathBuf,
    download_allowed: bool,
}

impl Speech {
    fn new(app_local_data: PathBuf, download_allowed: bool) -> Self {
        Self {
            controller: Controller::default(),
            piper: Mutex::new(PiperState::NotChecked),
            root: assets::piper_root(&app_local_data),
            download_allowed,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SpeechEvent<'a> {
    engine: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interrupted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

#[derive(Serialize, Clone)]
struct DownloadEvent<'a> {
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

/// Registers speech state. Piper is downloaded on the first [`speak`] unless
/// `KJ_TTS_DOWNLOAD=0`, and SAPI covers the wait.
pub fn init(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data = app.path().app_local_data_dir()?;
    let download_allowed = std::env::var("KJ_TTS_DOWNLOAD").map_or(true, |v| v != "0");
    app.manage(Speech::new(data, download_allowed));
    Ok(())
}

/// Speaks `text` without blocking, replacing any speech in progress.
pub fn speak(app: &AppHandle, text: String) {
    let speech = app.state::<Speech>();
    let cancel = speech.controller.begin();
    let app = app.clone();
    std::thread::spawn(move || run_utterance(&app, &text, &cancel));
}

/// Cuts the current speech off (barge-in). Does nothing when silent.
pub fn stop(app: &AppHandle) {
    app.state::<Speech>().controller.stop();
}

fn run_utterance(app: &AppHandle, text: &str, cancel: &Arc<Cancel>) {
    let speech = app.state::<Speech>();
    let (choice, start_download) = lock(&speech.piper).choose(
        || {
            let paths = PiperPaths::under(&speech.root);
            paths.is_installed().then_some(paths)
        },
        speech.download_allowed,
    );
    if start_download {
        start_piper_download(app);
    }

    let mut fallback = None;
    let choice = match choice {
        Choice::Piper(paths) => {
            let engine = PiperEngine::new(paths);
            // Synthesis happens before taking the output so a cancelled
            // utterance never holds up the next one.
            match engine.synthesize(text) {
                Ok(wav) => {
                    if !cancel.is_cancelled() {
                        play(app, &speech, cancel, engine.name(), None, || {
                            playback::play_wav(&wav, cancel)
                        });
                    }
                    return;
                }
                Err(error) => {
                    fallback = Some(format!("piper failed: {error}"));
                    Choice::Sapi(error)
                }
            }
        }
        sapi => sapi,
    };
    if let Choice::Sapi(reason) = choice {
        let reason = fallback.unwrap_or(reason);
        play(app, &speech, cancel, "sapi", Some(&reason), || {
            sapi::speak(text, cancel)
        });
    }
}

fn play(
    app: &AppHandle,
    speech: &Speech,
    cancel: &Cancel,
    engine: &str,
    fallback: Option<&str>,
    work: impl FnOnce() -> Result<(), String>,
) {
    let outcome = speech.controller.run(cancel, || {
        let _ = app.emit(
            "speech-started",
            SpeechEvent {
                engine,
                fallback,
                interrupted: None,
                error: None,
            },
        );
        work()
    });
    match outcome {
        Some(Ok(())) => {
            let _ = app.emit(
                "speech-finished",
                SpeechEvent {
                    engine,
                    fallback: None,
                    interrupted: Some(cancel.is_cancelled()),
                    error: None,
                },
            );
        }
        Some(Err(error)) => {
            let _ = app.emit(
                "speech-failed",
                SpeechEvent {
                    engine,
                    fallback: None,
                    interrupted: None,
                    error: Some(&error),
                },
            );
        }
        None => {}
    }
}

fn start_piper_download(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let speech = app.state::<Speech>();
        let _ = app.emit(
            "speech-download",
            DownloadEvent {
                status: "started",
                error: None,
            },
        );
        match assets::install(&speech.root) {
            Ok(paths) => {
                *lock(&speech.piper) = PiperState::Ready(paths);
                let _ = app.emit(
                    "speech-download",
                    DownloadEvent {
                        status: "ready",
                        error: None,
                    },
                );
            }
            Err(error) => {
                eprintln!("piper download failed: {error}");
                *lock(&speech.piper) = PiperState::Failed(error.clone());
                let _ = app.emit(
                    "speech-download",
                    DownloadEvent {
                        status: "failed",
                        error: Some(&error),
                    },
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn paths() -> PiperPaths {
        PiperPaths::under(std::path::Path::new("root"))
    }

    #[test]
    fn a_new_utterance_cancels_the_one_before_it() {
        let controller = Controller::default();
        let first = controller.begin();
        let second = controller.begin();
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
    }

    #[test]
    fn stop_cancels_the_current_utterance_and_is_harmless_when_silent() {
        let controller = Controller::default();
        controller.stop();
        let current = controller.begin();
        controller.stop();
        assert!(current.is_cancelled());
        controller.stop();
    }

    #[test]
    fn a_cancelled_utterance_never_gets_the_output() {
        let controller = Controller::default();
        let cancel = controller.begin();
        controller.stop();
        assert_eq!(controller.run(&cancel, || 1), None);
    }

    #[test]
    fn only_one_utterance_uses_the_output_at_a_time() {
        let controller = Arc::new(Controller::default());
        let first = controller.begin();
        let (entered, entered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let holder = {
            let controller = Arc::clone(&controller);
            thread::spawn(move || {
                controller.run(&first, || {
                    entered.send(()).unwrap();
                    release_rx.recv().unwrap();
                });
            })
        };
        entered_rx.recv().unwrap();

        let second = controller.begin();
        let (done, done_rx) = mpsc::channel();
        let waiter = {
            let controller = Arc::clone(&controller);
            thread::spawn(move || {
                let ran = controller.run(&second, || ()).is_some();
                done.send(ran).unwrap();
            })
        };
        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
        release.send(()).unwrap();
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(2)), Ok(true));
        holder.join().unwrap();
        waiter.join().unwrap();
    }

    #[test]
    fn installed_piper_is_used_without_downloading() {
        let mut state = PiperState::NotChecked;
        let (choice, download) = state.choose(|| Some(paths()), true);
        assert_eq!(choice, Choice::Piper(paths()));
        assert!(!download);
    }

    #[test]
    fn first_use_starts_one_download_and_speaks_with_sapi_meanwhile() {
        let mut state = PiperState::NotChecked;
        let (choice, download) = state.choose(|| None, true);
        assert!(matches!(choice, Choice::Sapi(_)));
        assert!(download);
        let (choice, download) = state.choose(|| panic!("must not look again"), true);
        assert!(matches!(choice, Choice::Sapi(_)));
        assert!(!download);
    }

    #[test]
    fn a_failed_download_is_not_retried_in_the_same_session() {
        let mut state = PiperState::Failed("offline".into());
        let (choice, download) = state.choose(|| None, true);
        assert_eq!(
            choice,
            Choice::Sapi("piper download failed: offline".into())
        );
        assert!(!download);
    }

    #[test]
    fn disabled_downloads_stay_on_sapi() {
        let mut state = PiperState::NotChecked;
        let (choice, download) = state.choose(|| None, false);
        assert!(matches!(choice, Choice::Sapi(_)));
        assert!(!download);
    }

    #[test]
    fn events_serialise_in_camel_case_without_empty_fields() {
        let json = serde_json::to_value(SpeechEvent {
            engine: "piper",
            fallback: None,
            interrupted: Some(true),
            error: None,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"engine": "piper", "interrupted": true})
        );
    }
}
