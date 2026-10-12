#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};

use meeting_minutes_core::{
    capture::{QpcClock, WasapiOpener},
    models::{self, DEFAULT_MODEL, MODELS},
    processing::{self, JobStage, ProcessingStatus, Processor, TranscribeRequest},
    recorder::{Phase, Recorder, RecorderError, RecorderStatus, SaveJob, SaveState},
    settings::Settings,
    transcript::{Language, Transcript},
    Library, LibraryError, MeetingDetail, MeetingSummary, ScanReport,
};
use meeting_minutes_whisper::WhisperEngine;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

type LibraryState = Mutex<Result<Library, String>>;
type RecorderState = Mutex<Recorder<WasapiOpener, QpcClock>>;
type ProcessorState = Mutex<Processor>;

const RECORD_SHORTCUT: &str = "ctrl+alt+shift+r";
const PAUSE_SHORTCUT: &str = "ctrl+alt+shift+p";

struct SettingsPath(PathBuf);
struct ModelsDir(PathBuf);
struct Quitting(AtomicBool);

fn lock_processor(app: &AppHandle) -> Result<std::sync::MutexGuard<'_, Processor>, String> {
    app.state::<ProcessorState>()
        .inner()
        .lock()
        .map_err(|e| e.to_string())
}

fn meeting_busy(app: &AppHandle, meeting_id: &str) -> Result<(), String> {
    let processor = lock_processor(app)?;
    if processor.is_busy(meeting_id) {
        return Err("This Meeting is being processed".into());
    }
    let recorder = lock_recorder(app)?;
    let status = recorder.status();
    if status.meeting_id.as_deref() == Some(meeting_id)
        || status
            .saves
            .iter()
            .any(|save| save.meeting_id == meeting_id)
    {
        return Err(LibraryError::UnsavedRecording.to_string());
    }
    Ok(())
}

fn publish_processing(app: &AppHandle) -> Result<ProcessingStatus, String> {
    let status = lock_processor(app)?.status();
    let _ = app.emit("processing-status", &status);
    Ok(status)
}

fn processing_stage(app: &AppHandle, id: &str, stage: JobStage, progress: f32) {
    if let Ok(mut processor) = lock_processor(app) {
        processor.set_stage(id, stage, progress);
    }
    let _ = publish_processing(app);
}

#[tauri::command]
fn processing_status(app: AppHandle) -> Result<ProcessingStatus, String> {
    publish_processing(&app)
}

#[tauri::command]
fn transcribe_meeting(app: AppHandle, meeting_id: String) -> Result<ProcessingStatus, String> {
    let mut processor = lock_processor(&app)?;
    {
        let library_state = app.state::<LibraryState>();
        let library = library_state.lock().map_err(|e| e.to_string())?;
        library
            .as_ref()
            .map_err(Clone::clone)?
            .meeting(&meeting_id)
            .map_err(|e| e.to_string())?;
    }
    processor.enqueue(&meeting_id);
    drop(processor);
    publish_processing(&app)
}

#[tauri::command]
fn cancel_processing(app: AppHandle, meeting_id: String) -> Result<ProcessingStatus, String> {
    lock_processor(&app)?.cancel(&meeting_id);
    publish_processing(&app)
}

#[tauri::command]
fn dismiss_processing(app: AppHandle, meeting_id: String) -> Result<ProcessingStatus, String> {
    lock_processor(&app)?.dismiss(&meeting_id);
    publish_processing(&app)
}

#[tauri::command]
fn meeting_transcript(
    state: State<'_, LibraryState>,
    meeting_id: String,
) -> Result<Option<Transcript>, String> {
    let library = state.lock().map_err(|e| e.to_string())?;
    library
        .as_ref()
        .map_err(Clone::clone)?
        .transcript(&meeting_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_meeting_language(
    app: AppHandle,
    meeting_id: String,
    language: Option<Language>,
) -> Result<(), String> {
    meeting_busy(&app, &meeting_id)?;
    let library_state = app.state::<LibraryState>();
    let library = library_state.lock().map_err(|e| e.to_string())?;
    library
        .as_ref()
        .map_err(Clone::clone)?
        .set_language(&meeting_id, language)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn get_settings(path: State<'_, SettingsPath>) -> Settings {
    Settings::load(&path.0)
}

#[tauri::command]
fn update_settings(
    path: State<'_, SettingsPath>,
    auto_process: bool,
    model: String,
    vocabulary: Vec<String>,
) -> Result<Settings, String> {
    let mut settings = Settings::load(&path.0);
    settings.auto_process = auto_process;
    settings.model = model;
    settings.vocabulary = vocabulary;
    settings.save(&path.0).map_err(|e| e.to_string())?;
    Ok(settings)
}

fn processing_worker(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(100));
        if app.state::<Quitting>().0.load(Ordering::SeqCst) {
            continue;
        }
        if !matches!(status(&app), Ok(status)
            if status.phase == Phase::Idle
                && !status.saves.iter().any(|save| save.state == SaveState::Saving))
        {
            continue;
        }
        let id = match lock_processor(&app).ok().and_then(|mut p| p.next()) {
            Some(id) => id,
            None => continue,
        };
        if app.state::<Quitting>().0.load(Ordering::SeqCst) {
            if let Ok(mut processor) = lock_processor(&app) {
                processor.finished(&id, Err(processing::ProcessingError::Cancelled));
            }
            continue;
        }
        let _ = publish_processing(&app);
        let cancel = match lock_processor(&app) {
            Ok(p) => p.cancel_flag(),
            Err(_) => continue,
        };
        let result = (|| -> Result<(), processing::ProcessingError> {
            let meeting_dir = {
                let library_state = app.state::<LibraryState>();
                let library = library_state
                    .lock()
                    .map_err(|e| processing::ProcessingError::Engine(e.to_string()))?;
                let library = library
                    .as_ref()
                    .map_err(|e| processing::ProcessingError::Engine(e.clone()))?;
                library
                    .meeting(&id)
                    .map_err(|e| processing::ProcessingError::Engine(e.to_string()))?;
                library.meeting_dir(&id)
            };
            let settings = Settings::load(&app.state::<SettingsPath>().0);
            let model = MODELS
                .iter()
                .find(|m| m.name == settings.model)
                .unwrap_or_else(|| MODELS.iter().find(|m| m.name == DEFAULT_MODEL).unwrap());
            let models_state = app.state::<ModelsDir>();
            let models_dir = &models_state.0;
            let model_path = if models::is_installed(models_dir, model) {
                models::model_path(models_dir, model)
            } else {
                processing_stage(&app, &id, JobStage::Downloading, 0.0);
                let mut last = 0u32;
                models::download(
                    models_dir,
                    model,
                    &mut |bytes| {
                        let percent = (bytes.saturating_mul(100) / model.size) as u32;
                        if percent > last {
                            last = percent;
                            processing_stage(
                                &app,
                                &id,
                                JobStage::Downloading,
                                bytes as f32 / model.size as f32,
                            );
                        }
                    },
                    &cancel,
                )?
            };
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(processing::ProcessingError::Cancelled);
            }
            processing_stage(&app, &id, JobStage::Transcribing, 0.0);
            let mut engine =
                WhisperEngine::load(&model_path).map_err(processing::ProcessingError::Engine)?;
            let mut last = 0u32;
            processing::transcribe_meeting(
                &TranscribeRequest {
                    meeting_dir: &meeting_dir,
                    model: model.name,
                    vocabulary: &settings.vocabulary,
                },
                &mut engine,
                &mut |fraction| {
                    let percent = (fraction * 100.0) as u32;
                    if percent > last {
                        last = percent;
                        processing_stage(&app, &id, JobStage::Transcribing, fraction);
                    }
                },
                &cancel,
            )?;
            Ok(())
        })();
        let success = result.is_ok();
        if let Ok(mut processor) = lock_processor(&app) {
            processor.finished(&id, result);
        }
        let _ = publish_processing(&app);
        if success {
            let _ = app.emit("library-changed", ());
        }
    });
}

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
fn meeting_detail(
    state: State<'_, LibraryState>,
    meeting_id: String,
) -> Result<MeetingDetail, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_ref()
        .map_err(Clone::clone)?
        .meeting(&meeting_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_meeting(
    app: AppHandle,
    meeting_id: String,
    title: String,
) -> Result<MeetingSummary, String> {
    meeting_busy(&app, &meeting_id)?;
    let summary = {
        let mut recorder = lock_recorder(&app)?;
        let library_state = app.state::<LibraryState>();
        let mut library = library_state.lock().map_err(|error| error.to_string())?;
        let library = library.as_mut().map_err(|error| error.clone())?;
        let summary = library
            .rename_meeting(&meeting_id, &title)
            .map_err(|error| error.to_string())?;
        recorder.meeting_renamed(&meeting_id, &summary.id);
        summary
    };
    let _ = app.emit("library-changed", ());
    let _ = publish(&app);
    Ok(summary)
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
                if !app.state::<Quitting>().0.load(Ordering::SeqCst)
                    && Settings::load(&app.state::<SettingsPath>().0).auto_process
                {
                    if let Ok(mut processor) = lock_processor(&app) {
                        processor.enqueue(&job.meeting_id);
                    }
                    let _ = publish_processing(&app);
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
    app.state::<Quitting>().0.store(true, Ordering::SeqCst);
    if let Ok(status) = publish_processing(app) {
        for job in status.jobs {
            if let Ok(mut processor) = lock_processor(app) {
                processor.cancel(&job.meeting_id);
            }
        }
    }
    if matches!(status(app), Ok(status) if status.phase != Phase::Idle) {
        let _ = stop(app);
    }
    let app = app.clone();
    thread::spawn(move || {
        while matches!(status(&app), Ok(status)
            if status.saves.iter().any(|save| save.state == SaveState::Saving))
            || lock_processor(&app).is_ok_and(|processor| processor.is_running())
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
            app.manage(ModelsDir(app.path().app_data_dir()?.join("models")));
            app.manage(Quitting(AtomicBool::new(false)));
            app.manage(Mutex::new(Processor::new()));
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
            processing_worker(app.handle().clone());

            // Elapsed time for the window indicator and the tray while a Recording runs.
            let handle = app.handle().clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(500));
                if matches!(status(&handle), Ok(status) if status.phase == Phase::Recording) {
                    if let Ok(mut recorder) = lock_recorder(&handle) {
                        let _ = recorder.collect_device_switches();
                    }
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
            meeting_detail,
            rename_meeting,
            recording_status,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            retry_save,
            continue_meeting,
            dismiss_continue,
            acknowledge_recording_notice,
            processing_status,
            transcribe_meeting,
            cancel_processing,
            dismiss_processing,
            meeting_transcript,
            set_meeting_language,
            get_settings,
            update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Meeting Minutes");
}
