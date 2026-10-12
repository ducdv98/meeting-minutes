use std::{
    fmt, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use chrono::{DateTime, FixedOffset};
use serde::{Serialize, Serializer};

use crate::{folder_name, index::Index, meeting_file::MeetingFile, MeetingSummary};

#[derive(Debug)]
pub enum LibraryError {
    Io(std::io::Error),
    Index(rusqlite::Error),
    Json(serde_json::Error),
    CollisionLimit,
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Library I/O error: {error}"),
            Self::Index(error) => write!(f, "Library index error: {error}"),
            Self::Json(error) => write!(f, "Meeting metadata error: {error}"),
            Self::CollisionLimit => write!(f, "Could not find an unused Meeting folder name"),
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
        },
        started_at.timestamp_millis(),
    ))
}
