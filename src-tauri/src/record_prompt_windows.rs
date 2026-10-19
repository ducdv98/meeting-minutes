use std::{sync::Mutex, thread, time::Duration};

use meeting_minutes_core::{
    diagnostics::count_record_prompt_shown,
    microphone_usage::{MicrophoneApp, MicrophoneUsage, SystemMicrophoneUsage},
    record_prompt::{PromptDecision, PromptSettings, RecordPromptDecider},
    recorder::Phase,
};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

use crate::status;

const LABEL: &str = "record-prompt";

#[derive(Default)]
pub struct PromptState(Mutex<PromptUiState>);

#[derive(Default)]
struct PromptUiState {
    decider: RecordPromptDecider,
    current: Option<MicrophoneApp>,
}

pub fn current(app: &AppHandle) -> Option<MicrophoneApp> {
    app.state::<PromptState>()
        .0
        .lock()
        .ok()
        .and_then(|state| state.current.clone())
}

pub fn dismiss(app: &AppHandle) {
    if let Ok(mut state) = app.state::<PromptState>().0.lock() {
        state.decider.dismiss();
        state.current = None;
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.close();
    }
}

fn show(app: &AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or("Main window is unavailable")?;
    let monitor = main
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or("No display is available for the Record prompt")?;
    let window = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("index.html#record-prompt".into()),
    )
    .title("Record prompt")
    .decorations(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .shadow(false)
    .visible(false)
    .inner_size(340.0, 154.0)
    .build()
    .map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        let size = window.outer_size().map_err(|error| error.to_string())?;
        window
            .set_position(PhysicalPosition::new(
                monitor.position().x + monitor.size().width as i32 - size.width as i32 - 24,
                monitor.position().y + monitor.size().height as i32 - size.height as i32 - 24,
            ))
            .map_err(|error| error.to_string())?;
        unsafe {
            SetWindowDisplayAffinity(
                window.hwnd().map_err(|error| error.to_string())?,
                WDA_EXCLUDEFROMCAPTURE,
            )
            .map_err(|error| {
                format!("Could not exclude the Record prompt from screen sharing: {error}")
            })?;
        }
        window.show().map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = window.close();
    }
    result
}

pub fn start_polling(app: AppHandle) {
    thread::spawn(move || loop {
        if let Ok(apps) = SystemMicrophoneUsage.apps_using_microphone() {
            let recording_active = status(&app)
                .map(|status| status.phase != Phase::Idle)
                .unwrap_or(true);
            let decision = {
                let state = app.state::<PromptState>();
                let Ok(mut ui) = state.0.lock() else {
                    thread::sleep(Duration::from_secs(2));
                    continue;
                };
                if ui.current.is_some() && app.get_webview_window(LABEL).is_none() {
                    ui.decider.dismiss();
                    ui.current = None;
                }
                ui.decider
                    .observe(&apps, recording_active, PromptSettings::default())
            };
            match decision {
                PromptDecision::Show(prompt) => {
                    if let Ok(mut ui) = app.state::<PromptState>().0.lock() {
                        ui.current = Some(prompt);
                    }
                    count_record_prompt_shown();
                    if let Err(error) = show(&app) {
                        dismiss(&app);
                        eprintln!("Could not show Record prompt: {error}");
                    }
                }
                PromptDecision::Close => dismiss(&app),
                PromptDecision::None => {}
            }
        }
        thread::sleep(Duration::from_secs(2));
    });
}
