use crate::{
    audio_prep::prepare_recording,
    meeting_file::{MeetingFile, RecordingEntry},
    transcript::{
        assign_ids, place_segments, render_markdown, Language, PlacedSegment, RawSegment,
        Transcript,
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
};

pub trait SpeechEngine {
    fn detect_language(&mut self, samples_16k: &[f32]) -> Result<Language, String>;
    fn transcribe(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        initial_prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError>;
}

#[derive(Debug)]
pub enum ProcessingError {
    Cancelled,
    Io(io::Error),
    Json(serde_json::Error),
    Audio(String),
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

pub struct TranscribeRequest<'a> {
    pub meeting_dir: &'a Path,
    pub model: &'a str,
    pub vocabulary: &'a [String],
}

pub fn transcribe_meeting(
    request: &TranscribeRequest,
    engine: &mut dyn SpeechEngine,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
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
    if recordings.is_empty() {
        return Err(ProcessingError::NoAudio);
    }
    let total: u64 = recordings.iter().map(|r| r.duration_ms).sum();
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
        None => None,
        Some(other) => {
            return Err(ProcessingError::Engine(format!(
                "Unsupported Meeting language: {other}"
            )))
        }
    };
    let detected = language.is_none();
    let mut placed = Vec::<PlacedSegment>::new();
    let mut elapsed = 0u64;
    for recording in recordings {
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        let prepared = prepare_recording(request.meeting_dir, recording, cancel)?;
        if language.is_none() {
            language = Some(
                engine
                    .detect_language(&prepared.samples_16k)
                    .map_err(ProcessingError::Engine)?,
            );
        }
        let weight = if total == 0 {
            1.0
        } else {
            recording.duration_ms as f32 / total as f32
        };
        let offset = if total == 0 {
            0.0
        } else {
            elapsed as f32 / total as f32
        };
        let raw = engine.transcribe(
            &prepared.samples_16k,
            language.unwrap(),
            prompt,
            &mut |p| progress((offset + p.clamp(0.0, 1.0) * weight).clamp(0.0, 1.0)),
            cancel,
        )?;
        placed.extend(place_segments(
            &raw,
            prepared.duration_ms,
            prepared.timeline_start_ms,
            &prepared.activity,
        ));
        elapsed += recording.duration_ms;
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(ProcessingError::Cancelled);
    }
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
        language: language.unwrap(),
        language_detected: detected,
        duration_ms: metadata.duration_ms,
        segments: assign_ids(placed, metadata.duration_ms),
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
    let markdown = render_markdown(&metadata.title, &transcript);
    let write = (|| -> Result<(), ProcessingError> {
        let mut md = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&markdown_tmp)?;
        md.write_all(markdown.as_bytes())?;
        md.sync_all()?;
        fs::rename(&markdown_tmp, request.meeting_dir.join("transcript.md"))?;
        Ok(())
    })();
    if let Err(error) = write {
        let _ = fs::remove_file(&markdown_tmp);
        return Err(error);
    }
    progress(1.0);
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
