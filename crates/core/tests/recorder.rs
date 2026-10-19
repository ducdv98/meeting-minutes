use meeting_minutes_core::{
    capture::{Clock, DeviceInfo, InputDevice, InputOpener, Packet, Source},
    recorder::{Phase, Recorder, RecorderError, RecorderStatus, SaveState, SourceHealthWarning},
    Library,
};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
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

#[derive(Clone)]
struct ChangeOpener {
    default: Arc<Mutex<String>>,
    microphone_opens: Arc<AtomicU64>,
}
struct NamedDevice(DeviceInfo);
impl InputDevice for NamedDevice {
    fn read_packet(&mut self) -> Result<Option<Packet>, String> {
        Ok(None)
    }
    fn info(&self) -> &DeviceInfo {
        &self.0
    }
}
impl InputOpener for ChangeOpener {
    type Device = NamedDevice;
    fn open(&self, source: Source) -> Result<Self::Device, String> {
        let id = self.default_device_id(source)?;
        if source == Source::Microphone {
            self.microphone_opens.fetch_add(1, Ordering::SeqCst);
        }
        Ok(NamedDevice(DeviceInfo {
            name: id.clone(),
            id,
        }))
    }
    fn default_device_id(&self, source: Source) -> Result<String, String> {
        if source == Source::Microphone {
            Ok(self.default.lock().unwrap().clone())
        } else {
            Ok("speaker".into())
        }
    }
}

#[test]
fn status_reports_switch_and_saved_meeting_keeps_marker() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let default = Arc::new(Mutex::new("laptop".to_owned()));
    let microphone_opens = Arc::new(AtomicU64::new(0));
    let time = Arc::new(AtomicU64::new(1_000_000));
    let mut recorder = Recorder::new(
        ChangeOpener {
            default: default.clone(),
            microphone_opens: microphone_opens.clone(),
        },
        FakeClock(time.clone()),
        true,
    );
    recorder.start(&mut library, started_at()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while microphone_opens.load(Ordering::SeqCst) == 0 {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    time.store(6_000_000, Ordering::SeqCst);
    *default.lock().unwrap() = "headset".into();
    while recorder.status().device_switches.is_empty() {
        recorder.collect_device_switches().unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(recorder.status().device_switches[0].at_ms, 500);
    let job = recorder.stop().unwrap();
    assert!(recorder.status().device_switches.is_empty());
    job.run().unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(job.meeting_dir.join("meeting.json")).unwrap())
            .unwrap();
    assert_eq!(
        json["markers"],
        serde_json::json!([{
            "kind": "device_switch", "at_ms": 500, "source": "microphone", "device": "headset"
        }])
    );
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

#[derive(Clone)]
struct PacketOpener {
    packets: Arc<Mutex<VecDeque<Packet>>>,
    processed: Arc<AtomicU64>,
    device: Arc<Mutex<String>>,
}

struct PacketDevice {
    info: DeviceInfo,
    packets: Arc<Mutex<VecDeque<Packet>>>,
    processed: Arc<AtomicU64>,
    pending_report: bool,
}

impl InputOpener for PacketOpener {
    type Device = PacketDevice;

    fn open(&self, source: Source) -> Result<Self::Device, String> {
        let id = self.default_device_id(source)?;
        Ok(PacketDevice {
            info: DeviceInfo {
                name: id.clone(),
                id,
            },
            packets: self.packets.clone(),
            processed: self.processed.clone(),
            pending_report: false,
        })
    }

    fn default_device_id(&self, source: Source) -> Result<String, String> {
        Ok(if source == Source::Microphone {
            self.device.lock().unwrap().clone()
        } else {
            "speaker".into()
        })
    }
}

impl InputDevice for PacketDevice {
    fn read_packet(&mut self) -> Result<Option<Packet>, String> {
        if self.pending_report {
            self.processed.fetch_add(1, Ordering::SeqCst);
            self.pending_report = false;
        }
        let packet = if self.info.id == "speaker" {
            None
        } else {
            self.packets.lock().unwrap().pop_front()
        };
        if packet.is_some() {
            self.pending_report = true;
        }
        Ok(packet)
    }

    fn info(&self) -> &DeviceInfo {
        &self.info
    }
}

fn packet_recording() -> (
    TempDir,
    Library,
    Arc<AtomicU64>,
    PacketOpener,
    Recorder<PacketOpener, FakeClock>,
) {
    let temp = TempDir::new().unwrap();
    let library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let time = Arc::new(AtomicU64::new(10_000_000));
    let opener = PacketOpener {
        packets: Arc::new(Mutex::new(VecDeque::new())),
        processed: Arc::new(AtomicU64::new(0)),
        device: Arc::new(Mutex::new("microphone".into())),
    };
    let recorder = Recorder::new(opener.clone(), FakeClock(time.clone()), true);
    (temp, library, time, opener, recorder)
}

fn deliver(opener: &PacketOpener, samples: Vec<f32>, silent: bool) {
    let before = opener.processed.load(Ordering::SeqCst);
    opener.packets.lock().unwrap().push_back(Packet {
        samples,
        qpc_100ns: None,
        silent,
        discontinuity: false,
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while opener.processed.load(Ordering::SeqCst) == before {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn microphone_warning(status: &RecorderStatus) -> Option<SourceHealthWarning> {
    status
        .source_health
        .iter()
        .find(|health| health.source == Source::Microphone)
        .and_then(|health| health.warning)
}

#[test]
fn microphone_no_signal_needs_sixty_seconds_of_digital_silence() {
    let (_temp, mut library, time, opener, mut recorder) = packet_recording();
    recorder.start(&mut library, started_at()).unwrap();
    deliver(&opener, vec![0.0, 0.0], true);
    time.fetch_add(59 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], false);
    assert!(recorder
        .status()
        .source_health
        .iter()
        .all(|health| health.warning.is_none()));
    time.fetch_add(10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], true);
    assert_eq!(
        microphone_warning(&recorder.status()),
        Some(SourceHealthWarning::NoSignal)
    );
    assert_eq!(
        serde_json::to_value(recorder.status()).unwrap()["source_health"][0]["warning"],
        "no_signal"
    );
    assert_eq!(recorder.status().phase, Phase::Recording);
    recorder.stop().unwrap();
}

#[test]
fn low_level_microphone_noise_is_not_no_signal() {
    let (_temp, mut library, time, opener, mut recorder) = packet_recording();
    recorder.start(&mut library, started_at()).unwrap();
    deliver(&opener, vec![0.0001, 0.0001], false);
    time.fetch_add(60 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0001, 0.0001], false);
    assert!(recorder
        .status()
        .source_health
        .iter()
        .all(|health| health.warning.is_none()));
    recorder.stop().unwrap();
}

#[test]
fn paused_time_does_not_count_toward_microphone_no_signal() {
    let (_temp, mut library, time, opener, mut recorder) = packet_recording();
    recorder.start(&mut library, started_at()).unwrap();
    deliver(&opener, vec![0.0, 0.0], true);
    time.fetch_add(30 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], true);
    recorder.pause().unwrap();
    time.fetch_add(120 * 10_000_000, Ordering::SeqCst);
    assert!(microphone_warning(&recorder.status()).is_none());
    recorder.resume().unwrap();
    time.fetch_add(29 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], true);
    assert!(microphone_warning(&recorder.status()).is_none());
    time.fetch_add(10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], true);
    assert!(microphone_warning(&recorder.status()).is_some());
    recorder.stop().unwrap();
}

#[test]
fn microphone_device_switch_starts_a_new_silence_window() {
    let (_temp, mut library, time, opener, mut recorder) = packet_recording();
    recorder.start(&mut library, started_at()).unwrap();
    deliver(&opener, vec![0.0, 0.0], true);
    time.fetch_add(40 * 10_000_000, Ordering::SeqCst);
    *opener.device.lock().unwrap() = "headset".into();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while recorder.status().device_switches.is_empty() {
        recorder.collect_device_switches().unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    deliver(&opener, vec![0.0, 0.0], true);
    time.fetch_add(20 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], true);
    assert!(microphone_warning(&recorder.status()).is_none());
    recorder.stop().unwrap();
}

#[test]
fn microphone_warning_clears_on_signal_and_dismissal_lasts_until_recovery() {
    let (_temp, mut library, time, opener, mut recorder) = packet_recording();
    recorder.start(&mut library, started_at()).unwrap();
    deliver(&opener, vec![0.0, 0.0], true);
    time.fetch_add(60 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.1, 0.1], true);
    assert!(microphone_warning(&recorder.status()).is_some());
    time.fetch_add(2 * 10_000_000, Ordering::SeqCst);
    assert!(microphone_warning(&recorder.status()).is_some());
    recorder.dismiss_source_warning(Source::Microphone).unwrap();
    assert!(microphone_warning(&recorder.status()).is_none());
    time.fetch_add(60 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], true);
    assert!(microphone_warning(&recorder.status()).is_none());
    deliver(&opener, vec![0.0001, 0.0001], false);
    assert!(microphone_warning(&recorder.status()).is_none());
    deliver(&opener, vec![0.0, 0.0], true);
    time.fetch_add(60 * 10_000_000, Ordering::SeqCst);
    deliver(&opener, vec![0.0, 0.0], true);
    assert!(microphone_warning(&recorder.status()).is_some());
    assert_eq!(recorder.status().phase, Phase::Recording);
    recorder.stop().unwrap();
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

#[test]
fn crash_recovery_offers_to_continue_the_same_meeting() {
    let mut s = setup(true);
    s.recorder.start(&mut s.library, started_at()).unwrap();
    let meeting_id = s.recorder.status().meeting_id.unwrap();
    s.time.fetch_add(10_000_000, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(30));
    drop(s.recorder);
    std::thread::sleep(std::time::Duration::from_millis(30));
    let mut recorder = Recorder::new(SilentOpener, FakeClock(s.time.clone()), true);
    let jobs = recorder.recover(&s.library).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].meeting_id, meeting_id);
    assert!(recorder.status().saves[0].recovered);
    assert_eq!(recorder.status().saves[0].state, SaveState::Saving);
    assert!(matches!(
        recorder.continue_meeting(&mut s.library, &meeting_id, started_at()),
        Err(RecorderError::SavePending)
    ));
    jobs[0].run().unwrap();
    recorder.save_finished(&meeting_id, Ok(()));
    assert_eq!(recorder.status().continue_offers, vec![meeting_id.clone()]);
    recorder
        .continue_meeting(&mut s.library, &meeting_id, started_at())
        .unwrap();
    assert!(recorder.status().continue_offers.is_empty());
    s.time.fetch_add(10_000_000, Ordering::SeqCst);
    let second = recorder.stop().unwrap();
    assert_eq!(second.number, 2);
    second.run().unwrap();
    recorder.save_finished(&meeting_id, Ok(()));
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(second.meeting_dir.join("meeting.json")).unwrap())
            .unwrap();
    assert_eq!(json["recordings"].as_array().unwrap().len(), 2);
}

#[test]
fn renaming_a_meeting_keeps_its_continue_offer() {
    let mut s = setup(true);
    s.recorder.start(&mut s.library, started_at()).unwrap();
    let old_id = s.recorder.status().meeting_id.unwrap();
    let job = s.recorder.stop().unwrap();
    drop(s.recorder);
    let mut recorder = Recorder::new(SilentOpener, FakeClock(s.time.clone()), true);
    let jobs = recorder.recover(&s.library).unwrap();
    assert_eq!(jobs.len(), 1);
    job.run().unwrap();
    recorder.save_finished(&old_id, Ok(()));
    assert_eq!(recorder.status().continue_offers, vec![old_id.clone()]);

    let renamed = s.library.rename_meeting(&old_id, "Recovered").unwrap();
    assert_eq!(recorder.status().continue_offers, vec![renamed.id.clone()]);
    recorder
        .continue_meeting(&mut s.library, &renamed.id, started_at())
        .unwrap();
    assert!(recorder.status().continue_offers.is_empty());
    recorder.stop().unwrap();
}
