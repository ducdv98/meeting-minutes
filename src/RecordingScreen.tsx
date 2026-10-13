import { useEffect, useState } from "react";
import { Mic, Pencil, Speaker } from "lucide-react";
import { renameMeeting } from "./library";
import { formatElapsed, type RecorderStatus } from "./recording";
import { Button } from "./components/ui/Button";
import { Input } from "./components/ui/Input";

export default function RecordingScreen({ status, title, onRenamed }: { status: RecorderStatus; title?: string; onRenamed(): void }) {
  const [currentTitle, setCurrentTitle] = useState(title ?? "Untitled meeting");
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { setCurrentTitle(title ?? "Untitled meeting"); }, [title, status.meeting_id]);
  async function saveTitle() {
    if (!status.meeting_id || !draft.trim() || saving) return;
    setSaving(true); setError(null);
    try {
      const summary = await renameMeeting(status.meeting_id, draft.trim());
      setCurrentTitle(summary.title); setEditing(false); onRenamed();
    } catch (reason) { setError(String(reason)); }
    finally { setSaving(false); }
  }
  const device = (source: "microphone" | "system_audio") => status.source_devices?.find((item) => item.source === source)?.device;
  const latestSwitch = status.device_switches.at(-1);
  return <section aria-label="Live Recording" className="grid min-h-0 flex-1 place-items-center p-10">
    <div className="grid max-w-[440px] justify-items-center gap-3 text-center">
      <span className="inline-flex h-[26px] items-center gap-1.5 rounded-full border border-line bg-raised px-2.5 text-xs text-ink-muted"><span aria-hidden="true" className={`size-2 rounded-full bg-record ${status.phase === "recording" ? "animate-[recording-pulse_2s_ease-in-out_infinite] motion-reduce:animate-none" : ""}`} />{status.phase === "paused" ? "Paused" : `Recording ${currentTitle}`}</span>
      {editing ? <div className="flex items-center gap-2"><label htmlFor="recording-title" className="sr-only">Title</label><Input id="recording-title" autoFocus value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); void saveTitle(); } if (event.key === "Escape") setEditing(false); }} /><Button disabled={!draft.trim() || saving} onClick={() => void saveTitle()}>Save</Button><Button variant="ghost" onClick={() => setEditing(false)}>Cancel</Button></div> : <button type="button" className="flex items-center gap-2 font-display text-heading-md" aria-label={`Rename ${currentTitle}`} onClick={() => { setDraft(currentTitle); setError(null); setEditing(true); }}>{currentTitle}<Pencil size={16} strokeWidth={1.5} aria-hidden="true" /></button>}
      {error && <p role="alert" className="m-0 text-sm text-record">{error}</p>}
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
