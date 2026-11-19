//! System tray icon with a small menu. The app lives in the tray; closing the
//! window hides it instead of quitting.

use tauri::{
    App, AppHandle, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";

pub fn create(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "Open kutty-jarvis", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("kutty-jarvis: hold Ctrl+Alt+Space to talk")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_SHOW => show_main_window(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
