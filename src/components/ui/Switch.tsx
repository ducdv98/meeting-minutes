import { Switch as Primitive } from "radix-ui";
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export function Switch({ className, ...props }: ComponentProps<typeof Primitive.Root>) {
  return <Primitive.Root className={cn("relative h-5 w-[34px] shrink-0 rounded-full bg-line-strong transition-colors duration-[120ms] ease-functional data-[state=checked]:bg-ink disabled:opacity-50", className)} {...props}>
    <Primitive.Thumb className="block size-3.5 translate-x-[3px] rounded-full bg-white transition-transform duration-[120ms] ease-functional data-[state=checked]:translate-x-[17px] data-[state=checked]:bg-canvas" />
  </Primitive.Root>;
}
