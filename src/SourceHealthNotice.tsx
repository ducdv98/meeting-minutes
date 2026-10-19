import { useState } from "react";
import { dismissSourceWarning, openSoundSettings, type RecorderStatus } from "./recording";

export default function SourceHealthNotice({ status, onStatus, compact = false }: {
  status: RecorderStatus;
  onStatus: (status: RecorderStatus) => void;
  compact?: boolean;
}) {
  const [error, setError] = useState<string | null>(null);
  if (status.phase !== "recording") return null;
  const warnings = status.source_health?.filter((health) => health.warning !== null) ?? [];
  return <>{warnings.map(({ source, warning }) => {
    const name = source === "microphone" ? "Microphone" : "System audio";
    const device = status.source_devices?.find((item) => item.source === source)?.device ?? "current device";
    return <div key={source} role="alert" className={compact ? "px-3 py-2 text-xs text-amber-100" : "mx-5 mt-3 rounded-md border border-warn bg-raised px-3 py-2.5 text-sm text-ink"}>
      <div className="font-medium">{warning === "no_signal" ? `${name} has no signal` : "System audio is silent; the other side may not be captured"} · {device}</div>
      <div className="mt-1 flex items-center gap-3">
        <button type="button" className="underline" onClick={() => void openSoundSettings().catch((reason) => setError(String(reason)))}>Open sound settings</button>
        <button type="button" className="underline" onClick={() => void dismissSourceWarning(source).then(onStatus, (reason) => setError(String(reason)))}>Dismiss</button>
      </div>
      {error && <span>{error}</span>}
    </div>;
  })}</>;
}
