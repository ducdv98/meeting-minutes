use meeting_minutes_core::{
    background::BackgroundTranscriber,
    capture::Source,
    cloud::{CloudEngine, EngineError},
    processing::{EngineChain, ProcessingError, SpeechDetector, SpeechEngine},
    transcript::{Language, RawSegment, RawWord, Speaker},
};
use std::{
    ops::Range,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

struct MarkedSpeech;
impl SpeechDetector for MarkedSpeech {
    fn speech_ranges(&mut self, samples: &[f32]) -> Result<Vec<Range<usize>>, String> {
        let mut ranges = Vec::new();
        let mut start = None;
        for (index, sample) in samples.iter().enumerate() {
            if *sample >= 0.5 {
                start.get_or_insert(index);
            } else if let Some(begin) = start.take() {
                ranges.push(begin..index);
            }
        }
        if let Some(begin) = start {
            ranges.push(begin..samples.len());
        }
        Ok(ranges)
    }
}

struct FailedSpeechDetector;
impl SpeechDetector for FailedSpeechDetector {
    fn speech_ranges(&mut self, _: &[f32]) -> Result<Vec<Range<usize>>, String> {
        Err("model failed".into())
    }
}

struct RecordingCloud(Arc<Mutex<Vec<Vec<f32>>>>);
impl CloudEngine for RecordingCloud {
    fn name(&self) -> &str {
        "Groq"
    }
    fn transcribe_span(
        &mut self,
        samples: &[f32],
        _: Language,
        _: Option<&str>,
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        self.0.lock().unwrap().push(samples.to_vec());
        let Some(start) = samples.iter().position(|sample| *sample >= 0.5) else {
            return Ok(vec![]);
        };
        let start_ms = start as u64 / 16;
        Ok(vec![RawSegment {
            start_ms,
            end_ms: start_ms + 500,
            text: "Speech.".into(),
            words: vec![RawWord {
                start_ms,
                end_ms: start_ms + 500,
                text: "Speech.".into(),
            }],
        }])
    }
}

fn chain(calls: Arc<Mutex<Vec<Vec<f32>>>>) -> EngineChain<'static> {
    let mut chain = EngineChain::new(
        vec![Box::new(RecordingCloud(calls))],
        "small",
        Box::new(|| Err(ProcessingError::Engine("local should not load".into()))),
    );
    chain.set_speech_detector(Box::new(MarkedSpeech));
    chain
}

fn transcribe(engine: &mut impl SpeechEngine, samples: &[f32]) -> Vec<RawSegment> {
    engine
        .transcribe(
            samples,
            Language::En,
            None,
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .unwrap()
}

#[derive(Default)]
struct OpeningState {
    opening: Vec<f32>,
    uploads: usize,
}

struct OpeningCloud(Arc<Mutex<OpeningState>>);
impl CloudEngine for OpeningCloud {
    fn name(&self) -> &str {
        "Managed transcription"
    }

    fn detect_language(
        &mut self,
        samples: &[f32],
        _: &AtomicBool,
    ) -> Result<Option<Language>, EngineError> {
        let mut state = self.0.lock().unwrap();
        state.opening = samples[..samples.len().min(30 * 16_000)].to_vec();
        state.uploads += 1;
        Ok(Some(Language::En))
    }

    fn transcribe_span(
        &mut self,
        samples: &[f32],
        _: Language,
        _: Option<&str>,
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        let mut state = self.0.lock().unwrap();
        if !samples.starts_with(&state.opening) {
            state.uploads += 1;
        }
        Ok(Vec::new())
    }
}

#[test]
fn language_detection_starts_at_first_padded_speech_region_and_reuses_opening() {
    let state = Arc::new(Mutex::new(OpeningState::default()));
    let mut engine = EngineChain::new(
        vec![Box::new(OpeningCloud(state.clone()))],
        "small",
        Box::new(|| Err(ProcessingError::Engine("local should not load".into()))),
    );
    engine.set_speech_detector(Box::new(MarkedSpeech));
    let mut samples = vec![0.02; 16 * 16_000];
    samples.extend(vec![0.5; 2 * 16_000]);

    assert_eq!(
        engine.detect_language(&samples, &AtomicBool::new(false)),
        Ok(Language::En)
    );
    assert!(state.lock().unwrap().opening == samples[16 * 16_000 - 4_000..]);
    transcribe(&mut engine, &samples);
    assert_eq!(state.lock().unwrap().uploads, 1);
}

#[test]
fn language_detection_excludes_the_gap_after_a_short_first_region() {
    let state = Arc::new(Mutex::new(OpeningState::default()));
    let mut engine = EngineChain::new(
        vec![Box::new(OpeningCloud(state.clone()))],
        "small",
        Box::new(|| Err(ProcessingError::Engine("local should not load".into()))),
    );
    engine.set_speech_detector(Box::new(MarkedSpeech));
    let mut samples = vec![0.5; 2 * 16_000];
    samples.extend(vec![0.02; 20 * 16_000]);
    samples.extend(vec![0.5; 2 * 16_000]);

    assert_eq!(
        engine.detect_language(&samples, &AtomicBool::new(false)),
        Ok(Language::En)
    );
    let opening = state.lock().unwrap().opening.clone();
    assert_eq!(opening.len(), 2 * 16_000 + 4_000);
    assert!(opening[2 * 16_000..].iter().all(|sample| *sample == 0.02));
    transcribe(&mut engine, &samples);
    assert_eq!(state.lock().unwrap().uploads, 2);
}

#[test]
fn noise_only_audio_never_reaches_cloud() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut engine = chain(calls.clone());
    assert!(transcribe(&mut engine, &vec![0.02; 30 * 16_000]).is_empty());
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn speech_regions_skip_long_noise_gap_and_keep_timeline_times() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut engine = chain(calls.clone());
    let mut samples = vec![0.5; 16_000];
    samples.extend(vec![0.02; 20 * 16_000]);
    samples.extend(vec![0.5; 16_000]);
    let segments = transcribe(&mut engine, &samples);
    let uploads = calls.lock().unwrap();
    assert_eq!(uploads.len(), 2);
    assert!(uploads.iter().all(|upload| upload.len() < 2 * 16_000));
    assert_eq!(
        segments
            .iter()
            .map(|segment| segment.start_ms)
            .collect::<Vec<_>>(),
        vec![0, 21_000]
    );
    assert_eq!(segments[1].words[0].start_ms, 21_000);
}

#[test]
fn background_skips_noise_only_span_with_chosen_language() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut engine = chain(calls.clone());
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    transcriber.push(Source::Microphone, &vec![0.02; 35 * 48_000]);
    assert!(transcriber
        .transcribe_ready(&mut engine, &AtomicBool::new(false))
        .unwrap()
        .is_empty());
    assert!(calls.lock().unwrap().is_empty());
    assert!(
        transcriber
            .finish()
            .source(Speaker::Me)
            .unwrap()
            .committed_ms
            >= 30_000
    );
}

#[test]
fn background_without_detector_keeps_uploading_chosen_language_spans() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut engine = EngineChain::new(
        vec![Box::new(RecordingCloud(calls.clone()))],
        "small",
        Box::new(|| Err(ProcessingError::Engine("local should not load".into()))),
    );
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    transcriber.push(Source::Microphone, &vec![0.0; 35 * 48_000]);
    transcriber
        .transcribe_ready(&mut engine, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn failed_detector_keeps_background_fallback_behavior() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut engine = EngineChain::new(
        vec![Box::new(RecordingCloud(calls.clone()))],
        "small",
        Box::new(|| Err(ProcessingError::Engine("local should not load".into()))),
    );
    engine.set_speech_detector(Box::new(FailedSpeechDetector));
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    transcriber.push(Source::Microphone, &vec![0.0; 35 * 48_000]);
    transcriber
        .transcribe_ready(&mut engine, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn background_cloud_result_is_reusable_only_with_working_vad() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut engine = chain(calls);
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    transcriber.push(Source::Microphone, &vec![0.5; 35 * 48_000]);
    transcriber
        .transcribe_ready(&mut engine, &AtomicBool::new(false))
        .unwrap();
    let with_vad = transcriber.finish();
    assert!(with_vad.speech_detected_by_vad);
    assert!(with_vad.is_reusable());

    let mut without_vad = with_vad.clone();
    without_vad.speech_detected_by_vad = false;
    assert!(!without_vad.is_reusable());
    without_vad.engines_used = vec![meeting_minutes_core::transcript::EngineName::Local(
        "small".into(),
    )];
    assert!(without_vad.is_reusable());
}
