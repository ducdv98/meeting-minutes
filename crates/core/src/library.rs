use std::{
    fmt, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use chrono::{DateTime, FixedOffset};
use serde::{Serialize, Serializer};

use crate::{
    folder_name,
    index::Index,
    meeting_file::{MeetingDetail, MeetingFile, RecordingEntry},
    recording::{write_manifest, RecordingManifest},
    MeetingSummary,
};

#[derive(Debug)]
pub enum LibraryError {
    Io(std::io::Error),
    Index(rusqlite::Error),
    Json(serde_json::Error),
    Encode(String),
    CollisionLimit,
    UnsavedRecording,
    EmptyTitle,
    MeetingNotFound,
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Library I/O error: {error}"),
            Self::Index(error) => write!(f, "Library index error: {error}"),
            Self::Json(error) => write!(f, "Meeting metadata error: {error}"),
            Self::Encode(error) => write!(f, "Audio encoding error: {error}"),
            Self::CollisionLimit => write!(f, "Could not find an unused Meeting folder name"),
            Self::UnsavedRecording => {
                write!(f, "This Meeting has a Recording that is not saved yet")
            }
            Self::EmptyTitle => write!(f, "A Meeting title cannot be empty"),
            Self::MeetingNotFound => write!(f, "This Meeting is no longer in the Library"),
        }
    }
}

impl std::error::Error for LibraryError {}

impl Serialize for LibraryError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for LibraryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for LibraryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Index(error)
    }
}

impl From<serde_json::Error> for LibraryError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Debug, Serialize)]
pub struct MalformedFolder {
    pub folder: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub meetings: Vec<MeetingSummary>,
    pub malformed: Vec<MalformedFolder>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OrphanRecording {
    pub meeting_id: String,
    pub number: u32,
}

pub struct Library {
    root: PathBuf,
    index: Index,
}

impl Library {
    pub fn open(
        root: impl Into<PathBuf>,
        index_path: impl AsRef<Path>,
    ) -> Result<Self, LibraryError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        let index = Index::open(index_path.as_ref())?;
        let fresh = index.fresh;
        let mut library = Self { root, index };
        if fresh {
            library.rescan()?;
        }
        Ok(library)
    }

    pub fn create_meeting(
        &mut self,
        title: &str,
        started_at: DateTime<FixedOffset>,
    ) -> Result<MeetingSummary, LibraryError> {
        let base = folder_name(started_at, title);
        for number in 1..=1000 {
            let id = if number == 1 {
                base.clone()
            } else {
                format!("{base} ({number})")
            };
            let path = self.root.join(&id);
            match fs::create_dir(&path) {
                Ok(()) => {
                    let started_at_string = started_at.to_rfc3339();
                    let meeting_file = MeetingFile::new(title, started_at_string.clone());
                    if let Err(error) = write_meeting_file(&path, &meeting_file) {
                        // The folder is ours and only holds the failed meeting.json.
                        let _ = fs::remove_dir_all(&path);
                        return Err(error);
                    }
                    let summary = MeetingSummary {
                        id,
                        title: title.to_owned(),
                        started_at: started_at_string,
                        duration_ms: 0,
                    };
                    self.index.insert(&summary, started_at.timestamp_millis())?;
                    return Ok(summary);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(LibraryError::CollisionLimit)
    }

    pub fn list_meetings(&self) -> Result<Vec<MeetingSummary>, LibraryError> {
        self.index.list()
    }

    pub fn meeting_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    pub fn meeting(&self, id: &str) -> Result<MeetingDetail, LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        let file: MeetingFile = serde_json::from_slice(&fs::read(folder.join("meeting.json"))?)?;
        let audio_path = file
            .audio
            .as_ref()
            // A plain file name in the folder; canonicalize would give \\?\ paths the asset scope rejects.
            .filter(|name| Path::new(name).file_name() == Some(std::ffi::OsStr::new(name)))
            .map(|name| folder.join(name))
            .filter(|path| path.is_file());
        Ok(MeetingDetail {
            id: id.to_owned(),
            title: file.title,
            started_at: file.started_at,
            duration_ms: file.duration_ms,
            language: file.language,
            audio_path,
            markers: file.markers,
        })
    }

    pub fn rename_meeting(
        &mut self,
        id: &str,
        title: &str,
    ) -> Result<MeetingSummary, LibraryError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(LibraryError::EmptyTitle);
        }
        let old_folder = self.checked_meeting_dir(id)?;
        if !unsaved_recordings(&old_folder)?.is_empty() {
            return Err(LibraryError::UnsavedRecording);
        }
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(old_folder.join("meeting.json"))?)?;
        let started_at = metadata["started_at"]
            .as_str()
            .ok_or_else(|| LibraryError::Encode("Invalid started_at".into()))?;
        let started_at = DateTime::parse_from_rfc3339(started_at)
            .map_err(|error| LibraryError::Encode(format!("Invalid started_at: {error}")))?;
        let base = folder_name(started_at, title);
        let mut selected = None;
        for number in 1..=1000 {
            let new_id = if number == 1 {
                base.clone()
            } else {
                format!("{base} ({number})")
            };
            let new_folder = self.root.join(&new_id);
            if new_id != id {
                let case_only = new_id.to_lowercase() == id.to_lowercase();
                if case_only {
                    // On Windows the target resolves to the source; on case-sensitive file systems
                    // it might be a distinct occupied folder.
                    if let Ok(info) = fs::symlink_metadata(&new_folder) {
                        if info.file_type().is_symlink()
                            || fs::canonicalize(&new_folder)? != fs::canonicalize(&old_folder)?
                        {
                            continue;
                        }
                    }
                } else if fs::symlink_metadata(&new_folder).is_ok() {
                    continue;
                }
                match fs::rename(&old_folder, &new_folder) {
                    Ok(()) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::AlreadyExists
                                | std::io::ErrorKind::DirectoryNotEmpty
                        ) =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            selected = Some((new_id, new_folder));
            break;
        }
        let (new_id, new_folder) = selected.ok_or(LibraryError::CollisionLimit)?;
        metadata["title"] = serde_json::Value::String(title.to_owned());
        let write_result = (|| -> Result<(), LibraryError> {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(new_folder.join("meeting.json.tmp"))?;
            serde_json::to_writer_pretty(&mut file, &metadata)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fs::rename(
                new_folder.join("meeting.json.tmp"),
                new_folder.join("meeting.json"),
            )?;
            Ok(())
        })();
        if let Err(error) = write_result {
            if new_id != id {
                let _ = fs::rename(&new_folder, &old_folder);
            }
            return Err(error);
        }
        let (summary, started_at_ms) =
            read_meeting(&new_folder, &new_id).map_err(LibraryError::Encode)?;
        // The folder is renamed; a stale index row is fixed by the next rescan.
        let _ = self.index.rename(id, &summary, started_at_ms);
        Ok(summary)
    }

    fn checked_meeting_dir(&self, id: &str) -> Result<PathBuf, LibraryError> {
        if id.is_empty()
            || id == "."
            || id == ".."
            || id.starts_with('.')
            || id.contains('/')
            || id.contains('\\')
        {
            return Err(LibraryError::MeetingNotFound);
        }
        let folder = self.meeting_dir(id);
        if !folder.join("meeting.json").is_file() {
            return Err(LibraryError::MeetingNotFound);
        }
        Ok(folder)
    }

    pub fn begin_recording(
        &mut self,
        started_at: DateTime<FixedOffset>,
    ) -> Result<(MeetingSummary, PathBuf, RecordingManifest), LibraryError> {
        let meeting = self.create_meeting("Untitled meeting", started_at)?;
        let chunk_dir = self.meeting_dir(&meeting.id).join(".chunks/recording-1");
        fs::create_dir_all(&chunk_dir)?;
        let manifest = RecordingManifest {
            number: 1,
            started_at: started_at.to_rfc3339(),
            duration_ms: None,
            markers: Vec::new(),
        };
        write_manifest(&chunk_dir, &manifest)?;
        Ok((meeting, chunk_dir, manifest))
    }

    pub fn orphan_recordings(&self) -> Result<Vec<OrphanRecording>, LibraryError> {
        let mut orphans = Vec::new();
        for meeting in fs::read_dir(&self.root)? {
            let meeting = meeting?;
            if !meeting.file_type()?.is_dir()
                || meeting.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            let meeting_id = meeting.file_name().to_string_lossy().into_owned();
            for number in unsaved_recordings(&meeting.path())? {
                orphans.push(OrphanRecording {
                    meeting_id: meeting_id.clone(),
                    number,
                });
            }
        }
        orphans.sort_by(|a, b| {
            a.meeting_id
                .cmp(&b.meeting_id)
                .then(a.number.cmp(&b.number))
        });
        Ok(orphans)
    }

    pub fn begin_next_recording(
        &mut self,
        meeting_id: &str,
        started_at: DateTime<FixedOffset>,
    ) -> Result<(PathBuf, RecordingManifest), LibraryError> {
        let meeting_dir = self.meeting_dir(meeting_id);
        let meeting: MeetingFile =
            serde_json::from_slice(&fs::read(meeting_dir.join("meeting.json"))?)?;
        if !unsaved_recordings(&meeting_dir)?.is_empty() {
            return Err(LibraryError::UnsavedRecording);
        }
        let chunks = meeting_dir.join(".chunks");
        let highest = meeting
            .recordings
            .iter()
            .filter_map(|entry| match entry {
                RecordingEntry::Known(recording) => Some(recording.number),
                RecordingEntry::Unknown(_) => None,
            })
            .max()
            .unwrap_or(0);
        let number = highest
            .checked_add(1)
            .ok_or_else(|| LibraryError::Encode("Recording number overflow".into()))?;
        fs::create_dir_all(&chunks)?;
        let chunk_dir = chunks.join(format!("recording-{number}"));
        fs::create_dir(&chunk_dir)?;
        let manifest = RecordingManifest {
            number,
            started_at: started_at.to_rfc3339(),
            duration_ms: None,
            markers: Vec::new(),
        };
        write_manifest(&chunk_dir, &manifest)?;
        Ok((chunk_dir, manifest))
    }

    pub fn record_finalized(&mut self, meeting_id: &str) -> Result<MeetingSummary, LibraryError> {
        let (summary, started_at_ms) = read_meeting(&self.meeting_dir(meeting_id), meeting_id)
            .map_err(LibraryError::Encode)?;
        self.index.insert(&summary, started_at_ms)?;
        Ok(summary)
    }

    pub fn rescan(&mut self) -> Result<ScanReport, LibraryError> {
        let mut valid = Vec::new();
        let mut malformed = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let folder = entry.file_name().to_string_lossy().into_owned();
            if folder.starts_with('.') {
                continue;
            }
            match read_meeting(&entry.path(), &folder) {
                Ok(meeting) => valid.push(meeting),
                Err(reason) => malformed.push(MalformedFolder { folder, reason }),
            }
        }
        self.index.replace(&valid)?;
        malformed.sort_by(|a, b| a.folder.cmp(&b.folder));
        Ok(ScanReport {
            meetings: self.list_meetings()?,
            malformed,
        })
    }
}

/// Numbers of the Recordings whose Chunks are still in the Meeting folder, waiting to be saved.
fn unsaved_recordings(meeting_dir: &Path) -> Result<Vec<u32>, LibraryError> {
    let chunks = meeting_dir.join(".chunks");
    if !chunks.is_dir() {
        return Ok(Vec::new());
    }
    let mut numbers = Vec::new();
    for entry in fs::read_dir(chunks)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(number) = entry
            .file_name()
            .to_string_lossy()
            .strip_prefix("recording-")
            .and_then(|n| n.parse().ok())
        {
            numbers.push(number);
        }
    }
    Ok(numbers)
}

fn write_meeting_file(folder: &Path, meeting_file: &MeetingFile) -> Result<(), LibraryError> {
    let bytes = serde_json::to_vec_pretty(meeting_file)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(folder.join("meeting.json"))?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn read_meeting(path: &Path, id: &str) -> Result<(MeetingSummary, i64), String> {
    let bytes = fs::read(path.join("meeting.json")).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => "missing meeting.json".to_owned(),
        _ => format!("could not read meeting.json: {error}"),
    })?;
    let file: MeetingFile = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if file.schema_version != 1 {
        return Err(format!(
            "Unsupported schema_version: {}",
            file.schema_version
        ));
    }
    let started_at = DateTime::parse_from_rfc3339(&file.started_at)
        .map_err(|error| format!("Invalid started_at: {error}"))?;
    Ok((
        MeetingSummary {
            id: id.to_owned(),
            title: file.title,
            started_at: file.started_at,
            duration_ms: file.duration_ms,
        },
        started_at.timestamp_millis(),
    ))
}
