//! Debug-build-only commands for trying UI element listing and pointing by
//! hand, before the model loop is connected. Compiled out of release builds.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::overlay::{self, DEFAULT_HOLD_MS, PointReport};
use crate::uia::{self, ElementList, ListOptions, WindowTarget};

/// The last listing, so "point at element N" refers to what was shown.
#[derive(Default)]
pub struct LastElements(pub Mutex<Option<ElementList>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub list: ElementList,
    pub prompt: String,
}

/// Lets the frontend detect a debug build: only exists when compiled in.
#[tauri::command]
pub fn debug_pointing_available() -> bool {
    true
}

/// Lists the foreground window's elements, optionally after a delay so the
/// owner can switch to the window they want listed. With `process_id`, lists
/// that process's main window instead (for scripted tests, where focus
/// cannot be arranged).
#[tauri::command]
pub async fn debug_list_elements(
    state: State<'_, LastElements>,
    delay_ms: Option<u64>,
    process_id: Option<u32>,
) -> Result<Listing, String> {
    let options = ListOptions {
        target: process_id.map_or(WindowTarget::Foreground, WindowTarget::Process),
        ..ListOptions::default()
    };
    let list = tauri::async_runtime::spawn_blocking(move || {
        if let Some(delay) = delay_ms {
            std::thread::sleep(Duration::from_millis(delay.min(10_000)));
        }
        uia::list_elements_with(&options)
    })
    .await
    .map_err(|error| format!("element listing failed: {error}"))??;
    let prompt = uia::render_for_prompt(&list);
    *state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(list.clone());
    Ok(Listing { list, prompt })
}

/// Lets the frontend detect a debug build with speech testing.
#[tauri::command]
pub fn debug_speech_available() -> bool {
    true
}

/// Speaks `text` through the normal engine choice (Piper, else SAPI).
#[tauri::command]
pub fn debug_speak(app: AppHandle, text: String) -> u64 {
    crate::tts::speak(&app, text)
}

#[tauri::command]
pub fn debug_stop_speaking(app: AppHandle) {
    crate::tts::stop(&app);
}

/// Points the overlay at element `id` of the last listing.
#[tauri::command]
pub async fn debug_point_at(
    app: AppHandle,
    state: State<'_, LastElements>,
    id: usize,
    hold_ms: Option<u32>,
) -> Result<PointReport, String> {
    let target = state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .ok_or("list the elements first")?
        .get(id)
        .ok_or_else(|| format!("there is no element {id} in the last listing"))?
        .rect;
    overlay::point_at(&app, target, hold_ms.unwrap_or(DEFAULT_HOLD_MS).min(10_000))
}
