//! Connects the model loop to the app: collects what the question needs while
//! the key is held, runs [`super::run`] off the hotkey thread on release, and
//! reports through events.
//!
//! Overlap policy: one question at a time. A question that finishes recording
//! while another is still being answered is dropped and reported as `busy`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{Failure, Input, Reply, Stage};
use crate::models::{Assistant, SpeechToText};
use crate::overlay::{self, DEFAULT_HOLD_MS};
use crate::uia::{self, ElementList};

pub const THINKING_EVENT: &str = "assistant-thinking";
pub const REPLIED_EVENT: &str = "assistant-replied";
pub const FAILED_EVENT: &str = "assistant-failed";

/// How long a question waits for the element list and the screenshot after
/// the key is released. Both normally finish while the user is speaking.
const ELEMENTS_WAIT: Duration = Duration::from_secs(4);
const SCREENSHOT_WAIT: Duration = Duration::from_secs(5);

/// The providers in use, chosen once at startup.
pub struct Providers {
    pub speech_to_text: Arc<dyn SpeechToText>,
    pub assistant: Arc<dyn Assistant>,
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

#[derive(Default)]
pub struct PendingTurn(Mutex<Option<Turn>>);

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
        *pending
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Turn {
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
pub fn answer(app: &AppHandle, wav: Vec<u8>, has_audio: bool) {
    let Some(turn) = app.try_state::<PendingTurn>().and_then(|pending| {
        pending
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }) else {
        return;
    };
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

    let input = Input {
        wav,
        screenshot_png,
        elements: elements.ok(),
    };
    let result = super::run(
        providers.speech_to_text.as_ref(),
        providers.assistant.as_ref(),
        input,
        |transcript| {
            emit(
                app,
                THINKING_EVENT,
                Thinking {
                    stage: "thinking",
                    transcript: Some(transcript.to_string()),
                },
            );
        },
    );
    match result {
        Ok(reply) => deliver(app, reply),
        Err(Failure { stage, message }) => fail(app, stage, message),
    }
}

fn deliver(app: &AppHandle, reply: Reply) {
    emit(
        app,
        REPLIED_EVENT,
        Replied {
            transcript: reply.transcript,
            reply: reply.text,
            pointed_element: reply.target.as_ref().map(|target| PointedElement {
                id: target.id,
                role: target.role.clone(),
                name: target.name.clone(),
            }),
            transcribe_ms: reply.transcribe_time.as_millis() as u64,
            answer_ms: reply.answer_time.as_millis() as u64,
        },
    );
    if let Some(target) = reply.target
        && let Err(error) = overlay::point_at(app, target.rect, DEFAULT_HOLD_MS)
    {
        eprintln!(
            "assistant: could not point at element {}: {error}",
            target.id
        );
    }
}
