use std::path::PathBuf;

use crate::ai::ProviderKind;
use crate::{capture::Source, recording::Marker};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeetingSummary {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    pub meeting_id: String,
    pub revision: u32,
    pub title: String,
    pub started_at: String,
    pub segment_id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeetingDetail {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub language: Option<String>,
    pub audio_path: Option<PathBuf>,
    pub markers: Vec<Marker>,
    pub imported: bool,
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RecordingFile {
    pub number: u32,
    pub started_at: String,
    pub timeline_start_ms: u64,
    pub duration_ms: u64,
    pub files: Vec<SourceFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SourceFile {
    pub source: Source,
    pub file: String,
    pub channels: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum RecordingEntry {
    Known(RecordingFile),
    Unknown(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportFile {
    pub file: String,
    pub original_name: String,
    pub imported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CloudConsent {
    pub provider: ProviderKind,
    pub consented_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MeetingFile {
    pub schema_version: u32,
    pub title: String,
    pub started_at: String,
    pub language: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportFile>,
    #[serde(default)]
    pub recordings: Vec<RecordingEntry>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cloud_consent: Vec<CloudConsent>,
}

impl MeetingFile {
    pub(crate) fn new(title: &str, started_at: String) -> Self {
        Self {
            schema_version: 1,
            title: title.to_owned(),
            started_at,
            language: None,
            duration_ms: 0,
            audio: None,
            import: None,
            recordings: Vec::new(),
            markers: Vec::new(),
            cloud_consent: Vec::new(),
        }
    }
}
