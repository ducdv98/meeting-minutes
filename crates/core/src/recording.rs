use crate::capture::{
    Clock, InputDevice, InputOpener, Source, SourceClock, CAPTURE_CHANNELS, HNS_PER_SECOND,
    SAMPLE_RATE,
};
use crate::{
    library::LibraryError,
    meeting_file::{MeetingFile, RecordingEntry, RecordingFile, SourceFile},
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
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const REOPEN_INTERVAL: Duration = Duration::from_secs(1);
const MAX_PACKETS_PER_TICK: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Marker {
    Pause { at_ms: u64, paused_ms: u64 },
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
        for source in [Source::Microphone, Source::SystemAudio] {
            let writer = ChunkWriter::new(&chunk_dir, source)?;
            let (tx, rx) = mpsc::channel();
            let opener = opener.clone();
            let worker_clock = clock.clone();
            let join = thread::Builder::new()
                .name(format!("capture-{}", source.stem()))
                .spawn(move || {
                    run_worker(source, writer, opener, worker_clock, start, rx);
                })?;
            workers.push(Worker { tx, join });
        }
        Ok(Self {
            clock,
            timeline: RecordingClock::new(start),
            manifest,
            chunk_dir,
            workers,
        })
    }
    pub fn elapsed_ms(&self) -> u64 {
        self.timeline.elapsed_ms(self.clock.now_100ns())
    }
    pub fn is_paused(&self) -> bool {
        self.timeline.is_paused()
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
        self.timeline.pause(at);
        write_manifest(&self.chunk_dir, &self.manifest)
    }
    pub fn resume(&mut self) -> io::Result<()> {
        if !self.timeline.is_paused() {
            return Ok(());
        }
        let at = self.clock.now_100ns();
        self.control(at, Command::Resume)?;
        self.timeline.resume(at);
        self.manifest.markers = self.timeline.markers().to_vec();
        write_manifest(&self.chunk_dir, &self.manifest)
    }
    pub fn stop(mut self) -> io::Result<RecordingManifest> {
        let at = self.clock.now_100ns();
        self.control(at, Command::Stop)?;
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
) {
    let mut timeline = SourceClock::new(start);
    let mut device: Option<O::Device> = None;
    let mut last_open_attempt = None;
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
                &mut device,
                &mut last_open_attempt,
                &opener,
                &clock,
            ) {
                failure = Some(format!("{} capture stopped: {error}", source.stem()));
                device = None;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn capture_pending<O: InputOpener, C: Clock>(
    source: Source,
    writer: &mut ChunkWriter,
    timeline: &mut SourceClock,
    device: &mut Option<O::Device>,
    last_open_attempt: &mut Option<Instant>,
    opener: &O,
    clock: &C,
) -> io::Result<()> {
    if device.is_none() && last_open_attempt.is_none_or(|at| at.elapsed() >= REOPEN_INTERVAL) {
        *last_open_attempt = Some(Instant::now());
        *device = opener.open(source).ok();
    }
    // WASAPI delivers about one packet per 10 ms; drain what is pending, bounded so control
    // messages are still checked.
    for _ in 0..MAX_PACKETS_PER_TICK {
        let Some(input) = device.as_mut() else { break };
        match input.read_packet() {
            Ok(Some(packet)) => writer.write(&timeline.packet(&packet, clock.now_100ns()))?,
            Ok(None) => break,
            Err(_) => *device = None,
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
    let manifest: RecordingManifest =
        serde_json::from_slice(&fs::read(chunk_dir.join("recording.json"))?)?;
    if manifest.number != number {
        return Err(LibraryError::Encode("Recording number mismatch".into()));
    }
    let mut meeting: MeetingFile =
        serde_json::from_slice(&fs::read(meeting_dir.join("meeting.json"))?)?;
    if meeting.schema_version != 1 {
        return Err(LibraryError::Encode("Unsupported Meeting schema".into()));
    }
    if meeting
        .recordings
        .iter()
        .any(|entry| matches!(entry, RecordingEntry::Known(r) if r.number == number))
    {
        return Err(LibraryError::Encode("Recording already finalised".into()));
    }
    let mic = ChunkSet::new(&chunk_dir, Source::Microphone)?;
    let system = ChunkSet::new(&chunk_dir, Source::SystemAudio)?;
    let frames = mic.frames.max(system.frames);
    let duration_ms = frames.saturating_mul(1000) / SAMPLE_RATE as u64;
    let timeline_start_ms = meeting
        .recordings
        .iter()
        .filter_map(|entry| match entry {
            RecordingEntry::Known(r) => Some(r.duration_ms),
            _ => None,
        })
        .sum();
    let mic_name = format!("recording-{number} microphone.flac");
    let system_name = format!("recording-{number} system.flac");
    let mut created = Vec::new();
    let result = (|| -> Result<(), LibraryError> {
        let mic_file = create_final(meeting_dir, &mic_name, &mut created)?;
        encode_flac(mic_file, &mic, frames)?;
        let system_file = create_final(meeting_dir, &system_name, &mut created)?;
        encode_flac(system_file, &system, frames)?;
        let mp3_file = create_final(meeting_dir, "meeting.mp3", &mut created)?;
        encode_mp3(mp3_file, &mic, &system, frames)?;
        meeting
            .recordings
            .push(RecordingEntry::Known(RecordingFile {
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
            }));
        meeting
            .markers
            .extend(manifest.markers.into_iter().map(|marker| match marker {
                Marker::Pause { at_ms, paused_ms } => Marker::Pause {
                    at_ms: at_ms + timeline_start_ms,
                    paused_ms,
                },
            }));
        meeting.duration_ms = timeline_start_ms + duration_ms;
        meeting.audio = Some("meeting.mp3".into());
        let temp = meeting_dir.join("meeting.json.tmp");
        let mut file = File::create(&temp)?;
        serde_json::to_writer_pretty(&mut file, &meeting)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(temp, meeting_dir.join("meeting.json"))?;
        Ok(())
    })();
    if let Err(error) = result {
        for path in created {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_file(meeting_dir.join("meeting.json.tmp"));
        return Err(error);
    }
    let _ = fs::remove_dir_all(chunk_dir);
    Ok(FinalizedRecording {
        number,
        duration_ms,
        timeline_start_ms,
    })
}

fn create_final(dir: &Path, name: &str, created: &mut Vec<PathBuf>) -> io::Result<File> {
    let path = dir.join(name);
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    created.push(path);
    Ok(file)
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
    mic: &ChunkSet,
    system: &ChunkSet,
    frames: u64,
) -> Result<(), LibraryError> {
    const BLOCK: usize = 4096;
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
    let mut offset = 0;
    let mut output = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(BLOCK).max(7200));
    while offset < frames {
        let count = (frames - offset).min(BLOCK as u64) as usize;
        let mut samples = Vec::with_capacity(count * 2);
        for _ in 0..count {
            let m = mic_reader.sample()?;
            samples.push(pcm(m + system_reader.sample()?));
            samples.push(pcm(m + system_reader.sample()?));
        }
        output.clear();
        encoder
            .encode_to_vec(InterleavedPcm(&samples), &mut output)
            .map_err(|e| LibraryError::Encode(e.to_string()))?;
        file.write_all(&output)?;
        offset += count as u64;
    }
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
