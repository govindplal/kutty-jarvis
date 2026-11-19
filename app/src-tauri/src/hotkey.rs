//! Global push-to-talk hotkey (Ctrl+Alt+Space).
//!
//! On each real transition the hotkey emits a `push-to-talk` event. When the
//! key goes down it also captures the screen in the background (ADR 0009) and
//! emits `screen-captured` or `screen-capture-failed`.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{App, AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::audio::{ActiveRecorder, LatestRecording, Recorder};
use crate::capture::{self, LatestScreenshot};
use crate::push_to_talk::{PushToTalk, PushToTalkState};

pub const PUSH_TO_TALK_EVENT: &str = "push-to-talk";
pub const SCREEN_CAPTURED_EVENT: &str = "screen-captured";
pub const SCREEN_CAPTURE_FAILED_EVENT: &str = "screen-capture-failed";
pub const RECORDING_FINISHED_EVENT: &str = "recording-finished";
pub const RECORDING_FAILED_EVENT: &str = "recording-failed";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RecordingFinished {
    duration_ms: u64,
    wav_bytes: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScreenCaptured {
    width: u32,
    height: u32,
    png_bytes: usize,
}

pub fn push_to_talk_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space)
}

pub fn register(app: &App) -> tauri::Result<()> {
    let shortcut = push_to_talk_shortcut();
    let state = Mutex::new(PushToTalk::default());

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(
                move |app: &AppHandle, pressed: &Shortcut, event: ShortcutEvent| {
                    if pressed != &shortcut {
                        return;
                    }
                    let mut machine = state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let change = match event.state() {
                        ShortcutState::Pressed => machine.press(),
                        ShortcutState::Released => machine.release(),
                    };
                    drop(machine);
                    if let Some(next) = change {
                        emit(app, PUSH_TO_TALK_EVENT, next);
                        match next {
                            PushToTalkState::Listening => {
                                capture_in_background(app.clone());
                                start_recording(app);
                            }
                            PushToTalkState::Idle => finish_recording_in_background(app.clone()),
                        }
                    }
                },
            )
            .build(),
    )?;

    app.global_shortcut()
        .register(push_to_talk_shortcut())
        .map_err(|error| tauri::Error::Anyhow(error.into()))?;
    Ok(())
}

/// Opens the microphone for the duration of the key press (ADR 0010).
fn start_recording(app: &AppHandle) {
    let Some(active) = app.try_state::<ActiveRecorder>() else {
        return;
    };
    match Recorder::start() {
        Ok(recorder) => {
            *active
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(recorder);
        }
        Err(message) => emit(app, RECORDING_FAILED_EVENT, message),
    }
}

/// Closes the microphone and processes the audio off the hotkey thread.
fn finish_recording_in_background(app: AppHandle) {
    let recorder = app.try_state::<ActiveRecorder>().and_then(|active| {
        active
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    });
    let Some(recorder) = recorder else {
        return;
    };
    std::thread::spawn(move || match recorder.finish() {
        Ok(recording) => {
            let payload = RecordingFinished {
                duration_ms: recording.duration_ms,
                wav_bytes: recording.wav.len(),
            };
            if let Some(latest) = app.try_state::<LatestRecording>() {
                *latest
                    .0
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(recording);
            }
            emit(&app, RECORDING_FINISHED_EVENT, payload);
        }
        Err(message) => emit(&app, RECORDING_FAILED_EVENT, message),
    });
}

/// Captures the screen off the hotkey thread. Only ever called on key press.
fn capture_in_background(app: AppHandle) {
    std::thread::spawn(move || match capture::capture_primary() {
        Ok(screenshot) => {
            let payload = ScreenCaptured {
                width: screenshot.width,
                height: screenshot.height,
                png_bytes: screenshot.png.len(),
            };
            if let Some(latest) = app.try_state::<LatestScreenshot>() {
                *latest
                    .0
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(screenshot);
            }
            if let Err(error) = app.emit(SCREEN_CAPTURED_EVENT, payload) {
                eprintln!("failed to emit {SCREEN_CAPTURED_EVENT}: {error}");
            }
        }
        Err(message) => emit(&app, SCREEN_CAPTURE_FAILED_EVENT, message),
    });
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(error) = app.emit(event, payload) {
        eprintln!("failed to emit {event}: {error}");
    }
}
