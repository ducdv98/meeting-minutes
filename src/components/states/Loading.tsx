import { useEffect, useState } from "react";
import { Skeleton } from "../ui/Skeleton";

/** Placeholder rows only, for a status that already shows its own text. */
export function LoadingRows({ rows = 3 }: { rows?: number }) {
  return <div className="grid max-w-md gap-3 py-6">
    {Array.from({ length: rows }, (_, index) => <Skeleton key={index} className={`h-5 ${index % 2 ? "w-3/4" : "w-full"}`} />)}
  </div>;
}

export function Loading({ message = "Loading…", rows = 3 }: { message?: string; rows?: number }) {
  const [showText, setShowText] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setShowText(true), 1000);
    return () => window.clearTimeout(timer);
  }, []);
  return <div role="status" aria-label={message} className="grid max-w-md gap-3 py-6">
    {Array.from({ length: rows }, (_, index) => <Skeleton key={index} className={`h-5 ${index % 2 ? "w-3/4" : "w-full"}`} />)}
    {showText && <p className="m-0 text-sm text-ink-muted">{message}</p>}
  </div>;
}
