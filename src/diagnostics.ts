import { invoke } from "@tauri-apps/api/core";

export const getDiagnostics = () => invoke<string>("get_diagnostics");

export type MicrophoneApp = { id: string; display_name: string };
export const getMicrophoneUsage = () => invoke<MicrophoneApp[]>("get_microphone_usage");
