import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export function Kbd({ className, ...props }: ComponentProps<"kbd">) {
  return <kbd className={cn("rounded-[4px] border border-b-2 border-line-strong bg-raised px-1 text-xs text-ink-muted", className)} {...props} />;
}
/** A shortcut such as "Ctrl+Alt+Shift+R", one key cap per key. */
export function KeyCombo({ keys, className }: { keys: string; className?: string }) {
  return <span className={cn("inline-flex gap-1", className)}>{keys.split("+").map((key) => <Kbd key={key}>{key}</Kbd>)}</span>;
}
