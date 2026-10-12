use meeting_minutes_core::{
    capture::{Clock, InputDevice, InputOpener, Packet, Source},
    recorder::{Phase, Recorder, RecorderError, SaveState},
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

struct Setup {
    _temp: TempDir,
    library: Library,
    time: Arc<AtomicU64>,
    recorder: Recorder<SilentOpener, FakeClock>,
}

fn setup(notice_acknowledged: bool) -> Setup {
    let temp = TempDir::new().unwrap();
    let library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let time = Arc::new(AtomicU64::new(10_000_000));
    let recorder = Recorder::new(SilentOpener, FakeClock(time.clone()), notice_acknowledged);
    Setup {
        _temp: temp,
        library,
        time,
        recorder,
    }
}

fn started_at() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00+07:00").unwrap()
}

#[test]
fn starts_idle_and_never_records_on_its_own() {
    let s = setup(true);
    let status = s.recorder.status();
    assert_eq!(status.phase, Phase::Idle);
    assert_eq!(status.meeting_id, None);
    assert!(status.saves.is_empty());
}

#[test]
fn first_recording_ever_requires_the_notice() {
    let mut s = setup(false);
    assert!(matches!(
        s.recorder.start(&mut s.library, started_at()),
        Err(RecorderError::NoticeRequired)
    ));
    assert!(s.library.list_meetings().unwrap().is_empty());
    s.recorder.acknowledge_notice();
    s.recorder.start(&mut s.library, started_at()).unwrap();
    assert_eq!(s.recorder.status().phase, Phase::Recording);
    s.recorder.stop().unwrap();
}

#[test]
fn only_one_recording_can_be_active() {
    let mut s = setup(true);
    s.recorder.start(&mut s.library, started_at()).unwrap();
    assert!(matches!(
        s.recorder.start(&mut s.library, started_at()),
        Err(RecorderError::AlreadyActive)
    ));
    assert_eq!(s.library.list_meetings().unwrap().len(), 1);
    s.recorder.stop().unwrap();
}

#[test]
fn pause_freezes_elapsed_time_and_resume_continues_the_same_recording() {
    let mut s = setup(true);
    s.recorder.start(&mut s.library, started_at()).unwrap();
    let meeting_id = s.recorder.status().meeting_id.unwrap();
    s.time.fetch_add(20_000_000, Ordering::SeqCst);
    s.recorder.pause().unwrap();
    s.time.fetch_add(50_000_000, Ordering::SeqCst);
    let paused = s.recorder.status();
    assert_eq!(paused.phase, Phase::Paused);
    assert_eq!(paused.elapsed_ms, 2_000);
    s.recorder.resume().unwrap();
    s.time.fetch_add(10_000_000, Ordering::SeqCst);
    let resumed = s.recorder.status();
    assert_eq!(resumed.phase, Phase::Recording);
    assert_eq!(resumed.meeting_id.as_deref(), Some(meeting_id.as_str()));
    assert_eq!(resumed.elapsed_ms, 3_000);
    s.recorder.stop().unwrap();
}

#[test]
fn stop_saves_the_meeting_with_duration_and_pause_marker() {
    let mut s = setup(true);
    s.recorder.start(&mut s.library, started_at()).unwrap();
    s.time.fetch_add(10_000_000, Ordering::SeqCst);
    s.recorder.pause().unwrap();
    s.time.fetch_add(30_000_000, Ordering::SeqCst);
    s.recorder.resume().unwrap();
    s.time.fetch_add(5_000_000, Ordering::SeqCst);

    let job = s.recorder.stop().unwrap();
    let saving = s.recorder.status();
    assert_eq!(saving.phase, Phase::Idle);
    assert_eq!(saving.saves[0].state, SaveState::Saving);
    job.run().unwrap();
    s.recorder.save_finished(&job.meeting_id, Ok(()));
    assert!(s.recorder.status().saves.is_empty());

    let summary = s.library.record_finalized(&job.meeting_id).unwrap();
    assert_eq!(summary.duration_ms, 1_500);
    for file in [
        "recording-1 microphone.flac",
        "recording-1 system.flac",
        "meeting.mp3",
    ] {
        assert!(job.meeting_dir.join(file).exists(), "{file}");
    }
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(job.meeting_dir.join("meeting.json")).unwrap())
            .unwrap();
    assert_eq!(json["duration_ms"], 1_500);
    assert_eq!(json["recordings"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["markers"],
        serde_json::json!([{ "kind": "pause", "at_ms": 1_000, "paused_ms": 3_000 }])
    );
}

#[test]
fn failed_save_keeps_chunks_and_retry_save_succeeds() {
    let mut s = setup(true);
    s.recorder.start(&mut s.library, started_at()).unwrap();
    s.time.fetch_add(10_000_000, Ordering::SeqCst);
    let job = s.recorder.stop().unwrap();
    let blocker = job.meeting_dir.join("meeting.mp3");
    std::fs::write(&blocker, b"not ours").unwrap();

    let error = job.run().unwrap_err().to_string();
    s.recorder.save_finished(&job.meeting_id, Err(error));
    let failed = &s.recorder.status().saves[0];
    assert_eq!(failed.state, SaveState::Failed);
    assert!(failed.error.is_some());
    assert!(job.meeting_dir.join(".chunks/recording-1").exists());

    std::fs::remove_file(blocker).unwrap();
    let retry = s.recorder.retry_save(&job.meeting_id).unwrap();
    assert_eq!(s.recorder.status().saves[0].state, SaveState::Saving);
    retry.run().unwrap();
    s.recorder.save_finished(&retry.meeting_id, Ok(()));
    assert!(s.recorder.status().saves.is_empty());
    assert!(!job.meeting_dir.join(".chunks").join("recording-1").exists());
}

#[test]
fn retry_is_only_for_failed_saves() {
    let mut s = setup(true);
    assert!(matches!(
        s.recorder.retry_save("missing"),
        Err(RecorderError::NoFailedSave)
    ));
    s.recorder.start(&mut s.library, started_at()).unwrap();
    let job = s.recorder.stop().unwrap();
    assert!(matches!(
        s.recorder.retry_save(&job.meeting_id),
        Err(RecorderError::NoFailedSave)
    ));
}

#[test]
fn pause_resume_and_stop_need_an_active_recording() {
    let mut s = setup(true);
    assert!(matches!(s.recorder.pause(), Err(RecorderError::NotActive)));
    assert!(matches!(s.recorder.resume(), Err(RecorderError::NotActive)));
    assert!(matches!(s.recorder.stop(), Err(RecorderError::NotActive)));
}
