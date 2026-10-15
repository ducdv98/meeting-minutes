import { useEffect, useState } from "react";
import { managedStatus, PROVIDER_NAMES, type AiStatus, type ManagedStatus } from "../ai";
import { Details } from "../components/Details";
import { CLOUD_NOTICE_TEXT, PRIVACY_DESTINATIONS, PRIVACY_INTRO } from "../privacy-text";
import type { Settings } from "../processing";

type Destination = keyof typeof PRIVACY_DESTINATIONS;
const cloudProviders = ["groq", "gemini", "openai", "anthropic"] as const;

export function PrivacyTab({ settings, providerStatus, aiError = null }: { settings: Settings; providerStatus: AiStatus | null; aiError?: string | null }) {
  const [managed, setManaged] = useState<ManagedStatus | null>(null);
  const [managedError, setManagedError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    let current = true;
    setManaged(null); setManagedError(null);
    managedStatus().then((next) => { if (current) setManaged(next); }, (reason) => { if (current) setManagedError(String(reason)); });
    return () => { current = false; };
  }, [refresh]);
  const correction = settings.language_models?.correction ?? null;
  const hasKey = (kind: typeof cloudProviders[number]) => providerStatus?.[`has_${kind}_key`];
  const automaticSpeech = (kind: "groq" | "gemini") => {
    const selected = settings.speech?.[kind === "groq" ? "automatic_groq" : "automatic_gemini"];
    return !!selected && !!settings.speech?.models.some((item) => item.provider === kind && item.model === selected);
  };
  const usedLanguageModel = (kind: typeof cloudProviders[number]) => {
    const language = settings.language_models;
    return !!language && ([language.correction, language.summary, language.ask]).some((item) => item?.provider === kind && language.models.some((model) => model.provider === item.provider && model.model === item.model));
  };
  const providerUsed = (kind: typeof cloudProviders[number]) => ((kind === "groq" || kind === "gemini") && automaticSpeech(kind)) || usedLanguageModel(kind);
  const unknownStatus = aiError ? "Status unavailable" : "Checking…";
  const correctionStatus = !correction ? "Off" : correction.provider === "ollama" ? "Off · On this PC" : !providerStatus ? unknownStatus : !settings.language_models?.models.some((item) => item.provider === correction.provider && item.model === correction.model) ? "Off · Model not added" : providerStatus[`has_${correction.provider}_key`] ? `On · ${PROVIDER_NAMES[correction.provider]} · ${correction.model}` : `Off · No ${PROVIDER_NAMES[correction.provider]} key`;
  const rows: { id: Destination; name: string; status: string }[] = [
    { id: "managed", name: "Managed transcription", status: managedError ? "Status unavailable" : !managed ? "Checking…" : managed.connected && !managed.email ? "Off · Previous Managed connection" : managed.connected && managed.enabled ? "On · Connected" : managed.connected ? "Off · Signed in" : "Off · Not connected" },
    ...cloudProviders.map((kind) => ({ id: kind, name: PROVIDER_NAMES[kind], status: !providerStatus ? unknownStatus : !hasKey(kind) ? "Off · No key" : providerUsed(kind) ? "On · Key saved" : "Off · Key saved, not used" })),
    { id: "correction", name: "Correction model", status: correctionStatus },
  ];
  return <div className="grid gap-6">
    <p className="m-0 text-sm text-ink-muted">{PRIVACY_INTRO}</p>
    <div className="divide-y divide-line">
      {rows.map(({ id, name, status }) => { const text = PRIVACY_DESTINATIONS[id]; return <section key={id} aria-label={name} className="grid gap-1.5 py-4 first:pt-0">
        <div className="flex flex-wrap items-center justify-between gap-2"><h3 className="m-0 font-display text-heading-sm">{name}</h3><span className="text-sm font-medium text-ink-muted">{status}</span></div>
        <p className="m-0 text-sm"><span className="text-ink-muted">What: </span>{text.sent}</p>
        <p className="m-0 text-sm"><span className="text-ink-muted">When: </span>{text.when}</p>
        <Details><p className="m-0">{text.details}</p></Details>
        {id === "managed" && managedError && <p role="alert" className="m-0 text-sm text-record">Couldn't check Managed transcription: {managedError}. <button type="button" className="text-accent hover:underline" onClick={() => setRefresh((value) => value + 1)}>Retry</button></p>}
      </section>; })}
    </div>
    <p className="m-0 text-sm text-ink-muted">{CLOUD_NOTICE_TEXT}</p>
  </div>;
}
