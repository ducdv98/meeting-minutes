import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SettingsView, { type SettingsTabId } from "./SettingsView";
import type { AiSettings } from "./ai";
import { acknowledgeCloudNotice, getSettings, installedModels, updateSettings, type Settings } from "./processing";
import { aiStatus, deleteApiKey, listAiModels, managedConnect, managedDisconnect, managedStatus, saveApiKey, testAiProvider, testCloudApiKey, updateAiSettings, type AiStatus } from "./ai";
import { openLink } from "./app-info";

vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), getSettings: vi.fn(), updateSettings: vi.fn(), installedModels: vi.fn(), acknowledgeCloudNotice: vi.fn() }));
vi.mock("./ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("./ai")>()), aiStatus: vi.fn(), listAiModels: vi.fn(), managedStatus: vi.fn(), managedConnect: vi.fn(), managedDisconnect: vi.fn(), saveApiKey: vi.fn(), deleteApiKey: vi.fn(), updateAiSettings: vi.fn(), testAiProvider: vi.fn(), testCloudApiKey: vi.fn() }));
vi.mock("./app-info", () => ({ openLink: vi.fn() }));
afterEach(() => { cleanup(); vi.useRealTimers(); vi.resetAllMocks(); });
beforeEach(() => { vi.mocked(installedModels).mockResolvedValue([]); vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false }); });

const noAi: AiSettings = { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null };
const empty: AiStatus = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };

const base: Settings = { recording_notice_acknowledged: true, cloud_notice_acknowledged: false, auto_process: true, model: "small", vocabulary: ["Nguyễn"], language: null, your_name: null, ai: noAi };

async function openTab(name: string) { fireEvent.mouseDown(await screen.findByRole("tab", { name }), { button: 0, ctrlKey: false }); }

it("shows five tabs, remembers the last one, and lets an opening link choose another", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  const first = render(<SettingsView initialTab="general" />);
  expect((await screen.findByRole("tab", { name: "General" })).getAttribute("aria-selected")).toBe("true");
  expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["General", "Speech-to-text", "Language models", "Keys", "Privacy"]);
  await openTab("Privacy");
  expect(screen.getByText("Privacy settings will appear here.")).toBeTruthy();
  first.unmount();
  const second = render(<SettingsView />);
  expect((await screen.findByRole("tab", { name: "Privacy" })).getAttribute("aria-selected")).toBe("true");
  second.unmount();
  render(<SettingsView initialTab={"speech-to-text" satisfies SettingsTabId} />);
  expect((await screen.findByRole("tab", { name: "Speech-to-text" })).getAttribute("aria-selected")).toBe("true");
  expect(await screen.findByRole("textbox", { name: "Vocabulary" })).toBeTruthy();
});

it("moves between tabs with arrow keys and exposes the active content in tab order", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  render(<SettingsView initialTab="general" />);
  const general = await screen.findByRole("tab", { name: "General" });
  general.focus();
  fireEvent.keyDown(general, { key: "ArrowRight" });
  const speech = screen.getByRole("tab", { name: "Speech-to-text" });
  await waitFor(() => expect(document.activeElement).toBe(speech));
  await waitFor(() => expect(speech.getAttribute("aria-selected")).toBe("true"));
  expect(speech.getAttribute("tabindex")).toBe("0");
  expect(general.getAttribute("tabindex")).toBe("-1");
  const panel = screen.getByRole("tabpanel", { name: "Speech-to-text" });
  expect(panel.getAttribute("tabindex")).toBe("0");
  panel.focus();
  expect(document.activeElement).toBe(panel);
  const vocabulary = screen.getByRole("textbox", { name: "Vocabulary" });
  expect(panel.contains(vocabulary)).toBe(true);
  vocabulary.focus();
  expect(document.activeElement).toBe(vocabulary);
  expect(panel.className).toContain("settings-tab");
});

it("keeps Saved in the header across tabs and fades it out after two seconds", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(updateSettings).mockResolvedValue(base);
  render(<SettingsView initialTab="general" />);
  await screen.findByRole("switch", { name: "Process after I stop" });
  vi.useFakeTimers();
  fireEvent.click(screen.getByRole("switch", { name: "Process after I stop" }));
  await act(async () => {});
  const saved = screen.getByText("Saved");
  expect(saved.getAttribute("role")).toBe("status");
  expect(saved.getAttribute("data-state")).toBe("visible");
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Keys" }), { button: 0, ctrlKey: false });
  expect(screen.getByText("Saved")).toBe(saved);
  act(() => { vi.advanceTimersByTime(2000); });
  expect(saved.getAttribute("data-state")).toBe("leaving");
  act(() => { vi.advanceTimersByTime(120); });
  expect(screen.queryByText("Saved")).toBeNull();
});

it("connects Managed transcription and shows Connected", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(managedConnect).mockResolvedValue({ connected: true, revoked: false });
  render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
  const section = await screen.findByRole("region", { name: "Managed transcription (experimental)" });
  fireEvent.click(within(section).getByRole("button", { name: "Have an invite code?" }));
  fireEvent.change(within(section).getByLabelText("Invite code"), { target: { value: " code " } });
  fireEvent.click(within(section).getByRole("button", { name: "Connect" }));
  await waitFor(() => expect(managedConnect).toHaveBeenCalledWith(" code "));
  expect(await within(section).findByText("Connected")).toBeTruthy();
  expect(within(section).queryByLabelText("Invite code")).toBeNull();
});

it("shows the invite error when Managed transcription rejects a code", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(managedConnect).mockRejectedValue("This invite code is used, expired or unknown. Ask for a new one.");
  render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
  const section = await screen.findByRole("region", { name: "Managed transcription (experimental)" });
  fireEvent.click(within(section).getByRole("button", { name: "Have an invite code?" }));
  fireEvent.change(within(section).getByLabelText("Invite code"), { target: { value: "bad-code" } });
  fireEvent.click(within(section).getByRole("button", { name: "Connect" }));
  expect((await within(section).findByRole("alert")).textContent).toBe("This invite code is used, expired or unknown. Ask for a new one.");
});

it("disconnects Managed transcription and returns to an empty invite field", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false });
  vi.mocked(managedDisconnect).mockResolvedValue({ connected: false, revoked: false });
  render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
  const section = await screen.findByRole("region", { name: "Managed transcription (experimental)" });
  fireEvent.click(await within(section).findByRole("button", { name: "Disconnect" }));
  await waitFor(() => expect(managedDisconnect).toHaveBeenCalledTimes(1));
  fireEvent.click(within(section).getByRole("button", { name: "Have an invite code?" }));
  expect((await within(section).findByLabelText("Invite code") as HTMLInputElement).value).toBe("");
});

it("explains when Managed transcription was revoked", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: true });
  render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
  const section = await screen.findByRole("region", { name: "Managed transcription (experimental)" });
  expect(await within(section).findByText("Managed transcription was disconnected.")).toBeTruthy();
  expect(within(section).getByLabelText("Invite code")).toBeTruthy();
});

it("saves, tests and removes Groq and OpenAI keys on Keys, and shows the cloud notice once", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(saveApiKey).mockResolvedValueOnce({ ...empty, has_groq_key: true }).mockResolvedValueOnce({ ...empty, has_groq_key: true, has_openai_key: true });
  vi.mocked(deleteApiKey).mockResolvedValueOnce({ ...empty, has_openai_key: true }).mockResolvedValueOnce(empty);
  vi.mocked(acknowledgeCloudNotice).mockResolvedValue({ ...base, cloud_notice_acknowledged: true });
  vi.mocked(testCloudApiKey).mockResolvedValue();
  vi.mocked(listAiModels).mockResolvedValue(["gpt-4o-mini"]);
  vi.mocked(openLink).mockResolvedValue();
  render(<SettingsView initialTab="general" />);
  await openTab("Keys");
  const keys = await screen.findByRole("tabpanel", { name: "Keys" });
  fireEvent.click(within(keys).getByRole("button", { name: "Get a free Groq key" }));
  fireEvent.click(within(keys).getByRole("button", { name: "Get a free Gemini key" }));
  expect(openLink).toHaveBeenCalledWith("groq");
  expect(openLink).toHaveBeenCalledWith("gemini");
  fireEvent.change(within(keys).getByLabelText("Groq API key"), { target: { value: "secret" } });
  fireEvent.click(within(keys).getByRole("button", { name: "Save Groq key" }));
  await waitFor(() => expect(saveApiKey).toHaveBeenCalledWith("groq", "secret"));
  expect(await within(keys).findByText(/Gemini's free tier may use content for training/)).toBeTruthy();
  fireEvent.click(within(keys).getByRole("button", { name: "Got it" }));
  await waitFor(() => expect(acknowledgeCloudNotice).toHaveBeenCalledTimes(1));
  fireEvent.click(within(keys).getByRole("button", { name: "Test Groq key" }));
  expect(await within(keys).findByText("Groq key: OK")).toBeTruthy();
  fireEvent.change(within(keys).getByLabelText("OpenAI API key"), { target: { value: "openai-secret" } });
  fireEvent.click(within(keys).getByRole("button", { name: "Save OpenAI key" }));
  await waitFor(() => expect(saveApiKey).toHaveBeenCalledWith("openai", "openai-secret"));
  fireEvent.click(within(keys).getByRole("button", { name: "Test OpenAI key" }));
  await waitFor(() => expect(listAiModels).toHaveBeenCalledWith("openai"));
  expect(await within(keys).findByText("OpenAI key: OK")).toBeTruthy();
  fireEvent.click(within(keys).getByRole("button", { name: "Remove Groq key" }));
  await waitFor(() => expect(deleteApiKey).toHaveBeenCalledWith("groq"));
  fireEvent.click(within(keys).getByRole("button", { name: "Remove OpenAI key" }));
  await waitFor(() => expect(deleteApiKey).toHaveBeenCalledWith("openai"));
  expect(within(keys).queryByText(/Gemini's free tier may use content for training/)).toBeNull();
  cleanup();
  vi.mocked(getSettings).mockResolvedValue({ ...base, cloud_notice_acknowledged: true });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, has_groq_key: true, has_openai_key: true });
  render(<SettingsView initialTab="general" />);
  await openTab("Keys");
  await screen.findByRole("region", { name: "Provider keys" });
  expect(screen.queryByText(/Gemini's free tier may use content for training/)).toBeNull();
});

it("saves Your name and clears it to the Me default", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(updateSettings).mockImplementation(async (auto_process, model, vocabulary, language, your_name) => ({ ...base, auto_process, model, vocabulary, language, your_name }));
  render(<SettingsView initialTab="general" />);
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
  render(<SettingsView initialTab="general" />);
  const auto = await screen.findByRole("switch", { name: "Process after I stop" });
  expect(auto.getAttribute("aria-checked")).toBe("true");
  fireEvent.click(auto);
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(false, "small", ["Nguyễn"], null, null));
  await openTab("Speech-to-text");
  fireEvent.click(screen.getByRole("radio", { name: "Large v3 turbo" }));
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(false, "large-v3-turbo-q5_0", ["Nguyễn"], null, null));
  await openTab("General");
  fireEvent.change(screen.getByRole("combobox", { name: "Default language" }), { target: { value: "vi" } });
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(false, "large-v3-turbo-q5_0", ["Nguyễn"], "vi", null));
  await openTab("Speech-to-text");
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
  render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
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
  render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
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
  const { unmount } = render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
  fireEvent.change(await screen.findByRole("textbox", { name: "Vocabulary" }), { target: { value: "Nguyễn\nLan" } });
  unmount();
  await waitFor(() => expect(updateSettings).toHaveBeenCalledWith(true, "small", ["Nguyễn", "Lan"], null, null));
});

it("retries a failed save", async () => {
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(updateSettings).mockRejectedValueOnce("disk full").mockImplementation(async (auto_process, model, vocabulary, language) => ({ ...base, auto_process, model, vocabulary, language }));
  render(<SettingsView initialTab="general" />);
  await openTab("Speech-to-text");
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
  render(<SettingsView initialTab="general" />);
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
  render(<SettingsView initialTab="general" />);
  await openTab("Language models");
  fireEvent.click(await screen.findByRole("radio", { name: /^Ollama/ }));
  await waitFor(() => expect(updateAiSettings).toHaveBeenCalledWith("ollama", null));
  fireEvent.click(await screen.findByRole("button", { name: "Find Ollama models" }));
  fireEvent.change(await screen.findByRole("combobox", { name: "Model" }), { target: { value: "llama3.2" } });
  await waitFor(() => expect(updateAiSettings).toHaveBeenCalledWith("ollama", "llama3.2"));
  fireEvent.click(await screen.findByRole("button", { name: "Test" }));
  expect((await screen.findByRole("status")).textContent).toContain("Test reply: ready");
});

it("offers Gemini for Summary and Ask with its saved key and model", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, has_gemini_key: true });
  vi.mocked(updateAiSettings).mockResolvedValue({ ...empty, provider: "gemini", model: "gemini-2.5-flash", configured: true, has_gemini_key: true });
  vi.mocked(listAiModels).mockResolvedValue(["gemini-2.5-flash", "gemini-2.5-pro"]);
  render(<SettingsView initialTab="general" />);
  await openTab("Language models");
  fireEvent.click(await screen.findByRole("radio", { name: /^Gemini/ }));
  await waitFor(() => expect(updateAiSettings).toHaveBeenCalledWith("gemini", null));
  expect(await screen.findByText(/Key saved in Windows Credential Manager/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Manage keys" }));
  expect(screen.getByRole("tab", { name: "Keys" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getAllByLabelText("Gemini API key")).toHaveLength(1);
  await openTab("Language models");
  expect((screen.getByRole("combobox", { name: "Model" }) as HTMLSelectElement).value).toBe("gemini-2.5-flash");
  fireEvent.click(screen.getByRole("button", { name: "Load models" }));
  await waitFor(() => expect(listAiModels).toHaveBeenCalledWith("gemini"));
});

it("routes a Language model with no key to the Keys tab", async () => {
  vi.mocked(getSettings).mockResolvedValue({ ...base, vocabulary: [] });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, provider: "openai" });
  render(<SettingsView initialTab="general" />);
  await openTab("Language models");
  const manage = screen.getByRole("button", { name: "Manage keys" });
  expect(manage.parentElement?.textContent).toContain("No key");
  expect(within(screen.getByRole("tabpanel", { name: "Language models" })).queryByLabelText("OpenAI API key")).toBeNull();
  fireEvent.click(manage);
  expect(screen.getByRole("tab", { name: "Keys" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getByLabelText("OpenAI API key")).toBeTruthy();
});

it("shows cloud transcription key status and opens Keys without a second Gemini field", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, has_gemini_key: true });
  render(<SettingsView initialTab="speech-to-text" />);
  const speech = await screen.findByRole("tabpanel", { name: "Speech-to-text" });
  const cloud = within(speech).getByRole("region", { name: "Cloud transcription" });
  expect(within(cloud).getByText("Gemini · Speech-to-text and Language models")).toBeTruthy();
  expect(within(cloud).getByText("Key saved")).toBeTruthy();
  expect(within(cloud).queryByLabelText("Gemini API key")).toBeNull();
  fireEvent.click(within(cloud).getAllByRole("button", { name: "Manage keys" })[1]);
  expect(screen.getByRole("tab", { name: "Keys" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getAllByLabelText("Gemini API key")).toHaveLength(1);
  expect(screen.getByText("http://localhost:11434")).toBeTruthy();
});

it("explains how to start Ollama when model discovery fails", async () => {
  vi.mocked(getSettings).mockResolvedValue({ ...base, vocabulary: [] });
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, provider: "ollama" });
  vi.mocked(listAiModels).mockRejectedValue("Ollama is not running at localhost:11434");
  render(<SettingsView initialTab="general" />);
  await openTab("Language models");
  fireEvent.click(await screen.findByRole("button", { name: "Find Ollama models" }));
  expect((await screen.findByRole("alert")).textContent).toContain("ollama pull llama3.2");
});
