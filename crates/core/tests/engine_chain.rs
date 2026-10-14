use meeting_minutes_core::{
    capture::Source,
    cloud::{CloudEngine, EngineError},
    diagnostics::LogEvent,
    processing::{EngineChain, ProcessingError, SpeechEngine},
    transcript::{EngineName, FallbackReason, Language, RawSegment},
};
use std::{
    cell::Cell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct FakeCloud {
    name: &'static str,
    calls: Arc<AtomicUsize>,
    replies: VecDeque<Result<Vec<RawSegment>, EngineError>>,
}

type CallLog = Arc<std::sync::Mutex<Vec<(Option<Source>, Language)>>>;

struct TrackingCloud {
    name: &'static str,
    replies: VecDeque<Result<Vec<RawSegment>, EngineError>>,
    detected: Option<Language>,
    source: Option<Source>,
    calls: CallLog,
    detections: Arc<AtomicUsize>,
}

fn call_log() -> CallLog {
    Arc::new(std::sync::Mutex::new(Vec::new()))
}

impl CloudEngine for TrackingCloud {
    fn name(&self) -> &str {
        self.name
    }
    fn set_source(&mut self, source: Option<Source>) {
        self.source = source;
    }
    fn detect_language(
        &mut self,
        _: &[f32],
        _: &AtomicBool,
    ) -> Result<Option<Language>, EngineError> {
        self.detections.fetch_add(1, Ordering::SeqCst);
        Ok(self.detected)
    }
    fn transcribe_span(
        &mut self,
        _: &[f32],
        language: Language,
        _: Option<&str>,
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        self.calls.lock().unwrap().push((self.source, language));
        self.replies.pop_front().unwrap_or(Ok(vec![]))
    }
}

fn tracking_cloud(
    name: &'static str,
    replies: Vec<Result<Vec<RawSegment>, EngineError>>,
    detected: Option<Language>,
    calls: CallLog,
    detections: Arc<AtomicUsize>,
) -> Box<dyn CloudEngine> {
    Box::new(TrackingCloud {
        name,
        replies: replies.into(),
        detected,
        source: None,
        calls,
        detections,
    })
}
impl CloudEngine for FakeCloud {
    fn name(&self) -> &str {
        self.name
    }
    fn transcribe_span(
        &mut self,
        _: &[f32],
        _: Language,
        _: Option<&str>,
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.replies.pop_front().unwrap_or(Ok(vec![]))
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
        progress: &mut dyn FnMut(f32),
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        progress(1.0);
        Ok(vec![])
    }
}
fn cloud(
    replies: Vec<Result<Vec<RawSegment>, EngineError>>,
    calls: Arc<AtomicUsize>,
) -> Box<dyn CloudEngine> {
    Box::new(FakeCloud {
        name: "Groq",
        calls,
        replies: replies.into(),
    })
}

#[test]
fn next_cloud_engine_can_handle_fallback_without_local_model() {
    let groq_calls = Arc::new(AtomicUsize::new(0));
    let gemini_calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let gemini = Box::new(FakeCloud {
        name: "Gemini",
        calls: gemini_calls.clone(),
        replies: vec![Ok(vec![])].into(),
    });
    let mut engine = chain(
        vec![
            cloud(vec![Err(EngineError::QuotaExhausted)], groq_calls.clone()),
            gemini,
        ],
        loads.clone(),
    );
    run(&mut engine, &[0.0; 16_000]).unwrap();
    assert_eq!(groq_calls.load(Ordering::SeqCst), 1);
    assert_eq!(gemini_calls.load(Ordering::SeqCst), 1);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(engine.engines_used(), vec![EngineName::Gemini]);
    assert_eq!(
        engine.fallback_reason(),
        Some(&FallbackReason::Quota(EngineName::Groq))
    );
}

#[test]
fn one_revision_can_use_groq_then_gemini_then_local_after_quotas() {
    let groq_calls = Arc::new(AtomicUsize::new(0));
    let gemini_calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![
            cloud(
                vec![Ok(vec![]), Err(EngineError::QuotaExhausted)],
                groq_calls.clone(),
            ),
            Box::new(FakeCloud {
                name: "Gemini",
                calls: gemini_calls.clone(),
                replies: vec![Ok(vec![]), Err(EngineError::QuotaExhausted)].into(),
            }),
        ],
        loads.clone(),
    );
    run(&mut engine, &vec![0.5; 21 * 60 * 16_000]).unwrap();
    assert_eq!(groq_calls.load(Ordering::SeqCst), 2);
    assert_eq!(gemini_calls.load(Ordering::SeqCst), 2);
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(
        engine.engines_used(),
        vec![
            EngineName::Groq,
            EngineName::Gemini,
            EngineName::Local("small".into())
        ]
    );
    assert_eq!(
        engine.fallback_reason(),
        Some(&FallbackReason::Quota(EngineName::Groq))
    );
}
fn chain(clouds: Vec<Box<dyn CloudEngine>>, loads: Arc<AtomicUsize>) -> EngineChain<'static> {
    EngineChain::new(
        clouds,
        "small",
        Box::new(move || {
            loads.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(Local))
        }),
    )
}
fn run(engine: &mut EngineChain<'_>, samples: &[f32]) -> Result<(), ProcessingError> {
    engine
        .transcribe(
            samples,
            Language::En,
            Some("Anna"),
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .map(|_| ())
}

#[test]
fn groq_only_does_not_load_local() {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(vec![cloud(vec![Ok(vec![])], calls.clone())], loads.clone());
    run(&mut engine, &[0.0; 16_000]).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(engine.engines_used(), vec![EngineName::Groq]);
}

#[test]
fn an_empty_remainder_does_not_call_an_engine() {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(vec![cloud(vec![], calls.clone())], loads.clone());
    run(&mut engine, &[]).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
}

#[test]
fn quota_midway_skips_groq_for_remaining_spans() {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![cloud(
            vec![Ok(vec![]), Err(EngineError::QuotaExhausted)],
            calls.clone(),
        )],
        loads.clone(),
    );
    // Three cloud spans of up to 10 minutes each.
    run(&mut engine, &vec![0.0; 21 * 60 * 16_000]).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(
        engine.engines_used(),
        vec![EngineName::Groq, EngineName::Local("small".into())]
    );
    assert_eq!(
        engine.fallback_reason(),
        Some(&FallbackReason::Quota(EngineName::Groq))
    );
}

#[test]
fn no_key_uses_local_only() {
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(vec![], loads.clone());
    run(&mut engine, &[0.0; 16_000]).unwrap();
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(
        engine.engines_used(),
        vec![EngineName::Local("small".into())]
    );
    assert_eq!(engine.fallback_reason(), None);
}

#[test]
fn chain_run_reports_per_engine_audio_totals_for_diagnostics() {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![
            cloud(
                vec![Ok(vec![]), Err(EngineError::QuotaExhausted)],
                calls.clone(),
            ),
            Box::new(FakeCloud {
                name: "Gemini",
                calls: Arc::new(AtomicUsize::new(0)),
                replies: vec![Ok(vec![]), Err(EngineError::QuotaExhausted)].into(),
            }),
        ],
        loads,
    );
    for _ in 0..3 {
        run(&mut engine, &[0.1; 16_000]).unwrap();
    }
    let report = engine.chain_report();
    let totals = report.engine_audio_ms.unwrap();
    assert_eq!(totals.get(&EngineName::Groq), Some(&1_000));
    assert_eq!(totals.get(&EngineName::Gemini), Some(&1_000));
    assert_eq!(totals.get(&EngineName::Local("small".into())), Some(&1_000));
    assert_eq!(
        LogEvent::engine_usage(1_000, 1_000, 1_000, report.fallback_reason.as_ref()).to_string(),
        "engine_usage groq_ms=1000 gemini_ms=1000 local_ms=1000 fallback=quota"
    );
}

#[test]
fn local_fallback_waits_for_ac_power_during_a_recording() {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let on_ac = Rc::new(Cell::new(false));
    let power = Rc::clone(&on_ac);
    let mut engine = chain(
        vec![cloud(vec![Err(EngineError::QuotaExhausted)], calls.clone())],
        loads.clone(),
    );
    engine.set_background_local_allowed(Box::new(move || power.get()));
    assert!(matches!(
        run(&mut engine, &[0.1; 16_000]),
        Err(ProcessingError::Deferred)
    ));
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    on_ac.set(true);
    run(&mut engine, &[0.1; 16_000]).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(loads.load(Ordering::SeqCst), 1);
}

#[test]
fn short_rate_limit_retries_same_engine() {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![cloud(
            vec![
                Err(EngineError::RateLimited {
                    retry_after: Duration::from_millis(200),
                }),
                Ok(vec![]),
            ],
            calls.clone(),
        )],
        loads.clone(),
    );
    engine.set_sleeper(Box::new(|_, _| true));
    run(&mut engine, &[0.0; 16_000]).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
}

#[test]
fn cancellation_during_retry_wait_stops_before_fallback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![cloud(
            vec![Err(EngineError::RateLimited {
                retry_after: Duration::from_secs(5),
            })],
            calls.clone(),
        )],
        loads.clone(),
    );
    engine.set_sleeper(Box::new(|_, cancel| {
        cancel.store(true, Ordering::SeqCst);
        false
    }));
    assert!(matches!(
        run(&mut engine, &[0.0; 16_000]),
        Err(ProcessingError::Cancelled)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
}

#[test]
fn cloud_spans_end_at_the_quietest_point_before_the_target_length() {
    let rate = 16_000;
    let mut samples = vec![0.5f32; 70 * rate];
    // A pause at 25.0–25.2 s and another at 53.0–53.2 s.
    samples[25 * rate..25 * rate + rate / 5].fill(0.0);
    samples[53 * rate..53 * rate + rate / 5].fill(0.0);
    let spans = meeting_minutes_core::processing::cloud_span_bounds(&samples, 30 * rate, 10 * rate);
    assert_eq!(spans.first().unwrap().start, 0);
    assert_eq!(spans.last().unwrap().end, samples.len());
    assert!(spans.windows(2).all(|pair| pair[0].end == pair[1].start));
    let first_cut = spans[0].end;
    assert!(
        (25 * rate..25 * rate + rate / 5).contains(&first_cut),
        "cut at {first_cut}"
    );
    let second_cut = spans[1].end;
    assert!(
        (53 * rate..53 * rate + rate / 5).contains(&second_cut),
        "cut at {second_cut}"
    );
}

#[test]
fn cloud_spans_cut_at_the_target_when_audio_is_uniform_and_cover_short_input_whole() {
    let samples = vec![0.5f32; 65 * 16_000];
    let spans =
        meeting_minutes_core::processing::cloud_span_bounds(&samples, 30 * 16_000, 10 * 16_000);
    assert_eq!(spans.len(), 3);
    assert!(spans.iter().all(|span| span.len() <= 30 * 16_000));
    let short = vec![0.5f32; 1000];
    assert_eq!(
        meeting_minutes_core::processing::cloud_span_bounds(&short, 30 * 16_000, 10 * 16_000),
        vec![0..1000]
    );
}

#[test]
fn managed_only_records_usage_and_audio() {
    let calls = call_log();
    let detections = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![tracking_cloud(
            "Managed transcription",
            vec![],
            None,
            calls.clone(),
            detections,
        )],
        loads.clone(),
    );
    run(&mut engine, &[0.1; 16_000]).unwrap();
    assert_eq!(engine.engines_used(), vec![EngineName::Managed]);
    assert_eq!(
        engine
            .chain_report()
            .engine_audio_ms
            .unwrap()
            .get(&EngineName::Managed),
        Some(&1_000)
    );
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn managed_quota_midway_uses_groq_for_the_rest() {
    let managed = call_log();
    let groq = call_log();
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![
            tracking_cloud(
                "Managed transcription",
                vec![Ok(vec![]), Err(EngineError::QuotaExhausted)],
                None,
                managed.clone(),
                Arc::new(AtomicUsize::new(0)),
            ),
            tracking_cloud(
                "Groq",
                vec![],
                Some(Language::Vi),
                groq.clone(),
                Arc::new(AtomicUsize::new(0)),
            ),
        ],
        loads.clone(),
    );
    for _ in 0..3 {
        run(&mut engine, &[0.1; 16_000]).unwrap();
    }
    assert_eq!(managed.lock().unwrap().len(), 2);
    assert_eq!(groq.lock().unwrap().len(), 2);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(
        engine.engines_used(),
        vec![EngineName::Managed, EngineName::Groq]
    );
    assert_eq!(
        engine.fallback_reason(),
        Some(&FallbackReason::Quota(EngineName::Managed))
    );
}

#[test]
fn managed_busy_or_disconnected_falls_through_to_next_engine() {
    for error in [EngineError::QuotaExhausted, EngineError::Unauthorized] {
        let managed = call_log();
        let gemini = call_log();
        let mut engine = chain(
            vec![
                tracking_cloud(
                    "Managed transcription",
                    vec![Ok(vec![]), Err(error.clone())],
                    None,
                    managed.clone(),
                    Arc::new(AtomicUsize::new(0)),
                ),
                tracking_cloud(
                    "Gemini",
                    vec![],
                    None,
                    gemini.clone(),
                    Arc::new(AtomicUsize::new(0)),
                ),
            ],
            Arc::new(AtomicUsize::new(0)),
        );
        for _ in 0..3 {
            run(&mut engine, &[0.1; 16_000]).unwrap();
        }
        assert_eq!(managed.lock().unwrap().len(), 2);
        assert_eq!(gemini.lock().unwrap().len(), 2);
        assert_eq!(
            engine.engines_used(),
            vec![EngineName::Managed, EngineName::Gemini]
        );
        let expected = if error == EngineError::Unauthorized {
            FallbackReason::Unauthorized(EngineName::Managed)
        } else {
            FallbackReason::Quota(EngineName::Managed)
        };
        assert_eq!(engine.fallback_reason(), Some(&expected));
    }
}

#[test]
fn managed_auto_language_comes_from_groq_and_source_changes_per_stream() {
    let managed = call_log();
    let groq_detections = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(
        vec![
            tracking_cloud(
                "Managed transcription",
                vec![],
                None,
                managed.clone(),
                Arc::new(AtomicUsize::new(0)),
            ),
            tracking_cloud(
                "Groq",
                vec![],
                Some(Language::Vi),
                call_log(),
                groq_detections.clone(),
            ),
        ],
        loads.clone(),
    );
    let language = engine
        .detect_language(&[0.1; 160], &AtomicBool::new(false))
        .unwrap();
    assert_eq!(language, Language::Vi);
    let audio = [0.1; 160];
    engine
        .transcribe_sourced_streams(
            &[&audio, &audio, &audio],
            &[Some(Source::Microphone), Some(Source::SystemAudio), None],
            language,
            None,
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(
        *managed.lock().unwrap(),
        vec![
            (Some(Source::Microphone), Language::Vi),
            (Some(Source::SystemAudio), Language::Vi),
            (None, Language::Vi)
        ]
    );
    assert_eq!(groq_detections.load(Ordering::SeqCst), 1);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
}

struct VietnameseLocal;
impl SpeechEngine for VietnameseLocal {
    fn backend(&self) -> &str {
        "CPU"
    }
    fn thread_count(&self) -> usize {
        1
    }
    fn detect_language(&mut self, _: &[f32], _: &AtomicBool) -> Result<Language, String> {
        Ok(Language::Vi)
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

#[test]
fn managed_auto_language_comes_from_local_when_no_cloud_can_detect() {
    let managed = call_log();
    let loads = Arc::new(AtomicUsize::new(0));
    let local_loads = loads.clone();
    let mut engine = EngineChain::new(
        vec![tracking_cloud(
            "Managed transcription",
            vec![],
            None,
            managed.clone(),
            Arc::new(AtomicUsize::new(0)),
        )],
        "small",
        Box::new(move || {
            local_loads.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(VietnameseLocal))
        }),
    );
    let language = engine
        .detect_language(&[0.1; 160], &AtomicBool::new(false))
        .unwrap();
    run_language(&mut engine, language);
    assert_eq!(language, Language::Vi);
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(*managed.lock().unwrap(), vec![(None, Language::Vi)]);
}

fn run_language(engine: &mut EngineChain<'_>, language: Language) {
    engine
        .transcribe(
            &[0.1; 160],
            language,
            None,
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .unwrap();
}

#[test]
fn managed_auto_language_defers_instead_of_guessing_while_local_is_not_allowed() {
    let managed = call_log();
    let mut engine = EngineChain::new(
        vec![tracking_cloud(
            "Managed transcription",
            vec![],
            None,
            managed.clone(),
            Arc::new(AtomicUsize::new(0)),
        )],
        "small",
        Box::new(|| Ok(Box::new(VietnameseLocal))),
    );
    engine.set_background_local_allowed(Box::new(|| false));
    assert!(engine
        .detect_language(&[0.1; 160], &AtomicBool::new(false))
        .is_err());
    assert!(engine.chain_report().deferred);
    assert!(managed.lock().unwrap().is_empty());
}
