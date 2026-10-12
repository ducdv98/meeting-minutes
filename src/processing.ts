import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Language = "en" | "vi";
export type Speaker = "me" | "remote" | "unknown";
export interface Segment { id: string; start_ms: number; end_ms: number; text: string; speaker: Speaker }
export interface Transcript { schema_version: number; revision: number; created_at: string; model: string; language: Language; language_detected: boolean; duration_ms: number; segments: Segment[] }
export interface ProcessingJob { meeting_id: string; model: string | null; stage: "queued" | "downloading" | "transcribing" | "failed"; progress: number; error: string | null }
export interface ProcessingStatus { jobs: ProcessingJob[] }
export function processingStageLabel(job: ProcessingJob): string {
  switch (job.stage) {
    case "queued": return "Waiting to transcribe";
    case "downloading": return `Downloading model ${Math.round(job.progress * 100)}%`;
    case "transcribing": return `Transcribing ${Math.round(job.progress * 100)}%`;
    case "failed": return "Transcription failed";
  }
}
export const MODELS = [
  { name: "base", size_mb: 148 },
  { name: "small", size_mb: 488 },
  { name: "medium", size_mb: 1534 },
  { name: "large-v3-turbo", size_mb: 1625 },
] as const;
export interface Settings { recording_notice_acknowledged: boolean; auto_process: boolean; model: string; vocabulary: string[] }

export const processingStatus = () => invoke<ProcessingStatus>("processing_status");
export const enqueueTranscription = (meetingId: string, model?: string) => invoke<ProcessingStatus>("transcribe_meeting", { meetingId, model: model ?? null });
export const cancelProcessing = (meetingId: string) => invoke<ProcessingStatus>("cancel_processing", { meetingId });
export const dismissProcessing = (meetingId: string) => invoke<ProcessingStatus>("dismiss_processing", { meetingId });
export const meetingTranscript = (meetingId: string) => invoke<Transcript | null>("meeting_transcript", { meetingId });
export const setMeetingLanguage = (meetingId: string, language: Language | null) => invoke<void>("set_meeting_language", { meetingId, language });
export const getSettings = () => invoke<Settings>("get_settings");
export const updateSettings = (autoProcess: boolean, model: string, vocabulary: string[]) => invoke<Settings>("update_settings", { autoProcess, model, vocabulary });
export const onProcessingStatus = (handler: (status: ProcessingStatus) => void): Promise<UnlistenFn> => listen<ProcessingStatus>("processing-status", (event) => handler(event.payload));
