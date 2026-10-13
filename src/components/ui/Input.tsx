import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export function Input({ className, ...props }: ComponentProps<"input">) {
  return <input className={cn("h-[34px] w-full rounded-sm border border-line-strong bg-raised px-2.5 text-base outline-none placeholder:text-ink-faint focus:border-accent focus:shadow-[0_0_0_3px_var(--accent-soft)] disabled:opacity-50", className)} {...props} />;
}
