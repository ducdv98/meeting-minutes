import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import SettingsView from "./SettingsView";
import type { AiSettings } from "./ai";
import { getSettings, updateSettings } from "./processing";
import { aiStatus, listAiModels, saveApiKey, testAiProvider, updateAiSettings, type AiStatus } from "./ai";

vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), getSettings: vi.fn(), updateSettings: vi.fn() }));
vi.mock("./ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("./ai")>()), aiStatus: vi.fn(), listAiModels: vi.fn(), saveApiKey: vi.fn(), deleteApiKey: vi.fn(), updateAiSettings: vi.fn(), testAiProvider: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });

const noAi: AiSettings = { provider: null, ollama_model: null, openai_model: null, anthropic_model: null };
const empty: AiStatus = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false };

it("loads and saves transcription settings", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: ["Nguyễn"], ai: noAi });
  vi.mocked(updateSettings).mockResolvedValue({ recording_notice_acknowledged: true, auto_process: false, model: "medium", vocabulary: ["Alice", "Project X"], ai: noAi });
  render(<SettingsView onBack={() => {}} />);
  const auto = await screen.findByRole("checkbox", { name: "Transcribe automatically after recording" });
  expect((auto as HTMLInputElement).checked).toBe(true);
  fireEvent.click(auto);
  fireEvent.change(screen.getByRole("combobox", { name: "Transcription model" }), { target: { value: "medium" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Vocabulary" }), { target: { value: " Alice \n\nProject X" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(updateSettings).toHaveBeenCalledWith(false, "medium", ["Alice", "Project X"]));
});

it("selects Ollama, lists models, and tests the selected model", async () => {
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], ai: noAi });
  vi.mocked(aiStatus).mockResolvedValue(empty);
  const ollama = { ...empty, provider: "ollama" as const };
  vi.mocked(updateAiSettings).mockResolvedValueOnce(ollama).mockResolvedValueOnce({ ...ollama, model: "llama3.2", configured: true });
  vi.mocked(listAiModels).mockResolvedValue(["llama3.2"]);
  vi.mocked(testAiProvider).mockResolvedValue("ready");
  render(<SettingsView onBack={() => {}} />);
  fireEvent.change(await screen.findByRole("combobox", { name: "Provider" }), { target: { value: "ollama" } });
  await waitFor(() => expect(updateAiSettings).toHaveBeenCalledWith("ollama", null));
  fireEvent.click(await screen.findByRole("button", { name: "Find Ollama models" }));
  fireEvent.change(await screen.findByRole("combobox", { name: "Model" }), { target: { value: "llama3.2" } });
  await waitFor(() => expect(updateAiSettings).toHaveBeenCalledWith("ollama", "llama3.2"));
  fireEvent.click(await screen.findByRole("button", { name: "Test" }));
  expect((await screen.findByRole("status")).textContent).toContain("Test reply: ready");
});

it("clears a saved key and shows the stored key status", async () => {
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], ai: noAi });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, provider: "openai" });
  vi.mocked(saveApiKey).mockResolvedValue({ ...empty, provider: "openai", has_openai_key: true });
  render(<SettingsView onBack={() => {}} />);
  fireEvent.change(await screen.findByLabelText("API key"), { target: { value: "secret" } });
  fireEvent.click(screen.getByRole("button", { name: "Save key" }));
  await waitFor(() => expect(saveApiKey).toHaveBeenCalledWith("openai", "secret"));
  expect(screen.queryByLabelText("API key")).toBeNull();
  expect(await screen.findByText(/API key saved in Windows Credential Manager/)).toBeTruthy();
});

it("explains how to start Ollama when model discovery fails", async () => {
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], ai: noAi });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, provider: "ollama" });
  vi.mocked(listAiModels).mockRejectedValue("Ollama is not running at localhost:11434");
  render(<SettingsView onBack={() => {}} />);
  fireEvent.click(await screen.findByRole("button", { name: "Find Ollama models" }));
  expect((await screen.findByRole("alert")).textContent).toContain("ollama pull llama3.2");
});
