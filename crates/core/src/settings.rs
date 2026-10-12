use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub recording_notice_acknowledged: bool,
    pub auto_process: bool,
    pub model: String,
    pub vocabulary: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recording_notice_acknowledged: false,
            auto_process: true,
            model: "small".into(),
            vocabulary: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let mut file = fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_settings_keep_notice_and_get_processing_defaults() {
        let settings: Settings =
            serde_json::from_str(r#"{"recording_notice_acknowledged":true}"#).unwrap();
        assert!(settings.recording_notice_acknowledged);
        assert!(settings.auto_process);
        assert_eq!(settings.model, "small");
        assert!(settings.vocabulary.is_empty());
    }
}
