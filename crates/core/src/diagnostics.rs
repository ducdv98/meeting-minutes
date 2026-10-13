use std::fmt;

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
    pub recent_log_lines: Vec<String>,
}

#[derive(Clone, Copy)]
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

impl LogEvent {
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

pub fn build_diagnostics(input: &DiagnosticsInput) -> String {
    let devices = match &input.audio_devices {
        Ok(devices) if devices.is_empty() => "None detected".to_owned(),
        Ok(devices) => devices.join("\n"),
        Err(_) => "Unavailable".to_owned(),
    };
    let lines = input
        .recent_log_lines
        .iter()
        .filter(|line| safe_log_line(line))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Meeting Minutes {} Beta\nWindows: {}\nCPU: {}\nRAM: {} MB\nAudio devices:\n{}\nSelected model: {}\nProvider: {}\nRecent log lines:\n{}",
        input.version,
        input.windows_version,
        input.cpu,
        input.ram_mb,
        devices,
        input.selected_model.as_deref().unwrap_or("None"),
        input.provider.map(ProviderKind::label).unwrap_or("None"),
        lines,
    )
}
