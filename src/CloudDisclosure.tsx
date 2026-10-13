import { PROVIDER_NAMES, type Disclosure } from "./ai";
import { Button } from "./components/ui/Button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./components/ui/Dialog";

export default function CloudDisclosure({ disclosure, onSend, onCancel }: { disclosure: Disclosure; onSend(): void; onCancel(): void }) {
  const label = PROVIDER_NAMES[disclosure.provider];
  return <Dialog open onOpenChange={(open) => { if (!open) onCancel(); }}><DialogContent aria-label={`Send to ${label}?`} className="max-h-[90vh] overflow-y-auto">
    <DialogTitle>Send to {label}?</DialogTitle>
    <DialogDescription>Review the exact Transcript text and instructions that will be sent for this Meeting.</DialogDescription>
    <dl className="mt-5 grid grid-cols-[80px_1fr] gap-x-3 gap-y-2 text-sm"><dt className="text-ink-muted">Provider</dt><dd className="m-0">{label}</dd><dt className="text-ink-muted">Model</dt><dd className="m-0 break-all">{disclosure.model}</dd><dt className="text-ink-muted">Endpoint</dt><dd className="m-0 break-all">{disclosure.endpoint}</dd><dt className="text-ink-muted">Size</dt><dd className="m-0">{disclosure.characters} characters</dd></dl>
    <div className="mt-4 max-h-72 space-y-3 overflow-y-auto rounded-md border border-line bg-panel p-3 text-sm"><div><h3 className="mb-1 text-xs font-medium text-ink-muted">System text</h3><pre className="m-0 whitespace-pre-wrap break-words font-text">{disclosure.system}</pre></div>
      {disclosure.messages.map((message, index) => <div key={index}><h3 className="mb-1 text-xs font-medium capitalize text-ink-muted">{message.role}</h3><pre className="m-0 whitespace-pre-wrap break-words font-text">{message.content}</pre></div>)}</div>
    <p className="mt-4 text-sm text-ink-muted">Your consent is remembered for this Meeting; each send still needs your click.</p>
    <div className="mt-5 flex justify-end gap-2"><Button variant="secondary" onClick={onCancel}>Cancel</Button><Button onClick={onSend}>Send</Button></div>
  </DialogContent></Dialog>;
}
