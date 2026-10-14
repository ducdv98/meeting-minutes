use crate::{
    ai::TextProvider,
    audio_prep::{prepare_import, prepare_recording, PreparedRecording},
    background::BackgroundResult,
    cloud::{CloudEngine, EngineError},
    correction,
    diagnostics::{log_event, LogEvent, TranscriptionEnvironment},
    meeting_file::{MeetingFile, RecordingEntry, RecordingFile},
    transcript::{
        assign_ids, place_segments, segment_sentences, shift_segment_times, EngineName,
        FallbackReason, Language, PlacedSegment, RawSegment, Speaker, Transcript,
    },
};
use serde::{Serialize, Serializer};
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    ops::Range,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub trait SpeechEngine {
    fn backend(&self) -> &str;
    fn thread_count(&self) -> usize;
    /// Name of the device the engine runs on, such as the GPU name; None when unknown.
    fn device(&self) -> Option<&str> {
        None
    }
    fn power_state(&self) -> PowerState {
        PowerState::Unknown
    }
    /// Duration sent to Whisper after VAD for the latest `transcribe` call; None without VAD.
    fn audio_after_vad_ms(&self) -> Option<u64> {
        None
    }
    fn detect_language(
        &mut self,
        samples_16k: &[f32],
        cancel: &AtomicBool,
    ) -> Result<Language, String>;
    fn has_speech(&mut self, samples_16k: &[f32]) -> bool {
        samples_16k.iter().any(|sample| sample.abs() > 0.01)
    }
    fn transcribe(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        initial_prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError>;

    fn transcribe_streams(
        &mut self,
        streams: &[&[f32]],
        language: Language,
        initial_prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<StreamTranscription>, ProcessingError> {
        transcribe_sequentially(self, streams, language, initial_prompt, progress, cancel)
    }
}

/// Cloud spans are long so a one-hour Meeting stays within Groq's per-minute request limit.
const CHAIN_SPAN_SAMPLES: usize = 10 * 60 * 16_000;
/// How far before the target length a span may end, to land on a pause instead of a word.
const CHAIN_CUT_SEARCH_SAMPLES: usize = 10 * 16_000;
const QUIET_WINDOW_SAMPLES: usize = 1_600;

/// Splits audio into spans of at most `target` samples, each ending at the quietest
/// 100 ms window within `search` samples before the target length.
pub fn cloud_span_bounds(samples: &[f32], target: usize, search: usize) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut start = 0;
    while samples.len() - start > target {
        let latest = start + target;
        let earliest = latest - search.min(target / 2);
        let mut cut = latest;
        let mut quietest = f32::INFINITY;
        let mut window = earliest;
        while window + QUIET_WINDOW_SAMPLES <= latest {
            let energy: f32 = samples[window..window + QUIET_WINDOW_SAMPLES]
                .iter()
                .map(|sample| sample * sample)
                .sum();
            if energy < quietest {
                quietest = energy;
                cut = window + QUIET_WINDOW_SAMPLES / 2;
            }
            window += QUIET_WINDOW_SAMPLES / 2;
        }
        spans.push(start..cut);
        start = cut;
    }
    spans.push(start..samples.len());
    spans
}
type LocalFactory<'a> =
    Box<dyn FnMut() -> Result<Box<dyn SpeechEngine + 'a>, ProcessingError> + 'a>;
type RetrySleeper<'a> = Box<dyn FnMut(Duration, &AtomicBool) -> bool + 'a>;

#[derive(Clone, Debug, Default)]
pub struct ChainReport {
    pub engines_used: Vec<EngineName>,
    pub fallback_reason: Option<FallbackReason>,
    pub engine_audio_ms: Option<std::collections::BTreeMap<EngineName, u64>>,
    pub offline: bool,
    pub deferred: bool,
}

/// An ordered list of cloud engines followed by a local model that is loaded on demand.
pub struct EngineChain<'a> {
    cloud: Vec<Box<dyn CloudEngine>>,
    unavailable: Vec<bool>,
    local_name: String,
    local_factory: LocalFactory<'a>,
    local: Option<Box<dyn SpeechEngine + 'a>>,
    background_local_allowed: Option<Box<dyn FnMut() -> bool + 'a>>,
    offline: bool,
    deferred: bool,
    sleeper: RetrySleeper<'a>,
    used: Vec<EngineName>,
    fallback: Option<FallbackReason>,
    audio_ms: std::collections::BTreeMap<EngineName, u64>,
}

struct BorrowedEngine<'a>(&'a mut dyn SpeechEngine);
impl SpeechEngine for BorrowedEngine<'_> {
    fn backend(&self) -> &str {
        self.0.backend()
    }
    fn thread_count(&self) -> usize {
        self.0.thread_count()
    }
    fn device(&self) -> Option<&str> {
        self.0.device()
    }
    fn power_state(&self) -> PowerState {
        self.0.power_state()
    }
    fn audio_after_vad_ms(&self) -> Option<u64> {
        self.0.audio_after_vad_ms()
    }
    fn detect_language(
        &mut self,
        samples: &[f32],
        cancel: &AtomicBool,
    ) -> Result<Language, String> {
        self.0.detect_language(samples, cancel)
    }
    fn has_speech(&mut self, samples: &[f32]) -> bool {
        self.0.has_speech(samples)
    }
    fn transcribe(
        &mut self,
        samples: &[f32],
        language: Language,
        prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        self.0
            .transcribe(samples, language, prompt, progress, cancel)
    }
    fn transcribe_streams(
        &mut self,
        streams: &[&[f32]],
        language: Language,
        prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<StreamTranscription>, ProcessingError> {
        self.0
            .transcribe_streams(streams, language, prompt, progress, cancel)
    }
}

impl<'a> EngineChain<'a> {
    pub fn borrow_local(name: &str, engine: &'a mut dyn SpeechEngine) -> Self {
        let mut engine = Some(engine);
        let mut chain = Self::new(
            vec![],
            name,
            Box::new(move || match engine.take() {
                Some(engine) => Ok(Box::new(BorrowedEngine(engine))),
                None => Err(ProcessingError::Engine(
                    "Local engine was already loaded".into(),
                )),
            }),
        );
        chain.local().expect("borrowed local engine is available");
        chain
    }
    pub fn chain_report(&self) -> ChainReport {
        ChainReport {
            engines_used: self.used.clone(),
            fallback_reason: self.fallback.clone(),
            engine_audio_ms: (!self.cloud.is_empty()).then(|| self.audio_ms.clone()),
            offline: self.offline,
            deferred: self.deferred,
        }
    }

    pub fn engines_used(&self) -> Vec<EngineName> {
        self.used.clone()
    }

    pub fn fallback_reason(&self) -> Option<&FallbackReason> {
        self.fallback.as_ref()
    }

    pub fn new(
        cloud: Vec<Box<dyn CloudEngine>>,
        local_name: &str,
        local_factory: LocalFactory<'a>,
    ) -> Self {
        let unavailable = vec![false; cloud.len()];
        Self {
            cloud,
            unavailable,
            local_name: local_name.into(),
            local_factory,
            local: None,
            background_local_allowed: None,
            offline: false,
            deferred: false,
            sleeper: Box::new(|duration, cancel| {
                let mut left = duration;
                while !left.is_zero() {
                    if cancel.load(Ordering::SeqCst) {
                        return false;
                    }
                    let step = left.min(Duration::from_millis(100));
                    std::thread::sleep(step);
                    left -= step;
                }
                !cancel.load(Ordering::SeqCst)
            }),
            used: Vec::new(),
            fallback: None,
            audio_ms: std::collections::BTreeMap::new(),
        }
    }

    pub fn set_sleeper(&mut self, sleeper: RetrySleeper<'a>) {
        self.sleeper = sleeper;
    }

    /// During a Recording, local Whisper may run only while the GPU and power allow it.
    pub fn set_background_local_allowed(&mut self, allowed: Box<dyn FnMut() -> bool + 'a>) {
        self.background_local_allowed = Some(allowed);
    }

    fn local(&mut self) -> Result<&mut dyn SpeechEngine, ProcessingError> {
        if self
            .background_local_allowed
            .as_mut()
            .is_some_and(|allowed| !allowed())
        {
            self.local = None;
            self.deferred = true;
            return Err(ProcessingError::Deferred);
        }
        if self.local.is_none() {
            let loaded = (self.local_factory)();
            self.deferred = matches!(&loaded, Err(ProcessingError::Deferred));
            self.local = Some(loaded?);
        }
        Ok(self.local.as_deref_mut().expect("loaded local engine"))
    }

    fn remember(&mut self, name: &str) {
        let name = EngineName::from(name);
        if !self.used.contains(&name) {
            self.used.push(name);
        }
    }

    fn count_audio(&mut self, name: &str, samples: &[f32]) {
        let audio_ms = samples.len() as u64 / 16;
        *self.audio_ms.entry(EngineName::from(name)).or_default() += audio_ms;
    }

    fn transcribe_span(
        &mut self,
        samples: &[f32],
        language: Language,
        prompt: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        self.offline = false;
        self.deferred = false;
        let mut failed = Vec::new();
        for index in 0..self.cloud.len() {
            if self.unavailable[index] {
                continue;
            }
            let name = self.cloud[index].name().to_owned();
            let mut other_retries = 0;
            let mut waited = Duration::ZERO;
            loop {
                if cancel.load(Ordering::SeqCst) {
                    return Err(ProcessingError::Cancelled);
                }
                let started = Instant::now();
                let result = self.cloud[index].transcribe_span(samples, language, prompt, cancel);
                let round_trip = started.elapsed();
                match result {
                    Ok(segments) => {
                        self.remember(&name);
                        self.count_audio(&name, samples);
                        if self.fallback.is_none() {
                            self.fallback = failed.first().cloned();
                        }
                        log_event(LogEvent::cloud_span(
                            &name,
                            samples.len() as u64 / 16,
                            round_trip,
                            None,
                        ));
                        return Ok(segments);
                    }
                    Err(EngineError::Cancelled) => return Err(ProcessingError::Cancelled),
                    Err(EngineError::Offline) if self.background_local_allowed.is_some() => {
                        log_event(LogEvent::engine_failure(&name, &EngineError::Offline));
                        self.offline = true;
                        return Err(ProcessingError::Offline);
                    }
                    Err(EngineError::RateLimited { retry_after })
                        if waited < Duration::from_secs(60) =>
                    {
                        let pause = retry_after
                            .max(Duration::from_millis(100))
                            .min(Duration::from_secs(60) - waited);
                        waited += pause;
                        if !(self.sleeper)(pause, cancel) {
                            return Err(ProcessingError::Cancelled);
                        }
                    }
                    Err(error) => {
                        if matches!(
                            error,
                            EngineError::Other
                                | EngineError::InvalidResponse
                                | EngineError::HttpStatus(_)
                                | EngineError::Transport
                        ) && other_retries == 0
                        {
                            other_retries += 1;
                            continue;
                        }
                        log_event(LogEvent::engine_failure(&name, &error));
                        let engine_name = EngineName::from(name.as_str());
                        let reason = match error {
                            EngineError::QuotaExhausted => FallbackReason::Quota(engine_name),
                            EngineError::Unauthorized => FallbackReason::Unauthorized(engine_name),
                            EngineError::Offline => FallbackReason::Offline(engine_name),
                            EngineError::RateLimited { .. } => {
                                FallbackReason::RateLimit(engine_name)
                            }
                            _ => FallbackReason::Other(engine_name),
                        };
                        if matches!(
                            error,
                            EngineError::QuotaExhausted
                                | EngineError::Unauthorized
                                | EngineError::Offline
                        ) {
                            self.unavailable[index] = true;
                        }
                        log_event(LogEvent::cloud_span(&name, 0, round_trip, Some(&reason)));
                        failed.push(reason);
                        break;
                    }
                }
            }
        }
        let local_name = self.local_name.clone();
        let started = Instant::now();
        let result = self
            .local()?
            .transcribe(samples, language, prompt, &mut |_| {}, cancel)?;
        if !self.cloud.is_empty() {
            log_event(LogEvent::cloud_span(
                "local",
                samples.len() as u64 / 16,
                started.elapsed(),
                None,
            ));
        }
        self.remember(&local_name);
        self.count_audio(&local_name, samples);
        if self.fallback.is_none() {
            self.fallback = failed.into_iter().next();
        }
        Ok(result)
    }
}

impl SpeechEngine for EngineChain<'_> {
    fn backend(&self) -> &str {
        self.local
            .as_ref()
            .map_or("cloud", |engine| engine.backend())
    }
    fn thread_count(&self) -> usize {
        self.local
            .as_ref()
            .map_or(0, |engine| engine.thread_count())
    }
    fn device(&self) -> Option<&str> {
        self.local.as_ref().and_then(|engine| engine.device())
    }
    fn power_state(&self) -> PowerState {
        self.local
            .as_ref()
            .map_or(PowerState::Unknown, |engine| engine.power_state())
    }
    fn audio_after_vad_ms(&self) -> Option<u64> {
        self.local
            .as_ref()
            .and_then(|engine| engine.audio_after_vad_ms())
    }
    fn has_speech(&mut self, samples: &[f32]) -> bool {
        if self.cloud.is_empty() {
            return self.local().map_or_else(
                |_| samples.iter().any(|sample| sample.abs() > 0.01),
                |engine| engine.has_speech(samples),
            );
        }
        samples.iter().any(|sample| sample.abs() > 0.01)
    }
    fn detect_language(
        &mut self,
        samples: &[f32],
        cancel: &AtomicBool,
    ) -> Result<Language, String> {
        self.offline = false;
        self.deferred = false;
        for (index, engine) in self.cloud.iter_mut().enumerate() {
            if self.unavailable[index] {
                continue;
            }
            match engine.detect_language(samples, cancel) {
                Ok(Some(language)) => return Ok(language),
                Ok(None) => continue,
                Err(EngineError::Cancelled) => return Err("Processing cancelled".into()),
                Err(EngineError::Offline) if self.background_local_allowed.is_some() => {
                    log_event(LogEvent::engine_failure(
                        engine.name(),
                        &EngineError::Offline,
                    ));
                    self.offline = true;
                    return Err("Provider unavailable".into());
                }
                Err(error) => {
                    log_event(LogEvent::engine_failure(engine.name(), &error));
                    if matches!(
                        error,
                        EngineError::QuotaExhausted
                            | EngineError::Unauthorized
                            | EngineError::Offline
                    ) {
                        self.unavailable[index] = true;
                        let name = EngineName::from(engine.name());
                        let reason = match error {
                            EngineError::QuotaExhausted => FallbackReason::Quota(name),
                            EngineError::Unauthorized => FallbackReason::Unauthorized(name),
                            _ => FallbackReason::Offline(name),
                        };
                        self.fallback.get_or_insert(reason);
                    }
                }
            }
        }
        self.local()
            .map_err(|error| error.to_string())?
            .detect_language(samples, cancel)
    }
    fn transcribe(
        &mut self,
        samples: &[f32],
        language: Language,
        prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        if samples.is_empty() {
            progress(1.0);
            return Ok(Vec::new());
        }
        if self.cloud.is_empty() {
            let result = self
                .local()?
                .transcribe(samples, language, prompt, progress, cancel)?;
            let name = self.local_name.clone();
            self.remember(&name);
            return Ok(result);
        }
        let mut all = Vec::new();
        for span in cloud_span_bounds(samples, CHAIN_SPAN_SAMPLES, CHAIN_CUT_SEARCH_SAMPLES) {
            let offset = span.start as u64 / 16;
            let end = span.end;
            let mut segments = self.transcribe_span(&samples[span], language, prompt, cancel)?;
            for segment in &mut segments {
                shift_segment_times(
                    &mut segment.start_ms,
                    &mut segment.end_ms,
                    &mut segment.words,
                    offset,
                );
            }
            all.extend(segments);
            progress(end as f32 / samples.len().max(1) as f32);
        }
        Ok(all)
    }
    fn transcribe_streams(
        &mut self,
        streams: &[&[f32]],
        language: Language,
        prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<StreamTranscription>, ProcessingError> {
        if self.cloud.is_empty() {
            let result = self
                .local()?
                .transcribe_streams(streams, language, prompt, progress, cancel)?;
            let name = self.local_name.clone();
            self.remember(&name);
            return Ok(result);
        }
        transcribe_sequentially(self, streams, language, prompt, progress, cancel)
    }
}

/// Transcribes each Source's audio in turn, with progress across all of them.
pub fn transcribe_sequentially<E: SpeechEngine + ?Sized>(
    engine: &mut E,
    streams: &[&[f32]],
    language: Language,
    initial_prompt: Option<&str>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<Vec<StreamTranscription>, ProcessingError> {
    let total: usize = streams.iter().map(|samples| samples.len()).sum();
    let mut completed = 0usize;
    let mut results = Vec::with_capacity(streams.len());
    for samples in streams {
        let base = completed as f32 / total.max(1) as f32;
        let weight = samples.len() as f32 / total.max(1) as f32;
        let segments = engine.transcribe(
            samples,
            language,
            initial_prompt,
            &mut |fraction| progress(base + fraction.clamp(0.0, 1.0) * weight),
            cancel,
        )?;
        results.push(StreamTranscription {
            segments,
            audio_after_vad_ms: engine.audio_after_vad_ms(),
        });
        completed += samples.len();
    }
    Ok(results)
}

pub struct StreamTranscription {
    pub segments: Vec<RawSegment>,
    pub audio_after_vad_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerState {
    Ac,
    Battery,
    Unknown,
}

impl PowerState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ac => "ac",
            Self::Battery => "battery",
            Self::Unknown => "unknown",
        }
    }

    pub fn current() -> Self {
        #[cfg(windows)]
        {
            use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
            let mut status = SYSTEM_POWER_STATUS::default();
            if unsafe { GetSystemPowerStatus(&mut status) }.is_ok() {
                return match status.ACLineStatus {
                    0 => Self::Battery,
                    1 => Self::Ac,
                    _ => Self::Unknown,
                };
            }
        }
        Self::Unknown
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ProcessingStage {
    Downloading,
    Preparing,
    Transcribing,
    Publishing,
}
impl ProcessingStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Downloading => "downloading",
            Self::Preparing => "preparing",
            Self::Transcribing => "transcribing",
            Self::Publishing => "publishing",
        }
    }
}

struct ProcessingTelemetry {
    stage: ProcessingStage,
    audio_ms: u64,
    total_ms: u64,
    whisper: Duration,
}

#[derive(Debug)]
pub enum ProcessingError {
    Cancelled,
    Offline,
    Deferred,
    Io(io::Error),
    Json(serde_json::Error),
    Audio(String),
    Unsupported(String),
    Engine(String),
    Download(String),
    Checksum,
    NoAudio,
}
impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "Processing cancelled"),
            Self::Offline => write!(f, "Provider unavailable"),
            Self::Deferred => write!(f, "Background transcription deferred"),
            Self::Io(e) => write!(f, "Processing I/O error: {e}"),
            Self::Json(e) => write!(f, "Meeting metadata error: {e}"),
            Self::Audio(e) => write!(f, "Audio error: {e}"),
            Self::Unsupported(e) => write!(f, "Unsupported audio file: {e}"),
            Self::Engine(e) => write!(f, "Transcription error: {e}"),
            Self::Download(e) => write!(f, "Model download error: {e}"),
            Self::Checksum => write!(f, "Model checksum mismatch"),
            Self::NoAudio => write!(f, "This Meeting has no Recording audio"),
        }
    }
}
impl std::error::Error for ProcessingError {}
impl Serialize for ProcessingError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl From<io::Error> for ProcessingError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for ProcessingError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<crate::library::LibraryError> for ProcessingError {
    fn from(e: crate::library::LibraryError) -> Self {
        match e {
            crate::library::LibraryError::Io(e) => Self::Io(e),
            crate::library::LibraryError::Json(e) => Self::Json(e),
            e => Self::Engine(e.to_string()),
        }
    }
}

pub struct TranscribeRequest<'a> {
    pub meeting_dir: &'a Path,
    pub model: &'a str,
    pub vocabulary: &'a [String],
    /// Settings' Meeting language, used when the Meeting has none of its own.
    pub default_language: Option<Language>,
}

type DraftCallback<'a> = dyn FnMut(&[PlacedSegment]) + 'a;

/// The Vocabulary as Whisper's initial prompt, or None when it has no terms.
pub fn initial_prompt(vocabulary: &[String]) -> Option<String> {
    let prompt = vocabulary
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    (!prompt.is_empty()).then_some(prompt)
}

fn meeting_language(
    metadata: &MeetingFile,
    request: &TranscribeRequest,
) -> Result<Option<Language>, ProcessingError> {
    match metadata.language.as_deref() {
        None => Ok(request.default_language),
        Some(code) => Language::from_code(code).map(Some).ok_or_else(|| {
            ProcessingError::Engine(format!("Unsupported Meeting language: {code}"))
        }),
    }
}

/// The background results a transcription may reuse: a cache made with another model, prompt
/// or language would not match a fresh transcription, and an Import has no Recordings.
fn usable_background<'b>(
    metadata: &MeetingFile,
    request: &TranscribeRequest,
    prompt: Option<&str>,
    language: Option<Language>,
    background: &'b [BackgroundResult],
) -> Vec<&'b BackgroundResult> {
    background
        .iter()
        .filter(|cache| {
            metadata.import.is_none()
                && cache.model == request.model
                && cache.initial_prompt.as_deref() == prompt
                && (language.is_none() || cache.language.is_none() || cache.language == language)
        })
        .collect()
}

/// The Draft that background results give a Meeting before any audio is transcribed, so it can
/// be shown at once after Stop (ADR 0010).
pub fn background_draft(
    request: &TranscribeRequest,
    background: &[BackgroundResult],
) -> Result<Vec<PlacedSegment>, ProcessingError> {
    let metadata: MeetingFile =
        serde_json::from_slice(&fs::read(request.meeting_dir.join("meeting.json"))?)?;
    let prompt = initial_prompt(request.vocabulary);
    let language = meeting_language(&metadata, request)?;
    let background = usable_background(&metadata, request, prompt.as_deref(), language, background);
    let mut draft = Vec::new();
    for recording in metadata.recordings.iter().filter_map(|entry| match entry {
        RecordingEntry::Known(recording) => Some(recording),
        _ => None,
    }) {
        let Some(cache) = background
            .iter()
            .find(|cache| cache.recording == recording.number)
        else {
            continue;
        };
        for source in &cache.sources {
            draft.extend(segment_sentences(shifted(
                &source.segments,
                recording.timeline_start_ms,
            )));
        }
    }
    draft.sort_by_key(|segment| (segment.start_ms, segment.end_ms));
    Ok(draft)
}

/// Moves Segments from a Recording's own timeline onto the Meeting timeline.
fn shifted(segments: &[PlacedSegment], timeline_start_ms: u64) -> Vec<PlacedSegment> {
    segments
        .iter()
        .cloned()
        .map(|mut segment| {
            shift_segment_times(
                &mut segment.start_ms,
                &mut segment.end_ms,
                &mut segment.words,
                timeline_start_ms,
            );
            segment
        })
        .collect()
}

pub fn transcribe_meeting(
    request: &TranscribeRequest,
    engine: &mut EngineChain<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_named(request, engine, progress, cancel, None)
}

pub fn transcribe_meeting_named(
    request: &TranscribeRequest,
    engine: &mut EngineChain<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_with_draft(
        request,
        engine,
        progress,
        cancel,
        your_name,
        None,
        TranscriptionContext {
            background: &[],
            correction: CorrectionProvider::Disabled,
        },
    )
}

pub fn transcribe_meeting_named_with_draft(
    request: &TranscribeRequest,
    engine: &mut EngineChain<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
    draft: &mut dyn FnMut(&[PlacedSegment]),
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_resuming(request, engine, progress, cancel, your_name, draft, &[])
}

/// Transcribes a Meeting, reusing what a background transcriber finished during its Recordings
/// (ADR 0010): their Segments are shown as the Draft at once, and only later audio is transcribed.
pub fn transcribe_meeting_resuming(
    request: &TranscribeRequest,
    engine: &mut EngineChain<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
    draft: &mut dyn FnMut(&[PlacedSegment]),
    background: &[BackgroundResult],
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_resuming_with_correction(
        request,
        engine,
        progress,
        cancel,
        your_name,
        draft,
        TranscriptionContext {
            background,
            correction: CorrectionProvider::Disabled,
        },
    )
}

#[derive(Clone, Copy)]
pub enum CorrectionProvider<'a> {
    Disabled,
    Ready(&'a dyn TextProvider),
    Unavailable,
}

pub struct TranscriptionContext<'a> {
    pub background: &'a [BackgroundResult],
    pub correction: CorrectionProvider<'a>,
}

pub fn transcribe_meeting_resuming_with_correction(
    request: &TranscribeRequest,
    engine: &mut EngineChain<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
    draft: &mut dyn FnMut(&[PlacedSegment]),
    context: TranscriptionContext<'_>,
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_with_draft(
        request,
        engine,
        progress,
        cancel,
        your_name,
        Some(draft),
        context,
    )
}

fn transcribe_meeting_with_draft(
    request: &TranscribeRequest,
    engine: &mut EngineChain<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
    draft: Option<&mut DraftCallback<'_>>,
    context: TranscriptionContext<'_>,
) -> Result<Transcript, ProcessingError> {
    let mut telemetry = ProcessingTelemetry {
        stage: ProcessingStage::Preparing,
        audio_ms: 0,
        total_ms: 0,
        whisper: Duration::ZERO,
    };
    let backend = engine.backend().to_owned();
    let device = engine.device().map(str::to_owned);
    let result = transcribe_meeting_inner(
        request,
        engine,
        progress,
        cancel,
        Resume {
            your_name,
            draft,
            background: context.background,
            correction_provider: context.correction,
        },
        &mut telemetry,
    );
    if let Err(error) = &result {
        log_event(LogEvent::processing_result(
            error,
            telemetry.stage,
            Some(&backend),
            device.as_deref(),
            telemetry.audio_ms,
            telemetry.total_ms,
            telemetry.whisper,
        ));
    }
    result
}

/// What a transcription shows and reuses besides the audio: the user's name for Participants,
/// where Draft Segments go, and background results from the Recordings.
struct Resume<'a, 'd> {
    your_name: Option<&'a str>,
    draft: Option<&'a mut DraftCallback<'d>>,
    background: &'a [BackgroundResult],
    correction_provider: CorrectionProvider<'a>,
}

fn transcribe_meeting_inner(
    request: &TranscribeRequest,
    engine: &mut EngineChain<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    resume: Resume<'_, '_>,
    telemetry: &mut ProcessingTelemetry,
) -> Result<Transcript, ProcessingError> {
    let Resume {
        your_name,
        mut draft,
        background,
        correction_provider,
    } = resume;
    if cancel.load(Ordering::SeqCst) {
        return Err(ProcessingError::Cancelled);
    }
    let metadata: MeetingFile =
        serde_json::from_slice(&fs::read(request.meeting_dir.join("meeting.json"))?)?;
    let recordings: Vec<_> = metadata
        .recordings
        .iter()
        .filter_map(|entry| match entry {
            RecordingEntry::Known(r) => Some(r),
            _ => None,
        })
        .collect();
    if recordings.is_empty() && metadata.import.is_none() {
        return Err(ProcessingError::NoAudio);
    }
    let total: u64 = if metadata.import.is_some() {
        metadata.duration_ms
    } else {
        recordings
            .iter()
            .map(|r| r.duration_ms * r.files.len() as u64)
            .sum()
    };
    telemetry.total_ms = total;
    let prompt = initial_prompt(request.vocabulary);
    let prompt = prompt.as_deref();
    let mut language = meeting_language(&metadata, request)?;
    let detected = language.is_none();
    let background = usable_background(&metadata, request, prompt, language, background);
    let cached_local = background
        .iter()
        .any(|cache| cache.sources.iter().any(|source| source.committed_ms > 0));
    if language.is_none() {
        language = background.iter().find_map(|cache| cache.language);
    }
    let cached = |recording: &RecordingFile| {
        background
            .iter()
            .copied()
            .find(|cache| cache.recording == recording.number)
    };
    let mut per_source: [Vec<PlacedSegment>; 3] = std::array::from_fn(|_| Vec::new());
    let mut elapsed = 0u64;
    let mut preparation_wall_time = Duration::ZERO;
    let mut whisper_wall_time = Duration::ZERO;
    let mut transcribed_ms = 0u64;
    let mut audio_after_vad_ms = Some(0u64);
    let mut process = |prepared: Vec<PreparedRecording>,
                       elapsed: u64,
                       cache: Option<&BackgroundResult>,
                       telemetry: &mut ProcessingTelemetry|
     -> Result<(), ProcessingError> {
        // Audio before a Source's committed point was transcribed during the Recording.
        let skipped = prepared
            .iter()
            .map(|stream| {
                cache
                    .and_then(|cache| cache.source(stream.speaker))
                    .map_or(0, |source| {
                        ((source.committed_ms * 16) as usize).min(stream.samples_16k.len())
                    })
            })
            .collect::<Vec<_>>();
        telemetry.stage = ProcessingStage::Transcribing;
        let whisper_started = Instant::now();
        if language.is_none() {
            if let Some(stream) = prepared
                .iter()
                .find(|stream| engine.has_speech(&stream.samples_16k))
            {
                language = Some(match engine.detect_language(&stream.samples_16k, cancel) {
                    Ok(language) => language,
                    Err(error) => {
                        telemetry.whisper += whisper_started.elapsed();
                        return Err(if cancel.load(Ordering::SeqCst) {
                            ProcessingError::Cancelled
                        } else {
                            ProcessingError::Engine(error)
                        });
                    }
                });
            }
        }
        let group_ms: u64 = prepared
            .iter()
            .map(|stream| stream.samples_16k.len() as u64 * 1000 / 16_000)
            .sum();
        let weight = if total == 0 {
            1.0
        } else {
            group_ms as f32 / total as f32
        };
        let offset = if total == 0 {
            0.0
        } else {
            elapsed as f32 / total as f32
        };
        let streams = prepared
            .iter()
            .zip(&skipped)
            .map(|(stream, &skip)| &stream.samples_16k[skip..])
            .collect::<Vec<_>>();
        let remaining_ms: u64 = streams
            .iter()
            .map(|samples| samples.len() as u64 * 1000 / 16_000)
            .sum();
        let done = 1.0 - remaining_ms as f32 / group_ms.max(1) as f32;
        let mut group_progress = 0.0f32;
        let result = engine.transcribe_streams(
            &streams,
            language.unwrap_or(Language::En),
            prompt,
            &mut |p| {
                group_progress = group_progress.max(p.clamp(0.0, 1.0));
                progress(
                    (offset + (done + group_progress * (1.0 - done)) * weight).clamp(0.0, 1.0),
                );
            },
            cancel,
        );
        let whisper_elapsed = whisper_started.elapsed();
        telemetry.whisper += whisper_elapsed;
        let results = match result {
            Ok(results) => results,
            Err(error) => {
                telemetry.audio_ms += (group_ms as f64 * group_progress as f64) as u64;
                return Err(error);
            }
        };
        if results.len() != prepared.len() {
            return Err(ProcessingError::Engine(
                "Engine returned the wrong number of streams".into(),
            ));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        for ((stream, result), skip) in prepared.iter().zip(results).zip(skipped) {
            audio_after_vad_ms = audio_after_vad_ms
                .and_then(|total| result.audio_after_vad_ms.map(|duration| total + duration));
            let index = match stream.speaker {
                Speaker::Me => 0,
                Speaker::Remote => 1,
                Speaker::Unknown => 2,
            };
            let skipped_ms = skip as u64 / 16;
            let placed = place_segments(
                &result.segments,
                stream.duration_ms.saturating_sub(skipped_ms),
                stream.timeline_start_ms + skipped_ms,
                stream.speaker,
            );
            if let Some(source) = cache.and_then(|cache| cache.source(stream.speaker)) {
                per_source[index].extend(shifted(&source.segments, stream.timeline_start_ms));
            }
            // The Draft shows this Source's finished Segments; the Revision re-segments all of them at the end.
            if let Some(callback) = draft.as_deref_mut() {
                let sentences = segment_sentences(placed.clone());
                if !sentences.is_empty() {
                    callback(&sentences);
                }
            }
            per_source[index].extend(placed);
        }
        whisper_wall_time += whisper_elapsed;
        transcribed_ms += remaining_ms;
        telemetry.audio_ms += group_ms;
        Ok(())
    };
    if let Some(import) = &metadata.import {
        telemetry.stage = ProcessingStage::Preparing;
        let preparation_started = Instant::now();
        let prepared = prepare_import(request.meeting_dir, &import.file, cancel)?;
        preparation_wall_time += preparation_started.elapsed();
        process(vec![prepared], 0, None, telemetry)?;
    } else {
        for recording in recordings {
            telemetry.stage = ProcessingStage::Preparing;
            if cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            let preparation_started = Instant::now();
            let prepared = prepare_recording(request.meeting_dir, recording, cancel)?;
            preparation_wall_time += preparation_started.elapsed();
            process(prepared, elapsed, cached(recording), telemetry)?;
            elapsed += recording.duration_ms * recording.files.len() as u64;
        }
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(ProcessingError::Cancelled);
    }
    telemetry.stage = ProcessingStage::Publishing;
    let revision = fs::read_dir(request.meeting_dir)?
        .filter_map(|entry| {
            let name = entry.ok()?.file_name();
            let name = name.to_str()?;
            name.strip_prefix("transcript.r")?
                .strip_suffix(".json")?
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| ProcessingError::Engine("Transcript revision overflow".into()))?;
    let mut transcript = Transcript {
        schema_version: 1,
        revision,
        created_at: chrono::Local::now().to_rfc3339(),
        model: {
            let mut used = Vec::new();
            let processing_used = engine.chain_report().engines_used;
            for name in background
                .iter()
                .flat_map(|cache| cache.engines_used.iter())
                .chain(processing_used.iter())
            {
                if !used.contains(name) {
                    used.push(name.clone());
                }
            }
            if cached_local
                && background.iter().any(|cache| cache.engines_used.is_empty())
                && !used.iter().any(|name| name.label() == request.model)
            {
                used.insert(0, request.model.into());
            }
            if used.is_empty() {
                vec![request.model.into()]
            } else {
                used
            }
        },
        fallback_reason: background
            .iter()
            .find_map(|cache| cache.fallback_reason.clone())
            .or_else(|| engine.chain_report().fallback_reason),
        language: language.unwrap_or(Language::En),
        language_detected: detected && language.is_some(),
        duration_ms: metadata.duration_ms,
        segments: assign_ids(
            per_source.into_iter().flat_map(segment_sentences).collect(),
            metadata.duration_ms,
        ),
    };
    let mut correction_notice = None;
    if let CorrectionProvider::Ready(provider) = correction_provider {
        let mut names = std::collections::BTreeSet::new();
        let participants = crate::participant::Participants::new(
            metadata.participants.clone(),
            metadata.segment_participants.clone(),
            your_name,
            metadata.is_voice_note(),
        );
        names.extend(participants.vocabulary_names(None));
        names.extend(
            transcript
                .segments
                .iter()
                .map(|segment| participants.label_name(segment.speaker)),
        );
        if let Some(assignments) = &metadata.segment_participants {
            names.extend(assignments.segments.values().cloned());
        }
        let names = names.into_iter().collect::<Vec<_>>();
        let started = Instant::now();
        match correction::correct(
            provider,
            &transcript.segments,
            transcript.language,
            request.vocabulary,
            &names,
            cancel,
        ) {
            Ok(corrected) => {
                let changed = transcript
                    .segments
                    .iter()
                    .zip(&corrected)
                    .filter(|(before, after)| before.text != after.text)
                    .count();
                transcript.segments = corrected;
                log_event(LogEvent::CorrectionResult {
                    outcome: "completed",
                    changed_segments: changed,
                    duration_ms: started.elapsed().as_millis(),
                });
            }
            Err(_) if cancel.load(Ordering::SeqCst) => return Err(ProcessingError::Cancelled),
            Err(error) => {
                correction_notice =
                    Some("Correction was unavailable; this Revision has uncorrected text.");
                log_event(LogEvent::correction_failure(&error));
                log_event(LogEvent::CorrectionResult {
                    outcome: "failed",
                    changed_segments: 0,
                    duration_ms: started.elapsed().as_millis(),
                });
            }
        }
    } else if matches!(correction_provider, CorrectionProvider::Unavailable) {
        correction_notice = Some("Correction was unavailable; this Revision has uncorrected text.");
        log_event(LogEvent::CorrectionResult {
            outcome: "unavailable",
            changed_segments: 0,
            duration_ms: 0,
        });
    }
    let markdown_tmp = request.meeting_dir.join("transcript.md.tmp");
    let mut published = false;
    for _ in 0..10 {
        let json_path = request
            .meeting_dir
            .join(format!("transcript.r{}.json", transcript.revision));
        let json_tmp = request
            .meeting_dir
            .join(format!("transcript.r{}.json.tmp", transcript.revision));
        let write = (|| -> Result<(), ProcessingError> {
            let mut json = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&json_tmp)?;
            serde_json::to_writer_pretty(&mut json, &transcript)?;
            json.write_all(b"\n")?;
            json.sync_all()?;
            if cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            fs::hard_link(&json_tmp, &json_path)?;
            Ok(())
        })();
        let _ = fs::remove_file(&json_tmp);
        match write {
            Ok(()) => {
                published = true;
                break;
            }
            Err(ProcessingError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists => {
                transcript.revision = transcript.revision.checked_add(1).ok_or_else(|| {
                    ProcessingError::Engine("Transcript revision overflow".into())
                })?;
            }
            Err(error) => return Err(error),
        }
    }
    if !published {
        return Err(ProcessingError::Engine(
            "Transcript revision collision limit".into(),
        ));
    }
    let write = crate::metadata::update_metadata_and(
        request.meeting_dir,
        |metadata| {
            if let Some(notice) = correction_notice {
                metadata["correction_notice"] = serde_json::Value::String(notice.into());
            } else if let Some(object) = metadata.as_object_mut() {
                object.remove("correction_notice");
            }
            let file: MeetingFile = serde_json::from_value(metadata.clone())?;
            // Segment moves belong to the Revision they were made on; carry them onto this one by time.
            let previous_path = file
                .segment_participants
                .as_ref()
                .filter(|map| map.revision != transcript.revision)
                .map(|map| {
                    request
                        .meeting_dir
                        .join(format!("transcript.r{}.json", map.revision))
                });
            if let Some(previous_path) = previous_path.filter(|path| path.is_file()) {
                let previous: Transcript = serde_json::from_slice(&fs::read(previous_path)?)?;
                let participants = crate::participant::Participants::new(
                    file.participants.clone(),
                    file.segment_participants.clone(),
                    your_name,
                    file.is_voice_note(),
                );
                metadata["segment_participants"] = serde_json::to_value(
                    crate::participant::carry_over(&previous, &transcript, &participants),
                )?;
            }
            Ok(())
        },
        |metadata| {
            let file: MeetingFile =
                serde_json::from_value(metadata.clone()).map_err(io::Error::other)?;
            let participants = crate::participant::Participants::new(
                file.participants.clone(),
                file.segment_participants.clone(),
                your_name,
                file.is_voice_note(),
            );
            let markdown =
                crate::transcript::render_markdown_named(&file.title, &transcript, &participants);
            let mut md = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&markdown_tmp)?;
            md.write_all(markdown.as_bytes())?;
            md.sync_all()?;
            fs::rename(&markdown_tmp, request.meeting_dir.join("transcript.md"))?;
            Ok(())
        },
    )
    .map_err(ProcessingError::from);
    if let Err(error) = write {
        let _ = fs::remove_file(&markdown_tmp);
        return Err(error);
    }
    progress(1.0);
    log_event(LogEvent::transcription_completed_with_device(
        request.model,
        TranscriptionEnvironment {
            backend: engine.backend(),
            device: engine.device(),
            power: engine.power_state(),
        },
        engine.thread_count(),
        transcribed_ms,
        audio_after_vad_ms,
        preparation_wall_time,
        whisper_wall_time,
    ));
    let engine_audio = engine.chain_report().engine_audio_ms;
    let mut audio_totals = engine_audio.clone().unwrap_or_default();
    for (name, duration) in background
        .iter()
        .filter_map(|cache| cache.engine_audio_ms.as_ref())
        .flat_map(|usage| usage.iter())
    {
        *audio_totals.entry(name.clone()).or_default() += duration;
    }
    if engine_audio.is_some()
        || background
            .iter()
            .any(|cache| cache.engine_audio_ms.is_some())
    {
        let groq_ms = audio_totals
            .get(&EngineName::Groq)
            .copied()
            .unwrap_or_default();
        let gemini_ms = audio_totals
            .get(&EngineName::Gemini)
            .copied()
            .unwrap_or_default();
        let local_ms = audio_totals
            .iter()
            .filter(|(name, _)| matches!(name, EngineName::Local(_)))
            .map(|(_, duration)| duration)
            .sum();
        log_event(LogEvent::engine_usage(
            groq_ms,
            gemini_ms,
            local_ms,
            transcript.fallback_reason.as_ref(),
        ));
    }
    Ok(transcript)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStage {
    Queued,
    Downloading,
    Transcribing,
    Failed,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProcessingJob {
    pub meeting_id: String,
    pub model: Option<String>,
    pub stage: JobStage,
    pub progress: f32,
    pub error: Option<String>,
    #[serde(skip)]
    rerun: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProcessingStatus {
    pub jobs: Vec<ProcessingJob>,
}
pub struct Processor {
    jobs: Vec<ProcessingJob>,
    running: Option<String>,
    cancel: Arc<AtomicBool>,
}
impl Default for Processor {
    fn default() -> Self {
        Self::new()
    }
}
impl Processor {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            running: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn enqueue(&mut self, meeting_id: &str) -> bool {
        self.enqueue_with_model(meeting_id, None)
    }
    pub fn enqueue_with_model(&mut self, meeting_id: &str, model: Option<&str>) -> bool {
        if let Some(index) = self.jobs.iter().position(|j| j.meeting_id == meeting_id) {
            if self.running.as_deref() == Some(meeting_id) {
                let job = &mut self.jobs[index];
                let newly_queued = !job.rerun;
                job.rerun = true;
                job.model = model.map(str::to_owned);
                return newly_queued;
            }
            if self.jobs[index].stage != JobStage::Failed {
                self.jobs[index].model = model.map(str::to_owned);
                return false;
            }
            self.jobs.remove(index);
        }
        self.jobs.push(ProcessingJob {
            meeting_id: meeting_id.into(),
            model: model.map(str::to_owned),
            stage: JobStage::Queued,
            progress: 0.0,
            error: None,
            rerun: false,
        });
        true
    }
    pub fn job_model(&self, meeting_id: &str) -> Option<String> {
        self.jobs
            .iter()
            .find(|job| job.meeting_id == meeting_id)
            .and_then(|job| job.model.clone())
    }
    #[allow(clippy::should_implement_trait)] // The ticket names this queue operation `next`.
    pub fn next(&mut self) -> Option<String> {
        if self.running.is_some() {
            return None;
        }
        let job = self.jobs.iter().find(|j| j.stage == JobStage::Queued)?;
        let id = job.meeting_id.clone();
        self.cancel = Arc::new(AtomicBool::new(false));
        self.running = Some(id.clone());
        Some(id)
    }
    pub fn set_stage(&mut self, meeting_id: &str, stage: JobStage, progress: f32) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.meeting_id == meeting_id) {
            job.stage = stage;
            job.progress = progress.clamp(0.0, 1.0);
        }
    }
    pub fn finished(&mut self, meeting_id: &str, result: Result<(), ProcessingError>) {
        if self.running.as_deref() == Some(meeting_id) {
            self.running = None;
        }
        match result {
            Ok(())
                if self
                    .jobs
                    .iter()
                    .any(|j| j.meeting_id == meeting_id && j.rerun) =>
            {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.meeting_id == meeting_id) {
                    job.stage = JobStage::Queued;
                    job.progress = 0.0;
                    job.error = None;
                    job.rerun = false;
                }
            }
            Ok(()) | Err(ProcessingError::Cancelled) => {
                self.jobs.retain(|j| j.meeting_id != meeting_id)
            }
            Err(error) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.meeting_id == meeting_id) {
                    job.stage = JobStage::Failed;
                    job.error = Some(error.to_string());
                    job.rerun = false;
                }
            }
        }
    }
    pub fn cancel(&mut self, meeting_id: &str) {
        if self.running.as_deref() == Some(meeting_id) {
            self.cancel.store(true, Ordering::SeqCst);
        } else {
            self.jobs
                .retain(|j| j.meeting_id != meeting_id || j.stage != JobStage::Queued);
        }
    }
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }
    pub fn is_busy(&self, meeting_id: &str) -> bool {
        self.jobs
            .iter()
            .any(|j| j.meeting_id == meeting_id && j.stage != JobStage::Failed)
    }
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }
    pub fn dismiss(&mut self, meeting_id: &str) {
        self.jobs
            .retain(|j| j.meeting_id != meeting_id || j.stage != JobStage::Failed);
    }
    pub fn status(&self) -> ProcessingStatus {
        ProcessingStatus {
            jobs: self.jobs.clone(),
        }
    }
}
