use crate::{models::DEFAULT_MODEL, settings::DEFAULT_GEMINI_MODEL};
use serde::{Deserialize, Serialize};

pub const DEFAULT_GROQ_MODEL: &str = "whisper-large-v3";
pub const GROQ_MODELS: &[&str] = &[DEFAULT_GROQ_MODEL, "whisper-large-v3-turbo"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechProvider {
    Groq,
    Gemini,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechModel {
    pub provider: SpeechProvider,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpeechSettings {
    pub models: Vec<SpeechModel>,
    pub automatic_groq: Option<String>,
    pub automatic_gemini: Option<String>,
    pub automatic_whisper: String,
}

impl Default for SpeechSettings {
    fn default() -> Self {
        Self::migrate(DEFAULT_MODEL, None)
    }
}

impl SpeechSettings {
    pub fn migrate(whisper: &str, gemini: Option<&str>) -> Self {
        let gemini = gemini.unwrap_or(DEFAULT_GEMINI_MODEL).to_owned();
        Self {
            models: vec![
                SpeechModel {
                    provider: SpeechProvider::Groq,
                    model: DEFAULT_GROQ_MODEL.into(),
                },
                SpeechModel {
                    provider: SpeechProvider::Gemini,
                    model: gemini.clone(),
                },
            ],
            automatic_groq: Some(DEFAULT_GROQ_MODEL.into()),
            automatic_gemini: Some(gemini),
            automatic_whisper: whisper.into(),
        }
    }

    pub fn automatic(&self, provider: SpeechProvider) -> Option<&str> {
        let selected = match provider {
            SpeechProvider::Groq => self.automatic_groq.as_deref(),
            SpeechProvider::Gemini => self.automatic_gemini.as_deref(),
        }?;
        self.models
            .iter()
            .any(|entry| entry.provider == provider && entry.model == selected)
            .then_some(selected)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !crate::models::MODELS
            .iter()
            .any(|model| model.name == self.automatic_whisper)
        {
            return Err("Choose a built-in local Whisper model".into());
        }
        for entry in &self.models {
            if entry.model.trim().is_empty() || entry.model != entry.model.trim() {
                return Err("Enter a Speech-to-text model ID".into());
            }
            if entry.provider == SpeechProvider::Groq
                && !GROQ_MODELS.contains(&entry.model.as_str())
            {
                return Err("Choose a supported Groq Speech-to-text model".into());
            }
        }
        if self.automatic_groq.is_some() && self.automatic(SpeechProvider::Groq).is_none() {
            return Err("Choose an added Groq model for automatic transcription".into());
        }
        if self.automatic_gemini.is_some() && self.automatic(SpeechProvider::Gemini).is_none() {
            return Err("Choose an added Gemini model for automatic transcription".into());
        }
        Ok(())
    }
}
