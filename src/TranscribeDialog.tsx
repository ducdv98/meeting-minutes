import { useEffect, useState } from "react";
import { aiStatus, managedStatus, type AiStatus, type ManagedStatus } from "./ai";
import ModelPicker, { type ModelOption } from "./components/ModelPicker";
import { Button } from "./components/ui/Button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./components/ui/Dialog";
import { getSettings, installedModels, MODELS, modelSizeLabel, type Settings } from "./processing";

export default function TranscribeDialog({ again, onClose, onTranscribe, onOpenSettings }: { again: boolean; onClose(): void; onTranscribe(model?: string): void; onOpenSettings(): void }) {
  const [choice, setChoice] = useState<string | null>(null);
  const [confirmingCloud, setConfirmingCloud] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [managed, setManaged] = useState<ManagedStatus | null>(null);
  const [installed, setInstalled] = useState<string[] | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void Promise.allSettled([getSettings(), aiStatus(), managedStatus(), installedModels()]).then(([nextSettings, nextStatus, nextManaged, nextInstalled]) => {
      if (!active) return;
      if (nextSettings.status === "fulfilled") setSettings(nextSettings.value);
      else setError("Cloud model settings could not load. Local Whisper is still available.");
      if (nextStatus.status === "fulfilled") setStatus(nextStatus.value);
      if (nextManaged.status === "fulfilled") setManaged(nextManaged.value);
      if (nextInstalled.status === "fulfilled") setInstalled(nextInstalled.value);
      setLoaded(true);
    });
    return () => { active = false; };
  }, []);
  const managedReady = !!managed?.connected && (managed.enabled ?? !managed.email) && !managed.revoked;
  const options: ModelOption[] = [
    ...MODELS.map((model) => ({ id: `local:${model.name}`, provider: "Local Whisper", model: model.name, label: `Local Whisper · ${model.label}`, group: "local" as const, ready: true, description: installed?.includes(model.name) ? "Downloaded" : `Download in Settings (${modelSizeLabel(model.size_mb)})` })),
    { id: "managed", provider: "Managed transcription", model: "Fixed model", label: "Managed transcription · Fixed model", group: "cloud", ready: managedReady, sendsTo: "Managed transcription", description: managedReady ? "Ready · sends audio to Managed transcription" : managed?.connected ? "Off in Settings" : "Not connected" },
    ...(settings?.speech?.models ?? []).map((model) => { const provider = model.provider === "groq" ? "Groq" : "Gemini"; const ready = !!status?.[`has_${model.provider}_key`]; return { id: `${model.provider}:${model.model}`, provider, model: model.model, label: `${provider} · ${model.model}`, group: "cloud" as const, ready, sendsTo: provider, description: ready ? `Ready · sends audio to ${provider}` : "No key" }; }),
  ];
  const selected = options.find((option) => option.id === choice);
  function begin() {
    if (selected?.group === "cloud" && !confirmingCloud) { setConfirmingCloud(true); return; }
    onTranscribe(choice ?? undefined);
  }
  return <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}><DialogContent aria-label={confirmingCloud && selected ? `Send audio to ${selected.sendsTo}?` : again ? "Transcribe again with…" : "Transcribe…"}>
    <DialogTitle>{confirmingCloud && selected ? `Send audio to ${selected.sendsTo}?` : again ? "Transcribe again with…" : "Transcribe…"}</DialogTitle>
    <DialogDescription>{confirmingCloud && selected ? `This Meeting's audio will be sent to ${selected.sendsTo} for transcription with ${selected.model}.` : again ? "This makes a new Revision. The current Revision stays available if transcription fails." : "Choose a Speech-to-text model for this Meeting."}</DialogDescription>
    {!confirmingCloud && <>
    <div className="mt-5 grid gap-3 text-sm">
      <button type="button" aria-pressed={!choice} className={`rounded-md border px-3 py-2 text-left ${!choice ? "border-accent bg-accent-soft" : "border-line bg-raised hover:bg-hover"}`} onClick={() => setChoice(null)}>Automatic (Transcription order)</button>
      <div className="flex items-center gap-3"><ModelPicker options={options} value={choice} onChange={setChoice} onSetup={() => { onClose(); onOpenSettings(); }} label="Choose Speech-to-text model" disabled={!loaded}>{selected?.label ?? "Choose a model"}</ModelPicker>{selected && <span className="text-xs text-ink-muted">{selected.description}</span>}</div>
      {choice && <p className="m-0 text-xs text-ink-muted">This choice applies to this run only. A failure will not try another Provider.</p>}
      {error && <p role="status" className="m-0 text-sm text-ink-muted">{error}</p>}
    </div>
    </>}
    <div className="mt-6 flex justify-end gap-2"><Button variant="secondary" onClick={confirmingCloud ? () => setConfirmingCloud(false) : onClose}>{confirmingCloud ? "Back" : "Cancel"}</Button><Button disabled={!loaded || !!choice && !selected?.ready} onClick={begin}>{confirmingCloud ? "Send and transcribe" : "Transcribe"}</Button></div>
  </DialogContent></Dialog>;
}
