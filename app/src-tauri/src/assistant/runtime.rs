//! Connects the model loop to the app: collects what the question needs while
//! the key is held, runs [`super::run`] off the hotkey thread on release, and
//! reports through events.
//!
//! Overlap policy: one question at a time. A question that finishes recording
//! while another is still being answered is dropped and reported as `busy`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{Failure, Input, Reply, Stage};
use crate::models::select;
use crate::models::{Assistant, SpeechToText};
use crate::overlay::{self, DEFAULT_HOLD_MS};
use crate::secrets::KeyStore;
use crate::settings;
use crate::tts;
use crate::uia::{self, ElementList};

pub const THINKING_EVENT: &str = "assistant-thinking";
pub const REPLIED_EVENT: &str = "assistant-replied";
pub const FAILED_EVENT: &str = "assistant-failed";

/// How long a question waits for the element list and the screenshot after
/// the key is released. Both normally finish while the user is speaking.
const ELEMENTS_WAIT: Duration = Duration::from_secs(4);
const SCREENSHOT_WAIT: Duration = Duration::from_secs(5);

/// The local providers plus what is needed to pick cloud ones per question
/// (ADR 0015).
pub struct Providers {
    pub local_speech_to_text: Arc<dyn SpeechToText>,
    pub local_assistant: Arc<dyn Assistant>,
    pub store: Arc<dyn KeyStore>,
    pub config_dir: PathBuf,
}

/// Set while a question is being answered.
#[derive(Default)]
pub struct Busy(AtomicBool);

struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// What was gathered since the hotkey went down.
pub struct Turn {
    elements: Receiver<Result<ElementList, String>>,
    screenshot: Receiver<Option<Vec<u8>>>,
}

/// Holds the turn started by the latest key press until its release takes it.
/// The release moves the turn into the finalize worker, so a later press can
/// never hand its context to an earlier question.
pub struct TurnSlot<T>(Mutex<Option<T>>);

impl<T> Default for TurnSlot<T> {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl<T> TurnSlot<T> {
    fn begin(&self, turn: T) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = Some(turn);
    }

    fn take(&self) -> Option<T> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).take()
    }
}

pub type PendingTurn = TurnSlot<Turn>;

/// Key released: takes the turn started by the matching press. Call it
/// synchronously on release, together with taking the recorder.
pub fn take_turn(app: &AppHandle) -> Option<Turn> {
    app.try_state::<PendingTurn>()
        .and_then(|pending| pending.take())
}

/// Hands a finished capture (or its failure) to the question in progress.
pub type ScreenshotSender = Sender<Option<Vec<u8>>>;

/// Key went down: starts listing the foreground window's elements (the
/// overlay is never foreground, so this sees the app the user is in) and
/// returns where the screenshot should be delivered.
pub fn begin_turn(app: &AppHandle) -> ScreenshotSender {
    let (elements_tx, elements) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = elements_tx.send(uia::list_elements());
    });
    let (screenshot_tx, screenshot) = mpsc::channel();
    if let Some(pending) = app.try_state::<PendingTurn>() {
        pending.begin(Turn {
            elements,
            screenshot,
        });
    }
    screenshot_tx
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Thinking {
    stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PointedElement {
    id: usize,
    role: String,
    name: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Replied {
    transcript: String,
    reply: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pointed_element: Option<PointedElement>,
    transcribe_ms: u64,
    answer_ms: u64,
    used: select::Used,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Failed {
    stage: Stage,
    message: String,
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(error) = app.emit(event, payload) {
        eprintln!("failed to emit {event}: {error}");
    }
}

fn fail(app: &AppHandle, stage: Stage, message: impl Into<String>) {
    emit(
        app,
        FAILED_EVENT,
        Failed {
            stage,
            message: message.into(),
        },
    );
}

/// Key released and the recording is finished: answer it. Blocks, so call it
/// from a background thread.
pub fn answer(app: &AppHandle, turn: Turn, wav: Vec<u8>, has_audio: bool) {
    if !has_audio {
        fail(
            app,
            Stage::Recording,
            "No audio was recorded; check the microphone",
        );
        return;
    }
    let (Some(providers), Some(busy)) = (app.try_state::<Providers>(), app.try_state::<Busy>())
    else {
        return;
    };
    if busy.0.swap(true, Ordering::SeqCst) {
        fail(app, Stage::Busy, "Still answering the previous question");
        return;
    }
    let _guard = BusyGuard(&busy.0);

    let chosen = select::choose(
        &settings::load(&providers.config_dir),
        providers.store.as_ref(),
        &providers.local_speech_to_text,
        &providers.local_assistant,
    );
    let used = chosen.used;
    let (stt, assistant) = match (chosen.speech_to_text, chosen.assistant) {
        (Err(message), _) => return fail(app, Stage::Transcribe, message),
        (_, Err(message)) => return fail(app, Stage::Answer, message),
        (Ok(stt), Ok(assistant)) => (stt, assistant),
    };

    emit(
        app,
        THINKING_EVENT,
        Thinking {
            stage: "transcribing",
            transcript: None,
        },
    );
    let elements = turn
        .elements
        .recv_timeout(ELEMENTS_WAIT)
        .map_err(|_| "the element list was not ready".to_string())
        .and_then(|listed| listed);
    if let Err(message) = &elements {
        eprintln!("assistant: no UI element list ({message}); answering without pointing");
    }
    let screenshot_png = turn.screenshot.recv_timeout(SCREENSHOT_WAIT).ok().flatten();
    if screenshot_png.is_none() {
        eprintln!("assistant: no screenshot; the model is told it cannot see the screen");
    }

    let input = Input {
        wav,
        screenshot_png,
        elements: elements.ok(),
    };
    let result = super::run(stt.as_ref(), assistant.as_ref(), input, |transcript| {
        emit(
            app,
            THINKING_EVENT,
            Thinking {
                stage: "thinking",
                transcript: Some(transcript.to_string()),
            },
        );
    });
    match result {
        Ok(reply) => deliver(app, reply, used),
        Err(Failure { stage, message }) => fail(app, stage, message),
    }
}

fn deliver(app: &AppHandle, reply: Reply, used: select::Used) {
    // Errors are never spoken; only a real reply (tags already stripped).
    if !reply.text.trim().is_empty() {
        tts::speak(app, reply.text.clone());
    }
    // Report a pointed element only when the overlay really showed it.
    let pointed_element = reply.target.and_then(|target| {
        match overlay::point_at(app, target.rect, DEFAULT_HOLD_MS) {
            Ok(_) => Some(PointedElement {
                id: target.id,
                role: target.role,
                name: target.name,
            }),
            Err(error) => {
                eprintln!(
                    "assistant: could not point at element {}: {error}",
                    target.id
                );
                None
            }
        }
    });
    emit(
        app,
        REPLIED_EVENT,
        Replied {
            transcript: reply.transcript,
            reply: reply.text,
            pointed_element,
            transcribe_ms: reply.transcribe_time.as_millis() as u64,
            answer_ms: reply.answer_time.as_millis() as u64,
            used,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delayed_finalize_keeps_the_context_of_its_own_press() {
        let slot = TurnSlot::<&str>::default();
        slot.begin("screen 1");
        // Release 1 takes its turn synchronously, then hands it to a worker
        // that has not run yet.
        let first = slot.take();
        // A rapid second press starts its own turn before that worker runs.
        slot.begin("screen 2");
        assert_eq!(first, Some("screen 1"));
        assert_eq!(slot.take(), Some("screen 2"));
        assert_eq!(slot.take(), None);
    }

    #[test]
    fn a_release_without_a_press_has_no_turn() {
        assert_eq!(TurnSlot::<u8>::default().take(), None);
    }
}
