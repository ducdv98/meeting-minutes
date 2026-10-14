import { type Language, type Settings } from "../processing";
import { Input } from "../components/ui/Input";
import { Switch } from "../components/ui/Switch";
import { FormSection } from "../components/FormSection";

const selectClass = "h-[34px] w-full rounded-sm border border-line-strong bg-raised px-2.5 text-base outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--accent-soft)] disabled:opacity-50";

export function GeneralTab({ settings, save }: { settings: Settings; save: (next: Settings) => void }) {
  return <FormSection title="Meetings">
    <div className="grid gap-1.5"><label htmlFor="your-name" className="text-sm font-medium">Your name</label><Input id="your-name" value={settings.your_name ?? ""} placeholder="Me" onChange={(event) => save({ ...settings, your_name: event.target.value || null })} /><span className="text-xs text-ink-muted">Default Participant for Me in Meetings without a name override.</span></div>
    <div className="grid grid-cols-2 gap-3">
      <div className="grid gap-1.5">
        <label htmlFor="default-language" className="text-sm font-medium">Default language</label>
        <select id="default-language" className={selectClass} value={settings.language ?? ""} onChange={(event) => save({ ...settings, language: (event.target.value || null) as Language | null })}>
          <option value="">Detect automatically</option>
          <option value="en">English</option>
          <option value="vi">Tiếng Việt</option>
        </select>
      </div>
      <div className="grid gap-1.5">
        <span id="library-folder" className="text-sm font-medium">Library folder</span>
        <span aria-labelledby="library-folder" className="flex h-[34px] items-center rounded-sm border border-line-strong bg-raised px-2.5 text-base text-ink-muted">Documents\Meeting Minutes</span>
      </div>
    </div>
    <div className="flex items-center justify-between gap-4 rounded-md border border-line bg-raised px-3.5 py-3">
      <div className="grid gap-0.5">
        <span id="auto-process" className="text-base font-medium">Process after I stop</span>
        <span id="auto-process-note" className="text-xs text-ink-muted">Transcribe as soon as a Recording ends.</span>
      </div>
      <Switch aria-labelledby="auto-process" aria-describedby="auto-process-note" checked={settings.auto_process} onCheckedChange={(auto_process) => save({ ...settings, auto_process })} />
    </div>
  </FormSection>;
}
