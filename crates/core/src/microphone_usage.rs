use std::io;

use serde::{Deserialize, Serialize};

/// One app that Windows currently reports as using the Microphone.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MicrophoneApp {
    /// Stable across checks and suitable for a user's silenced-app setting.
    pub id: String,
    pub display_name: String,
}

pub trait MicrophoneUsage {
    fn apps_using_microphone(&self) -> io::Result<Vec<MicrophoneApp>>;
}

pub struct SystemMicrophoneUsage;

#[cfg(not(windows))]
impl MicrophoneUsage for SystemMicrophoneUsage {
    fn apps_using_microphone(&self) -> io::Result<Vec<MicrophoneApp>> {
        Ok(Vec::new())
    }
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
impl MicrophoneUsage for SystemMicrophoneUsage {
    fn apps_using_microphone(&self) -> io::Result<Vec<MicrophoneApp>> {
        windows::apps_using_microphone()
    }
}
