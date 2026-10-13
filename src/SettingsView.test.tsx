import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SettingsView from "./SettingsView";
import type { AiSettings } from "./ai";
import { acknowledgeCloudNotice, getSettings, installedModels, updateSettings, type Settings } from "./processing";
import { aiStatus, listAiModels, saveApiKey, testAiProvider, testCloudApiKey, updateAiSettings, type AiStatus } from "./ai";
import { openLink } from "./app-info";

vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), getSettings: vi.fn(), updateSettings: vi.fn(), installedModels: vi.fn(), acknowledgeCloudNotice: vi.fn() }));
vi.mock("./ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("./ai")>()), aiStatus: vi.fn(), listAiModels: vi.fn(), saveApiKey: vi.fn(), deleteApiKey: vi.fn(), updateAiSettings: vi.fn(), testAiProvider: vi.fn(), testCloudApiKey: vi.fn() }));
vi.mock("./app-info", () => ({ openLink: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
beforeEach(() => { vi.mocked(installedModels).mockResolvedValue([]); });

const noAi: AiSettings = { provider: null, ollama_model: null, openai_model: null, anthropic_model: null };
const empty: AiStatus = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };

const base: Settings = { recording_notice_acknowledged: true, cloud_notice_acknowledged: false, auto_process: true, model: "small", vocabulary: ["Nguyễn"], language: null, your_name: null, ai: noAi };

it("opens provider key pages, tests saved keys, and shows the cloud notice once", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(saveApiKey).mockResolvedValueOnce({ ...empty, has_groq_key: true }).mockResolvedValueOnce({ ...empty, has_groq_key: true, has_gemini_key: true });
  vi.mocked(acknowledgeCloudNotice).mockResolvedValue({ ...base, cloud_notice_acknowledged: true });
  vi.mocked(testCloudApiKey).mockResolvedValueOnce().mockRejectedValueOnce("Gemini quota exceeded");
  render(<SettingsView />);
  const cloud = await screen.findByRole("region", { name: "Cloud transcription" });
  fireEvent.click(within(cloud).getByRole("button", { name: "Get a free Groq key" }));
  fireEvent.click(within(cloud).getByRole("button", { name: "Get a free Gemini key" }));
  expect(openLink).toHaveBeenCalledWith("groq");
  expect(openLink).toHaveBeenCalledWith("gemini");
  fireEvent.change(within(cloud).getByLabelText("Groq API key"), { target: { value: "secret" } });
  fireEvent.click(within(cloud).getByRole("button", { name: "Save Groq key" }));
  await waitFor(() => expect(saveApiKey).toHaveBeenCalledWith("groq", "secret"));
  expect(await screen.findByText(/Gemini's free tier may use content for training/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Got it" }));
  await waitFor(() => expect(acknowledgeCloudNotice).toHaveBeenCalledTimes(1));
  fireEvent.change(within(cloud).getByLabelText("Gemini API key"), { target: { value: "gemini-secret" } });
  fireEvent.click(within(cloud).getByRole("button", { name: "Save Gemini key" }));
  await waitFor(() => expect(saveApiKey).toHaveBeenCalledWith("gemini", "gemini-secret"));
  fireEvent.click(within(cloud).getByRole("button", { name: "Test Groq key" }));
  expect(await within(cloud).findByText("Groq key: OK")).toBeTruthy();
  fireEvent.click(within(cloud).getByRole("button", { name: "Test Gemini key" }));
  expect(await within(cloud).findByText(/Gemini quota exceeded/)).toBeTruthy();
  expect(screen.queryByText(/Gemini's free tier may use content for training/)).toBeNull();
  cleanup();
  vi.mocked(getSettings).mockResolvedValue({ ...base, cloud_notice_acknowledged: true });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, has_groq_key: true, has_gemini_key: true });
  render(<SettingsView />);
  await screen.findByRole("region", { name: "Cloud transcription" });
  expect(screen.queryByText(/Gemini's free tier may use content for training/)).toBeNull();
});

it("saves Your name and clears it to the Me default", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(updateSettings).mockImplementation(async (auto_process, model, vocabulary, language, your_name) => ({ ...base, auto_process, model, vocabulary, language, your_name }));
  render(<SettingsView />);
  const field = await screen.findByRole("textbox", { name: "Your name" });
  fireEvent.change(field, { target: { value: "Duc" } });
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(true, "small", ["Nguyễn"], null, "Duc"));
  fireEvent.change(field, { target: { value: "" } });
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(true, "small", ["Nguyễn"], null, null));
});

it("saves each transcription setting as soon as it changes", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(installedModels).mockResolvedValue(["small"]);
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(updateSettings).mockImplementation(async (auto_process, model, vocabulary, language) => ({ ...base, auto_process, model, vocabulary, language }));
  render(<SettingsView />);
  const auto = await screen.findByRole("switch", { name: "Process after I stop" });
  expect(auto.getAttribute("aria-checked")).toBe("true");
  fireEvent.click(auto);
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(false, "small", ["Nguyễn"], null, null));
  fireEvent.click(screen.getByRole("radio", { name: "Large v3 turbo" }));
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(false, "large-v3-turbo-q5_0", ["Nguyễn"], null, null));
  fireEvent.change(screen.getByRole("combobox", { name: "Default language" }), { target: { value: "vi" } });
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(false, "large-v3-turbo-q5_0", ["Nguyễn"], "vi", null));
  const vocabulary = screen.getByRole("textbox", { name: "Vocabulary" });
  fireEvent.change(vocabulary, { target: { value: " Alice \n\nProject X" } });
  expect(updateSettings).toHaveBeenCalledTimes(3);
  fireEvent.blur(vocabulary);
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(false, "large-v3-turbo-q5_0", ["Alice", "Project X"], "vi", null));
  expect((await screen.findByText("Saved")).getAttribute("role")).toBe("status");
  fireEvent.blur(vocabulary);
  expect(updateSettings).toHaveBeenCalledTimes(4);
});

it("runs saves one at a time so a slow earlier save cannot win", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(installedModels).mockResolvedValue([]);
  vi.mocked(getSettings).mockResolvedValue(base);
  let release: () => void = () => {};
  vi.mocked(updateSettings).mockImplementationOnce((auto_process, model, vocabulary, language) => new Promise((resolve) => { release = () => resolve({ ...base, auto_process, model, vocabulary, language }); }))
    .mockImplementation(async (auto_process, model, vocabulary, language) => ({ ...base, auto_process, model, vocabulary, language }));
  render(<SettingsView />);
  fireEvent.click(await screen.findByRole("radio", { name: "Large v3 turbo" }));
  fireEvent.click(screen.getByRole("radio", { name: "Base" }));
  await waitFor(() => expect(updateSettings).toHaveBeenCalledTimes(1));
  release();
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(true, "base", ["Nguyễn"], null, null));
  expect(updateSettings).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("radio", { name: "Base" }).getAttribute("aria-checked")).toBe("true");
});

it("shows which models are on this PC", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(installedModels).mockResolvedValue(["small"]);
  vi.mocked(getSettings).mockResolvedValue(base);
  render(<SettingsView />);
  const small = await screen.findByRole("radio", { name: /^Small/ });
  await waitFor(() => expect(small.getAttribute("aria-describedby")).toBeTruthy());
  const status = (radio: HTMLElement) => radio.getAttribute("aria-describedby")!.split(" ").map((id) => document.getElementById(id)?.textContent).join(" ");
  await waitFor(() => expect(status(small)).toContain("Downloaded"));
  expect(status(small)).toContain("488 MB");
  const turbo = screen.getByRole("radio", { name: "Large v3 turbo" });
  expect(status(turbo)).toContain("574 MB");
  expect(status(turbo)).toContain("Most accurate. Runs best with a GPU.");
  expect(status(turbo)).toContain("Downloads when first used");
  expect(within(screen.getByRole("radiogroup", { name: "Transcription model" })).getAllByRole("radio")).toHaveLength(3);
  expect(screen.queryByRole("radio", { name: "Medium" })).toBeNull();
});

it("saves Vocabulary typed just before leaving Settings", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(updateSettings).mockImplementation(async (auto_process, model, vocabulary, language) => ({ ...base, auto_process, model, vocabulary, language }));
  const { unmount } = render(<SettingsView />);
  fireEvent.change(await screen.findByRole("textbox", { name: "Vocabulary" }), { target: { value: "Nguyễn\nLan" } });
  unmount();
  await waitFor(() => expect(updateSettings).toHaveBeenCalledWith(true, "small", ["Nguyễn", "Lan"], null, null));
});

it("retries a failed save", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(updateSettings).mockRejectedValueOnce("disk full").mockImplementation(async (auto_process, model, vocabulary, language) => ({ ...base, auto_process, model, vocabulary, language }));
  render(<SettingsView />);
  fireEvent.click(await screen.findByRole("radio", { name: "Large v3 turbo" }));
  fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
  await waitFor(() => expect(updateSettings).toHaveBeenCalledTimes(2));
  expect(updateSettings).toHaveBeenLastCalledWith(true, "large-v3-turbo-q5_0", ["Nguyễn"], null, null);
  expect(await screen.findByText("Saved")).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
});

it("explains a failed save and a failed load", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(installedModels).mockResolvedValue([]);
  vi.mocked(getSettings).mockRejectedValueOnce("settings unreadable").mockResolvedValue(base);
  vi.mocked(updateSettings).mockRejectedValue("disk full");
  render(<SettingsView />);
  fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
  fireEvent.click(await screen.findByRole("switch", { name: "Process after I stop" }));
  expect((await screen.findByRole("alert")).textContent).toContain("disk full");
});

it("selects Ollama, lists models, and tests the selected model", async () => {
  vi.mocked(getSettings).mockResolvedValue({ ...base, vocabulary: [] });
  vi.mocked(aiStatus).mockResolvedValue(empty);
  const ollama = { ...empty, provider: "ollama" as const };
  vi.mocked(updateAiSettings).mockResolvedValueOnce(ollama).mockResolvedValueOnce({ ...ollama, model: "llama3.2", configured: true });
  vi.mocked(listAiModels).mockResolvedValue(["llama3.2"]);
  vi.mocked(testAiProvider).mockResolvedValue("ready");
  render(<SettingsView />);
  fireEvent.click(await screen.findByRole("radio", { name: /^Ollama/ }));
  await waitFor(() => expect(updateAiSettings).toHaveBeenCalledWith("ollama", null));
  fireEvent.click(await screen.findByRole("button", { name: "Find Ollama models" }));
  fireEvent.change(await screen.findByRole("combobox", { name: "Model" }), { target: { value: "llama3.2" } });
  await waitFor(() => expect(updateAiSettings).toHaveBeenCalledWith("ollama", "llama3.2"));
  fireEvent.click(await screen.findByRole("button", { name: "Test" }));
  expect((await screen.findByRole("status")).textContent).toContain("Test reply: ready");
});

it("clears a saved key and shows the stored key status", async () => {
  vi.mocked(getSettings).mockResolvedValue({ ...base, vocabulary: [] });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, provider: "openai" });
  vi.mocked(saveApiKey).mockResolvedValue({ ...empty, provider: "openai", has_openai_key: true });
  render(<SettingsView />);
  fireEvent.change(await screen.findByLabelText("API key"), { target: { value: "secret" } });
  fireEvent.click(screen.getByRole("button", { name: "Save key" }));
  await waitFor(() => expect(saveApiKey).toHaveBeenCalledWith("openai", "secret"));
  expect(screen.queryByLabelText("API key")).toBeNull();
  expect(await screen.findByText(/API key saved in Windows Credential Manager/)).toBeTruthy();
});

it("explains how to start Ollama when model discovery fails", async () => {
  vi.mocked(getSettings).mockResolvedValue({ ...base, vocabulary: [] });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, provider: "ollama" });
  vi.mocked(listAiModels).mockRejectedValue("Ollama is not running at localhost:11434");
  render(<SettingsView />);
  fireEvent.click(await screen.findByRole("button", { name: "Find Ollama models" }));
  expect((await screen.findByRole("alert")).textContent).toContain("ollama pull llama3.2");
});
