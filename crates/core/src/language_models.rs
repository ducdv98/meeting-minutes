use serde::{Deserialize, Serialize};

use crate::{
    ai::ProviderKind,
    settings::{AiSettings, DEFAULT_GEMINI_MODEL},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageModel {
    pub provider: ProviderKind,
    pub model: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguageModelSettings {
    pub models: Vec<LanguageModel>,
    pub correction: Option<LanguageModel>,
    pub summary: Option<LanguageModel>,
    pub ask: Option<LanguageModel>,
    pub confirmed_correction_providers: Vec<ProviderKind>,
    /// Preserves the old automatic Gemini Correction until the key store is checked.
    pub legacy_correction_pending: bool,
}

impl LanguageModelSettings {
    pub fn from_legacy(ai: &AiSettings) -> Self {
        let mut settings = Self {
            legacy_correction_pending: true,
            ..Self::default()
        };
        if let Some(provider) = ai.provider.filter(|kind| kind.supports_text()) {
            if let Some(model) = ai
                .model_for(provider)
                .filter(|model| !model.trim().is_empty())
            {
                let selected = LanguageModel {
                    provider,
                    model: model.into(),
                };
                settings.models.push(selected.clone());
                settings.summary = Some(selected.clone());
                settings.ask = Some(selected);
            }
        }
        settings
    }

    pub fn finish_legacy_correction(&mut self, has_gemini_key: bool, ai: &AiSettings) {
        if !self.legacy_correction_pending {
            return;
        }
        if has_gemini_key {
            let selected = LanguageModel {
                provider: ProviderKind::Gemini,
                model: ai
                    .model_for(ProviderKind::Gemini)
                    .unwrap_or(DEFAULT_GEMINI_MODEL)
                    .into(),
            };
            if !self.models.contains(&selected) {
                self.models.push(selected.clone());
            }
            self.correction = Some(selected);
            self.confirmed_correction_providers
                .push(ProviderKind::Gemini);
        }
        self.legacy_correction_pending = false;
    }

    pub fn valid(&self) -> bool {
        self.models
            .iter()
            .all(|item| item.provider.supports_text() && !item.model.trim().is_empty())
            && [
                self.correction.as_ref(),
                self.summary.as_ref(),
                self.ask.as_ref(),
            ]
            .into_iter()
            .flatten()
            .all(|item| self.models.contains(item))
            && self.correction.as_ref().is_none_or(|item| {
                !item.provider.is_cloud()
                    || self.confirmed_correction_providers.contains(&item.provider)
            })
    }
}
