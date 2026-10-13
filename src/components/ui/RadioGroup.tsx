import { RadioGroup as Primitive } from "radix-ui";
import { useId, type ComponentProps, type ReactNode } from "react";
import { cn } from "../../lib/cn";

export function RadioGroup({ className, ...props }: ComponentProps<typeof Primitive.Root>) {
  return <Primitive.Root className={cn("overflow-hidden rounded-md border border-line bg-raised", className)} {...props} />;
}

export function RadioGroupItem({ className, children, description, sideNote, value, "aria-describedby": describedBy, ...props }: Omit<ComponentProps<typeof Primitive.Item>, "children"> & { children: ReactNode; description?: ReactNode; sideNote?: ReactNode }) {
  const id = useId();
  const descriptionIds = [describedBy, description && `${id}-description`, sideNote && `${id}-side-note`].filter(Boolean).join(" ") || undefined;
  return <Primitive.Item value={value} aria-labelledby={`${id}-label`} aria-describedby={descriptionIds} className={cn("group grid w-full grid-cols-[18px_1fr_auto] items-center gap-3 border-b border-line px-3.5 py-3 text-left last:border-b-0 hover:bg-hover data-[state=checked]:bg-accent-soft", className)} {...props}>
    <span className="grid size-4 place-items-center rounded-full border-[1.5px] border-line-strong group-data-[state=checked]:border-accent" aria-hidden="true"><Primitive.Indicator className="size-2 rounded-full bg-accent" /></span>
    <span className="grid gap-0.5"><span id={`${id}-label`} className="text-base font-medium">{children}</span>{description && <span id={`${id}-description`} className="text-xs text-ink-muted">{description}</span>}</span>
    {sideNote && <span id={`${id}-side-note`} className="text-xs text-ink-muted">{sideNote}</span>}
  </Primitive.Item>;
}
