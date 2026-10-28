import { useEffect, useState } from "react";
import { dismissRecordPrompt, getRecordPrompt, showRecordingNotice, silenceRecordPromptApp, type PromptApp } from "./record-prompt";
import { recordingStatus, startRecording } from "./recording";
import { usePlatformCapabilities } from "./platform-capabilities";

export default function RecordPromptWindow() {
  const { recordPrompt } = usePlatformCapabilities();
  const [app, setApp] = useState<PromptApp | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!recordPrompt) return;
    let active = true;
    getRecordPrompt().then(
      (prompt) => { if (active) setApp(prompt); },
      (reason) => { if (active) setError(String(reason)); },
    );
    return () => { active = false; };
  }, [recordPrompt]);

  useEffect(() => {
    if (!app) return;
    const timeout = window.setTimeout(() => { void dismissRecordPrompt(); }, 20_000);
    return () => window.clearTimeout(timeout);
  }, [app]);

  async function record() {
    setBusy(true);
    setError(null);
    try {
      const status = await recordingStatus();
      if (status.phase === "idle") {
        if (status.notice_acknowledged) await startRecording();
        else await showRecordingNotice();
      }
      await dismissRecordPrompt();
    } catch (reason) {
      setError(String(reason));
      setBusy(false);
    }
  }

  async function neverForThisApp() {
    setBusy(true);
    setError(null);
    try {
      await silenceRecordPromptApp();
      await dismissRecordPrompt();
    } catch (reason) {
      setError(String(reason));
      setBusy(false);
    }
  }

  if (!recordPrompt) return null;
  if (!app) return error ? <p role="alert">{error}</p> : null;
  return <section aria-label="Record prompt" className="box-border flex h-screen w-screen flex-col justify-between rounded-xl border border-white/25 bg-[#172234] px-5 py-4 text-white shadow-xl">
    <div>
      <p className="m-0 text-sm font-semibold">Record this call?</p>
      <p className="mt-2 text-sm text-white/75">{app.display_name} started using your Microphone.</p>
    </div>
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
    <div className="flex justify-end gap-2">
      <button type="button" disabled={busy} className="rounded-md px-3 py-2 text-sm text-white/75 hover:bg-white/15 disabled:opacity-50" onClick={() => void neverForThisApp()}>Never for this app</button>
      <button type="button" disabled={busy} className="rounded-md px-3 py-2 text-sm hover:bg-white/15 disabled:opacity-50" onClick={() => void dismissRecordPrompt()}>Not now</button>
      <button type="button" disabled={busy} className="rounded-md bg-red-600 px-4 py-2 text-sm font-semibold hover:bg-red-500 disabled:opacity-50" onClick={() => void record()}>Record</button>
    </div>
  </section>;
}
