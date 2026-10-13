import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export function Textarea({ className, ...props }: ComponentProps<"textarea">) {
  return <textarea className={cn("min-h-20 w-full resize-y rounded-sm border border-line-strong bg-raised px-2.5 py-2 text-sm placeholder:text-ink-faint focus:border-accent focus:shadow-[0_0_0_3px_var(--accent-soft)] disabled:opacity-50", className)} {...props} />;
}
