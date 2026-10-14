import { useEffect, useRef, useState } from "react";
import { Check, Copy } from "lucide-react";
import { appInfo, openLink, type AppInfo, type OutsideLink } from "./app-info";
import { getDiagnostics } from "./diagnostics";
import { pauseHotkey, recordHotkey } from "./recording";
import { Wordmark } from "./components/Brand";
import { Badge } from "./components/ui/Badge";
import { Button } from "./components/ui/Button";
import { KeyCombo } from "./components/ui/Kbd";
import { Skeleton } from "./components/ui/Skeleton";
import { ErrorState } from "./components/states/ErrorState";
import { Loading } from "./components/states/Loading";
import { FormSection } from "./components/FormSection";

const SHORTCUTS = [
  { action: "Start or stop recording, from any app", keys: recordHotkey },
  { action: "Pause or resume, from any app", keys: pauseHotkey },
  { action: "Search the Library", keys: "Ctrl+K" },
];

export default function HelpView({ onOpenGuide }: { onOpenGuide?: () => void }) {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [infoFailed, setInfoFailed] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [linkError, setLinkError] = useState<{ reason: string; link: OutsideLink } | null>(null);
  const [previewLoad, setPreviewLoad] = useState(0);
  const copiedTimer = useRef<number | undefined>(undefined);
  useEffect(() => {
    let active = true;
    appInfo().then((next) => { if (active) setInfo(next); }, () => { if (active) setInfoFailed(true); });
    return () => { active = false; window.clearTimeout(copiedTimer.current); };
  }, []);
  useEffect(() => {
    let active = true;
    setPreviewError(null);
    getDiagnostics().then((text) => { if (active) setPreview(text); }, (reason) => { if (active) setPreviewError(String(reason)); });
    return () => { active = false; };
  }, [previewLoad]);
  async function copyDiagnostics() {
    setCopyError(null);
    setCopied(false);
    try {
      const text = await getDiagnostics();
      setPreview(text);
      setPreviewError(null);
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.clearTimeout(copiedTimer.current);
      copiedTimer.current = window.setTimeout(() => setCopied(false), 1600);
    } catch (reason) { setCopyError(String(reason)); }
  }
  async function open(link: OutsideLink) {
    setLinkError(null);
    try { await openLink(link); }
    catch (reason) { setLinkError({ reason: String(reason), link }); }
  }
  const linkClass = "w-fit rounded-sm text-left text-accent underline-offset-2 hover:underline";
  return <section aria-label="Help" className="min-h-0 flex-1 overflow-y-auto">
    <div className="grid max-w-[680px] gap-8 px-7 pb-12 pt-7">
      <section aria-label="About" className="grid gap-5">
        <div className="flex items-center gap-3"><Wordmark className="h-10 w-[210px]" />{info && <Badge variant="beta">{info.channel}</Badge>}</div>
        <dl className="m-0 grid grid-cols-[180px_1fr] gap-x-4 gap-y-2 text-sm tabular-nums">
          <dt className="text-ink-muted">Version</dt><dd className="m-0">{info ? info.version : infoFailed ? "Unknown" : <Skeleton className="h-4 w-16" />}</dd>
          <dt className="text-ink-muted">Made by</dt><dd className="m-0">Duc Dao</dd>
          <dt className="text-ink-muted">Support</dt><dd className="m-0"><button type="button" className={linkClass} onClick={() => void open("support")}>admin@meeting-minutes.tech</button></dd>
          <dt className="text-ink-muted">Privacy</dt><dd className="m-0"><button type="button" className={linkClass} onClick={() => void open("privacy")}>meeting-minutes.tech/privacy</button></dd>
        </dl>
        {linkError && <ErrorState title="Couldn't open the link" guidance={`${linkError.reason}. Write to admin@meeting-minutes.tech or visit meeting-minutes.tech/privacy in your browser.`} onRetry={() => void open(linkError.link)} />}
      </section>
      <FormSection title="Keyboard shortcuts">
        <div className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-2.5 rounded-md border border-line bg-raised p-3.5 text-sm">
          {SHORTCUTS.map((shortcut) => <div key={shortcut.action} className="contents"><span>{shortcut.action}</span><KeyCombo keys={shortcut.keys} /></div>)}
        </div>
      </FormSection>
      <FormSection title="Report a problem" hint="Copy diagnostics and paste them into an email or GitHub issue. They never include your Transcripts, Summaries or keys.">
        {preview !== null ? <pre aria-label="Diagnostics preview" className="m-0 max-h-[170px] overflow-auto whitespace-pre-wrap rounded-md border border-line bg-panel p-3 font-mono text-xs text-ink-muted tabular-nums">{preview}</pre> : previewError ? <ErrorState title="Couldn't read diagnostics" guidance={`${previewError}. Try again, or describe the problem in your message without them.`} onRetry={() => setPreviewLoad((value) => value + 1)} /> : <Loading message="Reading diagnostics…" />}
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => void copyDiagnostics()}>{copied ? <><Check aria-hidden="true" />Copied</> : <><Copy aria-hidden="true" />Copy diagnostics</>}</Button>
          {onOpenGuide && <Button variant="secondary" onClick={onOpenGuide}>Open the setup guide</Button>}
        </div>
        {copied && <p role="status" className="m-0 text-sm text-ink-muted">Diagnostics copied. Paste them into your message.</p>}
        {copyError && <ErrorState title="Couldn't copy diagnostics" guidance={`${copyError}. Select the text above and press Ctrl+C instead.`} onRetry={() => void copyDiagnostics()} />}
      </FormSection>
    </div>
  </section>;
}
