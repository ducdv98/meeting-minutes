import { expect, it } from "vitest";
import { currentSegmentId, speakerName } from "./transcript";
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
