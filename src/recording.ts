import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface PendingSave {
  meeting_id: string;
  state: "saving" | "failed";
  error: string | null;
  /** Recovered from Chunks left by a crash, not from a Stop. */
  recovered: boolean;
}

export interface DeviceSwitch {
  at_ms: number;
  source: "microphone" | "system_audio";
  device: string;
}

export interface RecorderStatus {
  phase: "idle" | "recording" | "paused";
  meeting_id: string | null;
  elapsed_ms: number;
  notice_acknowledged: boolean;
  saves: PendingSave[];
  /** Meetings recovered after a crash that can be continued with a new Recording. */
  continue_offers: string[];
  device_switches: DeviceSwitch[];
  /** Device names reported by the active capture workers. */
  source_devices?: { source: "microphone" | "system_audio"; device: string }[];
  source_levels?: { source: "microphone" | "system_audio"; level: number; silent: boolean }[];
  source_health?: { source: "microphone" | "system_audio"; warning: "no_signal" | null }[];
}

export const recordHotkey = "Ctrl+Alt+Shift+R";
export const pauseHotkey = "Ctrl+Alt+Shift+P";

export function recordingStatus(): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("recording_status");
}

export function startRecording(): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("start_recording");
}

export function pauseRecording(): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("pause_recording");
}

export function resumeRecording(): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("resume_recording");
}

export function stopRecording(): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("stop_recording");
}

export function takeSnapshot(drawNew = false): Promise<void> {
  return invoke("take_snapshot", { drawNew });
}

export function retrySave(meetingId: string): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("retry_save", { meetingId });
}

export function continueMeeting(meetingId: string): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("continue_meeting", { meetingId });
}

export function dismissContinue(meetingId: string): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("dismiss_continue", { meetingId });
}

export function dismissSourceWarning(source: "microphone" | "system_audio"): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("dismiss_source_warning", { source });
}

export function openSoundSettings(): Promise<void> {
  return invoke("open_sound_settings");
}

export function acknowledgeRecordingNotice(): Promise<RecorderStatus> {
  return invoke<RecorderStatus>("acknowledge_recording_notice");
}

export function onRecordingStatus(handler: (status: RecorderStatus) => void): Promise<UnlistenFn> {
  return listen<RecorderStatus>("recording-status", (event) => handler(event.payload));
}

/** The tray or hotkey asked to record before the Recording notice was ever shown. */
export function onRecordingNoticeRequested(handler: () => void): Promise<UnlistenFn> {
  return listen("recording-notice-requested", () => handler());
}

export function onLibraryChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("library-changed", () => handler());
}

export async function onSnapshotError(handler: (error: string) => void): Promise<UnlistenFn> {
  const [stopCapture, stopHotkey] = await Promise.all([
    listen<string>("snapshot-error", (event) => handler(event.payload)),
    listen<string>("snapshot-hotkey-error", (event) => handler(event.payload)),
  ]);
  return () => { stopCapture(); stopHotkey(); };
}

export function onSnapshotHotkeyError(handler: (error: string) => void): Promise<UnlistenFn> {
  return listen<string>("snapshot-hotkey-error", (event) => handler(event.payload));
}

export function formatElapsed(ms: number): string {
  const total = Math.floor(ms / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  return [hours, minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
}
