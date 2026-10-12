import { useEffect, useRef, useState } from "react";
import type { TimelinePlayer } from "./timeline";
import type { Transcript } from "./processing";
import { formatElapsed } from "./recording";
import { currentSegmentId, speakerName } from "./transcript";

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

  return <ol aria-label="Segments" className="segments">
    {transcript.segments.map((segment) => {
      const current = segment.id === currentId;
      return <li key={segment.id} ref={current ? currentRow : undefined} className={current ? "current" : undefined} aria-current={current ? "true" : undefined}>
        <button type="button" disabled={!player} onClick={() => player?.playSpan(segment.start_ms, segment.end_ms)}>
          [{formatElapsed(segment.start_ms)}] {speakerName(segment.speaker)}
        </button>
        {editing === segment.id ? <div className="segment-editor">
          <label>Segment text<textarea value={text} onChange={(event) => setText(event.target.value)} onKeyDown={(event) => {
            if (event.key === "Enter" && event.ctrlKey) { event.preventDefault(); void save(); }
            if (event.key === "Escape") cancel();
          }} /></label>
          <button type="button" disabled={!editable || !text.trim() || saving} onClick={() => void save()}>Save</button>
          <button type="button" onClick={cancel}>Cancel</button>
          {error && <p role="alert">{error}</p>}
        </div> : <><span>{segment.text}</span><button type="button" disabled={!editable} onClick={() => { setEditing(segment.id); setText(segment.text); setError(null); }}>Edit</button></>}
      </li>;
    })}
  </ol>;
}
