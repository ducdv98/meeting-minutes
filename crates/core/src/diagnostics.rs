use std::{fmt, time::Duration};

use crate::{
    ai::{AiError, ProviderKind},
    ask::AnswerState,
    cloud::EngineError,
    processing::{PowerState, ProcessingError, ProcessingStage},
    transcript::FallbackReason,
};

pub struct DiagnosticsInput {
    pub version: String,
    pub windows_version: String,
    pub cpu: String,
    pub ram_mb: u64,
    pub audio_devices: Result<Vec<String>, String>,
    pub selected_model: Option<String>,
    pub provider: Option<ProviderKind>,
    pub whisper_system_info: Option<String>,
    pub transcription_backend: Option<String>,
    pub recent_log_lines: Vec<String>,
}

#[derive(Clone)]
pub enum LogEvent {
    Startup,
    UpdateCheckFailed,
    UpdateAvailable,
    UpdatePromptFailed,
    UpdateInstalled,
    UpdateDeclined,
    UpdateInstallFailed,
    RecordingStarted,
    RecordingStopped,
    RecordingPaused,
    RecordingResumed,
    RecordingFailed,
    TrayUpdateFailed,
    DeviceSwitchMicrophone,
    DeviceSwitchSystemAudio,
    ProcessingQueued,
    ProcessingDownloading,
    ProcessingTranscribing,
    ProcessingCompleted,
    CorrectionResult {
        outcome: &'static str,
        changed_segments: usize,
        duration_ms: u128,
    },
    CorrectionFailure {
        kind: &'static str,
    },
    EngineFailure {
        engine: &'static str,
        kind: &'static str,
    },
    ProcessingFailed {
        stage: ProcessingStage,
        kind: &'static str,
        message: String,
        backend: &'static str,
        device: String,
        audio_ms: u64,
        total_ms: u64,
        whisper_ms: u128,
    },
    ProcessingCancelled {
        stage: ProcessingStage,
        backend: &'static str,
        device: String,
        audio_ms: u64,
        total_ms: u64,
        whisper_ms: u128,
    },
    VadUnavailable,
    GpuUnavailable,
    GpuLoadFailed,
    TranscriptionCompleted {
        model: String,
        backend: String,
        device: Option<String>,
        power: PowerState,
        threads: usize,
        audio_ms: u64,
        audio_after_vad_ms: Option<u64>,
        preparation_ms: u128,
        whisper_ms: u128,
        rtf: Option<f64>,
    },
    CloudSpan {
        engine: &'static str,
        audio_ms: u64,
        round_trip_ms: u128,
        fallback: &'static str,
    },
    EngineUsage {
        groq_ms: u64,
        gemini_ms: u64,
        local_ms: u64,
        fallback: &'static str,
    },
    SummaryStarted,
    SummaryCompleted,
    SummaryFailed,
    AskStarted,
    AskPending,
    AskAnswered,
    AskInsufficientEvidence,
    AskIncomplete,
    AskUnverified,
    AskOffTopic,
    AskFailed,
    ProviderError {
        kind: ProviderKind,
        status: Option<u16>,
    },
}

/// Backends a `transcription_completed` line may name; anything else logs as `unknown`.
pub const TRANSCRIPTION_BACKENDS: &[&str] = &["CPU", "Vulkan", "unknown"];

pub struct TranscriptionEnvironment<'a> {
    pub backend: &'a str,
    pub device: Option<&'a str>,
    pub power: PowerState,
}

/// A device name as the driver reports it, or None when it holds anything a GPU name would not, such as a path.
pub fn safe_device_name(name: &str) -> Option<&str> {
    (!name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')' | '+')))
    .then_some(name)
}

fn safe_engine_message(message: &str) -> String {
    // Only fixed engine diagnostics can be copied into logs; arbitrary errors may echo audio text.
    let first = message.lines().next().unwrap_or("").trim();
    if let Some(code) =
        first.strip_prefix("Generic whisper error. Varies depending on the function. Error code: ")
    {
        if let Ok(code) = code.parse::<i32>() {
            return format!("whisper_error_code_{code}");
        }
    }
    match first {
        "Whisper worker panicked" => "whisper_worker_panicked",
        "Transcript revision overflow" => "transcript_revision_overflow",
        "Transcript revision collision limit" => "transcript_revision_collision_limit",
        "English language id unavailable" => "english_language_id_unavailable",
        "Vietnamese language id unavailable" => "vietnamese_language_id_unavailable",
        "Failed to create a new whisper context." => "failed_to_create_whisper_context",
        "Failed to calculate the spectrogram for some reason." => "failed_to_calculate_spectrogram",
        "Failed to evaluate model." => "failed_to_evaluate_model",
        "Failed to run the encoder." => "failed_to_run_encoder",
        "Failed to run the decoder." => "failed_to_run_decoder",
        "Invalid number of mel bands." => "invalid_mel_bands",
        "Invalid thread count." => "invalid_thread_count",
        "Whisper returned a null pointer." => "whisper_null_pointer",
        "Whisper failed to convert the provided text into tokens." => "whisper_tokenization_failed",
        "Creating a state pointer failed." => "failed_to_create_whisper_state",
        "Input sample buffer was empty." => "empty_sample_buffer",
        _ => "redacted",
    }
    .to_owned()
}

impl LogEvent {
    pub fn engine_failure(engine: &str, error: &EngineError) -> Self {
        let engine = match engine {
            "Groq" => "groq",
            "Gemini" => "gemini",
            _ => "unknown",
        };
        Self::EngineFailure {
            engine,
            kind: error.diagnostic_kind(),
        }
    }
    pub fn correction_failure(error: &AiError) -> Self {
        Self::CorrectionFailure {
            kind: error.diagnostic_kind(),
        }
    }
    pub fn engine_usage(
        groq_ms: u64,
        gemini_ms: u64,
        local_ms: u64,
        reason: Option<&FallbackReason>,
    ) -> Self {
        Self::EngineUsage {
            groq_ms,
            gemini_ms,
            local_ms,
            fallback: safe_fallback(reason),
        }
    }
    pub fn cloud_span(
        engine: &str,
        audio_ms: u64,
        round_trip: Duration,
        fallback: Option<&FallbackReason>,
    ) -> Self {
        let engine = match engine {
            "Groq" => "groq",
            "Gemini" => "gemini",
            "local" => "local",
            _ => "unknown",
        };
        let fallback = safe_fallback(fallback);
        Self::CloudSpan {
            engine,
            audio_ms,
            round_trip_ms: round_trip.as_millis(),
            fallback,
        }
    }
    pub fn processing_result(
        error: &ProcessingError,
        stage: ProcessingStage,
        backend: Option<&str>,
        device: Option<&str>,
        audio_ms: u64,
        total_ms: u64,
        whisper: Duration,
    ) -> Self {
        let backend = TRANSCRIPTION_BACKENDS
            .iter()
            .copied()
            .find(|known| Some(*known) == backend)
            .unwrap_or("unknown");
        let device = device
            .and_then(safe_device_name)
            .unwrap_or("none")
            .to_owned();
        let audio_ms = audio_ms.min(total_ms);
        if matches!(error, ProcessingError::Cancelled) {
            return Self::ProcessingCancelled {
                stage,
                backend,
                device,
                audio_ms,
                total_ms,
                whisper_ms: whisper.as_millis(),
            };
        }
        let (kind, message) = match error {
            ProcessingError::Io(_) => ("io", "none".into()),
            ProcessingError::Json(_) => ("json", "none".into()),
            ProcessingError::Audio(_) => ("audio", "none".into()),
            ProcessingError::Unsupported(_) => ("unsupported", "none".into()),
            ProcessingError::Engine(message) => ("engine", safe_engine_message(message)),
            ProcessingError::Download(_) => ("download", "none".into()),
            ProcessingError::Checksum => ("checksum", "none".into()),
            ProcessingError::NoAudio => ("no_audio", "none".into()),
            ProcessingError::Offline => ("offline", "none".into()),
            ProcessingError::Deferred => ("deferred", "none".into()),
            ProcessingError::Cancelled => unreachable!(),
        };
        Self::ProcessingFailed {
            stage,
            kind,
            message,
            backend,
            device,
            audio_ms,
            total_ms,
            whisper_ms: whisper.as_millis(),
        }
    }

    pub fn transcription_completed(
        model: &str,
        backend: &str,
        threads: usize,
        audio_ms: u64,
        audio_after_vad_ms: Option<u64>,
        preparation: Duration,
        whisper: Duration,
    ) -> Self {
        Self::transcription_completed_with_device(
            model,
            TranscriptionEnvironment {
                backend,
                device: None,
                power: PowerState::Unknown,
            },
            threads,
            audio_ms,
            audio_after_vad_ms,
            preparation,
            whisper,
        )
    }

    pub fn transcription_completed_with_device(
        model: &str,
        environment: TranscriptionEnvironment<'_>,
        threads: usize,
        audio_ms: u64,
        audio_after_vad_ms: Option<u64>,
        preparation: Duration,
        whisper: Duration,
    ) -> Self {
        let model = crate::models::MODELS
            .iter()
            .find(|candidate| candidate.name == model)
            .map(|candidate| candidate.name)
            .unwrap_or("unknown");
        let backend = TRANSCRIPTION_BACKENDS
            .iter()
            .find(|candidate| **candidate == environment.backend)
            .unwrap_or(&"unknown");
        Self::TranscriptionCompleted {
            model: model.to_owned(),
            backend: (*backend).to_owned(),
            device: environment
                .device
                .and_then(safe_device_name)
                .map(str::to_owned),
            power: environment.power,
            threads,
            audio_ms,
            audio_after_vad_ms,
            preparation_ms: preparation.as_millis(),
            whisper_ms: whisper.as_millis(),
            rtf: (audio_ms > 0).then(|| whisper.as_secs_f64() / (audio_ms as f64 / 1000.0)),
        }
    }

    pub fn from_answer_state(state: AnswerState) -> Self {
        match state {
            AnswerState::Pending => Self::AskPending,
            AnswerState::Answered => Self::AskAnswered,
            AnswerState::InsufficientEvidence => Self::AskInsufficientEvidence,
            AnswerState::Incomplete => Self::AskIncomplete,
            AnswerState::Unverified => Self::AskUnverified,
            AnswerState::OffTopic => Self::AskOffTopic,
            AnswerState::Failed => Self::AskFailed,
        }
    }

    pub fn provider_error(kind: ProviderKind, error: &AiError) -> Self {
        Self::ProviderError {
            kind,
            status: match error {
                AiError::Http { status, .. } => Some(*status),
                _ => None,
            },
        }
    }
}

impl fmt::Display for LogEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Startup => "startup",
            Self::UpdateCheckFailed => "update_check_failed",
            Self::UpdateAvailable => "update_available",
            Self::UpdatePromptFailed => "update_prompt_failed",
            Self::UpdateInstalled => "update_installed",
            Self::UpdateDeclined => "update_declined",
            Self::UpdateInstallFailed => "update_install_failed",
            Self::RecordingStarted => "recording_started",
            Self::RecordingStopped => "recording_stopped",
            Self::RecordingPaused => "recording_paused",
            Self::RecordingResumed => "recording_resumed",
            Self::RecordingFailed => "recording_failed",
            Self::TrayUpdateFailed => "tray_update_failed",
            Self::DeviceSwitchMicrophone => "device_switch_microphone",
            Self::DeviceSwitchSystemAudio => "device_switch_system_audio",
            Self::ProcessingQueued => "processing_queued",
            Self::ProcessingDownloading => "processing_downloading",
            Self::ProcessingTranscribing => "processing_transcribing",
            Self::ProcessingCompleted => "processing_completed",
            Self::CorrectionResult {
                outcome,
                changed_segments,
                duration_ms,
            } => {
                return write!(f, "correction_result outcome={outcome} changed_segments={changed_segments} duration_ms={duration_ms}");
            }
            Self::CorrectionFailure { kind } => return write!(f, "correction_failure kind={kind}"),
            Self::EngineFailure { engine, kind } => {
                return write!(f, "engine_failure engine={engine} kind={kind}")
            }
            Self::ProcessingFailed {
                stage,
                kind,
                message,
                backend,
                device,
                audio_ms,
                total_ms,
                whisper_ms,
            } => {
                return write!(f, "processing_failed stage={} kind={kind} message={message} backend={backend} device=\"{device}\" audio_ms={audio_ms} total_ms={total_ms} whisper_ms={whisper_ms}", stage.as_str());
            }
            Self::ProcessingCancelled {
                stage,
                backend,
                device,
                audio_ms,
                total_ms,
                whisper_ms,
            } => {
                return write!(f, "processing_cancelled stage={} backend={backend} device=\"{device}\" audio_ms={audio_ms} total_ms={total_ms} whisper_ms={whisper_ms}", stage.as_str());
            }
            Self::VadUnavailable => "vad_unavailable",
            Self::GpuUnavailable => "gpu_unavailable",
            Self::GpuLoadFailed => "gpu_load_failed",
            Self::TranscriptionCompleted {
                model,
                backend,
                device,
                power,
                threads,
                audio_ms,
                audio_after_vad_ms,
                preparation_ms,
                whisper_ms,
                rtf,
            } => {
                let device = device.as_deref().unwrap_or("none");
                return write!(f, "transcription_completed model={model} backend={backend} device=\"{device}\" power={} threads={threads} audio_ms={audio_ms} audio_after_vad_ms={} preparation_ms={preparation_ms} whisper_ms={whisper_ms} rtf={}", power.label(),
                    audio_after_vad_ms.map_or_else(|| "none".to_owned(), |ms| ms.to_string()),
                    rtf.map_or_else(|| "none".to_owned(), |factor| format!("{factor:.3}")));
            }
            Self::CloudSpan {
                engine,
                audio_ms,
                round_trip_ms,
                fallback,
            } => {
                return write!(f, "cloud_span engine={engine} audio_ms={audio_ms} round_trip_ms={round_trip_ms} fallback={fallback}");
            }
            Self::EngineUsage {
                groq_ms,
                gemini_ms,
                local_ms,
                fallback,
            } => {
                return write!(f, "engine_usage groq_ms={groq_ms} gemini_ms={gemini_ms} local_ms={local_ms} fallback={fallback}");
            }
            Self::SummaryStarted => "summary_started",
            Self::SummaryCompleted => "summary_completed",
            Self::SummaryFailed => "summary_failed",
            Self::AskStarted => "ask_started",
            Self::AskPending => "ask_pending",
            Self::AskAnswered => "ask_answered",
            Self::AskInsufficientEvidence => "ask_insufficient_evidence",
            Self::AskIncomplete => "ask_incomplete",
            Self::AskUnverified => "ask_unverified",
            Self::AskOffTopic => "ask_off_topic",
            Self::AskFailed => "ask_failed",
            Self::ProviderError { kind, status } => {
                let kind = match kind {
                    ProviderKind::Ollama => "ollama",
                    ProviderKind::Openai => "openai",
                    ProviderKind::Anthropic => "anthropic",
                    ProviderKind::Groq => "groq",
                    ProviderKind::Gemini => "gemini",
                };
                return match status {
                    Some(status) => write!(f, "provider_error provider={kind} status={status}"),
                    None => write!(f, "provider_error provider={kind} status=none"),
                };
            }
        };
        f.write_str(name)
    }
}

pub fn log_event(event: LogEvent) {
    log::info!(target: "meeting_minutes_events", "{event}");
}

fn parse_cloud_span(event: &str) -> bool {
    let Some(rest) = event.strip_prefix("cloud_span engine=") else {
        return false;
    };
    let Some((engine, rest)) = rest.split_once(" audio_ms=") else {
        return false;
    };
    let Some((audio, rest)) = rest.split_once(" round_trip_ms=") else {
        return false;
    };
    let Some((round_trip, fallback)) = rest.split_once(" fallback=") else {
        return false;
    };
    matches!(engine, "groq" | "gemini" | "local" | "unknown")
        && audio.parse::<u64>().is_ok()
        && round_trip.parse::<u128>().is_ok()
        && matches!(
            fallback,
            "none" | "quota" | "unauthorized" | "offline" | "rate_limit" | "other"
        )
}

fn safe_fallback(reason: Option<&FallbackReason>) -> &'static str {
    match reason {
        Some(FallbackReason::Quota(_)) => "quota",
        Some(FallbackReason::Unauthorized(_)) => "unauthorized",
        Some(FallbackReason::Offline(_)) => "offline",
        Some(FallbackReason::RateLimit(_)) => "rate_limit",
        Some(_) => "other",
        None => "none",
    }
}

fn parse_engine_usage(event: &str) -> bool {
    let Some(rest) = event.strip_prefix("engine_usage groq_ms=") else {
        return false;
    };
    let Some((groq, rest)) = rest.split_once(" gemini_ms=") else {
        return false;
    };
    let Some((gemini, rest)) = rest.split_once(" local_ms=") else {
        return false;
    };
    let Some((local, fallback)) = rest.split_once(" fallback=") else {
        return false;
    };
    [groq, gemini, local]
        .iter()
        .all(|audio| audio.parse::<u64>().is_ok())
        && matches!(
            fallback,
            "none" | "quota" | "unauthorized" | "offline" | "rate_limit" | "other"
        )
}

fn parse_failure_kind(event: &str) -> bool {
    let kind = event.strip_prefix("correction_failure kind=").or_else(|| {
        let rest = event.strip_prefix("engine_failure engine=")?;
        let (engine, kind) = rest.split_once(" kind=")?;
        matches!(engine, "groq" | "gemini" | "unknown").then_some(kind)
    });
    kind.is_some_and(|kind| {
        matches!(
            kind,
            "rate_limit"
                | "quota"
                | "unauthorized"
                | "offline"
                | "cancelled"
                | "invalid_response"
                | "http_status"
                | "transport"
                | "invalid_output"
                | "key_store"
                | "other"
        )
    })
}

pub fn log_provider_error(kind: ProviderKind, error: &AiError) {
    log::warn!(target: "meeting_minutes_events", "{}", LogEvent::provider_error(kind, error));
}

pub fn log_provider_status(kind: ProviderKind, status: u16) {
    log::warn!(target: "meeting_minutes_events", "{}", LogEvent::ProviderError { kind, status: Some(status) });
}

fn safe_log_line(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('[') else {
        return false;
    };
    let Some((date, rest)) = rest.split_once("][") else {
        return false;
    };
    let Some((time, rest)) = rest.split_once("][") else {
        return false;
    };
    let Some(rest) = rest.strip_prefix("meeting_minutes_events][") else {
        return false;
    };
    let Some((level, event)) = rest.split_once("] ") else {
        return false;
    };
    let date = date.as_bytes();
    let time = time.as_bytes();
    if date.len() != 10
        || !date.iter().enumerate().all(|(i, c)| {
            if matches!(i, 4 | 7) {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        || time.len() != 8
        || !time.iter().enumerate().all(|(i, c)| {
            if matches!(i, 2 | 5) {
                *c == b':'
            } else {
                c.is_ascii_digit()
            }
        })
        || !matches!(level, "INFO" | "WARN")
    {
        return false;
    }
    if matches!(
        event,
        "startup"
            | "update_check_failed"
            | "update_available"
            | "update_prompt_failed"
            | "update_installed"
            | "update_declined"
            | "update_install_failed"
            | "recording_started"
            | "recording_stopped"
            | "recording_paused"
            | "recording_resumed"
            | "recording_failed"
            | "tray_update_failed"
            | "device_switch_microphone"
            | "device_switch_system_audio"
            | "processing_queued"
            | "processing_downloading"
            | "processing_transcribing"
            | "processing_completed"
            | "vad_unavailable"
            | "gpu_unavailable"
            | "gpu_load_failed"
            | "summary_started"
            | "summary_completed"
            | "summary_failed"
            | "ask_started"
            | "ask_pending"
            | "ask_answered"
            | "ask_insufficient_evidence"
            | "ask_incomplete"
            | "ask_unverified"
            | "ask_off_topic"
            | "ask_failed"
    ) {
        return true;
    }
    if parse_processing_event(event) {
        return true;
    }
    if parse_correction_event(event) {
        return true;
    }
    if parse_transcription_rtf(event).is_some()
        || parse_cloud_span(event)
        || parse_engine_usage(event)
        || parse_failure_kind(event)
    {
        return true;
    }
    let Some(rest) = event.strip_prefix("provider_error provider=") else {
        return false;
    };
    let Some((provider, status)) = rest.split_once(" status=") else {
        return false;
    };
    matches!(provider, "ollama" | "openai" | "anthropic")
        && (status == "none"
            || status
                .parse::<u16>()
                .is_ok_and(|value| value.to_string() == status))
}

fn parse_correction_event(event: &str) -> bool {
    let Some((outcome, rest)) = event
        .strip_prefix("correction_result outcome=")
        .and_then(|rest| rest.split_once(" changed_segments="))
    else {
        return false;
    };
    let Some((changed, duration)) = rest.split_once(" duration_ms=") else {
        return false;
    };
    matches!(outcome, "completed" | "failed" | "unavailable")
        && changed
            .parse::<usize>()
            .is_ok_and(|value| value.to_string() == changed)
        && duration
            .parse::<u128>()
            .is_ok_and(|value| value.to_string() == duration)
}

fn parse_processing_event(event: &str) -> bool {
    let (failed, rest) = if let Some(rest) = event.strip_prefix("processing_failed stage=") {
        (true, rest)
    } else if let Some(rest) = event.strip_prefix("processing_cancelled stage=") {
        (false, rest)
    } else {
        return false;
    };
    let Some((stage, rest)) = rest.split_once(' ') else {
        return false;
    };
    if !matches!(
        stage,
        "downloading" | "preparing" | "transcribing" | "publishing"
    ) {
        return false;
    }
    let rest = if failed {
        let Some((kind, rest)) = rest
            .strip_prefix("kind=")
            .and_then(|rest| rest.split_once(" message="))
        else {
            return false;
        };
        if !matches!(
            kind,
            "io" | "json"
                | "audio"
                | "unsupported"
                | "engine"
                | "download"
                | "checksum"
                | "no_audio"
        ) {
            return false;
        }
        let Some((message, rest)) = rest.split_once(" backend=") else {
            return false;
        };
        if !matches!(
            message,
            "none"
                | "redacted"
                | "whisper_worker_panicked"
                | "transcript_revision_overflow"
                | "transcript_revision_collision_limit"
                | "english_language_id_unavailable"
                | "vietnamese_language_id_unavailable"
                | "failed_to_create_whisper_context"
                | "failed_to_calculate_spectrogram"
                | "failed_to_evaluate_model"
                | "failed_to_run_encoder"
                | "failed_to_run_decoder"
                | "invalid_mel_bands"
                | "invalid_thread_count"
                | "whisper_null_pointer"
                | "whisper_tokenization_failed"
                | "failed_to_create_whisper_state"
                | "empty_sample_buffer"
        ) && !message
            .strip_prefix("whisper_error_code_")
            .is_some_and(|code| {
                code.parse::<i32>()
                    .is_ok_and(|value| value.to_string() == code)
            })
        {
            return false;
        }
        rest
    } else {
        rest
    };
    let rest = if failed {
        rest
    } else {
        rest.strip_prefix("backend=").unwrap_or("")
    };
    let Some((backend, rest)) = rest.split_once(" device=\"") else {
        return false;
    };
    if !TRANSCRIPTION_BACKENDS.contains(&backend) {
        return false;
    }
    let Some((device, rest)) = rest.split_once("\" audio_ms=") else {
        return false;
    };
    if device != "none" && safe_device_name(device).is_none() {
        return false;
    }
    let Some((audio, rest)) = rest.split_once(" total_ms=") else {
        return false;
    };
    let Some((total, whisper)) = rest.split_once(" whisper_ms=") else {
        return false;
    };
    let number = |s: &str| s.parse::<u128>().is_ok_and(|n| n.to_string() == s);
    number(audio) && number(total) && number(whisper)
}

fn parse_transcription_rtf(event: &str) -> Option<&str> {
    let rest = event.strip_prefix("transcription_completed model=")?;
    let (model, rest) = rest.split_once(" backend=")?;
    if !crate::models::MODELS
        .iter()
        .any(|candidate| candidate.name == model)
    {
        return None;
    }
    let (backend, rest) = rest.split_once(" device=\"")?;
    if !TRANSCRIPTION_BACKENDS.contains(&backend) {
        return None;
    }
    let (device, rest) = rest.split_once("\" power=")?;
    if device != "none" && safe_device_name(device).is_none() {
        return None;
    }
    let (power, rest) = rest.split_once(" threads=")?;
    if !matches!(power, "ac" | "battery" | "unknown") {
        return None;
    }
    let (threads, rest) = rest.split_once(" audio_ms=")?;
    let (audio_ms, rest) = rest.split_once(" audio_after_vad_ms=")?;
    let (after_vad_ms, rest) = rest.split_once(" preparation_ms=")?;
    let (preparation_ms, rest) = rest.split_once(" whisper_ms=")?;
    let (whisper_ms, rtf) = rest.split_once(" rtf=")?;
    let canonical_number = |value: &str| {
        value
            .parse::<u128>()
            .is_ok_and(|number| number.to_string() == value)
    };
    if !canonical_number(threads)
        || !canonical_number(audio_ms)
        || (after_vad_ms != "none" && !canonical_number(after_vad_ms))
        || !canonical_number(preparation_ms)
        || !canonical_number(whisper_ms)
    {
        return None;
    }
    if rtf == "none" && audio_ms == "0" {
        return Some(rtf);
    }
    let (whole, fraction) = rtf.split_once('.')?;
    (audio_ms != "0"
        && canonical_number(whole)
        && fraction.len() == 3
        && fraction.bytes().all(|digit| digit.is_ascii_digit()))
    .then_some(rtf)
}

pub fn build_diagnostics(input: &DiagnosticsInput) -> String {
    let devices = match &input.audio_devices {
        Ok(devices) if devices.is_empty() => "None detected".to_owned(),
        Ok(devices) => devices.join("\n"),
        Err(_) => "Unavailable".to_owned(),
    };
    let lines = input
        .recent_log_lines
        .iter()
        .rev()
        .take(100)
        .rev()
        .filter(|line| safe_log_line(line))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let last_rtf = input
        .recent_log_lines
        .iter()
        .rev()
        .find_map(|line| {
            if !safe_log_line(line) {
                return None;
            }
            let (_, event) = line.split_once("] ")?;
            parse_transcription_rtf(event)
        })
        .unwrap_or("None");
    let last_failure = input
        .recent_log_lines
        .iter()
        .rev()
        .find(|line| {
            if !safe_log_line(line) {
                return false;
            }
            line.split_once("] ")
                .is_some_and(|(_, event)| event.starts_with("processing_failed "))
        })
        .map_or("None", String::as_str);
    format!(
        "Meeting Minutes {} Beta\nWindows: {}\nCPU: {}\nRAM: {} MB\nAudio devices:\n{}\nSelected model: {}\nProvider: {}\nwhisper.cpp system info: {}\nTranscription backend: {}\nLast transcription RTF: {}\nLast processing failure: {}\nRecent log lines:\n{}",
        input.version,
        input.windows_version,
        input.cpu,
        input.ram_mb,
        devices,
        input.selected_model.as_deref().unwrap_or("None"),
        input.provider.map(ProviderKind::label).unwrap_or("None"),
        input.whisper_system_info.as_deref().unwrap_or("Unavailable"),
        input.transcription_backend.as_deref().unwrap_or("Unavailable"),
        if last_rtf == "None" || last_rtf == "none" { "None".to_owned() } else { format!("{last_rtf}x") },
        last_failure,
        lines,
    )
}
