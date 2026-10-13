import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Disclosure, ProviderKind } from "./ai";
export interface Citation { segment_id: string; start_ms: number; end_ms: number }
export interface SummaryItem { text: string; citations: Citation[] }
export interface ActionItem extends SummaryItem { owner: string | null; due: string | null }
export interface Summary { schema_version: number; revision: number; created_at: string; provider: ProviderKind; model: string; overview: SummaryItem[]; key_points: SummaryItem[]; decisions: SummaryItem[]; action_items: ActionItem[]; dropped_items: number; participants_fingerprint?: string | null }
export interface SummaryJob { meeting_id: string; state: "running" | "failed"; done: number; total: number; error: string | null }
export interface SummaryStatus { jobs: SummaryJob[] }
export interface SendPlan { provider: ProviderKind; cloud: boolean; consented: boolean; disclosure: Disclosure | null }
export const meetingSummary = (meetingId: string) => invoke<Summary | null>("meeting_summary", { meetingId });
export const summaryParticipantsChanged = (meetingId: string) => invoke<boolean>("summary_participants_changed", { meetingId });
export const summaryStatus = () => invoke<SummaryStatus>("summary_status");
export const summarySendPlan = (meetingId: string) => invoke<SendPlan>("summary_send_plan", { meetingId });
export const generateSummary = (meetingId: string, consent: boolean) => invoke<SummaryStatus>("generate_summary", { meetingId, consent });
export const cancelSummary = (meetingId: string) => invoke<SummaryStatus>("cancel_summary", { meetingId });
export const dismissSummary = (meetingId: string) => invoke<SummaryStatus>("dismiss_summary", { meetingId });
export const onSummaryStatus = (handler: (status: SummaryStatus) => void): Promise<UnlistenFn> => listen<SummaryStatus>("summary-status", (event) => handler(event.payload));
