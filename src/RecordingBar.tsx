import { useEffect, useRef, useState, type ReactNode } from "react";
import { Mic, Pause, Play, Speaker, Square, X } from "lucide-react";
import { Button } from "./components/ui/Button";
import { LevelMeter } from "./components/ui/LevelMeter";
import { ErrorState } from "./components/states/ErrorState";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./components/ui/Dialog";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "./components/ui/Tooltip";
import { RECORDING_NOTICE } from "./recording-notice";
import {
  acknowledgeRecordingNotice,
  continueMeeting,
  dismissContinue,
  formatElapsed,
  onRecordingNoticeRequested,
  onRecordingStatus,
  onSnapshotError,
  pauseHotkey,
  pauseRecording,
  recordHotkey,
  recordingStatus,
  resumeRecording,
  retrySave,
  startRecording,
  stopRecording,
  type RecorderStatus,
} from "./recording";

function RecordingControl({ label, hotkey, variant, onClick, children }: {
  label: string;
  hotkey: string;
  variant: "secondary" | "record";
  onClick: () => void;
  children: ReactNode;
}) {
  return <Tooltip>
    <TooltipTrigger asChild><Button variant={variant} onClick={onClick} title={`${label} (${hotkey})`}>{children}{label}</Button></TooltipTrigger>
    <TooltipContent>{label} ({hotkey})</TooltipContent>
  </Tooltip>;
}

export default function RecordingBar({ recordRequest = 0, hideIdleButton = false }: { recordRequest?: number; hideIdleButton?: boolean }) {
  const [status, setStatus] = useState<RecorderStatus | null>(null);
  const [noticeAction, setNoticeAction] = useState<(() => Promise<RecorderStatus>) | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [snapshotError, setSnapshotError] = useState<string | null>(null);
  const [dismissedSwitches, setDismissedSwitches] = useState(0);
  const handledRecordRequest = useRef(0);

  useEffect(() => {
    if (status?.phase === "idle") setDismissedSwitches(0);
  }, [status?.phase]);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    const unlisten = [
      onRecordingStatus((next) => { if (active) { eventArrived = true; setStatus(next); } }),
      onRecordingNoticeRequested(() => { if (active) setNoticeAction(() => startRecording); }),
      onSnapshotError((reason) => { if (active) setSnapshotError(reason); }),
    ];
    recordingStatus().then(
      (next) => { if (active && !eventArrived) setStatus(next); },
      () => { if (active) setError("Could not read the recording state"); },
    );
    return () => {
      active = false;
      unlisten.forEach((promise) => promise.then((stop) => stop(), () => {}));
    };
  }, []);

  function run(action: () => Promise<RecorderStatus>) {
    setError(null);
    action().then(setStatus, (reason) => setError(String(reason)));
  }

  function record(action: () => Promise<RecorderStatus>) {
    if (status && !status.notice_acknowledged) {
      setNoticeAction(() => action);
      return;
    }
    run(action);
  }

  function confirmNotice() {
    const action = noticeAction;
    setNoticeAction(null);
    if (!action) return;
    run(async () => {
      await acknowledgeRecordingNotice();
      return action();
    });
  }

  useEffect(() => {
    if (recordRequest > handledRecordRequest.current && status?.phase === "idle") {
      handledRecordRequest.current = recordRequest;
      record(startRecording);
    }
    // A later status update can complete a request made during loading.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recordRequest, status?.phase]);

  if (!status) return error ? <p role="alert" className="px-5 py-3 text-sm text-record">{error}</p> : null;

  const capturing = status.phase !== "idle";
  const latestSwitch = capturing && status.device_switches.length > dismissedSwitches
    ? status.device_switches.at(-1) : undefined;
  if (hideIdleButton && !capturing && !status.saves.length && !status.continue_offers.length && !noticeAction && !error && !snapshotError) return null;
  const sourceLevel = (source: "microphone" | "system_audio") => status.source_levels?.find((item) => item.source === source);
  const sourceDevice = (source: "microphone" | "system_audio") => status.source_devices?.find((item) => item.source === source)?.device;

  return <section aria-label="Recording controls">
    {capturing && <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-record/25 bg-record-soft px-5 py-2.5">
      <p className="m-0 inline-flex items-center gap-2 whitespace-nowrap text-[13px] font-semibold text-record" role="status" aria-live="polite">
        <span aria-hidden="true" className={`size-2 shrink-0 rounded-full bg-record ${status.phase === "recording" ? "animate-[recording-pulse_2s_ease-in-out_infinite] motion-reduce:animate-none" : ""}`} />
        {status.phase === "paused" ? "Paused" : "Recording"} <time aria-live="off" className="ml-2 text-[15px] font-semibold tabular-nums text-ink">{formatElapsed(status.elapsed_ms)}</time>
      </p>
      <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1 text-xs text-ink-muted" aria-label="Recording Sources">
        <span className="inline-flex min-w-0 items-center gap-2"><Mic size={16} strokeWidth={1.5} aria-hidden="true" />Microphone{sourceDevice("microphone") && <span className="max-w-36 truncate text-ink" title={sourceDevice("microphone")}>{sourceDevice("microphone")}</span>}<LevelMeter level={sourceLevel("microphone")?.level ?? 0} label="Microphone" />{status.phase === "recording" && sourceLevel("microphone")?.silent && <span className="font-medium text-warn">Silent</span>}</span>
        <span className="inline-flex min-w-0 items-center gap-2"><Speaker size={16} strokeWidth={1.5} aria-hidden="true" />System audio{sourceDevice("system_audio") && <span className="max-w-36 truncate text-ink" title={sourceDevice("system_audio")}>{sourceDevice("system_audio")}</span>}<LevelMeter level={sourceLevel("system_audio")?.level ?? 0} label="System audio" />{status.phase === "recording" && sourceLevel("system_audio")?.silent && <span className="font-medium text-warn">Silent</span>}</span>
      </div>
      <div className="ml-auto flex gap-2"><TooltipProvider>
        {status.phase === "recording" && <RecordingControl label="Pause" hotkey={pauseHotkey} variant="secondary" onClick={() => run(pauseRecording)}><Pause size={16} strokeWidth={1.5} aria-hidden="true" /></RecordingControl>}
        {status.phase === "paused" && <RecordingControl label="Resume" hotkey={pauseHotkey} variant="secondary" onClick={() => run(resumeRecording)}><Play size={16} strokeWidth={1.5} aria-hidden="true" /></RecordingControl>}
        <RecordingControl label="Stop" hotkey={recordHotkey} variant="record" onClick={() => run(stopRecording)}><Square size={16} strokeWidth={1.5} fill="currentColor" aria-hidden="true" /></RecordingControl>
      </TooltipProvider></div>
    </div>}
    {latestSwitch && <div role="status" className="mx-5 mt-3 flex max-w-[420px] items-start gap-2.5 rounded-md border border-line bg-raised px-3 py-2.5 text-[13px] leading-[19px] shadow-pop">
      {latestSwitch.source === "microphone" ? <Mic size={16} strokeWidth={1.5} className="mt-0.5 shrink-0 text-ink-muted" aria-hidden="true" /> : <Speaker size={16} strokeWidth={1.5} className="mt-0.5 shrink-0 text-ink-muted" aria-hidden="true" />}
      <p className="m-0 min-w-0 flex-1"><span className="font-medium">{latestSwitch.source === "microphone" ? "Microphone" : "System audio"} switched to {latestSwitch.device}</span><br /><span className="text-ink-muted">Recording continued. The switch is marked on the Timeline at {formatElapsed(latestSwitch.at_ms)}.</span></p>
      <Button variant="ghost" size="icon" className="size-6" aria-label="Dismiss" onClick={() => setDismissedSwitches(status.device_switches.length)}><X size={16} strokeWidth={1.5} /></Button>
    </div>}
    {!capturing && !hideIdleButton && <div className="px-5 py-3"><Button variant="recordSoft" title={`Record (${recordHotkey})`} onClick={() => record(startRecording)}><span aria-hidden="true" className="size-2 rounded-full bg-current shadow-[0_0_0_3px_color-mix(in_srgb,currentColor_25%,transparent)]" />Record</Button></div>}
    {error && <div className="mx-5 my-3"><ErrorState title="Couldn't update the Recording" guidance={`${error}. Try again.`} /></div>}
    {snapshotError && <div className="mx-5 my-3"><ErrorState title="Snapshot issue" guidance={snapshotError} /><Button variant="ghost" onClick={() => setSnapshotError(null)}>Dismiss</Button></div>}
    {status.saves.map((save) => <div key={save.meeting_id} className="mx-5 my-3 rounded-md border border-line bg-raised px-3 py-2 text-sm" aria-live="polite">
      {save.state === "saving" ? <>{save.recovered ? "Recovering" : "Saving"} {save.meeting_id}…</> : <>
        <ErrorState title="Couldn't save the Recording" guidance={`The audio for ${save.meeting_id} is kept. ${save.error ?? "Try again."}`} onRetry={() => run(() => retrySave(save.meeting_id))} />
      </>}
    </div>)}
    {!capturing && status.continue_offers.map((meetingId) => <div key={meetingId} className="mx-5 my-3 rounded-md border border-line bg-raised p-3 text-sm">
      Recovered {meetingId} after the app closed unexpectedly. Continue it with a new Recording? <span className="inline-flex gap-2"><Button variant="secondary" onClick={() => record(() => continueMeeting(meetingId))}>Continue meeting</Button><Button variant="ghost" onClick={() => run(() => dismissContinue(meetingId))}>Dismiss</Button></span>
    </div>)}
    <Dialog open={!!noticeAction} onOpenChange={(open) => { if (!open) setNoticeAction(null); }}>
      <DialogContent><DialogTitle>Before you record</DialogTitle><DialogDescription>{RECORDING_NOTICE}</DialogDescription><div className="mt-6 flex justify-end gap-2"><Button variant="secondary" onClick={() => setNoticeAction(null)}>Cancel</Button><Button variant="record" onClick={confirmNotice}>Start recording</Button></div></DialogContent>
    </Dialog>
  </section>;
}
