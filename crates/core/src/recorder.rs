//! The app's recording state: at most one active Recording, plus Recordings waiting to be saved.

use std::{fmt, path::PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::{Serialize, Serializer};

use crate::{
    capture::{Clock, InputOpener},
    finalize_recording,
    recording::CaptureSession,
    Library, LibraryError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Recording,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveState {
    Saving,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PendingSave {
    pub meeting_id: String,
    pub state: SaveState,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecorderStatus {
    pub phase: Phase,
    pub meeting_id: Option<String>,
    pub elapsed_ms: u64,
    pub notice_acknowledged: bool,
    pub saves: Vec<PendingSave>,
}

/// A stopped Recording whose Chunks still have to be written as FLAC, MP3 and `meeting.json`.
/// Run it with [`SaveJob::run`] outside any lock, then report back with [`Recorder::save_finished`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveJob {
    pub meeting_id: String,
    pub meeting_dir: PathBuf,
    pub number: u32,
}

impl SaveJob {
    pub fn run(&self) -> Result<(), LibraryError> {
        finalize_recording(&self.meeting_dir, self.number).map(|_| ())
    }
}

#[derive(Debug)]
pub enum RecorderError {
    NoticeRequired,
    AlreadyActive,
    NotActive,
    NoFailedSave,
    Library(LibraryError),
    Capture(std::io::Error),
}

impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoticeRequired => write!(f, "The Recording notice has not been shown yet"),
            Self::AlreadyActive => write!(f, "A Recording is already active"),
            Self::NotActive => write!(f, "No Recording is active"),
            Self::NoFailedSave => write!(f, "That Meeting has no failed save to retry"),
            Self::Library(error) => error.fmt(f),
            Self::Capture(error) => write!(f, "Capture error: {error}"),
        }
    }
}

impl std::error::Error for RecorderError {}

impl Serialize for RecorderError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<LibraryError> for RecorderError {
    fn from(error: LibraryError) -> Self {
        Self::Library(error)
    }
}

impl From<std::io::Error> for RecorderError {
    fn from(error: std::io::Error) -> Self {
        Self::Capture(error)
    }
}

struct Active<C: Clock> {
    session: CaptureSession<C>,
    job: SaveJob,
}

struct Save {
    job: SaveJob,
    state: SaveState,
    error: Option<String>,
}

pub struct Recorder<O: InputOpener, C: Clock> {
    opener: O,
    clock: C,
    notice_acknowledged: bool,
    active: Option<Active<C>>,
    saves: Vec<Save>,
}

impl<O: InputOpener, C: Clock> Recorder<O, C> {
    pub fn new(opener: O, clock: C, notice_acknowledged: bool) -> Self {
        Self {
            opener,
            clock,
            notice_acknowledged,
            active: None,
            saves: Vec::new(),
        }
    }

    pub fn acknowledge_notice(&mut self) {
        self.notice_acknowledged = true;
    }

    pub fn status(&self) -> RecorderStatus {
        let (phase, meeting_id, elapsed_ms) = match &self.active {
            None => (Phase::Idle, None, 0),
            Some(active) => (
                if active.session.is_paused() {
                    Phase::Paused
                } else {
                    Phase::Recording
                },
                Some(active.job.meeting_id.clone()),
                active.session.elapsed_ms(),
            ),
        };
        RecorderStatus {
            phase,
            meeting_id,
            elapsed_ms,
            notice_acknowledged: self.notice_acknowledged,
            saves: self
                .saves
                .iter()
                .map(|save| PendingSave {
                    meeting_id: save.job.meeting_id.clone(),
                    state: save.state,
                    error: save.error.clone(),
                })
                .collect(),
        }
    }

    /// Starts a new Meeting and its first Recording. Only a deliberate call starts capture.
    pub fn start(
        &mut self,
        library: &mut Library,
        started_at: DateTime<FixedOffset>,
    ) -> Result<(), RecorderError> {
        if !self.notice_acknowledged {
            return Err(RecorderError::NoticeRequired);
        }
        if self.active.is_some() {
            return Err(RecorderError::AlreadyActive);
        }
        let (meeting, chunk_dir, manifest) = library.begin_recording(started_at)?;
        let job = SaveJob {
            meeting_dir: library.meeting_dir(&meeting.id),
            meeting_id: meeting.id,
            number: manifest.number,
        };
        let session =
            CaptureSession::start(chunk_dir, manifest, self.opener.clone(), self.clock.clone())?;
        self.active = Some(Active { session, job });
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), RecorderError> {
        let active = self.active.as_mut().ok_or(RecorderError::NotActive)?;
        active.session.pause()?;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), RecorderError> {
        let active = self.active.as_mut().ok_or(RecorderError::NotActive)?;
        active.session.resume()?;
        Ok(())
    }

    /// Stops capture and queues the Recording for saving. A capture error on stop still queues
    /// the save as failed, so the Chunks written so far can be saved with Retry save.
    pub fn stop(&mut self) -> Result<SaveJob, RecorderError> {
        let active = self.active.take().ok_or(RecorderError::NotActive)?;
        let job = active.job.clone();
        match active.session.stop() {
            Ok(_) => {
                self.saves.push(Save {
                    job: job.clone(),
                    state: SaveState::Saving,
                    error: None,
                });
                Ok(job)
            }
            Err(error) => {
                self.saves.push(Save {
                    job,
                    state: SaveState::Failed,
                    error: Some(error.to_string()),
                });
                Err(error.into())
            }
        }
    }

    pub fn save_finished(&mut self, meeting_id: &str, result: Result<(), String>) {
        let Some(position) = self
            .saves
            .iter()
            .position(|save| save.job.meeting_id == meeting_id)
        else {
            return;
        };
        match result {
            Ok(()) => {
                self.saves.remove(position);
            }
            Err(error) => {
                let save = &mut self.saves[position];
                save.state = SaveState::Failed;
                save.error = Some(error);
            }
        }
    }

    pub fn retry_save(&mut self, meeting_id: &str) -> Result<SaveJob, RecorderError> {
        let save = self
            .saves
            .iter_mut()
            .find(|save| save.job.meeting_id == meeting_id && save.state == SaveState::Failed)
            .ok_or(RecorderError::NoFailedSave)?;
        save.state = SaveState::Saving;
        save.error = None;
        Ok(save.job.clone())
    }
}
