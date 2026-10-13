import { useEffect, useRef, useState } from "react";
import { Check } from "lucide-react";
import { acknowledgeCloudNotice, getSettings, installedModels, MODELS, modelSizeLabel, updateSettings, type Language, type Settings } from "./processing";
import { aiStatus, deleteApiKey, listAiModels, PROVIDER_NAMES, PROVIDERS, saveApiKey, testAiProvider, testCloudApiKey, updateAiSettings, type AiStatus, type ProviderKind } from "./ai";
import { openLink } from "./app-info";
import { Badge } from "./components/ui/Badge";
import { Button } from "./components/ui/Button";
import { Input } from "./components/ui/Input";
import { RadioGroup, RadioGroupItem } from "./components/ui/RadioGroup";
import { Switch } from "./components/ui/Switch";
import { Textarea } from "./components/ui/Textarea";
import { ErrorState } from "./components/states/ErrorState";
import { Loading } from "./components/states/Loading";
import { FormSection } from "./components/FormSection";

const selectClass = "h-[34px] w-full rounded-sm border border-line-strong bg-raised px-2.5 text-base outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--accent-soft)] disabled:opacity-50";

const PROVIDER_NOTES: Record<ProviderKind, string> = {
  ollama: "On this PC",
  openai: "Sends the Transcript to OpenAI when you click. Key kept in Windows Credential Manager.",
  anthropic: "Sends the Transcript to Anthropic when you click. Key kept in Windows Credential Manager.",
  groq: "Transcription key",
  gemini: "Correction key",
};

/** Settings with the typed Vocabulary, or null when it matches what is saved. */
function withVocabulary(settings: Settings | null, text: string): Settings | null {
  if (!settings) return null;
  const vocabulary = text.split("\n").map((term) => term.trim()).filter(Boolean);
  return vocabulary.join("\n") === settings.vocabulary.join("\n") ? null : { ...settings, vocabulary };
}

export default function SettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [load, setLoad] = useState(0);
  const [installed, setInstalled] = useState<string[] | null>(null);
  const [vocabulary, setVocabulary] = useState("");
  const [error, setError] = useState<{ reason: string; settings: Settings } | null>(null);
  const [saved, setSaved] = useState(false);
  const saveQueue = useRef(Promise.resolve());
  const saveVersion = useRef(0);
  const [providerStatus, setProviderStatus] = useState<AiStatus | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [key, setKey] = useState("");
  const [aiError, setAiError] = useState<string | null>(null);
  const [reply, setReply] = useState<string | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [cloudKeys, setCloudKeys] = useState({ groq: "", gemini: "" });
  const [cloudBusy, setCloudBusy] = useState(false);
  const [cloudResults, setCloudResults] = useState<{ groq?: string; gemini?: string }>({});
  const [cloudNoticeError, setCloudNoticeError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    setLoadFailed(false);
    getSettings().then((next) => { if (active) { setSettings(next); setVocabulary(next.vocabulary.join("\n")); } }, () => { if (active) setLoadFailed(true); });
    installedModels().then((names) => { if (active) setInstalled(names); }, () => {});
    return () => { active = false; };
  }, [load]);
  useEffect(() => {
    let active = true;
    aiStatus().then((next) => { if (active) setProviderStatus(next); }, (reason) => { if (active) setAiError(String(reason)); });
    return () => { active = false; };
  }, []);
  /** Shows the change at once and saves it after any save still running, so the last change wins. */
  function save(next: Settings) {
    setSettings(next);
    setSaved(false);
    setError(null);
    const version = ++saveVersion.current;
    saveQueue.current = saveQueue.current.then(async () => {
      try {
        await updateSettings(next.auto_process, next.model, next.vocabulary, next.language, next.your_name);
        if (version === saveVersion.current) setSaved(true);
      } catch (reason) {
        if (version === saveVersion.current) setError({ reason: String(reason), settings: next });
      }
    });
  }
  function saveVocabulary() {
    const next = withVocabulary(settings, vocabulary);
    if (!next) return;
    setVocabulary(next.vocabulary.join("\n"));
    save(next);
  }
  // Leaving Settings while typing in Vocabulary fires no blur, so save the edit on the way out.
  const unsaved = useRef({ settings, vocabulary });
  unsaved.current = { settings, vocabulary };
  useEffect(() => () => {
    const next = withVocabulary(unsaved.current.settings, unsaved.current.vocabulary);
    if (next) saveQueue.current = saveQueue.current.then(() => updateSettings(next.auto_process, next.model, next.vocabulary, next.language, next.your_name)).then(() => {}, () => {});
  }, []);
  async function runAi(action: () => Promise<AiStatus | string[] | string>) {
    setAiBusy(true); setAiError(null); setReply(null);
    try {
      const result = await action();
      if (Array.isArray(result)) setModels(result);
      else if (typeof result === "string") setReply(result);
      else setProviderStatus(result);
    } catch (reason) { setAiError(String(reason)); }
    finally { setAiBusy(false); }
  }
  const provider = providerStatus?.provider ?? null;
  const isCloud = provider === "openai" || provider === "anthropic";
  const hasKey = (kind: ProviderKind) => kind === "openai" ? providerStatus?.has_openai_key : kind === "anthropic" ? providerStatus?.has_anthropic_key : false;
  const selectedLabel = provider ? PROVIDER_NAMES[provider] : "";
  const cloudNoticeVisible = !!settings && !settings.cloud_notice_acknowledged && !!(providerStatus?.has_groq_key || providerStatus?.has_gemini_key);
  async function saveCloudKey(kind: "groq" | "gemini") {
    const entered = cloudKeys[kind];
    setCloudBusy(true);
    setCloudResults((current) => ({ ...current, [kind]: undefined }));
    try {
      const status = await saveApiKey(kind, entered);
      setProviderStatus(status);
      setCloudKeys((current) => ({ ...current, [kind]: "" }));
    } catch (reason) { setCloudResults((current) => ({ ...current, [kind]: String(reason) })); }
    finally { setCloudBusy(false); }
  }
  async function removeCloudKey(kind: "groq" | "gemini") {
    setCloudBusy(true);
    try { setProviderStatus(await deleteApiKey(kind)); setCloudResults((current) => ({ ...current, [kind]: undefined })); }
    catch (reason) { setCloudResults((current) => ({ ...current, [kind]: String(reason) })); }
    finally { setCloudBusy(false); }
  }
  async function testCloudKey(kind: "groq" | "gemini") {
    setCloudBusy(true);
    try { await testCloudApiKey(kind); setCloudResults((current) => ({ ...current, [kind]: "OK" })); }
    catch (reason) { setCloudResults((current) => ({ ...current, [kind]: String(reason) })); }
    finally { setCloudBusy(false); }
  }
  async function openCloudKeyPage(kind: "groq" | "gemini") {
    try { await openLink(kind); }
    catch (reason) { setCloudResults((current) => ({ ...current, [kind]: String(reason) })); }
  }
  return <section aria-label="Settings" className="min-h-0 flex-1 overflow-y-auto">
    <div className="grid max-w-[680px] gap-8 px-7 pb-12 pt-7">
      <div className="flex items-center gap-3">
        <h2 className="m-0 font-display text-heading-md">Settings</h2>
        {saved && <span role="status" className="inline-flex items-center gap-1 text-xs text-ink-muted"><Check size={14} strokeWidth={1.5} aria-hidden="true" />Saved</span>}
      </div>
      {error && <ErrorState title="Couldn't save Settings" guidance={`${error.reason}. The change shown below isn't saved yet.`} onRetry={() => save(error.settings)} />}
      {loadFailed ? <ErrorState title="Couldn't load Settings" guidance="Try again in a moment." onRetry={() => setLoad((value) => value + 1)} /> : !settings ? <Loading message="Loading Settings…" /> : <>
          <FormSection title="Cloud transcription" hint="Use your own keys for cloud Processing. Keys stay in Windows Credential Manager.">
            {(["groq", "gemini"] as const).map((kind) => {
              const label = PROVIDER_NAMES[kind];
              const stored = kind === "groq" ? providerStatus?.has_groq_key : providerStatus?.has_gemini_key;
              return <div key={kind} className="grid gap-2 rounded-md border border-line bg-raised p-3.5">
                <div className="flex flex-wrap items-center justify-between gap-2"><span className="font-medium">{label} · {kind === "groq" ? "Transcription" : "Correction"}</span><Button variant="ghost" onClick={() => void openCloudKeyPage(kind)}>Get a free {label} key</Button></div>
                <label htmlFor={`${kind}-key`} className="text-sm">{label} API key</label>
                <div className="flex flex-wrap gap-2"><Input id={`${kind}-key`} type="password" value={cloudKeys[kind]} onChange={(event) => setCloudKeys((current) => ({ ...current, [kind]: event.target.value }))} /><Button variant="secondary" disabled={cloudBusy || !cloudKeys[kind].trim()} onClick={() => void saveCloudKey(kind)}>Save {label} key</Button></div>
                <div className="flex flex-wrap items-center gap-2 text-sm"><span>{stored ? "Key saved" : "No key saved"}</span><Button variant="secondary" disabled={cloudBusy || !stored} onClick={() => void testCloudKey(kind)}>Test {label} key</Button>{stored && <Button variant="secondary" disabled={cloudBusy} onClick={() => void removeCloudKey(kind)}>Remove {label} key</Button>}</div>
                {cloudResults[kind] && <p role="status" className="m-0 text-sm">{label} key: {cloudResults[kind]}</p>}
              </div>;
            })}
          </FormSection>
          {cloudNoticeVisible && <div role="alert" className="mt-3 grid gap-2 rounded-md border border-line bg-raised p-3.5 text-sm"><p className="m-0">When Processing runs, Meeting audio or text will be sent to Groq or Gemini when their keys are set. Gemini's free tier may use content for training.</p><Button variant="secondary" onClick={() => void acknowledgeCloudNotice().then((next) => { setSettings(next); setCloudNoticeError(null); }, (reason) => setCloudNoticeError(String(reason)))}>Got it</Button>{cloudNoticeError && <p className="m-0">{cloudNoticeError}</p>}</div>}
        <FormSection title="Transcription model" hint="Runs on this PC. Larger models are more accurate and slower. You can change this any time.">
          <RadioGroup aria-label="Transcription model" value={settings.model} onValueChange={(model) => save({ ...settings, model })}>
            {MODELS.map((model) => <RadioGroupItem key={model.name} value={model.name} description={model.description} sideNote={<span className="grid justify-items-end tabular-nums">
              <span>{modelSizeLabel(model.size_mb)}</span>
              {installed && (installed.includes(model.name) ? <span className="text-ok">Downloaded</span> : <span>Downloads when first used</span>)}
            </span>}>
              <span className="inline-flex flex-wrap items-center gap-2">{model.label}{model.name === "small" && <Badge variant="beta">Default</Badge>}</span>
            </RadioGroupItem>)}
          </RadioGroup>
        </FormSection>
        <FormSection title="Meetings">
          <div className="grid gap-1.5"><label htmlFor="your-name" className="text-sm font-medium">Your name</label><Input id="your-name" value={settings.your_name ?? ""} placeholder="Me" onChange={(event) => save({ ...settings, your_name: event.target.value || null })} /><span className="text-xs text-ink-muted">Default Participant for Me in Meetings without a name override.</span></div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5">
              <label htmlFor="default-language" className="text-sm font-medium">Default language</label>
              <select id="default-language" className={selectClass} value={settings.language ?? ""} onChange={(event) => save({ ...settings, language: (event.target.value || null) as Language | null })}>
                <option value="">Detect automatically</option>
                <option value="en">English</option>
                <option value="vi">Tiếng Việt</option>
              </select>
            </div>
            <div className="grid gap-1.5">
              <span id="library-folder" className="text-sm font-medium">Library folder</span>
              <span aria-labelledby="library-folder" className="flex h-[34px] items-center rounded-sm border border-line-strong bg-raised px-2.5 text-base text-ink-muted">Documents\Meeting Minutes</span>
            </div>
          </div>
          <div className="grid gap-1.5">
            <label htmlFor="vocabulary" className="text-sm font-medium">Vocabulary</label>
            <Textarea id="vocabulary" aria-describedby="vocabulary-note" rows={3} value={vocabulary} onChange={(event) => setVocabulary(event.target.value)} onBlur={saveVocabulary} />
            <span id="vocabulary-note" className="text-xs text-ink-muted">Names and terms to help transcription spell them right. One per line.</span>
          </div>
          <div className="flex items-center justify-between gap-4 rounded-md border border-line bg-raised px-3.5 py-3">
            <div className="grid gap-0.5">
              <span id="auto-process" className="text-base font-medium">Process after I stop</span>
              <span id="auto-process-note" className="text-xs text-ink-muted">Transcribe on this PC as soon as a Recording ends.</span>
            </div>
            <Switch aria-labelledby="auto-process" aria-describedby="auto-process-note" checked={settings.auto_process} onCheckedChange={(auto_process) => save({ ...settings, auto_process })} />
          </div>
        </FormSection>
      </>}
      <FormSection title="Summary and Ask" hint="Local by default. Cloud providers use your own key and only run when you click.">
        <RadioGroup aria-label="Provider" value={provider ?? "none"} disabled={aiBusy || !providerStatus} onValueChange={(value) => {
          const next = value === "none" ? null : value as ProviderKind;
          setModels([]); void runAi(() => updateAiSettings(next, null));
        }}>
          {PROVIDERS.map((item) => <RadioGroupItem key={item.kind} value={item.kind} description={`${PROVIDER_NOTES[item.kind]}${item.kind === provider && providerStatus?.model ? ` · ${providerStatus.model}` : ""}`} sideNote={item.kind === provider ? (providerStatus?.configured ? <span className="inline-flex items-center gap-1 font-medium text-ok"><Check size={14} strokeWidth={1.5} aria-hidden="true" />Ready</span> : "Not set up") : hasKey(item.kind) ? "Key saved" : undefined}>
            <span className="inline-flex items-center gap-2">{PROVIDER_NAMES[item.kind]}{item.kind === provider && <Badge variant="accent" className="rounded-full">In use</Badge>}</span>
          </RadioGroupItem>)}
          <RadioGroupItem value="none" description="Summary and Ask stay off.">None</RadioGroupItem>
        </RadioGroup>
        {isCloud && (hasKey(provider) ? <p className="m-0 flex flex-wrap items-center gap-2 text-sm text-ink-muted">API key saved in Windows Credential Manager <Button variant="secondary" disabled={aiBusy} onClick={() => void runAi(() => deleteApiKey(provider))}>Remove key</Button></p> : <div className="grid max-w-sm gap-1.5">
          <label htmlFor="ai-key" className="text-sm font-medium">API key</label>
          <div className="flex gap-2"><Input id="ai-key" type="password" value={key} onChange={(event) => setKey(event.target.value)} />
            <Button variant="secondary" disabled={aiBusy} onClick={() => { const entered = key; setKey(""); void runAi(() => saveApiKey(provider, entered)); }}>Save key</Button></div>
        </div>)}
        {provider && <div className="flex flex-wrap items-end gap-2">
          {(models.length > 0 || providerStatus?.model) && <div className="grid min-w-56 gap-1.5"><label htmlFor="ai-model" className="text-sm font-medium">Model</label>
            <select id="ai-model" className={selectClass} value={providerStatus?.model ?? ""} disabled={aiBusy} onChange={(event) => void runAi(() => updateAiSettings(provider, event.target.value))}>
              <option value="">Choose a model</option>
              {Array.from(new Set([...(providerStatus?.model ? [providerStatus.model] : []), ...models])).map((model) => <option key={model} value={model}>{model}</option>)}
            </select></div>}
          <Button variant="secondary" disabled={aiBusy} onClick={() => void runAi(() => listAiModels(provider))}>{provider === "ollama" ? "Find Ollama models" : "Load models"}</Button>
          {providerStatus?.configured && <Button variant="secondary" disabled={aiBusy} onClick={() => void runAi(testAiProvider)}>{isCloud ? `Send test message to ${selectedLabel}` : "Test"}</Button>}
        </div>}
        {providerStatus?.configured && <p className="m-0 text-xs text-ink-muted">The test sends: “You are a connection test.” and “Reply with the single word: ready”. No Meeting content is sent.</p>}
        {reply !== null && <p role="status" className="m-0 text-sm">Test reply: {reply}</p>}
        {aiError && <ErrorState title={`Couldn't complete that with ${selectedLabel || "the provider"}`} guidance={`${aiError}. ${provider === "ollama" ? "Install Ollama from ollama.com, start it, and pull a model (for example: ollama pull llama3.2)." : "Check the key and your connection, then try again."}`} />}
      </FormSection>
    </div>
  </section>;
}
