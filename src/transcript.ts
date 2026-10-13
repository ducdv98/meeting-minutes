import { invoke } from "@tauri-apps/api/core";
import type { Segment, Speaker, Transcript } from "./processing";
import type { MeetingDetail } from "./library";

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

export function labelParticipantName(speaker: Speaker, participants: Partial<Record<Speaker, string>>, yourName: string | null, isVoiceNote = false): string {
  const label = isVoiceNote ? "me" : speaker;
  return participants[label] ?? (label === "me" ? yourName?.trim() || "Me" : speakerName(label));
}

export function participantName(segment: Segment, revision: number, participants: Partial<Record<Speaker, string>>, segmentParticipants: MeetingDetail["segment_participants"], yourName: string | null, isVoiceNote = false): string {
  // Every Segment of a Voice note belongs to the user.
  return (!isVoiceNote && segmentParticipants?.revision === revision ? segmentParticipants.segments[segment.id] : undefined) ?? labelParticipantName(segment.speaker, participants, yourName, isVoiceNote);
}

export function participantList(transcript: Transcript, participants: Partial<Record<Speaker, string>>, segmentParticipants: MeetingDetail["segment_participants"], yourName: string | null, isVoiceNote = false): string[] {
  // Each present Speaker label keeps its Participant even when all its Segments moved away.
  return [...new Set(transcript.segments.flatMap((segment) => [participantName(segment, transcript.revision, participants, segmentParticipants, yourName, isVoiceNote), labelParticipantName(segment.speaker, participants, yourName, isVoiceNote)]))];
}
