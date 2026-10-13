import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export function Skeleton({ className, ...props }: ComponentProps<"div">) {
  return <div aria-hidden="true" className={cn("rounded-sm bg-page-shadow", className)} {...props} />;
}
