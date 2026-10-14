import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { GROQ_SPEECH_MODELS, MODELS, modelSizeLabel, testSpeechModel, type Settings, type SpeechProvider, type SpeechSettings } from "../processing";
import { listAiModels, managedConnect, managedDisconnect, managedStatus, PROVIDER_NAMES, type AiStatus, type ManagedStatus } from "../ai";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { Input } from "../components/ui/Input";
import { RadioGroup, RadioGroupItem } from "../components/ui/RadioGroup";
import { Textarea } from "../components/ui/Textarea";
import { FormSection } from "../components/FormSection";
import { Details } from "../components/Details";
import { MANAGED_PRIVACY_STATEMENT, MANAGED_PRIVACY_SUMMARY } from "../privacy-text";

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
  const [inviteCode, setInviteCode] = useState("");
  const [managedBusy, setManagedBusy] = useState(false);
  const [managedError, setManagedError] = useState<string | null>(null);
  const [inviteOpen, setInviteOpen] = useState(false);
  const [adding, setAdding] = useState(false);
  const [provider, setProvider] = useState<SpeechProvider>("groq");
  const [model, setModel] = useState<string>(GROQ_SPEECH_MODELS[0]);
  const [geminiModels, setGeminiModels] = useState<string[]>([]);
  const [tested, setTested] = useState(false);
  const [modelBusy, setModelBusy] = useState(false);
  const [modelError, setModelError] = useState<string | null>(null);
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
  const first = managed?.connected ? "Managed transcription" : groqReady ? `Groq · ${speech.automatic_groq}` : geminiReady ? `Gemini · ${speech.automatic_gemini}` : `Local Whisper · ${speech.automatic_whisper}`;
  async function connectManaged() {
    setManagedBusy(true); setManagedError(null);
    try { setManaged(await managedConnect(inviteCode)); setInviteCode(""); }
    catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
  async function disconnectManaged() {
    setManagedBusy(true); setManagedError(null);
    try { setManaged(await managedDisconnect()); setInviteCode(""); setInviteOpen(false); }
    catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
  return <div className="grid gap-8">
    <FormSection title="Transcription order" hint="Processing tries each ready Provider in this fixed order. If one fails or hits its limit, it tries the next.">
      <ol className="m-0 grid gap-2 p-0 text-sm list-none">
        <li className="flex justify-between gap-3"><span>Managed transcription</span><span className="text-ink-muted">{managed?.connected ? "Ready" : "Not connected"}</span></li>
        <li className="flex justify-between gap-3"><span>Groq · {speech.automatic_groq ?? "No automatic model"}</span><span className="text-ink-muted">{providerStatus?.has_groq_key ? speech.automatic_groq ? "Ready" : "No model" : "No key"}</span></li>
        <li className="flex justify-between gap-3"><span>Gemini · {speech.automatic_gemini ?? "No automatic model"}</span><span className="text-ink-muted">{providerStatus?.has_gemini_key ? speech.automatic_gemini ? "Ready" : "No model" : "No key"}</span></li>
        <li className="flex justify-between gap-3"><span>Local Whisper · {speech.automatic_whisper}</span><span className="text-ink-muted">{installed?.includes(speech.automatic_whisper) ? "Ready" : "Downloads when first used"}</span></li>
      </ol>
      <p className="m-0 text-sm font-medium">New Meetings use: {first}</p>
    </FormSection>
    <FormSection title="Managed transcription (experimental)">
      {managed?.connected ? <div className="flex flex-wrap items-center gap-2 text-sm"><span role="status">Connected</span><Button variant="secondary" disabled={managedBusy} onClick={() => void disconnectManaged()}>Disconnect</Button></div> : (managed?.revoked || inviteOpen) ? <>
        {managed?.revoked && <p role="status" className="m-0 text-sm">Managed transcription was disconnected.</p>}
        <div className="grid max-w-sm gap-1.5">
          <label htmlFor="managed-invite-code" className="text-sm">Invite code</label>
          <div className="flex gap-2"><Input id="managed-invite-code" type="password" value={inviteCode} onChange={(event) => setInviteCode(event.target.value)} /><Button variant="secondary" disabled={managedBusy || !managed || !inviteCode.trim()} onClick={() => void connectManaged()}>Connect</Button></div>
        </div>
      </> : <button type="button" className="justify-self-start text-sm font-medium text-accent hover:underline" onClick={() => setInviteOpen(true)}>Have an invite code?</button>}
      {managedError && <p role="alert" className="m-0 text-sm text-record">{managedError}</p>}
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
      {managed?.connected && <div className="flex flex-wrap items-center gap-2 rounded-md border border-line bg-raised px-3 py-2 text-sm"><span className="min-w-0 flex-1">Managed transcription · Fixed model</span><Badge>Used automatically</Badge><span className="text-ink-muted">Ready</span></div>}
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
        {MODELS.map((model) => <RadioGroupItem key={model.name} value={model.name} description={model.description} sideNote={<span className="grid justify-items-end tabular-nums">
          <span>{modelSizeLabel(model.size_mb)}</span>
          {installed && (installed.includes(model.name) ? <span className="text-ok">Downloaded</span> : <span>Downloads when first used</span>)}
        </span>}>
          <span className="inline-flex flex-wrap items-center gap-2">{model.label}{model.name === "small" && <Badge variant="beta">Default</Badge>}</span>
        </RadioGroupItem>)}
      </RadioGroup>
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
