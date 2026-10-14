import type { ReactNode } from "react";

interface Props {
  title: ReactNode;
  context: ReactNode;
  actions?: ReactNode;
  leading?: ReactNode;
}

export function PageHeader({ title, context, actions, leading }: Props) {
  return <header className="flex min-h-[88px] shrink-0 items-start gap-4 px-7 py-5">
    {leading}
    <div className="grid min-w-0 flex-1 gap-1.5">
      {typeof title === "string" ? <h1 className="m-0 truncate font-display text-heading-md">{title}</h1> : title}
      <div className="min-w-0 text-xs text-ink-muted">{context}</div>
    </div>
    {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
  </header>;
}
