use meeting_minutes_core::{
    cloud::{CloudEngine, EngineError},
    processing::{EngineChain, ProcessingError, SpeechEngine},
    transcript::{Language, RawSegment},
};
use std::{
    collections::VecDeque,
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
    assert_eq!(engine.engines_used(), vec!["Gemini"]);
    assert_eq!(engine.fallback_reason(), Some("Groq daily limit reached"));
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
    assert_eq!(engine.engines_used(), vec!["Groq", "Gemini", "small"]);
    assert_eq!(engine.fallback_reason(), Some("Groq daily limit reached"));
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
    assert_eq!(engine.engines_used(), vec!["Groq"]);
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
    assert_eq!(engine.engines_used(), vec!["Groq", "small"]);
    assert_eq!(engine.fallback_reason(), Some("Groq daily limit reached"));
}

#[test]
fn no_key_uses_local_only() {
    let loads = Arc::new(AtomicUsize::new(0));
    let mut engine = chain(vec![], loads.clone());
    run(&mut engine, &[0.0; 16_000]).unwrap();
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(engine.engines_used(), vec!["small"]);
    assert_eq!(engine.fallback_reason(), None);
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
