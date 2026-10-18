//! Windows-only Snapshot adapter. Coordinates at this boundary are physical desktop pixels.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};

use meeting_minutes_core::{
    recorder::{Phase, RecorderStatus},
    snapshot::{CaptureRegion, RgbaFrame, ScreenGrabber, Snapshot},
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use windows::Win32::{
    Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
        HGDIOBJ, SRCCOPY,
    },
    UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE},
};

use crate::lock_recorder;

const REPEAT_HOTKEY: &str = "ctrl+alt+s";
const DRAW_HOTKEY: &str = "ctrl+alt+shift+s";
const OVERLAY_PREFIX: &str = "capture-region-";
const CONFIRM_PREFIX: &str = "snapshot-confirmation-";
static OVERLAY_NUMBER: AtomicU64 = AtomicU64::new(0);
static CONFIRM_NUMBER: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub struct SnapshotState(Mutex<SnapshotUiState>);

#[derive(Default)]
struct SnapshotUiState {
    region: Option<CaptureRegion>,
    selecting: bool,
    recording_id: Option<String>,
    hotkeys_registered: bool,
}

pub struct GdiGrabber;

#[derive(Clone, serde::Serialize)]
struct SnapshotTakenEvent {
    meeting_id: String,
    order: u32,
    timeline_ms: u64,
}

impl ScreenGrabber for GdiGrabber {
    fn grab(&self, region: CaptureRegion) -> Result<RgbaFrame, String> {
        if region.width == 0 || region.height == 0 {
            return Err("Select a nonempty Capture region".into());
        }
        let width = i32::try_from(region.width).map_err(|_| "Capture region is too wide")?;
        let height = i32::try_from(region.height).map_err(|_| "Capture region is too tall")?;
        let bytes = usize::try_from(region.width)
            .ok()
            .and_then(|w| w.checked_mul(region.height as usize))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("Capture region is too large")?;
        unsafe {
            let screen = GetDC(None);
            if screen.0.is_null() {
                return Err("GetDC failed".into());
            }
            let memory = CreateCompatibleDC(Some(screen));
            if memory.0.is_null() {
                ReleaseDC(None, screen);
                return Err("CreateCompatibleDC failed".into());
            }
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            if bitmap.0.is_null() {
                let _ = DeleteDC(memory);
                ReleaseDC(None, screen);
                return Err("CreateCompatibleBitmap failed".into());
            }
            let old = SelectObject(memory, HGDIOBJ(bitmap.0));
            let result = (|| -> Result<RgbaFrame, String> {
                if old.0.is_null() {
                    return Err("SelectObject failed".into());
                }
                BitBlt(
                    memory,
                    0,
                    0,
                    width,
                    height,
                    Some(screen),
                    region.x,
                    region.y,
                    windows::Win32::Graphics::Gdi::ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0),
                )
                .map_err(|error| error.to_string())?;
                // GetDIBits requires the bitmap to be deselected from the memory DC.
                SelectObject(memory, old);
                let mut info = BITMAPINFO::default();
                info.bmiHeader.biSize = std::mem::size_of_val(&info.bmiHeader) as u32;
                info.bmiHeader.biWidth = width;
                info.bmiHeader.biHeight = -height; // top-down native pixels
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB.0;
                let mut bgra = vec![0u8; bytes];
                if GetDIBits(
                    memory,
                    bitmap,
                    0,
                    region.height,
                    Some(bgra.as_mut_ptr().cast()),
                    &mut info,
                    DIB_RGB_COLORS,
                ) != height
                {
                    return Err("GetDIBits failed".into());
                }
                let (pixels, _) = bgra.as_chunks_mut::<4>();
                for pixel in pixels {
                    pixel.swap(0, 2);
                    pixel[3] = 255;
                }
                Ok(RgbaFrame {
                    width: region.width,
                    height: region.height,
                    pixels: bgra,
                })
            })();
            if !old.0.is_null() {
                SelectObject(memory, old);
            }
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory);
            ReleaseDC(None, screen);
            result
        }
    }
}

fn exclude(window: &tauri::WebviewWindow) -> Result<(), String> {
    unsafe {
        SetWindowDisplayAffinity(
            window.hwnd().map_err(|error| error.to_string())?,
            WDA_EXCLUDEFROMCAPTURE,
        )
        .map_err(|error| {
            format!("Could not exclude the Snapshot window from screen sharing: {error}")
        })
    }
}

fn close_overlays(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) {
            let _ = window.close();
        }
    }
}

fn show_overlay(app: &AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or("Main window is unavailable")?;
    let monitors = main
        .available_monitors()
        .map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Err("No display is available for Capture region selection".into());
    }
    let request_number = OVERLAY_NUMBER.fetch_add(1, Ordering::Relaxed);
    for (index, monitor) in monitors.iter().enumerate() {
        let label = format!("{OVERLAY_PREFIX}{request_number}-{index}");
        let window = WebviewWindowBuilder::new(
            app,
            &label,
            WebviewUrl::App("index.html#capture-region".into()),
        )
        .title("Draw Capture region")
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?;
        let result = (|| -> Result<(), String> {
            window
                .set_position(PhysicalPosition::new(
                    monitor.position().x,
                    monitor.position().y,
                ))
                .map_err(|error| error.to_string())?;
            window
                .set_size(PhysicalSize::new(
                    monitor.size().width,
                    monitor.size().height,
                ))
                .map_err(|error| error.to_string())?;
            exclude(&window)?;
            window.show().map_err(|error| error.to_string())?;
            Ok(())
        })();
        if let Err(error) = result {
            close_overlays(app);
            return Err(error);
        }
    }
    Ok(())
}

fn show_confirmation(app: &AppHandle) -> Result<(), String> {
    for (label, old) in app.webview_windows() {
        if label.starts_with(CONFIRM_PREFIX) {
            let _ = old.close();
        }
    }
    let main = app
        .get_webview_window("main")
        .ok_or("Main window is unavailable")?;
    let monitor = main
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or("No display is available")?;
    let label = format!(
        "{CONFIRM_PREFIX}{}",
        CONFIRM_NUMBER.fetch_add(1, Ordering::Relaxed)
    );
    let window = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::App("index.html#snapshot-confirmation".into()),
    )
    .title("Snapshot saved")
    .decorations(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focusable(false)
    .transparent(true)
    .visible(false)
    .inner_size(220.0, 54.0)
    .build()
    .map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        let size = window.outer_size().map_err(|error| error.to_string())?;
        window
            .set_position(PhysicalPosition::new(
                monitor.position().x + (monitor.size().width as i32 - size.width as i32) / 2,
                monitor.position().y + 32,
            ))
            .map_err(|error| error.to_string())?;
        exclude(&window)?;
        window.show().map_err(|error| error.to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = window.close();
        return Err(error);
    }
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(1500));
        let _ = window.close();
    });
    Ok(())
}

fn save(app: &AppHandle, region: CaptureRegion) -> Result<Snapshot, String> {
    let recorder = lock_recorder(app)?;
    let meeting_id = recorder
        .active_job()
        .ok_or("A Recording must be running to take a Snapshot")?
        .meeting_id
        .clone();
    let snapshot = recorder
        .take_snapshot(region, &GdiGrabber)
        .map_err(|error| error.to_string())?;
    drop(recorder);
    let _ = app.emit(
        "snapshot-taken",
        SnapshotTakenEvent {
            meeting_id,
            order: snapshot.order,
            timeline_ms: snapshot.timeline_ms,
        },
    );
    let _ = app.emit("library-changed", ());
    if let Err(error) = show_confirmation(app) {
        let _ = app.emit(
            "snapshot-error",
            format!("Snapshot saved, but confirmation could not be shown: {error}"),
        );
    }
    Ok(snapshot)
}

pub fn request(app: &AppHandle, draw_new: bool) -> Result<(), String> {
    let recording_id = lock_recorder(app)?
        .status()
        .meeting_id
        .ok_or("A Recording must be running to take a Snapshot")?;
    let region = {
        let state = app.state::<SnapshotState>();
        let mut ui = state.0.lock().map_err(|error| error.to_string())?;
        if ui.selecting {
            return Ok(());
        }
        if ui.recording_id.as_deref() != Some(recording_id.as_str()) {
            ui.region = None;
            ui.recording_id = Some(recording_id);
        }
        if draw_new || ui.region.is_none() {
            ui.selecting = true;
            None
        } else {
            ui.region
        }
    };
    if let Some(region) = region {
        save(app, region)?;
    } else if let Err(error) = show_overlay(app) {
        if let Ok(mut ui) = app.state::<SnapshotState>().0.lock() {
            ui.selecting = false;
        }
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn take_snapshot(app: AppHandle, draw_new: bool) -> Result<(), String> {
    request(&app, draw_new)
}

#[tauri::command]
pub fn cancel_capture_region(app: AppHandle) {
    if let Ok(mut ui) = app.state::<SnapshotState>().0.lock() {
        ui.selecting = false;
    }
    close_overlays(&app);
}

pub fn overlay_closed(app: &AppHandle) {
    let selecting = if let Ok(mut ui) = app.state::<SnapshotState>().0.lock() {
        let selecting = ui.selecting;
        ui.selecting = false;
        selecting
    } else {
        false
    };
    if selecting {
        close_overlays(app);
    }
}

#[tauri::command]
pub fn select_capture_region(
    app: AppHandle,
    window: tauri::WebviewWindow,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if !window.label().starts_with(OVERLAY_PREFIX) {
        return Err("Capture region must come from the region overlay".into());
    }
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let origin = window.outer_position().map_err(|error| error.to_string())?;
    let bounds = window.inner_size().map_err(|error| error.to_string())?;
    if ![x, y, width, height].iter().all(|value| value.is_finite()) {
        return Err("Capture region coordinates are invalid".into());
    }
    let (left, top, pixel_width, pixel_height) = (
        (x * scale).round(),
        (y * scale).round(),
        (width * scale).round(),
        (height * scale).round(),
    );
    if left < 0.0
        || top < 0.0
        || pixel_width < 1.0
        || pixel_height < 1.0
        || left + pixel_width > f64::from(bounds.width)
        || top + pixel_height > f64::from(bounds.height)
    {
        return Err("Capture region must stay within one display".into());
    }
    let region = CaptureRegion {
        x: origin
            .x
            .checked_add(left as i32)
            .ok_or("Capture region is out of range")?,
        y: origin
            .y
            .checked_add(top as i32)
            .ok_or("Capture region is out of range")?,
        width: pixel_width as u32,
        height: pixel_height as u32,
    };
    let recording_id = lock_recorder(&app)?
        .status()
        .meeting_id
        .ok_or("A Recording must be running to take a Snapshot")?;
    {
        let state = app.state::<SnapshotState>();
        let mut ui = state.0.lock().map_err(|error| error.to_string())?;
        if !ui.selecting || ui.recording_id.as_deref() != Some(recording_id.as_str()) {
            return Err("Capture region selection has expired".into());
        }
        ui.selecting = false;
        ui.region = Some(region);
    }
    close_overlays(&app);
    if let Err(error) = save(&app, region) {
        let _ = app.emit("snapshot-error", error.clone());
        return Err(error);
    }
    Ok(())
}

pub fn recording_changed(app: &AppHandle, status: &RecorderStatus) {
    let state = app.state::<SnapshotState>();
    let mut ui = match state.0.lock() {
        Ok(ui) => ui,
        Err(_) => return,
    };
    if status.phase == Phase::Idle {
        ui.region = None;
        ui.recording_id = None;
        ui.selecting = false;
        let had_hotkeys = ui.hotkeys_registered;
        ui.hotkeys_registered = false;
        drop(ui);
        close_overlays(app);
        if had_hotkeys {
            let _ = app.global_shortcut().unregister(REPEAT_HOTKEY);
            let _ = app.global_shortcut().unregister(DRAW_HOTKEY);
        }
        return;
    }
    if ui.recording_id.as_deref() != status.meeting_id.as_deref() {
        ui.region = None;
        ui.recording_id = status.meeting_id.clone();
    }
    if ui.hotkeys_registered {
        return;
    }
    for (shortcut, draw_new) in [(REPEAT_HOTKEY, false), (DRAW_HOTKEY, true)] {
        if let Err(error) = app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, _, event| {
                if event.state == ShortcutState::Pressed {
                    if let Err(error) = request(app, draw_new) {
                        let _ = app.emit("snapshot-error", error);
                    }
                }
            })
        {
            let _ = app.global_shortcut().unregister(REPEAT_HOTKEY);
            let _ = app.global_shortcut().unregister(DRAW_HOTKEY);
            let _ = app.emit(
                "snapshot-hotkey-error",
                format!("Could not register {shortcut}: {error}"),
            );
            return;
        }
    }
    ui.hotkeys_registered = true;
}
