import { useState, type Dispatch, type SetStateAction } from "react";
import { Check } from "lucide-react";
import { aiStatus, listAiModels, PROVIDER_NAMES, PROVIDERS, testAiProvider, testLanguageModel, updateAiSettings, updateLanguageModels, type AiStatus, type LanguageModel, type LanguageModelSettings, type ProviderKind } from "../ai";
import type { Settings } from "../processing";
import { Badge } from "../components/ui/Badge";
import { Button } from "../components/ui/Button";
import { RadioGroup, RadioGroupItem } from "../components/ui/RadioGroup";
import { ErrorState } from "../components/states/ErrorState";
import { FormSection } from "../components/FormSection";

const selectClass = "h-[34px] w-full rounded-sm border border-line-strong bg-raised px-2.5 text-base outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--accent-soft)] disabled:opacity-50";

const PROVIDER_NOTES: Record<ProviderKind, string> = {
  ollama: "On this PC",
  openai: "Sends the Transcript to OpenAI when you click. Key kept in Windows Credential Manager.",
  anthropic: "Sends the Transcript to Anthropic when you click. Key kept in Windows Credential Manager.",
  groq: "Transcription key",
  gemini: "Sends the Transcript to Gemini when you click. Also used for Correction. Key kept in Windows Credential Manager.",
};

function LegacyLanguageModelsTab({ providerStatus, setProviderStatus, aiError, setAiError, onManageKeys }: { providerStatus: AiStatus | null; setProviderStatus: Dispatch<SetStateAction<AiStatus | null>>; aiError: string | null; setAiError: Dispatch<SetStateAction<string | null>>; onManageKeys: () => void }) {
  const [models, setModels] = useState<string[]>([]);
  const [reply, setReply] = useState<string | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
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
  const isCloud = provider === "openai" || provider === "anthropic" || provider === "gemini";
  const hasKey = (kind: ProviderKind) => kind === "openai" ? providerStatus?.has_openai_key : kind === "anthropic" ? providerStatus?.has_anthropic_key : kind === "gemini" ? providerStatus?.has_gemini_key : false;
  const selectedLabel = provider ? PROVIDER_NAMES[provider] : "";
  return <FormSection title="Summary and Ask" hint="Local by default. Cloud providers use your own key and only run when you click.">
    <RadioGroup aria-label="Provider" value={provider ?? "none"} disabled={aiBusy || !providerStatus} onValueChange={(value) => {
      const next = value === "none" ? null : value as ProviderKind;
      setModels([]); void runAi(() => updateAiSettings(next, null));
    }}>
      {PROVIDERS.map((item) => <RadioGroupItem key={item.kind} value={item.kind} description={`${PROVIDER_NOTES[item.kind]}${item.kind === provider && providerStatus?.model ? ` · ${providerStatus.model}` : ""}`} sideNote={item.kind === provider ? (providerStatus?.configured ? <span className="inline-flex items-center gap-1 font-medium text-ok"><Check size={14} strokeWidth={1.5} aria-hidden="true" />Ready</span> : "Not set up") : hasKey(item.kind) ? "Key saved" : undefined}>
        <span className="inline-flex items-center gap-2">{PROVIDER_NAMES[item.kind]}{item.kind === provider && <Badge variant="accent" className="rounded-full">In use</Badge>}</span>
      </RadioGroupItem>)}
      <RadioGroupItem value="none" description="Summary and Ask stay off.">None</RadioGroupItem>
    </RadioGroup>
    {isCloud && <p className="m-0 flex flex-wrap items-center gap-2 text-sm text-ink-muted">{hasKey(provider) ? "Key saved in Windows Credential Manager" : "No key"}<button type="button" className="text-accent hover:underline" onClick={onManageKeys}>Manage keys</button></p>}
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
  </FormSection>;
}

const identity = (item: LanguageModel) => JSON.stringify(item);
const label = (item: LanguageModel) => `${PROVIDER_NAMES[item.provider]} · ${item.model}`;
type UseFor = "correction" | "summary" | "ask";

export function LanguageModelsTab({ settings, setSettings, providerStatus, setProviderStatus, aiError, setAiError, onManageKeys }: { settings?: Settings; setSettings?: Dispatch<SetStateAction<Settings | null>>; providerStatus: AiStatus | null; setProviderStatus: Dispatch<SetStateAction<AiStatus | null>>; aiError: string | null; setAiError: Dispatch<SetStateAction<string | null>>; onManageKeys: () => void }) {
  const [provider, setProvider] = useState<LanguageModel["provider"]>("ollama");
  const [available, setAvailable] = useState<string[]>([]);
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false);
  const [reply, setReply] = useState<string | null>(null);
  const [pendingCorrection, setPendingCorrection] = useState<LanguageModel | null>(null);
  const [pendingChange, setPendingChange] = useState<LanguageModelSettings | null>(null);
  const [removing, setRemoving] = useState<LanguageModel | null>(null);
  const [replacement, setReplacement] = useState("");
  if (!settings?.language_models || !setSettings) return <LegacyLanguageModelsTab providerStatus={providerStatus} setProviderStatus={setProviderStatus} aiError={aiError} setAiError={setAiError} onManageKeys={onManageKeys} />;
  const current = settings.language_models;
  const commitSettings = setSettings;
  const hasKey = (kind: ProviderKind) => kind === "ollama" || !!providerStatus?.[`has_${kind}_key` as keyof AiStatus];
  async function save(next: LanguageModelSettings) {
    setBusy(true); setAiError(null);
    try { commitSettings(await updateLanguageModels(next)); setProviderStatus(await aiStatus()); }
    catch (reason) { setAiError(String(reason)); }
    finally { setBusy(false); }
  }
  function choose(use: UseFor, value: string) {
    const selected = current.models.find((item) => identity(item) === value) ?? null;
    const next = { ...current, [use]: selected };
    if (use === "correction" && selected?.provider !== undefined && selected.provider !== "ollama" && !current.confirmed_correction_providers.includes(selected.provider)) { setPendingCorrection(selected); setPendingChange(next); return; }
    void save(next);
  }
  function remove() {
    if (!removing) return;
    const chosen = current.models.find((item) => identity(item) === replacement) ?? null;
    const next = current.models.filter((item) => identity(item) !== identity(removing));
    const update = (use: UseFor) => current[use] && identity(current[use]) === identity(removing) ? chosen : current[use];
    const change = { ...current, models: next, correction: update("correction"), summary: update("summary"), ask: update("ask") };
    setRemoving(null);
    if (change.correction?.provider !== "ollama" && change.correction && !current.confirmed_correction_providers.includes(change.correction.provider)) { setPendingCorrection(change.correction); setPendingChange(change); return; }
    void save(change);
  }
  return <div className="grid gap-8">
    <FormSection title="Language models" hint="Add models for Correction, Summary and Ask. Cloud Providers use your key in Keys.">
      {current.models.length ? <div className="grid gap-2">{current.models.map((item) => <div key={identity(item)} className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-line bg-raised p-3.5"><span>{label(item)}</span><div className="flex gap-2"><Button variant="secondary" disabled={busy || !hasKey(item.provider)} onClick={() => { setBusy(true); setReply(null); setAiError(null); void testLanguageModel(item).then((value) => setReply(`${label(item)}: ${value}`), (reason) => setAiError(String(reason))).finally(() => setBusy(false)); }}>Test</Button><Button variant="secondary" disabled={busy} onClick={() => { setRemoving(item); setReplacement(""); }}>Remove</Button></div></div>)}</div> : <p className="m-0 text-sm text-ink-muted">No Language models added.</p>}
      <div className="grid gap-2 rounded-md bg-panel p-3.5"><span className="font-medium">Add Language model</span><label htmlFor="language-provider" className="text-sm">Provider</label><select id="language-provider" className={selectClass} value={provider} onChange={(event) => { setProvider(event.target.value as LanguageModel["provider"]); setAvailable([]); setModel(""); }}>{PROVIDERS.map((item) => <option key={item.kind} value={item.kind}>{PROVIDER_NAMES[item.kind]}</option>)}</select>
        {hasKey(provider) ? <><Button variant="secondary" disabled={busy} onClick={() => { setBusy(true); setAiError(null); void listAiModels(provider).then(setAvailable, (reason) => setAiError(String(reason))).finally(() => setBusy(false)); }}>Load models</Button>{available.length > 0 && <><label htmlFor="available-language-model" className="text-sm">Provider models</label><select id="available-language-model" className={selectClass} value="" onChange={(event) => setModel(event.target.value)}><option value="">Choose a model</option>{available.map((name) => <option key={name} value={name}>{name}</option>)}</select></>}<label htmlFor="language-model-id" className="text-sm">Model ID</label><input id="language-model-id" className={selectClass} value={model} onChange={(event) => setModel(event.target.value)} placeholder="Type a model ID" /><Button disabled={busy || !model.trim()} onClick={() => { const selected = { provider, model: model.trim() }; if (!current.models.some((item) => identity(item) === identity(selected))) void save({ ...current, models: [...current.models, selected] }); setModel(""); }}>Add model</Button></> : <p className="m-0 text-sm text-ink-muted">No key for {PROVIDER_NAMES[provider]}. <button type="button" className="text-accent hover:underline" onClick={onManageKeys}>Manage keys</button></p>}
      </div>
    </FormSection>
    <FormSection title="Use for" hint="Each step has its own default Language model.">{(["correction", "summary", "ask"] as const).map((use) => <div key={use} className="grid gap-1.5"><label htmlFor={`use-${use}`} className="font-medium">{use[0].toUpperCase() + use.slice(1)}</label><select id={`use-${use}`} className={selectClass} disabled={busy} value={current[use] ? identity(current[use]) : ""} onChange={(event) => choose(use, event.target.value)}><option value="">{use === "correction" ? "Off" : "None"}</option>{current.models.map((item) => <option key={identity(item)} value={identity(item)}>{label(item)}</option>)}</select></div>)}</FormSection>
    {reply && <p role="status" className="m-0 text-sm">Test reply: {reply}</p>}
    {aiError && <ErrorState title="Couldn't update Language models" guidance={aiError} />}
    {pendingCorrection && <div role="alertdialog" aria-label="Confirm Correction Provider" className="grid gap-3 rounded-md border border-line-strong bg-raised p-4"><p className="m-0">Transcripts will be sent to {PROVIDER_NAMES[pendingCorrection.provider]} after every transcription.</p><div className="flex gap-2"><Button onClick={() => { const selected = pendingCorrection; const change = pendingChange; setPendingCorrection(null); setPendingChange(null); if (change) void save({ ...change, confirmed_correction_providers: [...current.confirmed_correction_providers, selected.provider] }); }}>Confirm</Button><Button variant="secondary" onClick={() => { setPendingCorrection(null); setPendingChange(null); }}>Cancel</Button></div></div>}
    {removing && <div role="alertdialog" aria-label="Remove Language model" className="grid gap-3 rounded-md border border-line-strong bg-raised p-4"><p className="m-0">Remove {label(removing)}. Choose a replacement for any default that uses it.</p><label htmlFor="replacement-model">Replacement</label><select id="replacement-model" className={selectClass} value={replacement} onChange={(event) => setReplacement(event.target.value)}><option value="">None (Correction Off)</option>{current.models.filter((item) => identity(item) !== identity(removing)).map((item) => <option key={identity(item)} value={identity(item)}>{label(item)}</option>)}</select><div className="flex gap-2"><Button onClick={remove}>Remove model</Button><Button variant="secondary" onClick={() => setRemoving(null)}>Cancel</Button></div></div>}
  </div>;
}
