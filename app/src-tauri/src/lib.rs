mod audio;
mod capture;
#[cfg(debug_assertions)]
mod debug_commands;
mod hotkey;
/// Pointing overlay; public so the model loop can call `overlay::point_at`.
pub mod overlay;
mod push_to_talk;
mod tray;
/// On-screen UI elements; public so the model loop can call
/// `uia::list_elements` and `uia::render_for_prompt`.
pub mod uia;

use tauri::{Manager, WindowEvent};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(capture::LatestScreenshot::default());
            app.manage(audio::ActiveRecorder::default());
            app.manage(audio::LatestRecording::default());
            std::thread::spawn(audio::warm_up);
            tray::create(app)?;
            hotkey::register(app)?;
            overlay::init(app)?;
            #[cfg(debug_assertions)]
            app.manage(debug_commands::LastElements::default());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Live in the tray: closing the window hides it; Quit is in the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        });

    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        greet,
        hotkey::push_to_talk_status,
        debug_commands::debug_pointing_available,
        debug_commands::debug_list_elements,
        debug_commands::debug_point_at,
    ]);
    #[cfg(not(debug_assertions))]
    let builder =
        builder.invoke_handler(tauri::generate_handler![greet, hotkey::push_to_talk_status]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::greet;

    #[test]
    fn greet_includes_the_name() {
        assert_eq!(greet("Ada"), "Hello, Ada! You've been greeted from Rust!");
    }
}
