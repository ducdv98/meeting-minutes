import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AiSettings, LanguageModelSettings } from "./ai";
import type { Language } from "./languages";

export type { Language } from "./languages";
export type Speaker = "me" | "remote" | "unknown";
export interface Segment { id: string; start_ms: number; end_ms: number; text: string; speaker: Speaker }
export type DraftSegment = Omit<Segment, "id">;
export interface TranscriptDraftEvent { meeting_id: string; segments: DraftSegment[] }
export interface Transcript { schema_version: number; revision: number; created_at: string; model: string[]; fallback_reason?: string | null; language: Language; language_detected: boolean; duration_ms: number; segments: Segment[] }
export interface ProcessingJob { meeting_id: string; model: string | null; stage: "queued" | "downloading" | "transcribing" | "failed"; progress: number; error: string | null }
export interface ProcessingStatus { jobs: ProcessingJob[] }
export function processingStageLabel(job: ProcessingJob): string {
  switch (job.stage) {
    case "queued": return "Waiting to transcribe";
    case "downloading": return `Waiting for model download (${Math.round(job.progress * 100)}%)`;
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
export type SpeechProvider = "groq" | "gemini";
export interface SpeechModel { provider: SpeechProvider; model: string }
export interface SpeechSettings { models: SpeechModel[]; automatic_groq: string | null; automatic_gemini: string | null; automatic_whisper: string }
export const GROQ_SPEECH_MODELS = ["whisper-large-v3", "whisper-large-v3-turbo"] as const;
export interface Settings { recording_notice_acknowledged: boolean; cloud_notice_acknowledged: boolean; auto_process: boolean; model: string; vocabulary: string[]; language: Language | null; your_name: string | null; ai: AiSettings; speech?: SpeechSettings; language_models?: LanguageModelSettings }

export const processingStatus = () => invoke<ProcessingStatus>("processing_status");
/** A chosen Speech-to-text model is local:<name>, groq:<id>, gemini:<id>, or managed. Omit it for the Transcription order. */
export const enqueueTranscription = (meetingId: string, chosenModel?: string) => invoke<ProcessingStatus>("transcribe_meeting", { meetingId, model: chosenModel ?? null });
export const cancelProcessing = (meetingId: string) => invoke<ProcessingStatus>("cancel_processing", { meetingId });
export const dismissProcessing = (meetingId: string) => invoke<ProcessingStatus>("dismiss_processing", { meetingId });
export const meetingTranscript = (meetingId: string) => invoke<Transcript | null>("meeting_transcript", { meetingId });
export const transcriptDraft = (meetingId: string) => invoke<DraftSegment[]>("transcript_draft", { meetingId });
export const setMeetingLanguage = (meetingId: string, language: Language | null) => invoke<void>("set_meeting_language", { meetingId, language });
export const getSettings = () => invoke<Settings>("get_settings");
export const acknowledgeCloudNotice = () => invoke<Settings>("acknowledge_cloud_notice");
export const updateSettings = (autoProcess: boolean, model: string, vocabulary: string[], language: Language | null, yourName: string | null, speech?: SpeechSettings) => invoke<Settings>("update_settings", { autoProcess, model, vocabulary, language, yourName, speech: speech ?? null });
export const testSpeechModel = (provider: SpeechProvider, model: string) => invoke<void>("test_speech_model", { provider, model });
export const installedModels = () => invoke<string[]>("installed_models");
export interface ModelDownloadStatus { model_id: string; state: "not_downloaded" | "downloading" | "downloaded" | "failed" | "cancelled"; progress: number; error: string | null }
export const modelDownloadStatus = () => invoke<ModelDownloadStatus[]>("model_download_status");
export const startModelDownload = (modelId: string) => invoke<ModelDownloadStatus[]>("start_model_download", { modelId });
export const cancelModelDownload = (modelId: string) => invoke<ModelDownloadStatus[]>("cancel_model_download", { modelId });
export const deleteModelDownload = (modelId: string) => invoke<ModelDownloadStatus[]>("delete_model_download", { modelId });
export const onModelDownloadStatus = (handler: (status: ModelDownloadStatus) => void): Promise<UnlistenFn> => listen<ModelDownloadStatus>("model-download-status", (event) => handler(event.payload));
export const onProcessingStatus = (handler: (status: ProcessingStatus) => void): Promise<UnlistenFn> => listen<ProcessingStatus>("processing-status", (event) => handler(event.payload));
export const onTranscriptDraft = (handler: (draft: TranscriptDraftEvent) => void): Promise<UnlistenFn> => listen<TranscriptDraftEvent>("transcript-draft", (event) => handler(event.payload));
