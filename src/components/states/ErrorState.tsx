import { Button } from "../ui/Button";

export function ErrorState({ title, guidance, onRetry }: { title: string; guidance: string; onRetry?: () => void }) {
  return <div role="alert" className="grid max-w-md gap-3 py-6">
    <h2 className="m-0 font-display text-heading-md">{title}</h2>
    <p className="m-0 text-base text-ink-muted">{guidance}</p>
    {onRetry && <Button className="justify-self-start" onClick={onRetry}>Retry</Button>}
  </div>;
}
