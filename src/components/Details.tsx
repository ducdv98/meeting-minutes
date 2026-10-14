import { useId, useState, type ReactNode } from "react";

export function Details({ children, label = "Details", defaultOpen = false }: { children: ReactNode; label?: string; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  const panelId = useId();
  return <div className="details-disclosure">
    <button type="button" aria-expanded={open} aria-controls={panelId} className="text-sm font-medium text-accent hover:underline" onClick={() => setOpen((current) => !current)}>{label}</button>
    <div id={panelId} className="details-panel" data-expanded={open} aria-hidden={!open} inert={!open}>
      <div className="details-panel-inner"><div className="pt-2 text-xs text-ink-muted">{children}</div></div>
    </div>
  </div>;
}
