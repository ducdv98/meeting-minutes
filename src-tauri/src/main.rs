#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use meeting_minutes_core::{Library, MeetingSummary, ScanReport};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State, WindowEvent,
};

type LibraryState = Mutex<Result<Library, String>>;

#[tauri::command]
fn list_meetings(state: State<'_, LibraryState>) -> Result<Vec<MeetingSummary>, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_ref()
        .map_err(Clone::clone)?
        .list_meetings()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rescan_library(state: State<'_, LibraryState>) -> Result<ScanReport, String> {
    let mut guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_mut()
        .map_err(|error| error.clone())?
        .rescan()
        .map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let library = (|| -> Result<Library, String> {
                let root = app
                    .path()
                    .document_dir()
                    .map_err(|error| error.to_string())?
                    .join("Meeting Minutes");
                let index = app
                    .path()
                    .app_data_dir()
                    .map_err(|error| error.to_string())?
                    .join("index.sqlite3");
                Library::open(root, index).map_err(|error| error.to_string())
            })();
            app.manage(Mutex::new(library));

            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .expect("configured app icon")
                        .clone(),
                )
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app in the tray; Quit exits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![list_meetings, rescan_library])
        .run(tauri::generate_context!())
        .expect("error while running Meeting Minutes");
}
