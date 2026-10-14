import { useState, type Dispatch, type SetStateAction } from "react";
import { Check } from "lucide-react";
import { listAiModels, PROVIDER_NAMES, PROVIDERS, testAiProvider, updateAiSettings, type AiStatus, type ProviderKind } from "../ai";
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

export function LanguageModelsTab({ providerStatus, setProviderStatus, aiError, setAiError, onManageKeys }: { providerStatus: AiStatus | null; setProviderStatus: Dispatch<SetStateAction<AiStatus | null>>; aiError: string | null; setAiError: Dispatch<SetStateAction<string | null>>; onManageKeys: () => void }) {
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
