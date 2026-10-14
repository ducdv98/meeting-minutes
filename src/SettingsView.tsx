import { useEffect, useRef, useState } from "react";
import { Check } from "lucide-react";
import { getSettings, installedModels, onProcessingStatus, updateSettings, type Settings } from "./processing";
import { aiStatus, type AiStatus } from "./ai";
import { ErrorState } from "./components/states/ErrorState";
import { Loading } from "./components/states/Loading";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/Tabs";
import { GeneralTab } from "./settings/GeneralTab";
import { SpeechToTextTab } from "./settings/SpeechToTextTab";
import { LanguageModelsTab } from "./settings/LanguageModelsTab";
import { KeysTab } from "./settings/KeysTab";
import { PrivacyTab } from "./settings/PrivacyTab";
import "./settings/settings.css";

export type SettingsTabId = "general" | "speech-to-text" | "language-models" | "keys" | "privacy";
let lastSettingsTab: SettingsTabId = "general";

/** Settings with the typed Vocabulary, or null when it matches what is saved. */
function withVocabulary(settings: Settings | null, text: string): Settings | null {
  if (!settings) return null;
  const vocabulary = text.split("\n").map((term) => term.trim()).filter(Boolean);
  return vocabulary.join("\n") === settings.vocabulary.join("\n") ? null : { ...settings, vocabulary };
}

export default function SettingsView({ initialTab }: { initialTab?: SettingsTabId }) {
  const [tab, setTab] = useState<SettingsTabId>(() => initialTab ?? lastSettingsTab);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [load, setLoad] = useState(0);
  const [installed, setInstalled] = useState<string[] | null>(null);
  const [vocabulary, setVocabulary] = useState("");
  const [error, setError] = useState<{ reason: string; settings: Settings } | null>(null);
  const [saved, setSaved] = useState(false);
  const [savedLeaving, setSavedLeaving] = useState(false);
  const saveQueue = useRef(Promise.resolve());
  const saveVersion = useRef(0);
  const [providerStatus, setProviderStatus] = useState<AiStatus | null>(null);
  const [aiError, setAiError] = useState<string | null>(null);
  useEffect(() => { lastSettingsTab = tab; }, [tab]);
  useEffect(() => {
    let active = true;
    setLoadFailed(false);
    getSettings().then((next) => { if (active) { setSettings(next); setVocabulary(next.vocabulary.join("\n")); } }, () => { if (active) setLoadFailed(true); });
    installedModels().then((names) => { if (active) setInstalled(names); }, () => {});
    return () => { active = false; };
  }, [load]);
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void onProcessingStatus(() => { void installedModels().then((names) => { if (active) setInstalled(names); }, () => {}); }).then((stop) => { if (active) unlisten = stop; else stop(); }, () => {});
    return () => { active = false; unlisten?.(); };
  }, []);
  useEffect(() => {
    let active = true;
    aiStatus().then((next) => { if (active) setProviderStatus(next); }, (reason) => { if (active) setAiError(String(reason)); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    if (!saved) return;
    const fade = window.setTimeout(() => setSavedLeaving(true), 2000);
    const hide = window.setTimeout(() => setSaved(false), 2120);
    return () => { window.clearTimeout(fade); window.clearTimeout(hide); };
  }, [saved]);
  /** Shows the change at once and saves it after any save still running, so the last change wins. */
  function save(next: Settings) {
    setSettings(next);
    setSaved(false);
    setSavedLeaving(false);
    setError(null);
    const version = ++saveVersion.current;
    saveQueue.current = saveQueue.current.then(async () => {
      try {
        await persist(next);
        if (version === saveVersion.current) { setSavedLeaving(false); setSaved(true); }
      } catch (reason) {
        if (version === saveVersion.current) setError({ reason: String(reason), settings: next });
      }
    });
  }
  function persist(next: Settings) {
    return next.speech ? updateSettings(next.auto_process, next.model, next.vocabulary, next.language, next.your_name, next.speech) : updateSettings(next.auto_process, next.model, next.vocabulary, next.language, next.your_name);
  }
  function saveVocabulary() {
    const next = withVocabulary(settings, vocabulary);
    if (!next) return;
    setVocabulary(next.vocabulary.join("\n"));
    save(next);
  }
  // Leaving Settings while typing in Vocabulary fires no blur, so save the edit on the way out.
  const unsaved = useRef({ settings, vocabulary });
  unsaved.current = { settings, vocabulary };
  useEffect(() => () => {
    const next = withVocabulary(unsaved.current.settings, unsaved.current.vocabulary);
    if (next) saveQueue.current = saveQueue.current.then(() => next.speech ? updateSettings(next.auto_process, next.model, next.vocabulary, next.language, next.your_name, next.speech) : updateSettings(next.auto_process, next.model, next.vocabulary, next.language, next.your_name)).then(() => {}, () => {});
  }, []);
  return <section aria-label="Settings" className="min-h-0 flex-1 overflow-y-auto">
    <div className="grid max-w-[680px] gap-4 px-7 pb-12">
      <div className="flex h-5 items-center justify-end">
        {saved && <span role="status" data-state={savedLeaving ? "leaving" : "visible"} className="settings-saved inline-flex items-center gap-1 text-xs text-ink-muted"><Check size={14} strokeWidth={1.5} aria-hidden="true" />Saved</span>}
      </div>
      {error && <ErrorState title="Couldn't save Settings" guidance={`${error.reason}. The change shown below isn't saved yet.`} onRetry={() => save(error.settings)} />}
      {loadFailed ? <ErrorState title="Couldn't load Settings" guidance="Try again in a moment." onRetry={() => setLoad((value) => value + 1)} /> : !settings ? <Loading message="Loading Settings…" /> : <Tabs value={tab} onValueChange={(value) => setTab(value as SettingsTabId)} className="min-w-0">
        <TabsList aria-label="Settings tabs" className="flex-wrap px-0"><TabsTrigger value="general">General</TabsTrigger><TabsTrigger value="speech-to-text">Speech-to-text</TabsTrigger><TabsTrigger value="language-models">Language models</TabsTrigger><TabsTrigger value="keys">Keys</TabsTrigger><TabsTrigger value="privacy">Privacy</TabsTrigger></TabsList>
        <TabsContent forceMount hidden={tab !== "general"} value="general" className="settings-tab pt-7"><GeneralTab settings={settings} save={save} /></TabsContent>
        <TabsContent forceMount hidden={tab !== "speech-to-text"} value="speech-to-text" className="settings-tab pt-7"><SpeechToTextTab settings={settings} save={save} installed={installed} vocabulary={vocabulary} setVocabulary={setVocabulary} saveVocabulary={saveVocabulary} providerStatus={providerStatus} onManageKeys={() => setTab("keys")} /></TabsContent>
        <TabsContent forceMount hidden={tab !== "language-models"} value="language-models" className="settings-tab pt-7"><LanguageModelsTab settings={settings} setSettings={setSettings} providerStatus={providerStatus} setProviderStatus={setProviderStatus} aiError={aiError} setAiError={setAiError} onManageKeys={() => setTab("keys")} /></TabsContent>
        <TabsContent forceMount hidden={tab !== "keys"} value="keys" className="settings-tab pt-7"><KeysTab settings={settings} setSettings={setSettings} providerStatus={providerStatus} setProviderStatus={setProviderStatus} /></TabsContent>
        <TabsContent forceMount hidden={tab !== "privacy"} value="privacy" className="settings-tab pt-7">{tab === "privacy" && <PrivacyTab settings={settings} providerStatus={providerStatus} aiError={aiError} />}</TabsContent>
      </Tabs>}
    </div>
  </section>;
}
