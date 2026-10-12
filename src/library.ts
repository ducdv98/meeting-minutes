import { invoke } from "@tauri-apps/api/core";

export interface MeetingSummary {
  id: string;
  title: string;
  started_at: string;
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
