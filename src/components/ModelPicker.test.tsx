import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import ModelPicker, { languageModelOptions, type ModelOption } from "./ModelPicker";

afterEach(cleanup);
it("groups ready models and keeps an unready cloud model disabled", async () => {
  const options: ModelOption[] = [
    { id: "local", provider: "Ollama", model: "llama", label: "Ollama · llama", group: "local", ready: true },
    { id: "cloud", provider: "OpenAI", model: "gpt", label: "OpenAI · gpt", group: "cloud", ready: false, sendsTo: "OpenAI" },
  ];
  const onChange = vi.fn();
  const onSetup = vi.fn();
  render(<ModelPicker options={options} value="local" onChange={onChange} onSetup={onSetup} label="Choose model" />);
  fireEvent.pointerDown(screen.getByRole("button", { name: "Choose model" }), { button: 0, ctrlKey: false });
  expect(await screen.findByText("On this PC")).toBeTruthy();
  expect(screen.getByText("Cloud")).toBeTruthy();
  expect(screen.getByText("sends to OpenAI")).toBeTruthy();
  expect(screen.getByRole("menuitem", { name: "OpenAI · gpt, set up in Settings" }).hasAttribute("data-disabled")).toBe(true);
  fireEvent.click(screen.getByRole("menuitem", { name: "Set up in Settings →" }));
  expect(onSetup).toHaveBeenCalledOnce();
  expect(onChange).not.toHaveBeenCalled();
});
it("marks cloud models ready only when their Provider key exists", () => {
  const models = { models: [{ provider: "ollama" as const, model: "llama" }, { provider: "openai" as const, model: "gpt" }], correction: null, summary: null, ask: null, confirmed_correction_providers: [], legacy_correction_pending: false };
  const options = languageModelOptions(models, { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false });
  expect(options.map((option) => option.ready)).toEqual([true, false]);
});
