use meeting_minutes_core::ai::{AiError, GenerateRequest, ProviderKind, TextProvider};
use meeting_minutes_core::processing::{
    transcribe_meeting, transcribe_meeting_named_with_draft,
    transcribe_meeting_resuming_with_correction, CorrectionProvider, SpeechEngine,
    TranscribeRequest, TranscriptionContext,
};
use meeting_minutes_core::processing::{JobStage, ProcessingError, Processor};
use meeting_minutes_core::transcript::{Language, RawSegment, Speaker};
use meeting_minutes_core::{capture::Source, finalize_recording, recording::ChunkWriter, Library};
use meeting_minutes_core::{
    cloud::{CloudEngine, EngineError},
    processing::EngineChain,
};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, Once, OnceLock};
use tempfile::TempDir;

struct FakeCorrection(Result<String, AiError>);
impl TextProvider for FakeCorrection {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gemini
    }
    fn model(&self) -> &str {
        "gemini-2.5-flash"
    }
    fn generate(
        &self,
        _: &GenerateRequest,
        _: &mut dyn FnMut(&str),
        _: &AtomicBool,
    ) -> Result<String, AiError> {
        match &self.0 {
            Ok(response) => Ok(response.clone()),
            Err(_) => Err(AiError::GeminiLimit),
        }
    }
}

#[test]
fn correction_changes_the_revision_after_the_uncorrected_draft() {
    let (_temp, dir) = saved_meeting();
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let provider = FakeCorrection(Ok(
        r#"{"segments":[{"id":"s00001","text":"Corrected speech"}]}"#.into(),
    ));
    let mut draft = vec![];
    let transcript = transcribe_meeting_resuming_with_correction(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
        None,
        &mut |segments| draft.extend(segments.iter().map(|segment| segment.text.clone())),
        TranscriptionContext {
            background: &[],
            correction: CorrectionProvider::Ready(&provider),
        },
    )
    .unwrap();
    assert!(draft.iter().all(|text| text == "Speech"));
    assert_eq!(transcript.segments[0].text, "Corrected speech");
    let published: meeting_minutes_core::transcript::Transcript =
        serde_json::from_slice(&std::fs::read(dir.join("transcript.r1.json")).unwrap()).unwrap();
    assert_eq!(published.segments[0].text, "Corrected speech");
}

#[test]
fn correction_quota_failure_publishes_original_revision_with_notice() {
    let (temp, dir) = saved_meeting();
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let provider = FakeCorrection(Err(AiError::GeminiLimit));
    let transcript = transcribe_meeting_resuming_with_correction(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
        None,
        &mut |_| {},
        TranscriptionContext {
            background: &[],
            correction: CorrectionProvider::Ready(&provider),
        },
    )
    .unwrap();
    assert!(transcript
        .segments
        .iter()
        .all(|segment| segment.text == "Speech"));
    let published: meeting_minutes_core::transcript::Transcript =
        serde_json::from_slice(&std::fs::read(dir.join("transcript.r1.json")).unwrap()).unwrap();
    assert_eq!(published.segments, transcript.segments);
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("meeting.json")).unwrap()).unwrap();
    assert_eq!(
        metadata["correction_notice"],
        "Correction was unavailable; this Revision has uncorrected text."
    );
    let library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let id = dir.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        library.meeting(id).unwrap().correction_notice.as_deref(),
        Some("Correction was unavailable; this Revision has uncorrected text.")
    );
}

struct FakeEngine {
    calls: usize,
    prompts: Vec<Option<String>>,
    detected: usize,
    lengths: Vec<usize>,
}
impl SpeechEngine for FakeEngine {
    fn backend(&self) -> &str {
        "CPU"
    }
    fn thread_count(&self) -> usize {
        1
    }
    fn audio_after_vad_ms(&self) -> Option<u64> {
        Some(40)
    }
    fn detect_language(&mut self, _: &[f32]) -> Result<Language, String> {
        self.detected += 1;
        Ok(Language::Vi)
    }
    fn transcribe(
        &mut self,
        samples: &[f32],
        _: Language,
        prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        self.calls += 1;
        self.prompts.push(prompt.map(str::to_owned));
        self.lengths.push(samples.len());
        progress(1.0);
        Ok(samples
            .iter()
            .any(|sample| sample.abs() > 0.01)
            .then(|| RawSegment {
                start_ms: 0,
                end_ms: 120,
                text: format!(" words {} ", self.calls),
                words: vec![],
            })
            .into_iter()
            .collect())
    }
}

struct EventRecorder;
static EVENT_RECORDER: EventRecorder = EventRecorder;
static EVENTS: OnceLock<Mutex<Vec<(std::thread::ThreadId, String)>>> = OnceLock::new();
static EVENT_RECORDER_INIT: Once = Once::new();
fn record_events() {
    EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    EVENT_RECORDER_INIT.call_once(|| {
        log::set_logger(&EVENT_RECORDER).unwrap();
        log::set_max_level(log::LevelFilter::Info);
    });
}
impl log::Log for EventRecorder {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target() == "meeting_minutes_events"
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            EVENTS
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .push((std::thread::current().id(), record.args().to_string()));
        }
    }
    fn flush(&self) {}
}

#[test]
fn completed_transcription_reports_speech_duration_across_recordings() {
    record_events();
    let (_temp, dir) = saved_meeting();
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let current = std::thread::current().id();
    let events = EVENTS.get().unwrap().lock().unwrap();
    assert!(events.iter().any(|(thread, event)| {
        *thread == current
            && event.starts_with("transcription_completed ")
            && event.contains("audio_ms=400 audio_after_vad_ms=160")
    }));
}

#[test]
fn engine_failure_logs_transcribing_stage_and_completed_audio() {
    struct FailingEngine {
        calls: usize,
    }
    impl SpeechEngine for FailingEngine {
        fn backend(&self) -> &str {
            "Vulkan"
        }
        fn device(&self) -> Option<&str> {
            Some("NVIDIA GeForce RTX 4050")
        }
        fn thread_count(&self) -> usize {
            1
        }
        fn detect_language(&mut self, _: &[f32]) -> Result<Language, String> {
            Ok(Language::En)
        }
        fn transcribe(
            &mut self,
            _: &[f32],
            _: Language,
            _: Option<&str>,
            progress: &mut dyn FnMut(f32),
            _: &AtomicBool,
        ) -> Result<Vec<RawSegment>, ProcessingError> {
            self.calls += 1;
            if self.calls == 2 {
                progress(0.5);
                Err(ProcessingError::Engine(
                    "Whisper worker panicked\n/private/Meeting/audio.flac secret Transcript text"
                        .into(),
                ))
            } else {
                Ok(vec![])
            }
        }
    }
    record_events();
    let (_temp, dir) = saved_meeting();
    let mut engine = FailingEngine { calls: 0 };
    let result = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(ProcessingError::Engine(_))));
    let current = std::thread::current().id();
    let events = EVENTS.get().unwrap().lock().unwrap();
    let event = events
        .iter()
        .rev()
        .find(|(thread, event)| *thread == current && event.starts_with("processing_failed "))
        .unwrap()
        .1
        .as_str();
    assert!(event.contains("stage=transcribing kind=engine message=whisper_worker_panicked backend=Vulkan device=\"NVIDIA GeForce RTX 4050\" audio_ms=150 total_ms=400 whisper_ms="), "{event}");
    assert!(!event.contains("/private"));
    assert!(!event.contains("secret Transcript text"));
}

#[test]
fn cancelled_transcription_logs_cancellation() {
    record_events();
    let (_temp, dir) = saved_meeting();
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    let result = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: None,
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(true),
    );
    assert!(matches!(result, Err(ProcessingError::Cancelled)));
    let current = std::thread::current().id();
    let events = EVENTS.get().unwrap().lock().unwrap();
    assert!(events.iter().any(|(thread, event)| *thread == current
        && event.starts_with("processing_cancelled stage=preparing ")));
}

fn saved_meeting() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap();
    let (meeting, first, _) = library.begin_recording(at).unwrap();
    let dir = library.meeting_dir(&meeting.id);
    for source in [Source::Microphone, Source::SystemAudio] {
        let mut writer = ChunkWriter::new(&first, source).unwrap();
        let frames = if source == Source::Microphone {
            4_800
        } else {
            9_600
        };
        writer
            .write(&vec![
                if source == Source::Microphone {
                    0.2
                } else {
                    0.0
                };
                frames
            ])
            .unwrap();
        writer.finish().unwrap();
    }
    finalize_recording(&dir, 1).unwrap();
    let (second, _) = library.begin_next_recording(&meeting.id, at).unwrap();
    for source in [Source::Microphone, Source::SystemAudio] {
        let mut writer = ChunkWriter::new(&second, source).unwrap();
        writer
            .write(&vec![
                if source == Source::SystemAudio {
                    0.2
                } else {
                    0.0
                };
                if source == Source::Microphone {
                    4_800
                } else {
                    9_600
                }
            ])
            .unwrap();
        writer.finish().unwrap();
    }
    finalize_recording(&dir, 2).unwrap();
    (temp, dir)
}

#[test]
fn cloud_quota_mid_meeting_publishes_one_revision_with_both_engines() {
    record_events();
    struct QuotaCloud {
        calls: usize,
    }
    impl CloudEngine for QuotaCloud {
        fn name(&self) -> &str {
            "Groq"
        }
        fn transcribe_span(
            &mut self,
            _: &[f32],
            _: Language,
            _: Option<&str>,
            _: &AtomicBool,
        ) -> Result<Vec<RawSegment>, EngineError> {
            self.calls += 1;
            if self.calls == 2 {
                Err(EngineError::QuotaExhausted)
            } else {
                Ok(vec![])
            }
        }
    }
    let (_temp, dir) = saved_meeting();
    let mut engine = EngineChain::new(
        vec![Box::new(QuotaCloud { calls: 0 })],
        "small",
        Box::new(|| {
            Ok(Box::new(FakeEngine {
                calls: 0,
                prompts: vec![],
                detected: 0,
                lengths: vec![],
            }))
        }),
    );
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(transcript.revision, 1);
    assert_eq!(transcript.model, vec!["Groq", "small"]);
    assert_eq!(
        transcript.fallback_reason.as_deref(),
        Some("Groq daily limit reached")
    );
    assert!(dir.join("transcript.r1.json").exists());
    assert!(!dir.join("transcript.r2.json").exists());
    let current = std::thread::current().id();
    assert!(EVENTS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|(thread, event)| *thread == current
            && event.starts_with("engine_usage groq_ms=")
            && event.contains("fallback=quota")));
}

struct AudibleEngine {
    calls: Vec<(usize, f32)>,
    detected: usize,
    detected_peak: f32,
}

impl SpeechEngine for AudibleEngine {
    fn backend(&self) -> &str {
        "CPU"
    }
    fn thread_count(&self) -> usize {
        1
    }
    fn audio_after_vad_ms(&self) -> Option<u64> {
        Some(60)
    }
    fn detect_language(&mut self, samples: &[f32]) -> Result<Language, String> {
        self.detected += 1;
        self.detected_peak = samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        Ok(Language::En)
    }
    fn transcribe(
        &mut self,
        samples: &[f32],
        _: Language,
        _: Option<&str>,
        progress: &mut dyn FnMut(f32),
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        let peak = samples
            .iter()
            .copied()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        self.calls.push((samples.len(), peak));
        progress(1.0);
        Ok((peak > 0.01)
            .then(|| RawSegment {
                start_ms: 0,
                end_ms: 80,
                text: "Speech".into(),
                words: vec![],
            })
            .into_iter()
            .collect())
    }
}

#[test]
fn each_source_is_transcribed_and_labelled_on_the_meeting_timeline() {
    record_events();
    let (_temp, dir) = saved_meeting();
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let mut progress = vec![];
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: None,
        },
        &mut engine,
        &mut |p| progress.push(p),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        transcript
            .segments
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.speaker))
            .collect::<Vec<_>>(),
        vec![(0, 80, Speaker::Me), (100, 180, Speaker::Remote)]
    );
    assert_eq!(engine.calls.len(), 4);
    assert_eq!(engine.detected, 1);
    assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(progress.last(), Some(&1.0));
    let thread = std::thread::current().id();
    assert!(EVENTS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .any(|(id, event)| *id == thread
            && event.starts_with("transcription_completed ")
            && event.contains("audio_ms=400 audio_after_vad_ms=240")));
}

#[test]
fn simultaneous_sources_keep_overlapping_segments_with_distinct_speakers() {
    let (_temp, dir) = meeting_with_sources(0.2, 0.2);
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        transcript
            .segments
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.speaker))
            .collect::<Vec<_>>(),
        vec![(0, 80, Speaker::Me), (0, 80, Speaker::Remote)]
    );
}

#[test]
fn draft_callback_reports_placed_segments_per_source_with_bounded_progress() {
    let (_temp, dir) = saved_meeting();
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let mut batches = vec![];
    let mut progress = vec![];
    let transcript = transcribe_meeting_named_with_draft(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |fraction| progress.push(fraction),
        &AtomicBool::new(false),
        None,
        &mut |segments| batches.push(segments.to_vec()),
    )
    .unwrap();
    assert_eq!(batches.len(), 2);
    assert_eq!(
        batches[0]
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.speaker))
            .collect::<Vec<_>>(),
        vec![(0, 80, Speaker::Me)]
    );
    assert_eq!(
        batches[1]
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.speaker))
            .collect::<Vec<_>>(),
        vec![(100, 180, Speaker::Remote)]
    );
    assert_eq!(transcript.segments.len(), 2);
    assert!(progress
        .iter()
        .all(|fraction| (0.0..=1.0).contains(fraction)));
}

fn meeting_with_sources(mic: f32, system: f32) -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap();
    let (meeting, recording, _) = library.begin_recording(at).unwrap();
    let dir = library.meeting_dir(&meeting.id);
    for source in [Source::Microphone, Source::SystemAudio] {
        let mut writer = ChunkWriter::new(&recording, source).unwrap();
        writer
            .write(&vec![
                if source == Source::Microphone {
                    mic
                } else {
                    system
                };
                if source == Source::Microphone {
                    4_800
                } else {
                    9_600
                }
            ])
            .unwrap();
        writer.finish().unwrap();
    }
    finalize_recording(&dir, 1).unwrap();
    (temp, dir)
}

#[test]
fn language_detection_uses_system_audio_when_microphone_has_no_speech() {
    let (_temp, dir) = meeting_with_sources(0.0, 0.2);
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: None,
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(engine.detected, 1);
    assert!(engine.detected_peak > 0.01);
    assert_eq!(transcript.segments.len(), 1);
    assert_eq!(transcript.segments[0].speaker, Speaker::Remote);
}

#[test]
fn silent_sources_do_not_claim_a_detected_language() {
    let (_temp, dir) = meeting_with_sources(0.0, 0.0);
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: None,
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(engine.detected, 0);
    assert!(!transcript.language_detected);
    assert!(transcript.segments.is_empty());
}

#[test]
fn voice_note_segments_are_me_when_only_microphone_source_is_present() {
    let (_temp, dir) = saved_meeting();
    let path = dir.join("meeting.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for recording in metadata["recordings"].as_array_mut().unwrap() {
        recording["files"]
            .as_array_mut()
            .unwrap()
            .retain(|file| file["source"] == "microphone");
    }
    std::fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    let mut engine = AudibleEngine {
        calls: vec![],
        detected: 0,
        detected_peak: 0.0,
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(transcript.segments.len(), 1);
    assert_eq!(transcript.segments[0].speaker, Speaker::Me);
    assert_eq!(engine.calls.len(), 2);
}

#[test]
fn transcribes_two_recordings_on_timeline_and_creates_revisions() {
    let (_temp, dir) = saved_meeting();
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    let vocabulary = vec!["  Nguyễn  ".into(), " ".into(), "Project X".into()];
    let request = TranscribeRequest {
        meeting_dir: &dir,
        model: "small",
        vocabulary: &vocabulary,
        default_language: None,
    };
    let first =
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.language, Language::Vi);
    assert!(first.language_detected);
    assert_eq!(engine.detected, 1);
    assert_eq!(engine.prompts, vec![Some("Nguyễn, Project X".into()); 4]);
    assert_eq!(engine.lengths, vec![1600; 4]);
    assert_eq!(
        first
            .segments
            .iter()
            .map(|s| (s.id.as_str(), s.start_ms, s.end_ms))
            .collect::<Vec<_>>(),
        vec![("s00001", 0, 100), ("s00002", 100, 200)]
    );
    assert_eq!(
        first.segments[0].speaker,
        meeting_minutes_core::transcript::Speaker::Me
    );
    assert_eq!(
        first.segments[1].speaker,
        meeting_minutes_core::transcript::Speaker::Remote
    );
    assert!(std::fs::read_to_string(dir.join("transcript.md"))
        .unwrap()
        .contains("Transcript revision 1 · model small · Vietnamese"));
    let library = Library::open(
        _temp.path().join("Library"),
        _temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let id = dir.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        library.transcript(id).unwrap().unwrap().segments,
        first.segments
    );
    library
        .assign_segments(id, 1, &["s00001".into()], Some("Anna"))
        .unwrap();
    let second =
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(second.revision, 2);
    assert!(dir.join("transcript.r1.json").exists());
    assert_eq!(
        library.transcript(id).unwrap().unwrap().segments,
        second.segments
    );
    assert!(std::fs::read_to_string(dir.join("transcript.md"))
        .unwrap()
        .contains("Transcript revision 2"));
    assert!(std::fs::read_to_string(dir.join("transcript.md"))
        .unwrap()
        .contains("Anna:** words"));
    let carried = library.meeting(id).unwrap().segment_participants.unwrap();
    assert_eq!(carried.revision, 2);
    assert_eq!(
        carried.segments.get("s00001").map(String::as_str),
        Some("Anna")
    );
    // Moves made on an older Revision still carry when that Revision is not the previous file.
    let mut moved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("transcript.r2.json")).unwrap()).unwrap();
    moved["revision"] = serde_json::json!(5);
    std::fs::write(
        dir.join("transcript.r5.json"),
        serde_json::to_vec(&moved).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(dir.join("transcript.r2.json")).unwrap();
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("meeting.json")).unwrap()).unwrap();
    metadata["segment_participants"]["revision"] = serde_json::json!(5);
    std::fs::write(
        dir.join("meeting.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    let third =
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(third.revision, 6);
    let carried = library.meeting(id).unwrap().segment_participants.unwrap();
    assert_eq!(carried.revision, 6);
    assert_eq!(
        carried.segments.get("s00001").map(String::as_str),
        Some("Anna")
    );
}

#[test]
fn stale_json_temp_is_not_a_revision_and_existing_revision_is_preserved() {
    let (temp, dir) = saved_meeting();
    std::fs::write(dir.join("transcript.r1.json.tmp"), b"partial").unwrap();
    let library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let id = dir.file_name().unwrap().to_str().unwrap();
    assert!(library.transcript(id).unwrap().is_none());
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    let request = TranscribeRequest {
        meeting_dir: &dir,
        model: "small",
        vocabulary: &[],
        default_language: None,
    };
    let first =
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(first.revision, 1);
    let original = std::fs::read(dir.join("transcript.r1.json")).unwrap();
    let second =
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(second.revision, 2);
    assert_eq!(
        std::fs::read(dir.join("transcript.r1.json")).unwrap(),
        original
    );
    assert!(!dir.join("transcript.r2.json.tmp").exists());
}

#[test]
fn chosen_meeting_language_skips_detection() {
    let (temp, dir) = saved_meeting();
    let id = dir.file_name().unwrap().to_str().unwrap();
    let library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    library.set_language(id, Some(Language::Vi)).unwrap();
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: None,
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(engine.detected, 0);
    assert_eq!(transcript.language, Language::Vi);
    assert!(!transcript.language_detected);
}

#[test]
fn default_language_from_settings_skips_detection() {
    let (_temp, dir) = saved_meeting();
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(engine.detected, 0);
    assert_eq!(transcript.language, Language::En);
    assert!(!transcript.language_detected);
}

#[test]
fn meeting_language_wins_over_default_language() {
    let (temp, dir) = saved_meeting();
    let id = dir.file_name().unwrap().to_str().unwrap();
    let library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    library.set_language(id, Some(Language::Vi)).unwrap();
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(engine.detected, 0);
    assert_eq!(transcript.language, Language::Vi);
}

#[test]
fn cancellation_and_no_audio_do_not_write_a_transcript() {
    let (_temp, dir) = saved_meeting();
    let mut engine = FakeEngine {
        calls: 0,
        prompts: vec![],
        detected: 0,
        lengths: vec![],
    };
    let request = TranscribeRequest {
        meeting_dir: &dir,
        model: "small",
        vocabulary: &[],
        default_language: None,
    };
    assert!(matches!(
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(true)),
        Err(ProcessingError::Cancelled)
    ));
    assert!(!dir.join("transcript.r1.json").exists());
    struct CancelEngine;
    impl SpeechEngine for CancelEngine {
        fn backend(&self) -> &str {
            "CPU"
        }
        fn thread_count(&self) -> usize {
            1
        }
        fn detect_language(&mut self, _: &[f32]) -> Result<Language, String> {
            Ok(Language::En)
        }
        fn transcribe(
            &mut self,
            _: &[f32],
            _: Language,
            _: Option<&str>,
            _: &mut dyn FnMut(f32),
            cancel: &AtomicBool,
        ) -> Result<Vec<RawSegment>, ProcessingError> {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            Err(ProcessingError::Cancelled)
        }
    }
    assert!(matches!(
        transcribe_meeting(
            &request,
            &mut CancelEngine,
            &mut |_| {},
            &AtomicBool::new(false)
        ),
        Err(ProcessingError::Cancelled)
    ));
    assert!(!dir.join("transcript.r1.json").exists());
    let temp = TempDir::new().unwrap();
    std::fs::write(temp.path().join("meeting.json"), r#"{"schema_version":1,"title":"Empty","started_at":"2026-10-12T09:30:00Z","language":null,"recordings":[]}"#).unwrap();
    let request = TranscribeRequest {
        meeting_dir: temp.path(),
        model: "small",
        vocabulary: &[],
        default_language: None,
    };
    assert!(matches!(
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)),
        Err(ProcessingError::NoAudio)
    ));
}

struct SignalEngine {
    samples: Vec<f32>,
    on_transcribe: Option<Box<dyn FnMut()>>,
}
impl SpeechEngine for SignalEngine {
    fn backend(&self) -> &str {
        "CPU"
    }
    fn thread_count(&self) -> usize {
        1
    }
    fn detect_language(&mut self, _: &[f32]) -> Result<Language, String> {
        Ok(Language::En)
    }
    fn transcribe(
        &mut self,
        samples: &[f32],
        _: Language,
        _: Option<&str>,
        _: &mut dyn FnMut(f32),
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        if let Some(rename) = &mut self.on_transcribe {
            rename();
        }
        if samples.iter().any(|sample| sample.abs() > 0.01) {
            self.samples = samples.to_vec();
        }
        Ok(vec![])
    }
}

#[test]
fn transcription_completes_without_vad_and_reports_no_speech_duration() {
    record_events();
    let (_temp, dir) = saved_meeting();
    let mut engine = SignalEngine {
        samples: Vec::new(),
        on_transcribe: None,
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(transcript.segments.is_empty());
    let current = std::thread::current().id();
    let events = EVENTS.get().unwrap().lock().unwrap();
    assert!(events.iter().any(|(thread, event)| {
        *thread == current
            && event.starts_with("transcription_completed ")
            && event.contains("audio_ms=400 audio_after_vad_ms=none")
    }));
}

#[test]
fn rename_during_transcription_is_the_published_markdown_title() {
    let (temp, dir) = saved_meeting();
    let id = dir.file_name().unwrap().to_str().unwrap().to_owned();
    let mut writer = Library::open(
        temp.path().join("Library"),
        temp.path().join("rename-index.sqlite3"),
    )
    .unwrap();
    let mut engine = SignalEngine {
        samples: Vec::new(),
        on_transcribe: Some(Box::new(move || {
            writer.rename_meeting(&id, "Review").unwrap();
        })),
    };
    transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: Some(Language::En),
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(std::fs::read_to_string(dir.join("transcript.md"))
        .unwrap()
        .starts_with("# Review\n"));
}

fn prepared_tone(hz: f32) -> Vec<f32> {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap();
    let (meeting, chunks, _) = library.begin_recording(at).unwrap();
    let frames = 4_803;
    let mut mic = ChunkWriter::new(&chunks, Source::Microphone).unwrap();
    mic.write(
        &(0..frames)
            .flat_map(|i| {
                let sample = (std::f32::consts::TAU * hz * i as f32 / 48_000.0).sin() * 0.5;
                [sample, sample]
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    mic.finish().unwrap();
    let mut sys = ChunkWriter::new(&chunks, Source::SystemAudio).unwrap();
    sys.write(&vec![0.0; frames * 2]).unwrap();
    sys.finish().unwrap();
    let dir = library.meeting_dir(&meeting.id);
    finalize_recording(&dir, 1).unwrap();
    library
        .set_language(&meeting.id, Some(Language::En))
        .unwrap();
    let mut engine = SignalEngine {
        samples: vec![],
        on_transcribe: None,
    };
    transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
            default_language: None,
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    engine.samples
}

#[test]
fn preparation_decimates_and_rejects_out_of_band_tone() {
    let low = prepared_tone(1_000.0);
    let high = prepared_tone(10_000.0);
    assert_eq!(low.len(), 1601);
    assert_eq!(high.len(), 1601);
    let rms = |samples: &[f32]| {
        (samples[100..].iter().map(|s| s * s).sum::<f32>() / (samples.len() - 100) as f32).sqrt()
    };
    assert!(rms(&low) > 0.2);
    assert!(
        rms(&high) < rms(&low) / 10.0,
        "10 kHz must be attenuated by at least 20 dB"
    );
}

#[test]
fn queue_runs_one_job_and_retries_failures() {
    let mut queue = Processor::new();
    assert!(queue.enqueue("a"));
    assert!(!queue.enqueue("a"));
    assert!(queue.enqueue("b"));
    assert_eq!(queue.next().as_deref(), Some("a"));
    assert!(queue.is_running());
    queue.set_stage("a", JobStage::Transcribing, 0.5);
    assert_eq!(queue.status().jobs[0].progress, 0.5);
    assert_eq!(queue.next(), None);
    queue.finished("a", Err(ProcessingError::Engine("failed".into())));
    assert!(matches!(queue.status().jobs[0].stage, JobStage::Failed));
    assert!(queue.enqueue("a"));
    assert_eq!(queue.next().as_deref(), Some("b"));
    queue.cancel("b");
    assert!(queue
        .cancel_flag()
        .load(std::sync::atomic::Ordering::SeqCst));
    queue.finished("b", Err(ProcessingError::Cancelled));
    assert!(!queue.is_busy("b"));
    assert_eq!(queue.next().as_deref(), Some("a"));
    queue.finished("a", Ok(()));
    assert!(!queue.is_running());
    assert!(queue.status().jobs.is_empty());
}

#[test]
fn running_job_can_be_queued_for_one_more_revision() {
    let mut queue = Processor::new();
    assert!(queue.enqueue("a"));
    assert_eq!(queue.next().as_deref(), Some("a"));
    queue.set_stage("a", JobStage::Transcribing, 0.6);
    assert!(queue.enqueue("a"));
    assert!(!queue.enqueue("a"));
    queue.finished("a", Ok(()));
    assert!(queue.is_busy("a"));
    assert_eq!(queue.status().jobs[0].stage, JobStage::Queued);
    assert_eq!(queue.status().jobs[0].progress, 0.0);
    assert_eq!(queue.next().as_deref(), Some("a"));
    queue.finished("a", Ok(()));
    assert!(queue.status().jobs.is_empty());
}

#[test]
fn queued_and_running_reruns_use_the_latest_model_choice() {
    let mut queue = Processor::new();
    assert!(queue.enqueue_with_model("a", Some("base")));
    assert!(!queue.enqueue_with_model("a", Some("small")));
    assert_eq!(queue.job_model("a").as_deref(), Some("small"));
    assert_eq!(queue.next().as_deref(), Some("a"));
    assert!(queue.enqueue_with_model("a", Some("medium")));
    assert!(!queue.enqueue_with_model("a", Some("large-v3-turbo")));
    queue.finished("a", Ok(()));
    assert_eq!(queue.job_model("a").as_deref(), Some("large-v3-turbo"));
    assert_eq!(queue.next().as_deref(), Some("a"));
    queue.finished("a", Ok(()));
    assert!(queue.enqueue("b"));
    assert_eq!(queue.job_model("b"), None);
}

#[test]
fn cancellation_and_failure_clear_a_running_rerun() {
    let mut queue = Processor::new();
    queue.enqueue("a");
    queue.next();
    queue.set_stage("a", JobStage::Transcribing, 0.4);
    queue.enqueue("a");
    queue.finished("a", Err(ProcessingError::Cancelled));
    assert!(queue.status().jobs.is_empty());
    queue.enqueue("a");
    queue.next();
    queue.set_stage("a", JobStage::Transcribing, 0.4);
    queue.enqueue("a");
    queue.finished("a", Err(ProcessingError::Engine("failed".into())));
    assert_eq!(queue.status().jobs[0].stage, JobStage::Failed);
}
