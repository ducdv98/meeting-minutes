import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { LanguageModel, ProviderKind } from "./ai";
import type { Citation, SendPlan } from "./summary";
export type AnswerState = "pending" | "answered" | "insufficient_evidence" | "off_topic" | "incomplete" | "unverified" | "failed";
export interface AskEntry { id: string; question: string; asked_at: string; revision: number; provider?: ProviderKind | null; model?: string | null; state: AnswerState; answer: string; citations: Citation[]; invalid_citations: string[]; error: string | null }
export interface AskView { asks: AskEntry[]; active: { ask_id: string; partial: string } | null; retryable_id: string | null }
export interface AskDelta { meeting_id: string; ask_id: string; text: string }
export const meetingAsks = (meetingId: string) => invoke<AskView>("meeting_asks", { meetingId });
export const askSendPlan = (meetingId: string, question: string, model?: LanguageModel | null) => invoke<SendPlan>("ask_send_plan", { meetingId, question, model: model ?? null });
export const askMeeting = (meetingId: string, question: string, consent: boolean, model?: LanguageModel | null) => invoke<AskView>("ask_meeting", { meetingId, question, consent, model: model ?? null });
export const retryAsk = (meetingId: string, consent: boolean, model?: LanguageModel | null) => invoke<AskView>("retry_ask", { meetingId, consent, model: model ?? null });
export const stopAsk = (meetingId: string) => invoke<void>("stop_ask", { meetingId });
export const onAskDelta = (handler: (delta: AskDelta) => void): Promise<UnlistenFn> => listen<AskDelta>("ask-delta", (event) => handler(event.payload));
export const onAsksChanged = (handler: (meetingId: string) => void): Promise<UnlistenFn> => listen<{ meeting_id: string }>("asks-changed", (event) => handler(event.payload.meeting_id));
export function answerLabel(state: AnswerState): string {
  switch (state) { case "insufficient_evidence": return "Insufficient evidence"; case "incomplete": return "Incomplete"; case "unverified": return "Unverified"; case "failed": return "Failed"; default: return ""; }
}
