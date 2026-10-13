import { Tooltip as Primitive } from "radix-ui";
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export const TooltipProvider = Primitive.Provider;
export const Tooltip = Primitive.Root;
export const TooltipTrigger = Primitive.Trigger;
export function TooltipContent({ className, sideOffset = 6, ...props }: ComponentProps<typeof Primitive.Content>) {
  return <Primitive.Portal><Primitive.Content sideOffset={sideOffset} className={cn("z-50 max-w-72 rounded-md border border-line-strong bg-raised px-2.5 py-1.5 text-xs text-ink shadow-pop", className)} {...props} /></Primitive.Portal>;
}
