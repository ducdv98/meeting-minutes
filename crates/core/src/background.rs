//! Background transcription during a Recording (ADR 0010).
//!
//! Completed Chunks of each Source are transcribed span by span while the Recording runs. Words after
//! the last sentence end of a span are held back, with their audio, and transcribed again at the
//! start of the next span, so a sentence spoken across a Chunk edge becomes one Segment. The results
//! are a cache in memory: they never reach the Library, and Processing after Stop transcribes only
//! the audio after each Source's committed point.

use std::{
    fs, io,
    path::Path,
    sync::atomic::AtomicBool,
};

use crate::{
    audio_prep::{Decimator, WHISPER_RATE},
    capture::Source,
    processing::{ProcessingError, SpeechEngine},
    transcript::{ends_sentence, place_segments, Language, PlacedSegment, RawWord, Speaker},
};

/// New audio a Source must have pending before a span is transcribed.
pub const MIN_SPAN_MS: u64 = 30_000;
/// Held-back audio with no sentence end in it is committed once it grows this long.
pub const MAX_HELD_MS: u64 = 60_000;

const SAMPLES_PER_MS: u64 = WHISPER_RATE as u64 / 1000;

/// What one Source of one Recording has transcribed so far, on the Recording's own timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceProgress {
    pub speaker: Speaker,
    /// Audio before this point is transcribed; Processing transcribes from here on.
    pub committed_ms: u64,
    pub segments: Vec<PlacedSegment>,
}

/// The cache a background transcriber leaves for one Recording.
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundResult {
    pub recording: u32,
    pub model: String,
    pub language: Option<Language>,
    pub initial_prompt: Option<String>,
    pub sources: Vec<SourceProgress>,
}

impl BackgroundResult {
    pub fn source(&self, speaker: Speaker) -> Option<&SourceProgress> {
        self.sources
            .iter()
            .find(|source| source.speaker == speaker && source.committed_ms > 0)
    }
}

struct SourceStream {
    source: Source,
    speaker: Speaker,
    next_chunk: u32,
    decimator: Decimator,
    /// 16 kHz samples before this index are committed and dropped from the decimator output.
    committed: usize,
    /// Pending audio at the last span that found no sentence end; the next span waits for more.
    tried_ms: u64,
    segments: Vec<PlacedSegment>,
}

impl SourceStream {
    fn new(source: Source, speaker: Speaker) -> Self {
        Self {
            source,
            speaker,
            next_chunk: 0,
            decimator: Decimator::new(),
            committed: 0,
            tried_ms: 0,
            segments: Vec::new(),
        }
    }

    fn pending_ms(&self) -> u64 {
        self.decimator.output.len() as u64 / SAMPLES_PER_MS
    }

    fn committed_ms(&self) -> u64 {
        self.committed as u64 / SAMPLES_PER_MS
    }

    fn ready(&self) -> bool {
        self.pending_ms() >= self.tried_ms + MIN_SPAN_MS
    }

    /// Drops pending audio up to `commit_ms` on the Recording's timeline.
    fn commit(&mut self, commit_ms: u64) {
        let drop = ((commit_ms - self.committed_ms()) * SAMPLES_PER_MS) as usize;
        let drop = drop.min(self.decimator.output.len());
        self.decimator.output.drain(..drop);
        self.committed += drop;
        self.tried_ms = 0;
    }

    fn push(&mut self, samples_48k: &[f32]) {
        for &sample in samples_48k {
            self.decimator.push(sample);
        }
    }

    /// Reads the Chunks completed since the last call, in order; a Chunk still being written is skipped.
    fn read_chunks(&mut self, chunk_dir: &Path) -> io::Result<usize> {
        let mut read = 0;
        loop {
            let path = chunk_dir.join(format!("{}-{:06}.f32", self.source.stem(), self.next_chunk));
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(read),
                Err(error) => return Err(error),
            };
            let values = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| f32::from_le_bytes(*bytes))
                .collect::<Vec<_>>();
            let mono = match self.source {
                Source::Microphone => values,
                Source::SystemAudio => values
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|[left, right]| (left + right) / 2.0)
                    .collect(),
            };
            self.push(&mono);
            self.next_chunk += 1;
            read += 1;
        }
    }

    /// Transcribes the pending audio and commits the words up to its last sentence end.
    fn transcribe_span(
        &mut self,
        engine: &mut dyn SpeechEngine,
        language: Language,
        initial_prompt: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<PlacedSegment>, ProcessingError> {
        let pending = &self.decimator.output;
        let start_ms = self.committed_ms();
        let pending_ms = self.pending_ms();
        let raw = engine.transcribe(pending, language, initial_prompt, &mut |_| {}, cancel)?;
        let placed = place_segments(&raw, pending_ms, start_ms, self.speaker);
        let mut words = placed
            .into_iter()
            .flat_map(|segment| segment.words)
            .collect::<Vec<_>>();
        words.sort_by_key(|word| (word.start_ms, word.end_ms));
        let sentence_end = words.iter().rposition(|word| ends_sentence(&word.text));
        let (keep, commit_ms) = match sentence_end {
            // Cut midway through the pause after the sentence, so no audio of the next word is lost.
            Some(index) => (
                index + 1,
                words.get(index + 1).map_or(start_ms + pending_ms, |next| {
                    (words[index].end_ms + next.start_ms) / 2
                }),
            ),
            None if words.is_empty() || pending_ms >= MAX_HELD_MS => {
                (words.len(), start_ms + pending_ms)
            }
            None => {
                self.tried_ms = pending_ms;
                return Ok(Vec::new());
            }
        };
        let committed_words = words.drain(..keep).collect::<Vec<_>>();
        self.commit(commit_ms);
        let finished = words_segment(committed_words, self.speaker)
            .into_iter()
            .collect::<Vec<_>>();
        self.segments.extend(finished.iter().cloned());
        Ok(finished)
    }
}

fn words_segment(words: Vec<RawWord>, speaker: Speaker) -> Option<PlacedSegment> {
    Some(PlacedSegment {
        start_ms: words.first()?.start_ms,
        end_ms: words.last()?.end_ms,
        text: words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        speaker,
        words,
    })
}

/// Transcribes one Recording's Chunks while it runs. It owns no engine, so the caller decides when
/// a span may run (for example only on AC power) and keeps one loaded engine for the Recording.
pub struct BackgroundTranscriber {
    recording: u32,
    model: String,
    language: Option<Language>,
    initial_prompt: Option<String>,
    streams: Vec<SourceStream>,
}

impl BackgroundTranscriber {
    pub fn new(
        recording: u32,
        model: &str,
        language: Option<Language>,
        initial_prompt: Option<String>,
    ) -> Self {
        Self {
            recording,
            model: model.to_owned(),
            language,
            initial_prompt,
            streams: vec![
                SourceStream::new(Source::Microphone, Speaker::Me),
                SourceStream::new(Source::SystemAudio, Speaker::Remote),
            ],
        }
    }

    /// Reads every Chunk completed since the last call. Returns how many were read.
    pub fn read_chunks(&mut self, chunk_dir: &Path) -> io::Result<usize> {
        let mut read = 0;
        for stream in &mut self.streams {
            read += stream.read_chunks(chunk_dir)?;
        }
        Ok(read)
    }

    /// Adds 48 kHz mono audio of one Source, as a Chunk would.
    pub fn push(&mut self, source: Source, samples_48k: &[f32]) {
        if let Some(stream) = self.streams.iter_mut().find(|stream| stream.source == source) {
            stream.push(samples_48k);
        }
    }

    /// Whether a Source has enough new audio for a span.
    pub fn span_ready(&self) -> bool {
        self.streams.iter().any(SourceStream::ready)
    }

    /// Transcribes one span of every Source with enough pending audio, in Source order. A
    /// cancelled span is discarded and the results committed before it are kept.
    pub fn transcribe_ready(
        &mut self,
        engine: &mut dyn SpeechEngine,
        cancel: &AtomicBool,
    ) -> Result<Vec<PlacedSegment>, ProcessingError> {
        let mut finished = Vec::new();
        for index in 0..self.streams.len() {
            if !self.streams[index].ready() {
                continue;
            }
            let language = match self.language {
                Some(language) => language,
                None => {
                    let stream = &mut self.streams[index];
                    if !engine.has_speech(&stream.decimator.output) {
                        // Silence is skipped, so it is not scanned again for a language.
                        let end = stream.committed_ms() + stream.pending_ms();
                        stream.commit(end);
                        continue;
                    }
                    let pending = &stream.decimator.output;
                    let language = engine
                        .detect_language(pending)
                        .map_err(ProcessingError::Engine)?;
                    self.language = Some(language);
                    language
                }
            };
            let prompt = self.initial_prompt.clone();
            finished.extend(self.streams[index].transcribe_span(
                engine,
                language,
                prompt.as_deref(),
                cancel,
            )?);
        }
        Ok(finished)
    }

    pub fn finish(self) -> BackgroundResult {
        BackgroundResult {
            recording: self.recording,
            model: self.model,
            language: self.language,
            initial_prompt: self.initial_prompt,
            sources: self
                .streams
                .into_iter()
                .map(|stream| SourceProgress {
                    speaker: stream.speaker,
                    committed_ms: stream.committed_ms(),
                    segments: stream.segments,
                })
                .collect(),
        }
    }
}

/// How a background run is driven: when to stop, whether a span may run now, and how to wait.
pub struct BackgroundControl<'a> {
    /// Set when the Recording stops or the app quits; it also cancels the span in progress.
    pub stop: &'a AtomicBool,
    /// Whether spans may run now, checked before each span (for example AC power).
    pub allowed: &'a mut dyn FnMut() -> bool,
    /// Called when there is nothing to do yet, so new Chunks can arrive.
    pub wait: &'a mut dyn FnMut(),
}

/// Transcribes a Recording's Chunks as they are completed, until told to stop. The engine is
/// loaded only when a span may run and is dropped while work is not allowed (on battery), so the
/// GPU is free; work resumes with the audio captured meanwhile once it is allowed again.
pub fn run<E: SpeechEngine>(
    transcriber: &mut BackgroundTranscriber,
    load: &mut dyn FnMut() -> Option<E>,
    chunk_dir: &Path,
    control: BackgroundControl<'_>,
    finished: &mut dyn FnMut(&[PlacedSegment]),
) -> Result<(), ProcessingError> {
    use std::sync::atomic::Ordering;
    let mut engine = None;
    loop {
        if control.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        // A missing Chunk reads as not yet written, so saving the Recording (which removes them) is harmless.
        transcriber.read_chunks(chunk_dir)?;
        if !(control.allowed)() {
            engine = None;
            (control.wait)();
            continue;
        }
        if !transcriber.span_ready() {
            (control.wait)();
            continue;
        }
        if engine.is_none() {
            engine = load();
        }
        let Some(engine) = engine.as_mut() else {
            (control.wait)();
            continue;
        };
        match transcriber.transcribe_ready(engine, control.stop) {
            Ok(segments) => finished(&segments),
            Err(ProcessingError::Cancelled) => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}
