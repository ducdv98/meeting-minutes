use crate::{
    audio_prep::{prepare_import, prepare_recording, PreparedRecording},
    diagnostics::{log_event, LogEvent, TranscriptionEnvironment},
    meeting_file::{MeetingFile, RecordingEntry},
    transcript::{
        assign_ids, place_segments, Language, PlacedSegment, RawSegment, Speaker, Transcript,
    },
};
use serde::{Serialize, Serializer};
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
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
    fn detect_language(&mut self, samples_16k: &[f32]) -> Result<Language, String>;
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
        let total: usize = streams.iter().map(|samples| samples.len()).sum();
        let mut completed = 0usize;
        let mut results = Vec::with_capacity(streams.len());
        for samples in streams {
            let base = completed as f32 / total.max(1) as f32;
            let weight = samples.len() as f32 / total.max(1) as f32;
            let segments = self.transcribe(
                samples,
                language,
                initial_prompt,
                &mut |fraction| progress(base + fraction.clamp(0.0, 1.0) * weight),
                cancel,
            )?;
            results.push(StreamTranscription {
                segments,
                audio_after_vad_ms: self.audio_after_vad_ms(),
            });
            completed += samples.len();
        }
        Ok(results)
    }
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

pub fn transcribe_meeting(
    request: &TranscribeRequest,
    engine: &mut dyn SpeechEngine,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_named(request, engine, progress, cancel, None)
}

pub fn transcribe_meeting_named(
    request: &TranscribeRequest,
    engine: &mut dyn SpeechEngine,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_with_draft(request, engine, progress, cancel, your_name, None)
}

pub fn transcribe_meeting_named_with_draft(
    request: &TranscribeRequest,
    engine: &mut dyn SpeechEngine,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
    draft: &mut dyn FnMut(&[PlacedSegment]),
) -> Result<Transcript, ProcessingError> {
    transcribe_meeting_with_draft(request, engine, progress, cancel, your_name, Some(draft))
}

fn transcribe_meeting_with_draft(
    request: &TranscribeRequest,
    engine: &mut dyn SpeechEngine,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
    draft: Option<&mut DraftCallback<'_>>,
) -> Result<Transcript, ProcessingError> {
    let mut telemetry = ProcessingTelemetry {
        stage: ProcessingStage::Preparing,
        audio_ms: 0,
        total_ms: 0,
        whisper: Duration::ZERO,
    };
    let backend = engine.backend().to_owned();
    let device = engine.device().map(str::to_owned);
    let result =
        transcribe_meeting_inner(request, engine, progress, cancel, your_name, draft, &mut telemetry);
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

fn transcribe_meeting_inner(
    request: &TranscribeRequest,
    engine: &mut dyn SpeechEngine,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
    your_name: Option<&str>,
    mut draft: Option<&mut DraftCallback<'_>>,
    telemetry: &mut ProcessingTelemetry,
) -> Result<Transcript, ProcessingError> {
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
    let prompt = request
        .vocabulary
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let prompt = (!prompt.is_empty()).then_some(prompt.as_str());
    let mut language = match metadata.language.as_deref() {
        Some("en") => Some(Language::En),
        Some("vi") => Some(Language::Vi),
        None => request.default_language,
        Some(other) => {
            return Err(ProcessingError::Engine(format!(
                "Unsupported Meeting language: {other}"
            )))
        }
    };
    let detected = language.is_none();
    let mut per_source: [Vec<PlacedSegment>; 3] = std::array::from_fn(|_| Vec::new());
    let mut elapsed = 0u64;
    let mut preparation_wall_time = Duration::ZERO;
    let mut whisper_wall_time = Duration::ZERO;
    let mut transcribed_ms = 0u64;
    let mut audio_after_vad_ms = Some(0u64);
    let mut process = |prepared: Vec<PreparedRecording>,
                       elapsed: u64,
                       telemetry: &mut ProcessingTelemetry|
     -> Result<(), ProcessingError> {
        telemetry.stage = ProcessingStage::Transcribing;
        let whisper_started = Instant::now();
        if language.is_none() {
            if let Some(stream) = prepared
                .iter()
                .find(|stream| engine.has_speech(&stream.samples_16k))
            {
                language = Some(match engine.detect_language(&stream.samples_16k) {
                    Ok(language) => language,
                    Err(error) => {
                        telemetry.whisper += whisper_started.elapsed();
                        return Err(ProcessingError::Engine(error));
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
            .map(|stream| stream.samples_16k.as_slice())
            .collect::<Vec<_>>();
        let mut group_progress = 0.0f32;
        let result = engine.transcribe_streams(
            &streams,
            language.unwrap_or(Language::En),
            prompt,
            &mut |p| {
                group_progress = group_progress.max(p.clamp(0.0, 1.0));
                progress((offset + group_progress * weight).clamp(0.0, 1.0));
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
        for (stream, result) in prepared.iter().zip(results) {
            audio_after_vad_ms = audio_after_vad_ms
                .and_then(|total| result.audio_after_vad_ms.map(|duration| total + duration));
            let index = match stream.speaker {
                Speaker::Me => 0,
                Speaker::Remote => 1,
                Speaker::Unknown => 2,
            };
            let placed = place_segments(
                &result.segments,
                stream.duration_ms,
                stream.timeline_start_ms,
                stream.speaker,
            );
            // Emit only finished, placed Segments from this Source; ticket 07 moves this with its segmenter.
            if let Some(callback) = draft.as_deref_mut() {
                if !placed.is_empty() { callback(&placed); }
            }
            per_source[index].extend(placed);
        }
        whisper_wall_time += whisper_elapsed;
        transcribed_ms += group_ms;
        telemetry.audio_ms += group_ms;
        Ok(())
    };
    if let Some(import) = &metadata.import {
        telemetry.stage = ProcessingStage::Preparing;
        let preparation_started = Instant::now();
        let prepared = prepare_import(request.meeting_dir, &import.file, cancel)?;
        preparation_wall_time += preparation_started.elapsed();
        process(vec![prepared], 0, telemetry)?;
    } else {
        for recording in recordings {
            telemetry.stage = ProcessingStage::Preparing;
            if cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            let preparation_started = Instant::now();
            let prepared = prepare_recording(request.meeting_dir, recording, cancel)?;
            preparation_wall_time += preparation_started.elapsed();
            process(prepared, elapsed, telemetry)?;
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
        model: request.model.into(),
        language: language.unwrap_or(Language::En),
        language_detected: detected && language.is_some(),
        duration_ms: metadata.duration_ms,
        segments: assign_ids(
            per_source.into_iter().flatten().collect(),
            metadata.duration_ms,
        ),
    };
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
