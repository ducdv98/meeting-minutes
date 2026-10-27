import { useState } from "react";
import { PROVIDER_NAMES, type ProviderKind } from "./ai";
import type { PlannedMeeting } from "./libraryAsk";
import { Button } from "./components/ui/Button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./components/ui/Dialog";

export default function LibraryAskConfirmation({ provider, meetings, onSend, onCancel }: { provider: ProviderKind; meetings: PlannedMeeting[]; onSend(excludedMeetings: string[]): void; onCancel(): void }) {
  const [selected, setSelected] = useState(() => new Set(meetings.map((meeting) => meeting.meeting_id)));
  const label = PROVIDER_NAMES[provider];
  function toggle(id: string) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }
  return <Dialog open onOpenChange={(open) => { if (!open) onCancel(); }}><DialogContent aria-label={`Send to ${label}?`} className="max-h-[90vh] overflow-y-auto">
    <DialogTitle>Send to {label}?</DialogTitle>
    <DialogDescription>Your question was sent to {label} to find relevant Meetings. Excerpts from the ticked Meetings will be sent next.</DialogDescription>
    <ul aria-label="Meetings" className="mt-5 max-h-72 list-none overflow-y-auto p-0">{meetings.map((meeting) => <li key={meeting.meeting_id} className="border-b border-line py-2"><label className="flex items-start gap-3 text-sm"><input type="checkbox" checked={selected.has(meeting.meeting_id)} onChange={() => toggle(meeting.meeting_id)} className="mt-1" /><span><span className="block font-medium">{meeting.title}</span><span className="text-ink-muted">{new Date(meeting.started_at).toLocaleDateString()} · {meeting.excerpts} {meeting.excerpts === 1 ? "excerpt" : "excerpts"}</span></span></label></li>)}</ul>
    <div className="mt-5 flex justify-end gap-2"><Button variant="secondary" onClick={onCancel}>Cancel</Button><Button disabled={!selected.size} onClick={() => onSend(meetings.filter((meeting) => !selected.has(meeting.meeting_id)).map((meeting) => meeting.meeting_id))}>Send</Button></div>
  </DialogContent></Dialog>;
}
