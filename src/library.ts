import { invoke } from "@tauri-apps/api/core";
import type { Speaker } from "./processing";

export interface MeetingSummary {
  id: string;
  title: string;
  started_at: string;
  duration_ms: number;
}

export interface SearchHit {
  meeting_id: string;
  revision: number;
  title: string;
  started_at: string;
  segment_id: string;
  start_ms: number;
  end_ms: number;
  snippet: string;
}

export type Marker =
  | { kind: "pause"; at_ms: number; paused_ms: number }
  | { kind: "gap"; at_ms: number }
  | { kind: "device_switch"; at_ms: number; source: "microphone" | "system_audio"; device: string };

export interface MeetingDetail extends MeetingSummary {
  language: string | null;
  audio_path: string | null;
  markers: Marker[];
  imported: boolean;
  sources?: ("microphone" | "system_audio")[];
  voice_note: boolean;
  participants: Partial<Record<Speaker, string>>;
  segment_participants?: { revision: number; segments: Record<string, string> } | null;
  correction_notice?: string | null;
}

export interface MalformedFolder {
  folder: string;
  reason: string;
}

export interface ScanReport {
  meetings: MeetingSummary[];
  malformed: MalformedFolder[];
}

export function listMeetings(): Promise<MeetingSummary[]> {
  return invoke<MeetingSummary[]>("list_meetings");
}

export function rescanLibrary(): Promise<ScanReport> {
  return invoke<ScanReport>("rescan_library");
}

export function meetingDetail(id: string): Promise<MeetingDetail> {
  return invoke<MeetingDetail>("meeting_detail", { meetingId: id });
}

export function openMeetingFolder(id: string): Promise<void> {
  return invoke<void>("open_meeting_folder", { meetingId: id });
}

export function renameMeeting(id: string, title: string): Promise<MeetingSummary> {
  return invoke<MeetingSummary>("rename_meeting", { meetingId: id, title });
}

export function setParticipantName(id: string, label: Speaker, name: string | null): Promise<void> {
  return invoke<void>("set_participant_name", { meetingId: id, label, name });
}

export function assignSegments(id: string, revision: number, segmentIds: string[], name: string | null): Promise<void> {
  return invoke<void>("assign_segments", { meetingId: id, revision, segmentIds, name });
}

export function renameParticipant(id: string, from: string, to: string): Promise<void> {
  return invoke<void>("rename_participant", { meetingId: id, from, to });
}

export function removeParticipant(id: string, name: string): Promise<void> {
  return invoke<void>("remove_participant", { meetingId: id, name });
}

export function participantNames(): Promise<string[]> {
  return invoke<string[]>("list_participant_names");
}

export function searchLibrary(query: string): Promise<SearchHit[]> {
  return invoke<SearchHit[]>("search_library", { query });
}

export function deleteMeeting(id: string): Promise<void> {
  return invoke<void>("delete_meeting", { meetingId: id });
}

export function snippetParts(snippet: string): { text: string; match: boolean }[] {
  const parts: { text: string; match: boolean }[] = [];
  let matching = false;
  for (const text of snippet.split(/(\u0002|\u0003)/)) {
    if (text === "\u0002") matching = true;
    else if (text === "\u0003") matching = false;
    else if (text) parts.push({ text, match: matching });
  }
  return parts;
}
