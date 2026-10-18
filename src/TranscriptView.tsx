import { useEffect, useRef, useState } from "react";
import type { TimelinePlayer } from "./timeline";
import type { Speaker, Transcript } from "./processing";
import { formatElapsed } from "./recording";
import { currentSegmentId, participantName, speakerName } from "./transcript";
import { participantList } from "./transcript";
import type { MeetingDetail } from "./library";
import { CircleHelp } from "lucide-react";
import { Button } from "./components/ui/Button";
import { Textarea } from "./components/ui/Textarea";
import type { SnapshotWithImage } from "./snapshots";

interface Props {
  transcript: Transcript;
  positionMs: number;
  player: TimelinePlayer | null;
  editable: boolean;
  onEdit(segmentId: string, text: string): Promise<void>;
  onMove?(segmentIds: string[], name: string): Promise<void>;
  participants?: Partial<Record<Speaker, string>>;
  segmentParticipants?: MeetingDetail["segment_participants"];
  yourName?: string | null;
  isVoiceNote?: boolean;
  skipInitialScroll?: boolean;
  snapshots?: SnapshotWithImage[];
  onOpenSnapshot?(order: number): void;
}

export default function TranscriptView({ transcript, positionMs, player, editable, onEdit, onMove, participants = {}, segmentParticipants, yourName = null, isVoiceNote = false, skipInitialScroll = false, snapshots = [], onOpenSnapshot }: Props) {
  const [editing, setEditing] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [newParticipant, setNewParticipant] = useState(false);
  const [newName, setNewName] = useState("");
  const currentId = currentSegmentId(transcript.segments, positionMs);
  const currentRow = useRef<HTMLLIElement>(null);
  const firstScroll = useRef(true);
  const timelineItems = [
    ...transcript.segments.map((segment) => ({ kind: "segment" as const, at: segment.start_ms, order: 0, segment })),
    ...snapshots.map((snapshot) => ({ kind: "snapshot" as const, at: snapshot.timeline_ms, order: snapshot.order, snapshot })),
  ].sort((a, b) => a.at - b.at || (a.kind === b.kind ? a.order - b.order : a.kind === "segment" ? -1 : 1));

  useEffect(() => {
    if (firstScroll.current) { firstScroll.current = false; if (skipInitialScroll) return; }
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

  async function move(name: string) {
    if (!editable || !onMove || !name.trim() || selected.length === 0 || saving) return;
    setSaving(true); setError(null);
    try { await onMove(selected, name.trim()); setSelected([]); setNewParticipant(false); setNewName(""); }
    catch (reason) { setError(String(reason)); }
    finally { setSaving(false); }
  }

  return <><div className="sticky top-11 z-10 bg-canvas">{selected.length > 0 && <div className="mb-2 flex flex-wrap items-center gap-2 rounded-md border border-line bg-raised p-2 text-xs"><span>{selected.length} selected</span><span aria-hidden="true">·</span><label>Move to <select aria-label="Move to Participant" className="rounded-sm border border-line-strong bg-raised px-2 py-1" value={newParticipant ? "__new__" : ""} disabled={!editable || saving} onChange={(event) => { if (event.target.value === "__new__") setNewParticipant(true); else void move(event.target.value); }}><option value="">Participant</option>{participantList(transcript, participants, segmentParticipants, yourName, isVoiceNote).map((name) => <option key={name} value={name}>{name}</option>)}<option value="__new__">New Participant…</option></select></label>{newParticipant && <><input autoFocus aria-label="New Participant name" className="rounded-sm border border-line-strong bg-raised px-2 py-1" value={newName} onChange={(event) => setNewName(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void move(newName); }} /><Button disabled={!newName.trim() || saving} onClick={() => void move(newName)}>Move</Button></>}<span aria-hidden="true">·</span><Button variant="ghost" onClick={() => { setSelected([]); setNewParticipant(false); }}>Clear</Button></div>}</div><ol aria-label="Segments" className="m-0 list-none p-0">
    {timelineItems.map((item) => {
      if (item.kind === "snapshot") {
        const { snapshot } = item;
        const label = snapshot.caption || `Snapshot at ${formatElapsed(snapshot.timeline_ms)}`;
        return <li key={`snapshot-${snapshot.order}`} className="-mx-2 rounded-sm px-2 py-3">
          <button type="button" aria-label={`Open Snapshot: ${label}`} onClick={() => onOpenSnapshot?.(snapshot.order)} className="ml-[30px] block max-w-[360px] overflow-hidden rounded-md border border-line bg-raised text-left hover:border-accent focus-visible:outline-2 focus-visible:outline-accent">
            <img src={snapshot.image_url} alt={label} className="max-h-44 w-full object-contain" />
            <span className="block px-3 py-2 text-xs text-ink-muted">{snapshot.caption || `Snapshot · ${formatElapsed(snapshot.timeline_ms)}`}</span>
          </button>
        </li>;
      }
      const { segment } = item;
      const current = segment.id === currentId;
      const elapsed = formatElapsed(segment.start_ms);
      const time = elapsed.startsWith("00:") ? elapsed.slice(3) : elapsed;
      const name = participantName(segment, transcript.revision, participants, segmentParticipants, yourName, isVoiceNote);
      const defaultUnknown = !isVoiceNote && segment.speaker === "unknown" && name === "Unknown";
      return <li key={segment.id} ref={current ? currentRow : undefined} data-segment-start={segment.start_ms} className={`group -mx-2 grid grid-cols-[18px_52px_68px_minmax(0,1fr)] gap-3 rounded-sm px-2 py-2 transition-colors duration-panel hover:bg-panel ${current ? "current bg-accent-soft hover:bg-accent-soft" : ""}`} aria-current={current ? "true" : undefined}>
        {onMove ? <input type="checkbox" aria-label={`Select Segment ${segment.id}`} disabled={!editable || !onMove} checked={selected.includes(segment.id)} onChange={(event) => setSelected((current) => event.target.checked ? [...current, segment.id] : current.filter((id) => id !== segment.id))} className={`mt-1 accent-accent ${selected.length ? "opacity-100" : "opacity-0 group-hover:opacity-100 focus:opacity-100"}`} /> : <span aria-hidden="true" />}
        <button type="button" disabled={!player} className="self-start pt-[3px] text-left text-xs tabular-nums text-ink-faint hover:text-accent disabled:opacity-50" aria-label={`Play Segment at ${time}, ${name}`} onClick={() => player?.playSpan(segment.start_ms, segment.end_ms)}>{time}</button>
        <span title={`Speaker label: ${speakerName(segment.speaker)}`} className={`flex items-start gap-1 pt-[3px] text-xs font-medium ${defaultUnknown ? "text-ink-faint" : segment.speaker === "me" ? "text-ink" : "text-ink-muted"}`}>{defaultUnknown && <CircleHelp size={13} strokeWidth={1.5} className="mt-0.5" aria-hidden="true" />}{name}</span>
        {editing === segment.id ? <div className="min-w-0">
          <label className="block text-xs text-ink-muted">Segment text<Textarea autoFocus className="mt-1 min-h-[78px] text-read leading-[26px]" value={text} onChange={(event) => setText(event.target.value)} onKeyDown={(event) => {
            if (event.key === "Enter" && event.ctrlKey) { event.preventDefault(); void save(); }
            if (event.key === "Escape") cancel();
          }} /></label>
          <div className="mt-2 flex gap-2"><Button disabled={!editable || !text.trim() || saving} onClick={() => void save()}>Save</Button><Button variant="ghost" onClick={cancel}>Cancel</Button></div>
          {error && <p role="alert" className="mt-2 text-sm text-record">{error}</p>}
        </div> : <div className="flex min-w-0 items-start gap-2"><p className={`m-0 min-w-0 flex-1 text-read leading-[26px] ${defaultUnknown ? "text-ink-muted" : ""}`}>{segment.text}</p><Button variant="ghost" size="icon" className="h-7 w-auto px-1 text-xs opacity-0 group-hover:opacity-100 focus:opacity-100" disabled={!editable} onClick={() => { setEditing(segment.id); setText(segment.text); setError(null); }}>Edit</Button></div>}
      </li>;
    })}
  </ol>{error && !editing && <p role="alert" className="text-sm text-record">{error}</p>}</>;
}
