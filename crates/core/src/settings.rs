use crate::{ai::ProviderKind, models::TURBO_MODEL, transcript::Language};
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
    #[serde(default)]
    pub cloud_notice_acknowledged: bool,
    pub auto_process: bool,
    pub model: String,
    pub vocabulary: Vec<String>,
    #[serde(default)]
    pub your_name: Option<String>,
    /// Default Meeting language; `None` detects it.
    pub language: Option<Language>,
    pub ai: AiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recording_notice_acknowledged: false,
            cloud_notice_acknowledged: false,
            auto_process: true,
            model: "small".into(),
            vocabulary: Vec::new(),
            your_name: None,
            language: None,
            ai: AiSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub provider: Option<ProviderKind>,
    pub ollama_model: Option<String>,
    pub openai_model: Option<String>,
    pub anthropic_model: Option<String>,
    pub gemini_model: Option<String>,
}

impl AiSettings {
    pub fn model_for(&self, kind: ProviderKind) -> Option<&str> {
        match kind {
            ProviderKind::Ollama => self.ollama_model.as_deref(),
            ProviderKind::Openai => self.openai_model.as_deref(),
            ProviderKind::Anthropic => {
                Some(self.anthropic_model.as_deref().unwrap_or("claude-opus-5-5"))
            }
            ProviderKind::Gemini => Some(self.gemini_model.as_deref().unwrap_or("gemini-2.5-flash")),
            ProviderKind::Groq => None,
        }
    }
    pub fn set_model(&mut self, kind: ProviderKind, model: String) {
        let field = match kind {
            ProviderKind::Ollama => &mut self.ollama_model,
            ProviderKind::Openai => &mut self.openai_model,
            ProviderKind::Anthropic => &mut self.anthropic_model,
            ProviderKind::Gemini => &mut self.gemini_model,
            ProviderKind::Groq => return,
        };
        *field = Some(model);
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let mut settings: Self = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if matches!(settings.model.as_str(), "medium" | "large-v3-turbo") {
            settings.model = TURBO_MODEL.into();
        }
        settings
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
    fn legacy_model_settings_load_as_quantized_turbo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for legacy in ["medium", "large-v3-turbo"] {
            fs::write(
                &path,
                format!(r#"{{"model":"{legacy}","auto_process":false}}"#),
            )
            .unwrap();
            let settings = Settings::load(&path);
            assert_eq!(settings.model, "large-v3-turbo-q5_0");
            assert!(!settings.auto_process);
        }
    }

    #[test]
    fn old_settings_keep_notice_and_get_processing_defaults() {
        let settings: Settings =
            serde_json::from_str(r#"{"recording_notice_acknowledged":true}"#).unwrap();
        assert!(settings.recording_notice_acknowledged);
        assert!(settings.auto_process);
        assert_eq!(settings.model, "small");
        assert!(settings.vocabulary.is_empty());
        assert_eq!(settings.language, None);
    }

    #[test]
    fn pre_cloud_settings_load_without_changing_existing_values() {
        let settings: Settings = serde_json::from_str(r#"{"recording_notice_acknowledged":true,"auto_process":false,"model":"small","vocabulary":["Lan"],"your_name":"Minh","language":"vi","ai":{"provider":"openai","openai_model":"gpt-4o"}}"#).unwrap();
        assert!(settings.recording_notice_acknowledged);
        assert!(!settings.cloud_notice_acknowledged);
        assert!(!settings.auto_process);
        assert_eq!(settings.vocabulary, ["Lan"]);
        assert_eq!(settings.your_name.as_deref(), Some("Minh"));
        assert_eq!(settings.language, Some(Language::Vi));
        assert_eq!(settings.ai.provider, Some(ProviderKind::Openai));
        assert_eq!(settings.ai.openai_model.as_deref(), Some("gpt-4o"));
        let json = serde_json::to_string(&settings).unwrap();
        assert!(!json.contains("secret"));
    }
}
