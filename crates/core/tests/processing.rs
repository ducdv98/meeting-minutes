use meeting_minutes_core::processing::{transcribe_meeting, SpeechEngine, TranscribeRequest};
use meeting_minutes_core::processing::{JobStage, ProcessingError, Processor};
use meeting_minutes_core::transcript::{Language, RawSegment};
use meeting_minutes_core::{capture::Source, finalize_recording, recording::ChunkWriter, Library};
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

struct FakeEngine {
    calls: usize,
    prompts: Vec<Option<String>>,
    detected: usize,
    lengths: Vec<usize>,
}
impl SpeechEngine for FakeEngine {
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
        Ok(vec![RawSegment {
            start_ms: 0,
            end_ms: 120,
            text: format!(" words {} ", self.calls),
        }])
    }
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
    };
    let first =
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.language, Language::Vi);
    assert!(first.language_detected);
    assert_eq!(engine.detected, 1);
    assert_eq!(engine.prompts, vec![Some("Nguyễn, Project X".into()); 2]);
    assert_eq!(engine.lengths, vec![1600; 2]);
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
    };
    assert!(matches!(
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(true)),
        Err(ProcessingError::Cancelled)
    ));
    assert!(!dir.join("transcript.r1.json").exists());
    struct CancelEngine;
    impl SpeechEngine for CancelEngine {
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
    };
    assert!(matches!(
        transcribe_meeting(&request, &mut engine, &mut |_| {}, &AtomicBool::new(false)),
        Err(ProcessingError::NoAudio)
    ));
}

struct SignalEngine {
    samples: Vec<f32>,
}
impl SpeechEngine for SignalEngine {
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
        self.samples = samples.to_vec();
        Ok(vec![])
    }
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
    let mut engine = SignalEngine { samples: vec![] };
    transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &dir,
            model: "small",
            vocabulary: &[],
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
