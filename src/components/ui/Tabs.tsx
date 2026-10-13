import { Tabs as Primitive } from "radix-ui";
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export const Tabs = Primitive.Root;
export function TabsList({ className, ...props }: ComponentProps<typeof Primitive.List>) {
  return <Primitive.List className={cn("flex gap-1 border-b border-line px-4 pt-2.5", className)} {...props} />;
}
export function TabsTrigger({ className, ...props }: ComponentProps<typeof Primitive.Trigger>) {
  return <Primitive.Trigger className={cn("-mb-px border-b-2 border-transparent px-2.5 pb-2.5 pt-2 text-[13px] font-medium text-ink-muted data-[state=active]:border-ink data-[state=active]:text-ink", className)} {...props} />;
}
export const TabsContent = Primitive.Content;
