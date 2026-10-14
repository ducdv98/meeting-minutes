import { useEffect, useRef, useState } from "react";
import type { AiStatus } from "./ai";
import { PROVIDER_NAMES } from "./ai";
import CloudDisclosure from "./CloudDisclosure";
import CitationLink from "./CitationLink";
import Markdown from "./markdown";
import type { Transcript } from "./processing";
import { onLibraryChanged } from "./recording";
import { cancelSummary, dismissSummary, generateSummary, meetingSummary, onSummaryStatus, summaryParticipantsChanged, summarySendPlan, summaryStatus, type SendPlan, type Summary, type SummaryItem, type SummaryJob } from "./summary";
import type { TimelinePlayer } from "./timeline";
import { LockKeyhole, RefreshCw } from "lucide-react";
import { Button } from "./components/ui/Button";
import { Loading, LoadingRows } from "./components/states/Loading";
import { EmptyState } from "./components/states/EmptyState";
import { ErrorState } from "./components/states/ErrorState";
import type { Citation } from "./summary";

export default function SummaryPanel({ meetingId, transcript, player, aiStatus, onCitationPlay, transcriptLoaded = true }: { meetingId: string; transcript: Transcript | null; transcriptLoaded?: boolean; player: TimelinePlayer | null; aiStatus: AiStatus | null; onCitationPlay?: (citation: Citation) => void }) {
  const [summary, setSummary] = useState<Summary | null>(null);
  const [participantsChanged, setParticipantsChanged] = useState(false);
  const [job, setJob] = useState<SummaryJob | null>(null);
  const [plan, setPlan] = useState<SendPlan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const configured = aiStatus?.summary_configured ?? aiStatus?.configured;
  const summaryProvider = aiStatus?.summary_provider ?? aiStatus?.provider;
  const statusEventSeen = useRef(false);
  const reloadVersion = useRef(0);
  useEffect(() => {
    let active = true;
    statusEventSeen.current = false;
    setSummary(null); setParticipantsChanged(false); setJob(null); setLoaded(false);
    const reload = () => { const version = ++reloadVersion.current; void Promise.all([meetingSummary(meetingId), summaryParticipantsChanged(meetingId).catch(() => false)]).then(([value, changed]) => { if (active && version === reloadVersion.current) { setSummary(value); setParticipantsChanged(changed); setError(null); setLoaded(true); } }, (reason) => { if (active && version === reloadVersion.current) { setError(String(reason)); setLoaded(true); } }); };
    reload();
    void summaryStatus().then((value) => { if (active && !statusEventSeen.current) setJob(value.jobs.find((item) => item.meeting_id === meetingId) ?? null); }, (reason) => { if (active) setError(String(reason)); });
    const statusListener = onSummaryStatus((status) => { if (!active) return; statusEventSeen.current = true; const next = status.jobs.find((item) => item.meeting_id === meetingId) ?? null; setJob(next); if (!next) reload(); });
    const libraryListener = onLibraryChanged(() => { if (active) reload(); });
    return () => { active = false; void statusListener.then((stop) => stop(), () => {}); void libraryListener.then((stop) => stop(), () => {}); };
  }, [meetingId]);
  async function begin() {
    setError(null); setBusy(true);
    try {
      const next = await summarySendPlan(meetingId);
      if (next.disclosure) setPlan(next);
      else { const status = await generateSummary(meetingId, false); setJob(status.jobs.find((item) => item.meeting_id === meetingId) ?? null); }
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }
  async function send() {
    setPlan(null); setBusy(true);
    try { const status = await generateSummary(meetingId, true); setJob(status.jobs.find((item) => item.meeting_id === meetingId) ?? null); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }
  async function stop() { try { setJob((await cancelSummary(meetingId)).jobs.find((item) => item.meeting_id === meetingId) ?? null); } catch (reason) { setError(String(reason)); } }
  async function dismiss() { try { setJob((await dismissSummary(meetingId)).jobs.find((item) => item.meeting_id === meetingId) ?? null); } catch (reason) { setError(String(reason)); } }
  const cite = (citation: Citation) => <CitationLink key={citation.segment_id} citation={citation} player={player} segmentText={summary?.revision === transcript?.revision ? transcript?.segments.find((segment) => segment.id === citation.segment_id)?.text : "earlier Revision"} onPlay={onCitationPlay} />;
  function section(title: string, items: SummaryItem[], overview = false) { return <section className="mb-5 grid gap-2"><h3 className="m-0 text-xs font-medium text-ink-faint">{title}</h3>{items.length ? overview ? <div className="grid gap-2">{items.map((item, index) => <div key={index} className="text-base leading-[23px]"><Markdown text={item.text} trailing={item.citations.map(cite)} /></div>)}</div> : <ul className="m-0 grid gap-1.5 pl-[18px]">{items.map((item, index) => <li key={index} className="text-base leading-[23px]"><Markdown text={item.text} trailing={item.citations.map(cite)} /></li>)}</ul> : <p className="m-0 text-sm text-ink-muted">None</p>}</section>; }
  return <section aria-label="Summary" className="min-w-0">
    {!loaded && <Loading message="Loading Summary…" rows={3} />}
    {error && <ErrorState title="Couldn't load the Summary" guidance={error} />}
    {loaded && !error && transcriptLoaded && !transcript && !summary && <EmptyState title="No Summary yet" description="Transcribe the Meeting to get a Summary." />}
    {job?.state === "running" && <div><LoadingRows rows={2} /><p className="text-sm text-ink-muted">Summarising… ({job.done}/{job.total})</p><Button variant="ghost" onClick={() => void stop()}>Stop</Button></div>}
    {job?.state === "failed" && <><ErrorState title="Couldn't make the Summary" guidance={job.error ?? "Try again."} onRetry={() => void begin()} /><Button variant="ghost" onClick={() => void dismiss()}>Dismiss</Button></>}
    {summary && <>{section("Overview", summary.overview, true)}{section("Key points", summary.key_points)}{section("Decisions", summary.decisions)}
      <section className="mb-5 grid gap-2"><h3 className="m-0 text-xs font-medium text-ink-faint">Action items</h3>{summary.action_items.length ? <ul className="m-0 grid list-none gap-2 p-0">{summary.action_items.map((item, index) => <li key={index} className="grid grid-cols-[18px_1fr] items-start gap-2 text-base leading-[23px]"><span aria-hidden="true" className="mt-1 size-4 rounded-[4px] border-[1.5px] border-line-strong" /><div><Markdown text={item.text} trailing={item.citations.map(cite)} />{(item.owner || item.due) && <div className="text-xs text-ink-muted">{[item.owner, item.due].filter(Boolean).join(" · ")}</div>}</div></li>)}</ul> : <p className="m-0 text-sm text-ink-muted">None</p>}</section>
      {transcript && summary.revision !== transcript.revision && <p className="text-xs text-warn">This Summary is from an earlier Transcript revision.</p>}
      {participantsChanged && <div className="mb-4 flex items-center gap-2 text-xs text-warn"><span>Participants changed: regenerate Summary?</span>{transcript && configured && !job && <Button variant="ghost" disabled={busy} onClick={() => void begin()}>Regenerate</Button>}</div>}
      <div className="flex items-center gap-2 border-t border-line pt-3 text-xs text-ink-muted"><LockKeyhole size={16} strokeWidth={1.5} aria-hidden="true" /><span className="min-w-0 flex-1">Made {summary.provider === "ollama" ? "on this PC with Ollama" : `with ${PROVIDER_NAMES[summary.provider]}`} · {summary.model}</span>{transcript && configured && !job && <Button variant="ghost" className="h-[26px] px-2" disabled={busy} onClick={() => void begin()}><RefreshCw size={16} strokeWidth={1.5} />Redo</Button>}</div>
    </>}
    {loaded && !error && transcript && !summary && !job && <EmptyState title="No Summary yet" description="Make a cited Summary from this Meeting's Transcript." action={configured ? <Button disabled={busy} onClick={() => void begin()}>{summaryProvider && summaryProvider !== "ollama" ? `Generate Summary with ${PROVIDER_NAMES[summaryProvider]}` : "Generate Summary"}</Button> : undefined} />}
    {plan?.disclosure && <CloudDisclosure disclosure={plan.disclosure} onSend={() => void send()} onCancel={() => setPlan(null)} />}
  </section>;
}
