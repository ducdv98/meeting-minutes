import { cn } from "../../lib/cn";

/** Presentational meter for a measured Source level in the range 0..1. */
export function LevelMeter({ level, label, className }: { level: number; label: string; className?: string }) {
  const shown = Math.max(0, Math.min(1, Number.isFinite(level) ? level : 0));
  return <span role="meter" aria-label={`${label} level`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(shown * 100)} className={cn("inline-flex h-4 items-end gap-0.5", className)}>
    {Array.from({ length: 10 }, (_, index) => <span key={index} aria-hidden="true" className={cn("w-[3px] rounded-sm bg-ink-faint", shown > index / 10 && "bg-ink")} style={{ height: `${shown > index / 10 ? 4 + index * 1.2 : 3}px` }} />)}
  </span>;
}
