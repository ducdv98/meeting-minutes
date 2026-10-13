import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export interface CitationProps extends ComponentProps<"button"> {
  time: string;
  segmentText?: string;
  withBrackets?: boolean;
}
export function Citation({ time, segmentText, withBrackets = false, className, title, ...props }: CitationProps) {
  return <button type="button" title={title ?? segmentText} aria-label={segmentText ? `Play from ${time}: ${segmentText}` : `Play from ${time}`} className={cn("mx-px inline-flex h-5 items-center rounded-sm bg-accent-soft px-1.5 align-[1px] font-text text-xs font-medium tabular-nums text-accent transition-shadow duration-[120ms] ease-functional hover:shadow-[0_0_0_1px_var(--accent)] disabled:opacity-50", className)} {...props}>{withBrackets ? `[${time}]` : time}</button>;
}
