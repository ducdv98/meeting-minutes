import type { ReactNode } from "react";
import { Mark } from "../Brand";

export function EmptyState({ title, description, action }: { title: string; description: string; action?: ReactNode }) {
  return <div className="grid max-w-[380px] justify-items-center gap-3 py-10 text-center">
    <Mark faint className="mb-1 size-[72px]" />
    <h2 className="m-0 font-display text-heading-md">{title}</h2>
    <p className="m-0 text-base text-ink-muted">{description}</p>
    {action && <div className="mt-1 flex flex-wrap justify-center gap-2">{action}</div>}
  </div>;
}
