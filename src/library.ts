import { invoke } from "@tauri-apps/api/core";

export interface MeetingSummary {
  id: string;
  title: string;
  started_at: string;
  duration_ms: number;
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

export function renameMeeting(id: string, title: string): Promise<MeetingSummary> {
  return invoke<MeetingSummary>("rename_meeting", { meetingId: id, title });
}
