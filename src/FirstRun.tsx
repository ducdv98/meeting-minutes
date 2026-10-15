import { useEffect, useRef, useState } from "react";
import { Button } from "./components/ui/Button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./components/ui/Dialog";
import { MODELS, modelDownloadStatus, modelSizeLabel, onModelDownloadStatus, startModelDownload, updateSettings, type ModelDownloadStatus, type Settings } from "./processing";
import { acknowledgeRecordingNotice } from "./recording";
import { RECORDING_NOTICE } from "./recording-notice";

export function recommendedModel(settings: Settings, discreteGpu: boolean) {
  return MODELS.find((model) => model.name === (discreteGpu || settings.language && settings.language !== "en" ? "large-v3-turbo-q5_0" : "small"))!;
}

export default function FirstRun({ settings, discreteGpu, libraryLocation = "Documents\\Meeting Minutes", onFinish, onUseKey }: {
  settings: Settings;
  discreteGpu: boolean;
  libraryLocation?: string;
  onFinish: () => void | Promise<void>;
  onUseKey: () => void | Promise<void>;
}) {
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [downloads, setDownloads] = useState<Record<string, ModelDownloadStatus>>({});
  const downloadEvents = useRef<Record<string, number>>({});
  const model = recommendedModel(settings, discreteGpu);
  const download = downloads[model.name];

  useEffect(() => {
    let active = true;
    const seen = new Set<string>();
    let unlisten: (() => void) | undefined;
    void onModelDownloadStatus((status) => {
      if (!active) return;
      seen.add(status.model_id);
      downloadEvents.current[status.model_id] = (downloadEvents.current[status.model_id] ?? 0) + 1;
      setDownloads((current) => ({ ...current, [status.model_id]: status }));
    }).then((stop) => {
      if (!active) { stop(); return; }
      unlisten = stop;
      return modelDownloadStatus().then((statuses) => {
        if (active) setDownloads((current) => ({ ...current, ...Object.fromEntries(statuses.filter((status) => !seen.has(status.model_id)).map((status) => [status.model_id, status])) }));
      });
    }).catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; unlisten?.(); };
  }, []);

  async function acknowledge() {
    setBusy(true); setError(null);
    try { await acknowledgeRecordingNotice(); setStep(1); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  async function downloadNow() {
    setBusy(true); setError(null);
    const version = downloadEvents.current[model.name] ?? 0;
    try {
      const speech = settings.speech ? { ...settings.speech, automatic_whisper: model.name } : undefined;
      await updateSettings(settings.auto_process, model.name, settings.vocabulary, settings.language, settings.your_name, speech);
      const statuses = await startModelDownload(model.name);
      const status = statuses.find((entry) => entry.model_id === model.name);
      if (status && (downloadEvents.current[model.name] ?? 0) === version) setDownloads((current) => ({ ...current, [model.name]: status }));
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
      {step === 0 ? <DialogDescription id="first-run-description">{RECORDING_NOTICE}</DialogDescription>
        : step === 1 ? <div id="first-run-description" className="mt-2 grid gap-4 text-sm text-ink-muted">
          <p className="m-0">Recommended for this PC and your default Meeting language: <strong className="text-ink">{model.label}</strong> · {modelSizeLabel(model.size_mb)}.</p>
          <p className="m-0">You can record while it downloads. Processing will wait for the model.</p>
          {download?.state === "downloading" && <p role="status" className="m-0 text-ink">Downloading {model.label}: {download.progress}%</p>}
          {download?.state === "downloaded" && <p role="status" className="m-0 text-ink">{model.label} is ready.</p>}
          {download?.state === "failed" && <p role="alert" className="m-0 text-record">{download.error ?? "Download failed"}</p>}
        </div>
        : <DialogDescription id="first-run-description">Meetings are saved in your Library at <strong className="text-ink">{libraryLocation}</strong>. You can find this location in Settings → General.</DialogDescription>}
      {error && <p role="alert" className="mt-3 text-sm text-record">{error}</p>}
      <div className="mt-6 flex flex-wrap justify-end gap-2">
        <Button variant="ghost" disabled={busy} onClick={skip}>Skip</Button>
        {step === 0 ? <Button disabled={busy} onClick={() => void acknowledge()}>I understand</Button>
          : step === 1 ? <><Button variant="secondary" disabled={busy} onClick={() => void finish(true)}>I'll use my own key</Button><Button disabled={busy || download?.state === "downloading" || download?.state === "downloaded"} onClick={() => void downloadNow()}>Download now</Button><Button variant="secondary" disabled={busy} onClick={() => setStep(2)}>Continue</Button></>
          : <Button disabled={busy} onClick={() => void finish()}>Done</Button>}
      </div>
    </DialogContent>
  </Dialog>;
}
