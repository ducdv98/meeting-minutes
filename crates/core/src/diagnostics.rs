use std::{fmt, time::Duration};

use crate::{
    ai::{AiError, ProviderKind},
    ask::AnswerState,
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
    ProcessingFailed,
    VadUnavailable,
    GpuUnavailable,
    GpuLoadFailed,
    TranscriptionCompleted {
        model: String,
        backend: String,
        threads: usize,
        audio_ms: u64,
        audio_after_vad_ms: Option<u64>,
        preparation_ms: u128,
        whisper_ms: u128,
        rtf: Option<f64>,
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

impl LogEvent {
    pub fn transcription_completed(
        model: &str,
        backend: &str,
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
            .find(|candidate| **candidate == backend)
            .unwrap_or(&"unknown");
        Self::TranscriptionCompleted {
            model: model.to_owned(),
            backend: (*backend).to_owned(),
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
            Self::ProcessingFailed => "processing_failed",
            Self::VadUnavailable => "vad_unavailable",
            Self::GpuUnavailable => "gpu_unavailable",
            Self::GpuLoadFailed => "gpu_load_failed",
            Self::TranscriptionCompleted {
                model,
                backend,
                threads,
                audio_ms,
                audio_after_vad_ms,
                preparation_ms,
                whisper_ms,
                rtf,
            } => {
                return write!(f, "transcription_completed model={model} backend={backend} threads={threads} audio_ms={audio_ms} audio_after_vad_ms={} preparation_ms={preparation_ms} whisper_ms={whisper_ms} rtf={}",
                    audio_after_vad_ms.map_or_else(|| "none".to_owned(), |ms| ms.to_string()),
                    rtf.map_or_else(|| "none".to_owned(), |factor| format!("{factor:.3}")));
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
            | "processing_failed"
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
    if parse_transcription_rtf(event).is_some() {
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

fn parse_transcription_rtf(event: &str) -> Option<&str> {
    let rest = event.strip_prefix("transcription_completed model=")?;
    let (model, rest) = rest.split_once(" backend=")?;
    if !crate::models::MODELS
        .iter()
        .any(|candidate| candidate.name == model)
    {
        return None;
    }
    let (backend, rest) = rest.split_once(" threads=")?;
    if !TRANSCRIPTION_BACKENDS.contains(&backend) {
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
    format!(
        "Meeting Minutes {} Beta\nWindows: {}\nCPU: {}\nRAM: {} MB\nAudio devices:\n{}\nSelected model: {}\nProvider: {}\nwhisper.cpp system info: {}\nTranscription backend: {}\nLast transcription RTF: {}\nRecent log lines:\n{}",
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
        lines,
    )
}
