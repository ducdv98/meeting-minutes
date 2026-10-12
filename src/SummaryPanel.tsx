import { useEffect, useRef, useState } from "react";
import type { AiStatus } from "./ai";
import { PROVIDER_NAMES } from "./ai";
import CloudDisclosure from "./CloudDisclosure";
import CitationLink from "./CitationLink";
import type { Transcript } from "./processing";
import { onLibraryChanged } from "./recording";
import { cancelSummary, dismissSummary, generateSummary, meetingSummary, onSummaryStatus, summarySendPlan, summaryStatus, type SendPlan, type Summary, type SummaryItem, type SummaryJob } from "./summary";
import type { TimelinePlayer } from "./timeline";

export default function SummaryPanel({ meetingId, transcript, player, aiStatus }: { meetingId: string; transcript: Transcript | null; player: TimelinePlayer | null; aiStatus: AiStatus | null }) {
  const [summary, setSummary] = useState<Summary | null>(null);
  const [job, setJob] = useState<SummaryJob | null>(null);
  const [plan, setPlan] = useState<SendPlan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const statusEventSeen = useRef(false);
  useEffect(() => {
    let active = true;
    statusEventSeen.current = false;
    setSummary(null); setJob(null);
    const reload = () => { void meetingSummary(meetingId).then((value) => { if (active) setSummary(value); }, (reason) => { if (active) setError(String(reason)); }); };
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
  function section(title: string, items: SummaryItem[]) { return <div><h3>{title}</h3>{items.length ? <ul>{items.map((item, index) => <li key={index}>{item.text} {item.citations.map((citation) => <CitationLink key={citation.segment_id} citation={citation} player={player} />)}</li>)}</ul> : <p>None</p>}</div>; }
  return <section aria-label="Summary" className="summary-panel"><h2>Summary</h2>
    {error && <p role="alert">{error}</p>}
    {!transcript && <p>Transcribe the Meeting to get a Summary.</p>}
    {job?.state === "running" && <p>Summarising… ({job.done}/{job.total}) <button type="button" onClick={() => void stop()}>Stop</button></p>}
    {job?.state === "failed" && <><p role="alert">{job.error}</p><button type="button" onClick={() => void begin()}>Retry</button><button type="button" onClick={() => void dismiss()}>Dismiss</button></>}
    {summary && <>{section("Overview", summary.overview)}{section("Key points", summary.key_points)}{section("Decisions", summary.decisions)}
      <div><h3>Action items</h3>{summary.action_items.length ? <ul>{summary.action_items.map((item, index) => <li key={index}>{item.text} {item.owner && <span>Owner: {item.owner}</span>} {item.due && <span>Due: {item.due}</span>} {item.citations.map((citation) => <CitationLink key={citation.segment_id} citation={citation} player={player} />)}</li>)}</ul> : <p>None</p>}</div>
      <p>From transcript revision {summary.revision} · {PROVIDER_NAMES[summary.provider]} {summary.model}</p>
      {transcript && summary.revision !== transcript.revision && <p>This Summary is from an earlier Transcript revision.</p>}</>}
    {transcript && aiStatus?.configured && !job && <button type="button" disabled={busy} onClick={() => void begin()}>{summary ? "Regenerate Summary" : aiStatus.provider && aiStatus.provider !== "ollama" ? `Generate Summary with ${PROVIDER_NAMES[aiStatus.provider]}` : "Generate Summary"}</button>}
    {plan?.disclosure && <CloudDisclosure disclosure={plan.disclosure} onSend={() => void send()} onCancel={() => setPlan(null)} />}
  </section>;
}
