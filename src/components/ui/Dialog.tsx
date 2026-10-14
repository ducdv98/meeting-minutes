import { Dialog as Primitive } from "radix-ui";
import type { ComponentProps } from "react";
import { X } from "lucide-react";
import { cn } from "../../lib/cn";

export const Dialog = Primitive.Root;
export const DialogTrigger = Primitive.Trigger;
export const DialogClose = Primitive.Close;
export function DialogContent({ className, children, ...props }: ComponentProps<typeof Primitive.Content>) {
  return <Primitive.Portal><Primitive.Overlay className="motion-dialog-overlay fixed inset-0 z-40 bg-ink/40" />
    <Primitive.Content className={cn("motion-dialog-content fixed left-1/2 top-1/2 z-50 w-[min(92vw,480px)] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-line-strong bg-raised p-6 shadow-dialog", className)} {...props}>
      {children}<Primitive.Close aria-label="Close" className="absolute right-4 top-4 text-ink-muted hover:text-ink"><X size={16} strokeWidth={1.5} /></Primitive.Close>
    </Primitive.Content></Primitive.Portal>;
}
export function DialogTitle({ className, ...props }: ComponentProps<typeof Primitive.Title>) {
  return <Primitive.Title className={cn("font-display text-heading-md", className)} {...props} />;
}
export function DialogDescription({ className, ...props }: ComponentProps<typeof Primitive.Description>) {
  return <Primitive.Description className={cn("mt-2 text-sm text-ink-muted", className)} {...props} />;
}
