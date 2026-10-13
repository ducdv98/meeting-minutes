import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";

const badgeVariants = cva("inline-flex h-5 items-center px-2 font-text text-xs", {
  variants: { variant: { default: "rounded-sm bg-panel text-ink-muted", accent: "rounded-sm bg-accent-soft text-accent", beta: "rounded-full border border-line-strong text-ink-muted" } },
  defaultVariants: { variant: "default" },
});
export function Badge({ className, variant, ...props }: HTMLAttributes<HTMLSpanElement> & VariantProps<typeof badgeVariants>) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}
export function BetaBadge(props: Omit<HTMLAttributes<HTMLSpanElement>, "children">) {
  return <Badge variant="beta" {...props}>Beta</Badge>;
}
