import { type Language, type Settings } from "../processing";
import { MEETING_LANGUAGES } from "../languages";
import { Input } from "../components/ui/Input";
import { Switch } from "../components/ui/Switch";
import { FormSection } from "../components/FormSection";
import { pauseHotkey, recordHotkey } from "../recording";
import { platformWording, type PlatformCapabilities } from "../platform-capabilities";

const selectClass = "h-[34px] w-full rounded-sm border border-line-strong bg-raised px-2.5 text-base outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--accent-soft)] disabled:opacity-50";

function pressedHotkey(event: React.KeyboardEvent<HTMLInputElement>): string | null {
  const physical = /^Key([A-Z])$/.exec(event.code)?.[1] ?? /^Digit([0-9])$/.exec(event.code)?.[1];
  const key = physical ?? (event.key.length === 1 && /^[a-z0-9]$/i.test(event.key) ? event.key.toUpperCase()
    : /^F(?:[1-9]|1[0-2])$/.test(event.key) ? event.key : null);
  if (!key || (!event.ctrlKey && !event.altKey)) return null;
  return [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", key].filter(Boolean).join("+");
}

export function GeneralTab({ settings, save, libraryLocation, capabilities, changeSnapshotHotkey, setRecordPromptEnabled, removeSilencedRecordPromptApp, hotkeyError }: { settings: Settings; save: (next: Settings) => void; libraryLocation: string | null; capabilities: PlatformCapabilities; changeSnapshotHotkey: (snapshotHotkey: string, drawSnapshotHotkey: string) => void; setRecordPromptEnabled: (enabled: boolean) => void; removeSilencedRecordPromptApp: (id: string) => void; hotkeyError: string | null }) {
  const promptEnabled = settings.record_prompt_enabled ?? true;
  const silencedApps = settings.silenced_record_prompt_apps ?? [];
  const snapshotHotkey = settings.snapshot_hotkey ?? "Ctrl+Alt+S";
  const drawSnapshotHotkey = settings.draw_snapshot_hotkey ?? "Ctrl+Alt+Shift+S";
  return <><FormSection title="Meetings">
    <div className="grid gap-1.5"><label htmlFor="your-name" className="text-sm font-medium">Your name</label><Input id="your-name" value={settings.your_name ?? ""} placeholder="Me" onChange={(event) => save({ ...settings, your_name: event.target.value || null })} /><span className="text-xs text-ink-muted">Default Participant for Me in Meetings without a name override.</span></div>
    <div className="grid grid-cols-2 gap-3">
      <div className="grid gap-1.5">
        <label htmlFor="default-language" className="text-sm font-medium">Default language</label>
        <select id="default-language" className={selectClass} value={settings.language ?? ""} onChange={(event) => save({ ...settings, language: (event.target.value || null) as Language | null })}>
          <option value="">Detect automatically</option>
          {MEETING_LANGUAGES.map((language) => <option key={language.code} value={language.code}>{language.label}</option>)}
        </select>
      </div>
      <div className="grid gap-1.5">
        <span id="library-folder" className="text-sm font-medium">Library folder</span>
        <span aria-labelledby="library-folder" className="flex min-h-[34px] items-center break-all rounded-sm border border-line-strong bg-raised px-2.5 text-base text-ink-muted">{libraryLocation ?? "Loading…"}</span>
      </div>
    </div>
    <div className="flex items-center justify-between gap-4 rounded-md border border-line bg-raised px-3.5 py-3">
      <div className="grid gap-0.5">
        <span id="auto-process" className="text-base font-medium">Process after I stop</span>
        <span id="auto-process-note" className="text-xs text-ink-muted">Transcribe as soon as a Recording ends.</span>
      </div>
      <Switch aria-labelledby="auto-process" aria-describedby="auto-process-note" checked={settings.auto_process} onCheckedChange={(auto_process) => save({ ...settings, auto_process })} />
    </div>
  </FormSection>
    {capabilities.recordPrompt && <FormSection title="Recording">
      <div className="flex items-center justify-between gap-4 rounded-md border border-line bg-raised px-3.5 py-3">
        <div className="grid gap-0.5">
          <span id="record-prompt" className="text-base font-medium">Record prompt</span>
          <span id="record-prompt-note" className="text-xs text-ink-muted">{platformWording("Offer to record when another app starts using your Microphone. Checked on this PC only.", capabilities)}</span>
        </div>
        <Switch aria-labelledby="record-prompt" aria-describedby="record-prompt-note" checked={promptEnabled} onCheckedChange={setRecordPromptEnabled} />
      </div>
      {silencedApps.length > 0 && <div className="grid gap-1.5">
        <span className="text-sm font-medium">Apps you silenced</span>
        <ul className="m-0 grid list-none gap-1 p-0">
          {silencedApps.map((silenced) => <li key={silenced.id} className="flex items-center justify-between gap-3 rounded-md border border-line bg-raised px-3 py-2 text-sm"><span>{silenced.display_name}</span><button type="button" className="text-sm text-accent hover:underline" aria-label={`Remove ${silenced.display_name}`} onClick={() => removeSilencedRecordPromptApp(silenced.id)}>Remove</button></li>)}
        </ul>
      </div>}
    </FormSection>}
    <FormSection title="Hotkeys">
      {capabilities.snapshots && <p className="m-0 text-xs text-ink-muted">Press a new combination in a Snapshot field. Changes apply to the next Recording.</p>}
      <div className="grid gap-1 text-sm text-ink-muted"><span>Record: {recordHotkey}</span><span>Pause or resume: {pauseHotkey}</span></div>
      {capabilities.snapshots && <>
        <div className="grid gap-1.5"><label htmlFor="snapshot-hotkey" className="text-sm font-medium">Take Snapshot</label><Input id="snapshot-hotkey" readOnly value={snapshotHotkey} onKeyDown={(event) => { const key = pressedHotkey(event); if (key) { event.preventDefault(); changeSnapshotHotkey(key, drawSnapshotHotkey); } }} /></div>
        <div className="grid gap-1.5"><label htmlFor="draw-snapshot-hotkey" className="text-sm font-medium">Draw new Capture region</label><Input id="draw-snapshot-hotkey" readOnly value={drawSnapshotHotkey} onKeyDown={(event) => { const key = pressedHotkey(event); if (key) { event.preventDefault(); changeSnapshotHotkey(snapshotHotkey, key); } }} /></div>
        {hotkeyError && <p role="alert" className="m-0 text-sm text-record">{hotkeyError}</p>}
      </>}
    </FormSection>
  </>;
}
