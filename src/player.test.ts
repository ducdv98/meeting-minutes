import { expect, it, vi } from "vitest";
import { createTimelinePlayer, LEAD_IN_MS, markerLabel, markerPosition } from "./player";

function media() {
  const element = document.createElement("audio");
  element.play = vi.fn().mockResolvedValue(undefined);
  element.pause = vi.fn();
  return element;
}

it("plays a span with a 500 ms lead-in and pauses at its end", () => {
  const element = media();
  const player = createTimelinePlayer(element);
  expect(LEAD_IN_MS).toBe(500);
  player.playSpan(2_000, 4_000);
  expect(element.currentTime).toBe(1.5);
  expect(element.play).toHaveBeenCalledTimes(1);
  element.currentTime = 3.9;
  element.dispatchEvent(new Event("timeupdate"));
  expect(element.pause).not.toHaveBeenCalled();
  element.currentTime = 4;
  element.dispatchEvent(new Event("timeupdate"));
  expect(element.pause).toHaveBeenCalledTimes(1);
  player.dispose();
});

it("clamps the lead-in at zero and cancels a span on manual seek", () => {
  const element = media();
  const player = createTimelinePlayer(element);
  player.playSpan(200, 1_000);
  expect(element.currentTime).toBe(0);
  player.seek(5_000);
  expect(element.currentTime).toBe(5);
  element.dispatchEvent(new Event("timeupdate"));
  expect(element.pause).not.toHaveBeenCalled();
  player.dispose();
});

it("pauses on the first update for an empty span and removes its listener on dispose", () => {
  const element = media();
  const player = createTimelinePlayer(element);
  player.playSpan(2_000, 2_000);
  element.dispatchEvent(new Event("timeupdate"));
  expect(element.pause).toHaveBeenCalledTimes(1);
  player.playSpan(3_000, 4_000);
  player.dispose();
  element.currentTime = 4;
  element.dispatchEvent(new Event("timeupdate"));
  expect(element.pause).toHaveBeenCalledTimes(1);
});

it("positions and labels Timeline markers", () => {
  expect(markerPosition(5_000, 20_000)).toBe(25);
  expect(markerPosition(-1, 20_000)).toBe(0);
  expect(markerPosition(30_000, 20_000)).toBe(100);
  expect(markerPosition(1, 0)).toBe(0);
  expect(markerLabel({ kind: "pause", at_ms: 1_000, paused_ms: 125_000 })).toBe("Paused for 2m 05s");
  expect(markerLabel({ kind: "gap", at_ms: 2_000 })).toBe("Gap between Recordings");
  expect(markerLabel({ kind: "device_switch", at_ms: 3_000, source: "microphone", device: "Headset" })).toBe("Microphone switched to Headset");
  expect(markerLabel({ kind: "device_switch", at_ms: 4_000, source: "system_audio", device: "Speakers" })).toBe("System audio switched to Speakers");
});
