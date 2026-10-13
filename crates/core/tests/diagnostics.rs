use meeting_minutes_core::ai::{AiError, ProviderKind};
use meeting_minutes_core::ask::AnswerState;
use meeting_minutes_core::diagnostics::{build_diagnostics, DiagnosticsInput, LogEvent};
use std::time::Duration;

#[test]
fn transcription_event_formats_timings_and_rtf_without_meeting_content() {
    let event = LogEvent::transcription_completed(
        "small",
        "CPU",
        8,
        16_000,
        None,
        Duration::from_millis(320),
        Duration::from_millis(4_000),
    );
    assert_eq!(event.to_string(), "transcription_completed model=small backend=CPU threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250");
}

#[test]
fn transcription_event_handles_zero_audio_and_does_not_echo_unknown_model() {
    let event = LogEvent::transcription_completed(
        "private Meeting title",
        "CPU",
        1,
        0,
        None,
        Duration::ZERO,
        Duration::from_millis(10),
    )
    .to_string();
    assert!(event.contains("model=unknown"));
    assert!(event.ends_with("rtf=none"));
    assert!(!event.contains("private Meeting title"));
}

#[test]
fn diagnostics_shows_whisper_info_backend_and_latest_rtf() {
    let line = |rtf: &str| {
        format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=small backend=CPU threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf={rtf}")
    };
    let text = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows 11".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: Some("small".into()),
        provider: None,
        whisper_system_info: Some("AVX2 = 1 | FMA = 1".into()),
        transcription_backend: Some("CPU".into()),
        recent_log_lines: vec![line("0.500"), line("0.250")],
    });
    assert!(text.contains("whisper.cpp system info: AVX2 = 1 | FMA = 1"));
    assert!(text.contains("Transcription backend: CPU"));
    assert!(text.contains("Last transcription RTF: 0.250x"));
    assert!(!text.contains("Last transcription RTF: 0.500x"));
}

#[test]
fn diagnostics_finds_last_rtf_before_recent_log_window() {
    let completed = "[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=small backend=CPU threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250";
    let mut lines = vec![completed.to_owned()];
    lines.extend(std::iter::repeat_n(
        "[2026-10-13][10:47:03][meeting_minutes_events][INFO] startup".to_owned(),
        101,
    ));
    let text = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows 11".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: None,
        recent_log_lines: lines,
    });
    assert!(text.contains("Last transcription RTF: 0.250x"));
    assert!(!text.contains(completed));
}

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
        whisper_system_info: None,
        transcription_backend: None,
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
        whisper_system_info: None, transcription_backend: None,
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
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=small backend=CPU threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250 title=private",
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=private backend=CPU threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250",
    ];
    let text = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows 11".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: None,
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
        whisper_system_info: None,
        transcription_backend: None,
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
        LogEvent::transcription_completed(
            "small",
            "Vulkan",
            8,
            960_000,
            None,
            Duration::from_millis(1_200),
            Duration::from_secs(96),
        ),
        LogEvent::transcription_completed(
            "small",
            "Some GPU name",
            8,
            960_000,
            None,
            Duration::from_millis(1_200),
            Duration::from_secs(96),
        ),
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
            whisper_system_info: None,
            transcription_backend: None,
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
