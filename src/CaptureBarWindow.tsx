import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { getCurrentWindow, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";
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

// Wide enough for Source health warnings to wrap to a readable line length.
const warningWidth = 354;

export default function CaptureBarWindow() {
  const capabilities = usePlatformCapabilities();
  const [status, setStatus] = useState<RecorderStatus | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const visible = status !== null && status.phase !== "idle";
  const hasWarning = status?.phase === "recording" && (status.source_health?.some((health) => health.warning !== null) ?? false);

  useEffect(() => {
    if (hasWarning) setCollapsed(false);
  }, [hasWarning]);

  // The window follows the bar's measured size rather than fixed numbers, so display scaling,
  // text size and fonts can never clip the controls. The right edge stays put, like a toolbar.
  useLayoutEffect(() => {
    const element = bar.current;
    if (!visible || !element) return;
    const appWindow = getCurrentWindow();
    let running = false;
    let again = false;
    async function fit() {
      if (running) { again = true; return; }
      running = true;
      try {
        do {
          again = false;
          const rect = element!.getBoundingClientRect();
          const current = await appWindow.innerSize();
          const pixelsPerCss = current.width / window.innerWidth;
          const width = Math.ceil(rect.width * pixelsPerCss);
          const height = Math.ceil(rect.height * pixelsPerCss);
          if (Math.abs(width - current.width) <= 1 && Math.abs(height - current.height) <= 1) continue;
          const position = await appWindow.outerPosition();
          await appWindow.setSize(new PhysicalSize(width, height));
          await appWindow.setPosition(new PhysicalPosition(position.x + current.width - width, position.y));
        } while (again);
      } catch (reason) { setError(String(reason)); }
      finally { running = false; }
    }
    const observer = new ResizeObserver(() => void fit());
    observer.observe(element);
    window.addEventListener("resize", fit);
    return () => { observer.disconnect(); window.removeEventListener("resize", fit); };
  }, [visible]);

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

  function toggleCollapsed() {
    if (!hasWarning) setCollapsed(!collapsed);
  }

  if (!status || status.phase === "idle") return null;

  const control = "flex size-9 shrink-0 items-center justify-center rounded-md text-ink-muted transition-colors hover:bg-hover hover:text-ink focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent disabled:opacity-50";
  return <div ref={bar} aria-label="Capture bar" style={{ width: hasWarning ? warningWidth : "max-content" }} className="box-border flex flex-col overflow-hidden rounded-lg border border-line-strong bg-raised text-ink select-none">
    <div className="flex min-h-14 w-full items-center gap-1 px-2">
    <button type="button" aria-label="Drag Capture bar" title="Drag Capture bar" className="flex h-9 w-5 shrink-0 cursor-move items-center justify-center text-ink-faint hover:text-ink-muted" onPointerDown={() => void getCurrentWindow().startDragging().catch((reason) => setError(String(reason)))}><GripVertical size={17} aria-hidden="true" /></button>
    <div role="status" aria-live="polite" title={error ?? undefined} className={`min-w-0 shrink-0 whitespace-nowrap text-sm font-medium tabular-nums ${error ? "text-record" : "text-ink"}`}>
      <span aria-hidden="true" className={`mr-2 inline-block size-2 rounded-full ${status.phase === "paused" ? "bg-warn" : "bg-record"}`} />
      {error ? <span role="alert" className="inline-block max-w-[105px] truncate align-middle">{error}</span> : formatElapsed(status.elapsed_ms)}
    </div>
    {!collapsed && <div className="ml-auto flex items-center gap-0.5">
      <button type="button" aria-label={status.phase === "paused" ? "Resume" : "Pause"} title={status.phase === "paused" ? "Resume" : "Pause"} disabled={busy} className={control} onClick={() => void changeRecording(status.phase === "paused" ? resumeRecording : pauseRecording)}>{status.phase === "paused" ? <Play size={17} aria-hidden="true" /> : <Pause size={17} aria-hidden="true" />}</button>
      {capabilities.snapshots && <button type="button" aria-label="Take Snapshot" title="Take Snapshot" disabled={busy} className={control} onClick={() => void snapshot()}><Camera size={17} aria-hidden="true" /></button>}
      <button type="button" aria-label="Stop" title="Stop" disabled={busy} className={`${control} text-record hover:text-record`} onClick={() => void changeRecording(stopRecording)}><Square size={15} fill="currentColor" aria-hidden="true" /></button>
    </div>}
    <button type="button" aria-label={collapsed ? "Expand Capture bar" : "Collapse Capture bar"} title={collapsed ? "Expand Capture bar" : "Collapse Capture bar"} disabled={hasWarning} className={control} onClick={toggleCollapsed}>{collapsed ? <ChevronRight size={17} aria-hidden="true" /> : <ChevronLeft size={17} aria-hidden="true" />}</button>
    </div>
    <SourceHealthNotice status={status} onStatus={setStatus} compact />
  </div>;
}
