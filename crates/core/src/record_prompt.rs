use std::collections::{HashSet, VecDeque};

use crate::microphone_usage::MicrophoneApp;

#[derive(Clone, Copy)]
pub struct PromptSettings<'a> {
    pub enabled: bool,
    pub silenced_app_ids: &'a [String],
}

impl Default for PromptSettings<'_> {
    fn default() -> Self {
        Self {
            enabled: true,
            silenced_app_ids: &[],
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PromptDecision {
    None,
    Show(MicrophoneApp),
    Close,
}

#[derive(Default)]
pub struct RecordPromptDecider {
    previous: Option<HashSet<String>>,
    open_app: Option<String>,
    pending: VecDeque<MicrophoneApp>,
}

impl RecordPromptDecider {
    pub fn observe(
        &mut self,
        apps: &[MicrophoneApp],
        recording_active: bool,
        settings: PromptSettings<'_>,
    ) -> PromptDecision {
        let current: HashSet<_> = apps.iter().map(|app| app.id.clone()).collect();
        let Some(previous) = self.previous.replace(current.clone()) else {
            return PromptDecision::None;
        };

        if recording_active || !settings.enabled {
            self.pending.clear();
        } else {
            self.pending.retain(|app| {
                current.contains(&app.id) && !settings.silenced_app_ids.contains(&app.id)
            });
            self.pending.extend(
                apps.iter()
                    .filter(|app| {
                        !previous.contains(&app.id)
                            && !settings.silenced_app_ids.contains(&app.id)
                            && self.open_app.as_ref() != Some(&app.id)
                    })
                    .cloned(),
            );
        }

        if self.open_app.as_ref().is_some_and(|id| {
            recording_active
                || !settings.enabled
                || !current.contains(id)
                || settings.silenced_app_ids.contains(id)
        }) {
            self.open_app = None;
            return PromptDecision::Close;
        }
        if recording_active || !settings.enabled || self.open_app.is_some() {
            return PromptDecision::None;
        }
        if let Some(app) = self.pending.pop_front() {
            self.open_app = Some(app.id.clone());
            return PromptDecision::Show(app);
        }
        PromptDecision::None
    }

    pub fn dismiss(&mut self) {
        self.open_app = None;
    }
}
