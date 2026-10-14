use std::path::PathBuf;

use crate::ai::ProviderKind;
use crate::participant::{MeetingParticipants, SegmentParticipants};
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
    pub voice_note: bool,
    pub participants: MeetingParticipants,
    pub segment_participants: Option<SegmentParticipants>,
    pub correction_notice: Option<String>,
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
    #[serde(default, skip_serializing_if = "MeetingParticipants::is_empty")]
    pub participants: MeetingParticipants,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_participants: Option<SegmentParticipants>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cloud_consent: Vec<CloudConsent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    // Keep this in meeting.json: moving it beside the Transcript fallback notice changes saved Meetings.
    pub correction_notice: Option<String>,
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
            participants: MeetingParticipants::new(),
            segment_participants: None,
            cloud_consent: Vec::new(),
            correction_notice: None,
        }
    }
}

impl MeetingFile {
    pub(crate) fn is_voice_note(&self) -> bool {
        if self.import.is_some() {
            return false;
        }
        let sources: Vec<_> = self
            .recordings
            .iter()
            .filter_map(|recording| match recording {
                RecordingEntry::Known(known) => Some(known.files.iter().map(|file| file.source)),
                _ => None,
            })
            .flatten()
            .collect();
        !sources.is_empty() && sources.iter().all(|source| *source == Source::Microphone)
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use crate::transcript::Transcript;

    #[test]
    fn saved_transcript_and_meeting_formats_still_load_and_round_trip() {
        let transcript = serde_json::json!({
            "schema_version": 1, "revision": 2, "created_at": "2026-10-12T10:00:00Z",
            "model": ["Groq", "Gemini", "turbo-q5_0"],
            "fallback_reason": "Groq daily limit reached", "language": "en",
            "language_detected": false, "duration_ms": 1000, "segments": []
        });
        let decoded: Transcript = serde_json::from_value(transcript.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), transcript);

        let meeting = serde_json::json!({
            "schema_version": 1, "title": "Planning", "started_at": "2026-10-12T09:00:00Z",
            "language": "en", "duration_ms": 1000, "audio": null, "recordings": [],
            "markers": [], "correction_notice": "Correction unavailable"
        });
        let decoded: MeetingFile = serde_json::from_value(meeting.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), meeting);

        let mut old = transcript;
        old["model"] = serde_json::json!("turbo-q5_0");
        old.as_object_mut().unwrap().remove("fallback_reason");
        let decoded: Transcript = serde_json::from_value(old).unwrap();
        assert_eq!(decoded.model, vec!["turbo-q5_0".into()]);
        assert_eq!(
            serde_json::to_value(decoded).unwrap()["model"],
            serde_json::json!(["turbo-q5_0"])
        );
    }
}
