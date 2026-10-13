use crate::capture::{
    Clock, DeviceInfo, InputDevice, InputOpener, Source, SourceClock, CAPTURE_CHANNELS,
    HNS_PER_SECOND, SAMPLE_RATE,
};
use crate::{
    library::LibraryError,
    meeting_file::{MeetingFile, RecordingEntry, RecordingFile, SourceFile},
    metadata::update_metadata,
    timeline::place_recording,
};
use flacenc::error::Verify;
use flacenc::{
    bitsink::MemSink,
    component::{BitRepr, StreamInfo},
    config,
    source::{Fill, FrameBuf},
};
use mp3lame_encoder::{Bitrate, Builder, FlushGap, InterleavedPcm, VbrMode};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

const REOPEN_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_CHECK_INTERVAL: Duration = Duration::from_millis(500);
const MAX_PACKETS_PER_TICK: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceSwitch {
    pub at_ms: u64,
    pub source: Source,
    pub device: String,
}

struct SwitchReport {
    at_100ns: u64,
    source: Source,
    device: String,
    switched: bool,
}

struct WorkerFeedback {
    switches: mpsc::Sender<SwitchReport>,
    level: Arc<SourceLevel>,
}

struct InputState<D> {
    device: Option<D>,
    last_open_attempt: Option<Instant>,
    last_opened: Option<DeviceInfo>,
    last_default_check: Option<Instant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Marker {
    Pause {
        at_ms: u64,
        paused_ms: u64,
    },
    Gap {
        at_ms: u64,
    },
    DeviceSwitch {
        at_ms: u64,
        source: Source,
        device: String,
    },
}

impl Marker {
    pub fn at_ms(&self) -> u64 {
        match self {
            Self::Pause { at_ms, .. } | Self::Gap { at_ms } | Self::DeviceSwitch { at_ms, .. } => {
                *at_ms
            }
        }
    }
}

pub struct RecordingClock {
    start_100ns: u64,
    paused_total: u64,
    paused_at: Option<u64>,
    markers: Vec<Marker>,
}
impl RecordingClock {
    pub fn new(start_100ns: u64) -> Self {
        Self {
            start_100ns,
            paused_total: 0,
            paused_at: None,
            markers: Vec::new(),
        }
    }
    pub fn elapsed_ms(&self, now_100ns: u64) -> u64 {
        self.paused_at
            .unwrap_or(now_100ns)
            .saturating_sub(self.start_100ns)
            .saturating_sub(self.paused_total)
            / (HNS_PER_SECOND / 1000)
    }
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }
    pub fn pause(&mut self, now_100ns: u64) {
        if self.paused_at.is_none() {
            self.paused_at = Some(now_100ns);
        }
    }
    pub fn resume(&mut self, now_100ns: u64) {
        if let Some(at) = self.paused_at.take() {
            let at_ms = self.elapsed_ms(at);
            let paused_ms = now_100ns.saturating_sub(at) / (HNS_PER_SECOND / 1000);
            self.paused_total += now_100ns.saturating_sub(at);
            self.markers.push(Marker::Pause { at_ms, paused_ms });
        }
    }
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }
    /// The two Source threads report switches independently, so a report can arrive after a later one.
    pub fn add_device_switch(&mut self, at_100ns: u64, source: Source, device: String) {
        let at_ms = self.elapsed_ms(at_100ns);
        let index = self
            .markers
            .partition_point(|marker| marker.at_ms() <= at_ms);
        self.markers.insert(
            index,
            Marker::DeviceSwitch {
                at_ms,
                source,
                device,
            },
        );
    }
}

pub struct ChunkWriter {
    dir: PathBuf,
    source: Source,
    chunk_frames: u64,
    number: u32,
    frames: u64,
    file: Option<BufWriter<File>>,
}
impl ChunkWriter {
    pub fn new(dir: &Path, source: Source) -> io::Result<Self> {
        Self::with_chunk_frames(dir, source, SAMPLE_RATE as u64 * 10)
    }
    pub fn with_chunk_frames(dir: &Path, source: Source, frames: u64) -> io::Result<Self> {
        if frames == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk frames must be positive",
            ));
        }
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            source,
            chunk_frames: frames,
            number: 0,
            frames: 0,
            file: None,
        })
    }
    fn path(&self, extension: &str) -> PathBuf {
        self.dir.join(format!(
            "{}-{:06}.{extension}",
            self.source.stem(),
            self.number
        ))
    }
    fn complete(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
            let file = file.into_inner()?;
            file.sync_all()?;
            if self.path("f32").exists() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "completed Chunk exists",
                ));
            }
            fs::rename(self.path("part"), self.path("f32"))?;
            self.number += 1;
            self.frames = 0;
        }
        Ok(())
    }
    pub fn write(&mut self, stereo: &[f32]) -> io::Result<()> {
        if !stereo.len().is_multiple_of(CAPTURE_CHANNELS) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "stereo frames required",
            ));
        }
        for frame in stereo.as_chunks::<2>().0 {
            if self.file.is_none() {
                self.file = Some(BufWriter::new(
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(self.path("part"))?,
                ));
            }
            let file = self.file.as_mut().unwrap();
            if self.source == Source::Microphone {
                file.write_all(&((frame[0] + frame[1]) / 2.0).to_le_bytes())?;
            } else {
                file.write_all(&frame[0].to_le_bytes())?;
                file.write_all(&frame[1].to_le_bytes())?;
            }
            self.frames += 1;
            if self.frames == self.chunk_frames {
                self.complete()?;
            }
        }
        Ok(())
    }
    pub fn finish(mut self) -> io::Result<()> {
        self.complete()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingManifest {
    pub number: u32,
    pub started_at: String,
    pub duration_ms: Option<u64>,
    pub markers: Vec<Marker>,
}

pub(crate) fn write_manifest(dir: &Path, manifest: &RecordingManifest) -> io::Result<()> {
    let temp = dir.join("recording.json.tmp");
    let mut file = File::create(&temp)?;
    serde_json::to_writer_pretty(&mut file, manifest).map_err(io::Error::other)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(temp, dir.join("recording.json"))
}

enum Command {
    Pause(u64, mpsc::SyncSender<Result<(), String>>),
    Resume(u64, mpsc::SyncSender<Result<(), String>>),
    Stop(u64, mpsc::SyncSender<Result<(), String>>),
}

struct Worker {
    tx: mpsc::Sender<Command>,
    join: thread::JoinHandle<()>,
}

pub struct CaptureSession<C: Clock> {
    clock: C,
    timeline: RecordingClock,
    manifest: RecordingManifest,
    chunk_dir: PathBuf,
    workers: Vec<Worker>,
    switch_rx: mpsc::Receiver<SwitchReport>,
    source_devices: Vec<(Source, String)>,
    source_levels: Vec<(Source, Arc<SourceLevel>)>,
    manifest_dirty: bool,
}

struct SourceLevel {
    value: AtomicU32,
    at_100ns: AtomicU64,
    audible_at_100ns: AtomicU64,
}

impl SourceLevel {
    fn new(start: u64) -> Self {
        Self {
            value: AtomicU32::new(0),
            at_100ns: AtomicU64::new(start),
            audible_at_100ns: AtomicU64::new(start),
        }
    }
    fn report(&self, samples: &[f32], at: u64) {
        let value = crate::capture::buffer_level(samples);
        self.value.store(value.to_bits(), Ordering::Relaxed);
        self.at_100ns.store(at, Ordering::Relaxed);
        if value > 0.0 {
            self.audible_at_100ns.store(at, Ordering::Relaxed);
        }
    }
    fn current(&self, now: u64) -> (f32, bool) {
        let age = now.saturating_sub(self.at_100ns.load(Ordering::Relaxed));
        // Decay a stopped stream over one second; the capture worker never waits on the UI.
        let level = f32::from_bits(self.value.load(Ordering::Relaxed))
            * (1.0 - age as f32 / HNS_PER_SECOND as f32).clamp(0.0, 1.0);
        let silent =
            now.saturating_sub(self.audible_at_100ns.load(Ordering::Relaxed)) > 3 * HNS_PER_SECOND;
        (level, silent)
    }
}

impl<C: Clock> CaptureSession<C> {
    pub fn start<O: InputOpener>(
        chunk_dir: PathBuf,
        manifest: RecordingManifest,
        opener: O,
        clock: C,
    ) -> io::Result<Self> {
        fs::create_dir_all(&chunk_dir)?;
        write_manifest(&chunk_dir, &manifest)?;
        let start = clock.now_100ns();
        let mut workers = Vec::new();
        let mut source_levels = Vec::new();
        let (switch_tx, switch_rx) = mpsc::channel();
        for source in [Source::Microphone, Source::SystemAudio] {
            let level = Arc::new(SourceLevel::new(start));
            source_levels.push((source, level.clone()));
            let writer = ChunkWriter::new(&chunk_dir, source)?;
            let (tx, rx) = mpsc::channel();
            let opener = opener.clone();
            let worker_clock = clock.clone();
            let worker_switch_tx = switch_tx.clone();
            let join = thread::Builder::new()
                .name(format!("capture-{}", source.stem()))
                .spawn(move || {
                    run_worker(
                        source,
                        writer,
                        opener,
                        worker_clock,
                        start,
                        rx,
                        WorkerFeedback {
                            switches: worker_switch_tx,
                            level,
                        },
                    );
                })?;
            workers.push(Worker { tx, join });
        }
        Ok(Self {
            clock,
            timeline: RecordingClock::new(start),
            manifest,
            chunk_dir,
            workers,
            switch_rx,
            source_devices: Vec::new(),
            source_levels,
            manifest_dirty: false,
        })
    }
    pub fn elapsed_ms(&self) -> u64 {
        self.timeline.elapsed_ms(self.clock.now_100ns())
    }
    pub fn is_paused(&self) -> bool {
        self.timeline.is_paused()
    }
    pub fn collect_device_switches(&mut self) -> io::Result<()> {
        let mut changed = false;
        while let Ok(report) = self.switch_rx.try_recv() {
            if let Some((_, name)) = self
                .source_devices
                .iter_mut()
                .find(|(source, _)| *source == report.source)
            {
                *name = report.device.clone();
            } else {
                self.source_devices
                    .push((report.source, report.device.clone()));
            }
            if report.switched {
                self.timeline
                    .add_device_switch(report.at_100ns, report.source, report.device);
                changed = true;
            }
        }
        if changed {
            self.manifest.markers = self.timeline.markers().to_vec();
            self.manifest_dirty = true;
        }
        if self.manifest_dirty {
            write_manifest(&self.chunk_dir, &self.manifest)?;
            self.manifest_dirty = false;
        }
        Ok(())
    }
    pub fn device_switches(&self) -> Vec<DeviceSwitch> {
        self.timeline
            .markers()
            .iter()
            .filter_map(|marker| match marker {
                Marker::DeviceSwitch {
                    at_ms,
                    source,
                    device,
                } => Some(DeviceSwitch {
                    at_ms: *at_ms,
                    source: *source,
                    device: device.clone(),
                }),
                _ => None,
            })
            .collect()
    }
    pub fn source_devices(&self) -> Vec<(Source, String)> {
        self.source_devices.clone()
    }
    pub fn source_levels(&self) -> Vec<(Source, f32, bool)> {
        let now = self.clock.now_100ns();
        self.source_levels
            .iter()
            .map(|(source, level)| {
                let (value, silent) = level.current(now);
                (
                    *source,
                    if self.is_paused() { 0.0 } else { value },
                    silent && !self.is_paused(),
                )
            })
            .collect()
    }
    fn control(
        &self,
        at: u64,
        make: impl Fn(u64, mpsc::SyncSender<Result<(), String>>) -> Command,
    ) -> io::Result<()> {
        let mut replies = Vec::new();
        for worker in &self.workers {
            let (reply, recv) = mpsc::sync_channel(1);
            worker.tx.send(make(at, reply)).map_err(io::Error::other)?;
            replies.push(recv);
        }
        for recv in replies {
            recv.recv()
                .map_err(io::Error::other)?
                .map_err(io::Error::other)?;
        }
        Ok(())
    }
    pub fn pause(&mut self) -> io::Result<()> {
        if self.timeline.is_paused() {
            return Ok(());
        }
        let at = self.clock.now_100ns();
        self.control(at, Command::Pause)?;
        let _ = self.collect_device_switches();
        self.timeline.pause(at);
        self.manifest.markers = self.timeline.markers().to_vec();
        write_manifest(&self.chunk_dir, &self.manifest)?;
        self.manifest_dirty = false;
        Ok(())
    }
    pub fn resume(&mut self) -> io::Result<()> {
        if !self.timeline.is_paused() {
            return Ok(());
        }
        let at = self.clock.now_100ns();
        self.control(at, Command::Resume)?;
        // A worker can report a switch right after it resumes, so its time must count from the resume.
        self.timeline.resume(at);
        let _ = self.collect_device_switches();
        self.manifest.markers = self.timeline.markers().to_vec();
        write_manifest(&self.chunk_dir, &self.manifest)?;
        self.manifest_dirty = false;
        Ok(())
    }
    pub fn stop(mut self) -> io::Result<RecordingManifest> {
        let at = self.clock.now_100ns();
        self.control(at, Command::Stop)?;
        let _ = self.collect_device_switches();
        for worker in self.workers.drain(..) {
            worker
                .join
                .join()
                .map_err(|_| io::Error::other("capture thread panicked"))?;
        }
        if self.timeline.is_paused() {
            self.timeline.resume(at);
        }
        self.manifest.duration_ms = Some(self.timeline.elapsed_ms(at));
        self.manifest.markers = self.timeline.markers().to_vec();
        write_manifest(&self.chunk_dir, &self.manifest)?;
        Ok(self.manifest)
    }
}

fn run_worker<O: InputOpener, C: Clock>(
    source: Source,
    mut writer: ChunkWriter,
    opener: O,
    clock: C,
    start: u64,
    rx: mpsc::Receiver<Command>,
    feedback: WorkerFeedback,
) {
    let mut timeline = SourceClock::new(start);
    let mut input = InputState {
        device: None,
        last_open_attempt: None,
        last_opened: None,
        last_default_check: None,
    };
    // After a write error the worker stops capturing but keeps answering, so the error reaches the caller.
    let mut failure: Option<String> = None;
    let fail = |failure: &Option<String>| Err(failure.clone().unwrap_or_default());
    loop {
        match rx.try_recv() {
            Ok(Command::Pause(at, reply)) => {
                let result = match &failure {
                    Some(_) => fail(&failure),
                    None => writer.write(&timeline.pause(at)).map_err(|e| e.to_string()),
                };
                let _ = reply.send(result);
                continue;
            }
            Ok(Command::Resume(at, reply)) => {
                timeline.resume(at);
                let _ = reply.send(if failure.is_some() {
                    fail(&failure)
                } else {
                    Ok(())
                });
                continue;
            }
            Ok(Command::Stop(at, reply)) => {
                let result = match &failure {
                    Some(_) => {
                        let _ = writer.finish();
                        fail(&failure)
                    }
                    None => writer
                        .write(&timeline.pad_to(at, true))
                        .and_then(|()| writer.finish())
                        .map_err(|e| e.to_string()),
                };
                let _ = reply.send(result);
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => return,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if failure.is_none() && !timeline.is_paused() {
            if let Err(error) = capture_pending(
                source,
                &mut writer,
                &mut timeline,
                &mut input,
                &opener,
                &clock,
                &feedback,
            ) {
                failure = Some(format!("{} capture stopped: {error}", source.stem()));
                input.device = None;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn capture_pending<O: InputOpener, C: Clock>(
    source: Source,
    writer: &mut ChunkWriter,
    timeline: &mut SourceClock,
    input: &mut InputState<O::Device>,
    opener: &O,
    clock: &C,
    feedback: &WorkerFeedback,
) -> io::Result<()> {
    if input.device.is_some()
        && input
            .last_default_check
            .is_none_or(|at| at.elapsed() >= DEFAULT_CHECK_INTERVAL)
    {
        input.last_default_check = Some(Instant::now());
        if opener.default_device_id(source).ok().as_deref()
            != input.device.as_ref().map(|d| d.info().id.as_str())
        {
            input.device = None;
            input.last_open_attempt = None;
        }
    }
    if input.device.is_none()
        && input
            .last_open_attempt
            .is_none_or(|at| at.elapsed() >= REOPEN_INTERVAL)
    {
        input.last_open_attempt = Some(Instant::now());
        if let Ok(opened) = opener.open(source) {
            let info = opened.info().clone();
            let switched = input
                .last_opened
                .as_ref()
                .is_some_and(|previous| previous.id != info.id);
            let _ = feedback.switches.send(SwitchReport {
                at_100ns: clock.now_100ns(),
                source,
                device: info.name.clone(),
                switched,
            });
            input.last_opened = Some(info);
            input.last_default_check = Some(Instant::now());
            input.device = Some(opened);
        }
    }
    // WASAPI delivers about one packet per 10 ms; drain what is pending, bounded so control
    // messages are still checked.
    for _ in 0..MAX_PACKETS_PER_TICK {
        let Some(device) = input.device.as_mut() else {
            break;
        };
        match device.read_packet() {
            Ok(Some(packet)) => {
                let now = clock.now_100ns();
                feedback.level.report(&packet.samples, now);
                writer.write(&timeline.packet(&packet, now))?
            }
            Ok(None) => break,
            Err(_) => input.device = None,
        }
    }
    writer.write(&timeline.pad_to(clock.now_100ns(), false))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalizedRecording {
    pub number: u32,
    pub duration_ms: u64,
    pub timeline_start_ms: u64,
}

pub fn finalize_recording(
    meeting_dir: &Path,
    number: u32,
) -> Result<FinalizedRecording, LibraryError> {
    let chunk_dir = meeting_dir.join(format!(".chunks/recording-{number}"));
    let initial_metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(meeting_dir.join("meeting.json"))?)?;
    let initial_recordings = initial_metadata["recordings"].clone();
    let meeting: MeetingFile = serde_json::from_value(initial_metadata)?;
    if meeting.schema_version != 1 {
        return Err(LibraryError::Encode("Unsupported Meeting schema".into()));
    }
    if let Some(recording) = meeting.recordings.iter().find_map(|entry| match entry {
        RecordingEntry::Known(r) if r.number == number => Some(r),
        _ => None,
    }) {
        let _ = fs::remove_dir_all(chunk_dir);
        return Ok(FinalizedRecording {
            number,
            duration_ms: recording.duration_ms,
            timeline_start_ms: recording.timeline_start_ms,
        });
    }
    for previous in 1..number {
        if !meeting
            .recordings
            .iter()
            .any(|entry| matches!(entry, RecordingEntry::Known(r) if r.number == previous))
        {
            return Err(LibraryError::Encode(format!(
                "Recording {previous} is not saved yet"
            )));
        }
    }
    let manifest: RecordingManifest =
        serde_json::from_slice(&fs::read(chunk_dir.join("recording.json"))?)?;
    if manifest.number != number {
        return Err(LibraryError::Encode("Recording number mismatch".into()));
    }
    salvage_chunks(&chunk_dir)?;
    let mic = ChunkSet::new(&chunk_dir, Source::Microphone)?;
    let system = ChunkSet::new(&chunk_dir, Source::SystemAudio)?;
    let frames = mic.frames.max(system.frames);
    let duration_ms = frames.saturating_mul(1000) / SAMPLE_RATE as u64;
    let earlier: Vec<u64> = meeting
        .recordings
        .iter()
        .filter_map(|entry| match entry {
            RecordingEntry::Known(r) => Some(r.duration_ms),
            _ => None,
        })
        .collect();
    let placement = place_recording(&earlier, duration_ms, &manifest.markers);
    let timeline_start_ms = placement.timeline_start_ms;
    let mic_name = format!("recording-{number} microphone.flac");
    let system_name = format!("recording-{number} system.flac");
    let journal_path = chunk_dir.join("finalizing");
    if journal_path.exists() {
        for name in fs::read_to_string(&journal_path)?.lines() {
            if Path::new(name).file_name().is_some_and(|file| file == name) {
                match fs::remove_file(meeting_dir.join(name)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    let mut journal = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&journal_path)?;
    journal.sync_all()?;
    let mut created = Vec::new();
    let result = (|| -> Result<(), LibraryError> {
        let mic_file = create_final(meeting_dir, &mic_name, &mut journal, &mut created)?;
        encode_flac(mic_file, &mic, frames)?;
        let system_file = create_final(meeting_dir, &system_name, &mut journal, &mut created)?;
        encode_flac(system_file, &system, frames)?;
        let audio_name = meeting
            .audio
            .clone()
            .unwrap_or_else(|| "meeting.mp3".into());
        let mp3_name = if meeting.audio.is_some() {
            format!("{audio_name}.tmp")
        } else {
            audio_name.clone()
        };
        let mp3_file = create_final(meeting_dir, &mp3_name, &mut journal, &mut created)?;
        encode_mp3(
            mp3_file,
            meeting_dir,
            &meeting.recordings,
            &mic,
            &system,
            frames,
        )?;
        if meeting.audio.is_some() {
            fs::rename(meeting_dir.join(&mp3_name), meeting_dir.join(&audio_name))?;
        }
        let recording = RecordingFile {
            number,
            started_at: manifest.started_at,
            timeline_start_ms,
            duration_ms,
            files: vec![
                SourceFile {
                    source: Source::Microphone,
                    file: mic_name,
                    channels: 1,
                },
                SourceFile {
                    source: Source::SystemAudio,
                    file: system_name,
                    channels: 2,
                },
            ],
        };
        update_metadata(meeting_dir, |metadata| {
            if metadata["recordings"] != initial_recordings {
                return Err(LibraryError::Encode(
                    "Meeting changed during save; retry".into(),
                ));
            }
            let mut recordings = metadata["recordings"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            recordings.push(serde_json::to_value(&recording)?);
            metadata["recordings"] = serde_json::Value::Array(recordings);
            let mut markers = metadata["markers"].as_array().cloned().unwrap_or_default();
            let added: Vec<serde_json::Value> = placement
                .markers
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<_, _>>()?;
            markers.extend(added);
            metadata["markers"] = serde_json::Value::Array(markers);
            metadata["duration_ms"] = serde_json::to_value(timeline_start_ms + duration_ms)?;
            metadata["audio"] = serde_json::to_value(&audio_name)?;
            Ok(())
        })?;
        Ok(())
    })();
    if let Err(error) = result {
        for path in created {
            let _ = fs::remove_file(path);
        }
        let _ = journal.set_len(0);
        let _ = journal.sync_all();
        return Err(error);
    }
    drop(journal);
    let _ = fs::remove_dir_all(chunk_dir);
    Ok(FinalizedRecording {
        number,
        duration_ms,
        timeline_start_ms,
    })
}

fn create_final(
    dir: &Path,
    name: &str,
    journal: &mut File,
    created: &mut Vec<PathBuf>,
) -> io::Result<File> {
    let path = dir.join(name);
    journal.write_all(name.as_bytes())?;
    journal.write_all(b"\n")?;
    journal.sync_all()?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    created.push(path);
    Ok(file)
}

fn salvage_chunks(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        for source in [Source::Microphone, Source::SystemAudio] {
            let Some(rest) = name.strip_prefix(&format!("{}-", source.stem())) else {
                continue;
            };
            let part = rest.strip_suffix(".part");
            let complete = rest.strip_suffix(".f32");
            if !part
                .or(complete)
                .is_some_and(|digits| digits.len() == 6 && digits.parse::<u32>().is_ok())
            {
                continue;
            }
            let frame_bytes = source.channels() as u64 * 4;
            let file = OpenOptions::new().write(true).open(entry.path())?;
            let size = file.metadata()?.len();
            file.set_len(size - size % frame_bytes)?;
            drop(file);
            if part.is_some() {
                let target = entry.path().with_extension("f32");
                if target.exists() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "completed Chunk exists",
                    ));
                }
                fs::rename(entry.path(), target)?;
            }
        }
    }
    Ok(())
}

struct ChunkSet {
    dir: PathBuf,
    source: Source,
    files: u32,
    frames: u64,
    channels: usize,
}
impl ChunkSet {
    fn new(dir: &Path, source: Source) -> io::Result<Self> {
        let mut files = 0u32;
        let mut highest = None;
        let mut frames = 0;
        let channels = source.channels();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let number = name
                .strip_prefix(&format!("{}-", source.stem()))
                .and_then(|rest| rest.strip_suffix(".f32"))
                .filter(|digits| digits.len() == 6)
                .and_then(|digits| digits.parse::<u32>().ok());
            if let Some(number) = number {
                let size = entry.metadata()?.len();
                if size % (channels as u64 * 4) != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "partial Chunk frame",
                    ));
                }
                frames += size / (channels as u64 * 4);
                files = files
                    .checked_add(1)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "too many Chunks"))?;
                highest = Some(highest.map_or(number, |current: u32| current.max(number)));
            }
        }
        if highest.is_some_and(|number| number != files - 1) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "missing completed Chunk",
            ));
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            source,
            files,
            frames,
            channels,
        })
    }
    fn reader(&self) -> ChunkReader<'_> {
        ChunkReader {
            set: self,
            index: 0,
            file: None,
        }
    }
}

struct ChunkReader<'a> {
    set: &'a ChunkSet,
    index: u32,
    file: Option<BufReader<File>>,
}
impl ChunkReader<'_> {
    fn sample(&mut self) -> io::Result<f32> {
        loop {
            if self.file.is_none() {
                if self.index == self.set.files {
                    return Ok(0.0);
                }
                let path =
                    self.set
                        .dir
                        .join(format!("{}-{:06}.f32", self.set.source.stem(), self.index));
                self.file = Some(BufReader::new(File::open(path)?));
                self.index += 1;
            }
            let mut bytes = [0; 4];
            let n = self.file.as_mut().unwrap().read(&mut bytes)?;
            if n == 0 {
                self.file = None;
                continue;
            }
            if n < 4 {
                self.file.as_mut().unwrap().read_exact(&mut bytes[n..])?;
            }
            return Ok(f32::from_le_bytes(bytes));
        }
    }
}

fn pcm(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn encode_flac(mut file: File, set: &ChunkSet, frames: u64) -> Result<(), LibraryError> {
    const BLOCK: usize = 4096;
    let config = config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| LibraryError::Encode(e.to_string()))?;
    let mut info = StreamInfo::new(SAMPLE_RATE as usize, set.channels, 16)
        .map_err(|e| LibraryError::Encode(e.to_string()))?;
    info.set_total_samples(frames as usize);
    info.set_block_sizes(16, BLOCK)
        .map_err(|e| LibraryError::Encode(e.to_string()))?;
    info.set_frame_sizes(0, 0)
        .map_err(|e| LibraryError::Encode(e.to_string()))?;
    let mut sink = MemSink::<u8>::new();
    info.write(&mut sink)
        .map_err(|e| LibraryError::Encode(e.to_string()))?;
    file.write_all(b"fLaC")?;
    file.write_all(&[0x80, 0, 0, 34])?;
    file.write_all(sink.as_slice())?;
    let mut reader = set.reader();
    let mut framebuf = FrameBuf::with_size(set.channels, BLOCK)
        .map_err(|e| LibraryError::Encode(e.to_string()))?;
    let mut offset = 0;
    let mut frame_number = 0;
    while offset < frames {
        let count = (frames - offset).min(BLOCK as u64) as usize;
        let mut samples = Vec::with_capacity(count * set.channels);
        for _ in 0..count * set.channels {
            samples.push(pcm(reader.sample()?) as i32);
        }
        framebuf
            .fill_interleaved(&samples)
            .map_err(|e| LibraryError::Encode(e.to_string()))?;
        let frame = flacenc::encode_fixed_size_frame(&config, &framebuf, frame_number, &info)
            .map_err(|e| LibraryError::Encode(e.to_string()))?;
        sink.clear();
        frame
            .write(&mut sink)
            .map_err(|e| LibraryError::Encode(e.to_string()))?;
        file.write_all(sink.as_slice())?;
        offset += count as u64;
        frame_number += 1;
    }
    file.sync_all()?;
    Ok(())
}

fn encode_mp3(
    mut file: File,
    meeting_dir: &Path,
    earlier: &[RecordingEntry],
    mic: &ChunkSet,
    system: &ChunkSet,
    frames: u64,
) -> Result<(), LibraryError> {
    let mut encoder = Builder::new()
        .ok_or_else(|| LibraryError::Encode("Could not create MP3 encoder".into()))?
        .with_num_channels(2)
        .map_err(|e| LibraryError::Encode(e.to_string()))?
        .with_sample_rate(SAMPLE_RATE)
        .map_err(|e| LibraryError::Encode(e.to_string()))?
        .with_brate(Bitrate::Kbps128)
        .map_err(|e| LibraryError::Encode(e.to_string()))?
        .with_vbr_mode(VbrMode::Off)
        .map_err(|e| LibraryError::Encode(e.to_string()))?
        // Reserves the first frame for the Info/LAME tag that tells players to trim the encoder
        // delay; without it the Timeline audio starts about 47 ms late.
        .with_to_write_vbr_tag(true)
        .map_err(|e| LibraryError::Encode(e.to_string()))?
        .build()
        .map_err(|e| LibraryError::Encode(e.to_string()))?;
    let mut mic_reader = mic.reader();
    let mut system_reader = system.reader();
    let mut output = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(4096).max(7200));
    for entry in earlier {
        let RecordingEntry::Known(recording) = entry else {
            continue;
        };
        let source_path = |source| {
            recording
                .files
                .iter()
                .find(|file| file.source == source)
                .map(|file| meeting_dir.join(&file.file))
                .ok_or_else(|| {
                    LibraryError::Encode(format!(
                        "Recording {} is missing a Source",
                        recording.number
                    ))
                })
        };
        let mut earlier_mic = claxon::FlacReader::open(source_path(Source::Microphone)?)
            .map_err(|e| LibraryError::Encode(e.to_string()))?;
        let mut earlier_system = claxon::FlacReader::open(source_path(Source::SystemAudio)?)
            .map_err(|e| LibraryError::Encode(e.to_string()))?;
        let mut mic_samples = earlier_mic.samples();
        let mut system_samples = earlier_system.samples();
        encode_mp3_frames(
            &mut encoder,
            &mut file,
            &mut output,
            recording
                .duration_ms
                .saturating_mul(SAMPLE_RATE as u64 / 1000),
            || {
                let sample =
                    |value: Option<Result<i32, claxon::Error>>| -> Result<f32, LibraryError> {
                        value
                            .transpose()
                            .map_err(|e| LibraryError::Encode(e.to_string()))
                            .map(|sample| sample.unwrap_or(0) as f32 / 32767.0)
                    };
                let m = sample(mic_samples.next())?;
                Ok((
                    m + sample(system_samples.next())?,
                    m + sample(system_samples.next())?,
                ))
            },
        )?;
    }
    encode_mp3_frames(&mut encoder, &mut file, &mut output, frames, || {
        let m = mic_reader.sample()?;
        Ok((m + system_reader.sample()?, m + system_reader.sample()?))
    })?;
    output.clear();
    // FlushGap pads and writes the final partial frame; the LAME tag records that padding.
    encoder
        .flush_to_vec::<FlushGap>(&mut output)
        .map_err(|e| LibraryError::Encode(e.to_string()))?;
    file.write_all(&output)?;
    output.clear();
    output.reserve(encoder.lame_tag_size());
    encoder
        .lame_tag_encode_to_vec(&mut output)
        .ok_or_else(|| LibraryError::Encode("Could not write the MP3 LAME tag".into()))?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&output)?;
    file.sync_all()?;
    Ok(())
}

fn encode_mp3_frames(
    encoder: &mut mp3lame_encoder::Encoder,
    file: &mut File,
    output: &mut Vec<u8>,
    frames: u64,
    mut next: impl FnMut() -> Result<(f32, f32), LibraryError>,
) -> Result<(), LibraryError> {
    const BLOCK: usize = 4096;
    let mut offset = 0;
    while offset < frames {
        let count = (frames - offset).min(BLOCK as u64) as usize;
        let mut samples = Vec::with_capacity(count * 2);
        for _ in 0..count {
            let (left, right) = next()?;
            samples.push(pcm(left));
            samples.push(pcm(right));
        }
        output.clear();
        encoder
            .encode_to_vec(InterleavedPcm(&samples), output)
            .map_err(|e| LibraryError::Encode(e.to_string()))?;
        file.write_all(output)?;
        offset += count as u64;
    }
    Ok(())
}
