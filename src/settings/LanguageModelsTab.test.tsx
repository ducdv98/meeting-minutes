import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { LanguageModelsTab } from "./LanguageModelsTab";
import { aiStatus, listAiModels, testLanguageModel, updateLanguageModels, type AiStatus, type LanguageModelSettings } from "../ai";
import type { Settings } from "../processing";

vi.mock("../ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("../ai")>()), aiStatus: vi.fn(), listAiModels: vi.fn(), testLanguageModel: vi.fn(), updateLanguageModels: vi.fn() }));
const ollama = { provider: "ollama" as const, model: "llama3.2" };
const anthropic = { provider: "anthropic" as const, model: "claude-test" };
const gemini = { provider: "gemini" as const, model: "gemini-test" };
const models: LanguageModelSettings = { models: [ollama, anthropic, gemini], correction: null, summary: anthropic, ask: ollama, confirmed_correction_providers: [], legacy_correction_pending: false };
const status: AiStatus = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: true, has_groq_key: false, has_gemini_key: true };
const initial: Settings = { recording_notice_acknowledged: true, cloud_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null }, language_models: models };

function Harness() {
  const [settings, setSettings] = useState<Settings | null>(initial);
  const [providerStatus, setProviderStatus] = useState<AiStatus | null>(status);
  const [aiError, setAiError] = useState<string | null>(null);
  return <LanguageModelsTab settings={settings ?? undefined} setSettings={setSettings} providerStatus={providerStatus} setProviderStatus={setProviderStatus} aiError={aiError} setAiError={setAiError} onManageKeys={() => {}} />;
}

beforeEach(() => {
  vi.mocked(updateLanguageModels).mockImplementation(async (next) => ({ ...initial, language_models: next }));
  vi.mocked(aiStatus).mockResolvedValue(status);
  vi.mocked(listAiModels).mockResolvedValue(["llama3.2"]);
  vi.mocked(testLanguageModel).mockResolvedValue("ready");
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("keeps separate Summary and Ask defaults and tests a saved model", async () => {
  render(<Harness />);
  expect((screen.getByLabelText("Summary") as HTMLSelectElement).value).toBe(JSON.stringify(anthropic));
  expect((screen.getByLabelText("Ask") as HTMLSelectElement).value).toBe(JSON.stringify(ollama));
  fireEvent.click(screen.getAllByRole("button", { name: "Test" })[0]);
  expect((await screen.findByRole("status")).textContent).toContain("ready");
  expect(testLanguageModel).toHaveBeenCalledWith(ollama);
});

it("asks once before cloud Correction and lets Cancel keep Off", async () => {
  render(<Harness />);
  fireEvent.change(screen.getByLabelText("Correction"), { target: { value: JSON.stringify(anthropic) } });
  expect(screen.getByRole("alertdialog").textContent).toContain("Transcripts will be sent to Anthropic after every transcription.");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(updateLanguageModels).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Correction"), { target: { value: JSON.stringify(anthropic) } });
  fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() => expect(updateLanguageModels).toHaveBeenCalledWith(expect.objectContaining({ correction: anthropic, confirmed_correction_providers: ["anthropic"] })));
  fireEvent.change(screen.getByLabelText("Correction"), { target: { value: JSON.stringify(gemini) } });
  expect(screen.getByRole("alertdialog").textContent).toContain("Gemini");
});

it("selects Ollama and Off without confirmation", async () => {
  render(<Harness />);
  fireEvent.change(screen.getByLabelText("Correction"), { target: { value: JSON.stringify(ollama) } });
  await waitFor(() => expect(updateLanguageModels).toHaveBeenCalledWith(expect.objectContaining({ correction: ollama })));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  fireEvent.change(screen.getByLabelText("Correction"), { target: { value: "" } });
  await waitFor(() => expect(updateLanguageModels).toHaveBeenCalledWith(expect.objectContaining({ correction: null })));
  expect(screen.queryByRole("alertdialog")).toBeNull();
});

it("asks for a replacement when removing a default model", async () => {
  render(<Harness />);
  fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
  expect(screen.getByRole("alertdialog").textContent).toContain("Choose a replacement");
  fireEvent.change(screen.getByLabelText("Replacement"), { target: { value: JSON.stringify(gemini) } });
  fireEvent.click(screen.getByRole("button", { name: "Remove model" }));
  await waitFor(() => expect(updateLanguageModels).toHaveBeenCalledWith(expect.objectContaining({ ask: gemini, models: [anthropic, gemini] })));
});

it("adds a typed model ID and tests it after saving", async () => {
  render(<Harness />);
  fireEvent.change(screen.getByLabelText("Model ID"), { target: { value: "llama-new" } });
  fireEvent.click(screen.getByRole("button", { name: "Add model" }));
  await waitFor(() => expect(updateLanguageModels).toHaveBeenCalledWith(expect.objectContaining({ models: [...models.models, { provider: "ollama", model: "llama-new" }] })));
  const rowLabel = screen.getAllByText("Ollama · llama-new").find((element) => element.tagName === "SPAN");
  expect(rowLabel).toBeTruthy();
  fireEvent.click(rowLabel!.parentElement!.querySelector("button")!);
  await waitFor(() => expect(testLanguageModel).toHaveBeenCalledWith({ provider: "ollama", model: "llama-new" }));
});
