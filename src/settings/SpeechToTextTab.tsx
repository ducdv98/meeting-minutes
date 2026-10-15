import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { cancelModelDownload, deleteModelDownload, GROQ_SPEECH_MODELS, MODELS, modelSizeLabel, startModelDownload, testSpeechModel, type Settings, type SpeechProvider, type SpeechSettings } from "../processing";
import { listAiModels, managedDeleteAccount, managedDisconnect, managedSendCode, managedSetEnabled, managedSignIn, managedStatus, PROVIDER_NAMES, type AiStatus, type ManagedStatus } from "../ai";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { Input } from "../components/ui/Input";
import { Switch } from "../components/ui/Switch";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "../components/ui/Dialog";
import { RadioGroup, RadioGroupItem } from "../components/ui/RadioGroup";
import { Textarea } from "../components/ui/Textarea";
import { FormSection } from "../components/FormSection";
import { Details } from "../components/Details";
import { MANAGED_PRIVACY_STATEMENT, MANAGED_PRIVACY_SUMMARY } from "../privacy-text";
import { useModelDownloads } from "./useModelDownloads";

type Props = {
  settings: Settings;
  save: (next: Settings) => void;
  installed: string[] | null;
  vocabulary: string;
  setVocabulary: Dispatch<SetStateAction<string>>;
  saveVocabulary: () => void;
  providerStatus: AiStatus | null;
  onManageKeys: () => void;
};

export function SpeechToTextTab({ settings, save, installed, vocabulary, setVocabulary, saveVocabulary, providerStatus, onManageKeys }: Props) {
  const speech: SpeechSettings = settings.speech ?? { models: [{ provider: "groq", model: "whisper-large-v3" }, { provider: "gemini", model: settings.ai.gemini_model ?? "gemini-2.5-flash" }], automatic_groq: "whisper-large-v3", automatic_gemini: settings.ai.gemini_model ?? "gemini-2.5-flash", automatic_whisper: settings.model };
  const [managed, setManaged] = useState<ManagedStatus | null>(null);
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [codeSent, setCodeSent] = useState(false);
  const [managedBusy, setManagedBusy] = useState(false);
  const [managedError, setManagedError] = useState<string | null>(null);
  const [signInOpen, setSignInOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [adding, setAdding] = useState(false);
  const [provider, setProvider] = useState<SpeechProvider>("groq");
  const [model, setModel] = useState<string>(GROQ_SPEECH_MODELS[0]);
  const [geminiModels, setGeminiModels] = useState<string[]>([]);
  const [tested, setTested] = useState(false);
  const [modelBusy, setModelBusy] = useState(false);
  const [modelError, setModelError] = useState<string | null>(null);
  const { downloads, error: downloadError, run: runDownload } = useModelDownloads();
  useEffect(() => {
    let active = true;
    managedStatus().then((next) => { if (active) setManaged(next); }, (reason) => { if (active) setManagedError(String(reason)); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    if (!adding || provider !== "gemini" || !providerStatus?.has_gemini_key) return;
    let active = true;
    listAiModels("gemini").then((names) => { if (active) setGeminiModels(names); }, (reason) => { if (active) setModelError(String(reason)); });
    return () => { active = false; };
  }, [adding, provider, providerStatus?.has_gemini_key]);
  function setAutomatic(kind: SpeechProvider, selected: string | null) {
    save({ ...settings, speech: { ...speech, [kind === "groq" ? "automatic_groq" : "automatic_gemini"]: selected } });
  }
  function removeModel(kind: SpeechProvider, selected: string) {
    const models = speech.models.filter((entry) => entry.provider !== kind || entry.model !== selected);
    const key = kind === "groq" ? "automatic_groq" : "automatic_gemini";
    const automatic = speech[key] === selected ? models.find((entry) => entry.provider === kind)?.model ?? null : speech[key];
    save({ ...settings, speech: { ...speech, models, [key]: automatic } });
  }
  async function testModel() {
    setModelBusy(true); setModelError(null); setTested(false);
    try { await testSpeechModel(provider, model.trim()); setTested(true); }
    catch (reason) { setModelError(String(reason)); }
    finally { setModelBusy(false); }
  }
  function addModel() {
    const selected = model.trim();
    const models = speech.models.some((entry) => entry.provider === provider && entry.model === selected) ? speech.models : [...speech.models, { provider, model: selected }];
    const key = provider === "groq" ? "automatic_groq" : "automatic_gemini";
    save({ ...settings, speech: { ...speech, models, [key]: speech[key] ?? selected } });
    setAdding(false); setTested(false);
  }
  const groqReady = !!providerStatus?.has_groq_key && !!speech.automatic_groq;
  const geminiReady = !!providerStatus?.has_gemini_key && !!speech.automatic_gemini;
  const managedReady = !!managed?.connected && (managed.enabled ?? !managed.email) && !managed.revoked;
  const first = managedReady ? "Managed transcription" : groqReady ? `Groq · ${speech.automatic_groq}` : geminiReady ? `Gemini · ${speech.automatic_gemini}` : `Local Whisper · ${speech.automatic_whisper}`;
  function localStatus(name: string) {
    return downloads[name] ?? { model_id: name, state: installed?.includes(name) ? "downloaded" as const : "not_downloaded" as const, progress: 0, error: null };
  }
  async function changeDownload(name: string, action: "start" | "cancel" | "delete") {
    await runDownload(name, () => action === "start" ? startModelDownload(name) : action === "cancel" ? cancelModelDownload(name) : deleteModelDownload(name));
  }
  async function sendCode() {
    setManagedBusy(true); setManagedError(null);
    try { await managedSendCode(email.trim()); setCodeSent(true); setCode(""); }
    catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
  async function signIn() {
    setManagedBusy(true); setManagedError(null);
    try { setManaged(await managedSignIn(email.trim(), code)); setCode(""); setCodeSent(false); setSignInOpen(false); }
    catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
  async function disconnectManaged() {
    setManagedBusy(true); setManagedError(null);
    try { setManaged(await managedDisconnect()); setCode(""); setCodeSent(false); setSignInOpen(false); }
    catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
  async function setManagedConsent(enabled: boolean) {
    setManagedBusy(true); setManagedError(null);
    try { setManaged(await managedSetEnabled(enabled)); }
    catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
  async function deleteAccount() {
    setManagedBusy(true); setManagedError(null);
    try { setManaged(await managedDeleteAccount()); setConfirmDelete(false); setCodeSent(false); setSignInOpen(false); }
    catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
  return <div className="grid gap-8">
    <FormSection title="Transcription order" hint="Processing tries each ready Provider in this fixed order. If one fails or hits its limit, it tries the next.">
      <ol className="m-0 grid gap-2 p-0 text-sm list-none">
        <li className="flex justify-between gap-3"><span>Managed transcription</span><span className="text-ink-muted">{managedReady ? "Ready" : managed?.connected ? "Off" : "Not connected"}</span></li>
        <li className="flex justify-between gap-3"><span>Groq · {speech.automatic_groq ?? "No automatic model"}</span><span className="text-ink-muted">{providerStatus?.has_groq_key ? speech.automatic_groq ? "Ready" : "No model" : "No key"}</span></li>
        <li className="flex justify-between gap-3"><span>Gemini · {speech.automatic_gemini ?? "No automatic model"}</span><span className="text-ink-muted">{providerStatus?.has_gemini_key ? speech.automatic_gemini ? "Ready" : "No model" : "No key"}</span></li>
        <li className="flex justify-between gap-3"><span>Local Whisper · {speech.automatic_whisper}</span><span className="text-ink-muted">{localStatus(speech.automatic_whisper).state === "downloaded" ? "Ready" : "Download required"}</span></li>
      </ol>
      <p className="m-0 text-sm font-medium">New Meetings use: {first}</p>
    </FormSection>
    <FormSection title="Account">
      {managed?.email && !signInOpen ? <>
        <p role="status" className="m-0 text-sm">Signed in as {managed.email}</p>
        <div className="flex flex-wrap gap-2"><Button variant="secondary" disabled={managedBusy} onClick={() => void disconnectManaged()}>Sign out</Button><Button variant="ghost" disabled={managedBusy} onClick={() => setConfirmDelete(true)}>Delete account</Button></div>
      </> : <>
        {managed?.connected && !managed.email && !signInOpen && <div className="flex flex-wrap items-center gap-2 text-sm"><span role="status">Connected (legacy invite)</span><Button variant="secondary" disabled={managedBusy} onClick={() => setSignInOpen(true)}>Sign in</Button><Button variant="ghost" disabled={managedBusy} onClick={() => void disconnectManaged()}>Sign out</Button></div>}
        {(!managed?.connected || signInOpen) && <div className="grid max-w-sm gap-3">
          {managed?.revoked && <p role="status" className="m-0 text-sm">Your previous connection expired. Sign in again.</p>}
          <div className="grid gap-1.5"><label htmlFor="account-email" className="text-sm">Email address</label><Input id="account-email" type="email" autoComplete="email" value={email} onChange={(event) => { setEmail(event.target.value); setCodeSent(false); }} /></div>
          <Button variant="secondary" className="justify-self-start" disabled={managedBusy || !email.trim()} onClick={() => void sendCode()}>{codeSent ? "Send new code" : "Send code"}</Button>
          {codeSent && <><p role="status" className="m-0 text-sm">If this address can receive mail, a sign-in code has been sent.</p><div className="grid gap-1.5"><label htmlFor="account-code" className="text-sm">Six-digit code</label><Input id="account-code" inputMode="numeric" autoComplete="one-time-code" maxLength={6} value={code} onChange={(event) => setCode(event.target.value.replace(/\D/g, "").slice(0, 6))} /></div><Button className="justify-self-start" disabled={managedBusy || code.length !== 6} onClick={() => void signIn()}>Sign in</Button></>}
        </div>}
      </>}
      {managedError && !confirmDelete && <p role="alert" className="m-0 text-sm text-record">{managedError}</p>}
      {confirmDelete && <Dialog open onOpenChange={(open) => { if (!open && !managedBusy) setConfirmDelete(false); }}><DialogContent onEscapeKeyDown={(event) => { if (managedBusy) event.preventDefault(); }} onPointerDownOutside={(event) => { if (managedBusy) event.preventDefault(); }}><DialogTitle>Delete account?</DialogTitle><DialogDescription>We will delete your email address and Managed transcription usage history, and sign you out on every PC. All Meetings and Transcripts on this PC stay here. Local Processing and Processing with your own keys will keep working.</DialogDescription>{managedError && <p role="alert" className="mt-3 text-sm text-record">{managedError}</p>}<div className="mt-6 flex justify-end gap-2"><Button variant="secondary" disabled={managedBusy} onClick={() => setConfirmDelete(false)}>Cancel</Button><Button variant="record" disabled={managedBusy} onClick={() => void deleteAccount()}>Delete account</Button></div></DialogContent></Dialog>}
    </FormSection>
    <FormSection title="Managed transcription">
      <div className="flex items-center gap-3"><Switch aria-label="Enable Managed transcription" checked={managedReady} disabled={!managed?.email || managedBusy} onCheckedChange={(enabled) => void setManagedConsent(enabled)} /><span className="text-sm">{managedReady ? "On" : "Off"}</span></div>
      <p className="m-0 text-sm text-ink-muted">Turning this on allows Meeting audio to be sent to Managed transcription. Signing in alone does not send audio.</p>
      <div className="grid gap-1"><p className="m-0 text-xs text-ink-muted">{MANAGED_PRIVACY_SUMMARY}</p><Details><p className="m-0">{MANAGED_PRIVACY_STATEMENT}</p></Details></div>
    </FormSection>
    <FormSection title="Cloud transcription" hint="Use your own keys for cloud Processing. Keys stay in Windows Credential Manager.">
      {(["groq", "gemini"] as const).map((kind) => {
        const label = PROVIDER_NAMES[kind];
        const stored = kind === "groq" ? providerStatus?.has_groq_key : providerStatus?.has_gemini_key;
        return <p key={kind} className="m-0 flex flex-wrap items-center gap-2 text-sm"><span className="font-medium">{label} · {kind === "groq" ? "Speech-to-text" : "Speech-to-text and Language models"}</span><span className="text-ink-muted">{stored ? "Key saved" : "No key"}</span><button type="button" className="text-accent hover:underline" onClick={onManageKeys}>Manage keys</button></p>;
      })}
    </FormSection>
    <FormSection title="Speech-to-text models" hint="Choose one model per Provider to use automatically. Other models stay available for a single transcription.">
      {managedReady && <div className="flex flex-wrap items-center gap-2 rounded-md border border-line bg-raised px-3 py-2 text-sm"><span className="min-w-0 flex-1">Managed transcription · Fixed model</span><Badge>Used automatically</Badge><span className="text-ink-muted">Ready</span></div>}
      <div className="grid gap-2">{speech.models.map((entry) => { const automatic = speech[entry.provider === "groq" ? "automatic_groq" : "automatic_gemini"] === entry.model; const hasKey = entry.provider === "groq" ? providerStatus?.has_groq_key : providerStatus?.has_gemini_key; return <div key={`${entry.provider}-${entry.model}`} className="flex flex-wrap items-center gap-2 rounded-md border border-line bg-raised px-3 py-2 text-sm"><span className="min-w-0 flex-1">{PROVIDER_NAMES[entry.provider]} · {entry.model}</span>{automatic ? <Badge>Used automatically</Badge> : <Button variant="secondary" onClick={() => setAutomatic(entry.provider, entry.model)}>Use automatically</Button>}<span className="text-ink-muted">{hasKey ? "Ready" : "No key"}</span><Button variant="ghost" onClick={() => removeModel(entry.provider, entry.model)}>Remove</Button></div>; })}</div>
      {!adding ? <Button variant="secondary" className="justify-self-start" onClick={() => setAdding(true)}>Add Speech-to-text model</Button> : <div className="grid gap-3 rounded-md border border-line bg-raised p-3.5">
        <div className="grid gap-1"><label htmlFor="speech-provider" className="text-sm">Provider</label><select id="speech-provider" className="rounded-sm border border-line-strong bg-raised px-2 py-1.5 text-sm" value={provider} onChange={(event) => { const kind = event.target.value as SpeechProvider; setProvider(kind); setModel(kind === "groq" ? GROQ_SPEECH_MODELS[0] : ""); setTested(false); setModelError(null); }}><option value="groq">Groq</option><option value="gemini">Gemini</option></select></div>
        <div className="grid gap-1"><label htmlFor="speech-model" className="text-sm">Model ID</label>{provider === "groq" ? <select id="speech-model" className="rounded-sm border border-line-strong bg-raised px-2 py-1.5 text-sm" value={model} onChange={(event) => { setModel(event.target.value); setTested(false); }}>{GROQ_SPEECH_MODELS.map((name) => <option key={name} value={name}>{name}</option>)}</select> : <><Input id="speech-model" list="gemini-speech-models" value={model} onChange={(event) => { setModel(event.target.value); setTested(false); }} placeholder="Gemini model ID" /><datalist id="gemini-speech-models">{geminiModels.map((name) => <option key={name} value={name} />)}</datalist></>}</div>
        {!(provider === "groq" ? providerStatus?.has_groq_key : providerStatus?.has_gemini_key) && <p className="m-0 text-sm text-ink-muted">No key. <button type="button" className="text-accent hover:underline" onClick={onManageKeys}>Add a key in Keys</button></p>}
        <div className="flex gap-2"><Button variant="secondary" disabled={modelBusy || !model.trim() || !(provider === "groq" ? providerStatus?.has_groq_key : providerStatus?.has_gemini_key)} onClick={() => void testModel()}>Test</Button><Button disabled={!tested} onClick={addModel}>Add model</Button><Button variant="ghost" onClick={() => { setAdding(false); setModelError(null); }}>Cancel</Button></div>
        {tested && <p role="status" className="m-0 text-sm text-ok">Model ready</p>}{modelError && <p role="alert" className="m-0 text-sm text-record">{modelError}</p>}
      </div>}
    </FormSection>
    <FormSection title="Local Whisper" hint="Runs on this PC after cloud Providers, or when no cloud Provider is ready. Larger models are more accurate and slower.">
      <RadioGroup aria-label="Transcription model" value={speech.automatic_whisper} onValueChange={(model) => save({ ...settings, model, ...(settings.speech ? { speech: { ...speech, automatic_whisper: model } } : {}) })}>
        {MODELS.map((model) => { const status = localStatus(model.name); return <div key={model.name} className="border-b border-line last:border-b-0"><RadioGroupItem value={model.name} description={model.description} className="border-b-0" sideNote={<span className="grid justify-items-end tabular-nums"><span>{modelSizeLabel(model.size_mb)}</span><span>{status.state === "downloaded" ? "Downloaded" : status.state === "downloading" ? `${status.progress}%` : "Download required"}</span></span>}>
          <span className="inline-flex flex-wrap items-center gap-2">{model.label}{model.name === "small" && <Badge variant="beta">Default</Badge>}</span>
        </RadioGroupItem><div className="flex items-center justify-end gap-2 px-3.5 pb-3 text-xs">
          {status.state === "downloading" ? <Button variant="ghost" onClick={() => void changeDownload(model.name, "cancel")}>Cancel {model.label}</Button> : status.state === "downloaded" ? <Button variant="ghost" onClick={() => void changeDownload(model.name, "delete")}>Delete {model.label}</Button> : <><span>{status.state === "failed" ? status.error : status.state === "cancelled" ? "Cancelled" : "Download required"}</span><Button variant="secondary" onClick={() => void changeDownload(model.name, "start")}>Download {model.label}</Button></>}
        </div></div>; })}
      </RadioGroup>
      {downloadError && <p role="alert" className="m-0 text-sm text-record">{downloadError}</p>}
    </FormSection>
    <FormSection title="Vocabulary">
      <div className="grid gap-1.5">
        <label htmlFor="vocabulary" className="text-sm font-medium">Vocabulary</label>
        <Textarea id="vocabulary" aria-describedby="vocabulary-note" rows={3} value={vocabulary} onChange={(event) => setVocabulary(event.target.value)} onBlur={saveVocabulary} />
        <span id="vocabulary-note" className="text-xs text-ink-muted">Names and terms to help transcription spell them right. One per line.</span>
      </div>
    </FormSection>
  </div>;
}
