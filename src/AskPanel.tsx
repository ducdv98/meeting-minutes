import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { PROVIDER_NAMES, type AiStatus } from "./ai";
import { answerLabel, askMeeting, askSendPlan, meetingAsks, onAskDelta, onAsksChanged, retryAsk, stopAsk, type AskView } from "./ask";
import CitationLink from "./CitationLink";
import CloudDisclosure from "./CloudDisclosure";
import type { Transcript } from "./processing";
import type { SendPlan } from "./summary";
import type { TimelinePlayer } from "./timeline";

export default function AskPanel({ meetingId, transcript, player, aiStatus }: { meetingId: string; transcript: Transcript | null; player: TimelinePlayer | null; aiStatus: AiStatus | null }) {
  const [view, setView] = useState<AskView>({ asks: [], active: null, retryable_id: null });
  const [question, setQuestion] = useState("");
  const [plan, setPlan] = useState<SendPlan | null>(null);
  const [pending, setPending] = useState<{ question: string; retry: boolean } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reloadVersion = useRef(0);
  useEffect(() => {
    let active = true;
    setView({ asks: [], active: null, retryable_id: null });
    const reload = () => { const version = ++reloadVersion.current; void meetingAsks(meetingId).then((next) => { if (active && version === reloadVersion.current) setView(next); }, (reason) => { if (active && version === reloadVersion.current) setError(String(reason)); }); };
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
  return <section aria-label="Ask" className="ask-panel"><h2>Ask</h2>
    {error && <p role="alert">{error}</p>}
    <ol aria-label="Asks">{view.asks.map((entry) => <li key={entry.id}>
      <p>{entry.question}</p>{answerLabel(entry.state) && <strong>{answerLabel(entry.state)}</strong>}
      {entry.answer && <p>{entry.answer}</p>}
      {entry.citations.map((citation) => <CitationLink key={citation.segment_id} citation={citation} player={player} />)}
      {entry.state === "unverified" && <p>Some Citations could not be checked against this Meeting.</p>}
      {transcript && entry.revision !== transcript.revision && <p>Asked about transcript revision {entry.revision}</p>}
      {entry.error && <p role={view.asks.at(-1)?.id === entry.id ? "alert" : undefined}>{entry.error}</p>}
      {view.retryable_id === entry.id && aiStatus?.configured && <button type="button" disabled={busy || !!view.active} onClick={() => void start(entry.question, true)}>Retry</button>}
    </li>)}</ol>
    {view.active && <div><p aria-live="polite">{view.active.partial}</p><button type="button" onClick={() => void stopAsk(meetingId).catch((reason) => setError(String(reason)))}>Stop</button></div>}
    {!transcript && <p>Transcribe the Meeting to ask about it.</p>}
    {aiStatus?.configured && <form onSubmit={submit}><textarea aria-label="Question" value={question} onChange={(event) => setQuestion(event.target.value)} onKeyDown={keyDown} disabled={!transcript || !!view.active || busy} />
      <button type="submit" disabled={!transcript || !!view.active || busy || !question.trim()}>{aiStatus.provider && aiStatus.provider !== "ollama" ? `Ask ${PROVIDER_NAMES[aiStatus.provider]}` : "Ask"}</button></form>}
    {plan?.disclosure && pending && <CloudDisclosure disclosure={plan.disclosure} onSend={() => void send(pending.question, pending.retry, true)} onCancel={() => { setPlan(null); setPending(null); }} />}
  </section>;
}
