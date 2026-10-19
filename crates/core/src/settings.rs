use crate::{
    ai::ProviderKind, language_models::LanguageModelSettings, microphone_usage::MicrophoneApp,
    models::TURBO_MODEL, speech_models::SpeechSettings, transcript::Language,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";
pub const DEFAULT_SNAPSHOT_HOTKEY: &str = "Ctrl+Alt+S";
pub const DEFAULT_DRAW_SNAPSHOT_HOTKEY: &str = "Ctrl+Alt+Shift+S";

fn default_snapshot_hotkey() -> String {
    DEFAULT_SNAPSHOT_HOTKEY.into()
}

fn default_draw_snapshot_hotkey() -> String {
    DEFAULT_DRAW_SNAPSHOT_HOTKEY.into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub first_run_completed: bool,
    pub recording_notice_acknowledged: bool,
    pub record_prompt_enabled: bool,
    pub silenced_record_prompt_apps: Vec<MicrophoneApp>,
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
    pub speech: SpeechSettings,
    pub language_models: LanguageModelSettings,
    #[serde(default = "default_snapshot_hotkey")]
    pub snapshot_hotkey: String,
    #[serde(default = "default_draw_snapshot_hotkey")]
    pub draw_snapshot_hotkey: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            first_run_completed: false,
            recording_notice_acknowledged: false,
            record_prompt_enabled: true,
            silenced_record_prompt_apps: Vec::new(),
            cloud_notice_acknowledged: false,
            auto_process: true,
            model: "small".into(),
            vocabulary: Vec::new(),
            your_name: None,
            language: None,
            ai: AiSettings::default(),
            speech: SpeechSettings::default(),
            language_models: LanguageModelSettings::default(),
            snapshot_hotkey: default_snapshot_hotkey(),
            draw_snapshot_hotkey: default_draw_snapshot_hotkey(),
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
            ProviderKind::Gemini => {
                Some(self.gemini_model.as_deref().unwrap_or(DEFAULT_GEMINI_MODEL))
            }
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
        let value = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let legacy = value
            .as_ref()
            .is_some_and(|value| value.get("language_models").is_none());
        let legacy_speech = value
            .as_ref()
            .is_some_and(|value| value.get("speech").is_none());
        let past_first_run = path.is_file()
            && value
                .as_ref()
                .is_none_or(|value| value.get("first_run_completed").is_none());
        let mut settings: Self = value
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        if past_first_run {
            settings.first_run_completed = true;
        }
        if legacy {
            settings.language_models = LanguageModelSettings::from_legacy(&settings.ai);
        }
        if matches!(settings.model.as_str(), "medium" | "large-v3-turbo") {
            settings.model = TURBO_MODEL.into();
        }
        if legacy_speech {
            settings.speech =
                SpeechSettings::migrate(&settings.model, settings.ai.gemini_model.as_deref());
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
    fn existing_settings_skip_first_run_but_new_settings_do_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert!(!Settings::load(&path).first_run_completed);
        fs::write(&path, r#"{"model":"small"}"#).unwrap();
        assert!(Settings::load(&path).first_run_completed);
        fs::write(&path, r#"{"first_run_completed":false}"#).unwrap();
        assert!(!Settings::load(&path).first_run_completed);
        fs::write(&path, b"invalid").unwrap();
        assert!(Settings::load(&path).first_run_completed);
    }

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
        assert!(settings.record_prompt_enabled);
        assert!(settings.silenced_record_prompt_apps.is_empty());
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

    #[test]
    fn old_format_settings_file_migrates_speech_models_and_keeps_behavior() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"model":"base","ai":{"gemini_model":"gemini-2.5-flash-lite"},"auto_process":false}"#).unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.speech.automatic_whisper, "base");
        assert_eq!(
            settings
                .speech
                .automatic(crate::speech_models::SpeechProvider::Groq),
            Some("whisper-large-v3")
        );
        assert_eq!(
            settings
                .speech
                .automatic(crate::speech_models::SpeechProvider::Gemini),
            Some("gemini-2.5-flash-lite")
        );
        assert!(!settings.auto_process);
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
    }

    #[test]
    fn old_ai_settings_migrate_summary_ask_and_gemini_correction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"ai":{"provider":"anthropic","anthropic_model":"claude-test","gemini_model":"gemini-test"}}"#).unwrap();
        let mut settings = Settings::load(&path);
        let summary = settings.language_models.summary.as_ref().unwrap();
        assert_eq!(summary.provider, ProviderKind::Anthropic);
        assert_eq!(summary.model, "claude-test");
        assert_eq!(settings.language_models.ask.as_ref(), Some(summary));
        assert!(settings.language_models.legacy_correction_pending);
        settings
            .language_models
            .finish_legacy_correction(true, &settings.ai);
        assert_eq!(
            settings.language_models.correction.as_ref().unwrap().model,
            "gemini-test"
        );
        assert!(settings
            .language_models
            .confirmed_correction_providers
            .contains(&ProviderKind::Gemini));
        settings.save(&path).unwrap();
        let reloaded = Settings::load(&path);
        assert_eq!(reloaded.language_models, settings.language_models);
    }

    #[test]
    fn old_ai_settings_without_gemini_key_leave_correction_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"ai":{"provider":"ollama","ollama_model":"llama3.2"}}"#,
        )
        .unwrap();
        let mut settings = Settings::load(&path);
        settings
            .language_models
            .finish_legacy_correction(false, &settings.ai);
        assert!(settings.language_models.correction.is_none());
        assert_eq!(
            settings.language_models.summary.as_ref().unwrap().model,
            "llama3.2"
        );
    }
}
