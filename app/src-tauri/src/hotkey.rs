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
    held_ms: u64,
    ready_ms: u64,
    first_sample_ms: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScreenCaptured {
    width: u32,
    height: u32,
    png_bytes: usize,
}

/// Environment variable that overrides the push-to-talk shortcut (ADR 0008).
pub const SHORTCUT_ENV: &str = "KJ_PUSH_TO_TALK";
const DEFAULT_LABEL: &str = "Ctrl+Alt+Space";

pub fn push_to_talk_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space)
}

/// What the UI needs to know about the push-to-talk shortcut.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    /// Human-readable shortcut, e.g. `Ctrl+Alt+Space`.
    pub label: String,
    /// False when another program already owns the shortcut.
    pub registered: bool,
    /// Explains a fallback or a conflict; `None` when all is well.
    pub notice: Option<String>,
}

struct Resolved {
    shortcut: Shortcut,
    label: String,
    notice: Option<String>,
}

/// Picks the shortcut from the override value, falling back to the default
/// when it is absent or cannot be parsed.
fn resolve_shortcut(override_value: Option<&str>) -> Resolved {
    let default = || Resolved {
        shortcut: push_to_talk_shortcut(),
        label: DEFAULT_LABEL.to_string(),
        notice: None,
    };
    let Some(raw) = override_value.map(str::trim).filter(|v| !v.is_empty()) else {
        return default();
    };
    match raw.parse::<Shortcut>() {
        Ok(shortcut) => Resolved {
            shortcut,
            label: raw.to_string(),
            notice: None,
        },
        Err(error) => Resolved {
            notice: Some(format!(
                "{SHORTCUT_ENV}=\"{raw}\" is not a valid shortcut ({error}); using {DEFAULT_LABEL}"
            )),
            ..default()
        },
    }
}

fn conflict_message(label: &str) -> String {
    format!("{label} is used by another app; push-to-talk is off")
}

fn finish_status(resolved: &Resolved, registered: bool) -> ShortcutStatus {
    let notice = if registered {
        resolved.notice.clone()
    } else {
        let conflict = conflict_message(&resolved.label);
        Some(match &resolved.notice {
            Some(earlier) => format!("{earlier}. {conflict}"),
            None => conflict,
        })
    };
    ShortcutStatus {
        label: resolved.label.clone(),
        registered,
        notice,
    }
}

/// Lets the window ask for the status after it loads (events sent during
/// setup would arrive before the page listens).
#[tauri::command]
pub fn push_to_talk_status(status: tauri::State<ShortcutStatus>) -> ShortcutStatus {
    status.inner().clone()
}

pub fn register(app: &App) -> tauri::Result<()> {
    let resolved = resolve_shortcut(std::env::var(SHORTCUT_ENV).ok().as_deref());
    let shortcut = resolved.shortcut;
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

    // A taken shortcut must not stop the app: stay in the tray and say so.
    let registered = match app.global_shortcut().register(shortcut) {
        Ok(()) => {
            eprintln!("push-to-talk registered: {}", resolved.label);
            true
        }
        Err(error) => {
            eprintln!("push-to-talk not registered ({}): {error}", resolved.label);
            false
        }
    };
    let status = finish_status(&resolved, registered);
    if let Some(notice) = &status.notice {
        eprintln!("push-to-talk notice: {notice}");
    }
    let tooltip = if registered {
        format!("kutty-jarvis: hold {} to talk", status.label)
    } else {
        format!("kutty-jarvis: {}", conflict_message(&status.label))
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(tooltip));
    }
    app.manage(status);
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
                held_ms: recording.held_ms,
                ready_ms: recording.ready_ms,
                first_sample_ms: recording.first_sample_ms,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_override_uses_the_default() {
        for value in [None, Some(""), Some("   ")] {
            let resolved = resolve_shortcut(value);
            assert_eq!(resolved.shortcut, push_to_talk_shortcut());
            assert_eq!(resolved.label, "Ctrl+Alt+Space");
            assert_eq!(resolved.notice, None);
        }
    }

    #[test]
    fn valid_override_replaces_the_default() {
        let resolved = resolve_shortcut(Some(" ctrl+alt+k "));
        let expected = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyK);
        assert_eq!(resolved.shortcut, expected);
        assert_eq!(resolved.label, "ctrl+alt+k");
        assert_eq!(resolved.notice, None);
    }

    #[test]
    fn invalid_override_falls_back_with_a_notice() {
        let resolved = resolve_shortcut(Some("ctrl+alt+banana"));
        assert_eq!(resolved.shortcut, push_to_talk_shortcut());
        assert_eq!(resolved.label, "Ctrl+Alt+Space");
        let notice = resolved.notice.expect("notice");
        assert!(notice.contains("ctrl+alt+banana"));
        assert!(notice.contains("using Ctrl+Alt+Space"));
    }

    #[test]
    fn conflict_is_reported_and_keeps_an_earlier_notice() {
        let plain = resolve_shortcut(None);
        let status = finish_status(&plain, false);
        assert!(!status.registered);
        assert_eq!(
            status.notice.as_deref(),
            Some("Ctrl+Alt+Space is used by another app; push-to-talk is off")
        );

        let fallback = resolve_shortcut(Some("nonsense"));
        let status = finish_status(&fallback, false);
        let notice = status.notice.expect("notice");
        assert!(notice.contains("not a valid shortcut"));
        assert!(notice.ends_with("push-to-talk is off"));
    }

    #[test]
    fn success_has_no_notice_unless_it_fell_back() {
        assert_eq!(finish_status(&resolve_shortcut(None), true).notice, None);
        assert!(
            finish_status(&resolve_shortcut(Some("nonsense")), true)
                .notice
                .is_some()
        );
    }
}
