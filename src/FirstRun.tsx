import { useState } from "react";
import { Button } from "./components/ui/Button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./components/ui/Dialog";
import { MEETING_LANGUAGES, type Language } from "./languages";
import { MODELS, modelSizeLabel, openLibraryFolder, startModelDownload, updateSettings, type Settings } from "./processing";
import { acknowledgeRecordingNotice } from "./recording";
import { recordingNotice } from "./recording-notice";
import { usePlatformCapabilities } from "./platform-capabilities";
import { useModelDownloads } from "./settings/useModelDownloads";

export function recommendedModel(settings: Settings, discreteGpu: boolean) {
  return MODELS.find((model) => model.name === (discreteGpu || settings.language && settings.language !== "en" ? "large-v3-turbo-q5_0" : "small"))!;
}

export default function FirstRun({ settings, discreteGpu, libraryLocation, onFinish, onUseKey }: {
  settings: Settings;
  discreteGpu: boolean;
  libraryLocation: string;
  onFinish: () => void | Promise<void>;
  onUseKey: () => void | Promise<void>;
}) {
  const capabilities = usePlatformCapabilities();
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [language, setLanguage] = useState<Language | null>(settings.language);
  const [savedModel, setSavedModel] = useState(settings.model);
  const { downloads, error: downloadError, run } = useModelDownloads();
  const model = recommendedModel({ ...settings, language }, discreteGpu);
  const download = downloads[model.name];

  async function chooseLanguage(next: Language | null) {
    setBusy(true); setError(null);
    try {
      const speech = settings.speech ? { ...settings.speech, automatic_whisper: savedModel } : undefined;
      await updateSettings(settings.auto_process, savedModel, settings.vocabulary, next, settings.your_name, speech);
      setLanguage(next);
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  async function acknowledge() {
    setBusy(true); setError(null);
    try { await acknowledgeRecordingNotice(); setStep(1); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  async function downloadNow() {
    setBusy(true); setError(null);
    try {
      const speech = settings.speech ? { ...settings.speech, automatic_whisper: model.name } : undefined;
      await updateSettings(settings.auto_process, model.name, settings.vocabulary, language, settings.your_name, speech);
      setSavedModel(model.name);
      await run(model.name, () => startModelDownload(model.name));
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  async function finish(useKey = false) {
    setBusy(true); setError(null);
    try { await (useKey ? onUseKey() : onFinish()); }
    catch (reason) { setError(String(reason)); setBusy(false); }
  }

  function skip() { if (step === 2) void finish(); else setStep(step + 1); }
  return <Dialog open onOpenChange={(open) => { if (!open) void finish(); }}>
    <DialogContent aria-describedby="first-run-description">
      <p className="m-0 text-xs font-medium uppercase tracking-wide text-ink-muted">Setup · {step + 1} of 3</p>
      <DialogTitle>{step === 0 ? "Before you record" : step === 1 ? "Set up a local model" : "Library location"}</DialogTitle>
      {step === 0 ? <DialogDescription id="first-run-description">{recordingNotice(capabilities)}</DialogDescription>
        : step === 1 ? <div id="first-run-description" className="mt-2 grid gap-4 text-sm text-ink-muted">
          <div className="grid gap-1.5"><label htmlFor="first-run-language" className="font-medium text-ink">Meetings are mostly in</label><select id="first-run-language" className="rounded-sm border border-line-strong bg-raised px-2.5 py-1.5 text-sm text-ink" value={language ?? ""} disabled={busy} onChange={(event) => void chooseLanguage((event.target.value || null) as Language | null)}><option value="">Auto-detect</option>{MEETING_LANGUAGES.map((entry) => <option key={entry.code} value={entry.code}>{entry.label}</option>)}</select></div>
          <p className="m-0">Recommended for this PC and your default Meeting language: <strong className="text-ink">{model.label}</strong> · {modelSizeLabel(model.size_mb)}.</p>
          <p className="m-0">You can record while it downloads. Processing will wait for the model.</p>
          {download?.state === "downloading" && <p role="status" className="m-0 text-ink">Downloading {model.label}: {download.progress}%</p>}
          {download?.state === "downloaded" && <p role="status" className="m-0 text-ink">{model.label} is ready.</p>}
          {download?.state === "failed" && <p role="alert" className="m-0 text-record">{download.error ?? "Download failed"}</p>}
        </div>
        : <div id="first-run-description" className="mt-2 grid gap-3 text-sm text-ink-muted"><p className="m-0">Meetings are saved in your Library at <strong className="text-ink">{libraryLocation}</strong>. You can find this location in Settings → General.</p><Button variant="secondary" className="justify-self-start" disabled={busy} onClick={() => void openLibraryFolder().catch((reason) => setError(String(reason)))}>Open folder</Button></div>}
      {(error || downloadError) && <p role="alert" className="mt-3 text-sm text-record">{error || downloadError}</p>}
      <div className="mt-6 flex flex-wrap justify-end gap-2">
        <Button variant="ghost" disabled={busy} onClick={skip}>Skip</Button>
        {step === 0 ? <Button disabled={busy} onClick={() => void acknowledge()}>I understand</Button>
          : step === 1 ? <><Button variant="secondary" disabled={busy} onClick={() => void finish(true)}>I'll use a cloud key</Button><Button disabled={busy || download?.state === "downloading" || download?.state === "downloaded"} onClick={() => void downloadNow()}>Download now</Button><Button variant="secondary" disabled={busy} onClick={() => setStep(2)}>Continue</Button></>
          : <Button disabled={busy} onClick={() => void finish()}>Done</Button>}
      </div>
    </DialogContent>
  </Dialog>;
}
