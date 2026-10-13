import { useCallback, useEffect, useRef, useState } from "react";
import { assignSegments, deleteMeeting, meetingDetail, openMeetingFolder, renameMeeting, renameParticipant, removeParticipant, participantNames, type MeetingDetail, type SearchHit } from "./library";
import Player, { PlayerContext, type PlayerHandle } from "./Player";
import type { TimelinePlayer } from "./timeline";
import { formatElapsed } from "./recording";
import { onLibraryChanged } from "./recording";
import { cancelProcessing, dismissProcessing, enqueueTranscription, getSettings, meetingTranscript, MODELS, onProcessingStatus, onTranscriptDraft, processingStageLabel, processingStatus, setMeetingLanguage, transcriptDraft, type DraftSegment, type Language, type ProcessingJob, type Speaker, type Transcript } from "./processing";
import TranscriptView from "./TranscriptView";
import DraftView from "./DraftView";
import { editSegment, labelParticipantName, participantList, speakerName } from "./transcript";
import { aiStatus, type AiStatus } from "./ai";
import AiSetupGuidance from "./AiSetupGuidance";
import ConfirmDelete from "./ConfirmDelete";
import SummaryPanel from "./SummaryPanel";
import AskPanel from "./AskPanel";
import { Clock3, Ellipsis, FolderOpen, Globe2, Mic, RefreshCw, Trash2, Pencil, ChevronLeft } from "lucide-react";
import { Button } from "./components/ui/Button";
import { Badge } from "./components/ui/Badge";
import { Input } from "./components/ui/Input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/Tabs";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "./components/ui/DropdownMenu";
import { EmptyState } from "./components/states/EmptyState";
import { ErrorState } from "./components/states/ErrorState";
import { Loading } from "./components/states/Loading";
import type { Citation } from "./summary";

interface Props {
  id: string;
  onBack(): void;
  onRenamed(): void;
  onOpenSettings(): void;
  openAt?: Pick<SearchHit, "segment_id" | "revision" | "start_ms" | "end_ms"> | null;
  onOpened?(): void;
  onDeleted?(): void;
  recordingMeetingId?: string | null;
}

function languageName(code: string | null): string {
  if (code === "en") return "English";
  if (code === "vi") return "Vietnamese";
  return code ?? "Not set";
}

function meetingDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value.slice(0, 16).replace("T", " ");
  const today = new Date();
  const day = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const meetingDay = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((day - meetingDay) / 86_400_000);
  const label = days === 0 ? "Today" : days === 1 ? "Yesterday" : new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" }).format(date);
  const time = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit", hour12: false }).format(date);
  return `${label}, ${time}`;
}

const draftKey = (segment: DraftSegment) => `${segment.start_ms}:${segment.end_ms}:${segment.speaker}:${segment.text}`;

/** Adds Draft Segments in time order, skipping ones already shown (the stored Draft and live events overlap). */
function mergeDraft(current: DraftSegment[], incoming: DraftSegment[]): DraftSegment[] {
  const seen = new Set(current.map(draftKey));
  const added = incoming.filter((segment) => { const key = draftKey(segment); if (seen.has(key)) return false; seen.add(key); return true; });
  return added.length ? [...current, ...added].sort((a, b) => a.start_ms - b.start_ms || a.end_ms - b.end_ms) : current;
}

export default function MeetingView({ id, onBack, onRenamed, onOpenSettings, openAt, onOpened, onDeleted, recordingMeetingId }: Props) {
  const [providerStatus, setProviderStatus] = useState<AiStatus | null>(null);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [yourName, setYourName] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [participantDrafts, setParticipantDrafts] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [staleSearchResult, setStaleSearchResult] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [saving, setSaving] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [audioOpen, setAudioOpen] = useState(true);
  const [player, setPlayer] = useState<TimelinePlayer | null>(null);
  const [positionMs, setPositionMs] = useState(0);
  const [highlight, setHighlight] = useState<Citation | null>(null);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [draft, setDraft] = useState<DraftSegment[]>([]);
  const [model, setModel] = useState("");
  const [job, setJob] = useState<ProcessingJob | null>(null);
  const jobRef = useRef<ProcessingJob | null>(null);
  const jobKnown = useRef(false);
  const processingEventSeen = useRef(false);
  const detailVersion = useRef(0);
  const transcriptVersion = useRef(0);
  const publishedRevision = useRef(0);
  const [transcriptLoaded, setTranscriptLoaded] = useState(false);
  const playerHandle = useRef<PlayerHandle>(null);
  const opened = useRef<string | null>(null);
  const onPlayerChange = useCallback((next: TimelinePlayer | null) => setPlayer(next), []);
  const onPosition = useCallback((ms: number) => setPositionMs(ms), []);

  useEffect(() => {
    let active = true;
    processingEventSeen.current = false;
    jobRef.current = null;
    jobKnown.current = false;
    publishedRevision.current = 0;
    opened.current = null;
    setError(null);
    setDetail(null); setTranscript(null); setDraft([]); setTranscriptLoaded(false); setProviderStatus(null); setJob(null); setPlayer(null);
    setStaleSearchResult(false);
    setPositionMs(0);
    setHighlight(null);
    const detailRequest = ++detailVersion.current;
    const transcriptRequest = ++transcriptVersion.current;
    meetingDetail(id).then(
      (next) => { if (active && detailRequest === detailVersion.current) { setDetail(next); setAudioOpen(true); } },
      (reason) => { if (active && detailRequest === detailVersion.current) setError(String(reason)); },
    );
    meetingTranscript(id).then((next) => { if (active && transcriptRequest === transcriptVersion.current) { publishedRevision.current = next?.revision ?? 0; setTranscript(next); setTranscriptLoaded(true); } }, (reason) => { if (active && transcriptRequest === transcriptVersion.current) { setError(String(reason)); setTranscriptLoaded(true); } });
    processingStatus().then((next) => {
      if (!active || processingEventSeen.current) return;
      jobRef.current = next.jobs.find((item) => item.meeting_id === id) ?? null; jobKnown.current = true; setJob(jobRef.current);
      // Segments emitted before this view opened only live in the stored Draft.
      if (jobRef.current?.stage === "transcribing") void transcriptDraft(id).then((stored) => { if (active && jobRef.current?.stage === "transcribing") setDraft((current) => mergeDraft(current, stored)); }, () => {});
    }, (reason) => { if (active) setError(String(reason)); });
    aiStatus().then((next) => { if (active) setProviderStatus(next); }, (reason) => { if (active) setError(String(reason)); });
    getSettings().then((next) => { if (active) setYourName(next.your_name); }, (reason) => { if (active) setError(String(reason)); });
    participantNames().then((next) => { if (active) setSuggestions(next); }, () => {});
    return () => { active = false; };
  }, [id]);

  useEffect(() => {
    if (!openAt || !transcript) return;
    const key = `${id}:${openAt.revision}:${openAt.segment_id}:${openAt.start_ms}:${openAt.end_ms}`;
    if (opened.current === key) return;
    if (transcript.revision !== openAt.revision || !transcript.segments.some((segment) => segment.id === openAt.segment_id)) {
      opened.current = key;
      setStaleSearchResult(true);
      onOpened?.();
      return;
    }
    if (!player) return;
    opened.current = key;
    setStaleSearchResult(false);
    setPositionMs(openAt.start_ms);
    setHighlight({ segment_id: openAt.segment_id, start_ms: openAt.start_ms, end_ms: openAt.end_ms });
    player.playSpan(openAt.start_ms, openAt.end_ms);
    onOpened?.();
  }, [id, openAt, player, transcript, onOpened]);

  useEffect(() => {
    let active = true;
    const processing = onProcessingStatus((status) => { if (active) { processingEventSeen.current = true; const next = status.jobs.find((item) => item.meeting_id === id) ?? null; jobRef.current = next; jobKnown.current = true; setJob(next); if (!next || next.stage === "failed") setDraft([]); } });
    const draftListener = onTranscriptDraft((event) => { if (active && event.meeting_id === id && (!jobKnown.current || jobRef.current?.stage === "transcribing")) setDraft((current) => mergeDraft(current, event.segments)); });
    const library = onLibraryChanged(() => {
      const transcriptRequest = ++transcriptVersion.current;
      const detailRequest = ++detailVersion.current;
      void meetingTranscript(id).then((next) => { if (active && transcriptRequest === transcriptVersion.current) { if (next && next.revision > publishedRevision.current) setDraft([]); publishedRevision.current = next?.revision ?? 0; setTranscript(next); setTranscriptLoaded(true); } }, (reason) => { if (active && transcriptRequest === transcriptVersion.current) setError(String(reason)); });
      void meetingDetail(id).then((next) => { if (active && detailRequest === detailVersion.current) setDetail(next); }, () => {});
    });
    return () => { active = false; void processing.then((stop) => stop(), () => {}); void draftListener.then((stop) => stop(), () => {}); void library.then((stop) => stop(), () => {}); };
  }, [id]);

  async function runTranscription(chosenModel?: string) {
    setError(null);
    setDraft([]);
    try { const status = chosenModel ? await enqueueTranscription(id, chosenModel) : await enqueueTranscription(id); jobRef.current = status.jobs.find((item) => item.meeting_id === id) ?? null; jobKnown.current = true; setJob(jobRef.current); }
    catch (reason) { setError(String(reason)); }
  }
  async function saveSegment(segmentId: string, text: string) {
    if (!transcript) return;
    const updated = await editSegment(id, transcript.revision, segmentId, text);
    // A newer Revision may have loaded while the edit was saving; keep it.
    ++transcriptVersion.current;
    setTranscript((current) => current && current.revision > updated.revision ? current : updated);
  }
  async function cancelJob() {
    try { const status = await cancelProcessing(id); jobRef.current = status.jobs.find((item) => item.meeting_id === id) ?? null; jobKnown.current = true; setJob(jobRef.current); setDraft([]); }
    catch (reason) { setError(String(reason)); }
  }
  async function dismissJob() {
    try { const status = await dismissProcessing(id); setJob(status.jobs.find((item) => item.meeting_id === id) ?? null); }
    catch (reason) { setError(String(reason)); }
  }
  async function changeLanguage(value: string) {
    try {
      await setMeetingLanguage(id, value ? value as Language : null);
      setDetail((current) => current ? { ...current, language: value || null } : current);
    } catch (reason) { setError(String(reason)); }
  }

  // The Library decides merges and defaults; show what it stored.
  async function reloadParticipants() {
    const next = await meetingDetail(id);
    setDetail((current) => current ? { ...current, participants: next.participants, segment_participants: next.segment_participants } : next);
  }

  async function saveParticipant(from: string) {
    if (!detail) return;
    const name = participantDrafts[from]?.trim();
    if (!name || name === from) { setParticipantDrafts((current) => { const next = { ...current }; delete next[from]; return next; }); return; }
    setError(null);
    try {
      await renameParticipant(id, from, name);
      await reloadParticipants();
      setParticipantDrafts((current) => { const next = { ...current }; delete next[from]; return next; });
      setSuggestions((current) => !current.includes(name) ? [...current, name].sort() : current);
    } catch (reason) { setError(String(reason)); }
  }

  async function removeName(name: string) {
    setError(null);
    try {
      await removeParticipant(id, name);
      await reloadParticipants();
    } catch (reason) { setError(String(reason)); }
  }

  async function moveSegments(segmentIds: string[], name: string) {
    if (!transcript) return;
    await assignSegments(id, transcript.revision, segmentIds, name);
    await reloadParticipants();
    setSuggestions((current) => !current.includes(name) ? [...current, name].sort() : current);
  }

  function cancelRename() {
    setRenaming(false);
    setError(null);
  }

  async function saveRename() {
    if (!detail || !title.trim() || saving) return;
    setSaving(true);
    setError(null);
    try {
      const summary = await renameMeeting(detail.id, title.trim());
      setDetail((current) => current ? { ...current, title: summary.title } : current);
      setRenaming(false);
      onRenamed();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  }

  async function confirmDelete() {
    if (!detail || deleteBusy) return;
    setDeleteBusy(true);
    setError(null);
    playerHandle.current?.release();
    setAudioOpen(false);
    await new Promise((resolve) => setTimeout(resolve, 0));
    try {
      await deleteMeeting(detail.id);
      setConfirmingDelete(false);
      onDeleted?.();
    } catch (reason) {
      setError(String(reason));
      setAudioOpen(true);
    } finally { setDeleteBusy(false); }
  }

  const sources = detail?.imported ? "Imported audio" : detail?.sources?.length
    ? detail.sources.map((source) => source === "microphone" ? "Microphone" : "System audio").join(" + ")
    : "Recording";
  const stageAnnouncement = job ? job.stage === "failed" ? "Processing failed" : job.stage === "queued" ? "Processing queued" : job.stage === "transcribing" ? "Transcribing" : job.stage === "downloading" ? "Downloading model" : null : transcriptLoaded && transcript ? "Processing done" : null;
  const durationMinutes = detail ? Math.max(1, Math.round(detail.duration_ms / 60_000)) : 0;
  const isVoiceNote = !!detail?.voice_note;
  const showingDraft = job?.stage === "transcribing" && draft.length > 0;
  // Every Segment of a Voice note belongs to the user, so only Me is named.
  const speakerLabels: Speaker[] = !transcript ? [] : isVoiceNote ? ["me"] : ["me", "remote", "unknown"].filter((label): label is Speaker => transcript.segments.some((segment) => segment.speaker === label));
  const meetingParticipants = transcript && detail ? participantList(transcript, detail.participants, detail.segment_participants, yourName, isVoiceNote) : [];
  return <PlayerContext.Provider value={player}><section className="flex min-h-0 min-w-0 flex-1 flex-col" aria-label="Meeting">
    <span className="sr-only" aria-live="polite" aria-atomic="true">{stageAnnouncement}</span>
    {error && <div className="mx-7 mt-4"><ErrorState title="Couldn't update the Meeting" guidance={error} /></div>}
    {staleSearchResult && <p className="mx-7 mt-3 text-sm text-warn">This search result is from an older transcript revision.</p>}
    {!detail && !error && <div className="px-7"><Loading message="Opening Meeting…" /></div>}
    {detail && <>
      <header className="flex items-start gap-4 px-7 pt-5">
        <Button variant="ghost" size="icon" aria-label="Back to Library" title="Back to Library" disabled={saving} onClick={onBack}><ChevronLeft size={16} strokeWidth={1.5} /></Button>
        <div className="grid min-w-0 flex-1 gap-1.5">
          {renaming ? <div className="flex items-center gap-2"><label htmlFor="meeting-title" className="sr-only">Title</label><Input id="meeting-title" autoFocus className="max-w-[420px] font-display text-heading-md" value={title} onChange={(event) => setTitle(event.target.value)} onKeyDown={(event) => {
            if (event.key === "Enter") { event.preventDefault(); void saveRename(); }
            if (event.key === "Escape") cancelRename();
          }} /><Button disabled={!title.trim() || saving} onClick={() => void saveRename()}>Save</Button><Button variant="ghost" onClick={cancelRename}>Cancel</Button></div>
            : <button type="button" className="group flex min-w-0 items-center gap-2 justify-self-start text-left" aria-label={`Rename ${detail.title}`} onClick={() => { setTitle(detail.title); setError(null); setRenaming(true); }}><h1 className="m-0 truncate font-display text-heading-md">{detail.title}</h1><Pencil size={16} strokeWidth={1.5} className="shrink-0 text-ink-faint opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100" aria-hidden="true" /></button>}
          <div className="flex flex-wrap items-center gap-x-3.5 gap-y-1.5 text-xs text-ink-muted">
            <span className="inline-flex items-center gap-1.5"><Clock3 size={14} strokeWidth={1.5} aria-hidden="true" /><time dateTime={detail.started_at}>{meetingDateTime(detail.started_at)}</time> · {durationMinutes} min</span>
            <label className="inline-flex items-center gap-1.5"><Globe2 size={14} strokeWidth={1.5} aria-hidden="true" /><span className="sr-only">Language</span><select aria-label="Language" className="max-w-[130px] cursor-pointer border-0 bg-transparent text-xs text-ink-muted hover:text-ink" value={detail.language ?? ""} disabled={!!job && job.stage !== "failed"} onChange={(event) => void changeLanguage(event.target.value)}><option value="">Auto-detect</option><option value="en">English</option><option value="vi">Vietnamese</option></select></label>
            <span className="inline-flex items-center gap-1.5"><Mic size={14} strokeWidth={1.5} aria-hidden="true" />{sources}</span>
            <button type="button" className="inline-flex items-center gap-1.5 hover:text-accent" onClick={() => void openMeetingFolder(id).catch((reason) => setError(String(reason)))}><FolderOpen size={14} strokeWidth={1.5} aria-hidden="true" />Open folder</button>
          </div>
        </div>
        <DropdownMenu><DropdownMenuTrigger asChild><Button variant="ghost" size="icon" aria-label="More actions"><Ellipsis size={16} strokeWidth={1.5} /></Button></DropdownMenuTrigger><DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={() => { setTitle(detail.title); setRenaming(true); }}><Pencil size={16} strokeWidth={1.5} />Rename</DropdownMenuItem>
          <DropdownMenuItem disabled={!transcript || !!job} onSelect={() => void runTranscription(model || undefined)}><RefreshCw size={16} strokeWidth={1.5} />Re-transcribe</DropdownMenuItem>
          <DropdownMenuSeparator className="my-1 h-px bg-line" />
          <DropdownMenuItem className="text-record" disabled={recordingMeetingId === id || (!!job && job.stage !== "failed")} onSelect={() => { setError(null); setConfirmingDelete(true); }}><Trash2 size={16} strokeWidth={1.5} />Delete</DropdownMenuItem>
        </DropdownMenuContent></DropdownMenu>
      </header>
      {confirmingDelete && <ConfirmDelete title={detail.title} busy={deleteBusy} onConfirm={() => void confirmDelete()} onCancel={() => setConfirmingDelete(false)} />}
      {detail.audio_path && audioOpen ? <Player ref={playerHandle} audio_path={detail.audio_path} duration_ms={detail.duration_ms} markers={detail.markers} highlight={highlight} onPlayerChange={onPlayerChange} onPosition={onPosition} /> : !detail.audio_path ? <p className="mx-7 mt-4 text-sm text-ink-muted">No audio yet</p> : null}
      <div className="mt-4 grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_380px] border-t border-line max-[900px]:grid-cols-[minmax(0,1fr)_minmax(300px,42%)] max-[680px]:grid-cols-1">
        <section aria-label="Transcript" className="min-h-0 min-w-0 overflow-y-auto px-7 pb-8 pt-2 max-[680px]:max-h-[50vh]">
          <div className="sticky top-0 z-10 flex items-center gap-2 bg-canvas py-2.5"><h2 className="m-0 mr-auto font-display text-heading-sm">Transcript</h2>
            {showingDraft ? <Badge className="rounded-full border border-line bg-transparent">Draft · Transcribing…</Badge> : transcript && <Badge className="rounded-full border border-line bg-transparent">Revision {transcript.revision} · {transcript.model.join(", ")}</Badge>}
            {transcript && !job && <Button variant="ghost" onClick={() => void runTranscription(model || undefined)}><RefreshCw size={16} strokeWidth={1.5} />Re-transcribe</Button>}
          </div>
          {transcript && !showingDraft && <p className="sr-only">Transcript revision {transcript.revision} · model {transcript.model.join(", ")} · {transcript.segments.length} segments · {languageName(transcript.language)}</p>}
          {transcript?.fallback_reason && !showingDraft && <p role="status" className="mb-3 text-sm text-ink-muted">Part of this Meeting was transcribed {transcript.model.every((engine) => engine === "Groq" || engine === "Gemini") ? "with Gemini" : "locally"}: {transcript.fallback_reason}.</p>}
          {transcript && !showingDraft && detail.correction_notice && <p role="status" className="mb-4 text-sm text-ink-muted">{detail.correction_notice}</p>}
          {transcript && !showingDraft && meetingParticipants.length > 0 && <div className="mb-4 grid gap-2 rounded-md border border-line bg-raised p-3"><h3 className="m-0 text-sm font-medium">Participants</h3><datalist id="participant-names">{suggestions.map((name) => <option key={name} value={name} />)}</datalist><div className="flex flex-wrap gap-3">{meetingParticipants.map((name) => { const labels = speakerLabels.filter((label) => labelParticipantName(label, detail.participants, yourName, isVoiceNote) === name); const removable = Object.values(detail.participants).includes(name) || Object.values(detail.segment_participants?.segments ?? {}).includes(name); return <div key={name} className="grid min-w-[130px] flex-1 gap-1 text-xs text-ink-muted"><label>{labels.length ? labels.map(speakerName).join(", ") : name} Participant<Input aria-label={`${labels.length === 1 ? speakerName(labels[0]) : name} Participant`} list="participant-names" value={participantDrafts[name] ?? name} onChange={(event) => setParticipantDrafts((current) => ({ ...current, [name]: event.target.value }))} onBlur={() => void saveParticipant(name)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); event.currentTarget.blur(); } }} /></label>{labels.length > 0 && <span className="text-ink-faint">Speaker labels: {labels.map(speakerName).join(", ")}</span>}{removable && <Button variant="ghost" className="justify-self-start px-0 text-xs" onClick={() => void removeName(name)}>Remove {name}</Button>}</div>; })}</div></div>}
          {transcript && !job && <div className="mb-3 flex items-center gap-2 text-xs text-ink-muted"><label htmlFor="meeting-model">Model</label><select id="meeting-model" className="rounded-sm border border-line-strong bg-raised px-2 py-1 text-xs outline-none focus:border-accent" value={model} onChange={(event) => setModel(event.target.value)}><option value="">Model from Settings</option>{MODELS.map((option) => <option key={option.name} value={option.name}>{option.name}</option>)}</select><span>Re-transcribing adds a new Revision.</span></div>}
          {job && job.stage !== "failed" && <div role="status" className="mb-4"><Loading message={processingStageLabel(job)} rows={2} /><p className="text-sm text-ink-muted">{processingStageLabel(job)}</p>{job.stage !== "queued" && <div role="progressbar" aria-label={processingStageLabel(job)} aria-valuenow={Math.round(job.progress * 100)} aria-valuemin={0} aria-valuemax={100} className="mt-2 h-1.5 max-w-md rounded-sm bg-page-shadow"><div className="h-full rounded-sm bg-accent" style={{ width: `${Math.round(job.progress * 100)}%` }} /></div>}<Button variant="ghost" className="mt-2" onClick={() => void cancelJob()}>Cancel processing</Button></div>}
          {job?.stage === "failed" && <div className="mb-4"><ErrorState title="Transcription failed" guidance={job.error ?? "Try again."} onRetry={() => void runTranscription()} /><Button variant="ghost" onClick={() => void dismissJob()}>Dismiss</Button></div>}
          {!transcriptLoaded && <Loading message="Loading Transcript…" />}
          {transcriptLoaded && !job && !transcript && <EmptyState title="No Transcript yet" description="Transcribe this Meeting to see its Segments." action={<Button onClick={() => void runTranscription()}>Transcribe</Button>} />}
          {showingDraft ? <DraftView segments={draft} participants={detail.participants} yourName={yourName} isVoiceNote={isVoiceNote} /> : transcript && <TranscriptView key={`${id}:${transcript.revision}`} transcript={transcript} positionMs={positionMs} player={player} editable={!job || job.stage === "failed"} onEdit={saveSegment} onMove={isVoiceNote ? undefined : moveSegments} participants={detail.participants} segmentParticipants={detail.segment_participants} yourName={yourName} isVoiceNote={isVoiceNote} />}
        </section>
        <aside className="flex min-h-0 min-w-0 flex-col border-l border-line max-[680px]:border-l-0 max-[680px]:border-t" aria-label="Summary and Ask">
          {showingDraft ? <p className="p-5 text-sm text-ink-muted">Summary, Ask and Citations are available when the Transcript is ready.</p> : !providerStatus && <div className="px-5 py-4"><Loading message="Loading Summary and Ask…" /></div>}
          {!showingDraft && providerStatus && <Tabs defaultValue="summary" className="flex min-h-0 flex-1 flex-col"><TabsList className="shrink-0"><TabsTrigger value="summary">Summary</TabsTrigger><TabsTrigger value="ask">Ask</TabsTrigger></TabsList>
            <TabsContent value="summary" className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{!providerStatus.configured && <AiSetupGuidance onOpenSettings={onOpenSettings} />}<SummaryPanel meetingId={id} transcript={transcriptLoaded ? transcript : null} transcriptLoaded={transcriptLoaded} player={player} aiStatus={providerStatus} onCitationPlay={setHighlight} /></TabsContent>
            <TabsContent value="ask" className="flex min-h-0 flex-1 flex-col"><AskPanel meetingId={id} transcript={transcriptLoaded ? transcript : null} transcriptLoaded={transcriptLoaded} player={player} aiStatus={providerStatus} onCitationPlay={setHighlight} />{!providerStatus.configured && <div className="px-5"><AiSetupGuidance onOpenSettings={onOpenSettings} /></div>}</TabsContent>
          </Tabs>}
        </aside>
      </div>
    </>}
  </section></PlayerContext.Provider>;
}
