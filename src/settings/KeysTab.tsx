import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { deleteApiKey, listAiModels, managedRemoveKey, managedSaveKey, managedStatus, PROVIDER_NAMES, saveApiKey, testCloudApiKey, type AiStatus, type ManagedStatus, type ProviderKind } from "../ai";
import { openLink } from "../app-info";
import { acknowledgeCloudNotice, type Settings } from "../processing";
import { Button } from "../components/ui/Button";
import { Input } from "../components/ui/Input";
import { FormSection } from "../components/FormSection";
import { Details } from "../components/Details";
import { CLOUD_NOTICE_SUMMARY, cloudNoticeText, MANAGED_PRIVACY_STATEMENT, MANAGED_PRIVACY_SUMMARY } from "../privacy-text";
import { usePlatformCapabilities } from "../platform-capabilities";

type CloudProvider = Exclude<ProviderKind, "ollama">;
const providers: { kind: CloudProvider; use: string }[] = [
  { kind: "groq", use: "Speech-to-text" },
  { kind: "gemini", use: "Speech-to-text and Language models" },
  { kind: "openai", use: "Language models" },
  { kind: "anthropic", use: "Language models" },
];

export function KeysTab({ settings, setSettings, providerStatus, setProviderStatus, active = true }: { settings: Settings; setSettings: Dispatch<SetStateAction<Settings | null>>; providerStatus: AiStatus | null; setProviderStatus: Dispatch<SetStateAction<AiStatus | null>>; active?: boolean }) {
  const capabilities = usePlatformCapabilities();
  const [managed, setManaged] = useState<ManagedStatus | null>(null);
  const [managedKey, setManagedKey] = useState("");
  const [managedBusy, setManagedBusy] = useState(false);
  const [managedResult, setManagedResult] = useState<string | null>(null);
  const [managedError, setManagedError] = useState<string | null>(null);
  useEffect(() => {
    if (!active) return;
    let alive = true;
    managedStatus().then((status) => { if (alive) { setManaged(status); setManagedError(null); } }, (reason) => { if (alive) { setManaged(null); setManagedError(String(reason)); } });
    return () => { alive = false; };
  }, [active]);
  async function changeManagedKey(remove: boolean) {
    setManagedBusy(true); setManagedError(null); setManagedResult(null);
    try {
      const next = remove ? await managedRemoveKey() : await managedSaveKey(managedKey.trim());
      setManaged(next); setManagedKey(""); setManagedResult(remove ? "Key removed" : "Key saved");
      window.dispatchEvent(new Event("managed-key-changed"));
    } catch (reason) { setManagedError(String(reason)); }
    finally { setManagedBusy(false); }
  }
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
      <div role="group" aria-label="Managed transcription Provider" className="grid gap-2 rounded-md border border-line bg-raised p-3.5">
        <div className="flex flex-wrap items-center justify-between gap-2"><div className="grid"><span className="font-medium">Managed transcription</span><span className="text-sm text-ink-muted">Speech-to-text · mm-transcription-internal</span></div><a className="text-sm text-accent hover:underline" href="https://meeting-minutes.tech/request-a-key/" target="_blank" rel="noopener noreferrer">Request a key</a></div>
        <label htmlFor="managed-key" className="text-sm">Managed key</label>
        <div className="flex flex-wrap gap-2"><Input id="managed-key" type="password" autoComplete="off" value={managedKey} onChange={(event) => setManagedKey(event.target.value)} /><Button variant="secondary" disabled={managedBusy || !managedKey.trim()} onClick={() => void changeManagedKey(false)}>Save Managed key</Button></div>
        <div className="flex flex-wrap items-center gap-2 text-sm"><span role="status">{managed?.revoked ? "Key invalid or revoked" : managed?.connected ? "Key saved" : "No key"}</span>{managed?.connected && <Button variant="secondary" disabled={managedBusy} onClick={() => void changeManagedKey(true)}>Remove Managed key</Button>}</div>
        {managed?.connected && managed.remaining_minutes != null && <p className="m-0 text-sm">{managed.remaining_minutes.toLocaleString(undefined, { maximumFractionDigits: 1 })} minutes remaining this month.</p>}
        {managed?.revoked && <p role="alert" className="m-0 text-sm text-record">This Managed key is invalid or revoked. Paste a new key to resume Managed transcription.</p>}
        <div className="grid gap-1 text-xs text-ink-muted"><p className="m-0">Saving a key allows Meeting audio to be sent to the cloud. {MANAGED_PRIVACY_SUMMARY}</p><Details><p className="m-0">{MANAGED_PRIVACY_STATEMENT}</p></Details></div>
        {managedResult && <p role="status" className="m-0 text-sm">Managed transcription: {managedResult}</p>}
        {managedError && <p role="alert" className="m-0 text-sm text-record">{managedError}</p>}
      </div>
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
    {noticeVisible && <div role="alert" className="grid gap-2 rounded-md border border-line bg-raised p-3.5 text-sm"><p className="m-0">{CLOUD_NOTICE_SUMMARY}</p><Details>{cloudNoticeText(capabilities)}</Details><Button variant="secondary" onClick={() => void acknowledgeCloudNotice().then((next) => { setSettings(next); setNoticeError(null); }, (reason) => setNoticeError(String(reason)))}>Got it</Button>{noticeError && <p role="status" className="m-0">{noticeError}</p>}</div>}
    <FormSection title="Ollama" hint="On this PC. No key needed."><div className="grid gap-1.5 text-sm"><span className="font-medium">Address</span><span>http://localhost:11434</span><span className="text-ink-muted">This address is fixed in the app.</span></div></FormSection>
  </div>;
}
