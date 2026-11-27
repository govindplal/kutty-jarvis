/// The model loop; public for the `ask` example.
pub mod assistant;
mod audio;
mod capture;
#[cfg(debug_assertions)]
mod debug_commands;
mod hotkey;
mod model_commands;
/// Model providers and local server management; public for the `ask` example.
pub mod models;
/// Pointing overlay; public so the model loop can call `overlay::point_at`.
pub mod overlay;
mod push_to_talk;
mod secrets;
mod settings;
mod tray;
/// Spoken replies; public so the model loop can call `tts::speak` and
/// `tts::stop`.
pub mod tts;
/// On-screen UI elements; public so the model loop can call
/// `uia::list_elements` and `uia::render_for_prompt`.
pub mod uia;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};

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
            app.manage(assistant::runtime::PendingTurn::default());
            app.manage(assistant::runtime::Busy::default());
            let servers = Arc::new(models::llama_server::LlamaServers::new(
                app.path().app_local_data_dir()?,
            ));
            app.manage(assistant::runtime::Providers {
                local_speech_to_text: servers.speech_to_text(),
                local_assistant: servers.assistant(),
                store: secrets::default_store(),
                config_dir: app.path().app_config_dir()?,
            });
            app.manage(servers);
            std::thread::spawn(audio::warm_up);
            tray::create(app)?;
            hotkey::register(app)?;
            overlay::init(app)?;
            tts::init(app)?;
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
        model_commands::get_model_settings,
        model_commands::set_model_settings,
        model_commands::set_api_key,
        model_commands::clear_api_key,
        model_commands::api_key_status,
        debug_commands::debug_pointing_available,
        debug_commands::debug_list_elements,
        debug_commands::debug_point_at,
        debug_commands::debug_speech_available,
        debug_commands::debug_speak,
        debug_commands::debug_stop_speaking,
    ]);
    #[cfg(not(debug_assertions))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        greet,
        hotkey::push_to_talk_status,
        model_commands::get_model_settings,
        model_commands::set_model_settings,
        model_commands::set_api_key,
        model_commands::clear_api_key,
        model_commands::api_key_status,
    ]);

    builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                tts::shutdown(app);
                // Only the model servers this app started are stopped.
                if let Some(servers) = app.try_state::<Arc<models::llama_server::LlamaServers>>() {
                    servers.stop_all();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::greet;

    #[test]
    fn greet_includes_the_name() {
        assert_eq!(greet("Ada"), "Hello, Ada! You've been greeted from Rust!");
    }
}
