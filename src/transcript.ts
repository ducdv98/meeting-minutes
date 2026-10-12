import { invoke } from "@tauri-apps/api/core";
import type { Segment, Speaker, Transcript } from "./processing";

export const editSegment = (meetingId: string, revision: number, segmentId: string, text: string) =>
  invoke<Transcript>("edit_segment", { meetingId, revision, segmentId, text });

export function currentSegmentId(segments: Segment[], positionMs: number): string | null {
  let low = 0;
  let high = segments.length;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (segments[middle].start_ms <= positionMs) low = middle + 1;
    else high = middle;
  }
  for (let index = low - 1; index >= 0; index--) {
    if (positionMs < segments[index].end_ms) return segments[index].id;
  }
  return null;
}

export function speakerName(speaker: Speaker): string {
  switch (speaker) {
    case "me": return "Me";
    case "remote": return "Remote";
    case "unknown": return "Unknown";
  }
}
