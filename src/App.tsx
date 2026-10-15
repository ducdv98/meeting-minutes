import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { CircleHelp, Ellipsis, Import, Pencil, Search, Settings, Trash2, RefreshCw } from "lucide-react";
import { dismissImportErrors, importStatus, onImportHover, onImportStatus, pickAudioFiles, type ImportStatus } from "./importing";
import { Wordmark } from "./components/Brand";
import { PageHeader } from "./components/PageHeader";
import { NotificationArea } from "./components/NotificationArea";
import { deleteMeeting, meetingDetail, renameMeeting, rescanLibrary, type MalformedFolder, type MeetingSummary, type SearchHit } from "./library";
import { Button } from "./components/ui/Button";
import { Input } from "./components/ui/Input";
import { Kbd, KeyCombo } from "./components/ui/Kbd";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "./components/ui/DropdownMenu";
import { EmptyState } from "./components/states/EmptyState";
import { ErrorState } from "./components/states/ErrorState";
import { Loading } from "./components/states/Loading";
import { MeetingRowMotion } from "./components/ui/MeetingRowMotion";
import MeetingView from "./MeetingView";
import LibrarySearch from "./LibrarySearch";
import LibraryTable from "./LibraryTable";
import ConfirmDelete from "./ConfirmDelete";
import RecordingBar from "./RecordingBar";
import RecordingScreen from "./RecordingScreen";
import SettingsView, { type SettingsTabId } from "./SettingsView";
import HelpView from "./HelpView";
import { onLibraryChanged, onRecordingStatus, recordHotkey, recordingStatus, type RecorderStatus } from "./recording";
import { cancelProcessing, onProcessingStatus, processingStageLabel, processingStatus, type ProcessingJob } from "./processing";
import { meetingLanguageLabel } from "./languages";

function groupName(value: string) {
  const date = new Date(value);
  const today = new Date();
  const day = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const meetingDay = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((day - meetingDay) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 8) return "Last week";
  return new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" }).format(date);
}

function rowTime(value: string) {
  const date = new Date(value);
  if (groupName(value) === "Today" || groupName(value) === "Yesterday") {
    return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit", hour12: false }).format(date);
  }
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(date);
}

function durationLabel(ms: number) {
  const totalMinutes = Math.floor(ms / 60_000);
  if (totalMinutes < 60) return `${totalMinutes} min`;
  return `${Math.floor(totalMinutes / 60)} h ${String(totalMinutes % 60).padStart(2, "0")} min`;
}

function MainViewTransition({ viewKey, children }: { viewKey: string; children: ReactNode }) {
  const current = useRef({ key: viewKey, content: children });
  const [views, setViews] = useState([{ key: viewKey, content: children }]);
  const [entered, setEntered] = useState(true);
  if (current.current.key === viewKey) current.current.content = children;

  useLayoutEffect(() => {
    if (current.current.key === viewKey) return;
    const outgoing = current.current;
    current.current = { key: viewKey, content: children };
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) { setViews([{ key: viewKey, content: children }]); setEntered(true); return; }
    setViews([outgoing, { key: viewKey, content: children }]);
    setEntered(false);
    const frame = window.requestAnimationFrame(() => setEntered(true));
    const timer = window.setTimeout(() => setViews([{ key: viewKey, content: children }]), 180);
    return () => { window.cancelAnimationFrame(frame); window.clearTimeout(timer); };
  }, [viewKey]);

  const transition = "opacity var(--duration-panel) var(--ease-functional)";
  return <div className="relative flex min-h-0 flex-1 flex-col">
    {views.map((view) => view.key === viewKey ? <div key={view.key} className="flex min-h-0 flex-1 flex-col overflow-y-auto" style={{ opacity: entered ? 1 : 0, transition }}>{children}</div>
      : <div key={view.key} aria-hidden="true" inert className="pointer-events-none absolute inset-0 flex flex-col overflow-hidden" style={{ opacity: entered ? 0 : 1, transition }}>{view.content}</div>)}
  </div>;
}

export default function App() {
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const detailCache = useRef<Record<string, { fingerprint: string; language: string | null; imported: boolean }>>({});
  const [details, setDetails] = useState<Record<string, { language: string | null; imported: boolean }>>({});
  const scanVersion = useRef(0);
  const [malformed, setMalformed] = useState<MalformedFolder[]>([]);
  const [failed, setFailed] = useState(false);
  const [scan, setScan] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [openAt, setOpenAt] = useState<Pick<SearchHit, "segment_id" | "revision" | "start_ms" | "end_ms"> | null>(null);
  const [deleting, setDeleting] = useState<MeetingSummary | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<{ id: string; title: string; saving: boolean; error: string | null } | null>(null);
  const [panel, setPanel] = useState<"settings" | "help" | null>(null);
  const [settingsTab, setSettingsTab] = useState<SettingsTabId | undefined>();
  const [showSearch, setShowSearch] = useState(false);
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const [recordRequest, setRecordRequest] = useState(0);
  const [showRecording, setShowRecording] = useState(false);
  const [jobs, setJobs] = useState<ProcessingJob[]>([]);
  const [recorder, setRecorder] = useState<RecorderStatus | null>(null);
  const [imports, setImports] = useState<ImportStatus>({ current: null, queued: 0, errors: [] });
  const [importHover, setImportHover] = useState(false);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    recordingStatus().then((status) => { if (active && !eventArrived) setRecorder(status); }, () => {});
    const listener = onRecordingStatus((status) => { if (active) { eventArrived = true; setRecorder(status); } });
    return () => { active = false; void listener.then((stop) => stop(), () => {}); };
  }, []);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    processingStatus().then((status) => { if (active && !eventArrived) setJobs(status.jobs); }, () => {});
    const listener = onProcessingStatus((status) => { if (active) { eventArrived = true; setJobs(status.jobs); } });
    return () => { active = false; void listener.then((stop) => stop(), () => {}); };
  }, []);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    importStatus().then((status) => { if (active && !eventArrived) setImports(status); }, () => {});
    const statusListener = onImportStatus((status) => { if (active) { eventArrived = true; setImports(status); } });
    const hoverListener = onImportHover((hover) => { if (active) setImportHover(hover); });
    return () => { active = false; void statusListener.then((stop) => stop(), () => {}); void hoverListener.then((stop) => stop(), () => {}); };
  }, []);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        if (document.querySelector('[role="dialog"], [role="alertdialog"], [role="menu"], [data-state="open"][aria-haspopup="menu"]')) return;
        event.preventDefault();
        setPanel(null);
        setShowSearch(true);
        searchRef.current?.focus();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  async function pickFiles() {
    try { await pickAudioFiles(); }
    catch (reason) { setImports((current) => ({ ...current, errors: [...current.errors, String(reason)] })); }
  }

  async function dismissErrors() {
    try { await dismissImportErrors(); }
    catch (reason) { setImports((current) => ({ ...current, errors: [...current.errors, String(reason)] })); }
  }

  async function cancelJob(meetingId: string) {
    const status = await cancelProcessing(meetingId);
    setJobs(status.jobs);
  }

  async function confirmDelete() {
    if (!deleting || deleteBusy) return;
    setDeleteBusy(true);
    setDeleteError(null);
    try {
      await deleteMeeting(deleting.id);
      if (selectedId === deleting.id) setSelectedId(null);
      setDeleting(null);
      setScan((value) => value + 1);
    } catch (reason) { setDeleteError(String(reason)); }
    finally { setDeleteBusy(false); }
  }

  async function saveRename() {
    if (!renaming || renaming.saving || !renaming.title.trim()) return;
    const { id, title } = renaming;
    setRenaming({ ...renaming, saving: true, error: null });
    try {
      await renameMeeting(id, title.trim());
      setRenaming(null);
      setScan((value) => value + 1);
    } catch (reason) { setRenaming({ id, title, saving: false, error: String(reason) }); }
  }

  useEffect(() => {
    let active = true;
    const version = ++scanVersion.current;
    setFailed(false);
    rescanLibrary().then(
      (report) => {
        if (!active || version !== scanVersion.current) return;
        setMeetings(report.meetings);
        setMalformed(report.malformed);
        const ids = new Set(report.meetings.map((meeting) => meeting.id));
        for (const id of Object.keys(detailCache.current)) if (!ids.has(id)) delete detailCache.current[id];
        setDetails(Object.fromEntries(Object.entries(detailCache.current).map(([id, value]) => [id, value])));
        for (const meeting of report.meetings) {
          const fingerprint = `${meeting.title}:${meeting.started_at}:${meeting.duration_ms}`;
          if (detailCache.current[meeting.id]?.fingerprint === fingerprint) continue;
          void meetingDetail(meeting.id).then((detail) => {
            if (!active || version !== scanVersion.current) return;
            detailCache.current[meeting.id] = { fingerprint, language: detail.language, imported: detail.imported };
            setDetails((current) => ({ ...current, [meeting.id]: detailCache.current[meeting.id] }));
          }, () => {});
        }
      },
      () => { if (active && version === scanVersion.current) setFailed(true); },
    );
    return () => { active = false; };
  }, [scan]);

  useEffect(() => {
    const unlisten = onLibraryChanged(() => setScan((value) => value + 1));
    return () => { unlisten.then((stop) => stop(), () => {}); };
  }, []);

  function openMeeting(id: string) {
    setShowRecording(false);
    setSelectedId(id);
    setPanel(null);
    setShowSearch(false);
    setOpenAt(null);
  }

  function openHit(hit: SearchHit) {
    setShowRecording(false);
    setSelectedId(hit.meeting_id);
    setPanel(null);
    setShowSearch(false);
    setOpenAt({ segment_id: hit.segment_id, revision: hit.revision, start_ms: hit.start_ms, end_ms: hit.end_ms });
  }

  function requestRecording() { setSelectedId(null); setShowSearch(false); setPanel(null); setShowRecording(true); setRecordRequest((value) => value + 1); }
  const sorted = [...(meetings ?? [])].sort((a, b) => b.started_at.localeCompare(a.started_at));
  let previousGroup = "";
  const busyRecording = recorder?.phase !== "idle" && recorder?.phase != null;
  const notifications = <NotificationArea importing={imports.current} queued={imports.queued} importErrors={imports.errors} deleteError={deleting ? null : deleteError} malformed={malformed}
    onDismissImportErrors={() => void dismissErrors()} onDismissDeleteError={() => setDeleteError(null)} onDismissMalformed={() => setMalformed([])} />;
  const viewKey = panel ?? (showSearch ? "search" : selectedId ? `meeting:${selectedId}` : showRecording && busyRecording ? "recording" : "library");
  const pageHeader = panel === "settings" ? null
    : panel === "help" ? <PageHeader title="Help" context="App information, shortcuts and diagnostics." />
    : showSearch ? <PageHeader title="Search" context="Find a Segment across Transcripts in your Library." />
    : <PageHeader title="Library" context="Your Meetings, kept on this PC." actions={<Button variant="secondary" onClick={() => void pickFiles()}><Import size={16} strokeWidth={1.5} />Import a file</Button>} />;
  const view = panel === "settings" ? <SettingsView initialTab={settingsTab} /> : panel === "help" ? <HelpView /> : showSearch ? <LibrarySearch query={query} onQueryChange={setQuery} onOpen={openHit} /> : selectedId ? <MeetingView key={selectedId} id={selectedId} recordingMeetingId={busyRecording ? recorder?.meeting_id : null} openAt={openAt} onOpened={() => setOpenAt(null)} onDeleted={() => { setSelectedId(null); setOpenAt(null); setScan((value) => value + 1); }} onRenamed={() => setScan((value) => value + 1)} onOpenSettings={(tab) => { setSettingsTab(tab); setPanel("settings"); }} onBack={() => { setSelectedId(null); setOpenAt(null); setScan((value) => value + 1); }} notifications={notifications} /> : showRecording && busyRecording && recorder ? <RecordingScreen status={recorder} title={meetings?.find((meeting) => meeting.id === recorder.meeting_id)?.title} onRenamed={() => setScan((value) => value + 1)} /> : <section className="flex min-h-0 flex-1 flex-col" aria-label="Library home">
    {failed ? <div className="grid flex-1 place-items-center p-10"><ErrorState title="Couldn't load the Library" guidance="Try rescanning the Library." onRetry={() => setScan((value) => value + 1)} /></div> : meetings === null ? <div className="grid flex-1 place-items-center p-10"><Loading message="Loading the Library…" /></div> : meetings.length === 0 ? <div className="grid flex-1 place-items-center p-10"><div className="grid justify-items-center gap-3"><EmptyState title="Your Library is empty" description={<>Record a call from any app, or import an audio file. Meetings are saved to <span className="text-ink">Documents\Meeting Minutes</span>.</>} action={<><Button variant="recordSoft" disabled={!recorder || busyRecording} onClick={requestRecording}><span aria-hidden="true" className="size-2 rounded-full bg-current shadow-[0_0_0_3px_color-mix(in_srgb,currentColor_25%,transparent)]" />Record</Button><Button variant="secondary" onClick={() => void pickFiles()}><Import size={16} strokeWidth={1.5} />Import a file</Button></>} /><p className="m-0 text-center text-xs text-ink-muted">Or press <KeyCombo keys={recordHotkey} className="mx-0.5 align-middle" /> from any app.</p></div></div> : <LibraryTable meetings={meetings} details={details} jobs={jobs} recordingMeetingId={busyRecording ? recorder?.meeting_id ?? null : null} onOpen={openMeeting} onRename={(meeting) => setRenaming({ id: meeting.id, title: meeting.title, saving: false, error: null })} onDelete={(meeting) => { setDeleteError(null); setDeleting(meeting); }} onCancelProcessing={(id) => void cancelJob(id)} />}
  </section>;

  return <main className="grid h-screen min-h-[480px] grid-cols-[248px_minmax(0,1fr)] bg-canvas text-ink">
    <aside aria-label="Library sidebar" className="flex min-h-0 flex-col border-r border-line bg-panel">
      <div className="grid gap-3 px-3 pb-2 pt-4">
        <div className="flex min-w-0 items-center gap-2">
          <button type="button" className="w-fit shrink-0 rounded-sm" aria-label="Library home" onClick={() => { setSelectedId(null); setShowRecording(false); setPanel(null); setShowSearch(false); }}><Wordmark className="h-[22px]" /></button>
          {busyRecording && <span className="inline-flex items-center gap-1.5 text-xs font-medium text-record"><span aria-hidden="true" className={`size-2 rounded-full bg-record ${recorder?.phase === "recording" ? "animate-[recording-pulse_2s_ease-in-out_infinite] motion-reduce:animate-none" : ""}`} />{recorder?.phase === "paused" ? "Paused" : "Recording"}</span>}
        </div>
        <div className="grid grid-cols-[1fr_auto] gap-1.5">
          {busyRecording ? <Button variant="secondary" title="Open live Meeting" onClick={() => { setSelectedId(null); setShowRecording(true); setShowSearch(false); setPanel(null); }}>Live Meeting</Button> : <Button variant="recordSoft" title={`Record (${recordHotkey})`} disabled={!recorder} onClick={requestRecording}><span aria-hidden="true" className="size-2 rounded-full bg-current shadow-[0_0_0_3px_color-mix(in_srgb,currentColor_25%,transparent)]" />Record</Button>}
          <Button variant="secondary" size="icon" aria-label="Import an audio file" title="Import an audio file" onClick={() => void pickFiles()}><Import size={16} strokeWidth={1.5} /></Button>
        </div>
        <div className="flex h-8 items-center gap-2 rounded-sm border border-line bg-raised px-2.5 focus-within:border-accent">
          <Search size={16} strokeWidth={1.5} className="shrink-0 text-ink-faint" aria-hidden="true" />
          <Input ref={searchRef} type="search" aria-label="Search transcripts" placeholder="Search" value={query} onFocus={() => { setShowSearch(true); setPanel(null); }} onChange={(event) => { setQuery(event.target.value); setShowSearch(true); }} className="h-full min-w-0 border-0 bg-transparent px-0 text-sm shadow-none focus:border-0 focus:shadow-none" />
          <Kbd className="shrink-0 border-b text-[11px] text-ink-faint">Ctrl K</Kbd>
        </div>
      </div>
      <nav aria-label="Meetings" className="min-h-0 flex-1 overflow-y-auto px-2 pb-2 pt-1">
        {sorted.map((meeting) => {
          const group = groupName(meeting.started_at);
          const showGroup = group !== previousGroup;
          previousGroup = group;
          const job = jobs.find((item) => item.meeting_id === meeting.id);
          const detail = details[meeting.id];
          const language = detail?.language ? meetingLanguageLabel(detail.language) : detail?.imported ? "Imported" : "Auto-detect";
          return <MeetingRowMotion key={meeting.id} animate={scan > 0}>
            {showGroup && <h2 className="px-2 pb-1.5 pt-3.5 text-xs font-medium text-ink-faint">{group}</h2>}
            {renaming?.id === meeting.id ? <div className="grid gap-1 px-1 py-1">
              <Input autoFocus aria-label={`Title for ${meeting.title}`} className="h-8 text-[13px] font-medium" value={renaming.title} disabled={renaming.saving} onFocus={(event) => event.target.select()} onChange={(event) => setRenaming({ ...renaming, title: event.target.value })} onBlur={() => { if (!renaming.saving && !renaming.error) setRenaming(null); }} onKeyDown={(event) => {
                if (event.key === "Enter") { event.preventDefault(); void saveRename(); }
                if (event.key === "Escape") setRenaming(null);
              }} />
              {renaming.error && <p role="alert" className="m-0 px-1 text-xs text-record">{renaming.error}</p>}
            </div> : <div className={`group relative rounded-sm ${selectedId === meeting.id && panel === null && !showSearch ? "bg-accent-soft hover:bg-accent-soft" : "hover:bg-hover"}`}>
              <button type="button" aria-label={meeting.title} aria-current={selectedId === meeting.id && panel === null && !showSearch ? "page" : undefined} onClick={() => openMeeting(meeting.id)} className="grid w-full grid-cols-[minmax(0,1fr)_auto] gap-x-2 gap-y-0.5 rounded-sm px-2 py-2 pr-8 text-left">
                <span className="min-w-0 truncate text-[13px] font-medium leading-[18px]">{meeting.title}</span>
                <time dateTime={meeting.started_at} className="text-xs tabular-nums leading-4 text-ink-faint">{rowTime(meeting.started_at)}</time>
                <span className={`col-span-2 text-xs leading-4 ${job ? "text-warn" : "text-ink-muted"}`}>
                  {job ? processingStageLabel(job).replace(/ (\d+%)/, " · $1") : <>{meeting.duration_ms > 0 && durationLabel(meeting.duration_ms)}{meeting.duration_ms > 0 && language && " · "}{language}</>}
                </span>
              </button>
              <DropdownMenu><DropdownMenuTrigger asChild><Button variant="ghost" size="icon" className="absolute right-0.5 top-1 size-7 opacity-0 focus:opacity-100 group-hover:opacity-100 data-[state=open]:opacity-100" aria-label={`More options for ${meeting.title}`}><Ellipsis size={16} strokeWidth={1.5} /></Button></DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  {job && job.stage !== "failed" && <DropdownMenuItem onSelect={() => void cancelJob(meeting.id)}>Cancel processing</DropdownMenuItem>}
                  <DropdownMenuItem onSelect={() => setRenaming({ id: meeting.id, title: meeting.title, saving: false, error: null })}><Pencil size={16} strokeWidth={1.5} />Rename {meeting.title}</DropdownMenuItem>
                  <DropdownMenuItem disabled={busyRecording && recorder?.meeting_id === meeting.id} onSelect={() => { setDeleteError(null); setDeleting(meeting); }} className="text-record"><Trash2 size={16} strokeWidth={1.5} />Delete {meeting.title}</DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>}
          </MeetingRowMotion>;
        })}
      </nav>
      <div className="grid gap-0.5 border-t border-line p-2">
        <Button variant="ghost" className="w-full justify-start gap-2.5 px-2" aria-current={panel === "settings" ? "page" : undefined} onClick={() => { setSelectedId(null); setShowSearch(false); setSettingsTab(undefined); setPanel("settings"); }}><Settings size={16} strokeWidth={1.5} />Settings</Button>
        <Button variant="ghost" className="w-full justify-start gap-2.5 px-2" aria-current={panel === "help" ? "page" : undefined} onClick={() => { setSelectedId(null); setShowSearch(false); setPanel("help"); }}><CircleHelp size={16} strokeWidth={1.5} />Help</Button>
        <DropdownMenu><DropdownMenuTrigger asChild><Button variant="ghost" className="w-full justify-start gap-2.5 px-2"><Ellipsis size={16} strokeWidth={1.5} />More</Button></DropdownMenuTrigger><DropdownMenuContent align="start"><DropdownMenuItem onSelect={() => setScan((value) => value + 1)}><RefreshCw size={16} strokeWidth={1.5} />Rescan Library</DropdownMenuItem></DropdownMenuContent></DropdownMenu>
      </div>
    </aside>
    <div className="flex min-h-0 min-w-0 flex-col overflow-hidden bg-canvas">
      <RecordingBar recordRequest={recordRequest} hideIdleButton />
      <MainViewTransition viewKey={viewKey}>{!panel && !showSearch && (selectedId || showRecording && busyRecording) ? view : <>{pageHeader}{notifications}{view}</>}</MainViewTransition>
    </div>
    {deleting && <ConfirmDelete title={deleting.title} busy={deleteBusy} onConfirm={() => void confirmDelete()} onCancel={() => setDeleting(null)} error={deleteError} />}
    {importHover && <div role="status" className="pointer-events-none fixed inset-0 z-50 grid place-items-center border-4 border-dashed border-accent bg-canvas/95 p-10 text-center text-heading-md text-ink">Drop audio to import (mp3, m4a, wav, flac)</div>}
  </main>;
}
