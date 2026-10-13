import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export function Kbd({ className, ...props }: ComponentProps<"kbd">) {
  return <kbd className={cn("rounded-[4px] border border-b-2 border-line-strong bg-raised px-1 text-xs text-ink-muted", className)} {...props} />;
}
