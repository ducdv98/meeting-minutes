use meeting_minutes_core::background::{self, BackgroundControl, BackgroundTranscriber};
use meeting_minutes_core::processing::{
    background_draft, transcribe_meeting, transcribe_meeting_resuming, EngineChain,
    ProcessingError, SpeechEngine, TranscribeRequest,
};
use meeting_minutes_core::transcript::{Language, RawSegment, RawWord, Speaker, Transcript};
use meeting_minutes_core::{capture::Source, finalize_recording, recording::ChunkWriter, Library};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::AtomicBool,
};
use tempfile::TempDir;

#[test]
fn managed_cloud_recording_starts_on_battery_without_a_discrete_gpu() {
    use background::BackgroundStart;
    assert_eq!(
        background::start_decision(true, false, false),
        BackgroundStart::Cloud
    );
    assert_eq!(
        background::start_decision(false, false, false),
        BackgroundStart::Disabled
    );
    assert_eq!(
        background::start_decision(false, true, true),
        BackgroundStart::Local
    );
    assert_eq!(
        background::start_decision(false, true, false),
        BackgroundStart::Waiting
    );
}

const RATE: usize = 48_000;
const WORD_MS: usize = 400;
const GAP_MS: usize = 200;
const SENTENCE_GAP_MS: usize = 800;
const VOCABULARY: [&str; 10] = [
    "", "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "end.",
];

/// Hears a word wherever the audio holds a steady level: level k / 20 is VOCABULARY[k].
#[derive(Default)]
struct WordEngine {
    transcribed_samples: usize,
    /// Samples heard by every engine sharing this counter, and how many were loaded.
    heard: Rc<Cell<usize>>,
    loads: Rc<Cell<usize>>,
}

impl SpeechEngine for WordEngine {
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
        samples: &[f32],
        _: Language,
        _: Option<&str>,
        progress: &mut dyn FnMut(f32),
        _: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        progress(0.0);
        self.transcribed_samples += samples.len();
        self.heard.set(self.heard.get() + samples.len());
        let ms = |index: usize| index as u64 * 1000 / 16_000;
        let mut words = Vec::new();
        let mut index = 0;
        while index < samples.len() {
            if samples[index].abs() < 0.02 {
                index += 1;
                continue;
            }
            let start = index;
            while index < samples.len() && samples[index].abs() >= 0.02 {
                index += 1;
            }
            // A word cut off at the edge of the audio is not heard.
            if index - start < 16 * 300 {
                continue;
            }
            let level = samples[(start + index) / 2];
            let word = VOCABULARY[((level * 20.0).round() as usize).min(9)];
            words.push(RawWord {
                start_ms: ms(start),
                end_ms: ms(index),
                text: word.into(),
            });
        }
        let (Some(first), Some(last)) = (words.first(), words.last()) else {
            return Ok(Vec::new());
        };
        Ok(vec![RawSegment {
            start_ms: first.start_ms,
            end_ms: last.end_ms,
            text: words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            words,
        }])
    }
}

/// 48 kHz audio speaking sentences of the given words, each followed by "end.".
fn speech(sentences: &[&[usize]], lead_ms: usize) -> Vec<f32> {
    let mut audio = vec![0.0; lead_ms * RATE / 1000];
    for sentence in sentences {
        for &word in sentence.iter().chain(&[9]) {
            audio.extend(std::iter::repeat_n(
                word as f32 / 20.0,
                WORD_MS * RATE / 1000,
            ));
            audio.extend(std::iter::repeat_n(0.0, GAP_MS * RATE / 1000));
        }
        audio.extend(std::iter::repeat_n(0.0, SENTENCE_GAP_MS * RATE / 1000));
    }
    audio
}

fn stereo(mono: &[f32]) -> Vec<f32> {
    mono.iter().flat_map(|&sample| [sample, sample]).collect()
}

struct Recording {
    _temp: TempDir,
    meeting_dir: PathBuf,
    chunk_dir: PathBuf,
    microphone: ChunkWriter,
    system: ChunkWriter,
}

fn begin_recording() -> Recording {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-13T09:30:00Z").unwrap();
    let (meeting, chunk_dir, _) = library.begin_recording(at).unwrap();
    Recording {
        meeting_dir: library.meeting_dir(&meeting.id),
        microphone: ChunkWriter::new(&chunk_dir, Source::Microphone).unwrap(),
        system: ChunkWriter::new(&chunk_dir, Source::SystemAudio).unwrap(),
        chunk_dir,
        _temp: temp,
    }
}

impl Recording {
    fn write(&mut self, microphone: &[f32], system: &[f32]) {
        self.microphone.write(&stereo(microphone)).unwrap();
        self.system.write(&stereo(system)).unwrap();
    }

    fn stop(self) -> (TempDir, PathBuf) {
        self.microphone.finish().unwrap();
        self.system.finish().unwrap();
        finalize_recording(&self.meeting_dir, 1).unwrap();
        (self._temp, self.meeting_dir)
    }
}

fn request(meeting_dir: &Path) -> TranscribeRequest<'_> {
    TranscribeRequest {
        meeting_dir,
        model: "small",
        vocabulary: &[],
    }
}

fn texts(transcript: &Transcript) -> Vec<(Speaker, String)> {
    transcript
        .segments
        .iter()
        .map(|segment| (segment.speaker, segment.text.clone()))
        .collect()
}

/// Microphone speech of about 75 s, with a sentence spoken across the 30 s Chunk edge.
fn microphone_speech() -> Vec<f32> {
    let sentences: Vec<Vec<usize>> = (0..20)
        .map(|n| vec![1 + n % 8, 1 + (n + 3) % 8, 1 + (n + 5) % 8, 1 + (n + 6) % 8])
        .collect();
    speech(
        &sentences.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        1_000,
    )
}

fn background_run(
    transcriber: &mut BackgroundTranscriber,
    engine: &mut WordEngine,
    chunk_dir: &Path,
    allowed: &mut dyn FnMut() -> bool,
) -> Vec<String> {
    let stop = AtomicBool::new(false);
    let waits = Cell::new(0);
    let mut finished = Vec::new();
    background::run(
        transcriber,
        &mut || {
            engine.loads.set(engine.loads.get() + 1);
            let heard = Rc::clone(&engine.heard);
            Some(EngineChain::new(
                vec![],
                "small",
                Box::new(move || {
                    Ok(Box::new(WordEngine {
                        heard: Rc::clone(&heard),
                        ..WordEngine::default()
                    }))
                }),
            ))
        },
        chunk_dir,
        BackgroundControl {
            stop: &stop,
            allowed,
            now: &mut || std::time::Duration::ZERO,
            wait: &mut || {
                waits.set(waits.get() + 1);
                if waits.get() > 3 {
                    stop.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
        },
        &mut |segments| finished.extend(segments.iter().map(|segment| segment.text.clone())),
    )
    .unwrap();
    finished
}

#[test]
fn chunks_are_transcribed_in_order_and_a_sentence_across_an_edge_stays_whole() {
    let microphone = microphone_speech();
    let mut recording = begin_recording();
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    let mut engine = WordEngine::default();
    let mut finished = Vec::new();
    for piece in microphone.chunks(10 * RATE) {
        recording.write(piece, &vec![0.0; piece.len()]);
        finished.extend(background_run(
            &mut transcriber,
            &mut engine,
            &recording.chunk_dir,
            &mut || true,
        ));
    }
    assert!(!finished.is_empty());
    let result = transcriber.finish();
    let source = result.source(Speaker::Me).unwrap();
    assert!(source.committed_ms >= 30_000);
    let words = source
        .segments
        .iter()
        .flat_map(|segment| &segment.words)
        .collect::<Vec<_>>();
    assert!(words
        .windows(2)
        .all(|pair| pair[0].start_ms < pair[1].start_ms));
    // No sentence is split: every committed span ends at a sentence end.
    for segment in &source.segments {
        assert!(segment.text.ends_with("end."), "{}", segment.text);
    }
    // The committed words are exactly the words the audio holds up to the committed point.
    let (_temp, meeting_dir) = recording.stop();
    let full = transcribe_meeting(
        &request(&meeting_dir),
        &mut EngineChain::borrow_local("small", &mut WordEngine::default()),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let expected = full
        .segments
        .iter()
        .filter(|segment| segment.speaker == Speaker::Me && segment.end_ms <= source.committed_ms)
        .map(|segment| segment.text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    let cached = source
        .segments
        .iter()
        .map(|segment| segment.text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(cached, expected);
}

#[test]
fn stop_transcribes_only_the_remainder_and_matches_transcription_after_stop() {
    let microphone = microphone_speech();
    let system = {
        let mut audio = vec![0.0; 20 * RATE];
        audio.extend(speech(&[&[2, 4, 6], &[8, 7, 6, 5, 4]], 0));
        audio.resize(microphone.len(), 0.0);
        audio
    };
    let mut recording = begin_recording();
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    let mut engine = WordEngine::default();
    for (mic, sys) in microphone.chunks(10 * RATE).zip(system.chunks(10 * RATE)) {
        recording.write(mic, sys);
        background_run(
            &mut transcriber,
            &mut engine,
            &recording.chunk_dir,
            &mut || true,
        );
    }
    let cache = transcriber.finish();
    let (_temp, meeting_dir) = recording.stop();

    let mut after_stop_only = WordEngine::default();
    let expected = transcribe_meeting(
        &request(&meeting_dir),
        &mut EngineChain::borrow_local("small", &mut after_stop_only),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    std::fs::remove_file(meeting_dir.join(format!("transcript.r{}.json", expected.revision)))
        .unwrap();

    let mut resuming = WordEngine::default();
    let mut draft = Vec::new();
    let mut first_progress = None;
    let transcript = transcribe_meeting_resuming(
        &request(&meeting_dir),
        &mut EngineChain::borrow_local("small", &mut resuming),
        &mut |fraction| {
            first_progress.get_or_insert(fraction);
        },
        &AtomicBool::new(false),
        None,
        &mut |segments| draft.push(segments.to_vec()),
        std::slice::from_ref(&cache),
    )
    .unwrap();

    assert_eq!(texts(&transcript), texts(&expected));
    for (actual, wanted) in transcript.segments.iter().zip(&expected.segments) {
        assert!(
            actual.start_ms.abs_diff(wanted.start_ms) <= 20
                && actual.end_ms.abs_diff(wanted.end_ms) <= 20
        );
    }
    let skipped: u64 = cache
        .sources
        .iter()
        .map(|source| source.committed_ms * 16)
        .sum();
    assert!(skipped > 0);
    assert_eq!(
        resuming.transcribed_samples as u64 + skipped,
        after_stop_only.transcribed_samples as u64
    );
    // The cached Segments are the Draft before any audio is transcribed, so it shows at once.
    let early = background_draft(&request(&meeting_dir), std::slice::from_ref(&cache)).unwrap();
    assert!(!early.is_empty());
    assert!(early
        .iter()
        .all(|segment| segment.end_ms <= cache.source(segment.speaker).unwrap().committed_ms));
    // Transcription then sends only Segments of the remaining audio.
    assert!(draft.iter().flatten().all(|segment| segment.end_ms
        > cache
            .source(segment.speaker)
            .map_or(0, |source| source.committed_ms)));
    assert!(first_progress.unwrap() > 0.0);
}

#[test]
fn work_waits_on_battery_and_resumes_on_ac() {
    let microphone = microphone_speech();
    let mut recording = begin_recording();
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    let mut engine = WordEngine::default();
    let on_ac = Cell::new(false);
    for piece in microphone.chunks(10 * RATE) {
        recording.write(piece, &vec![0.0; piece.len()]);
        background_run(
            &mut transcriber,
            &mut engine,
            &recording.chunk_dir,
            &mut || on_ac.get(),
        );
    }
    assert_eq!(engine.heard.get(), 0, "nothing is transcribed on battery");
    assert_eq!(engine.loads.get(), 0, "no engine holds the GPU on battery");
    on_ac.set(true);
    background_run(
        &mut transcriber,
        &mut engine,
        &recording.chunk_dir,
        &mut || on_ac.get(),
    );
    let result = transcriber.finish();
    assert!(result.source(Speaker::Me).unwrap().committed_ms >= 30_000);
}

#[test]
fn a_cache_from_another_model_is_ignored() {
    let microphone = microphone_speech();
    let mut recording = begin_recording();
    let mut transcriber = BackgroundTranscriber::new(1, "base", Some(Language::En), None);
    let mut engine = WordEngine::default();
    for piece in microphone.chunks(10 * RATE) {
        recording.write(piece, &vec![0.0; piece.len()]);
        background_run(
            &mut transcriber,
            &mut engine,
            &recording.chunk_dir,
            &mut || true,
        );
    }
    let cache = transcriber.finish();
    assert!(cache.source(Speaker::Me).is_some());
    let (_temp, meeting_dir) = recording.stop();
    let mut plain = WordEngine::default();
    transcribe_meeting(
        &request(&meeting_dir),
        &mut EngineChain::borrow_local("small", &mut plain),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let mut resuming = WordEngine::default();
    transcribe_meeting_resuming(
        &request(&meeting_dir),
        &mut EngineChain::borrow_local("small", &mut resuming),
        &mut |_| {},
        &AtomicBool::new(false),
        None,
        &mut |_| {},
        &[cache],
    )
    .unwrap();
    assert_eq!(resuming.transcribed_samples, plain.transcribed_samples);
}

#[test]
fn background_work_writes_nothing_so_a_crash_leaves_only_the_chunks() {
    let microphone = microphone_speech();
    let mut recording = begin_recording();
    let listing = |dir: &Path| {
        let mut names = walk(dir);
        names.sort();
        names
    };
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    let mut engine = WordEngine::default();
    let mut before = Vec::new();
    for piece in microphone.chunks(10 * RATE) {
        recording.write(piece, &vec![0.0; piece.len()]);
        before = listing(&recording.meeting_dir);
        background_run(
            &mut transcriber,
            &mut engine,
            &recording.chunk_dir,
            &mut || true,
        );
        assert_eq!(listing(&recording.meeting_dir), before);
    }
    assert!(engine.heard.get() > 0);
    // A crash drops the transcriber; the Meeting then transcribes from its Chunks as usual.
    drop(transcriber);
    let (_temp, meeting_dir) = recording.stop();
    let transcript = transcribe_meeting(
        &request(&meeting_dir),
        &mut EngineChain::borrow_local("small", &mut WordEngine::default()),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(transcript
        .segments
        .iter()
        .all(|segment| segment.speaker == Speaker::Me));
    assert!(!before.is_empty());
}

fn walk(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            names.extend(walk(&path));
        } else {
            names.push(path.display().to_string());
        }
    }
    names
}

#[test]
fn audio_without_a_sentence_end_waits_for_more_instead_of_transcribing_again() {
    // 40 s of words with no sentence end, then silence: one span, then idle until more audio.
    let mut audio = Vec::new();
    for word in 0..60 {
        audio.extend(std::iter::repeat_n(
            (1 + word % 8) as f32 / 20.0,
            WORD_MS * RATE / 1000,
        ));
        audio.extend(std::iter::repeat_n(0.0, GAP_MS * RATE / 1000));
    }
    audio.truncate(40 * RATE);
    let mut transcriber = BackgroundTranscriber::new(1, "small", Some(Language::En), None);
    transcriber.push(Source::Microphone, &audio);
    let mut engine = WordEngine::default();
    assert!(transcriber.span_ready());
    transcriber
        .transcribe_ready(
            &mut EngineChain::borrow_local("small", &mut engine),
            &AtomicBool::new(false),
        )
        .unwrap();
    let after_one_span = engine.transcribed_samples;
    assert!(after_one_span > 0);
    assert!(
        !transcriber.span_ready(),
        "the same audio is not transcribed again"
    );
    transcriber.push(Source::Microphone, &vec![0.0; 30 * RATE]);
    assert!(transcriber.span_ready());
}

#[test]
fn silence_before_the_language_is_known_is_skipped_not_rescanned() {
    let mut transcriber = BackgroundTranscriber::new(1, "small", None, None);
    transcriber.push(Source::Microphone, &vec![0.0; 40 * RATE]);
    let mut engine = WordEngine::default();
    transcriber
        .transcribe_ready(
            &mut EngineChain::borrow_local("small", &mut engine),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert!(!transcriber.span_ready());
    assert_eq!(transcriber.finish().sources[0].committed_ms, 40_000);
}
