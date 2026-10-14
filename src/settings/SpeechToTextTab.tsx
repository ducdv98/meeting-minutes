import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { MODELS, modelSizeLabel, type Settings } from "../processing";
import { managedConnect, managedDisconnect, managedStatus, PROVIDER_NAMES, type AiStatus, type ManagedStatus } from "../ai";
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
  const [managed, setManaged] = useState<ManagedStatus | null>(null);
  const [inviteCode, setInviteCode] = useState("");
  const [managedBusy, setManagedBusy] = useState(false);
  const [managedError, setManagedError] = useState<string | null>(null);
  const [inviteOpen, setInviteOpen] = useState(false);
  useEffect(() => {
    let active = true;
    managedStatus().then((next) => { if (active) setManaged(next); }, (reason) => { if (active) setManagedError(String(reason)); });
    return () => { active = false; };
  }, []);
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
    <FormSection title="Transcription model" hint="Runs on this PC when no Groq or Gemini key is saved, or their limits are reached. Larger models are more accurate and slower.">
      <RadioGroup aria-label="Transcription model" value={settings.model} onValueChange={(model) => save({ ...settings, model })}>
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
