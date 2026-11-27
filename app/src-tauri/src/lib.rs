mod audio;
mod capture;
mod hotkey;
mod push_to_talk;
mod tray;

use tauri::{Manager, WindowEvent};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(capture::LatestScreenshot::default());
            app.manage(audio::ActiveRecorder::default());
            app.manage(audio::LatestRecording::default());
            std::thread::spawn(audio::warm_up);
            tray::create(app)?;
            hotkey::register(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Live in the tray: closing the window hides it; Quit is in the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![greet, hotkey::push_to_talk_status])
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
