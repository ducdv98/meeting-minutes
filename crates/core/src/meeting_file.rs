use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeetingSummary {
    pub id: String,
    pub title: String,
    pub started_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MeetingFile {
    pub schema_version: u32,
    pub title: String,
    pub started_at: String,
    pub language: Option<String>,
    pub recordings: Vec<Value>,
    pub markers: Vec<Value>,
}

impl MeetingFile {
    pub(crate) fn new(title: &str, started_at: String) -> Self {
        Self {
            schema_version: 1,
            title: title.to_owned(),
            started_at,
            language: None,
            recordings: Vec::new(),
            markers: Vec::new(),
        }
    }
}
