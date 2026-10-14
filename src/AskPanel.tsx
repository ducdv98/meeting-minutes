import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { PROVIDER_NAMES, type AiStatus } from "./ai";
import { askMeeting, askSendPlan, meetingAsks, onAskDelta, onAsksChanged, retryAsk, stopAsk, type AskEntry, type AskView } from "./ask";
import CitationLink from "./CitationLink";
import CloudDisclosure from "./CloudDisclosure";
import Markdown from "./markdown";
import type { Transcript } from "./processing";
import type { SendPlan } from "./summary";
import type { TimelinePlayer } from "./timeline";
import type { Citation } from "./summary";
import { AlertTriangle, ArrowUp, CircleCheck, CircleHelp, LockKeyhole, RotateCcw, Square } from "lucide-react";
import { Button } from "./components/ui/Button";
import { Textarea } from "./components/ui/Textarea";
import { EmptyState } from "./components/states/EmptyState";
import { ErrorState } from "./components/states/ErrorState";
import { Loading } from "./components/states/Loading";

export default function AskPanel({ meetingId, transcript, player, aiStatus, onCitationPlay, transcriptLoaded = true }: { meetingId: string; transcript: Transcript | null; transcriptLoaded?: boolean; player: TimelinePlayer | null; aiStatus: AiStatus | null; onCitationPlay?: (citation: Citation) => void }) {
  const [view, setView] = useState<AskView>({ asks: [], active: null, retryable_id: null });
  const [question, setQuestion] = useState("");
  const [plan, setPlan] = useState<SendPlan | null>(null);
  const [pending, setPending] = useState<{ question: string; retry: boolean } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const reloadVersion = useRef(0);
  useEffect(() => {
    let active = true;
    setLoaded(false);
    setView({ asks: [], active: null, retryable_id: null });
    const reload = () => { const version = ++reloadVersion.current; void meetingAsks(meetingId).then((next) => { if (active && version === reloadVersion.current) { setView(next); setError(null); setLoaded(true); } }, (reason) => { if (active && version === reloadVersion.current) { setError(String(reason)); setLoaded(true); } }); };
    reload();
    const delta = onAskDelta((event) => { if (active && event.meeting_id === meetingId) setView((current) => current.active?.ask_id === event.ask_id ? { ...current, active: { ...current.active, partial: event.text } } : current); });
    const changed = onAsksChanged((id) => { if (active && id === meetingId) reload(); });
    return () => { active = false; void delta.then((stop) => stop(), () => {}); void changed.then((stop) => stop(), () => {}); };
  }, [meetingId]);
  async function start(text: string, retry: boolean) {
    setError(null); setBusy(true);
    try {
      const next = await askSendPlan(meetingId, text);
      if (next.disclosure) { setPlan(next); setPending({ question: text, retry }); }
      else await send(text, retry, false);
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }
  async function send(text: string, retry: boolean, consent: boolean) {
    setPlan(null); setPending(null); setBusy(true);
    try {
      const next = retry ? await retryAsk(meetingId, consent) : await askMeeting(meetingId, text, consent);
      setView(next);
      if (!retry) setQuestion("");
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }
  function submit(event: FormEvent) { event.preventDefault(); if (!busy && !view.active && transcript && aiStatus?.configured && question.trim()) void start(question.trim(), false); }
  function keyDown(event: KeyboardEvent<HTMLTextAreaElement>) { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }
  function state(entry: AskEntry) {
    if (entry.state === "answered") return <span className="inline-flex items-center gap-1.5 text-xs font-medium text-ok"><CircleCheck size={14} strokeWidth={1.5} />Answered</span>;
    if (entry.state === "insufficient_evidence") return <span className="inline-flex items-center gap-1.5 text-xs font-medium text-ink-muted"><CircleHelp size={14} strokeWidth={1.5} />Insufficient evidence</span>;
    if (entry.state === "off_topic") return <span className="inline-flex items-center gap-1.5 text-xs font-medium text-ink-muted"><CircleHelp size={14} strokeWidth={1.5} />Off-topic</span>;
    if (entry.state === "incomplete" || entry.state === "unverified") return <><span className="inline-flex items-center gap-1.5 text-xs font-medium text-warn"><AlertTriangle size={14} strokeWidth={1.5} />{entry.state === "incomplete" ? "Incomplete" : "Unverified"}</span><p className="m-0 text-xs text-warn">{entry.state === "incomplete" ? "This Answer stopped early. It won't be used as context for later Asks." : "Some Citations could not be checked against this Meeting. This Answer won't be used as context for later Asks."}</p></>;
    if (entry.state === "failed") return <span className="inline-flex items-center gap-1.5 text-xs font-medium text-warn"><AlertTriangle size={14} strokeWidth={1.5} />Failed</span>;
    return null;
  }
  const provider = aiStatus?.provider ? PROVIDER_NAMES[aiStatus.provider] : "the selected provider";
  return <section aria-label="Ask" className="flex min-h-0 flex-1 flex-col">
    <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      {!loaded && <Loading message="Loading Asks…" rows={2} />}
      {error && <ErrorState title="Couldn't load the Asks" guidance={error} />}
      {loaded && !error && transcriptLoaded && !transcript && <EmptyState title="No Transcript yet" description="Transcribe the Meeting to Ask about it." />}
      {loaded && !error && transcript && !view.asks.length && !view.active && <EmptyState title="Ask about this Meeting" description="Answers use this Meeting's Transcript and point to the moments they came from." />}
      <ol aria-label="Asks" className="m-0 grid list-none gap-5 p-0">{view.asks.map((entry) => <li key={entry.id} className="grid gap-2">
        <p className="m-0 max-w-[85%] justify-self-end rounded-md bg-panel px-3 py-2 text-base leading-[21px]">{entry.question}</p>
        <div className="grid gap-1.5 text-base leading-[23px]">
          {state(entry)}
          {entry.answer && <div className={entry.state === "insufficient_evidence" ? "rounded-md border border-dashed border-line-strong px-3 py-2.5 text-sm text-ink-muted" : entry.state === "off_topic" ? "text-sm text-ink-muted" : ""}><Markdown text={entry.answer} trailing={entry.citations.map((citation) => <CitationLink key={citation.segment_id} citation={citation} player={player} segmentText={entry.revision === transcript?.revision ? transcript?.segments.find((segment) => segment.id === citation.segment_id)?.text : "earlier Revision"} onPlay={onCitationPlay} />)} /></div>}
          {transcript && entry.revision !== transcript.revision && <p className="m-0 text-xs text-ink-muted">Asked about transcript revision {entry.revision}</p>}
          {entry.error && <p role={view.asks.at(-1)?.id === entry.id ? "alert" : undefined} className="m-0 text-sm text-warn">{entry.error}</p>}
          {view.retryable_id === entry.id && aiStatus?.configured && <Button variant="ghost" className="justify-self-start" disabled={busy || !!view.active} onClick={() => void start(entry.question, true)}><RotateCcw size={16} strokeWidth={1.5} />Retry</Button>}
        </div>
      </li>)}</ol>
      {view.active && <div className="mt-5 grid gap-2"><div aria-live="polite" className="text-base leading-[23px]"><Markdown text={view.active.partial || "Answering…"} /></div><Button variant="ghost" className="justify-self-start" onClick={() => void stopAsk(meetingId).catch((reason) => setError(String(reason)))}><Square size={16} strokeWidth={1.5} />Stop</Button></div>}
    </div>
    {aiStatus?.configured && <form onSubmit={submit} className="border-t border-line px-4 pb-4 pt-3"><div className="flex items-end gap-2 rounded-md border border-line-strong bg-raised py-1.5 pl-3 pr-1.5 focus-within:border-accent">
      <Textarea rows={1} aria-label="Question" placeholder="Ask about this Meeting" className="min-h-8 flex-1 resize-none border-0 bg-transparent px-0 py-[5px] text-base shadow-none focus:shadow-none" value={question} onChange={(event) => setQuestion(event.target.value)} onKeyDown={keyDown} disabled={!transcriptLoaded || !transcript || !!view.active || busy} />
      <Button type="submit" size="icon" aria-label={aiStatus.provider && aiStatus.provider !== "ollama" ? `Ask ${provider}` : "Ask"} disabled={!transcript || !!view.active || busy || !question.trim()}><ArrowUp size={16} strokeWidth={1.5} /></Button>
    </div><p className="mt-2 flex items-center gap-1.5 text-xs text-ink-faint"><LockKeyhole size={13} strokeWidth={1.5} aria-hidden="true" />Answers come from this Meeting only, using {provider}{aiStatus.provider === "ollama" ? " on this PC" : ""}.</p></form>}
    {plan?.disclosure && pending && <CloudDisclosure disclosure={plan.disclosure} onSend={() => void send(pending.question, pending.retry, true)} onCancel={() => { setPlan(null); setPending(null); }} />}
  </section>;
}
