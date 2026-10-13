use meeting_minutes_core::ai::{AiError, ProviderKind};
use meeting_minutes_core::ask::AnswerState;
use meeting_minutes_core::diagnostics::{build_diagnostics, DiagnosticsInput, LogEvent};

#[test]
fn diagnostics_lists_system_and_setup_even_without_model_or_provider() {
    let text = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows 11 24H2".into(),
        cpu: "Example CPU".into(),
        ram_mb: 16384,
        audio_devices: Ok(vec![
            "Microphone: Headset".into(),
            "System audio: Speakers".into(),
        ]),
        selected_model: None,
        provider: None,
        recent_log_lines: vec![
            "[2026-10-13][10:47:02][meeting_minutes_events][INFO] startup".into(),
        ],
    });
    assert!(text.contains("Meeting Minutes 0.1.0 Beta"));
    assert!(text.contains("Windows 11 24H2"));
    assert!(text.contains("Example CPU"));
    assert!(text.contains("16384 MB"));
    assert!(text.contains("Microphone: Headset"));
    assert!(text.contains("System audio: Speakers"));
    assert!(text.contains("Selected model: None"));
    assert!(text.contains("Provider: None"));
    assert!(text.contains("[2026-10-13][10:47:02][meeting_minutes_events][INFO] startup"));
}

#[test]
fn diagnostics_excludes_content_and_keys_from_untrusted_log_lines() {
    let transcript = "private transcript sentence";
    let summary = "private summary paragraph";
    let ask = "private Ask question";
    let key = "sk-private-api-key";
    let text = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(), windows_version: "Windows 11".into(),
        cpu: "CPU".into(), ram_mb: 1024, audio_devices: Ok(vec![]),
        selected_model: Some("small".into()), provider: Some(ProviderKind::Openai),
        recent_log_lines: vec![
            format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] startup {transcript}"),
            format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {summary}"),
            format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {ask}"),
            format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {key}"),
            "[2026-10-13][10:47:02][other_target][INFO] startup".into(),
            "[2026-10-13][10:47:02][meeting_minutes_events][INFO] C:\\Meetings\\Meeting-title\\audio.flac".into(),
            "[2026-10-13][10:47:02][meeting_minutes_events][WARN] provider_error provider=openai status=401".into(),
        ],
    });
    for secret in [transcript, summary, ask, key] {
        assert!(!text.contains(secret));
    }
    assert!(!text.contains("other_target"));
    assert!(!text.contains("Meeting-title"));
    assert!(text.contains("[2026-10-13][10:47:02][meeting_minutes_events][WARN] provider_error provider=openai status=401"));
}

#[test]
fn diagnostics_rejects_non_plugin_headers_and_noncanonical_events() {
    let bad_lines = [
        "[[2026-10-13]][INFO][meeting_minutes_events] startup",
        "[2026-10-13][10:47:02][meeting_minutes_events][DEBUG] startup",
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] provider_error provider=openai status=+401",
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] unknown_event",
    ];
    let text = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows 11".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        recent_log_lines: bad_lines.iter().map(|line| (*line).to_owned()).collect(),
    });
    for line in bad_lines {
        assert!(!text.contains(line), "unexpectedly accepted: {line}");
    }
}

#[test]
fn diagnostics_distinguishes_no_audio_devices_from_unavailable_enumeration() {
    let mut input = DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows 11".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        recent_log_lines: vec![],
    };
    assert!(build_diagnostics(&input).contains("Audio devices:\nNone detected"));
    input.audio_devices = Err("device access failed".into());
    let text = build_diagnostics(&input);
    assert!(text.contains("Audio devices:\nUnavailable"));
    assert!(!text.contains("device access failed"));
}

#[test]
fn every_log_event_is_accepted_in_the_plugin_format() {
    let events = [
        LogEvent::Startup,
        LogEvent::UpdateCheckFailed,
        LogEvent::UpdateAvailable,
        LogEvent::UpdatePromptFailed,
        LogEvent::UpdateInstalled,
        LogEvent::UpdateDeclined,
        LogEvent::UpdateInstallFailed,
        LogEvent::RecordingStarted,
        LogEvent::RecordingStopped,
        LogEvent::RecordingPaused,
        LogEvent::RecordingResumed,
        LogEvent::RecordingFailed,
        LogEvent::TrayUpdateFailed,
        LogEvent::DeviceSwitchMicrophone,
        LogEvent::DeviceSwitchSystemAudio,
        LogEvent::ProcessingQueued,
        LogEvent::ProcessingDownloading,
        LogEvent::ProcessingTranscribing,
        LogEvent::ProcessingCompleted,
        LogEvent::ProcessingFailed,
        LogEvent::SummaryStarted,
        LogEvent::SummaryCompleted,
        LogEvent::SummaryFailed,
        LogEvent::AskStarted,
        LogEvent::AskPending,
        LogEvent::AskAnswered,
        LogEvent::AskInsufficientEvidence,
        LogEvent::AskIncomplete,
        LogEvent::AskUnverified,
        LogEvent::AskOffTopic,
        LogEvent::AskFailed,
        LogEvent::ProviderError {
            kind: ProviderKind::Ollama,
            status: None,
        },
        LogEvent::ProviderError {
            kind: ProviderKind::Openai,
            status: Some(401),
        },
        LogEvent::ProviderError {
            kind: ProviderKind::Anthropic,
            status: Some(429),
        },
    ];
    for event in events {
        let line = format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {event}");
        let text = build_diagnostics(&DiagnosticsInput {
            version: "0.1.0".into(),
            windows_version: "Windows 11".into(),
            cpu: "CPU".into(),
            ram_mb: 1024,
            audio_devices: Ok(vec![]),
            selected_model: None,
            provider: None,
            recent_log_lines: vec![line.clone()],
        });
        assert!(text.contains(&line), "event rejected: {event}");
    }
}

#[test]
fn ask_states_have_distinct_log_events() {
    let cases = [
        (AnswerState::Pending, "ask_pending"),
        (AnswerState::Answered, "ask_answered"),
        (
            AnswerState::InsufficientEvidence,
            "ask_insufficient_evidence",
        ),
        (AnswerState::Incomplete, "ask_incomplete"),
        (AnswerState::Unverified, "ask_unverified"),
        (AnswerState::OffTopic, "ask_off_topic"),
        (AnswerState::Failed, "ask_failed"),
    ];
    for (state, expected) in cases {
        assert_eq!(LogEvent::from_answer_state(state).to_string(), expected);
    }
}

#[test]
fn provider_error_event_never_formats_server_content_or_api_key() {
    let error = AiError::Http {
        status: 429,
        message: "private transcript; private summary; private Ask; sk-private-api-key".into(),
    };
    let event = LogEvent::provider_error(ProviderKind::Anthropic, &error);
    assert_eq!(
        event.to_string(),
        "provider_error provider=anthropic status=429"
    );
    for secret in [
        "private transcript",
        "private summary",
        "private Ask",
        "sk-private-api-key",
    ] {
        assert!(!event.to_string().contains(secret));
    }
}
