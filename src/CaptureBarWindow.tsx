import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { Camera, ChevronLeft, ChevronRight, GripVertical, Pause, Play, Square } from "lucide-react";
import {
  formatElapsed,
  onRecordingStatus,
  onSnapshotError,
  pauseRecording,
  recordingStatus,
  resumeRecording,
  stopRecording,
  takeSnapshot,
  type RecorderStatus,
} from "./recording";
import SourceHealthNotice from "./SourceHealthNotice";
import { usePlatformCapabilities } from "./platform-capabilities";

const expandedSize = new LogicalSize(354, 58);
const collapsedSize = new LogicalSize(174, 58);
const warningHeight = 76;

export default function CaptureBarWindow() {
  const capabilities = usePlatformCapabilities();
  const [status, setStatus] = useState<RecorderStatus | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const previousWarningCount = useRef(0);
  const warningCount = status?.phase === "recording" ? status.source_health?.filter((health) => health.warning !== null).length ?? 0 : 0;
  const hasWarning = warningCount > 0;

  useEffect(() => {
    if (warningCount === previousWarningCount.current) return;
    previousWarningCount.current = warningCount;
    if (hasWarning) {
      setCollapsed(false);
      void getCurrentWindow().setSize(new LogicalSize(354, expandedSize.height + warningHeight * warningCount)).catch((reason) => setError(String(reason)));
    } else {
      void getCurrentWindow().setSize(expandedSize).catch((reason) => setError(String(reason)));
    }
  }, [hasWarning, warningCount]);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    const subscriptions = [
      onRecordingStatus((next) => {
        if (active) { eventArrived = true; setStatus(next); }
      }),
    ];
    recordingStatus().then(
      (next) => { if (active && !eventArrived) setStatus(next); },
      (reason) => { if (active) setError(String(reason)); },
    );
    return () => {
      active = false;
      subscriptions.forEach((subscription) => void subscription.then((stop) => stop(), () => {}));
    };
  }, []);

  useEffect(() => {
    if (!capabilities.snapshots) return;
    let active = true;
    const unlisten = onSnapshotError((reason) => { if (active) setError(reason); });
    return () => { active = false; void unlisten.then((stop) => stop(), () => {}); };
  }, [capabilities.snapshots]);

  async function changeRecording(action: () => Promise<RecorderStatus>) {
    setBusy(true);
    setError(null);
    try { setStatus(await action()); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  async function snapshot() {
    setBusy(true);
    setError(null);
    try { await takeSnapshot(true); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  async function toggleCollapsed() {
    if (hasWarning) return;
    const next = !collapsed;
    try {
      await getCurrentWindow().setSize(next ? collapsedSize : expandedSize);
      setCollapsed(next);
    } catch (reason) { setError(String(reason)); }
  }

  if (!status || status.phase === "idle") return null;

  const control = "flex size-9 shrink-0 items-center justify-center rounded-md text-white hover:bg-white/15 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-white disabled:opacity-50";
  return <div aria-label="Capture bar" className="box-border flex h-screen w-screen flex-col overflow-hidden rounded-xl border border-white/25 bg-[#172234] text-white select-none">
    <div className="flex min-h-[56px] w-full items-center gap-1 px-2">
    <button type="button" aria-label="Drag Capture bar" title="Drag Capture bar" className="flex h-9 w-5 shrink-0 cursor-move items-center justify-center text-white/60 hover:text-white" onPointerDown={() => void getCurrentWindow().startDragging().catch((reason) => setError(String(reason)))}><GripVertical size={17} aria-hidden="true" /></button>
    <div role="status" aria-live="polite" title={error ?? undefined} className={`min-w-0 shrink-0 whitespace-nowrap text-xs font-semibold tabular-nums ${error ? "text-red-300" : "text-white"}`}>
      <span aria-hidden="true" className={`mr-2 inline-block size-2 rounded-full ${status.phase === "paused" ? "bg-amber-400" : "bg-red-500"}`} />
      {error ? <span role="alert" className="inline-block max-w-[105px] truncate align-middle">{error}</span> : formatElapsed(status.elapsed_ms)}
    </div>
    {!collapsed && <div className="ml-auto flex items-center gap-0.5">
      <button type="button" aria-label={status.phase === "paused" ? "Resume" : "Pause"} title={status.phase === "paused" ? "Resume" : "Pause"} disabled={busy} className={control} onClick={() => void changeRecording(status.phase === "paused" ? resumeRecording : pauseRecording)}>{status.phase === "paused" ? <Play size={17} aria-hidden="true" /> : <Pause size={17} aria-hidden="true" />}</button>
      {capabilities.snapshots && <button type="button" aria-label="Take Snapshot" title="Take Snapshot" disabled={busy} className={control} onClick={() => void snapshot()}><Camera size={17} aria-hidden="true" /></button>}
      <button type="button" aria-label="Stop" title="Stop" disabled={busy} className={control} onClick={() => void changeRecording(stopRecording)}><Square size={15} fill="currentColor" aria-hidden="true" /></button>
    </div>}
    <button type="button" aria-label={collapsed ? "Expand Capture bar" : "Collapse Capture bar"} title={collapsed ? "Expand Capture bar" : "Collapse Capture bar"} disabled={hasWarning} className={control} onClick={() => void toggleCollapsed()}>{collapsed ? <ChevronRight size={17} aria-hidden="true" /> : <ChevronLeft size={17} aria-hidden="true" />}</button>
    </div>
    <SourceHealthNotice status={status} onStatus={setStatus} compact />
  </div>;
}
