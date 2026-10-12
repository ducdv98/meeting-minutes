import { useEffect, useState } from "react";
import { getSettings, MODELS, updateSettings, type Settings } from "./processing";

export default function SettingsView({ onBack }: { onBack(): void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [vocabulary, setVocabulary] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    let active = true;
    getSettings().then((next) => { if (active) { setSettings(next); setVocabulary(next.vocabulary.join("\n")); } }, (reason) => { if (active) setError(String(reason)); });
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
  </section>;
}
