#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Mutex, thread, time::Duration};

use meeting_minutes_core::{
    capture::{QpcClock, WasapiOpener},
    recorder::{Phase, Recorder, RecorderError, RecorderStatus, SaveJob, SaveState},
    settings::Settings,
    Library, MeetingSummary, ScanReport,
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

type LibraryState = Mutex<Result<Library, String>>;
type RecorderState = Mutex<Recorder<WasapiOpener, QpcClock>>;

const RECORD_SHORTCUT: &str = "ctrl+alt+shift+r";
const PAUSE_SHORTCUT: &str = "ctrl+alt+shift+p";

struct SettingsPath(PathBuf);

struct Tray {
    icon: TrayIcon,
    status: MenuItem<tauri::Wry>,
    record: MenuItem<tauri::Wry>,
    pause: MenuItem<tauri::Wry>,
    stop: MenuItem<tauri::Wry>,
    idle_image: Image<'static>,
    recording_image: Image<'static>,
}

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

#[tauri::command]
fn recording_status(app: AppHandle) -> Result<RecorderStatus, String> {
    status(&app)
}

#[tauri::command]
fn start_recording(app: AppHandle) -> Result<RecorderStatus, String> {
    start(&app)
}

#[tauri::command]
fn pause_recording(app: AppHandle) -> Result<RecorderStatus, String> {
    with_recorder(&app, |recorder| recorder.pause())
}

#[tauri::command]
fn resume_recording(app: AppHandle) -> Result<RecorderStatus, String> {
    with_recorder(&app, |recorder| recorder.resume())
}

#[tauri::command]
fn stop_recording(app: AppHandle) -> Result<RecorderStatus, String> {
    stop(&app)
}

#[tauri::command]
fn retry_save(app: AppHandle, meeting_id: String) -> Result<RecorderStatus, String> {
    let job = lock_recorder(&app)?
        .retry_save(&meeting_id)
        .map_err(|error| error.to_string())?;
    spawn_save(app.clone(), job);
    publish(&app)
}

#[tauri::command]
fn continue_meeting(app: AppHandle, meeting_id: String) -> Result<RecorderStatus, String> {
    start_with(&app, |recorder, library, started_at| {
        recorder.continue_meeting(library, &meeting_id, started_at)
    })
}

#[tauri::command]
fn dismiss_continue(app: AppHandle, meeting_id: String) -> Result<RecorderStatus, String> {
    lock_recorder(&app)?.dismiss_continue(&meeting_id);
    publish(&app)
}

#[tauri::command]
fn acknowledge_recording_notice(
    app: AppHandle,
    settings_path: State<'_, SettingsPath>,
) -> Result<RecorderStatus, String> {
    let mut settings = Settings::load(&settings_path.0);
    settings.recording_notice_acknowledged = true;
    settings
        .save(&settings_path.0)
        .map_err(|error| error.to_string())?;
    lock_recorder(&app)?.acknowledge_notice();
    publish(&app)
}

fn lock_recorder(
    app: &AppHandle,
) -> Result<std::sync::MutexGuard<'_, Recorder<WasapiOpener, QpcClock>>, String> {
    app.state::<RecorderState>()
        .inner()
        .lock()
        .map_err(|error| error.to_string())
}

fn status(app: &AppHandle) -> Result<RecorderStatus, String> {
    Ok(lock_recorder(app)?.status())
}

/// Sends the current state to the window and the tray.
fn publish(app: &AppHandle) -> Result<RecorderStatus, String> {
    let status = status(app)?;
    let _ = app.emit("recording-status", &status);
    update_tray(app, &status);
    Ok(status)
}

fn with_recorder(
    app: &AppHandle,
    action: impl FnOnce(&mut Recorder<WasapiOpener, QpcClock>) -> Result<(), RecorderError>,
) -> Result<RecorderStatus, String> {
    action(&mut *lock_recorder(app)?).map_err(|error| error.to_string())?;
    publish(app)
}

fn start(app: &AppHandle) -> Result<RecorderStatus, String> {
    start_with(app, |recorder, library, started_at| {
        recorder.start(library, started_at)
    })
}

/// Starts a Recording, in a new Meeting or continuing one, with both the recorder and Library locked.
fn start_with(
    app: &AppHandle,
    action: impl FnOnce(
        &mut Recorder<WasapiOpener, QpcClock>,
        &mut Library,
        chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<(), RecorderError>,
) -> Result<RecorderStatus, String> {
    {
        let mut recorder = lock_recorder(app)?;
        let library_state = app.state::<LibraryState>();
        let mut library = library_state.lock().map_err(|error| error.to_string())?;
        let library = library.as_mut().map_err(|error| error.clone())?;
        action(&mut recorder, library, chrono::Local::now().fixed_offset())
            .map_err(|error| error.to_string())?;
    }
    let _ = app.emit("library-changed", ());
    publish(app)
}

fn stop(app: &AppHandle) -> Result<RecorderStatus, String> {
    let stopped = lock_recorder(app)?.stop();
    match stopped {
        Ok(job) => spawn_save(app.clone(), job),
        Err(error) => {
            let _ = publish(app);
            return Err(error.to_string());
        }
    }
    publish(app)
}

/// Writes the FLAC, MP3 and meeting.json off the UI thread; a failure keeps the Chunks for Retry save.
fn spawn_save(app: AppHandle, job: SaveJob) {
    spawn_saves(app, vec![job]);
}

/// Runs saves one after another, so Recordings of the same Meeting are saved in order.
fn spawn_saves(app: AppHandle, jobs: Vec<SaveJob>) {
    thread::spawn(move || {
        for job in jobs {
            let result = job.run().map_err(|error| error.to_string());
            if result.is_ok() {
                if let Ok(mut library) = app.state::<LibraryState>().lock() {
                    if let Ok(library) = library.as_mut() {
                        // The files are saved; a stale index row is fixed by the next rescan.
                        let _ = library.record_finalized(&job.meeting_id);
                    }
                }
            }
            if let Ok(mut recorder) = app.state::<RecorderState>().lock() {
                recorder.save_finished(&job.meeting_id, result);
            }
            let _ = app.emit("library-changed", ());
            let _ = publish(&app);
        }
    });
}

/// Queues the Chunks a crash left behind, so they are saved as Recordings of their Meetings.
fn recover(app: &AppHandle) {
    let jobs = {
        let Ok(mut recorder) = lock_recorder(app) else {
            return;
        };
        let library_state = app.state::<LibraryState>();
        let Ok(library) = library_state.lock() else {
            return;
        };
        let Ok(library) = library.as_ref() else {
            return;
        };
        // Recovery is retried on the next launch if the Library cannot be read now.
        recorder.recover(library).unwrap_or_default()
    };
    if !jobs.is_empty() {
        spawn_saves(app.clone(), jobs);
    }
}

/// Record from the tray or hotkey: before the first Recording ever, show the notice instead.
fn request_start(app: &AppHandle) {
    let needs_notice = lock_recorder(app)
        .map(|recorder| !recorder.status().notice_acknowledged)
        .unwrap_or(false);
    if needs_notice {
        show_window(app);
        let _ = app.emit("recording-notice-requested", ());
        return;
    }
    let _ = start(app);
}

fn toggle_record(app: &AppHandle) {
    match status(app).map(|status| status.phase) {
        Ok(Phase::Idle) => request_start(app),
        Ok(_) => {
            let _ = stop(app);
        }
        Err(_) => {}
    }
}

fn toggle_pause(app: &AppHandle) {
    let _ = match status(app).map(|status| status.phase) {
        Ok(Phase::Recording) => with_recorder(app, |recorder| recorder.pause()),
        Ok(Phase::Paused) => with_recorder(app, |recorder| recorder.resume()),
        _ => return,
    };
}

/// Saves an active Recording and waits for running saves before exiting, so no save is cut off
/// half-written. Waits off the main thread: tray updates from the save thread need it.
fn quit(app: &AppHandle) {
    if matches!(status(app), Ok(status) if status.phase != Phase::Idle) {
        let _ = stop(app);
    }
    let app = app.clone();
    thread::spawn(move || {
        while matches!(status(&app), Ok(status)
            if status.saves.iter().any(|save| save.state == SaveState::Saving))
        {
            thread::sleep(Duration::from_millis(100));
        }
        app.exit(0);
    });
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn format_elapsed(ms: u64) -> String {
    let seconds = ms / 1000;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn update_tray(app: &AppHandle, status: &RecorderStatus) {
    let Some(tray) = app.try_state::<Tray>() else {
        return;
    };
    let elapsed = format_elapsed(status.elapsed_ms);
    let (label, image) = match status.phase {
        Phase::Idle => ("Not recording".to_owned(), &tray.idle_image),
        Phase::Recording => (format!("Recording {elapsed}"), &tray.recording_image),
        Phase::Paused => (format!("Paused {elapsed}"), &tray.recording_image),
    };
    let tooltip = match status.phase {
        Phase::Idle => "Meeting Minutes".to_owned(),
        _ => format!("Meeting Minutes - {label}"),
    };
    let _ = tray.icon.set_tooltip(Some(tooltip));
    let _ = tray.icon.set_icon(Some(image.clone()));
    let _ = tray.status.set_text(label);
    let _ = tray.record.set_enabled(status.phase == Phase::Idle);
    let _ = tray.pause.set_enabled(status.phase != Phase::Idle);
    let _ = tray.pause.set_text(if status.phase == Phase::Paused {
        "Resume"
    } else {
        "Pause"
    });
    let _ = tray.stop.set_enabled(status.phase != Phase::Idle);
}

/// The app icon with a red dot in the lower right corner.
fn recording_icon(icon: &Image<'_>) -> Image<'static> {
    let (width, height) = (icon.width(), icon.height());
    let mut rgba = icon.rgba().to_vec();
    let radius = width.min(height) as f32 * 0.22;
    let (cx, cy) = (width as f32 - radius - 1.0, height as f32 - radius - 1.0);
    for y in 0..height {
        for x in 0..width {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dx * dx + dy * dy <= radius * radius {
                let offset = ((y * width + x) * 4) as usize;
                rgba[offset..offset + 4].copy_from_slice(&[214, 40, 40, 255]);
            }
        }
    }
    Image::new_owned(rgba, width, height)
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts([RECORD_SHORTCUT, PAUSE_SHORTCUT])
                .expect("valid shortcuts")
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    if shortcut == &RECORD_SHORTCUT.parse::<Shortcut>().expect("valid shortcut") {
                        toggle_record(app);
                    } else if shortcut
                        == &PAUSE_SHORTCUT.parse::<Shortcut>().expect("valid shortcut")
                    {
                        toggle_pause(app);
                    }
                })
                .build(),
        )
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

            let settings_path = app.path().app_data_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
            app.manage(SettingsPath(settings_path));
            app.manage(Mutex::new(Recorder::new(
                WasapiOpener,
                QpcClock::new()?,
                settings.recording_notice_acknowledged,
            )));

            let open_item = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let status_item =
                MenuItem::with_id(app, "status", "Not recording", false, None::<&str>)?;
            let record_item = MenuItem::with_id(app, "record", "Record", true, None::<&str>)?;
            let pause_item = MenuItem::with_id(app, "pause", "Pause", false, None::<&str>)?;
            let stop_item = MenuItem::with_id(app, "stop", "Stop", false, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let separator_end = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(
                app,
                &[
                    &open_item,
                    &separator,
                    &status_item,
                    &record_item,
                    &pause_item,
                    &stop_item,
                    &separator_end,
                    &quit_item,
                ],
            )?;

            let idle_image = app
                .default_window_icon()
                .expect("configured app icon")
                .clone()
                .to_owned();
            let recording_image = recording_icon(&idle_image);
            let icon = TrayIconBuilder::new()
                .icon(idle_image.clone())
                .tooltip("Meeting Minutes")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_window(app),
                    "record" => request_start(app),
                    "pause" => toggle_pause(app),
                    "stop" => {
                        let _ = stop(app);
                    }
                    "quit" => quit(app),
                    _ => {}
                })
                .build(app)?;
            app.manage(Tray {
                icon,
                status: status_item,
                record: record_item,
                pause: pause_item,
                stop: stop_item,
                idle_image,
                recording_image,
            });

            recover(app.handle());

            // Elapsed time for the window indicator and the tray while a Recording runs.
            let handle = app.handle().clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(500));
                if matches!(status(&handle), Ok(status) if status.phase == Phase::Recording) {
                    let _ = publish(&handle);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app in the tray; Quit exits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_meetings,
            rescan_library,
            recording_status,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            retry_save,
            continue_meeting,
            dismiss_continue,
            acknowledge_recording_notice
        ])
        .run(tauri::generate_context!())
        .expect("error while running Meeting Minutes");
}
