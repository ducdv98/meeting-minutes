import { invoke } from "@tauri-apps/api/core";

export interface PromptApp { id: string; display_name: string }

export const getRecordPrompt = () => invoke<PromptApp | null>("current_record_prompt");
export const dismissRecordPrompt = () => invoke<void>("dismiss_record_prompt");
export const showRecordingNotice = () => invoke<void>("show_recording_notice");
export const silenceRecordPromptApp = () => invoke<void>("silence_record_prompt_app");
