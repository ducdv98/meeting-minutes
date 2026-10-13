import { Button } from "./components/ui/Button";
import { EmptyState } from "./components/states/EmptyState";
export default function AiSetupGuidance({ onOpenSettings }: { onOpenSettings(): void }) {
  return <section aria-label="Summary and Ask"><EmptyState title="Summary and Ask" description="Set up Ollama on this PC, or add your own OpenAI or Anthropic API key." action={<Button onClick={onOpenSettings}>Open AI settings</Button>} /></section>;
}
