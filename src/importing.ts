import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface ImportStatus {
  current: string | null;
  queued: number;
  errors: string[];
}

export function pickAudioFiles(): Promise<ImportStatus> {
  return invoke<ImportStatus>("pick_audio_files");
}

export function importStatus(): Promise<ImportStatus> {
  return invoke<ImportStatus>("import_status");
}

export function dismissImportErrors(): Promise<ImportStatus> {
  return invoke<ImportStatus>("dismiss_import_errors");
}

export function onImportStatus(handler: (status: ImportStatus) => void): Promise<UnlistenFn> {
  return listen<ImportStatus>("import-status", (event) => handler(event.payload));
}

export function onImportHover(handler: (hover: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>("import-hover", (event) => handler(event.payload));
}
