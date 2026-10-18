import { invoke } from "@tauri-apps/api/core";

export interface Snapshot {
  order: number;
  timeline_ms: number;
  caption: string | null;
}

export interface SnapshotWithImage extends Snapshot {
  image_url: string;
}

export interface SnapshotList {
  snapshots: Snapshot[];
  total_size_bytes: number;
}

export function listSnapshots(meetingId: string): Promise<SnapshotList> {
  return invoke<SnapshotList>("list_snapshots", { meetingId });
}

export async function readSnapshotImage(meetingId: string, order: number): Promise<string> {
  const bytes = await invoke<number[]>("read_snapshot_image", { meetingId, order });
  let binary = "";
  for (let start = 0; start < bytes.length; start += 8192) {
    binary += String.fromCharCode(...bytes.slice(start, start + 8192));
  }
  return `data:image/png;base64,${btoa(binary)}`;
}

export function revealSnapshotFolder(meetingId: string, order: number): Promise<void> {
  return invoke<void>("reveal_snapshot_folder", { meetingId, order });
}

export function setSnapshotCaption(meetingId: string, order: number, caption: string | null): Promise<Snapshot> {
  return invoke<Snapshot>("set_snapshot_caption", { meetingId, order, caption });
}

export function formatSnapshotSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
