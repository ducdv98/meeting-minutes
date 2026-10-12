export default function AiSetupGuidance({ onOpenSettings }: { onOpenSettings(): void }) {
  return <section aria-label="Summary and Ask">
    <h2>Summary and Ask</h2>
    <p>Summary and Ask need an AI provider. Set up Ollama on this PC, or add your own OpenAI or Anthropic API key.</p>
    <button type="button" onClick={onOpenSettings}>Open AI settings</button>
  </section>;
}
