import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { LanguageModel, ProviderKind } from "./ai";
import type { AnswerState } from "./ask";

export type DateRange = "last_7_days" | "last_30_days" | "last_90_days" | "all";
export interface PlannedMeeting { meeting_id: string; title: string; started_at: string; excerpts: number }
export interface LibraryAskPlan { plan_id: string; provider: ProviderKind; cloud: boolean; meetings: PlannedMeeting[] }
export interface LibraryCitation { meeting_id: string; segment_id: string; revision: number; start_ms: number; end_ms: number }
export interface LibraryAskEntry { id: string; question: string; range: DateRange; asked_at: string; provider: ProviderKind; model: string; state: AnswerState; answer: string; citations: LibraryCitation[]; invalid_citations: string[]; error: string | null; meetings_read: string[] }
export interface LibraryAskView { asks: LibraryAskEntry[]; active: { ask_id: string; partial: string } | null }
export interface LibraryAskDelta { ask_id: string; text: string }

export const libraryAskPlan = (question: string, range: DateRange, model?: LanguageModel | null) => invoke<LibraryAskPlan>("library_ask_plan", { question, range, model: model ?? null });
export const libraryAskStart = (planId: string, excludedMeetings: string[], consent: boolean) => invoke<LibraryAskView>("library_ask_start", { planId, excludedMeetings, consent });
export const libraryAsks = () => invoke<LibraryAskView>("library_asks");
export const stopLibraryAsk = () => invoke<void>("stop_library_ask");
export const onLibraryAskDelta = (handler: (delta: LibraryAskDelta) => void): Promise<UnlistenFn> => listen<LibraryAskDelta>("library-ask-delta", (event) => handler(event.payload));
export const onLibraryAsksChanged = (handler: () => void): Promise<UnlistenFn> => listen("library-asks-changed", handler);
