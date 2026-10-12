import { useEffect, useState } from "react";
import { getSettings, MODELS, updateSettings, type Settings } from "./processing";
import { aiStatus, deleteApiKey, listAiModels, PROVIDER_NAMES, PROVIDERS, saveApiKey, testAiProvider, updateAiSettings, type AiStatus, type ProviderKind } from "./ai";

export default function SettingsView({ onBack }: { onBack(): void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [vocabulary, setVocabulary] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [providerStatus, setProviderStatus] = useState<AiStatus | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [key, setKey] = useState("");
  const [aiError, setAiError] = useState<string | null>(null);
  const [reply, setReply] = useState<string | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  useEffect(() => {
    let active = true;
    getSettings().then((next) => { if (active) { setSettings(next); setVocabulary(next.vocabulary.join("\n")); } }, (reason) => { if (active) setError(String(reason)); });
    aiStatus().then((next) => { if (active) setProviderStatus(next); }, (reason) => { if (active) setAiError(String(reason)); });
    return () => { active = false; };
  }, []);
  async function save() {
    if (!settings || saving) return;
    setSaving(true); setError(null); setSaved(false);
    try {
      const next = await updateSettings(settings.auto_process, settings.model, vocabulary.split("\n").map((term) => term.trim()).filter(Boolean));
      setSettings(next); setVocabulary(next.vocabulary.join("\n")); setSaved(true);
    } catch (reason) { setError(String(reason)); }
    finally { setSaving(false); }
  }
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
  const hasKey = provider === "openai" ? providerStatus?.has_openai_key : provider === "anthropic" ? providerStatus?.has_anthropic_key : false;
  const selectedLabel = provider ? PROVIDER_NAMES[provider] : "";
  return <section className="settings-view">
    <button type="button" onClick={onBack}>Back to Library</button>
    <h1>Settings</h1>
    {error && <p role="alert">{error}</p>}
    {settings && <>
      <label><input type="checkbox" checked={settings.auto_process} onChange={(event) => setSettings({ ...settings, auto_process: event.target.checked })} /> Transcribe automatically after recording</label>
      <label htmlFor="transcription-model">Transcription model</label>
      <select id="transcription-model" value={settings.model} onChange={(event) => setSettings({ ...settings, model: event.target.value })}>
        {MODELS.map((model) => <option key={model.name} value={model.name}>{model.name} ({model.size_mb} MB{model.name === "small" ? ", default" : ""})</option>)}
      </select>
      <label htmlFor="vocabulary">Vocabulary</label>
      <textarea id="vocabulary" value={vocabulary} onChange={(event) => setVocabulary(event.target.value)} rows={6} />
      <p>One name or term per line.</p>
      <button type="button" disabled={saving} onClick={() => void save()}>Save</button>
      {saved && <p role="status">Settings saved</p>}
    </>}
    <section aria-label="AI provider">
      <h2>AI provider</h2>
      <p>Summary and Ask use a text model. Ollama runs on this PC; OpenAI and Anthropic send text to their servers only when you click.</p>
      <label htmlFor="ai-provider">Provider</label>
      <select id="ai-provider" value={provider ?? ""} disabled={aiBusy || !providerStatus} onChange={(event) => {
        const next = event.target.value ? event.target.value as ProviderKind : null;
        setModels([]); void runAi(() => updateAiSettings(next, null));
      }}><option value="">None</option>{PROVIDERS.map((item) => <option key={item.kind} value={item.kind}>{item.label}</option>)}</select>
      {isCloud && (hasKey ? <p>API key saved in Windows Credential Manager <button type="button" disabled={aiBusy} onClick={() => void runAi(() => deleteApiKey(provider))}>Remove key</button></p> : <div>
        <label htmlFor="ai-key">API key</label><input id="ai-key" type="password" value={key} onChange={(event) => setKey(event.target.value)} />
        <button type="button" disabled={aiBusy} onClick={() => { const entered = key; setKey(""); void runAi(() => saveApiKey(provider, entered)); }}>Save key</button>
      </div>)}
      {provider && <button type="button" disabled={aiBusy} onClick={() => void runAi(() => listAiModels(provider))}>{provider === "ollama" ? "Find Ollama models" : "Load models"}</button>}
      {provider && (models.length > 0 || providerStatus?.model) && <><label htmlFor="ai-model">Model</label>
        <select id="ai-model" value={providerStatus?.model ?? ""} disabled={aiBusy} onChange={(event) => void runAi(() => updateAiSettings(provider, event.target.value))}>
          <option value="">Choose a model</option>
          {Array.from(new Set([...(providerStatus?.model ? [providerStatus.model] : []), ...models])).map((model) => <option key={model} value={model}>{model}</option>)}
        </select></>}
      {providerStatus?.configured && <button type="button" disabled={aiBusy} onClick={() => void runAi(testAiProvider)}>{isCloud ? `Send test message to ${selectedLabel}` : "Test"}</button>}
      {providerStatus?.configured && <p>The test sends: “You are a connection test.” and “Reply with the single word: ready”. No Meeting content is sent.</p>}
      {reply !== null && <p role="status">Test reply: {reply}</p>}
      {aiError && <p role="alert">{aiError}{provider === "ollama" && <> Install Ollama from ollama.com, start it, and pull a model (for example <code>ollama pull llama3.2</code>).</>}</p>}
    </section>
  </section>;
}
