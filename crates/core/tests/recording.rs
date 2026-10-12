use meeting_minutes_core::{
    capture::{Clock, InputDevice, InputOpener, Packet, Source, SourceClock},
    finalize_recording,
    recording::{CaptureSession, ChunkWriter, Marker, RecordingClock, RecordingManifest},
    settings::Settings,
    Library,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tempfile::TempDir;

#[derive(Clone)]
struct FakeClock(Arc<AtomicU64>);
impl Clock for FakeClock {
    fn now_100ns(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
#[derive(Clone)]
struct SilentOpener;
struct SilentDevice;
impl InputOpener for SilentOpener {
    type Device = SilentDevice;
    fn open(&self, _: Source) -> Result<Self::Device, String> {
        Ok(SilentDevice)
    }
}
impl InputDevice for SilentDevice {
    fn read_packet(&mut self) -> Result<Option<Packet>, String> {
        Ok(None)
    }
}

#[test]
fn session_stop_pads_both_sources_and_records_pause() {
    let temp = TempDir::new().unwrap();
    let time = Arc::new(AtomicU64::new(1_000_000));
    let mut session = CaptureSession::start(
        temp.path().to_path_buf(),
        RecordingManifest {
            number: 1,
            started_at: "2026-10-12T09:30:00Z".into(),
            duration_ms: None,
            markers: vec![],
        },
        SilentOpener,
        FakeClock(time.clone()),
    )
    .unwrap();
    time.store(6_000_000, Ordering::SeqCst);
    session.pause().unwrap();
    time.store(16_000_000, Ordering::SeqCst);
    session.resume().unwrap();
    time.store(18_000_000, Ordering::SeqCst);
    let manifest = session.stop().unwrap();
    assert_eq!(manifest.duration_ms, Some(700));
    assert_eq!(
        manifest.markers,
        vec![Marker::Pause {
            at_ms: 500,
            paused_ms: 1000
        }]
    );
    assert_eq!(
        std::fs::metadata(temp.path().join("microphone-000000.f32"))
            .unwrap()
            .len(),
        33_600 * 4
    );
    assert_eq!(
        std::fs::metadata(temp.path().join("system-000000.f32"))
            .unwrap()
            .len(),
        33_600 * 8
    );
}

#[test]
fn stopping_while_paused_records_the_final_pause() {
    let temp = TempDir::new().unwrap();
    let time = Arc::new(AtomicU64::new(1_000_000));
    let mut session = CaptureSession::start(
        temp.path().to_path_buf(),
        RecordingManifest {
            number: 1,
            started_at: "2026-10-12T09:30:00Z".into(),
            duration_ms: None,
            markers: vec![],
        },
        SilentOpener,
        FakeClock(time.clone()),
    )
    .unwrap();
    time.store(6_000_000, Ordering::SeqCst);
    session.pause().unwrap();
    time.store(16_000_000, Ordering::SeqCst);
    let manifest = session.stop().unwrap();
    assert_eq!(manifest.duration_ms, Some(500));
    assert_eq!(
        manifest.markers,
        vec![Marker::Pause {
            at_ms: 500,
            paused_ms: 1000
        }]
    );
}

#[test]
fn source_clock_places_packets_and_excludes_pause() {
    let mut clock = SourceClock::new(1_000_000);
    let packet = Packet {
        samples: vec![0.5; 960],
        qpc_100ns: Some(1_000_000),
        silent: false,
        discontinuity: false,
    };
    assert_eq!(clock.packet(&packet, 1_000_000).len(), 960);
    assert_eq!(clock.pause(1_100_000).len(), 0);
    clock.resume(2_000_000);
    assert!(clock.packet(&packet, 2_000_000).is_empty());
    assert_eq!(clock.pad_to(2_100_000, true).len(), 960);
    assert_eq!(clock.frames_written(), 960);
}

#[test]
fn source_clock_handles_gaps_silence_and_idle_overlap() {
    let start = 1_000_000;
    let mut clock = SourceClock::new(start);
    let first = Packet {
        samples: vec![1.0; 480 * 2],
        qpc_100ns: Some(start),
        silent: false,
        discontinuity: false,
    };
    assert_eq!(clock.packet(&first, start).len(), 960);
    let second = Packet {
        samples: vec![1.0; 480 * 2],
        qpc_100ns: Some(start + 300_000),
        silent: true,
        discontinuity: false,
    };
    let placed = clock.packet(&second, start + 300_000);
    assert_eq!(placed.len(), 1_440 * 2);
    assert!(placed.iter().all(|v| *v == 0.0));
    assert_eq!(clock.pad_to(start + 5_000_000, false).len(), 12_480 * 2);
    assert_eq!(clock.pad_to(start + 5_000_000, true).len(), 9_600 * 2);
    assert_eq!(clock.frames_written(), 24_000);
}

#[test]
fn source_clock_pause_pads_and_resume_drops_old_packets() {
    let start = 1_000_000;
    let mut clock = SourceClock::new(start);
    assert_eq!(clock.pause(start + 100_000).len(), 480 * 2);
    clock.resume(start + 1_100_000);
    let stale = Packet {
        samples: vec![1.0; 480 * 2],
        qpc_100ns: Some(start + 50_000),
        silent: false,
        discontinuity: false,
    };
    assert!(clock.packet(&stale, start + 1_100_000).is_empty());
    assert_eq!(clock.pad_to(start + 1_200_000, true).len(), 480 * 2);
    assert_eq!(clock.frames_written(), 960);
}

#[test]
fn recording_clock_records_pause_duration() {
    let mut clock = RecordingClock::new(1_000_000);
    clock.pause(6_000_000);
    clock.pause(7_000_000);
    assert!(clock.is_paused());
    assert_eq!(clock.elapsed_ms(9_000_000), 500);
    clock.resume(16_000_000);
    assert_eq!(clock.elapsed_ms(18_000_000), 700);
    assert_eq!(
        clock.markers(),
        &[Marker::Pause {
            at_ms: 500,
            paused_ms: 1000
        }]
    );
}

#[test]
fn chunks_rotate_and_downmix_microphone() {
    let temp = TempDir::new().unwrap();
    let mut writer = ChunkWriter::with_chunk_frames(temp.path(), Source::Microphone, 2).unwrap();
    writer.write(&[1.0, 0.0, 0.0, -1.0, 0.5, 0.5]).unwrap();
    assert!(temp.path().join("microphone-000000.f32").exists());
    assert!(temp.path().join("microphone-000001.part").exists());
    writer.finish().unwrap();
    let bytes = std::fs::read(temp.path().join("microphone-000000.f32")).unwrap();
    assert_eq!(bytes.len(), 8);
    assert_eq!(f32::from_le_bytes(bytes[0..4].try_into().unwrap()), 0.5);
    assert_eq!(f32::from_le_bytes(bytes[4..8].try_into().unwrap()), -0.5);
}

#[test]
fn chunk_completion_keeps_an_existing_file() {
    let temp = TempDir::new().unwrap();
    let completed = temp.path().join("microphone-000000.f32");
    std::fs::write(&completed, b"foreign").unwrap();
    let mut writer = ChunkWriter::with_chunk_frames(temp.path(), Source::Microphone, 1).unwrap();
    assert!(writer.write(&[0.5, 0.5]).is_err());
    assert_eq!(std::fs::read(completed).unwrap(), b"foreign");
}

#[test]
fn settings_default_and_round_trip() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings.json");
    assert!(!Settings::load(&path).recording_notice_acknowledged);
    Settings {
        recording_notice_acknowledged: true,
    }
    .save(&path)
    .unwrap();
    assert!(Settings::load(&path).recording_notice_acknowledged);
    std::fs::write(&path, b"invalid").unwrap();
    assert!(!Settings::load(&path).recording_notice_acknowledged);
}

#[test]
fn finalise_streams_playable_audio_and_refreshes_index() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let (meeting, chunks, manifest) = library
        .begin_recording(chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap())
        .unwrap();
    assert_eq!(manifest.number, 1);
    for source in [Source::Microphone, Source::SystemAudio] {
        let mut writer = ChunkWriter::new(&chunks, source).unwrap();
        writer.write(&vec![0.25; 4_800 * 2]).unwrap();
        writer.finish().unwrap();
    }
    let mut manifest = manifest;
    manifest.duration_ms = Some(100);
    std::fs::write(
        chunks.join("recording.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let dir = library.meeting_dir(&meeting.id);
    let result = finalize_recording(&dir, 1).unwrap();
    assert_eq!(result.duration_ms, 100);
    assert!(!chunks.exists());
    for (file, channels) in [
        ("recording-1 microphone.flac", 1),
        ("recording-1 system.flac", 2),
    ] {
        let mut reader = claxon::FlacReader::open(dir.join(file)).unwrap();
        assert_eq!(reader.streaminfo().channels, channels);
        assert_eq!(reader.streaminfo().samples, Some(4_800));
        assert_eq!(reader.samples().count(), 4_800 * channels as usize);
    }
    let mut decoder = minimp3::Decoder::new(std::fs::File::open(dir.join("meeting.mp3")).unwrap());
    let mut decoded = 0;
    while let Ok(frame) = decoder.next_frame() {
        decoded += frame.data.len() / frame.channels;
    }
    // minimp3 ignores the gapless tag, so it also returns the encoder delay and padding.
    assert!(decoded >= 4_800, "decoded {decoded} MP3 frames");
    assert_eq!(
        library.record_finalized(&meeting.id).unwrap().duration_ms,
        100
    );
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("meeting.json")).unwrap()).unwrap();
    assert_eq!(json["recordings"][0]["files"][1]["source"], "system_audio");
}

#[test]
fn failed_finalisation_preserves_foreign_file_and_allows_retry() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let (meeting, chunks, mut manifest) = library
        .begin_recording(chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap())
        .unwrap();
    manifest.duration_ms = Some(0);
    std::fs::write(
        chunks.join("recording.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let dir = library.meeting_dir(&meeting.id);
    let blocker = dir.join("recording-1 system.flac");
    std::fs::write(&blocker, b"foreign").unwrap();
    assert!(finalize_recording(&dir, 1).is_err());
    assert_eq!(std::fs::read(&blocker).unwrap(), b"foreign");
    assert!(!dir.join("recording-1 microphone.flac").exists());
    assert!(chunks.exists());
    std::fs::remove_file(blocker).unwrap();
    finalize_recording(&dir, 1).unwrap();
    assert!(!chunks.exists());
}

#[test]
fn a_short_recording_preserves_its_single_frame() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let (meeting, chunks, mut manifest) = library
        .begin_recording(chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap())
        .unwrap();
    for source in [Source::Microphone, Source::SystemAudio] {
        let mut writer = ChunkWriter::new(&chunks, source).unwrap();
        writer.write(&[0.25, 0.25]).unwrap();
        writer.finish().unwrap();
    }
    manifest.duration_ms = Some(0);
    std::fs::write(
        chunks.join("recording.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let dir = library.meeting_dir(&meeting.id);
    finalize_recording(&dir, 1).unwrap();
    let mut reader = claxon::FlacReader::open(dir.join("recording-1 microphone.flac")).unwrap();
    assert_eq!(
        reader
            .samples()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn finalisation_pads_the_shorter_source_without_losing_audio() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let (meeting, chunks, mut manifest) = library
        .begin_recording(chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap())
        .unwrap();
    let mut mic = ChunkWriter::new(&chunks, Source::Microphone).unwrap();
    mic.write(&[0.5; 24 * 2]).unwrap();
    mic.finish().unwrap();
    let mut system = ChunkWriter::new(&chunks, Source::SystemAudio).unwrap();
    system.write(&vec![0.25; 48 * 2]).unwrap();
    system.finish().unwrap();
    manifest.duration_ms = Some(1);
    std::fs::write(
        chunks.join("recording.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let dir = library.meeting_dir(&meeting.id);
    let result = finalize_recording(&dir, 1).unwrap();
    assert_eq!(result.duration_ms, 1);
    let mut reader = claxon::FlacReader::open(dir.join("recording-1 microphone.flac")).unwrap();
    let samples = reader.samples().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(samples.len(), 48);
    assert!(samples[24..].iter().all(|sample| *sample == 0));
}

#[test]
fn chunk_write_failure_is_reported_when_the_recording_stops() {
    let temp = TempDir::new().unwrap();
    let chunks = temp.path().join("chunks");
    std::fs::create_dir_all(&chunks).unwrap();
    // A foreign file where the first completed microphone Chunk must go.
    std::fs::write(chunks.join("microphone-000000.f32"), b"foreign").unwrap();
    let time = Arc::new(AtomicU64::new(1_000_000));
    let session = CaptureSession::start(
        chunks.clone(),
        RecordingManifest {
            number: 1,
            started_at: "2026-10-12T09:30:00Z".into(),
            duration_ms: None,
            markers: Vec::new(),
        },
        SilentOpener,
        FakeClock(time.clone()),
    )
    .unwrap();
    time.fetch_add(150_000_000, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(200));

    let error = session.stop().unwrap_err().to_string();

    assert!(error.contains("microphone capture stopped"), "{error}");
    assert_eq!(
        std::fs::read(chunks.join("microphone-000000.f32")).unwrap(),
        b"foreign"
    );
}

#[test]
fn mp3_carries_gapless_info_so_players_trim_the_encoder_delay() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let (meeting, chunks, _) = library
        .begin_recording(chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap())
        .unwrap();
    for source in [Source::Microphone, Source::SystemAudio] {
        let mut writer = ChunkWriter::new(&chunks, source).unwrap();
        writer.write(&vec![0.25; 4_800 * 2]).unwrap();
        writer.finish().unwrap();
    }
    let dir = library.meeting_dir(&meeting.id);
    finalize_recording(&dir, 1).unwrap();

    let mp3 = std::fs::read(dir.join("meeting.mp3")).unwrap();
    // 128 kbps at 48 kHz: every frame is 384 bytes; the first holds the Info/LAME tag.
    const FRAME_BYTES: usize = 384;
    let first = &mp3[..FRAME_BYTES];
    assert!(first.windows(4).any(|w| w == b"Info"), "no Info tag");
    let lame = first
        .windows(4)
        .position(|w| w == b"LAME")
        .expect("no LAME tag");
    let gapless = &first[lame + 21..lame + 24];
    let delay = (usize::from(gapless[0]) << 4) | (usize::from(gapless[1]) >> 4);
    let padding = (usize::from(gapless[1] & 0x0f) << 8) | usize::from(gapless[2]);
    assert!(delay > 0);
    let audio_frames = mp3.len() / FRAME_BYTES - 1;
    assert_eq!(audio_frames * 1152 - delay - padding, 4_800);
}
