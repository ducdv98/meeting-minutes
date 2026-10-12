import type { Marker } from "./library";

export const LEAD_IN_MS = 500;

export interface TimelinePlayer {
  seek(ms: number): void;
  play(): void;
  pause(): void;
  playSpan(startMs: number, endMs: number): void;
}

export function createTimelinePlayer(media: HTMLMediaElement): TimelinePlayer & { dispose(): void } {
  let spanEndMs: number | null = null;
  let emptySpan = false;
  const checkSpanEnd = () => {
    if (spanEndMs !== null && (emptySpan || media.currentTime * 1000 >= spanEndMs)) {
      spanEndMs = null;
      emptySpan = false;
      media.pause();
    }
  };
  media.addEventListener("timeupdate", checkSpanEnd);
  return {
    seek(ms) {
      spanEndMs = null;
      emptySpan = false;
      media.currentTime = Math.max(0, ms) / 1000;
    },
    play() {
      spanEndMs = null;
      emptySpan = false;
      void media.play();
    },
    pause() {
      spanEndMs = null;
      emptySpan = false;
      media.pause();
    },
    playSpan(startMs, endMs) {
      media.currentTime = Math.max(0, startMs - LEAD_IN_MS) / 1000;
      spanEndMs = endMs;
      emptySpan = endMs <= startMs;
      void media.play();
    },
    dispose() {
      spanEndMs = null;
      emptySpan = false;
      media.removeEventListener("timeupdate", checkSpanEnd);
    },
  };
}

export function markerLabel(marker: Marker): string {
  switch (marker.kind) {
    case "pause": {
      const seconds = Math.floor(marker.paused_ms / 1000);
      return `Paused for ${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, "0")}s`;
    }
    case "gap":
      return "Gap between Recordings";
    case "device_switch":
      return `${marker.source === "microphone" ? "Microphone" : "System audio"} switched to ${marker.device}`;
  }
}

export function markerPosition(atMs: number, durationMs: number): number {
  return durationMs > 0 ? Math.min(100, Math.max(0, atMs / durationMs * 100)) : 0;
}
