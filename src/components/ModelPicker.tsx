import { useId, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import { PROVIDER_NAMES, type AiStatus, type LanguageModel, type LanguageModelSettings } from "../ai";
import { Button } from "./ui/Button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "./ui/DropdownMenu";
import { platformWording } from "../platform-capabilities";

export interface ModelOption { id: string; provider: string; model: string; label: string; group: "local" | "cloud"; ready: boolean; sendsTo?: string; description?: string }
export const modelId = (model: LanguageModel) => `${model.provider}:${model.model}`;
export function languageModelOptions(settings: LanguageModelSettings | undefined, status: AiStatus | null, fallback?: LanguageModel | null): ModelOption[] {
  const models = settings?.models ?? (fallback ? [fallback] : []);
  return models.map((item) => { const provider = PROVIDER_NAMES[item.provider]; return { id: modelId(item), provider, model: item.model, label: `${provider} · ${item.model}`, group: item.provider === "ollama" ? "local" as const : "cloud" as const, ready: item.provider === "ollama" || !!status?.[`has_${item.provider}_key` as keyof AiStatus], sendsTo: item.provider === "ollama" ? undefined : provider }; });
}

export default function ModelPicker({ options, value, onChange, onSetup, label, children, disabled = false }: { options: ModelOption[]; value: string | null; onChange(id: string): void; onSetup(): void; label: string; children?: ReactNode; disabled?: boolean }) {
  const descriptionId = useId();
  return <DropdownMenu><DropdownMenuTrigger asChild><Button variant="secondary" disabled={disabled} aria-label={label} className="min-w-0 gap-1.5">{children ?? "Choose model"}<ChevronDown aria-hidden="true" /></Button></DropdownMenuTrigger>
    <DropdownMenuContent align="start" aria-label="Choose model" className="min-w-64 max-w-[min(90vw,360px)]">
      {(["local", "cloud"] as const).map((group) => { const items = options.filter((option) => option.group === group); return items.length ? <div key={group}><p className="m-0 px-2 pb-1 pt-2 text-xs font-medium text-ink-muted">{group === "local" ? platformWording("On this PC") : "Cloud"}</p>{items.map((option) => <DropdownMenuItem key={option.id} disabled={!option.ready} onSelect={() => onChange(option.id)} aria-label={`${option.label}${option.ready ? "" : ", set up in Settings"}`} aria-describedby={option.description ? `${descriptionId}-${option.id}` : undefined} className="flex-col items-start gap-0.5"><span className="flex w-full items-center justify-between gap-3"><span className="truncate">{option.label}</span>{value === option.id && <span className="text-xs text-accent">Selected</span>}</span>{option.description ? <span id={`${descriptionId}-${option.id}`} className="text-xs text-ink-muted">{option.description}</span> : <>{option.group === "cloud" && <span className="text-xs text-ink-muted">sends to {option.sendsTo ?? option.provider}</span>}{!option.ready && <span className="text-xs text-ink-muted">Set up in Settings →</span>}</>}</DropdownMenuItem>)}</div> : null; })}
      {options.some((option) => !option.ready) && <DropdownMenuItem onSelect={onSetup}>Set up in Settings →</DropdownMenuItem>}
    </DropdownMenuContent>
  </DropdownMenu>;
}
