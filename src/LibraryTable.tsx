import { useEffect, useState } from "react";
import { Ellipsis, Pencil, Trash2 } from "lucide-react";
import type { MeetingSummary } from "./library";
import type { ProcessingJob } from "./processing";
import { processingStageLabel } from "./processing";
import { meetingSummary, onSummaryStatus } from "./summary";
import { Button } from "./components/ui/Button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "./components/ui/DropdownMenu";

type SortColumn = "title" | "date" | "duration";
type Direction = "ascending" | "descending";

interface Props {
  meetings: MeetingSummary[];
  details: Record<string, { language: string | null; imported: boolean }>;
  jobs: ProcessingJob[];
  recordingMeetingId: string | null;
  onOpen: (id: string) => void;
  onRename: (meeting: MeetingSummary) => void;
  onDelete: (meeting: MeetingSummary) => void;
  onCancelProcessing: (id: string) => void;
}

function durationLabel(ms: number) {
  const totalMinutes = Math.floor(ms / 60_000);
  if (totalMinutes < 60) return `${totalMinutes} min`;
  return `${Math.floor(totalMinutes / 60)} h ${String(totalMinutes % 60).padStart(2, "0")} min`;
}

export default function LibraryTable({ meetings, details, jobs, recordingMeetingId, onOpen, onRename, onDelete, onCancelProcessing }: Props) {
  const [sort, setSort] = useState<{ column: SortColumn; direction: Direction }>({ column: "date", direction: "descending" });
  const [summaries, setSummaries] = useState<Record<string, boolean>>({});
  const [summaryVersion, setSummaryVersion] = useState(0);

  useEffect(() => {
    let active = true;
    void Promise.all(meetings.map(async (meeting) => [meeting.id, !!(await meetingSummary(meeting.id).catch(() => null))] as const)).then((entries) => {
      if (active) setSummaries(Object.fromEntries(entries));
    });
    return () => { active = false; };
  }, [meetings, summaryVersion]);

  useEffect(() => {
    const listener = onSummaryStatus(() => setSummaryVersion((value) => value + 1));
    return () => { void listener.then((stop) => stop(), () => {}); };
  }, []);

  function sortBy(column: SortColumn) {
    setSort((current) => ({ column, direction: current.column === column && current.direction === "ascending" ? "descending" : "ascending" }));
  }

  const ordered = [...meetings].sort((a, b) => {
    const comparison = sort.column === "title" ? a.title.localeCompare(b.title, undefined, { sensitivity: "base" })
      : sort.column === "date" ? a.started_at.localeCompare(b.started_at)
      : a.duration_ms - b.duration_ms;
    return (sort.direction === "ascending" ? comparison : -comparison) || b.started_at.localeCompare(a.started_at) || a.id.localeCompare(b.id);
  });

  function heading(label: string, column: SortColumn) {
    return <th scope="col" aria-sort={sort.column === column ? sort.direction : "none"} className="px-4 py-3 text-left font-medium">
      <button type="button" onClick={() => sortBy(column)} className="rounded-sm text-left hover:text-ink">{label}<span aria-hidden="true" className="ml-1 text-ink-faint">{sort.column === column ? sort.direction === "ascending" ? "↑" : "↓" : ""}</span></button>
    </th>;
  }

  return <div className="min-h-0 flex-1 overflow-auto px-7 py-6">
    <table aria-label="Meetings in the Library" className="w-full min-w-[720px] border-collapse text-sm">
      <thead className="border-b border-line text-xs text-ink-muted"><tr>
        {heading("Title", "title")}{heading("Date", "date")}{heading("Duration", "duration")}
        <th scope="col" className="px-4 py-3 text-left font-medium">Language</th>
        <th scope="col" className="px-4 py-3 text-left font-medium">Status</th>
        <th scope="col" className="w-10 px-2 py-3"><span className="sr-only">Actions</span></th>
      </tr></thead>
      <tbody>{ordered.map((meeting) => {
        const job = jobs.find((item) => item.meeting_id === meeting.id);
        const language = details[meeting.id]?.language === "en" ? "English" : details[meeting.id]?.language === "vi" ? "Tiếng Việt" : "";
        const status = job?.stage === "failed" ? "Failed" : job ? processingStageLabel(job).replace(/ (\d+%)/, " · $1") : summaries[meeting.id] ? "Summary made" : "";
        return <tr key={meeting.id} onClick={(event) => { if (!(event.target instanceof Element && event.target.closest("button"))) onOpen(meeting.id); }} className="group border-b border-line transition-colors duration-fast ease-functional hover:bg-hover">
          <th scope="row" className="max-w-0 px-4 py-3 text-left font-medium"><button type="button" onClick={() => onOpen(meeting.id)} className="block max-w-full truncate rounded-sm text-left hover:text-accent">{meeting.title}</button></th>
          <td className="whitespace-nowrap px-4 py-3 text-ink-muted"><time dateTime={meeting.started_at}>{new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric" }).format(new Date(meeting.started_at))}</time></td>
          <td className="whitespace-nowrap px-4 py-3 tabular-nums text-ink-muted">{durationLabel(meeting.duration_ms)}</td>
          <td className="whitespace-nowrap px-4 py-3 text-ink-muted">{language}</td>
          <td className={`whitespace-nowrap px-4 py-3 ${job?.stage === "failed" ? "text-record" : job ? "text-warn" : "text-ink-muted"}`}>{status}</td>
          <td className="px-2 py-1.5" onClick={(event) => event.stopPropagation()}>
            <DropdownMenu><DropdownMenuTrigger asChild><Button variant="ghost" size="icon" className="size-7 opacity-0 focus:opacity-100 group-hover:opacity-100 data-[state=open]:opacity-100" aria-label={`Table options for ${meeting.title}`}><Ellipsis size={16} strokeWidth={1.5} /></Button></DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {job && job.stage !== "failed" && <DropdownMenuItem onSelect={() => onCancelProcessing(meeting.id)}>Cancel processing</DropdownMenuItem>}
                <DropdownMenuItem onSelect={() => onRename(meeting)}><Pencil size={16} strokeWidth={1.5} />Rename {meeting.title}</DropdownMenuItem>
                <DropdownMenuItem disabled={recordingMeetingId === meeting.id} onSelect={() => onDelete(meeting)} className="text-record"><Trash2 size={16} strokeWidth={1.5} />Delete {meeting.title}</DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </td>
        </tr>;
      })}</tbody>
    </table>
  </div>;
}
