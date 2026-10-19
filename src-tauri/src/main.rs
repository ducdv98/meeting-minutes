#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod snapshot_windows;

use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use meeting_minutes_core::{
    ai::{
        self, AiStatus, ApiKey, GenerateRequest, HttpClient, Message, ProviderKind, Role,
        SecretStore, TextProvider, UreqClient,
    },
    ask::{self, AskOptions},
    background,
    capture::{audio_devices, QpcClock, Source, WasapiOpener},
    cloud::{
        gemini::GeminiEngine,
        groq::GroqEngine,
        managed::{self, ManagedEngine, ManagedKey, ManagedKeyState, ManagedKeyStore},
        CloudEngine,
    },
    diagnostics::{build_diagnostics, log_event, DiagnosticsInput, LogEvent},
    import::import_extension,
    language_models::{LanguageModel, LanguageModelSettings},
    microphone_usage::{MicrophoneApp, MicrophoneUsage, SystemMicrophoneUsage},
    model_downloads::{ModelDownloadStatus, ModelDownloads},
    models::{self, DEFAULT_MODEL, MODELS},
    processing::{self, JobStage, ProcessingStatus, Processor, SpeechEngine, TranscribeRequest},
    recorder::{Phase, Recorder, RecorderError, RecorderStatus, SaveJob, SaveState},
    settings::Settings,
    speech_models::{SpeechProvider, SpeechSettings},
    summary::{self, Summary, SummaryOptions},
    transcript::{Language, Speaker, Transcript},
    window_fit::{fit_window, Rect},
    Library, LibraryError, MeetingDetail, MeetingSummary, ScanReport, SearchHit,
};
use meeting_minutes_whisper::{WhisperEngine, WhisperSpeechDetector};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, DragDropEvent, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
use tauri_plugin_updater::UpdaterExt;

type LibraryState = Mutex<Result<Library, String>>;
type RecorderState = Mutex<Recorder<WasapiOpener, QpcClock>>;
type ProcessorState = Mutex<Processor>;

/// Background transcription of the Recording in progress, and the caches of stopped ones (ADR 0010).
/// Everything lives in memory only, so a crash discards it and the Meeting is transcribed from its Chunks.
#[derive(Default)]
struct BackgroundTranscription {
    runs: Vec<BackgroundRun>,
    /// Results of stopped runs, by Meeting ID, waiting for that Meeting's Processing.
    results: Vec<(String, background::BackgroundResult)>,
}

struct BackgroundRun {
    meeting_id: String,
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<Option<background::BackgroundResult>>,
}

type BackgroundState = Mutex<BackgroundTranscription>;

#[derive(Clone, serde::Serialize)]
struct TranscriptDraftEvent {
    meeting_id: String,
    segments: Vec<meeting_minutes_core::transcript::PlacedSegment>,
}

/// The Draft of the Meeting being transcribed, kept in memory only so a Meeting
/// view opened mid-Processing can catch up. Never written to the Library.
#[derive(Default)]
struct DraftState {
    meeting_id: String,
    segments: Vec<meeting_minutes_core::transcript::PlacedSegment>,
}

type DraftStore = Mutex<DraftState>;

fn emit_draft(
    app: &AppHandle,
    meeting_id: &str,
    segments: &[meeting_minutes_core::transcript::PlacedSegment],
) {
    if let Ok(mut draft) = app.state::<DraftStore>().lock() {
        if draft.meeting_id != meeting_id {
            draft.meeting_id = meeting_id.to_string();
            draft.segments.clear();
        }
        draft.segments.extend_from_slice(segments);
    }
    let _ = app.emit(
        "transcript-draft",
        TranscriptDraftEvent {
            meeting_id: meeting_id.to_string(),
            segments: segments.to_vec(),
        },
    );
}

fn clear_draft(app: &AppHandle) {
    if let Ok(mut draft) = app.state::<DraftStore>().lock() {
        *draft = DraftState::default();
    }
}

const RECORD_SHORTCUT: &str = "ctrl+alt+shift+r";
const PAUSE_SHORTCUT: &str = "ctrl+alt+shift+p";
const UPDATER_PUBLIC_KEY_PLACEHOLDER: &str = "REPLACE_WITH_TICKET_04_PUBLIC_KEY";

struct SettingsPath(PathBuf);
struct ModelsDir(PathBuf);
struct Quitting(AtomicBool);
struct PromptingUpdate(AtomicBool);
struct InstallingUpdate(AtomicBool);
struct UpdateStartGate(Mutex<()>);
struct Secrets(Box<dyn SecretStore>);
struct Http(Arc<dyn HttpClient>);
struct Managed {
    keys: Arc<ManagedKeyState>,
}

#[derive(serde::Serialize)]
struct AppInfo {
    version: String,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
fn get_diagnostics(app: AppHandle) -> Result<String, String> {
    let system = sysinfo::System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    let log = fs::read_to_string(log_dir.join("meeting-minutes.log")).unwrap_or_default();
    let recent_log_lines = log.lines().map(str::to_owned).collect();
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    Ok(build_diagnostics(&DiagnosticsInput {
        version: app.package_info().version.to_string(),
        windows_version: sysinfo::System::long_os_version().unwrap_or_else(|| "Unknown".into()),
        cpu: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_owned())
            .unwrap_or_else(|| "Unknown".into()),
        ram_mb: system.total_memory() / (1024 * 1024),
        audio_devices: audio_devices(),
        selected_model: MODELS
            .iter()
            .find(|model| model.name == settings.model)
            .map(|model| model.name.to_owned()),
        provider: settings.ai.provider,
        whisper_system_info: Some(WhisperEngine::system_info().to_owned()),
        transcription_backend: Some(WhisperEngine::backend_description()),
        recent_log_lines,
    }))
}

#[tauri::command]
fn get_microphone_usage() -> Result<Vec<MicrophoneApp>, String> {
    SystemMicrophoneUsage
        .apps_using_microphone()
        .map_err(|error| error.to_string())
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SummaryJobState {
    Running,
    Failed,
}
#[derive(Clone, serde::Serialize)]
struct SummaryJob {
    meeting_id: String,
    state: SummaryJobState,
    done: usize,
    total: usize,
    error: Option<String>,
    #[serde(skip)]
    cancel: Arc<AtomicBool>,
}
#[derive(serde::Serialize)]
struct SummaryStatus {
    jobs: Vec<SummaryJob>,
}
#[derive(serde::Serialize)]
struct SendPlan {
    provider: ProviderKind,
    cloud: bool,
    consented: bool,
    disclosure: Option<ai::Disclosure>,
}
struct ActiveAsk {
    ask_id: String,
    partial: String,
    cancel: Arc<AtomicBool>,
}
#[derive(serde::Serialize)]
struct ActiveAskView {
    ask_id: String,
    partial: String,
}
#[derive(serde::Serialize)]
struct AskView {
    asks: Vec<ask::AskEntry>,
    active: Option<ActiveAskView>,
    retryable_id: Option<String>,
}
#[derive(Clone, serde::Serialize)]
struct AskDelta {
    meeting_id: String,
    ask_id: String,
    text: String,
}
#[derive(Clone, serde::Serialize)]
struct AsksChanged {
    meeting_id: String,
}
#[derive(Default)]
struct AiJobsInner {
    summaries: HashMap<String, SummaryJob>,
    asks: HashMap<String, ActiveAsk>,
}
type AiJobs = Mutex<AiJobsInner>;

#[cfg(windows)]
struct KeyringStore;
#[cfg(windows)]
impl KeyringStore {
    fn entry(kind: ProviderKind) -> Result<keyring::Entry, String> {
        let user = match kind {
            ProviderKind::Openai => "openai-api-key",
            ProviderKind::Anthropic => "anthropic-api-key",
            ProviderKind::Groq => "groq-api-key",
            ProviderKind::Gemini => "gemini-api-key",
            ProviderKind::Ollama => return Err("Ollama needs no API key".into()),
        };
        keyring::Entry::new("Meeting Minutes", user).map_err(|error| error.to_string())
    }

    fn managed_entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new("Meeting Minutes", "managed-key").map_err(|error| error.to_string())
    }
}
#[cfg(windows)]
impl SecretStore for KeyringStore {
    fn get(&self, kind: ProviderKind) -> Result<Option<ApiKey>, String> {
        if kind == ProviderKind::Ollama {
            return Ok(None);
        }
        match Self::entry(kind)?.get_password() {
            Ok(key) => Ok(ApiKey::new(&key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
    fn set(&self, kind: ProviderKind, key: &ApiKey) -> Result<(), String> {
        Self::entry(kind)?
            .set_password(key.expose())
            .map_err(|error| error.to_string())
    }
    fn delete(&self, kind: ProviderKind) -> Result<(), String> {
        match Self::entry(kind)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(windows)]
impl ManagedKeyStore for KeyringStore {
    fn get(&self) -> Result<Option<ManagedKey>, String> {
        match Self::managed_entry()?.get_password() {
            Ok(key) => Ok(ManagedKey::new(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn set(&self, key: &ManagedKey) -> Result<(), String> {
        Self::managed_entry()?
            .set_password(key.expose())
            .map_err(|error| error.to_string())
    }

    fn delete(&self) -> Result<(), String> {
        match Self::managed_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[derive(Default)]
struct ImportState {
    queue: VecDeque<PathBuf>,
    current: Option<String>,
    errors: Vec<String>,
    running: bool,
}

type ImportQueue = Mutex<ImportState>;
struct ImportCancel(AtomicBool);

#[derive(Clone, serde::Serialize)]
struct ImportStatus {
    current: Option<String>,
    queued: usize,
    errors: Vec<String>,
}

fn import_snapshot(state: &ImportState) -> ImportStatus {
    ImportStatus {
        current: state.current.clone(),
        queued: state.queue.len(),
        errors: state.errors.clone(),
    }
}

fn publish_import(app: &AppHandle) -> Result<ImportStatus, String> {
    let state = app.state::<ImportQueue>();
    let state = state.lock().map_err(|e| e.to_string())?;
    let status = import_snapshot(&state);
    drop(state);
    let _ = app.emit("import-status", &status);
    Ok(status)
}

fn queue_imports(app: &AppHandle, paths: Vec<PathBuf>) -> Result<ImportStatus, String> {
    let _update_gate = lock_update_start_gate(app)?;
    ensure_update_not_active(app)?;
    let queue = app.state::<ImportQueue>();
    let mut state = queue.lock().map_err(|e| e.to_string())?;
    for path in paths {
        if import_extension(&path).is_some() {
            state.queue.push_back(path);
        } else {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            state
                .errors
                .push(format!("{name}: not an mp3, m4a, wav or flac file"));
        }
    }
    let start = !state.running
        && !state.queue.is_empty()
        && !app.state::<Quitting>().0.load(Ordering::SeqCst);
    if start {
        state.running = true;
    }
    drop(state);
    let status = publish_import(app)?;
    if start {
        import_worker(app.clone());
    }
    Ok(status)
}

fn import_worker(app: AppHandle) {
    thread::spawn(move || loop {
        let path = {
            let state = app.state::<ImportQueue>();
            let Ok(mut state) = state.lock() else {
                return;
            };
            if app.state::<Quitting>().0.load(Ordering::SeqCst) {
                state.queue.clear();
            }
            let next = state.queue.pop_front();
            state.current = next
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned());
            if next.is_none() {
                state.running = false;
            }
            next
        };
        let _ = publish_import(&app);
        let Some(path) = path else {
            return;
        };
        let result = (|| -> Result<MeetingSummary, String> {
            let root = {
                let library = app.state::<LibraryState>();
                let library = library.lock().map_err(|e| e.to_string())?;
                library.as_ref().map_err(Clone::clone)?.root().to_path_buf()
            };
            let now = chrono::Local::now().fixed_offset();
            let language = Settings::load(&app.state::<SettingsPath>().0).language;
            let prepared = Library::prepare_import_meeting_with_language(
                &root,
                &path,
                now,
                language,
                &app.state::<ImportCancel>().0,
            )
            .map_err(|e| e.to_string())?;
            let library = app.state::<LibraryState>();
            let mut library = library.lock().map_err(|e| e.to_string())?;
            library
                .as_mut()
                .map_err(|e| e.clone())?
                .publish_import_meeting(prepared, &app.state::<ImportCancel>().0)
                .map_err(|e| e.to_string())
        })();
        match result {
            Ok(meeting) => {
                if !app.state::<Quitting>().0.load(Ordering::SeqCst)
                    && Settings::load(&app.state::<SettingsPath>().0).auto_process
                {
                    if let Ok(_gate) = lock_update_start_gate(&app) {
                        if ensure_update_not_active(&app).is_ok() {
                            if let Ok(mut processor) = lock_processor(&app) {
                                processor.enqueue(&meeting.id);
                            }
                        }
                    }
                    let _ = publish_processing(&app);
                }
                let _ = app.emit("library-changed", ());
            }
            Err(error) => {
                if !app.state::<Quitting>().0.load(Ordering::SeqCst) {
                    if let Ok(mut state) = app.state::<ImportQueue>().lock() {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        state.errors.push(format!("{name}: {error}"));
                    }
                }
            }
        }
        if let Ok(mut state) = app.state::<ImportQueue>().lock() {
            state.current = None;
        }
        let _ = publish_import(&app);
    });
}

#[tauri::command]
async fn pick_audio_files(app: AppHandle) -> Result<ImportStatus, String> {
    let paths = app
        .dialog()
        .file()
        .add_filter("Audio", &["mp3", "m4a", "wav", "flac"])
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.into_path().map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    queue_imports(&app, paths)
}

#[tauri::command]
fn import_status(app: AppHandle) -> Result<ImportStatus, String> {
    publish_import(&app)
}

#[tauri::command]
fn dismiss_import_errors(app: AppHandle) -> Result<ImportStatus, String> {
    app.state::<ImportQueue>()
        .lock()
        .map_err(|e| e.to_string())?
        .errors
        .clear();
    publish_import(&app)
}

fn lock_processor(app: &AppHandle) -> Result<std::sync::MutexGuard<'_, Processor>, String> {
    app.state::<ProcessorState>()
        .inner()
        .lock()
        .map_err(|e| e.to_string())
}

fn lock_update_start_gate(app: &AppHandle) -> Result<std::sync::MutexGuard<'_, ()>, String> {
    app.state::<UpdateStartGate>()
        .inner()
        .0
        .lock()
        .map_err(|error| error.to_string())
}

fn meeting_busy(app: &AppHandle, meeting_id: &str) -> Result<(), String> {
    let processor = lock_processor(app)?;
    meeting_busy_locked(app, &processor, meeting_id)
}

fn meeting_busy_locked(
    app: &AppHandle,
    processor: &Processor,
    meeting_id: &str,
) -> Result<(), String> {
    if processor.is_busy(meeting_id) {
        return Err("This Meeting is being processed".into());
    }
    ai_busy(app, meeting_id)?;
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

fn ai_busy(app: &AppHandle, meeting_id: &str) -> Result<(), String> {
    let state = app.state::<AiJobs>();
    let jobs = state.lock().map_err(|error| error.to_string())?;
    if jobs
        .summaries
        .get(meeting_id)
        .is_some_and(|job| matches!(job.state, SummaryJobState::Running))
    {
        return Err("A Summary is being made for this Meeting".into());
    }
    if jobs.asks.contains_key(meeting_id) {
        return Err("An Ask is running for this Meeting".into());
    }
    Ok(())
}

fn publish_processing(app: &AppHandle) -> Result<ProcessingStatus, String> {
    let status = lock_processor(app)?.status();
    let _ = app.emit("processing-status", &status);
    Ok(status)
}

fn processing_stage(app: &AppHandle, id: &str, stage: JobStage, progress: f32) {
    if progress == 0.0 {
        match stage {
            JobStage::Downloading => log_event(LogEvent::ProcessingDownloading),
            JobStage::Transcribing => log_event(LogEvent::ProcessingTranscribing),
            _ => {}
        }
    }
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
fn transcript_draft(
    state: State<'_, DraftStore>,
    meeting_id: String,
) -> Result<Vec<meeting_minutes_core::transcript::PlacedSegment>, String> {
    let draft = state.lock().map_err(|error| error.to_string())?;
    Ok(if draft.meeting_id == meeting_id {
        draft.segments.clone()
    } else {
        Vec::new()
    })
}

#[tauri::command]
fn transcribe_meeting(
    app: AppHandle,
    meeting_id: String,
    model: Option<String>,
) -> Result<ProcessingStatus, String> {
    let _update_gate = lock_update_start_gate(&app)?;
    ensure_update_not_active(&app)?;
    if let Some(name) = model.as_deref() {
        let settings = Settings::load(&app.state::<SettingsPath>().0);
        let valid = if let Some(local) = name.strip_prefix("local:") {
            MODELS.iter().any(|candidate| candidate.name == local)
        } else if let Some(cloud) = name.strip_prefix("groq:") {
            settings
                .speech
                .models
                .iter()
                .any(|entry| entry.provider == SpeechProvider::Groq && entry.model == cloud)
                && app.state::<Secrets>().0.get(ProviderKind::Groq)?.is_some()
        } else if let Some(cloud) = name.strip_prefix("gemini:") {
            settings
                .speech
                .models
                .iter()
                .any(|entry| entry.provider == SpeechProvider::Gemini && entry.model == cloud)
                && app
                    .state::<Secrets>()
                    .0
                    .get(ProviderKind::Gemini)?
                    .is_some()
        } else if name == "managed" {
            let managed = app.state::<Managed>();
            matches!(managed.keys.store().get(), Ok(Some(_)))
        } else {
            MODELS.iter().any(|candidate| candidate.name == name)
        };
        if !valid {
            return Err(format!("Speech-to-text model is not ready: {name}"));
        }
    }
    let mut processor = lock_processor(&app)?;
    ai_busy(&app, &meeting_id)?;
    {
        let library_state = app.state::<LibraryState>();
        let library = library_state.lock().map_err(|e| e.to_string())?;
        library
            .as_ref()
            .map_err(Clone::clone)?
            .meeting(&meeting_id)
            .map_err(|e| e.to_string())?;
    }
    processor.enqueue_with_model(&meeting_id, model.as_deref());
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
) -> Result<Option<UiTranscript>, String> {
    let library = state.lock().map_err(|e| e.to_string())?;
    library
        .as_ref()
        .map_err(Clone::clone)?
        .transcript(&meeting_id)
        .map(|transcript| transcript.map(UiTranscript::from))
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct UiTranscript {
    #[serde(flatten)]
    transcript: Transcript,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_reset_date: Option<String>,
}

impl From<Transcript> for UiTranscript {
    fn from(transcript: Transcript) -> Self {
        let fallback_kind = transcript
            .fallback_reason
            .as_ref()
            .map(|reason| reason.kind());
        let fallback_reset_date = transcript
            .fallback_reason
            .as_ref()
            .and_then(|reason| reason.reset_date().map(str::to_owned));
        Self {
            transcript,
            fallback_kind,
            fallback_reset_date,
        }
    }
}

#[tauri::command]
fn edit_segment(
    app: AppHandle,
    meeting_id: String,
    revision: u32,
    segment_id: String,
    text: String,
) -> Result<UiTranscript, String> {
    // Held through the edit: no transcription of this Meeting can start and publish a newer Revision meanwhile.
    let processor = lock_processor(&app)?;
    meeting_busy_locked(&app, &processor, &meeting_id)?;
    let library_state = app.state::<LibraryState>();
    let mut library = library_state.lock().map_err(|e| e.to_string())?;
    let transcript = library
        .as_mut()
        .map_err(|e| e.clone())?
        .edit_segment(&meeting_id, revision, &segment_id, &text)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("library-changed", ());
    Ok(UiTranscript::from(transcript))
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
fn get_settings(path: State<'_, SettingsPath>, secrets: State<'_, Secrets>) -> Settings {
    let mut settings = Settings::load(&path.0);
    if settings.language_models.legacy_correction_pending {
        if let Ok(key) = secrets.0.get(ProviderKind::Gemini) {
            settings
                .language_models
                .finish_legacy_correction(key.is_some(), &settings.ai);
            let _ = settings.save(&path.0);
        }
    }
    settings
}

#[derive(serde::Serialize)]
struct FirstRunInfo {
    discrete_gpu: bool,
    library_location: String,
}

#[tauri::command]
fn first_run_info(app: AppHandle) -> Result<FirstRunInfo, String> {
    let location = {
        let state = app.state::<LibraryState>();
        let guard = state.lock().map_err(|error| error.to_string())?;
        guard.as_ref().map_err(Clone::clone)?.root().to_path_buf()
    };
    Ok(FirstRunInfo {
        discrete_gpu: WhisperEngine::discrete_gpu_available(),
        library_location: location.display().to_string(),
    })
}

#[tauri::command]
fn complete_first_run(path: State<'_, SettingsPath>) -> Result<(), String> {
    let mut settings = Settings::load(&path.0);
    settings.first_run_completed = true;
    settings.save(&path.0).map_err(|error| error.to_string())
}

// Tauri maps each argument to a field of the invoke payload, so the count follows the Settings form.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn update_settings(
    app: AppHandle,
    path: State<'_, SettingsPath>,
    auto_process: bool,
    model: String,
    vocabulary: Vec<String>,
    language: Option<Language>,
    your_name: Option<String>,
    speech: Option<SpeechSettings>,
) -> Result<Settings, String> {
    let mut settings = Settings::load(&path.0);
    settings.auto_process = auto_process;
    settings.model = model;
    if let Some(speech) = speech {
        speech.validate()?;
        settings.model = speech.automatic_whisper.clone();
        settings.speech = speech;
    } else {
        settings.speech.automatic_whisper = settings.model.clone();
    }
    settings.vocabulary = vocabulary;
    settings.language = language;
    settings.your_name = your_name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());
    settings.save(&path.0).map_err(|e| e.to_string())?;
    if let Ok(mut guard) = app.state::<LibraryState>().lock() {
        if let Ok(library) = guard.as_mut() {
            library
                .set_your_name(settings.your_name.as_deref())
                .map_err(|error| error.to_string())?;
        }
    }
    let _ = app.emit("library-changed", ());
    Ok(settings)
}

#[tauri::command]
fn update_snapshot_hotkeys(
    path: State<'_, SettingsPath>,
    snapshot_hotkey: String,
    draw_snapshot_hotkey: String,
) -> Result<Settings, String> {
    let repeat = snapshot_hotkey
        .parse::<Shortcut>()
        .map_err(|error| format!("Invalid Snapshot hotkey: {error}"))?;
    let draw = draw_snapshot_hotkey
        .parse::<Shortcut>()
        .map_err(|error| format!("Invalid Capture region hotkey: {error}"))?;
    if repeat == draw {
        return Err("Snapshot hotkeys need different combinations".into());
    }
    if [RECORD_SHORTCUT, PAUSE_SHORTCUT].iter().any(|fixed| {
        let fixed = fixed.parse::<Shortcut>().expect("valid recording hotkey");
        repeat == fixed || draw == fixed
    }) {
        return Err("A Snapshot hotkey is already used by a recording control".into());
    }
    let mut settings = Settings::load(&path.0);
    settings.snapshot_hotkey = snapshot_hotkey;
    settings.draw_snapshot_hotkey = draw_snapshot_hotkey;
    settings.save(&path.0).map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
fn installed_models(models_dir: State<'_, ModelsDir>) -> Vec<&'static str> {
    MODELS
        .iter()
        .filter(|info| {
            models::is_installed(&models_dir.0, info)
                && models::is_installed(&models_dir.0, &models::VAD_MODEL)
        })
        .map(|info| info.name)
        .collect()
}

#[tauri::command]
fn model_download_status(app: AppHandle) -> Vec<ModelDownloadStatus> {
    app.state::<ModelDownloads>().status()
}

#[tauri::command]
fn start_model_download(
    app: AppHandle,
    model_id: String,
) -> Result<Vec<ModelDownloadStatus>, String> {
    let downloads = ModelDownloads::clone(app.state::<ModelDownloads>().inner());
    if downloads.start(&model_id)? {
        let statuses = downloads.status();
        if let Some(status) = statuses.iter().find(|status| status.model_id == model_id) {
            let _ = app.emit("model-download-status", status);
        }
        let app_for_thread = app.clone();
        thread::spawn(move || {
            let _ = downloads.run(&model_id, &|status| {
                let _ = app_for_thread.emit("model-download-status", status);
            });
        });
        return Ok(statuses);
    }
    Ok(downloads.status())
}

#[tauri::command]
fn cancel_model_download(
    app: AppHandle,
    model_id: String,
) -> Result<Vec<ModelDownloadStatus>, String> {
    let downloads = app.state::<ModelDownloads>();
    downloads.cancel(&model_id)?;
    Ok(downloads.status())
}

#[tauri::command]
fn delete_model_download(
    app: AppHandle,
    model_id: String,
) -> Result<Vec<ModelDownloadStatus>, String> {
    let downloads = app.state::<ModelDownloads>();
    downloads.delete(&model_id)?;
    let statuses = downloads.status();
    if let Some(status) = statuses.iter().find(|status| status.model_id == model_id) {
        let _ = app.emit("model-download-status", status);
    }
    Ok(statuses)
}

fn current_ai_status(path: &SettingsPath, secrets: &Secrets) -> AiStatus {
    let settings = Settings::load(&path.0);
    let mut status = ai::ai_status(&settings.ai, secrets.0.as_ref());
    if let Some(model) = &settings.language_models.summary {
        status.summary_provider = Some(model.provider);
        status.summary_model = Some(model.model.clone());
        status.summary_configured = ai::model_ready(model, secrets.0.as_ref());
    }
    if let Some(model) = &settings.language_models.ask {
        status.ask_provider = Some(model.provider);
        status.ask_model = Some(model.model.clone());
        status.ask_configured = ai::model_ready(model, secrets.0.as_ref());
    }
    status
}

#[tauri::command]
fn ai_status(
    path: State<'_, SettingsPath>,
    secrets: State<'_, Secrets>,
) -> Result<AiStatus, String> {
    Ok(current_ai_status(&path, &secrets))
}

#[derive(serde::Serialize)]
struct ManagedStatus {
    connected: bool,
    revoked: bool,
    email: Option<String>,
    enabled: bool,
    remaining_minutes: Option<f64>,
}

fn current_managed_status(managed: &Managed) -> Result<ManagedStatus, String> {
    let connected = managed.keys.store().get()?.is_some();
    Ok(ManagedStatus {
        connected,
        revoked: managed.keys.is_invalid(),
        email: None,
        enabled: connected,
        remaining_minutes: managed.keys.remaining_minutes(),
    })
}

#[tauri::command]
fn managed_status(managed: State<'_, Managed>) -> Result<ManagedStatus, String> {
    current_managed_status(&managed)
}

#[tauri::command]
async fn managed_save_key(app: AppHandle, key: String) -> Result<ManagedStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let key = ManagedKey::new(key.trim()).ok_or("Enter a Managed key.")?;
        let managed = app.state::<Managed>();
        managed.keys.store().set(&key)?;
        current_managed_status(&managed)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn managed_remove_key(app: AppHandle) -> Result<ManagedStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let managed = app.state::<Managed>();
        managed.keys.store().delete()?;
        current_managed_status(&managed)
    })
    .await
    .map_err(|error| error.to_string())?
}

// Keep these command names available for older callers while Account access is deferred.
#[tauri::command]
async fn managed_disconnect(app: AppHandle) -> Result<ManagedStatus, String> {
    managed_remove_key(app).await
}

#[tauri::command]
async fn managed_set_enabled(app: AppHandle, enabled: bool) -> Result<ManagedStatus, String> {
    let managed = app.state::<Managed>();
    if enabled && managed.keys.store().get()?.is_none() {
        return Err("Save a Managed key first.".into());
    }
    current_managed_status(&managed)
}

#[tauri::command]
async fn managed_account(_app: AppHandle) -> Result<managed::AccountStatus, String> {
    Err("Account access is deferred. Save a Managed key instead.".into())
}

#[tauri::command]
async fn managed_send_code(_app: AppHandle, _email: String) -> Result<(), String> {
    Err("Account sign-in is deferred. Save a Managed key instead.".into())
}

#[tauri::command]
async fn managed_sign_in(
    _app: AppHandle,
    _email: String,
    _code: String,
) -> Result<ManagedStatus, String> {
    Err("Account sign-in is deferred. Save a Managed key instead.".into())
}

#[tauri::command]
async fn managed_delete_account(_app: AppHandle) -> Result<ManagedStatus, String> {
    Err("Account deletion is deferred while Accounts are unavailable.".into())
}

#[tauri::command]
async fn list_ai_models(app: AppHandle, provider: ProviderKind) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let secrets = app.state::<Secrets>();
        let key = secrets.0.get(provider)?;
        let http = app.state::<Http>();
        ai::list_models(http.0.as_ref(), provider, key.as_ref()).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn test_cloud_api_key(app: AppHandle, provider: ProviderKind) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let secrets = app.state::<Secrets>();
        let key = secrets
            .0
            .get(provider)?
            .ok_or_else(|| format!("Add your {} API key in Settings", provider.label()))?;
        let http = app.state::<Http>();
        ai::test_cloud_key(http.0.as_ref(), provider, &key).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn test_speech_model(
    app: AppHandle,
    provider: SpeechProvider,
    model: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let kind = match provider {
            SpeechProvider::Groq => ProviderKind::Groq,
            SpeechProvider::Gemini => ProviderKind::Gemini,
        };
        let key = app
            .state::<Secrets>()
            .0
            .get(kind)?
            .ok_or_else(|| format!("Add your {} key in Keys", kind.label()))?;
        let http = app.state::<Http>();
        ai::test_cloud_key(http.0.as_ref(), kind, &key).map_err(|error| error.to_string())?;
        if provider == SpeechProvider::Groq {
            if !meeting_minutes_core::speech_models::GROQ_MODELS.contains(&model.as_str()) {
                return Err("Choose a supported Groq Speech-to-text model".into());
            }
        } else {
            let models = ai::list_models(http.0.as_ref(), kind, Some(&key))
                .map_err(|error| error.to_string())?;
            if !models.iter().any(|listed| {
                listed == &model || listed.strip_prefix("models/") == Some(model.as_str())
            }) {
                return Err("Gemini did not list that model".into());
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn acknowledge_cloud_notice(path: State<'_, SettingsPath>) -> Result<Settings, String> {
    let mut settings = Settings::load(&path.0);
    settings.cloud_notice_acknowledged = true;
    settings.save(&path.0).map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
fn save_api_key(
    path: State<'_, SettingsPath>,
    secrets: State<'_, Secrets>,
    provider: ProviderKind,
    key: String,
) -> Result<AiStatus, String> {
    if !provider.is_cloud() {
        return Err("Ollama needs no API key".into());
    }
    let key = ApiKey::new(&key).ok_or("Enter an API key")?;
    secrets.0.set(provider, &key)?;
    Ok(current_ai_status(&path, &secrets))
}

#[tauri::command]
fn delete_api_key(
    path: State<'_, SettingsPath>,
    secrets: State<'_, Secrets>,
    provider: ProviderKind,
) -> Result<AiStatus, String> {
    if !provider.is_cloud() {
        return Err("Ollama needs no API key".into());
    }
    secrets.0.delete(provider)?;
    Ok(current_ai_status(&path, &secrets))
}

#[tauri::command]
fn update_ai_settings(
    path: State<'_, SettingsPath>,
    secrets: State<'_, Secrets>,
    provider: Option<ProviderKind>,
    model: Option<String>,
) -> Result<AiStatus, String> {
    let mut settings = Settings::load(&path.0);
    settings.ai.provider = provider;
    if let (Some(provider), Some(model)) = (provider, model) {
        settings.ai.set_model(provider, model);
    }
    settings.save(&path.0).map_err(|error| error.to_string())?;
    Ok(ai::ai_status(&settings.ai, secrets.0.as_ref()))
}

#[tauri::command]
fn update_language_models(
    path: State<'_, SettingsPath>,
    language_models: LanguageModelSettings,
) -> Result<Settings, String> {
    if !language_models.valid() {
        return Err("Choose a saved Language model and confirm cloud Correction first".into());
    }
    let mut settings = Settings::load(&path.0);
    settings.language_models = language_models;
    settings.save(&path.0).map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
async fn test_language_model(app: AppHandle, model: LanguageModel) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = Settings::load(&app.state::<SettingsPath>().0);
        if !settings.language_models.models.contains(&model) {
            return Err("Add this Language model before testing it".into());
        }
        let provider = ai::configured_language_provider(
            &model,
            app.state::<Secrets>().0.as_ref(),
            app.state::<Http>().0.clone(),
        )
        .map_err(|error| error.to_string())?;
        let request = GenerateRequest {
            system: "You are a connection test.".into(),
            messages: vec![Message {
                role: Role::User,
                content: "Reply with the single word: ready".into(),
            }],
            schema: None,
            max_tokens: 1024,
            inline_audio: None,
        };
        provider
            .generate(&request, &mut |_| {}, &AtomicBool::new(false))
            .map(|reply| reply.trim().to_owned())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn test_ai_provider(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = Settings::load(&app.state::<SettingsPath>().0);
        let secrets = app.state::<Secrets>();
        let http = app.state::<Http>();
        let provider = ai::configured_provider(&settings.ai, secrets.0.as_ref(), http.0.clone())
            .map_err(|error| error.to_string())?;
        let request = GenerateRequest {
            system: "You are a connection test.".into(),
            messages: vec![Message {
                role: Role::User,
                content: "Reply with the single word: ready".into(),
            }],
            schema: None,
            // Reasoning models spend output tokens on thinking before the reply.
            max_tokens: 1024,
            inline_audio: None,
        };
        let answer = provider
            .generate(&request, &mut |_| {}, &AtomicBool::new(false))
            .map_err(|error| error.to_string())?;
        Ok(answer.trim().to_owned())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn configured_ai(app: &AppHandle, use_for: &str) -> Result<ai::HttpProvider, String> {
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    let secrets = app.state::<Secrets>();
    let http = app.state::<Http>();
    let selected = match use_for {
        "summary" => settings.language_models.summary.as_ref(),
        "ask" => settings.language_models.ask.as_ref(),
        _ => None,
    }
    .ok_or("No Language model is set up")?;
    ai::configured_language_provider(selected, secrets.0.as_ref(), http.0.clone())
        .map_err(|error| error.to_string())
}

fn selected_ai(
    app: &AppHandle,
    use_for: &str,
    override_model: Option<&LanguageModel>,
) -> Result<ai::HttpProvider, String> {
    let Some(model) = override_model else {
        return configured_ai(app, use_for);
    };
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    if !settings.language_models.models.contains(model) {
        return Err("This Language model is not in Settings".into());
    }
    let secrets = app.state::<Secrets>();
    let http = app.state::<Http>();
    ai::configured_language_provider(model, secrets.0.as_ref(), http.0.clone())
        .map_err(|error| error.to_string())
}

fn meeting_context(
    app: &AppHandle,
    id: &str,
) -> Result<
    (
        MeetingDetail,
        Transcript,
        meeting_minutes_core::participant::Participants,
        PathBuf,
    ),
    String,
> {
    let state = app.state::<LibraryState>();
    let guard = state.lock().map_err(|error| error.to_string())?;
    let library = guard.as_ref().map_err(Clone::clone)?;
    let detail = library.meeting(id).map_err(|error| error.to_string())?;
    let transcript = library
        .transcript(id)
        .map_err(|error| error.to_string())?
        .ok_or("Transcribe this Meeting first")?;
    let participants = library
        .participants(id)
        .map_err(|error| error.to_string())?;
    Ok((detail, transcript, participants, library.meeting_dir(id)))
}

fn cloud_consented(app: &AppHandle, id: &str, kind: ProviderKind) -> Result<bool, String> {
    let state = app.state::<LibraryState>();
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_ref()
        .map_err(Clone::clone)?
        .cloud_consent(id, kind)
        .map_err(|error| error.to_string())
}

fn require_cloud_consent(
    app: &AppHandle,
    id: &str,
    kind: ProviderKind,
    consent: bool,
) -> Result<(), String> {
    if !kind.is_cloud() || cloud_consented(app, id, kind)? {
        return Ok(());
    }
    if !consent {
        return Err("Review what will be sent first".into());
    }
    let state = app.state::<LibraryState>();
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_ref()
        .map_err(Clone::clone)?
        .record_cloud_consent(id, kind, chrono::Local::now().fixed_offset())
        .map_err(|error| error.to_string())
}

fn summary_snapshot(app: &AppHandle) -> Result<SummaryStatus, String> {
    let jobs = app.state::<AiJobs>();
    let jobs = jobs.lock().map_err(|error| error.to_string())?;
    Ok(SummaryStatus {
        jobs: jobs.summaries.values().cloned().collect(),
    })
}
fn publish_summary(app: &AppHandle) -> Result<SummaryStatus, String> {
    let status = summary_snapshot(app)?;
    let _ = app.emit("summary-status", &status);
    Ok(status)
}

#[tauri::command]
fn meeting_summary(app: AppHandle, meeting_id: String) -> Result<Option<Summary>, String> {
    let state = app.state::<LibraryState>();
    let guard = state.lock().map_err(|error| error.to_string())?;
    let library = guard.as_ref().map_err(Clone::clone)?;
    library
        .meeting(&meeting_id)
        .map_err(|error| error.to_string())?;
    summary::read_summary(&library.meeting_dir(&meeting_id)).map_err(|error| error.to_string())
}

#[tauri::command]
fn summary_participants_changed(app: AppHandle, meeting_id: String) -> Result<bool, String> {
    let state = app.state::<LibraryState>();
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_ref()
        .map_err(|error| error.clone())?
        .summary_participants_changed(&meeting_id)
        .map_err(|error| error.to_string())
}
#[tauri::command]
fn summary_status(app: AppHandle) -> Result<SummaryStatus, String> {
    summary_snapshot(&app)
}
#[tauri::command]
fn summary_send_plan(
    app: AppHandle,
    meeting_id: String,
    model: Option<LanguageModel>,
) -> Result<SendPlan, String> {
    let provider = selected_ai(&app, "summary", model.as_ref())?;
    let (_, transcript, participants, _) = meeting_context(&app, &meeting_id)?;
    let kind = provider.kind();
    let consented = !kind.is_cloud() || cloud_consented(&app, &meeting_id, kind)?;
    let disclosure = if kind.is_cloud() && !consented {
        let requests = summary::summary_requests(
            &transcript,
            &participants,
            &SummaryOptions::for_provider(kind),
        );
        let first = requests.first().ok_or("The Transcript has no Segments")?;
        let mut disclosure = ai::disclosure(&provider, first);
        disclosure.messages = requests
            .iter()
            .flat_map(|request| request.messages.clone())
            .collect();
        disclosure.characters = disclosure.system.chars().count()
            + disclosure
                .messages
                .iter()
                .map(|message| message.content.chars().count())
                .sum::<usize>();
        Some(disclosure)
    } else {
        None
    };
    Ok(SendPlan {
        provider: kind,
        cloud: kind.is_cloud(),
        consented,
        disclosure,
    })
}

fn start_summary(app: &AppHandle, id: String, provider: ai::HttpProvider) -> Result<(), String> {
    let _update_gate = lock_update_start_gate(app)?;
    ensure_update_not_active(app)?;
    let kind = provider.kind();
    {
        let state = app.state::<AiJobs>();
        let mut jobs = state.lock().map_err(|error| error.to_string())?;
        if jobs
            .summaries
            .get(&id)
            .is_some_and(|job| matches!(job.state, SummaryJobState::Running))
        {
            return Err("A Summary is already being made for this Meeting".into());
        }
        jobs.summaries.insert(
            id.clone(),
            SummaryJob {
                meeting_id: id.clone(),
                state: SummaryJobState::Running,
                done: 0,
                total: 0,
                error: None,
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );
    }
    let _ = publish_summary(app);
    log_event(LogEvent::SummaryStarted);
    let app = app.clone();
    thread::spawn(move || {
        let cancel = match app.state::<AiJobs>().lock() {
            Ok(jobs) => jobs.summaries.get(&id).map(|job| job.cancel.clone()),
            Err(_) => None,
        };
        let Some(cancel) = cancel else {
            return;
        };
        let result = (|| -> Result<(), String> {
            let (_detail, transcript, participants, dir) = meeting_context(&app, &id)?;
            let summary = summary::summarize(
                &provider,
                &transcript,
                &participants,
                &SummaryOptions::for_provider(kind),
                &mut |done, total| {
                    if let Ok(mut jobs) = app.state::<AiJobs>().lock() {
                        if let Some(job) = jobs.summaries.get_mut(&id) {
                            job.done = done;
                            job.total = total;
                        }
                    }
                    let _ = publish_summary(&app);
                },
                &cancel,
            )
            .map_err(|error| error.to_string())?;
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelled".into());
            }
            summary::write_summary_for_meeting(&dir, &summary).map_err(|error| error.to_string())
        })();
        log_event(if result.is_ok() {
            LogEvent::SummaryCompleted
        } else {
            LogEvent::SummaryFailed
        });
        if let Ok(mut jobs) = app.state::<AiJobs>().lock() {
            match result {
                Ok(()) => {
                    jobs.summaries.remove(&id);
                }
                Err(ref error) if error == "Cancelled" => {
                    jobs.summaries.remove(&id);
                }
                Err(error) => {
                    if let Some(job) = jobs.summaries.get_mut(&id) {
                        job.state = SummaryJobState::Failed;
                        job.error = Some(error);
                    }
                }
            }
        }
        let _ = publish_summary(&app);
        let _ = app.emit("library-changed", ());
    });
    Ok(())
}

#[tauri::command]
fn generate_summary(
    app: AppHandle,
    meeting_id: String,
    consent: bool,
    model: Option<LanguageModel>,
) -> Result<SummaryStatus, String> {
    meeting_busy(&app, &meeting_id)?;
    let (_, transcript, _, _) = meeting_context(&app, &meeting_id)?;
    if transcript.segments.is_empty() {
        return Err("The Transcript has no Segments".into());
    }
    let provider = selected_ai(&app, "summary", model.as_ref())?;
    require_cloud_consent(&app, &meeting_id, provider.kind(), consent)?;
    start_summary(&app, meeting_id, provider)?;
    summary_snapshot(&app)
}
#[tauri::command]
fn cancel_summary(app: AppHandle, meeting_id: String) -> Result<SummaryStatus, String> {
    if let Ok(jobs) = app.state::<AiJobs>().lock() {
        if let Some(job) = jobs.summaries.get(&meeting_id) {
            job.cancel.store(true, Ordering::SeqCst);
        }
    }
    publish_summary(&app)
}
#[tauri::command]
fn dismiss_summary(app: AppHandle, meeting_id: String) -> Result<SummaryStatus, String> {
    {
        let state = app.state::<AiJobs>();
        let mut jobs = state.lock().map_err(|error| error.to_string())?;
        if jobs
            .summaries
            .get(&meeting_id)
            .is_some_and(|job| matches!(job.state, SummaryJobState::Failed))
        {
            jobs.summaries.remove(&meeting_id);
        }
    }
    publish_summary(&app)
}

fn asks_view(app: &AppHandle, id: &str) -> Result<AskView, String> {
    let dir = {
        let state = app.state::<LibraryState>();
        let guard = state.lock().map_err(|error| error.to_string())?;
        let library = guard.as_ref().map_err(Clone::clone)?;
        library.meeting(id).map_err(|error| error.to_string())?;
        library.meeting_dir(id)
    };
    let state = app.state::<AiJobs>();
    let jobs = state.lock().map_err(|error| error.to_string())?;
    let active = jobs.asks.get(id).map(|ask| ActiveAskView {
        ask_id: ask.ask_id.clone(),
        partial: ask.partial.clone(),
    });
    let mut history = ask::read_asks(&dir).map_err(|error| error.to_string())?;
    if active.is_none() && history.interrupted() {
        ask::write_asks(&dir, &history).map_err(|error| error.to_string())?;
    }
    let retryable_id = history.retryable().map(|entry| entry.id.clone());
    Ok(AskView {
        asks: history.asks,
        active,
        retryable_id,
    })
}
#[tauri::command]
fn meeting_asks(app: AppHandle, meeting_id: String) -> Result<AskView, String> {
    asks_view(&app, &meeting_id)
}

#[tauri::command]
fn ask_send_plan(
    app: AppHandle,
    meeting_id: String,
    question: String,
    model: Option<LanguageModel>,
) -> Result<SendPlan, String> {
    let provider = selected_ai(&app, "ask", model.as_ref())?;
    let (_, transcript, participants, dir) = meeting_context(&app, &meeting_id)?;
    let kind = provider.kind();
    let consented = !kind.is_cloud() || cloud_consented(&app, &meeting_id, kind)?;
    let disclosure = if kind.is_cloud() && !consented {
        let history = ask::read_asks(&dir).map_err(|error| error.to_string())?;
        let request = ask::build_ask(
            &transcript,
            &participants,
            &history,
            &question,
            &AskOptions::for_provider(kind),
        );
        Some(ai::disclosure(&provider, &request.request))
    } else {
        None
    };
    Ok(SendPlan {
        provider: kind,
        cloud: kind.is_cloud(),
        consented,
        disclosure,
    })
}

fn begin_ask(
    app: &AppHandle,
    id: String,
    question: Option<String>,
    consent: bool,
    model: Option<LanguageModel>,
) -> Result<AskView, String> {
    let _update_gate = lock_update_start_gate(app)?;
    ensure_update_not_active(app)?;
    meeting_busy(app, &id)?;
    let (_, transcript, participants, dir) = meeting_context(app, &id)?;
    let provider = selected_ai(app, "ask", model.as_ref())?;
    let kind = provider.kind();
    // A retry uses the saved question. A new Ask uses the text supplied by the click.
    let cancel = Arc::new(AtomicBool::new(false));
    let (ask_id, ask_question, view) = {
        let state = app.state::<AiJobs>();
        let mut jobs = state.lock().map_err(|error| error.to_string())?;
        if jobs.asks.contains_key(&id) {
            return Err(ask::AskError::Busy.to_string());
        }
        let mut history = ask::read_asks(&dir).map_err(|error| error.to_string())?;
        history.interrupted();
        let at = chrono::Local::now().to_rfc3339();
        let ask_id = match question.as_deref() {
            Some(text) => history.begin(text, transcript.revision, kind, provider.model(), &at),
            None => history.begin_retry(transcript.revision, kind, provider.model(), &at),
        }
        .map_err(|error| error.to_string())?;
        let ask_question = history
            .asks
            .last()
            .map(|entry| entry.question.clone())
            .ok_or("Could not save the Ask")?;
        require_cloud_consent(app, &id, kind, consent)?;
        ask::write_asks(&dir, &history).map_err(|error| error.to_string())?;
        jobs.asks.insert(
            id.clone(),
            ActiveAsk {
                ask_id: ask_id.clone(),
                partial: String::new(),
                cancel: cancel.clone(),
            },
        );
        let view = AskView {
            asks: history.asks,
            active: Some(ActiveAskView {
                ask_id: ask_id.clone(),
                partial: String::new(),
            }),
            retryable_id: None,
        };
        (ask_id, ask_question, view)
    };
    let _ = app.emit(
        "asks-changed",
        AsksChanged {
            meeting_id: id.clone(),
        },
    );
    log_event(LogEvent::AskStarted);
    let app = app.clone();
    thread::spawn(move || {
        let outcome = (|| -> Result<ask::AskOutcome, String> {
            let mut history = ask::read_asks(&dir).map_err(|error| error.to_string())?;
            history.asks.retain(|entry| entry.id != ask_id);
            let request = ask::build_ask(
                &transcript,
                &participants,
                &history,
                &ask_question,
                &AskOptions::for_provider(kind),
            );
            Ok(ask::run_ask(
                &provider,
                &request,
                &transcript,
                &mut |text| {
                    if let Ok(mut jobs) = app.state::<AiJobs>().lock() {
                        if let Some(active) = jobs.asks.get_mut(&id) {
                            active.partial = text.to_owned();
                        }
                    }
                    let _ = app.emit(
                        "ask-delta",
                        AskDelta {
                            meeting_id: id.clone(),
                            ask_id: ask_id.clone(),
                            text: text.to_owned(),
                        },
                    );
                },
                &cancel,
            ))
        })();
        log_event(LogEvent::from_answer_state(
            outcome
                .as_ref()
                .map(|result| result.state)
                .unwrap_or(ask::AnswerState::Failed),
        ));
        if let Ok(mut history) = ask::read_asks(&dir) {
            history.finish(
                &ask_id,
                outcome.unwrap_or_else(|error| ask::AskOutcome {
                    state: ask::AnswerState::Failed,
                    answer: String::new(),
                    citations: Vec::new(),
                    invalid_citations: Vec::new(),
                    error: Some(error),
                }),
            );
            let _ = ask::write_asks(&dir, &history);
        }
        if let Ok(mut jobs) = app.state::<AiJobs>().lock() {
            jobs.asks.remove(&id);
        }
        let _ = app.emit("asks-changed", AsksChanged { meeting_id: id });
    });
    Ok(view)
}
#[tauri::command]
fn ask_meeting(
    app: AppHandle,
    meeting_id: String,
    question: String,
    consent: bool,
    model: Option<LanguageModel>,
) -> Result<AskView, String> {
    begin_ask(&app, meeting_id, Some(question), consent, model)
}
#[tauri::command]
fn retry_ask(
    app: AppHandle,
    meeting_id: String,
    consent: bool,
    model: Option<LanguageModel>,
) -> Result<AskView, String> {
    begin_ask(&app, meeting_id, None, consent, model)
}
#[tauri::command]
fn stop_ask(app: AppHandle, meeting_id: String) -> Result<(), String> {
    let state = app.state::<AiJobs>();
    let jobs = state.lock().map_err(|error| error.to_string())?;
    if let Some(active) = jobs.asks.get(&meeting_id) {
        active.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// The Settings Vocabulary plus this Meeting's Participant names.
fn meeting_vocabulary(
    app: &AppHandle,
    id: &str,
    settings: &Settings,
) -> Result<Vec<String>, processing::ProcessingError> {
    let state = app.state::<LibraryState>();
    let guard = state
        .lock()
        .map_err(|error| processing::ProcessingError::Engine(error.to_string()))?;
    let library = guard
        .as_ref()
        .map_err(|error| processing::ProcessingError::Engine(error.clone()))?;
    let participants = library.participants(id)?;
    let mut vocabulary = settings.vocabulary.clone();
    let transcript = library.transcript(id)?;
    for name in participants.vocabulary_names(transcript.as_ref()) {
        if !vocabulary.contains(&name) {
            vocabulary.push(name);
        }
    }
    Ok(vocabulary)
}

fn transcription_chain<'a>(
    app: &AppHandle,
    model: &str,
    chosen: Option<&str>,
    local: Box<dyn FnMut() -> Result<Box<dyn SpeechEngine + 'a>, processing::ProcessingError> + 'a>,
    download_missing_vad: Option<&AtomicBool>,
) -> Result<processing::EngineChain<'a>, processing::ProcessingError> {
    let mut cloud: Vec<Box<dyn CloudEngine>> = Vec::new();
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    let managed = app.state::<Managed>();
    let make_managed = || {
        let mut engine =
            ManagedEngine::new(app.state::<Http>().0.clone(), managed.keys.clone(), None);
        let keys = managed.keys.clone();
        engine.set_enabled_check(move || matches!(keys.store().get(), Ok(Some(_))));
        Box::new(engine) as Box<dyn CloudEngine>
    };
    if let Some(chosen) = chosen {
        let selected: Option<Box<dyn CloudEngine>> = if chosen == "managed" {
            Some(make_managed())
        } else if let Some(name) = chosen.strip_prefix("groq:") {
            let key = app
                .state::<Secrets>()
                .0
                .get(ProviderKind::Groq)
                .map_err(processing::ProcessingError::Engine)?
                .ok_or_else(|| processing::ProcessingError::Engine("Groq key is missing".into()))?;
            Some(Box::new(GroqEngine::new_with_model(
                app.state::<Http>().0.clone(),
                key,
                name,
            )))
        } else if let Some(name) = chosen.strip_prefix("gemini:") {
            let key = app
                .state::<Secrets>()
                .0
                .get(ProviderKind::Gemini)
                .map_err(processing::ProcessingError::Engine)?
                .ok_or_else(|| {
                    processing::ProcessingError::Engine("Gemini key is missing".into())
                })?;
            Some(Box::new(GeminiEngine::new(
                app.state::<Http>().0.clone(),
                key,
                name,
            )))
        } else {
            None
        };
        return match selected {
            Some(engine) => with_cloud_vad(
                app,
                processing::EngineChain::chosen_cloud(engine, model, local),
                download_missing_vad,
            ),
            None => Ok(processing::EngineChain::new(vec![], model, local)),
        };
    }
    if matches!(managed.keys.store().get(), Ok(Some(_))) {
        cloud.push(make_managed());
    }
    // A saved key is the user's choice of cloud transcription (ADR 0011).
    if let Some(key) = app
        .state::<Secrets>()
        .0
        .get(ProviderKind::Groq)
        .map_err(processing::ProcessingError::Engine)?
    {
        if let Some(model) = settings.speech.automatic(SpeechProvider::Groq) {
            cloud.push(Box::new(GroqEngine::new_with_model(
                app.state::<Http>().0.clone(),
                key,
                model,
            )));
        }
    }
    if let Some(key) = app
        .state::<Secrets>()
        .0
        .get(ProviderKind::Gemini)
        .map_err(processing::ProcessingError::Engine)?
    {
        if let Some(model) = settings.speech.automatic(SpeechProvider::Gemini) {
            cloud.push(Box::new(GeminiEngine::new(
                app.state::<Http>().0.clone(),
                key,
                model,
            )));
        }
    }
    let has_cloud = !cloud.is_empty();
    let chain = processing::EngineChain::new(cloud, model, local);
    if has_cloud {
        with_cloud_vad(app, chain, download_missing_vad)
    } else {
        Ok(chain)
    }
}

fn with_cloud_vad<'a>(
    app: &AppHandle,
    mut chain: processing::EngineChain<'a>,
    download_missing_vad: Option<&AtomicBool>,
) -> Result<processing::EngineChain<'a>, processing::ProcessingError> {
    let models_state = app.state::<ModelsDir>();
    let models_dir = &models_state.0;
    if !models::is_installed(models_dir, &models::VAD_MODEL) {
        if let Some(cancel) = download_missing_vad {
            if cancel.load(Ordering::SeqCst) {
                return Err(processing::ProcessingError::Cancelled);
            }
            if let Err(error) = models::download_vad(models_dir, &mut |_| {}, cancel) {
                if matches!(error, processing::ProcessingError::Cancelled)
                    || cancel.load(Ordering::SeqCst)
                {
                    return Err(processing::ProcessingError::Cancelled);
                }
                log_event(LogEvent::VadUnavailable);
                return Ok(chain);
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(processing::ProcessingError::Cancelled);
            }
        }
    }
    if models::is_installed(models_dir, &models::VAD_MODEL) {
        let path = models::model_path(models_dir, &models::VAD_MODEL);
        match WhisperSpeechDetector::load(&path) {
            Ok(detector) => chain.set_speech_detector(Box::new(detector)),
            Err(_) => log_event(LogEvent::VadUnavailable),
        }
    } else {
        log_event(LogEvent::VadUnavailable);
    }
    Ok(chain)
}

/// Starts transcribing the new Recording's Chunks in the background when cloud is configured or
/// local Whisper can run on a discrete GPU. The cache remains in memory until Stop.
fn start_background(app: &AppHandle) {
    let managed = app.state::<Managed>();
    let has_cloud_engine = matches!(managed.keys.key(), Ok(Some(_)))
        || matches!(
            app.state::<Secrets>().0.get(ProviderKind::Groq),
            Ok(Some(_))
        );
    let discrete_gpu = WhisperEngine::discrete_gpu_available();
    let on_ac = processing::PowerState::current() != processing::PowerState::Battery;
    // Cloud availability is learned from Offline errors; the worker retries after its backoff.
    if background::start_decision(has_cloud_engine, discrete_gpu, on_ac)
        == background::BackgroundStart::Disabled
    {
        return;
    }
    let Some(job) = lock_recorder(app)
        .ok()
        .and_then(|recorder| recorder.active_job().cloned())
    else {
        return;
    };
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    let models_dir = app.state::<ModelsDir>().0.clone();
    let Some(model) = MODELS
        .iter()
        .find(|m| m.name == settings.model)
        .or_else(|| MODELS.iter().find(|m| m.name == DEFAULT_MODEL))
        .filter(|model| has_cloud_engine || models::is_installed(&models_dir, model))
    else {
        return;
    };
    let vad_path = models::is_installed(&models_dir, &models::VAD_MODEL)
        .then(|| models::model_path(&models_dir, &models::VAD_MODEL));
    let Ok(vocabulary) = meeting_vocabulary(app, &job.meeting_id, &settings) else {
        return;
    };
    let language = fs::read(job.meeting_dir.join("meeting.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|meeting| meeting["language"].as_str().and_then(Language::from_code));
    let chunk_dir = job
        .meeting_dir
        .join(format!(".chunks/recording-{}", job.number));
    let stop = Arc::new(AtomicBool::new(false));
    let handle = {
        let app = app.clone();
        let stop = Arc::clone(&stop);
        let model_path = models::model_path(&models_dir, model);
        thread::spawn(move || {
            let mut transcriber = background::BackgroundTranscriber::new(
                job.number,
                model.name,
                language,
                processing::initial_prompt(&vocabulary),
            );
            let mut chain = match transcription_chain(
                &app,
                model.name,
                None,
                Box::new(|| {
                    // Never load a second model while Processing has one on the GPU.
                    if lock_processor(&app).map(|p| p.is_running()).unwrap_or(true) {
                        return Err(processing::ProcessingError::Deferred);
                    }
                    if !models::is_installed(&models_dir, model) {
                        return Err(processing::ProcessingError::Deferred);
                    }
                    let engine = WhisperEngine::load_with_vad(&model_path, vad_path.as_deref())
                        .map_err(processing::ProcessingError::Engine)?;
                    if engine.backend() != "Vulkan" {
                        return Err(processing::ProcessingError::Deferred);
                    }
                    Ok(Box::new(engine) as Box<dyn SpeechEngine>)
                }),
                None,
            ) {
                Ok(chain) => chain,
                Err(_) => return Some(transcriber.finish()),
            };
            chain.set_background_local_allowed(Box::new(move || {
                discrete_gpu && processing::PowerState::current() != processing::PowerState::Battery
            }));
            let mut chain = Some(chain);
            let started = std::time::Instant::now();
            let _ = background::run(
                &mut transcriber,
                &mut || chain.take(),
                &chunk_dir,
                background::BackgroundControl {
                    stop: &stop,
                    allowed: &mut || {
                        matches!(
                            background::start_decision(
                                has_cloud_engine,
                                discrete_gpu,
                                processing::PowerState::current()
                                    != processing::PowerState::Battery,
                            ),
                            background::BackgroundStart::Cloud | background::BackgroundStart::Local
                        )
                    },
                    now: &mut || started.elapsed(),
                    wait: &mut || thread::sleep(Duration::from_secs(2)),
                },
                &mut |_| {},
            );
            Some(transcriber.finish())
        })
    };
    if let Ok(mut state) = app.state::<BackgroundState>().lock() {
        state.runs.push(BackgroundRun {
            meeting_id: job.meeting_id,
            stop,
            handle,
        });
    }
}

/// Stops background work for a Meeting's Recordings; the results wait for Processing.
fn stop_background(app: &AppHandle, meeting_id: Option<&str>) {
    if let Ok(state) = app.state::<BackgroundState>().lock() {
        for run in &state.runs {
            if meeting_id.is_none_or(|id| id == run.meeting_id) {
                run.stop.store(true, Ordering::SeqCst);
            }
        }
    }
}

/// Takes what background transcription finished for a Meeting. Processing starts only while no
/// Recording runs, so every run is stopped here and joined, and no background model is left on
/// the GPU when Processing loads its own.
fn take_background_results(app: &AppHandle, meeting_id: &str) -> Vec<background::BackgroundResult> {
    stop_background(app, None);
    let background = app.state::<BackgroundState>();
    let Ok(mut state) = background.lock() else {
        return Vec::new();
    };
    for run in std::mem::take(&mut state.runs) {
        if let Ok(Some(result)) = run.handle.join() {
            state.results.push((run.meeting_id, result));
        }
    }
    let (taken, kept) = std::mem::take(&mut state.results)
        .into_iter()
        .partition::<Vec<_>, _>(|(id, _)| id == meeting_id);
    state.results = kept;
    taken.into_iter().map(|(_, result)| result).collect()
}

fn processing_worker(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(100));
        if app.state::<Quitting>().0.load(Ordering::SeqCst) {
            continue;
        }
        if !recorder_is_idle(&app).unwrap_or(false) {
            continue;
        }
        // The model is read with `next()`, so a rerun's choice cannot leak into this run.
        let (id, job_model) = match lock_processor(&app).ok().and_then(|mut p| {
            let id = p.next()?;
            let model = p.job_model(&id);
            Some((id, model))
        }) {
            Some(job) => job,
            None => continue,
        };
        log_event(LogEvent::ProcessingQueued);
        if app.state::<Quitting>().0.load(Ordering::SeqCst) {
            log_event(LogEvent::processing_result(
                &processing::ProcessingError::Cancelled,
                processing::ProcessingStage::Preparing,
                None,
                None,
                0,
                0,
                Duration::ZERO,
            ));
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
        let mut failure_stage = processing::ProcessingStage::Preparing;
        let mut transcription_started = false;
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
            let local_choice = job_model.as_deref().and_then(|name| {
                name.strip_prefix("local:").or_else(|| {
                    MODELS
                        .iter()
                        .any(|candidate| candidate.name == name)
                        .then_some(name)
                })
            });
            let model = MODELS
                .iter()
                .find(|m| Some(m.name) == local_choice)
                .or_else(|| MODELS.iter().find(|m| m.name == settings.model))
                .unwrap_or_else(|| MODELS.iter().find(|m| m.name == DEFAULT_MODEL).unwrap());
            let downloads = app.state::<ModelDownloads>();
            failure_stage = processing::ProcessingStage::Preparing;
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(processing::ProcessingError::Cancelled);
            }
            processing_stage(&app, &id, JobStage::Transcribing, 0.0);
            let vocabulary = meeting_vocabulary(&app, &id, &settings)?;
            // Joined before the engine loads, so two models never share the GPU.
            let background = take_background_results(&app, &id);
            let cached = if job_model.is_some() {
                Vec::new()
            } else {
                background
            };
            let request = TranscribeRequest {
                meeting_dir: &meeting_dir,
                model: model.name,
                vocabulary: &vocabulary,
            };
            // What the Recording already transcribed is shown before the model loads.
            let early_draft = processing::background_draft(&request, &cached)?;
            if !early_draft.is_empty() {
                emit_draft(&app, &id, &early_draft);
            }
            let mut engine = transcription_chain(
                &app,
                model.name,
                job_model.as_deref(),
                Box::new(|| {
                    if !models::is_installed(downloads.dir(), model)
                        || !models::is_installed(downloads.dir(), &models::VAD_MODEL)
                    {
                        failure_stage = processing::ProcessingStage::Downloading;
                        processing_stage(&app, &id, JobStage::Downloading, 0.0);
                    }
                    let mut last = u32::MAX;
                    let (model_path, vad_path) =
                        downloads.wait_for(model, &cancel, &mut |percent| {
                            if percent != last {
                                last = percent;
                                processing_stage(
                                    &app,
                                    &id,
                                    JobStage::Downloading,
                                    percent as f32 / 100.0,
                                );
                            }
                        })?;
                    let engine = WhisperEngine::load_with_vad(&model_path, Some(&vad_path))
                        .map_err(processing::ProcessingError::Engine)?;
                    Ok(Box::new(engine) as Box<dyn SpeechEngine>)
                }),
                Some(cancel.as_ref()),
            )?;
            let correction = meeting_minutes_core::correction::configured_provider(
                &settings,
                app.state::<Secrets>().0.as_ref(),
                app.state::<Http>().0.clone(),
            );
            let correction = match &correction {
                Ok(Some(provider)) => processing::CorrectionProvider::Ready(provider),
                Ok(None) => processing::CorrectionProvider::Disabled,
                Err(error) => {
                    log_event(LogEvent::correction_failure(error));
                    processing::CorrectionProvider::Unavailable
                }
            };
            let mut last = 0u32;
            transcription_started = true;
            processing::transcribe_meeting_resuming_with_correction(
                &request,
                &mut engine,
                &mut |fraction| {
                    let percent = (fraction * 100.0) as u32;
                    if percent > last {
                        last = percent;
                        processing_stage(&app, &id, JobStage::Transcribing, fraction);
                    }
                },
                &cancel,
                settings.your_name.as_deref(),
                &mut |segments| emit_draft(&app, &id, segments),
                processing::TranscriptionContext {
                    background: &cached,
                    correction,
                },
            )?;
            Ok(())
        })();
        let success = result.is_ok();
        if success {
            log_event(LogEvent::ProcessingCompleted);
        } else if !transcription_started {
            if let Err(error) = &result {
                log_event(LogEvent::processing_result(
                    error,
                    failure_stage,
                    None,
                    None,
                    0,
                    0,
                    Duration::ZERO,
                ));
            }
        }
        if success {
            let library_state = app.state::<LibraryState>();
            if let Ok(mut library) = library_state.lock() {
                if let Ok(library) = library.as_mut() {
                    let _ = library.refresh_transcript(&id);
                }
            }
            let _ = app.emit("library-changed", ());
            let settings = Settings::load(&app.state::<SettingsPath>().0);
            if !app.state::<Quitting>().0.load(Ordering::SeqCst)
                && settings
                    .language_models
                    .summary
                    .as_ref()
                    .is_some_and(|model| model.provider == ProviderKind::Ollama)
            {
                if let Ok(provider) = configured_ai(&app, "summary") {
                    if provider.kind() == ProviderKind::Ollama {
                        let _ = start_summary(&app, id.clone(), provider);
                    }
                }
            }
        }
        clear_draft(&app);
        // Keep the job running until transcript refresh and optional auto-summary have started.
        if let Ok(mut processor) = lock_processor(&app) {
            processor.finished(&id, result);
        }
        let _ = publish_processing(&app);
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
    shown: Mutex<TrayShown>,
}

#[derive(Default)]
struct TrayShown {
    phase: Option<Phase>,
    label: Option<String>,
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
fn search_library(state: State<'_, LibraryState>, query: String) -> Result<Vec<SearchHit>, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_ref()
        .map_err(Clone::clone)?
        .search(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_meeting(app: AppHandle, meeting_id: String) -> Result<(), String> {
    if let Err(error) = meeting_busy(&app, &meeting_id) {
        let recorder = lock_recorder(&app)?;
        if recorder.status().meeting_id.as_deref() == Some(meeting_id.as_str()) {
            return Err("Stop the Recording before deleting this Meeting".into());
        }
        return Err(error);
    }
    {
        let library_state = app.state::<LibraryState>();
        let mut library = library_state.lock().map_err(|error| error.to_string())?;
        library
            .as_mut()
            .map_err(|error| error.clone())?
            .delete_meeting(
                &meeting_id,
                &meeting_minutes_core::recycle_bin::move_to_recycle_bin,
            )
            .map_err(|error| error.to_string())?;
    }
    #[cfg(windows)]
    {
        let undo = app.state::<SnapshotUndoState>();
        if let Ok(mut pending) = undo.lock() {
            pending.remove(&meeting_id);
        };
    }
    lock_recorder(&app)?.dismiss_continue(&meeting_id);
    lock_processor(&app)?.dismiss(&meeting_id);
    let _ = app.emit("library-changed", ());
    let _ = publish(&app);
    let _ = publish_processing(&app);
    Ok(())
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
fn open_meeting_folder(state: State<'_, LibraryState>, meeting_id: String) -> Result<(), String> {
    let folder = {
        let guard = state.lock().map_err(|error| error.to_string())?;
        let library = guard.as_ref().map_err(Clone::clone)?;
        library
            .meeting(&meeting_id)
            .map_err(|error| error.to_string())?;
        library
            .refresh_transcript_export(&meeting_id)
            .map_err(|error| error.to_string())?;
        library.meeting_dir(&meeting_id)
    };
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(folder)
            .spawn()
            .map_err(|error| format!("Couldn't open the Meeting folder: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = folder;
        Err("Opening the Meeting folder is available on Windows".into())
    }
}

#[derive(Clone, serde::Serialize)]
struct SnapshotPayload {
    order: u32,
    timeline_ms: u64,
    caption: Option<String>,
}

#[derive(Clone, serde::Serialize)]
struct SnapshotListPayload {
    snapshots: Vec<SnapshotPayload>,
    total_size_bytes: u64,
}

#[cfg(windows)]
type SnapshotUndoState = Mutex<HashMap<String, meeting_minutes_core::snapshot::DeletedSnapshot>>;

fn snapshot_store(
    state: State<'_, LibraryState>,
    meeting_id: &str,
) -> Result<meeting_minutes_core::snapshot::SnapshotStore, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    let library = guard.as_ref().map_err(Clone::clone)?;
    library
        .meeting(meeting_id)
        .map_err(|error| error.to_string())?;
    Ok(meeting_minutes_core::snapshot::SnapshotStore::new(
        library.meeting_dir(meeting_id),
    ))
}

#[tauri::command]
fn list_snapshots(
    state: State<'_, LibraryState>,
    meeting_id: String,
) -> Result<SnapshotListPayload, String> {
    let store = snapshot_store(state, &meeting_id)?;
    let snapshots = store
        .list()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|snapshot| SnapshotPayload {
            order: snapshot.order,
            timeline_ms: snapshot.timeline_ms,
            caption: snapshot.caption,
        })
        .collect();
    let total_size_bytes = store
        .total_size_bytes()
        .map_err(|error| error.to_string())?;
    Ok(SnapshotListPayload {
        snapshots,
        total_size_bytes,
    })
}

#[tauri::command]
fn read_snapshot_image(
    state: State<'_, LibraryState>,
    meeting_id: String,
    order: u32,
) -> Result<Vec<u8>, String> {
    snapshot_store(state, &meeting_id)?
        .load(order)
        .map(|loaded| loaded.png)
        .map_err(|error| error.to_string())
}

/// Keeps Library search and the readable export in line after a Snapshot is deleted or restored.
#[cfg(windows)]
fn refresh_snapshots(app: &AppHandle, meeting_id: &str) {
    let state = app.state::<LibraryState>();
    if let Ok(mut guard) = state.lock() {
        if let Ok(library) = guard.as_mut() {
            let _ = library.refresh_snapshots(meeting_id);
        }
    };
}

#[cfg(windows)]
#[tauri::command]
fn delete_snapshot(
    app: AppHandle,
    state: State<'_, LibraryState>,
    undo: State<'_, SnapshotUndoState>,
    meeting_id: String,
    order: u32,
) -> Result<(), String> {
    let store = snapshot_store(state, &meeting_id)?;
    let deleted = store
        .delete(
            order,
            &meeting_minutes_core::recycle_bin::move_to_recycle_bin,
        )
        .map_err(|error| error.to_string())?;
    undo.lock()
        .map_err(|error| error.to_string())?
        .insert(meeting_id.clone(), deleted);
    refresh_snapshots(&app, &meeting_id);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[cfg(windows)]
#[tauri::command]
fn undo_snapshot_deletion(
    app: AppHandle,
    state: State<'_, LibraryState>,
    undo: State<'_, SnapshotUndoState>,
    meeting_id: String,
) -> Result<(), String> {
    let store = snapshot_store(state, &meeting_id)?;
    let mut pending = undo.lock().map_err(|error| error.to_string())?;
    let deleted = pending
        .get(&meeting_id)
        .ok_or("No Snapshot deletion to undo")?;
    store
        .undo_delete(deleted)
        .map_err(|error| error.to_string())?;
    pending.remove(&meeting_id);
    drop(pending);
    refresh_snapshots(&app, &meeting_id);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn set_snapshot_caption(
    app: AppHandle,
    state: State<'_, LibraryState>,
    meeting_id: String,
    order: u32,
    caption: Option<String>,
) -> Result<SnapshotPayload, String> {
    let mut guard = state.lock().map_err(|error| error.to_string())?;
    let snapshot = guard
        .as_mut()
        .map_err(|error| error.clone())?
        .set_snapshot_caption(&meeting_id, order, caption.as_deref())
        .map_err(|error| error.to_string())?;
    let _ = app.emit("library-changed", ());
    Ok(SnapshotPayload {
        order: snapshot.order,
        timeline_ms: snapshot.timeline_ms,
        caption: snapshot.caption,
    })
}

#[tauri::command]
fn reveal_snapshot_folder(
    state: State<'_, LibraryState>,
    meeting_id: String,
    order: u32,
) -> Result<(), String> {
    let path = snapshot_store(state, &meeting_id)?
        .list()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|snapshot| snapshot.order == order)
        .ok_or("Snapshot not found")?
        .png_path;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg("/select,")
            .arg(path)
            .spawn()
            .map_err(|error| format!("Couldn't reveal the Snapshot: {error}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("Revealing a Snapshot is available on Windows".into())
    }
}

#[tauri::command]
fn open_library_folder(state: State<'_, LibraryState>) -> Result<(), String> {
    let folder = {
        let guard = state.lock().map_err(|error| error.to_string())?;
        guard.as_ref().map_err(Clone::clone)?.root().to_path_buf()
    };
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(folder)
            .spawn()
            .map_err(|error| format!("Couldn't open the Library folder: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = folder;
        Err("Opening the Library folder is available on Windows".into())
    }
}

/// The app's fixed outside links, shown in Help.
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum OutsideLink {
    Privacy,
    Support,
    Groq,
    Gemini,
    #[serde(rename = "report_problem")]
    ReportProblem,
}

/// Opens an outside link in the default browser or mail app.
#[tauri::command]
fn open_link(link: OutsideLink) -> Result<(), String> {
    let url = match link {
        OutsideLink::Privacy => "https://meeting-minutes.tech/privacy",
        OutsideLink::Support => "mailto:admin@meeting-minutes.tech",
        OutsideLink::Groq => "https://console.groq.com/keys",
        OutsideLink::Gemini => "https://aistudio.google.com/app/apikey",
        OutsideLink::ReportProblem => "https://github.com/ducdv98/meeting-minutes/issues/new?template=bug_report.md&title=Bug%20report%3A%20",
    };
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Couldn't open {url}: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(format!("Opening {url} is available on Windows"))
    }
}

#[tauri::command]
fn rename_meeting(
    app: AppHandle,
    meeting_id: String,
    title: String,
) -> Result<MeetingSummary, String> {
    let summary = {
        let library_state = app.state::<LibraryState>();
        let mut library = library_state.lock().map_err(|error| error.to_string())?;
        let library = library.as_mut().map_err(|error| error.clone())?;
        library
            .rename_meeting(&meeting_id, &title)
            .map_err(|error| error.to_string())?
    };
    let _ = app.emit("library-changed", ());
    let _ = publish(&app);
    Ok(summary)
}

#[tauri::command]
fn set_participant_name(
    app: AppHandle,
    meeting_id: String,
    label: Speaker,
    name: Option<String>,
) -> Result<(), String> {
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    let state = app.state::<LibraryState>();
    let mut guard = state.lock().map_err(|error| error.to_string())?;
    let library = guard.as_mut().map_err(|error| error.clone())?;
    library
        .set_your_name(settings.your_name.as_deref())
        .map_err(|error| error.to_string())?;
    library
        .set_participant_name(&meeting_id, label, name.as_deref())
        .map_err(|error| error.to_string())?;
    drop(guard);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn assign_segments(
    app: AppHandle,
    meeting_id: String,
    revision: u32,
    segment_ids: Vec<String>,
    name: Option<String>,
) -> Result<(), String> {
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    let state = app.state::<LibraryState>();
    let mut guard = state.lock().map_err(|error| error.to_string())?;
    let library = guard.as_mut().map_err(|error| error.clone())?;
    library
        .set_your_name(settings.your_name.as_deref())
        .map_err(|error| error.to_string())?;
    library
        .assign_segments(&meeting_id, revision, &segment_ids, name.as_deref())
        .map_err(|error| error.to_string())?;
    drop(guard);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn rename_participant(
    app: AppHandle,
    meeting_id: String,
    from: String,
    to: String,
) -> Result<(), String> {
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    let state = app.state::<LibraryState>();
    let mut guard = state.lock().map_err(|error| error.to_string())?;
    let library = guard.as_mut().map_err(|error| error.clone())?;
    library
        .set_your_name(settings.your_name.as_deref())
        .map_err(|error| error.to_string())?;
    library
        .rename_participant(&meeting_id, &from, &to)
        .map_err(|error| error.to_string())?;
    drop(guard);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn remove_participant(app: AppHandle, meeting_id: String, name: String) -> Result<(), String> {
    let settings = Settings::load(&app.state::<SettingsPath>().0);
    let state = app.state::<LibraryState>();
    let mut guard = state.lock().map_err(|error| error.to_string())?;
    let library = guard.as_mut().map_err(|error| error.clone())?;
    library
        .set_your_name(settings.your_name.as_deref())
        .map_err(|error| error.to_string())?;
    library
        .remove_participant(&meeting_id, &name)
        .map_err(|error| error.to_string())?;
    drop(guard);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
fn list_participant_names(state: State<'_, LibraryState>) -> Result<Vec<String>, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .as_ref()
        .map_err(Clone::clone)?
        .participant_names()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn recording_status(app: AppHandle) -> Result<RecorderStatus, String> {
    status(&app)
}

// Async commands run off the main thread, so the Vulkan probe in start_background cannot freeze the UI.
#[tauri::command(async)]
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
    let _update_gate = lock_update_start_gate(&app)?;
    ensure_update_not_active(&app)?;
    let job = lock_recorder(&app)?
        .retry_save(&meeting_id)
        .map_err(|error| error.to_string())?;
    spawn_save(app.clone(), job);
    publish(&app)
}

#[tauri::command(async)]
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
    #[cfg(windows)]
    snapshot_windows::recording_changed(app, &status);
    #[cfg(windows)]
    snapshot_windows::capture_bar_changed(app, &status);
    let _ = app.emit("recording-status", &status);
    update_tray(app);
    Ok(status)
}

fn with_recorder(
    app: &AppHandle,
    action: impl FnOnce(&mut Recorder<WasapiOpener, QpcClock>) -> Result<(), RecorderError>,
) -> Result<RecorderStatus, String> {
    let result = (|| {
        {
            let mut recorder = lock_recorder(app)?;
            action(&mut recorder).map_err(|error| error.to_string())?;
            match recorder.status().phase {
                Phase::Paused => log_event(LogEvent::RecordingPaused),
                Phase::Recording => log_event(LogEvent::RecordingResumed),
                Phase::Idle => {}
            }
        }
        publish(app)
    })();
    if result.is_err() {
        log_event(LogEvent::RecordingFailed);
    }
    result
}

fn start(app: &AppHandle) -> Result<RecorderStatus, String> {
    let language = Settings::load(&app.state::<SettingsPath>().0).language;
    start_with(app, |recorder, library, started_at| {
        recorder.start_with_language(library, started_at, language)
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
    let result = (|| {
        {
            let _update_gate = lock_update_start_gate(app)?;
            let mut recorder = lock_recorder(app)?;
            ensure_update_not_active(app)?;
            let library_state = app.state::<LibraryState>();
            let mut library = library_state.lock().map_err(|error| error.to_string())?;
            let library = library.as_mut().map_err(|error| error.clone())?;
            action(&mut recorder, library, chrono::Local::now().fixed_offset())
                .map_err(|error| error.to_string())?;
        }
        let _ = app.emit("library-changed", ());
        log_event(LogEvent::RecordingStarted);
        start_background(app);
        publish(app)
    })();
    if result.is_err() {
        log_event(LogEvent::RecordingFailed);
    }
    result
}

fn stop(app: &AppHandle) -> Result<RecorderStatus, String> {
    let result = (|| {
        let stopped = lock_recorder(app)?.stop();
        match stopped {
            Ok(job) => {
                log_event(LogEvent::RecordingStopped);
                stop_background(app, Some(&job.meeting_id));
                spawn_save(app.clone(), job)
            }
            Err(error) => {
                let _ = publish(app);
                return Err(error.to_string());
            }
        }
        publish(app)
    })();
    if result.is_err() {
        log_event(LogEvent::RecordingFailed);
    }
    result
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
                    if let Ok(_gate) = lock_update_start_gate(&app) {
                        if ensure_update_not_active(&app).is_ok() {
                            if let Ok(mut processor) = lock_processor(&app) {
                                processor.enqueue(&job.meeting_id);
                            }
                        }
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
    // Off the main thread, like start_recording.
    let app = app.clone();
    thread::spawn(move || {
        let _ = start(&app);
    });
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
    stop_background(app, None);
    app.state::<ImportCancel>().0.store(true, Ordering::SeqCst);
    if let Ok(jobs) = app.state::<AiJobs>().lock() {
        for job in jobs.summaries.values() {
            job.cancel.store(true, Ordering::SeqCst);
        }
        for ask in jobs.asks.values() {
            ask.cancel.store(true, Ordering::SeqCst);
        }
    }
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
            || app
                .state::<ImportQueue>()
                .lock()
                .is_ok_and(|state| state.running)
            || app.state::<AiJobs>().lock().is_ok_and(|jobs| {
                jobs.summaries
                    .values()
                    .any(|job| matches!(job.state, SummaryJobState::Running))
                    || !jobs.asks.is_empty()
            })
        {
            thread::sleep(Duration::from_millis(100));
        }
        app.exit(0);
    });
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
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

/// Updates the tray and title on the main thread without waiting for it. Tray setters called
/// elsewhere block until the main thread runs them, so holding `shown` across them deadlocked a
/// main-thread Record against the elapsed-time ticker. The status is read when the task runs, so
/// a late task cannot show a stale phase.
fn update_tray(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Ok(status) = status(&handle) {
            update_tray_now(&handle, &status);
        }
    });
}

fn update_tray_now(app: &AppHandle, status: &RecorderStatus) {
    let Some(tray) = app.try_state::<Tray>() else {
        return;
    };
    let elapsed = format_elapsed(status.elapsed_ms);
    let (label, image) = match status.phase {
        Phase::Idle => ("Idle".to_owned(), &tray.idle_image),
        Phase::Recording => (format!("Recording {elapsed}"), &tray.recording_image),
        Phase::Paused => (format!("Paused {elapsed}"), &tray.recording_image),
    };
    let tooltip = format!("Meeting Minutes — {label}");
    let Ok(mut shown) = tray.shown.lock() else {
        log_event(LogEvent::TrayUpdateFailed);
        return;
    };
    if shown.phase != Some(status.phase) {
        let succeeded = setter_ok(tray.icon.set_icon(Some(image.clone())))
            & setter_ok(tray.record.set_enabled(status.phase == Phase::Idle))
            & setter_ok(tray.pause.set_enabled(status.phase != Phase::Idle))
            & setter_ok(tray.pause.set_text(if status.phase == Phase::Paused {
                "Resume"
            } else {
                "Pause"
            }))
            & setter_ok(tray.stop.set_enabled(status.phase != Phase::Idle));
        if succeeded {
            shown.phase = Some(status.phase);
        }
    }
    if shown.label.as_deref() != Some(label.as_str()) {
        let mut succeeded = setter_ok(tray.icon.set_tooltip(Some(tooltip)))
            & setter_ok(tray.status.set_text(&label));
        if let Some(window) = app.get_webview_window("main") {
            let title = if status.phase == Phase::Idle {
                "Meeting Minutes".to_owned()
            } else {
                format!("Meeting Minutes — {label}")
            };
            succeeded &= setter_ok(window.set_title(&title));
        }
        if succeeded {
            shown.label = Some(label);
        }
    }
}

fn setter_ok<T, E>(result: Result<T, E>) -> bool {
    if result.is_err() {
        log_event(LogEvent::TrayUpdateFailed);
        false
    } else {
        true
    }
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

fn recorder_is_idle(app: &AppHandle) -> Result<bool, String> {
    Ok(recorder_status_is_idle(&status(app)?))
}

fn recorder_status_is_idle(status: &RecorderStatus) -> bool {
    status.phase == Phase::Idle
        && !status
            .saves
            .iter()
            .any(|save| save.state == SaveState::Saving)
}

fn ensure_update_not_active(app: &AppHandle) -> Result<(), String> {
    if app.state::<PromptingUpdate>().0.load(Ordering::SeqCst)
        || app.state::<InstallingUpdate>().0.load(Ordering::SeqCst)
    {
        return Err("Finish the update prompt or installation before starting new work".into());
    }
    Ok(())
}

// Keep the established processor -> AI -> recorder lock order used by meeting_busy_locked.
fn with_update_state<T>(
    app: &AppHandle,
    check: impl FnOnce(&Processor, &AiJobsInner, &RecorderStatus, &ImportState) -> T,
) -> Result<T, String> {
    let processor = lock_processor(app)?;
    let jobs = app.state::<AiJobs>();
    let jobs = jobs.lock().map_err(|error| error.to_string())?;
    let recorder = lock_recorder(app)?;
    let recorder_status = recorder.status();
    let imports = app.state::<ImportQueue>();
    let imports = imports.lock().map_err(|error| error.to_string())?;
    Ok(check(&processor, &jobs, &recorder_status, &imports))
}

fn app_is_idle_for_update(
    recorder: &RecorderStatus,
    processor: &Processor,
    imports: &ImportState,
    jobs: &AiJobsInner,
) -> bool {
    recorder_status_is_idle(recorder)
        && !processor.is_running()
        && !processor
            .status()
            .jobs
            .iter()
            .any(|job| job.stage != JobStage::Failed)
        && !imports.running
        && imports.current.is_none()
        && imports.queue.is_empty()
        && !jobs
            .summaries
            .values()
            .any(|job| matches!(job.state, SummaryJobState::Running))
        && jobs.asks.is_empty()
}

struct UpdatePromptGuard(AppHandle);

impl Drop for UpdatePromptGuard {
    fn drop(&mut self) {
        self.0
            .state::<PromptingUpdate>()
            .0
            .store(false, Ordering::SeqCst);
    }
}

fn claim_update_prompt(app: &AppHandle) -> Result<Option<UpdatePromptGuard>, String> {
    let _update_gate = lock_update_start_gate(app)?;
    with_update_state(app, |processor, jobs, recorder, imports| {
        if !app_is_idle_for_update(recorder, processor, imports, jobs)
            || app.state::<Quitting>().0.load(Ordering::SeqCst)
            || app.state::<PromptingUpdate>().0.load(Ordering::SeqCst)
            || app.state::<InstallingUpdate>().0.load(Ordering::SeqCst)
        {
            return None;
        }
        app.state::<PromptingUpdate>()
            .0
            .store(true, Ordering::SeqCst);
        Some(UpdatePromptGuard(app.clone()))
    })
}

async fn wait_for_idle_app(app: &AppHandle) -> Result<(), String> {
    loop {
        if app.state::<Quitting>().0.load(Ordering::SeqCst) {
            return Err("App is quitting".into());
        }
        if with_update_state(app, |processor, jobs, recorder, imports| {
            app_is_idle_for_update(recorder, processor, imports, jobs)
        })? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn claim_update_install(app: &AppHandle) -> Result<bool, String> {
    let _update_gate = lock_update_start_gate(app)?;
    with_update_state(app, |processor, jobs, recorder, imports| {
        if !app_is_idle_for_update(recorder, processor, imports, jobs)
            || app.state::<Quitting>().0.load(Ordering::SeqCst)
            || app.state::<PromptingUpdate>().0.load(Ordering::SeqCst)
            || app.state::<InstallingUpdate>().0.load(Ordering::SeqCst)
        {
            return false;
        }
        app.state::<InstallingUpdate>()
            .0
            .store(true, Ordering::SeqCst);
        true
    })
}

async fn check_for_update(app: AppHandle) {
    let key_is_placeholder = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|config| config.get("pubkey"))
        .and_then(|key| key.as_str())
        == Some(UPDATER_PUBLIC_KEY_PLACEHOLDER);
    if key_is_placeholder {
        log_event(LogEvent::UpdateCheckFailed);
        return;
    }

    let update = match app.updater() {
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => update,
            Ok(None) => return,
            Err(_) => {
                log_event(LogEvent::UpdateCheckFailed);
                return;
            }
        },
        Err(_) => {
            log_event(LogEvent::UpdateCheckFailed);
            return;
        }
    };
    log_event(LogEvent::UpdateAvailable);

    let install = loop {
        if wait_for_idle_app(&app).await.is_err() {
            if !app.state::<Quitting>().0.load(Ordering::SeqCst) {
                log_event(LogEvent::UpdatePromptFailed);
            }
            return;
        }
        let dialog_app = app.clone();
        let version = update.version.clone();
        match tauri::async_runtime::spawn_blocking(move || {
            let _guard = claim_update_prompt(&dialog_app).ok()??;
            Some(
                dialog_app
                    .dialog()
                    .message(format!(
                        "Meeting Minutes {version} is available. Install it now? The app will restart."
                    ))
                    .title("Update available")
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Install".into(),
                        "Later".into(),
                    ))
                    .blocking_show(),
            )
        })
        .await
        {
            Ok(Some(install)) => break install,
            Ok(None) => continue,
            Err(_) => {
                log_event(LogEvent::UpdatePromptFailed);
                return;
            }
        }
    };
    if !install {
        log_event(LogEvent::UpdateDeclined);
        return;
    }

    loop {
        if wait_for_idle_app(&app).await.is_err() {
            if !app.state::<Quitting>().0.load(Ordering::SeqCst) {
                log_event(LogEvent::UpdateInstallFailed);
            }
            return;
        }
        match claim_update_install(&app) {
            Ok(true) => break,
            Ok(false) => continue,
            Err(_) => {
                log_event(LogEvent::UpdateInstallFailed);
                return;
            }
        }
    }
    if update.download_and_install(|_, _| {}, || {}).await.is_err() {
        app.state::<InstallingUpdate>()
            .0
            .store(false, Ordering::SeqCst);
        log_event(LogEvent::UpdateInstallFailed);
        return;
    }
    // Only non-Windows reaches this point; on Windows the updater exits after launching NSIS.
    log_event(LogEvent::UpdateInstalled);
    app.restart();
}

fn keep_window_on_screen(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    if window.is_maximized()? {
        return Ok(());
    }
    let monitors = window.available_monitors()?;
    let primary = window.primary_monitor()?;
    let primary_index = primary
        .as_ref()
        .and_then(|primary| {
            monitors
                .iter()
                .position(|monitor| monitor.position() == primary.position())
        })
        .unwrap_or(0);
    let work_areas: Vec<Rect> = monitors
        .iter()
        .map(|monitor| {
            let area = monitor.work_area();
            Rect {
                x: area.position.x,
                y: area.position.y,
                width: area.size.width,
                height: area.size.height,
            }
        })
        .collect();
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let restored = Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    if let Some(fitted) = fit_window(restored, &work_areas, primary_index) {
        if fitted.width != size.width || fitted.height != size.height {
            // Smaller work areas must be allowed to override the normal minimum size.
            window.set_min_size(Some(tauri::PhysicalSize::new(
                960.min(fitted.width),
                640.min(fitted.height),
            )))?;
            window.set_size(tauri::PhysicalSize::new(fitted.width, fitted.height))?;
        }
        window.set_position(tauri::PhysicalPosition::new(fitted.x, fitted.y))?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(Target::new(TargetKind::LogDir {
                    file_name: Some("meeting-minutes".into()),
                }))
                .rotation_strategy(RotationStrategy::KeepOne)
                .max_file_size(512 * 1024)
                .filter(|metadata| metadata.target() == "meeting_minutes_events")
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            // The window-state plugin restores before the app setup hook runs.
            if let Some(window) = app.get_webview_window("main") {
                keep_window_on_screen(&window)?;
            }
            log_event(LogEvent::Startup);
            let settings_path = app.path().app_data_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
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
                Library::open_with_name(root, index, settings.your_name.as_deref())
                    .map_err(|error| error.to_string())
            })();
            app.manage(Mutex::new(library));
            app.manage(SettingsPath(settings_path));
            #[cfg(windows)]
            app.manage(Secrets(Box::new(KeyringStore)));
            #[cfg(not(windows))]
            app.manage(Secrets(Box::new(ai::MemorySecretStore::default())));
            let http: Arc<dyn HttpClient> = Arc::new(UreqClient);
            #[cfg(windows)]
            let managed_store: Arc<dyn ManagedKeyStore> = Arc::new(KeyringStore);
            #[cfg(not(windows))]
            let managed_store: Arc<dyn ManagedKeyStore> =
                Arc::new(managed::MemoryManagedKeyStore::default());
            app.manage(Managed {
                keys: Arc::new(ManagedKeyState::new(managed_store)),
            });
            app.manage(Http(http));
            app.manage(Mutex::new(AiJobsInner::default()));
            app.manage(ModelsDir(app.path().app_data_dir()?.join("models")));
            app.manage(ModelDownloads::new(
                app.path().app_data_dir()?.join("models"),
            ));
            app.manage(Quitting(AtomicBool::new(false)));
            app.manage(PromptingUpdate(AtomicBool::new(false)));
            app.manage(InstallingUpdate(AtomicBool::new(false)));
            app.manage(UpdateStartGate(Mutex::new(())));
            app.manage(Mutex::new(Processor::new()));
            app.manage(BackgroundState::default());
            app.manage(DraftStore::default());
            app.manage(Mutex::new(ImportState::default()));
            app.manage(ImportCancel(AtomicBool::new(false)));
            app.manage(Mutex::new(Recorder::new(
                WasapiOpener,
                QpcClock::new()?,
                settings.recording_notice_acknowledged,
            )));
            #[cfg(windows)]
            app.manage(snapshot_windows::SnapshotState::default());
            #[cfg(windows)]
            app.manage(SnapshotUndoState::default());

            let open_item = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let status_item = MenuItem::with_id(app, "status", "Idle", false, None::<&str>)?;
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
                .tooltip("Meeting Minutes — Idle")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                    | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => show_window(tray.app_handle()),
                    _ => {}
                })
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
                shown: Mutex::new(TrayShown::default()),
            });

            recover(app.handle());
            processing_worker(app.handle().clone());
            tauri::async_runtime::spawn(check_for_update(app.handle().clone()));

            // Elapsed time for the window indicator and the tray while a Recording runs.
            let handle = app.handle().clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(500));
                if matches!(status(&handle), Ok(status) if status.phase == Phase::Recording) {
                    if let Ok(mut recorder) = lock_recorder(&handle) {
                        let before = recorder.status().device_switches.len();
                        let _ = recorder.collect_device_switches();
                        for switch in recorder.status().device_switches.iter().skip(before) {
                            log_event(match switch.source {
                                Source::Microphone => LogEvent::DeviceSwitchMicrophone,
                                Source::SystemAudio => LogEvent::DeviceSwitchSystemAudio,
                            });
                        }
                    }
                    let _ = publish(&handle);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app in the tray; Quit exits.
            match event {
                WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                #[cfg(windows)]
                WindowEvent::Destroyed if window.label().starts_with("capture-region-") => {
                    snapshot_windows::overlay_closed(window.app_handle());
                }
                WindowEvent::DragDrop(DragDropEvent::Enter { .. }) => {
                    let _ = window.emit("import-hover", true);
                }
                WindowEvent::DragDrop(DragDropEvent::Leave) => {
                    let _ = window.emit("import-hover", false);
                }
                WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) => {
                    let _ = window.emit("import-hover", false);
                    let _ = queue_imports(window.app_handle(), paths.clone());
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_diagnostics,
            get_microphone_usage,
            pick_audio_files,
            import_status,
            dismiss_import_errors,
            list_meetings,
            rescan_library,
            search_library,
            meeting_detail,
            open_meeting_folder,
            list_snapshots,
            read_snapshot_image,
            set_snapshot_caption,
            reveal_snapshot_folder,
            #[cfg(windows)]
            delete_snapshot,
            #[cfg(windows)]
            undo_snapshot_deletion,
            open_library_folder,
            open_link,
            rename_meeting,
            set_participant_name,
            assign_segments,
            rename_participant,
            remove_participant,
            list_participant_names,
            delete_meeting,
            recording_status,
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            #[cfg(windows)]
            snapshot_windows::take_snapshot,
            #[cfg(windows)]
            snapshot_windows::select_capture_region,
            #[cfg(windows)]
            snapshot_windows::cancel_capture_region,
            retry_save,
            continue_meeting,
            dismiss_continue,
            acknowledge_recording_notice,
            processing_status,
            transcript_draft,
            transcribe_meeting,
            cancel_processing,
            dismiss_processing,
            meeting_transcript,
            edit_segment,
            set_meeting_language,
            get_settings,
            first_run_info,
            complete_first_run,
            update_settings,
            update_snapshot_hotkeys,
            installed_models,
            model_download_status,
            start_model_download,
            cancel_model_download,
            delete_model_download,
            ai_status,
            managed_status,
            managed_save_key,
            managed_remove_key,
            managed_account,
            managed_send_code,
            managed_sign_in,
            managed_disconnect,
            managed_set_enabled,
            managed_delete_account,
            list_ai_models,
            test_cloud_api_key,
            test_speech_model,
            acknowledge_cloud_notice,
            save_api_key,
            delete_api_key,
            update_ai_settings,
            update_language_models,
            test_language_model,
            test_ai_provider,
            meeting_summary,
            summary_participants_changed,
            summary_status,
            summary_send_plan,
            generate_summary,
            cancel_summary,
            dismiss_summary,
            meeting_asks,
            ask_send_plan,
            ask_meeting,
            retry_ask,
            stop_ask
        ])
        .run(tauri::generate_context!())
        .expect("error while running Meeting Minutes");
}
