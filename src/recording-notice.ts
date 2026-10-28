import type { PlatformCapabilities } from "./platform-capabilities";

export function recordingNotice(capabilities: PlatformCapabilities): string {
  return [
    "Other people on the call may need to agree to being recorded. Let them know before you start.",
    capabilities.recordPrompt && "To offer a Recording, the app checks locally which apps use the Microphone; nothing about that check is sent anywhere. The offer never starts a Recording by itself.",
    capabilities.snapshots && "Snapshots may capture what others share. Each Snapshot is confirmed on screen when taken and stays on this PC.",
    "While a Recording runs, the window and the tray icon show it.",
  ].filter(Boolean).join(" ");
}
