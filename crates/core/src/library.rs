use std::{
    fmt, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, FixedOffset};
use serde::{Serialize, Serializer};

use crate::{
    ai::ProviderKind,
    folder_name,
    import::{audio_duration_ms, import_extension},
    index::Index,
    meeting_file::{ImportFile, MeetingDetail, MeetingFile, RecordingEntry},
    metadata::{update_metadata, update_metadata_and, with_current_metadata},
    participant::{default_name, label_key, Participants, SegmentParticipants},
    recording::{write_manifest, RecordingManifest},
    summary::{read_summary, render_summary_markdown},
    transcript::{render_markdown_named, Language, Speaker, Transcript},
    MeetingSummary, SearchHit,
};

#[derive(Debug)]
pub enum LibraryError {
    Io(std::io::Error),
    Index(rusqlite::Error),
    Json(serde_json::Error),
    Encode(String),
    CollisionLimit,
    UnsavedRecording,
    ImportedMeeting,
    EmptyTitle,
    MeetingNotFound,
    StaleRevision,
    SegmentNotFound,
    EmptySegmentText,
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Library I/O error: {error}"),
            Self::Index(error) => write!(f, "Library index error: {error}"),
            Self::Json(error) => write!(f, "Meeting metadata error: {error}"),
            Self::Encode(error) => write!(f, "Audio encoding error: {error}"),
            Self::CollisionLimit => write!(f, "Could not find an unused Meeting folder name"),
            Self::ImportedMeeting => write!(f, "An imported Meeting cannot get new Recordings"),
            Self::UnsavedRecording => {
                write!(f, "This Meeting has a Recording that is not saved yet")
            }
            Self::EmptyTitle => write!(f, "A Meeting title cannot be empty"),
            Self::MeetingNotFound => write!(f, "This Meeting is no longer in the Library"),
            Self::StaleRevision => write!(
                f,
                "The Transcript has a newer Revision; reload it before editing"
            ),
            Self::SegmentNotFound => write!(f, "This Segment is not in the Transcript"),
            Self::EmptySegmentText => write!(f, "A Segment's text cannot be empty"),
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

pub struct PreparedImport {
    staging: PathBuf,
    file: MeetingFile,
    imported_at: DateTime<FixedOffset>,
}

impl Drop for PreparedImport {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.staging);
    }
}

fn import_error(error: crate::processing::ProcessingError) -> LibraryError {
    match error {
        crate::processing::ProcessingError::Cancelled => {
            LibraryError::Encode("Import cancelled".into())
        }
        other => LibraryError::Encode(other.to_string()),
    }
}

pub struct Library {
    root: PathBuf,
    index: Index,
    your_name: Option<String>,
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
        let mut library = Self { root, index, your_name: None };
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
        let base = folder_name(started_at);
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

    pub fn prepare_import_meeting(
        root: &Path,
        source: &Path,
        imported_at: DateTime<FixedOffset>,
        cancel: &AtomicBool,
    ) -> Result<PreparedImport, LibraryError> {
        let extension = import_extension(source).ok_or_else(|| {
            LibraryError::Encode("Unsupported audio file: not an mp3, m4a, wav or flac file".into())
        })?;
        if cancel.load(Ordering::SeqCst) {
            return Err(LibraryError::Encode("Import cancelled".into()));
        }
        let original_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Imported audio")
            .to_owned();
        let title = source
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .trim();
        let title = if title.is_empty() {
            "Imported audio"
        } else {
            title
        };
        let audio = format!("imported.{extension}");
        let started_at = imported_at.to_rfc3339();
        let staging = root.join(format!(
            ".import-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir(&staging)?;
        let result = (|| -> Result<MeetingFile, LibraryError> {
            let mut input = fs::File::open(source)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(staging.join(format!("{audio}.tmp")))?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                if cancel.load(Ordering::SeqCst) {
                    return Err(LibraryError::Encode("Import cancelled".into()));
                }
                let read = std::io::Read::read(&mut input, &mut buffer)?;
                if read == 0 {
                    break;
                }
                output.write_all(&buffer[..read])?;
            }
            output.sync_all()?;
            fs::rename(staging.join(format!("{audio}.tmp")), staging.join(&audio))?;
            // The staged copy is what the Meeting keeps, so it is the file that gets validated and measured.
            let duration_ms =
                audio_duration_ms(&staging.join(&audio), cancel).map_err(import_error)?;
            if duration_ms == 0 {
                return Err(LibraryError::Encode(
                    "This audio file has no sound to import".into(),
                ));
            }
            let mut file = MeetingFile::new(title, started_at.clone());
            file.duration_ms = duration_ms;
            file.audio = Some(audio.clone());
            file.import = Some(ImportFile {
                file: audio,
                original_name,
                imported_at: started_at,
            });
            write_meeting_file(&staging, &file)?;
            Ok(file)
        })();
        match result {
            Ok(file) => Ok(PreparedImport {
                staging,
                file,
                imported_at,
            }),
            Err(error) => {
                let _ = fs::remove_dir_all(&staging);
                Err(error)
            }
        }
    }

    pub fn publish_import_meeting(
        &mut self,
        prepared: PreparedImport,
        cancel: &AtomicBool,
    ) -> Result<MeetingSummary, LibraryError> {
        let staging = &prepared.staging;
        let file = &prepared.file;
        let imported_at = prepared.imported_at;
        (|| -> Result<MeetingSummary, LibraryError> {
            if cancel.load(Ordering::SeqCst) {
                return Err(LibraryError::Encode("Import cancelled".into()));
            }
            let base = folder_name(imported_at);
            for number in 1..=1000 {
                let id = if number == 1 {
                    base.clone()
                } else {
                    format!("{base} ({number})")
                };
                let path = self.root.join(&id);
                if fs::symlink_metadata(&path).is_ok() {
                    continue;
                }
                match fs::rename(staging, &path) {
                    Ok(()) => {
                        let summary = MeetingSummary {
                            id,
                            title: file.title.clone(),
                            started_at: file.started_at.clone(),
                            duration_ms: file.duration_ms,
                        };
                        self.index
                            .insert(&summary, imported_at.timestamp_millis())?;
                        return Ok(summary);
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::AlreadyExists | io::ErrorKind::DirectoryNotEmpty
                        ) =>
                    {
                        continue
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Err(LibraryError::CollisionLimit)
        })()
    }

    pub fn import_audio(
        &mut self,
        source: &Path,
        imported_at: DateTime<FixedOffset>,
        cancel: &AtomicBool,
    ) -> Result<MeetingSummary, LibraryError> {
        let prepared = Self::prepare_import_meeting(&self.root, source, imported_at, cancel)?;
        self.publish_import_meeting(prepared, cancel)
    }

    pub fn list_meetings(&self) -> Result<Vec<MeetingSummary>, LibraryError> {
        self.index.list()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn set_your_name(&mut self, name: Option<&str>) {
        self.your_name = name.map(str::trim).filter(|name| !name.is_empty()).map(str::to_owned);
    }

    pub fn participant_names(&self) -> Result<Vec<String>, LibraryError> {
        let mut names = std::collections::BTreeSet::new();
        for meeting in self.index.list()? {
            let file: MeetingFile = serde_json::from_slice(&fs::read(self.meeting_dir(&meeting.id).join("meeting.json"))?)?;
            names.extend(file.participants.into_values());
            if let Some(assignments) = file.segment_participants {
                if highest_transcript_revision(&self.meeting_dir(&meeting.id))? == Some(assignments.revision) {
                    names.extend(assignments.segments.into_values());
                }
            }
        }
        if let Some(name) = &self.your_name { names.insert(name.clone()); }
        Ok(names.into_iter().collect())
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
        let voice_note = file.is_voice_note();
        Ok(MeetingDetail {
            id: id.to_owned(),
            title: file.title,
            started_at: file.started_at,
            duration_ms: file.duration_ms,
            language: file.language,
            audio_path,
            markers: file.markers,
            imported: file.import.is_some(),
            sources: [crate::capture::Source::Microphone, crate::capture::Source::SystemAudio]
                .into_iter()
                .filter(|source| file.recordings.iter().any(|recording| matches!(recording, RecordingEntry::Known(known) if known.files.iter().any(|entry| entry.source == *source))))
                .collect(),
            voice_note,
            participants: file.participants,
            segment_participants: file.segment_participants,
        })
    }

    pub fn transcript(&self, id: &str) -> Result<Option<Transcript>, LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        highest_transcript_revision(&folder)?
            .map(|revision| read_transcript_revision(&folder, revision))
            .transpose()
    }

    pub fn transcript_revision(
        &self,
        id: &str,
        revision: u32,
    ) -> Result<Option<Transcript>, LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        match read_transcript_revision(&folder, revision) {
            Ok(transcript) => Ok(Some(transcript)),
            Err(LibraryError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn edit_segment(
        &mut self,
        id: &str,
        revision: u32,
        segment_id: &str,
        text: &str,
    ) -> Result<Transcript, LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        if highest_transcript_revision(&folder)? != Some(revision) {
            return Err(LibraryError::StaleRevision);
        }
        let mut transcript = read_transcript_revision(&folder, revision)?;
        let segment = transcript
            .segments
            .iter_mut()
            .find(|segment| segment.id == segment_id)
            .ok_or(LibraryError::SegmentNotFound)?;
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return Err(LibraryError::EmptySegmentText);
        }
        segment.text = text;
        let path = folder.join(format!("transcript.r{revision}.json"));
        let tmp = folder.join(format!("transcript.r{revision}.json.edit.tmp"));
        let write = (|| -> Result<(), LibraryError> {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp)?;
            serde_json::to_writer_pretty(&mut file, &transcript)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fs::rename(&tmp, path)?;
            Ok(())
        })();
        if let Err(error) = write {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        write_current_transcript_markdown(&folder, &transcript, self.your_name.as_deref())?;
        self.index.set_transcript(id, Some(&transcript))?;
        Ok(transcript)
    }

    pub fn set_language(&self, id: &str, language: Option<Language>) -> Result<(), LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        update_metadata(&folder, |metadata| {
            metadata["language"] = serde_json::to_value(language)?;
            Ok(())
        })
    }

    pub fn set_participant_name(&self, id: &str, label: Speaker, name: Option<&str>) -> Result<(), LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        let name = name.map(str::trim).filter(|name| !name.is_empty());
        let default = default_name(label, self.your_name.as_deref());
        update_metadata_and(&folder, |metadata| {
            let participants = metadata.as_object_mut().ok_or_else(|| LibraryError::Encode("Invalid Meeting metadata".into()))?
                .entry("participants").or_insert_with(|| serde_json::json!({}));
            let participants = participants.as_object_mut().ok_or_else(|| LibraryError::Encode("Invalid Participants".into()))?;
            match name.filter(|name| *name != default) {
                Some(name) => { participants.insert(label_key(label).into(), serde_json::json!(name)); }
                None => { participants.remove(label_key(label)); }
            }
            normalize_assignments(&folder, metadata, self.your_name.as_deref())
        }, |metadata| {
            if let Some(transcript) = self.transcript(id)? {
                let markdown = markdown_from_metadata(metadata, &transcript, self.your_name.as_deref())?;
                write_transcript_markdown(&folder, &markdown)?;
            }
            Ok(())
        })
    }

    pub fn assign_segments(&self, id: &str, revision: u32, segment_ids: &[String], name: Option<&str>) -> Result<(), LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        let name = name.map(str::trim).filter(|name| !name.is_empty());
        update_metadata_and(&folder, |metadata| {
            if highest_transcript_revision(&folder)? != Some(revision) { return Err(LibraryError::StaleRevision); }
            let transcript = read_transcript_revision(&folder, revision)?;
            let file: MeetingFile = serde_json::from_value(metadata.clone())?;
            if file.is_voice_note() { return Err(LibraryError::Encode("Every Segment of a Voice note belongs to you".into())); }
            let resolver = Participants::new(file.participants.clone(), file.segment_participants.clone(), self.your_name.as_deref(), file.is_voice_note());
            let mut assignments = file.segment_participants.filter(|map| map.revision == revision).unwrap_or(SegmentParticipants { revision, segments: Default::default() });
            for segment_id in segment_ids {
                let segment = transcript.segment(segment_id).ok_or(LibraryError::SegmentNotFound)?;
                match name.filter(|name| *name != resolver.label_name(segment.speaker)) {
                    Some(name) => { assignments.segments.insert(segment_id.clone(), name.to_owned()); }
                    None => { assignments.segments.remove(segment_id); }
                }
            }
            metadata["segment_participants"] = serde_json::to_value(assignments)?;
            Ok(())
        }, |metadata| self.write_participant_markdown(&folder, id, metadata))
    }

    pub fn rename_participant(&self, id: &str, from: &str, to: &str) -> Result<(), LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        let to = to.trim();
        if to.is_empty() { return Err(LibraryError::Encode("A Participant name cannot be empty".into())); }
        update_metadata_and(&folder, |metadata| {
            let file: MeetingFile = serde_json::from_value(metadata.clone())?;
            let voice_note = file.is_voice_note();
            let resolver = Participants::new(file.participants.clone(), file.segment_participants.clone(), self.your_name.as_deref(), file.is_voice_note());
            let mut labels = file.participants;
            for label in [Speaker::Me, Speaker::Remote, Speaker::Unknown] {
                if voice_note && label != Speaker::Me { continue; }
                if resolver.label_name(label) == from {
                    if to == default_name(label, self.your_name.as_deref()) { labels.remove(label_key(label)); }
                    else { labels.insert(label_key(label).into(), to.into()); }
                }
            }
            let mut assignments = file.segment_participants;
            if let Some(map) = &mut assignments {
                for name in map.segments.values_mut() { if name == from { *name = to.into(); } }
            }
            metadata["participants"] = serde_json::to_value(labels)?;
            metadata["segment_participants"] = serde_json::to_value(assignments)?;
            normalize_assignments(&folder, metadata, self.your_name.as_deref())
        }, |metadata| self.write_participant_markdown(&folder, id, metadata))
    }

    pub fn remove_participant(&self, id: &str, name: &str) -> Result<(), LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        update_metadata_and(&folder, |metadata| {
            let file: MeetingFile = serde_json::from_value(metadata.clone())?;
            let mut labels = file.participants;
            labels.retain(|_, value| value != name);
            let mut assignments = file.segment_participants;
            if let Some(map) = &mut assignments { map.segments.retain(|_, value| value != name); }
            metadata["participants"] = serde_json::to_value(labels)?;
            metadata["segment_participants"] = serde_json::to_value(assignments)?;
            normalize_assignments(&folder, metadata, self.your_name.as_deref())
        }, |metadata| self.write_participant_markdown(&folder, id, metadata))
    }

    fn write_participant_markdown(&self, folder: &Path, id: &str, metadata: &serde_json::Value) -> Result<(), LibraryError> {
        if let Some(transcript) = self.transcript(id)? {
            let markdown = markdown_from_metadata(metadata, &transcript, self.your_name.as_deref())?;
            write_transcript_markdown(folder, &markdown)?;
        }
        Ok(())
    }

    pub fn cloud_consent(&self, id: &str, provider: ProviderKind) -> Result<bool, LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        let file: MeetingFile = serde_json::from_slice(&fs::read(folder.join("meeting.json"))?)?;
        Ok(file
            .cloud_consent
            .iter()
            .any(|consent| consent.provider == provider))
    }

    pub fn record_cloud_consent(
        &self,
        id: &str,
        provider: ProviderKind,
        at: DateTime<FixedOffset>,
    ) -> Result<(), LibraryError> {
        if !provider.is_cloud() {
            return Err(LibraryError::Encode(
                "Ollama runs on this PC and needs no consent".into(),
            ));
        }
        let folder = self.checked_meeting_dir(id)?;
        update_metadata(&folder, |metadata| {
            let entries = metadata["cloud_consent"].as_array().cloned().unwrap_or_default();
            if entries.iter().any(|entry| entry["provider"] == serde_json::json!(provider)) {
                return Ok(());
            }
            let mut entries = entries;
            entries.push(serde_json::json!({"provider": provider, "consented_at": at.to_rfc3339()}));
            metadata["cloud_consent"] = serde_json::Value::Array(entries);
            Ok(())
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
        let folder = self.checked_meeting_dir(id)?;
        update_metadata_and(&folder, |metadata| {
            metadata["title"] = serde_json::Value::String(title.to_owned());
            Ok(())
        }, |metadata| {
            if let Ok(Some(transcript)) = self.transcript(id) {
                if let Ok(markdown) = markdown_from_metadata(metadata, &transcript, self.your_name.as_deref()) {
                    let _ = write_transcript_markdown(&folder, &markdown);
                }
            }
            if let Ok(Some(saved_summary)) = read_summary(&folder) {
                let _ = write_summary_markdown(&folder, &render_summary_markdown(title, &saved_summary));
            }
            Ok(())
        })?;
        let (summary, started_at_ms) = read_meeting(&folder, id).map_err(LibraryError::Encode)?;
        self.index.insert(&summary, started_at_ms)?;
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
        if meeting.import.is_some() {
            return Err(LibraryError::ImportedMeeting);
        }
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

    pub fn refresh_transcript(&mut self, id: &str) -> Result<(), LibraryError> {
        let transcript = self.transcript(id)?;
        self.index.set_transcript(id, transcript.as_ref())
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>, LibraryError> {
        self.index.search(query, 100)
    }

    pub fn delete_meeting(
        &mut self,
        id: &str,
        trash: &dyn Fn(&Path) -> io::Result<()>,
    ) -> Result<(), LibraryError> {
        let folder = self.checked_meeting_dir(id)?;
        if !unsaved_recordings(&folder)?.is_empty() {
            return Err(LibraryError::UnsavedRecording);
        }
        trash(&folder)?;
        let _ = self.index.remove_meeting(id);
        Ok(())
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
                Ok((meeting, started_at_ms)) => {
                    let transcript = highest_transcript_revision(&entry.path())
                        .ok()
                        .flatten()
                        .and_then(|revision| {
                            read_transcript_revision(&entry.path(), revision).ok()
                        });
                    valid.push((meeting, started_at_ms, transcript));
                }
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

fn markdown_from_metadata(metadata: &serde_json::Value, transcript: &Transcript, your_name: Option<&str>) -> io::Result<String> {
    let file: MeetingFile = serde_json::from_value(metadata.clone()).map_err(io::Error::other)?;
    Ok(render_markdown_named(&file.title, transcript, &Participants::new(file.participants.clone(), file.segment_participants.clone(), your_name, file.is_voice_note())))
}

fn normalize_assignments(folder: &Path, metadata: &mut serde_json::Value, your_name: Option<&str>) -> Result<(), LibraryError> {
    let file: MeetingFile = serde_json::from_value(metadata.clone())?;
    let voice_note = file.is_voice_note();
    let Some(mut assignments) = file.segment_participants else { return Ok(()); };
    let transcript = match read_transcript_revision(folder, assignments.revision) {
        Ok(transcript) => transcript,
        Err(LibraryError::Io(error)) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let participants = Participants::new(file.participants, None, your_name, voice_note);
    assignments.segments.retain(|id, name| transcript.segment(id).is_none_or(|segment| *name != participants.label_name(segment.speaker)));
    metadata["segment_participants"] = serde_json::to_value(assignments)?;
    Ok(())
}

fn write_current_transcript_markdown(folder: &Path, transcript: &Transcript, your_name: Option<&str>) -> io::Result<()> {
    with_current_metadata(folder, |metadata| {
        write_transcript_markdown(folder, &markdown_from_metadata(metadata, transcript, your_name)?)
    })
}

fn write_transcript_markdown(folder: &Path, markdown: &str) -> io::Result<()> {
    let tmp = folder.join("transcript.md.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)?;
    file.write_all(markdown.as_bytes())?;
    file.sync_all()?;
    fs::rename(tmp, folder.join("transcript.md"))
}

fn write_summary_markdown(folder: &Path, markdown: &str) -> io::Result<()> {
    let tmp = folder.join("summary.md.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)?;
    file.write_all(markdown.as_bytes())?;
    file.sync_all()?;
    fs::rename(tmp, folder.join("summary.md"))
}

fn highest_transcript_revision(folder: &Path) -> Result<Option<u32>, LibraryError> {
    let mut latest = None;
    for entry in fs::read_dir(folder)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        let revision = name
            .strip_prefix("transcript.r")
            .and_then(|name| name.strip_suffix(".json"))
            .and_then(|name| name.parse::<u32>().ok());
        if let Some(revision) = revision {
            latest = Some(latest.map_or(revision, |current: u32| current.max(revision)));
        }
    }
    Ok(latest)
}

fn read_transcript_revision(folder: &Path, revision: u32) -> Result<Transcript, LibraryError> {
    Ok(serde_json::from_slice(&fs::read(
        folder.join(format!("transcript.r{revision}.json")),
    )?)?)
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
