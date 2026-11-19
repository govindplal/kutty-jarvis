//! Global push-to-talk hotkey (Ctrl+Alt+Space).
//!
//! Emits a `push-to-talk` event to the frontend with `"listening"` or `"idle"`
//! whenever the state really changes.

use std::sync::Mutex;

use tauri::{App, AppHandle, Emitter};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::push_to_talk::PushToTalk;

pub const PUSH_TO_TALK_EVENT: &str = "push-to-talk";

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
                    if let Some(next) = change
                        && let Err(error) = app.emit(PUSH_TO_TALK_EVENT, next)
                    {
                        eprintln!("failed to emit {PUSH_TO_TALK_EVENT}: {error}");
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
