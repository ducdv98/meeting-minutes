use meeting_minutes_core::ai::{AiError, ProviderKind};
use meeting_minutes_core::ask::AnswerState;
use meeting_minutes_core::cloud::EngineError;
use meeting_minutes_core::diagnostics::{
    build_diagnostics, DiagnosticsInput, LogEvent, TranscriptionEnvironment,
};
use meeting_minutes_core::processing::{PowerState, ProcessingError, ProcessingStage};
use meeting_minutes_core::transcript::{EngineName, FallbackReason};
use std::time::Duration;

#[test]
fn diagnostics_count_failure_kinds_without_provider_content() {
    let engine = LogEvent::engine_failure("Groq", &EngineError::InvalidResponse);
    let correction =
        LogEvent::correction_failure(&AiError::InvalidOutput("private Meeting text".into()));
    assert_eq!(
        engine.to_string(),
        "engine_failure engine=groq kind=invalid_response"
    );
    assert_eq!(
        correction.to_string(),
        "correction_failure kind=invalid_output"
    );
    let report = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: None,
        recent_log_lines: [engine, correction]
            .iter()
            .map(|event| format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {event}"))
            .collect(),
    });
    assert!(report.contains("engine_failure engine=groq kind=invalid_response"));
    assert!(report.contains("correction_failure kind=invalid_output"));
    assert!(!report.contains("private Meeting text"));
}

#[test]
fn diagnostics_preserves_invalid_managed_key_failure_kind() {
    let event = LogEvent::engine_failure("Managed transcription", &EngineError::ManagedKeyInvalid);
    assert_eq!(
        event.to_string(),
        "engine_failure engine=managed kind=invalid_key"
    );
    let report = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: None,
        recent_log_lines: vec![format!(
            "[2026-10-13][10:47:02][meeting_minutes_events][INFO] {event}"
        )],
    });
    assert!(report.contains("engine_failure engine=managed kind=invalid_key"));
}

#[test]
fn diagnostics_keeps_cloud_audio_time_and_fallback_without_content() {
    let event = LogEvent::cloud_span(
        "Groq",
        30_000,
        Duration::from_millis(420),
        Some(&FallbackReason::Quota(EngineName::Groq)),
    );
    assert_eq!(
        event.to_string(),
        "cloud_span engine=groq audio_ms=30000 round_trip_ms=420 fallback=quota"
    );
    let report = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: None,
        recent_log_lines: vec![format!(
            "[2026-10-13][10:47:02][meeting_minutes_events][INFO] {event}"
        )],
    });
    assert!(
        report.contains("cloud_span engine=groq audio_ms=30000 round_trip_ms=420 fallback=quota")
    );
    assert!(!report.contains("private words"));
    assert!(!LogEvent::cloud_span(
        "Groq",
        0,
        Duration::ZERO,
        Some(&FallbackReason::Legacy("private words".into()))
    )
    .to_string()
    .contains("private words"));
}

#[test]
fn diagnostics_keeps_engine_totals_after_many_spans() {
    let span = LogEvent::cloud_span("Groq", 30_000, Duration::from_millis(420), None);
    let summary = LogEvent::engine_usage(
        2_400_000,
        0,
        1_200_000,
        Some(&FallbackReason::Quota(EngineName::Groq)),
    );
    let mut lines =
        vec![format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {span}"); 120];
    lines.push(format!(
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] {summary}"
    ));
    let report = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: None,
        recent_log_lines: lines,
    });
    assert!(
        report.contains("engine_usage groq_ms=2400000 gemini_ms=0 local_ms=1200000 fallback=quota")
    );
}

#[test]
fn diagnostics_includes_managed_audio_total() {
    let usage = LogEvent::engine_usage_with_managed(
        30_000,
        10_000,
        0,
        0,
        Some(&FallbackReason::Quota(EngineName::Managed)),
    );
    let line = format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {usage}");
    let report = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: None,
        recent_log_lines: vec![line.clone()],
    });
    assert!(line.contains("engine_usage managed_ms=30000 groq_ms=10000"));
    assert!(report.contains(&line));
}

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
    assert_eq!(event.to_string(), "transcription_completed model=small backend=CPU device=\"none\" power=unknown threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250");
}

#[test]
fn diagnostics_keeps_selected_device_and_power_in_completed_transcription() {
    let event = LogEvent::transcription_completed_with_device(
        "small",
        TranscriptionEnvironment {
            backend: "Vulkan",
            device: Some("NVIDIA GeForce RTX 4050 Laptop GPU"),
            power: PowerState::Ac,
        },
        8,
        16_000,
        None,
        Duration::ZERO,
        Duration::from_millis(800),
    );
    let line = format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] {event}");
    assert!(event
        .to_string()
        .contains("backend=Vulkan device=\"NVIDIA GeForce RTX 4050 Laptop GPU\" power=ac"));
    let text = build_diagnostics(&DiagnosticsInput {
        version: "0.1.0".into(),
        windows_version: "Windows 11".into(),
        cpu: "CPU".into(),
        ram_mb: 1024,
        audio_devices: Ok(vec![]),
        selected_model: None,
        provider: None,
        whisper_system_info: None,
        transcription_backend: Some(
            "Vulkan device=\"NVIDIA GeForce RTX 4050 Laptop GPU\" power=ac".into(),
        ),
        recent_log_lines: vec![line.clone()],
    });
    assert!(text.contains(
        "Transcription backend: Vulkan device=\"NVIDIA GeForce RTX 4050 Laptop GPU\" power=ac"
    ));
    assert!(text.contains(&line));
    assert!(text.contains("Last transcription RTF: 0.050x"));
}

#[test]
fn fallback_events_are_content_free() {
    assert_eq!(LogEvent::VadUnavailable.to_string(), "vad_unavailable");
    assert_eq!(LogEvent::GpuUnavailable.to_string(), "gpu_unavailable");
    assert_eq!(LogEvent::GpuLoadFailed.to_string(), "gpu_load_failed");
}

#[test]
fn failure_event_reports_stage_kind_device_and_progress_without_private_content() {
    let event = LogEvent::processing_result(
        &ProcessingError::Engine(
            "Whisper worker panicked\n/private/Meeting/transcript: secret words".into(),
        ),
        ProcessingStage::Transcribing,
        Some("Vulkan"),
        Some("NVIDIA GeForce RTX 4050"),
        12_000,
        60_000,
        Duration::from_millis(2_500),
    );
    assert_eq!(event.to_string(), "processing_failed stage=transcribing kind=engine message=whisper_worker_panicked backend=Vulkan device=\"NVIDIA GeForce RTX 4050\" audio_ms=12000 total_ms=60000 whisper_ms=2500");
    let hostile = LogEvent::processing_result(
        &ProcessingError::Engine("secret Transcript words /home/user/Meeting/audio.flac".into()),
        ProcessingStage::Preparing,
        Some("CPU"),
        None,
        0,
        60_000,
        Duration::ZERO,
    )
    .to_string();
    assert!(hostile.contains("kind=engine message=redacted"));
    for secret in ["secret", "Transcript", "/home", "audio.flac"] {
        assert!(!hostile.contains(secret));
    }
}

#[test]
fn cancellation_has_its_own_log_event() {
    assert_eq!(
        LogEvent::processing_result(&ProcessingError::Cancelled, ProcessingStage::Transcribing, Some("CPU"), None, 500, 1_000, Duration::from_secs(2)).to_string(),
        "processing_cancelled stage=transcribing backend=CPU device=\"none\" audio_ms=500 total_ms=1000 whisper_ms=2000"
    );
}

#[test]
fn known_whisper_errors_keep_a_useful_private_content_free_reason() {
    let event = LogEvent::processing_result(
        &ProcessingError::Engine("Failed to run the decoder.\nTranscript: private words".into()),
        ProcessingStage::Transcribing,
        Some("CPU"),
        None,
        0,
        1_000,
        Duration::ZERO,
    )
    .to_string();
    assert!(event.contains("message=failed_to_run_decoder"));
    assert!(!event.contains("private words"));
    let code = LogEvent::processing_result(
        &ProcessingError::Engine("Generic whisper error. Varies depending on the function. Error code: -6\nC:\\Users\\private\\audio.flac".into()),
        ProcessingStage::Transcribing, Some("CPU"), None, 0, 1_000, Duration::ZERO,
    ).to_string();
    assert!(code.contains("message=whisper_error_code_-6"));
    assert!(!code.contains("C:\\Users"));
}

#[test]
fn diagnostics_shows_last_failure_line_beyond_recent_window() {
    let failure = "[2026-10-13][10:47:02][meeting_minutes_events][INFO] processing_failed stage=transcribing kind=engine message=redacted backend=CPU device=\"none\" audio_ms=500 total_ms=1000 whisper_ms=2000";
    let mut lines = vec![failure.to_owned()];
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
    assert!(text.contains(&format!("Last processing failure: {failure}")));
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
        format!("[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=small backend=CPU device=\"none\" power=unknown threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf={rtf}")
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
    let completed = "[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=small backend=CPU device=\"none\" power=unknown threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250";
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
    assert!(text.starts_with("Meeting Minutes 0.1.0\nWindows:"));
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
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=small backend=CPU device=\"none\" power=unknown threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250 title=private",
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] transcription_completed model=private backend=CPU device=\"none\" power=unknown threads=8 audio_ms=16000 audio_after_vad_ms=none preparation_ms=320 whisper_ms=4000 rtf=0.250",
        "[2026-10-13][10:47:02][meeting_minutes_events][INFO] processing_failed stage=transcribing kind=engine message=redacted backend=Vulkan device=\"C:\\Users\\private\\Transcript text\" audio_ms=500 total_ms=1000 whisper_ms=2000",
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
        LogEvent::CorrectionResult {
            outcome: "completed",
            changed_segments: 3,
            duration_ms: 42,
        },
        LogEvent::processing_result(
            &ProcessingError::NoAudio,
            ProcessingStage::Preparing,
            None,
            None,
            0,
            0,
            Duration::ZERO,
        ),
        LogEvent::processing_result(
            &ProcessingError::Cancelled,
            ProcessingStage::Transcribing,
            Some("CPU"),
            None,
            0,
            1_000,
            Duration::ZERO,
        ),
        LogEvent::VadUnavailable,
        LogEvent::GpuUnavailable,
        LogEvent::GpuLoadFailed,
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
