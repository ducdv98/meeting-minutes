import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { ArrowUp, Square } from "lucide-react";
import { PROVIDER_NAMES, type AiStatus, type LanguageModel, type LanguageModelSettings } from "./ai";
import { answerLabel } from "./ask";
import { libraryAskPlan, libraryAskStart, libraryAsks, onLibraryAskDelta, onLibraryAsksChanged, stopLibraryAsk, type DateRange, type LibraryAskEntry, type LibraryAskPlan, type LibraryAskView, type LibraryCitation } from "./libraryAsk";
import type { MeetingSummary } from "./library";
import LibraryAskConfirmation from "./LibraryAskConfirmation";
import Markdown from "./markdown";
import { Button } from "./components/ui/Button";
import { Textarea } from "./components/ui/Textarea";
import { Loading } from "./components/states/Loading";
import ModelPicker, { languageModelOptions, modelId } from "./components/ModelPicker";

const ranges: { value: DateRange; label: string }[] = [
  { value: "last_7_days", label: "Last 7 days" },
  { value: "last_30_days", label: "Last 30 days" },
  { value: "last_90_days", label: "Last 90 days" },
  { value: "all", label: "All" },
];

export default function LibraryAskPanel({ initialQuestion = "", meetings, aiStatus, languageModels, onOpenCitation, onOpenSettings }: { initialQuestion?: string; meetings: MeetingSummary[]; aiStatus: AiStatus | null; languageModels?: LanguageModelSettings; onOpenCitation(citation: LibraryCitation): void; onOpenSettings(): void }) {
  const [question, setQuestion] = useState(initialQuestion);
  const [range, setRange] = useState<DateRange>("all");
  const [view, setView] = useState<LibraryAskView>({ asks: [], active: null });
  const [plan, setPlan] = useState<LibraryAskPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [choice, setChoice] = useState<string | null>(null);
  const reloadVersion = useRef(0);
  const startedInitial = useRef(false);
  const fallback = aiStatus?.ask_provider && aiStatus.ask_model ? { provider: aiStatus.ask_provider as LanguageModel["provider"], model: aiStatus.ask_model } : aiStatus?.provider && aiStatus.model && aiStatus.provider !== "groq" ? { provider: aiStatus.provider, model: aiStatus.model } : null;
  const models = languageModels?.models ?? (fallback ? [fallback] : []);
  const options = languageModelOptions(languageModels, aiStatus, fallback);
  const defaultId = languageModels ? (languageModels.ask ? modelId(languageModels.ask) : null) : fallback ? modelId(fallback) : null;
  const selected = options.find((option) => option.id === (choice ?? defaultId) && option.ready);
  const selectedModel = models.find((model) => selected?.id === modelId(model)) ?? null;

  useEffect(() => {
    let active = true;
    const reload = () => { const version = ++reloadVersion.current; void libraryAsks().then((next) => { if (active && version === reloadVersion.current) { setView(next); setLoaded(true); } }, (reason) => { if (active && version === reloadVersion.current) { setError(String(reason)); setLoaded(true); } }); };
    reload();
    const delta = onLibraryAskDelta((event) => { if (active) setView((current) => current.active?.ask_id === event.ask_id ? { ...current, active: { ...current.active, partial: event.text } } : current); });
    const changed = onLibraryAsksChanged(() => { if (active) reload(); });
    return () => { active = false; void delta.then((stop) => stop(), () => {}); void changed.then((stop) => stop(), () => {}); };
  }, []);

  useEffect(() => {
    if (initialQuestion.trim() && selectedModel && !startedInitial.current) {
      startedInitial.current = true;
      void prepare(initialQuestion.trim(), "all", selectedModel);
    }
  });

  async function prepare(text: string, selectedRange: DateRange, model: LanguageModel) {
    setBusy(true); setError(null);
    try {
      const next = await libraryAskPlan(text, selectedRange, model);
      if (next.cloud && next.meetings.length) setPlan(next);
      else await send(next, [], false);
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  async function send(next: LibraryAskPlan, excludedMeetings: string[], consent: boolean) {
    setPlan(null); setBusy(true);
    try { setView(await libraryAskStart(next.plan_id, excludedMeetings, consent)); setQuestion(""); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  function submit(event: FormEvent) { event.preventDefault(); if (selectedModel && !busy && !view.active && question.trim()) void prepare(question.trim(), range, selectedModel); }
  function keyDown(event: KeyboardEvent<HTMLTextAreaElement>) { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }
  function date(value: string) { return new Date(value).toLocaleDateString(); }
  function entryView(entry: LibraryAskEntry) {
    const label = answerLabel(entry.state) || (entry.state === "answered" ? "Answered" : entry.state === "off_topic" ? "Off-topic" : "");
    return <li key={entry.id} className="grid gap-2 border-b border-line pb-5">
      <p className="m-0 text-sm font-medium">{entry.question}</p>
      {label && <span className={entry.state === "unverified" || entry.state === "incomplete" || entry.state === "failed" ? "text-xs text-warn" : "text-xs text-ink-muted"}>{label}</span>}
      {entry.state === "unverified" && <p className="m-0 text-xs text-warn">Some Citations could not be checked against these Meetings.</p>}
      {entry.state === "incomplete" && <p className="m-0 text-xs text-warn">This Answer stopped early.</p>}
      {entry.answer && <div className={entry.state === "insufficient_evidence" ? "rounded-md border border-dashed border-line-strong px-3 py-2.5 text-sm text-ink-muted" : "text-base"}><Markdown text={entry.answer} /></div>}
      <p className="m-0 text-xs text-ink-muted">Read {entry.meetings_read.length} {entry.meetings_read.length === 1 ? "Meeting" : "Meetings"}</p>
      {!!entry.citations.length && <div><p className="m-0 text-xs font-medium text-ink-muted">Citations</p><ul aria-label={`Citations for ${entry.question}`} className="m-0 list-none p-0">{entry.citations.map((citation) => {
        const meeting = meetings.find((item) => item.id === citation.meeting_id);
        return <li key={`${citation.meeting_id}:${citation.segment_id}`}><button type="button" className="text-left text-sm text-accent hover:underline" onClick={() => onOpenCitation(citation)}>{meeting?.title ?? "Meeting"} · {meeting ? date(meeting.started_at) : "Earlier Meeting"} · Segment {citation.segment_id}</button></li>;
      })}</ul></div>}
      {entry.error && <p role="alert" className="m-0 text-sm text-warn">{entry.error}</p>}
      <p className="m-0 text-xs text-ink-faint">{PROVIDER_NAMES[entry.provider]} · {entry.model}</p>
    </li>;
  }

  return <section aria-label="Library Ask" className="flex min-h-0 flex-1 flex-col">
    <div className="min-h-0 flex-1 overflow-y-auto px-7 py-5">
      {!loaded && <Loading message="Loading Library Asks…" rows={2} />}
      {error && <p role="alert" className="text-sm text-warn">{error}</p>}
      {busy && !plan && !view.active && <p role="status" className="text-sm text-ink-muted">Finding relevant Meetings…</p>}
      {loaded && !models.length && <div role="status" className="rounded-md bg-panel p-4 text-sm text-ink-muted">No Language model is set up for Ask. <button type="button" className="text-accent hover:underline" onClick={onOpenSettings}>Set one up →</button></div>}
      {view.active && <div className="mb-5 grid gap-2"><div aria-live="polite"><Markdown text={view.active.partial || "Answering…"} /></div><Button variant="ghost" className="justify-self-start" onClick={() => void stopLibraryAsk().catch((reason) => setError(String(reason)))}><Square size={16} strokeWidth={1.5} />Stop</Button></div>}
      {!!view.asks.length && <><h2 className="font-display text-heading-md">Past Library Asks</h2><ol aria-label="Past Library Asks" className="grid list-none gap-5 p-0">{[...view.asks].reverse().map(entryView)}</ol></>}
    </div>
    {models.length > 0 && <form onSubmit={submit} className="border-t border-line px-7 py-4">
      <div className="mb-3 flex flex-wrap gap-2" aria-label="Date range">{ranges.map((item) => <Button key={item.value} type="button" variant={range === item.value ? "secondary" : "ghost"} aria-pressed={range === item.value} disabled={busy || !!view.active || !!plan} onClick={() => setRange(item.value)}>{item.label}</Button>)}</div>
      <div className="flex items-end gap-2 rounded-md border border-line-strong bg-raised p-2"><Textarea rows={1} aria-label="Question" placeholder="Ask about your Library" value={question} onChange={(event) => setQuestion(event.target.value)} onKeyDown={keyDown} disabled={!selectedModel || busy || !!view.active || !!plan} className="min-h-8 flex-1 resize-none border-0 bg-transparent px-0 shadow-none" /><Button type="submit" size="icon" aria-label="Ask" disabled={!selectedModel || busy || !!view.active || !!plan || !question.trim()}><ArrowUp size={16} strokeWidth={1.5} /></Button></div>
      <div className="mt-2"><ModelPicker options={options} value={selected?.id ?? null} onChange={setChoice} onSetup={onOpenSettings} label="Choose Language model" disabled={busy || !!view.active || !!plan}>{selected?.label ?? "Choose a model"}</ModelPicker></div>
      {!selectedModel && <p role="status" className="text-xs text-ink-muted">{!defaultId ? "No default Language model is set for Ask." : "Choose a ready Language model for Ask."} <button type="button" className="text-accent hover:underline" onClick={onOpenSettings}>Language models in Settings →</button></p>}
    </form>}
    {plan && <LibraryAskConfirmation provider={plan.provider} meetings={plan.meetings} onSend={(excluded) => void send(plan, excluded, true)} onCancel={() => setPlan(null)} />}
  </section>;
}
