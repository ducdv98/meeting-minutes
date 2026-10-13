import { Slot } from "radix-ui";
import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-sm text-[13px] font-medium transition-colors duration-[120ms] ease-functional disabled:opacity-50 disabled:pointer-events-none [&_svg]:size-4 [&_svg]:stroke-[1.5]",
  {
    variants: {
      variant: {
        primary: "bg-ink text-canvas hover:bg-[color-mix(in_srgb,var(--ink)_86%,var(--canvas))]",
        secondary: "border border-line-strong bg-raised text-ink hover:bg-hover",
        ghost: "text-ink-muted hover:bg-hover hover:text-ink",
        accent: "bg-accent text-accent-ink hover:bg-[color-mix(in_srgb,var(--accent)_88%,var(--ink))]",
        record: "bg-record text-white hover:bg-[color-mix(in_srgb,var(--record)_88%,black)]",
      },
      size: {
        default: "h-8 px-3",
        icon: "size-8 p-0",
        lg: "h-10 px-4 text-sm",
      },
    },
    defaultVariants: { variant: "primary", size: "default" },
  },
);

export interface ButtonProps extends ComponentProps<"button">, VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

export function Button({ asChild = false, className, variant, size, type = "button", ...props }: ButtonProps) {
  const Component = asChild ? Slot.Root : "button";
  return <Component type={asChild ? undefined : type} className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}
