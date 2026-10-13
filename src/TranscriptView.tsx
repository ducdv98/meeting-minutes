import { useEffect, useRef, useState } from "react";
import type { TimelinePlayer } from "./timeline";
import type { Transcript } from "./processing";
import { formatElapsed } from "./recording";
import { currentSegmentId, speakerName } from "./transcript";
import { CircleHelp } from "lucide-react";
import { Button } from "./components/ui/Button";
import { Textarea } from "./components/ui/Textarea";

interface Props {
  transcript: Transcript;
  positionMs: number;
  player: TimelinePlayer | null;
  editable: boolean;
  onEdit(segmentId: string, text: string): Promise<void>;
}

export default function TranscriptView({ transcript, positionMs, player, editable, onEdit }: Props) {
  const [editing, setEditing] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const currentId = currentSegmentId(transcript.segments, positionMs);
  const currentRow = useRef<HTMLLIElement>(null);

  useEffect(() => {
    currentRow.current?.scrollIntoView?.({ block: "nearest" });
  }, [currentId]);

  async function save() {
    if (!editing || !editable || !text.trim() || saving) return;
    setSaving(true);
    setError(null);
    try {
      await onEdit(editing, text);
      setEditing(null);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  }

  function cancel() { setEditing(null); setError(null); }

  return <ol aria-label="Segments" className="m-0 list-none p-0">
    {transcript.segments.map((segment) => {
      const current = segment.id === currentId;
      const elapsed = formatElapsed(segment.start_ms);
      const time = elapsed.startsWith("00:") ? elapsed.slice(3) : elapsed;
      return <li key={segment.id} ref={current ? currentRow : undefined} className={`group -mx-2 grid grid-cols-[52px_68px_minmax(0,1fr)] gap-3 rounded-sm px-2 py-2 transition-colors duration-panel hover:bg-panel ${current ? "current bg-accent-soft hover:bg-accent-soft" : ""}`} aria-current={current ? "true" : undefined}>
        <button type="button" disabled={!player} className="self-start pt-[3px] text-left text-xs tabular-nums text-ink-faint hover:text-accent disabled:opacity-50" aria-label={`Play Segment at ${time}, ${speakerName(segment.speaker)}`} onClick={() => player?.playSpan(segment.start_ms, segment.end_ms)}>{time}</button>
        <span className={`flex items-start gap-1 pt-[3px] text-xs font-medium ${segment.speaker === "me" ? "text-ink" : segment.speaker === "remote" ? "text-ink-muted" : "text-ink-faint"}`}>{segment.speaker === "unknown" && <CircleHelp size={13} strokeWidth={1.5} className="mt-0.5" aria-hidden="true" />}{speakerName(segment.speaker)}</span>
        {editing === segment.id ? <div className="min-w-0">
          <label className="block text-xs text-ink-muted">Segment text<Textarea autoFocus className="mt-1 min-h-[78px] text-read leading-[26px]" value={text} onChange={(event) => setText(event.target.value)} onKeyDown={(event) => {
            if (event.key === "Enter" && event.ctrlKey) { event.preventDefault(); void save(); }
            if (event.key === "Escape") cancel();
          }} /></label>
          <div className="mt-2 flex gap-2"><Button disabled={!editable || !text.trim() || saving} onClick={() => void save()}>Save</Button><Button variant="ghost" onClick={cancel}>Cancel</Button></div>
          {error && <p role="alert" className="mt-2 text-sm text-record">{error}</p>}
        </div> : <div className="flex min-w-0 items-start gap-2"><p className={`m-0 min-w-0 flex-1 text-read leading-[26px] ${segment.speaker === "unknown" ? "text-ink-muted" : ""}`}>{segment.text}</p><Button variant="ghost" size="icon" className="h-7 w-auto px-1 text-xs opacity-0 group-hover:opacity-100 focus:opacity-100" disabled={!editable} onClick={() => { setEditing(segment.id); setText(segment.text); setError(null); }}>Edit</Button></div>}
      </li>;
    })}
  </ol>;
}
