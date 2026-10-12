import { invoke } from "@tauri-apps/api/core";

export interface MeetingSummary {
  id: string;
  title: string;
  started_at: string;
}

export function listMeetings(): Promise<MeetingSummary[]> {
  return invoke<MeetingSummary[]>("list_meetings");
}
