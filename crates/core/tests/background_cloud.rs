use meeting_minutes_core::{
    background::{self, BackgroundControl, BackgroundTranscriber},
    capture::Source,
    cloud::{CloudEngine, EngineError},
    finalize_recording,
    processing::{
        background_draft, transcribe_meeting_resuming, EngineChain, ProcessingError, SpeechEngine,
        TranscribeRequest,
    },
    recording::ChunkWriter,
    transcript::{EngineName, Language, RawSegment, RawWord, Speaker},
    Library,
};
use std::{
    cell::Cell,
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct FakeGroq {
    replies: VecDeque<Result<(), EngineError>>,
    detections: VecDeque<Result<Option<Language>, EngineError>>,
    calls: Arc<AtomicUsize>,
    samples: Arc<AtomicUsize>,
}

impl CloudEngine for FakeGroq {
    fn name(&self) -> &str {
        "Groq"
    }

    fn transcribe_span(
        &mut self,
        audio: &[f32],
        _: Language,
        _: Option<&str>,
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.samples.fetch_add(audio.len(), Ordering::SeqCst);
        self.replies.pop_front().unwrap_or(Ok(()))?;
        if audio.iter().all(|sample| sample.abs() < 0.01) {
            return Ok(vec![]);
        }
        Ok(vec![RawSegment {
            start_ms: 1_000,
            end_ms: 1_400,
            text: "hello.".into(),
            words: vec![RawWord {
                start_ms: 1_000,
                end_ms: 1_400,
                text: "hello.".into(),
            }],
        }])
    }

    fn detect_language(
        &mut self,
        _: &[f32],
        _: &AtomicBool,
    ) -> Result<Option<Language>, EngineError> {
        self.detections
            .pop_front()
            .unwrap_or(Ok(Some(Language::En)))
    }
}

struct Local;
impl SpeechEngine for Local {
    fn backend(&self) -> &str {
        "CPU"
    }
    fn thread_count(&self) -> usize {
        1
    }
    fn detect_language(&mut self, _: &[f32], _: &AtomicBool) -> Result<Language, String> {
        Ok(Language::En)
    }
    fn transcribe(
        &mut self,
        _: &[f32],
        _: Language,
        _: Option<&str>,
        _: &mut dyn FnMut(f32),
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        Ok(vec![])
    }
}

fn run_recording(replies: Vec<Result<(), EngineError>>) -> (usize, usize, u64, Vec<Duration>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let local_loads = loads.clone();
    let mut chain = EngineChain::new(
        vec![Box::new(FakeGroq {
            replies: replies.into(),
            detections: VecDeque::new(),
            calls: calls.clone(),
            samples: Arc::new(AtomicUsize::new(0)),
        })],
        "small",
        Box::new(move || {
            local_loads.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(Local))
        }),
    );
    chain.set_background_local_allowed(Box::new(|| false));
    let mut chain = Some(chain);
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    transcriber.push(Source::Microphone, &vec![0.1; 35 * 48_000]);
    let directory = tempfile::tempdir().unwrap();
    let stop = AtomicBool::new(false);
    let now = Cell::new(Duration::ZERO);
    let mut call_times = vec![];
    background::run(
        &mut transcriber,
        &mut || chain.take(),
        directory.path(),
        BackgroundControl {
            stop: &stop,
            allowed: &mut || true,
            now: &mut || now.get(),
            wait: &mut || {
                call_times.push(now.get());
                now.set(now.get() + Duration::from_secs(1));
                if now.get() > Duration::from_secs(15) {
                    stop.store(true, Ordering::SeqCst);
                }
            },
        },
        &mut |segments| {
            if !segments.is_empty() {
                stop.store(true, Ordering::SeqCst);
            }
        },
    )
    .unwrap();
    let committed = transcriber
        .finish()
        .source(Speaker::Me)
        .map_or(0, |source| source.committed_ms);
    (
        calls.load(Ordering::SeqCst),
        loads.load(Ordering::SeqCst),
        committed,
        call_times,
    )
}

#[test]
fn groq_transcribes_during_a_recording_on_an_integrated_gpu_and_battery() {
    let (calls, loads, committed, _) = run_recording(vec![Ok(())]);
    assert_eq!(calls, 1);
    assert_eq!(loads, 0);
    assert_eq!(committed, 35_000);
}

#[test]
fn offline_cloud_work_pauses_then_resumes_after_backoff() {
    let (calls, loads, committed, waits) = run_recording(vec![Err(EngineError::Offline), Ok(())]);
    assert_eq!(calls, 2);
    assert_eq!(loads, 0);
    assert_eq!(committed, 35_000);
    assert!(waits.len() >= 5);
}

#[test]
fn offline_during_meeting_language_detection_also_resumes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let local_loads = Arc::new(AtomicUsize::new(0));
    let load_count = local_loads.clone();
    let mut engine = EngineChain::new(
        vec![Box::new(FakeGroq {
            replies: vec![Ok(())].into(),
            detections: vec![Err(EngineError::Offline), Ok(Some(Language::En))].into(),
            calls: calls.clone(),
            samples: Arc::new(AtomicUsize::new(0)),
        })],
        "small",
        Box::new(move || {
            load_count.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(Local))
        }),
    );
    engine.set_background_local_allowed(Box::new(|| false));
    let mut engine = Some(engine);
    let mut transcriber = BackgroundTranscriber::new(1, "small", None, None);
    transcriber.push(Source::Microphone, &vec![0.1; 35 * 48_000]);
    let directory = tempfile::tempdir().unwrap();
    let stop = AtomicBool::new(false);
    let now = Cell::new(Duration::ZERO);
    background::run(
        &mut transcriber,
        &mut || engine.take(),
        directory.path(),
        BackgroundControl {
            stop: &stop,
            allowed: &mut || true,
            now: &mut || now.get(),
            wait: &mut || {
                now.set(now.get() + Duration::from_secs(1));
                if now.get() > Duration::from_secs(15) {
                    stop.store(true, Ordering::SeqCst);
                }
            },
        },
        &mut |segments| {
            if !segments.is_empty() {
                stop.store(true, Ordering::SeqCst);
            }
        },
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(local_loads.load(Ordering::SeqCst), 0);
    assert!(now.get() >= Duration::from_secs(5));
    assert_eq!(transcriber.finish().language, Some(Language::En));
}

#[test]
fn stop_during_language_detection_ends_like_stop_during_span_transcription() {
    struct StopCloud(Arc<AtomicBool>);
    impl CloudEngine for StopCloud {
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
            self.0.store(true, Ordering::SeqCst);
            Err(EngineError::Cancelled)
        }
        fn detect_language(
            &mut self,
            _: &[f32],
            _: &AtomicBool,
        ) -> Result<Option<Language>, EngineError> {
            self.0.store(true, Ordering::SeqCst);
            Err(EngineError::Cancelled)
        }
    }
    for language in [None, Some(Language::En)] {
        let stop = Arc::new(AtomicBool::new(false));
        let mut transcriber = BackgroundTranscriber::new(1, "small", language, None);
        transcriber.push(Source::Microphone, &vec![0.1; 35 * 48_000]);
        let mut engine = Some(EngineChain::new(
            vec![Box::new(StopCloud(stop.clone()))],
            "small",
            Box::new(|| Ok(Box::new(Local))),
        ));
        let directory = tempfile::tempdir().unwrap();
        let result = background::run(
            &mut transcriber,
            &mut || engine.take(),
            directory.path(),
            BackgroundControl {
                stop: &stop,
                allowed: &mut || true,
                now: &mut || Duration::ZERO,
                wait: &mut || {},
            },
            &mut |_| {},
        );
        assert!(result.is_ok(), "stop during {language:?}: {result:?}");
        assert_eq!(transcriber.finish().sources[0].committed_ms, 0);
    }
}

#[test]
fn stop_reuses_cloud_segments_and_transcribes_only_the_remainder() {
    let temp = tempfile::tempdir().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-13T09:30:00Z").unwrap();
    let (meeting, chunk_dir, _) = library.begin_recording(at).unwrap();
    let meeting_dir = library.meeting_dir(&meeting.id);
    let audio = vec![0.1; 70 * 48_000];
    let mut writer = ChunkWriter::new(&chunk_dir, Source::Microphone).unwrap();
    writer
        .write(
            &audio
                .iter()
                .flat_map(|&sample| [sample, sample])
                .collect::<Vec<_>>(),
        )
        .unwrap();
    writer.finish().unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    let samples = Arc::new(AtomicUsize::new(0));
    let cloud = || {
        Box::new(FakeGroq {
            replies: vec![Ok(())].into(),
            detections: VecDeque::new(),
            calls: calls.clone(),
            samples: samples.clone(),
        }) as Box<dyn CloudEngine>
    };
    let mut engine = EngineChain::new(vec![cloud()], "small", Box::new(|| Ok(Box::new(Local))));
    engine.set_background_local_allowed(Box::new(|| false));
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    transcriber.push(Source::Microphone, &audio[..35 * 48_000]);
    transcriber.push(Source::SystemAudio, &vec![0.0; 35 * 48_000]);
    transcriber
        .transcribe_ready(&mut engine, &AtomicBool::new(false))
        .unwrap();
    let cache = transcriber.finish();
    assert_eq!(cache.source(Speaker::Me).unwrap().committed_ms, 35_000);
    finalize_recording(&meeting_dir, 1).unwrap();

    let request = TranscribeRequest {
        meeting_dir: &meeting_dir,
        model: "small",
        vocabulary: &[],
        default_language: Some(Language::En),
    };
    assert!(!background_draft(&request, std::slice::from_ref(&cache))
        .unwrap()
        .is_empty());
    samples.store(0, Ordering::SeqCst);
    let mut after_stop = EngineChain::new(vec![cloud()], "small", Box::new(|| Ok(Box::new(Local))));
    let transcript = transcribe_meeting_resuming(
        &request,
        &mut after_stop,
        &mut |_| {},
        &AtomicBool::new(false),
        None,
        &mut |_| {},
        std::slice::from_ref(&cache),
    )
    .unwrap();
    assert!(!transcript.segments.is_empty());
    assert_eq!(transcript.model, vec![EngineName::Groq]);
    let after_stop_samples = samples.load(Ordering::SeqCst);
    assert!(after_stop_samples >= 68 * 16_000, "{after_stop_samples}");
    assert!(after_stop_samples <= 72 * 16_000, "{after_stop_samples}");
}
