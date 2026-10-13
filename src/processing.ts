import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AiSettings } from "./ai";

export type Language = "en" | "vi";
export type Speaker = "me" | "remote" | "unknown";
export interface Segment { id: string; start_ms: number; end_ms: number; text: string; speaker: Speaker }
export type DraftSegment = Omit<Segment, "id">;
export interface TranscriptDraftEvent { meeting_id: string; segments: DraftSegment[] }
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
  { name: "base", label: "Base", size_mb: 148, description: "Fastest. Fine for clear English." },
  { name: "small", label: "Small", size_mb: 488, description: "Good balance for English and Vietnamese." },
  { name: "large-v3-turbo-q5_0", label: "Large v3 turbo", size_mb: 574, description: "Most accurate. Runs best with a GPU." },
] as const;
export function modelSizeLabel(sizeMb: number) {
  return sizeMb < 1000 ? `${sizeMb} MB` : `${(sizeMb / 1000).toFixed(1)} GB`;
}
export interface Settings { recording_notice_acknowledged: boolean; auto_process: boolean; model: string; vocabulary: string[]; language: Language | null; your_name: string | null; ai: AiSettings }

export const processingStatus = () => invoke<ProcessingStatus>("processing_status");
export const enqueueTranscription = (meetingId: string, model?: string) => invoke<ProcessingStatus>("transcribe_meeting", { meetingId, model: model ?? null });
export const cancelProcessing = (meetingId: string) => invoke<ProcessingStatus>("cancel_processing", { meetingId });
export const dismissProcessing = (meetingId: string) => invoke<ProcessingStatus>("dismiss_processing", { meetingId });
export const meetingTranscript = (meetingId: string) => invoke<Transcript | null>("meeting_transcript", { meetingId });
export const transcriptDraft = (meetingId: string) => invoke<DraftSegment[]>("transcript_draft", { meetingId });
export const setMeetingLanguage = (meetingId: string, language: Language | null) => invoke<void>("set_meeting_language", { meetingId, language });
export const getSettings = () => invoke<Settings>("get_settings");
export const updateSettings = (autoProcess: boolean, model: string, vocabulary: string[], language: Language | null, yourName: string | null) => invoke<Settings>("update_settings", { autoProcess, model, vocabulary, language, yourName });
export const installedModels = () => invoke<string[]>("installed_models");
export const onProcessingStatus = (handler: (status: ProcessingStatus) => void): Promise<UnlistenFn> => listen<ProcessingStatus>("processing-status", (event) => handler(event.payload));
export const onTranscriptDraft = (handler: (draft: TranscriptDraftEvent) => void): Promise<UnlistenFn> => listen<TranscriptDraftEvent>("transcript-draft", (event) => handler(event.payload));
