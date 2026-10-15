import { useEffect, useRef, useState } from "react";
import { modelDownloadStatus, onModelDownloadStatus, type ModelDownloadStatus } from "../processing";

export function useModelDownloads() {
  const [downloads, setDownloads] = useState<Record<string, ModelDownloadStatus>>({});
  const [error, setError] = useState<string | null>(null);
  const eventVersions = useRef<Record<string, number>>({});

  useEffect(() => {
    let active = true;
    const seen = new Set<string>();
    let unlisten: (() => void) | undefined;
    void onModelDownloadStatus((status) => {
      if (!active) return;
      seen.add(status.model_id);
      eventVersions.current[status.model_id] = (eventVersions.current[status.model_id] ?? 0) + 1;
      setDownloads((current) => ({ ...current, [status.model_id]: status }));
    }).then((stop) => {
      if (!active) { stop(); return; }
      unlisten = stop;
      return modelDownloadStatus().then((statuses) => {
        if (active) setDownloads((current) => ({ ...current, ...Object.fromEntries(statuses.filter((status) => !seen.has(status.model_id)).map((status) => [status.model_id, status])) }));
      });
    }).catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; unlisten?.(); };
  }, []);

  async function run(modelId: string, request: () => Promise<ModelDownloadStatus[]>) {
    setError(null);
    const version = eventVersions.current[modelId] ?? 0;
    try {
      const statuses = await request();
      const status = statuses.find((entry) => entry.model_id === modelId);
      if (status && (eventVersions.current[modelId] ?? 0) === version) setDownloads((current) => ({ ...current, [modelId]: status }));
    } catch (reason) { setError(String(reason)); }
  }

  return { downloads, error, run };
}
