import { DropdownMenu as Primitive } from "radix-ui";
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export const DropdownMenu = Primitive.Root;
export const DropdownMenuTrigger = Primitive.Trigger;
export function DropdownMenuContent({ className, sideOffset = 4, ...props }: ComponentProps<typeof Primitive.Content>) {
  return <Primitive.Portal><Primitive.Content sideOffset={sideOffset} className={cn("z-50 min-w-40 rounded-md border border-line-strong bg-raised p-1 shadow-pop", className)} {...props} /></Primitive.Portal>;
}
export function DropdownMenuItem({ className, ...props }: ComponentProps<typeof Primitive.Item>) {
  return <Primitive.Item className={cn("flex min-h-8 cursor-pointer items-center gap-2 rounded-sm px-2 text-[13px] outline-none data-[highlighted]:bg-hover data-[disabled]:opacity-50", className)} {...props} />;
}
export const DropdownMenuSeparator = Primitive.Separator;
