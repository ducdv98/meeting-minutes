import type { ReactNode } from "react";

export function FormSection({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return <section aria-label={title} className="grid gap-3">
    <h3 className="m-0 font-display text-heading-sm">{title}</h3>
    {hint && <p className="-mt-1.5 mb-0 text-sm text-ink-muted">{hint}</p>}
    {children}
  </section>;
}
