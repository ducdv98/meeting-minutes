import { expect, it } from "vitest";
import { currentSegmentId, participantList, participantName, speakerName } from "./transcript";
import type { Segment } from "./processing";

const segments: Segment[] = [
  { id: "first", start_ms: 100, end_ms: 500, text: "One", speaker: "me" },
  { id: "overlap", start_ms: 300, end_ms: 400, text: "Two", speaker: "remote" },
  { id: "last", start_ms: 700, end_ms: 900, text: "Three", speaker: "unknown" },
];

it("finds the latest starting Segment covering the Timeline position", () => {
  expect(currentSegmentId(segments, 99)).toBeNull();
  expect(currentSegmentId(segments, 100)).toBe("first");
  expect(currentSegmentId(segments, 350)).toBe("overlap");
  expect(currentSegmentId(segments, 400)).toBe("first");
  expect(currentSegmentId(segments, 500)).toBeNull();
  expect(currentSegmentId(segments, 600)).toBeNull();
  expect(currentSegmentId(segments, 900)).toBeNull();
});

it("uses the domain Speaker labels", () => {
  expect(speakerName("me")).toBe("Me");
  expect(speakerName("remote")).toBe("Remote");
  expect(speakerName("unknown")).toBe("Unknown");
});

it("resolves revision-scoped assignments without trimming stored Participant names", () => {
  const transcript = { schema_version: 1, revision: 2, created_at: "now", model: "small", language: "en" as const, language_detected: false, duration_ms: 900, segments };
  const overrides = { revision: 2, segments: { first: "  Anna  ", overlap: "  Anna  " } };
  expect(participantName(segments[0], 2, {}, overrides, " Duc ")).toBe("  Anna  ");
  expect(participantName(segments[0], 1, {}, overrides, " Duc ")).toBe("Duc");
  expect(participantList(transcript, {}, overrides, " Duc ")).toEqual(["  Anna  ", "Duc", "Remote", "Unknown"]);
});
