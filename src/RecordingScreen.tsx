import { Mic, Speaker } from "lucide-react";
import { formatElapsed, type RecorderStatus } from "./recording";

export default function RecordingScreen({ status, title }: { status: RecorderStatus; title?: string }) {
  const device = (source: "microphone" | "system_audio") => status.source_devices?.find((item) => item.source === source)?.device;
  const latestSwitch = status.device_switches.at(-1);
  return <section aria-label="Live Recording" className="grid min-h-0 flex-1 place-items-center p-10">
    <div className="grid max-w-[440px] justify-items-center gap-3 text-center">
      <span className="inline-flex h-[26px] items-center gap-1.5 rounded-full border border-line bg-raised px-2.5 text-xs text-ink-muted"><span aria-hidden="true" className={`size-2 rounded-full bg-record ${status.phase === "recording" ? "animate-[recording-pulse_2s_ease-in-out_infinite] motion-reduce:animate-none" : ""}`} />{status.phase === "paused" ? "Paused" : title ? `Recording ${title}` : "Recording a new Meeting"}</span>
      <time className="font-display text-[44px] font-semibold leading-none tracking-[-0.02em] tabular-nums">{formatElapsed(status.elapsed_ms)}</time>
      <p className="m-0 text-base text-ink-muted">The Transcript appears after you stop. Keep this window open or close it to the tray; recording continues either way.</p>
      <div className="flex flex-wrap justify-center gap-2" aria-label="Recording Sources">
        <span className="inline-flex min-h-[26px] items-center gap-1.5 rounded-full border border-line bg-raised px-2.5 text-xs text-ink-muted"><Mic size={16} strokeWidth={1.5} />{device("microphone") ?? "Microphone"}</span>
        <span className="inline-flex min-h-[26px] items-center gap-1.5 rounded-full border border-line bg-raised px-2.5 text-xs text-ink-muted"><Speaker size={16} strokeWidth={1.5} />{device("system_audio") ?? "System audio"}</span>
      </div>
      {latestSwitch && <div role="status" className="mt-3 flex max-w-[420px] items-start gap-2.5 rounded-md border border-line bg-raised px-3 py-2.5 text-left text-sm shadow-pop"><Speaker size={16} strokeWidth={1.5} className="mt-0.5 shrink-0 text-ink-muted" /><p className="m-0"><span className="font-medium">Switched to {latestSwitch.device}</span><br /><span className="text-ink-muted">Recording continued. The switch is marked on the Timeline.</span></p></div>}
    </div>
  </section>;
}
