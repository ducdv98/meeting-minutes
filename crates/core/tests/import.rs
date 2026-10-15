use chrono::DateTime;
use meeting_minutes_core::transcript::Language;
use meeting_minutes_core::{Library, LibraryError};
use std::{f32::consts::PI, fs, path::Path, sync::atomic::AtomicBool};
use tempfile::TempDir;

fn wav(path: &Path, rate: u32, hz: f32, seconds: f32) {
    let frames = (rate as f32 * seconds) as u32;
    let data_len = frames * 4;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..frames {
        let sample = ((2.0 * PI * hz * n as f32 / rate as f32).sin() * 12000.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn new_import_meeting_saves_its_language_choice() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("speech.wav");
    wav(&source, 16_000, 440.0, 0.1);
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    for (at, language, expected) in [
        (
            "2026-10-12T10:00:00Z",
            Some(Language::Vi),
            serde_json::json!("vi"),
        ),
        ("2026-10-12T10:01:00Z", None, serde_json::Value::Null),
    ] {
        let meeting = library
            .import_audio_with_language(
                &source,
                DateTime::parse_from_rfc3339(at).unwrap(),
                language,
                &AtomicBool::new(false),
            )
            .unwrap();
        let metadata: serde_json::Value = serde_json::from_slice(
            &fs::read(library.meeting_dir(&meeting.id).join("meeting.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["language"], expected);
    }
}

#[test]
fn importing_wav_copies_source_and_creates_playable_meeting() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("Team sync.WAV");
    wav(&source, 44_100, 1000.0, 2.5);
    let original = fs::read(&source).unwrap();
    let modified = fs::metadata(&source).unwrap().modified().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00+07:00").unwrap();
    let meeting = library
        .import_audio(&source, at, &AtomicBool::new(false))
        .unwrap();
    let folder = library.meeting_dir(&meeting.id);
    assert_eq!(meeting.title, "Team sync");
    assert_eq!(meeting.id, "2026-10-12 1000");
    assert_eq!(meeting.duration_ms, 2500);
    assert_eq!(fs::read(folder.join("imported.wav")).unwrap(), original);
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(folder.join("meeting.json")).unwrap()).unwrap();
    assert_eq!(metadata["audio"], "imported.wav");
    assert_eq!(metadata["import"]["file"], "imported.wav");
    assert_eq!(metadata["import"]["original_name"], "Team sync.WAV");
    assert_eq!(
        metadata["import"]["imported_at"],
        "2026-10-12T10:00:00+07:00"
    );
    assert_eq!(metadata["recordings"].as_array().unwrap().len(), 0);
    assert!(library.meeting(&meeting.id).unwrap().imported);
    assert_eq!(
        library.meeting(&meeting.id).unwrap().audio_path,
        Some(folder.join("imported.wav"))
    );
    assert_eq!(library.list_meetings().unwrap(), vec![meeting]);
}

#[test]
fn imported_meeting_rejects_new_recordings_and_rename_preserves_audio() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("Sound.wav");
    wav(&source, 44_100, 1000.0, 0.1);
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap();
    let meeting = library
        .import_audio(&source, at, &AtomicBool::new(false))
        .unwrap();
    assert!(matches!(
        library.begin_next_recording(&meeting.id, at),
        Err(LibraryError::ImportedMeeting)
    ));
    let renamed = library.rename_meeting(&meeting.id, "Renamed").unwrap();
    assert_eq!(
        library.meeting(&renamed.id).unwrap().audio_path,
        Some(library.meeting_dir(&renamed.id).join("imported.wav"))
    );
}

#[test]
fn import_rejects_bad_sources_without_publishing_folders() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap();
    let bad = temp.path().join("bad.txt");
    fs::write(&bad, b"not audio").unwrap();
    assert!(library
        .import_audio(&bad, at, &AtomicBool::new(false))
        .is_err());
    let corrupt = temp.path().join("corrupt.wav");
    fs::write(&corrupt, b"not audio").unwrap();
    assert!(library
        .import_audio(&corrupt, at, &AtomicBool::new(false))
        .is_err());
    let good = temp.path().join("good.wav");
    wav(&good, 44_100, 1000.0, 0.1);
    assert!(library
        .import_audio(&good, at, &AtomicBool::new(true))
        .is_err());
    assert!(fs::read_dir(&root).unwrap().next().is_none());
}

#[test]
fn import_uses_collision_suffix_without_replacing_existing_meeting() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("Sound.wav");
    wav(&source, 44_100, 1000.0, 0.1);
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap();
    let first = library
        .import_audio(&source, at, &AtomicBool::new(false))
        .unwrap();
    let second = library
        .import_audio(&source, at, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(second.id, format!("{} (2)", first.id));
    assert!(library
        .meeting_dir(&first.id)
        .join("imported.wav")
        .is_file());
}

#[test]
fn preparation_preserves_speech_tone_and_rejects_high_frequency_alias() {
    use meeting_minutes_core::audio_prep::prepare_import;
    let temp = TempDir::new().unwrap();
    wav(&temp.path().join("tone.wav"), 44_100, 1000.0, 1.0);
    wav(&temp.path().join("high.wav"), 44_100, 10000.0, 1.0);
    let cancel = AtomicBool::new(false);
    let tone = prepare_import(temp.path(), "tone.wav", &cancel).unwrap();
    let high = prepare_import(temp.path(), "high.wav", &cancel).unwrap();
    assert!((tone.samples_16k.len() as i64 - 16_000).abs() < 160);
    assert_eq!(tone.duration_ms, 1000);
    assert_eq!(
        tone.speaker,
        meeting_minutes_core::transcript::Speaker::Unknown
    );
    let rms = |samples: &[f32]| {
        (samples
            .iter()
            .skip(1000)
            .take(14000)
            .map(|x| x * x)
            .sum::<f32>()
            / 14000.0)
            .sqrt()
    };
    let expected = 12000.0 / 32768.0 / 2.0f32.sqrt();
    assert!((20.0 * (rms(&tone.samples_16k) / expected).log10()).abs() < 1.0);
    assert!(20.0 * (rms(&high.samples_16k) / rms(&tone.samples_16k)).log10() < -20.0);
}

struct FakeEngine;
impl meeting_minutes_core::processing::SpeechEngine for FakeEngine {
    fn backend(&self) -> &str {
        "CPU"
    }
    fn thread_count(&self) -> usize {
        1
    }
    fn detect_language(
        &mut self,
        _: &[f32],
        _: &AtomicBool,
    ) -> Result<meeting_minutes_core::transcript::Language, String> {
        Ok(meeting_minutes_core::transcript::Language::En)
    }
    fn transcribe(
        &mut self,
        _: &[f32],
        _: meeting_minutes_core::transcript::Language,
        _: Option<&str>,
        _: &mut dyn FnMut(f32),
        _: &AtomicBool,
    ) -> Result<
        Vec<meeting_minutes_core::transcript::RawSegment>,
        meeting_minutes_core::processing::ProcessingError,
    > {
        Ok(vec![meeting_minutes_core::transcript::RawSegment {
            start_ms: 0,
            end_ms: 120,
            text: "hello".into(),
            words: vec![],
        }])
    }
}

#[test]
fn imported_audio_transcribes_with_unknown_speaker() {
    use meeting_minutes_core::processing::{transcribe_meeting, EngineChain, TranscribeRequest};
    use meeting_minutes_core::transcript::Speaker;
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("Sound.wav");
    wav(&source, 44_100, 1000.0, 1.0);
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap();
    let meeting = library
        .import_audio(&source, at, &AtomicBool::new(false))
        .unwrap();
    let folder = library.meeting_dir(&meeting.id);
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &folder,
            model: "small",
            vocabulary: &[],
        },
        &mut EngineChain::borrow_local("small", &mut FakeEngine),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(transcript.revision, 1);
    assert_eq!(transcript.segments.len(), 1);
    assert_eq!(transcript.segments[0].speaker, Speaker::Unknown);
    assert!(transcript.segments[0].end_ms <= meeting.duration_ms);
    assert!(folder.join("transcript.r1.json").exists());
}

#[test]
fn imported_audio_has_unknown_managed_source() {
    use meeting_minutes_core::{
        capture::Source,
        cloud::{CloudEngine, EngineError},
        processing::{transcribe_meeting, EngineChain, TranscribeRequest},
        transcript::{Language, RawSegment},
    };
    use std::sync::{Arc, Mutex};
    struct SourceCloud {
        source: Option<Source>,
        sent: Arc<Mutex<Vec<Option<Source>>>>,
    }
    impl CloudEngine for SourceCloud {
        fn name(&self) -> &str {
            "Managed transcription"
        }
        fn set_source(&mut self, source: Option<Source>) {
            self.source = source;
        }
        fn transcribe_span(
            &mut self,
            _: &[f32],
            _: Language,
            _: Option<&str>,
            _: &AtomicBool,
        ) -> Result<Vec<RawSegment>, EngineError> {
            self.sent.lock().unwrap().push(self.source);
            Ok(vec![])
        }
    }
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("Sound.wav");
    wav(&source, 44_100, 1000.0, 1.0);
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap();
    let meeting = library
        .import_audio(&source, at, &AtomicBool::new(false))
        .unwrap();
    let folder = library.meeting_dir(&meeting.id);
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut engine = EngineChain::new(
        vec![Box::new(SourceCloud {
            source: Some(Source::Microphone),
            sent: sent.clone(),
        })],
        "small",
        Box::new(|| panic!("local should not load")),
    );
    transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &folder,
            model: "small",
            vocabulary: &[],
        },
        &mut engine,
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(*sent.lock().unwrap(), vec![None]);
}

fn flac(path: &Path) {
    use flacenc::{
        bitsink::MemSink,
        component::{BitRepr, StreamInfo},
        config,
        error::Verify,
        source::{Fill, FrameBuf},
    };
    let config = config::Encoder::default().into_verified().unwrap();
    let mut info = StreamInfo::new(48_000, 1, 16).unwrap();
    info.set_total_samples(48_000);
    info.set_block_sizes(4096, 4096).unwrap();
    info.set_frame_sizes(0, 0).unwrap();
    let mut sink = MemSink::<u8>::new();
    info.write(&mut sink).unwrap();
    let mut bytes = b"fLaC".to_vec();
    bytes.extend_from_slice(&[0x80, 0, 0, 34]);
    bytes.extend_from_slice(sink.as_slice());
    let mut framebuf = FrameBuf::with_size(1, 4096).unwrap();
    for number in 0..12 {
        let count = if number == 11 { 2944 } else { 4096 };
        let samples: Vec<i32> = (0..count)
            .map(|n| {
                ((2.0 * PI * 1000.0 * (number * 4096 + n) as f32 / 48_000.0).sin() * 12000.0) as i32
            })
            .collect();
        framebuf.fill_interleaved(&samples).unwrap();
        sink.clear();
        flacenc::encode_fixed_size_frame(&config, &framebuf, number as usize, &info)
            .unwrap()
            .write(&mut sink)
            .unwrap();
        bytes.extend_from_slice(sink.as_slice());
    }
    fs::write(path, bytes).unwrap();
}

fn mp3(path: &Path) {
    use meeting_minutes_core::{capture::Source, finalize_recording, recording::ChunkWriter};
    let root = path.parent().unwrap();
    let mut library = Library::open(
        root.join("fixture-library"),
        root.join("fixture-index.sqlite3"),
    )
    .unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T09:00:00Z").unwrap();
    let (meeting, chunks, _) = library.begin_recording(at).unwrap();
    let mut microphone = ChunkWriter::new(&chunks, Source::Microphone).unwrap();
    let mut system = ChunkWriter::new(&chunks, Source::SystemAudio).unwrap();
    microphone.write(&vec![0.2; 4800]).unwrap();
    system.write(&vec![0.0; 9600]).unwrap();
    microphone.finish().unwrap();
    system.finish().unwrap();
    let folder = library.meeting_dir(&meeting.id);
    finalize_recording(&folder, 1).unwrap();
    fs::copy(folder.join("meeting.mp3"), path).unwrap();
}

#[test]
fn decodes_and_imports_flac_and_mp3() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap();
    for (name, create) in [
        ("Sound.mp3", mp3 as fn(&Path)),
        ("Sound.flac", flac as fn(&Path)),
    ] {
        let source = temp.path().join(name);
        create(&source);
        let meeting = library
            .import_audio(&source, at, &AtomicBool::new(false))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let expected = if name.ends_with("flac") { 1000 } else { 100 };
        assert!(
            (meeting.duration_ms as i64 - expected).abs() <= 50,
            "{name}: {} ms",
            meeting.duration_ms
        );
        let extension = source.extension().unwrap().to_str().unwrap();
        assert_eq!(
            fs::read(
                library
                    .meeting_dir(&meeting.id)
                    .join(format!("imported.{extension}"))
            )
            .unwrap(),
            fs::read(source).unwrap()
        );
    }
}

#[test]
fn preparation_resamples_low_and_odd_rates_to_16k() {
    use meeting_minutes_core::audio_prep::prepare_import;
    let temp = TempDir::new().unwrap();
    let cancel = AtomicBool::new(false);
    for (rate, seconds) in [(8_000, 1.5), (22_050, 1.5), (44_101, 0.5)] {
        let name = format!("tone-{rate}.wav");
        wav(&temp.path().join(&name), rate, 1000.0, seconds);
        let prepared = prepare_import(temp.path(), &name, &cancel).unwrap();
        let expected = (16_000.0 * seconds) as i64;
        assert!(
            (prepared.samples_16k.len() as i64 - expected).abs() <= 1,
            "{rate} Hz"
        );
        let middle = &prepared.samples_16k[1000..expected as usize - 1000];
        let rms = (middle.iter().map(|x| x * x).sum::<f32>() / middle.len() as f32).sqrt();
        let target = 12000.0 / 32768.0 / 2.0f32.sqrt();
        assert!(
            (20.0 * (rms / target).log10()).abs() < 1.0,
            "{rate} Hz: {rms}"
        );
    }
}

#[test]
fn import_refuses_sample_rates_it_cannot_transcribe() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let at = DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap();
    let source = temp.path().join("slow.wav");
    wav(&source, 7_000, 1000.0, 0.5);
    let error = library
        .import_audio(&source, at, &AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("7000 Hz"), "{error}");
    assert!(fs::read_dir(&root).unwrap().next().is_none());
}
