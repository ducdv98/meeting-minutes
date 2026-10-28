#[cfg(target_os = "macos")]
use meeting_minutes_core::recorder::Phase;
use meeting_minutes_core::recorder::RecorderStatus;
use tauri::AppHandle;

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Mutex;

    use objc2::{rc::Retained, runtime::ProtocolObject};
    use objc2_foundation::{NSActivityOptions, NSObjectProtocol, NSProcessInfo, NSString};
    use tauri::Manager;

    use super::{AppHandle, Phase, RecorderStatus};

    pub struct KeepAwake(Mutex<Option<Activity>>);

    // NSProcessInfo accepts an activity token on any thread. The token is only
    // accessed while the mutex is held, and endActivity is called exactly once.
    unsafe impl Send for Activity {}

    struct Activity(Retained<ProtocolObject<dyn NSObjectProtocol>>);

    impl Drop for Activity {
        fn drop(&mut self) {
            // SAFETY: This is the token returned by beginActivityWithOptions_reason.
            unsafe { NSProcessInfo::processInfo().endActivity(&self.0) };
        }
    }

    impl Default for KeepAwake {
        fn default() -> Self {
            Self(Mutex::new(None))
        }
    }

    pub fn recording_changed(app: &AppHandle, status: &RecorderStatus) {
        let state = app.state::<KeepAwake>();
        let mut activity = state.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if matches!(status.phase, Phase::Recording | Phase::Paused) {
            if activity.is_none() {
                let options =
                    NSActivityOptions::UserInitiated | NSActivityOptions::IdleSystemSleepDisabled;
                let reason = NSString::from_str("Recording a Meeting");
                *activity = Some(Activity(
                    NSProcessInfo::processInfo().beginActivityWithOptions_reason(options, &reason),
                ));
            }
        } else {
            *activity = None;
        }
    }

    pub fn release(app: &AppHandle) {
        let state = app.state::<KeepAwake>();
        let mut activity = state.0.lock().unwrap_or_else(|poison| poison.into_inner());
        *activity = None;
    }
}

#[cfg(not(target_os = "macos"))]
mod other {
    use super::{AppHandle, RecorderStatus};

    #[derive(Default)]
    pub struct KeepAwake {
        _private: (),
    }

    pub fn recording_changed(_app: &AppHandle, _status: &RecorderStatus) {}

    pub fn release(_app: &AppHandle) {}
}

#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(not(target_os = "macos"))]
pub use other::*;
