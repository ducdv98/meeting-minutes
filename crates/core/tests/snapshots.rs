use meeting_minutes_core::{
    capture::{Clock, DeviceInfo, InputDevice, InputOpener, Packet, Source},
    recording::CaptureSession,
    snapshot::{CaptureRegion, RgbaFrame, ScreenGrabber, SnapshotError, SnapshotStore},
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
    fn default_device_id(&self, _: Source) -> Result<String, String> {
        Ok("silent".into())
    }
}
impl InputDevice for SilentDevice {
    fn read_packet(&mut self) -> Result<Option<Packet>, String> {
        Ok(None)
    }
    fn info(&self) -> &DeviceInfo {
        static INFO: std::sync::LazyLock<DeviceInfo> = std::sync::LazyLock::new(|| DeviceInfo {
            id: "silent".into(),
            name: "Silent".into(),
        });
        &INFO
    }
}

struct FakeGrabber;
impl ScreenGrabber for FakeGrabber {
    fn grab(&self, region: CaptureRegion) -> Result<RgbaFrame, String> {
        assert_eq!(
            region,
            CaptureRegion {
                x: 10,
                y: 20,
                width: 2,
                height: 1
            }
        );
        Ok(RgbaFrame {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 255, 0, 255],
        })
    }
}

fn region() -> CaptureRegion {
    CaptureRegion {
        x: 10,
        y: 20,
        width: 2,
        height: 1,
    }
}

fn setup() -> (
    TempDir,
    Library,
    String,
    CaptureSession<FakeClock>,
    Arc<AtomicU64>,
) {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let (meeting, chunks, manifest) = library
        .begin_recording(chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap())
        .unwrap();
    let now = Arc::new(AtomicU64::new(1_000_000));
    let session =
        CaptureSession::start(chunks, manifest, SilentOpener, FakeClock(now.clone())).unwrap();
    (temp, library, meeting.id, session, now)
}

#[test]
fn takes_native_png_with_timeline_position_and_rebuilds_from_folder() {
    let (_temp, library, id, session, now) = setup();
    let store = SnapshotStore::new(library.meeting_dir(&id));
    now.store(1_000_000 + 872_000 * 10_000, Ordering::SeqCst);

    let snapshot = store.take(Some(&session), region(), &FakeGrabber).unwrap();

    assert_eq!(snapshot.timeline_ms, 872_000);
    assert_eq!(snapshot.order, 1);
    assert_eq!(snapshot.caption, None);
    assert_eq!(snapshot.png_path.file_name().unwrap(), "0001-00-14-32.png");
    let sidecar: serde_json::Value =
        serde_json::from_slice(&std::fs::read(snapshot.png_path.with_extension("json")).unwrap())
            .unwrap();
    assert_eq!(sidecar["timeline_ms"], 872_000);
    assert_eq!(sidecar["order"], 1);
    assert!(sidecar.get("caption").is_some());
    let png = std::fs::read(&snapshot.png_path).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let decoder = png::Decoder::new(std::io::Cursor::new(&png));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert_eq!(
        &pixels[..info.buffer_size()],
        &[255, 0, 0, 255, 0, 255, 0, 255]
    );

    let rebuilt = SnapshotStore::new(library.meeting_dir(&id));
    assert_eq!(rebuilt.list().unwrap(), vec![snapshot.clone()]);
    assert_eq!(rebuilt.load(1).unwrap().png, png);
    session.stop().unwrap();
}

#[test]
fn snapshots_during_pause_keep_the_marker_position_and_capture_order() {
    let (_temp, library, id, mut session, now) = setup();
    let store = SnapshotStore::new(library.meeting_dir(&id));
    now.store(1_000_000 + 5_000 * 10_000, Ordering::SeqCst);
    session.pause().unwrap();
    now.store(1_000_000 + 10_000 * 10_000, Ordering::SeqCst);

    let first = store.take(Some(&session), region(), &FakeGrabber).unwrap();
    let second = store.take(Some(&session), region(), &FakeGrabber).unwrap();

    assert_eq!((first.timeline_ms, first.order), (5_000, 1));
    assert_eq!((second.timeline_ms, second.order), (5_000, 2));
    assert_eq!(store.list().unwrap(), vec![first, second]);
    session.stop().unwrap();
}

#[test]
fn refuses_without_a_running_recording_or_for_an_import() {
    let (_temp, library, id, session, _now) = setup();
    let store = SnapshotStore::new(library.meeting_dir(&id));
    assert!(matches!(
        store.take(None::<&CaptureSession<FakeClock>>, region(), &FakeGrabber),
        Err(SnapshotError::NoRecording)
    ));
    let imported = library.meeting_dir(&id).join("meeting.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&imported).unwrap()).unwrap();
    metadata["import"] = serde_json::json!({"file":"audio.mp3","original_name":"audio.mp3","imported_at":"2026-10-12T09:30:00Z"});
    std::fs::write(imported, serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert!(matches!(
        store.take(Some(&session), region(), &FakeGrabber),
        Err(SnapshotError::ImportedMeeting)
    ));
    assert!(!library.meeting_dir(&id).join("snapshots").exists());
    session.stop().unwrap();
}

#[test]
fn a_written_snapshot_survives_a_crash_and_a_lost_library_index() {
    let (temp, library, id, session, now) = setup();
    let store = SnapshotStore::new(library.meeting_dir(&id));
    now.store(1_000_000 + 2_000 * 10_000, Ordering::SeqCst);
    let saved = store.take(Some(&session), region(), &FakeGrabber).unwrap();
    drop(session);
    drop(library);
    std::fs::remove_file(temp.path().join("index.sqlite3")).unwrap();

    let recovered = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    assert_eq!(recovered.orphan_recordings().unwrap().len(), 1);
    assert_eq!(
        SnapshotStore::new(recovered.meeting_dir(&id))
            .list()
            .unwrap(),
        vec![saved]
    );
}

#[test]
fn a_later_recording_uses_the_meeting_timeline() {
    let (_temp, mut library, id, session, now) = setup();
    now.store(1_000_000 + 3_000 * 10_000, Ordering::SeqCst);
    let store = SnapshotStore::new(library.meeting_dir(&id));
    let first = store.take(Some(&session), region(), &FakeGrabber).unwrap();
    let finished = session.stop().unwrap();
    assert_eq!(finished.duration_ms, Some(3_000));
    // Existing finalized Recording metadata puts the next Recording after it.
    let meeting_json = library.meeting_dir(&id).join("meeting.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&meeting_json).unwrap()).unwrap();
    metadata["recordings"] = serde_json::json!([{
        "number": 1, "started_at": "2026-10-12T09:30:00Z", "timeline_start_ms": 0,
        "duration_ms": 3_000, "files": []
    }]);
    std::fs::write(meeting_json, serde_json::to_vec(&metadata).unwrap()).unwrap();
    std::fs::remove_dir_all(library.meeting_dir(&id).join(".chunks/recording-1")).unwrap();
    let (chunks, manifest) = library
        .begin_next_recording(
            &id,
            chrono::DateTime::parse_from_rfc3339("2026-10-12T10:00:00Z").unwrap(),
        )
        .unwrap();
    let next_start = Arc::new(AtomicU64::new(5_000_000));
    let second_session = CaptureSession::start(
        chunks,
        manifest,
        SilentOpener,
        FakeClock(next_start.clone()),
    )
    .unwrap();
    next_start.store(5_000_000 + 2_000 * 10_000, Ordering::SeqCst);
    let second = store
        .take(Some(&second_session), region(), &FakeGrabber)
        .unwrap();
    assert_eq!((first.timeline_ms, first.order), (3_000, 1));
    assert_eq!((second.timeline_ms, second.order), (5_000, 2));
    second_session.stop().unwrap();
}
