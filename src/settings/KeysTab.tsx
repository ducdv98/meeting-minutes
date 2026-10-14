import { useState, type Dispatch, type SetStateAction } from "react";
import { deleteApiKey, listAiModels, PROVIDER_NAMES, saveApiKey, testCloudApiKey, type AiStatus, type ProviderKind } from "../ai";
import { openLink } from "../app-info";
import { acknowledgeCloudNotice, type Settings } from "../processing";
import { Button } from "../components/ui/Button";
import { Input } from "../components/ui/Input";
import { FormSection } from "../components/FormSection";
import { Details } from "../components/Details";
import { CLOUD_NOTICE_SUMMARY, CLOUD_NOTICE_TEXT } from "../privacy-text";

type CloudProvider = Exclude<ProviderKind, "ollama">;
const providers: { kind: CloudProvider; use: string }[] = [
  { kind: "groq", use: "Speech-to-text" },
  { kind: "gemini", use: "Speech-to-text and Language models" },
  { kind: "openai", use: "Language models" },
  { kind: "anthropic", use: "Language models" },
];

export function KeysTab({ settings, setSettings, providerStatus, setProviderStatus }: { settings: Settings; setSettings: Dispatch<SetStateAction<Settings | null>>; providerStatus: AiStatus | null; setProviderStatus: Dispatch<SetStateAction<AiStatus | null>> }) {
  const [keys, setKeys] = useState<Record<CloudProvider, string>>({ groq: "", gemini: "", openai: "", anthropic: "" });
  const [busy, setBusy] = useState<CloudProvider | null>(null);
  const [results, setResults] = useState<Partial<Record<CloudProvider, string>>>({});
  const [noticeError, setNoticeError] = useState<string | null>(null);
  const hasKey = (kind: CloudProvider) => providerStatus?.[`has_${kind}_key`];
  const noticeVisible = !settings.cloud_notice_acknowledged && providers.some(({ kind }) => hasKey(kind));
  async function run(kind: CloudProvider, action: () => Promise<AiStatus | void | string[]>, success: string) {
    setBusy(kind);
    setResults((current) => ({ ...current, [kind]: undefined }));
    try {
      const result = await action();
      if (result && !Array.isArray(result)) setProviderStatus(result);
      setResults((current) => ({ ...current, [kind]: success }));
      return true;
    } catch (reason) { setResults((current) => ({ ...current, [kind]: String(reason) })); return false; }
    finally { setBusy(null); }
  }
  return <div className="grid gap-8">
    <FormSection title="Provider keys" hint="Keys stay in Windows Credential Manager.">
      {providers.map(({ kind, use }) => {
        const label = PROVIDER_NAMES[kind];
        return <div key={kind} className="grid gap-2 rounded-md border border-line bg-raised p-3.5">
          <div className="flex flex-wrap items-center justify-between gap-2"><div className="grid"><span className="font-medium">{label}</span><span className="text-sm text-ink-muted">{use}</span></div>{(kind === "groq" || kind === "gemini") && <Button variant="ghost" onClick={() => void openLink(kind).catch((reason) => setResults((current) => ({ ...current, [kind]: String(reason) })))}>Get a free {label} key</Button>}</div>
          <label htmlFor={`${kind}-key`} className="text-sm">{label} API key</label>
          <div className="flex flex-wrap gap-2"><Input id={`${kind}-key`} type="password" value={keys[kind]} onChange={(event) => setKeys((current) => ({ ...current, [kind]: event.target.value }))} /><Button variant="secondary" disabled={!!busy || !keys[kind].trim()} onClick={() => { const entered = keys[kind]; void run(kind, () => saveApiKey(kind, entered), "Key saved").then((saved) => { if (saved) setKeys((current) => ({ ...current, [kind]: "" })); }); }}>Save {label} key</Button></div>
          <div className="flex flex-wrap items-center gap-2 text-sm"><span>{hasKey(kind) ? "Key saved" : "No key"}</span><Button variant="secondary" disabled={!!busy || !hasKey(kind)} onClick={() => void run(kind, () => kind === "groq" || kind === "gemini" ? testCloudApiKey(kind) : listAiModels(kind), "OK")}>Test {label} key</Button>{hasKey(kind) && <Button variant="secondary" disabled={!!busy} onClick={() => void run(kind, () => deleteApiKey(kind), "Key removed")}>Remove {label} key</Button>}</div>
          {results[kind] && <p role="status" className="m-0 text-sm">{label} key: {results[kind]}</p>}
        </div>;
      })}
    </FormSection>
    {noticeVisible && <div role="alert" className="grid gap-2 rounded-md border border-line bg-raised p-3.5 text-sm"><p className="m-0">{CLOUD_NOTICE_SUMMARY}</p><Details>{CLOUD_NOTICE_TEXT}</Details><Button variant="secondary" onClick={() => void acknowledgeCloudNotice().then((next) => { setSettings(next); setNoticeError(null); }, (reason) => setNoticeError(String(reason)))}>Got it</Button>{noticeError && <p role="status" className="m-0">{noticeError}</p>}</div>}
    <FormSection title="Ollama" hint="On this PC. No key needed."><div className="grid gap-1.5 text-sm"><span className="font-medium">Address</span><span>http://localhost:11434</span><span className="text-ink-muted">This address is fixed in the app.</span></div></FormSection>
  </div>;
}
