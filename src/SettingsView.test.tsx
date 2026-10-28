import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SettingsView, { type SettingsTabId } from "./SettingsView";
import type { AiSettings } from "./ai";
import { acknowledgeCloudNotice, firstRunInfo, getSettings, installedModels, modelDownloadStatus, onModelDownloadStatus, removeSilencedRecordPromptApp, setRecordPromptEnabled, updateSettings, type Settings } from "./processing";
import { aiStatus, deleteApiKey, listAiModels, managedAccount, managedSendCode, managedSignIn, managedDisconnect, managedRemoveKey, managedSaveKey, managedStatus, saveApiKey, testCloudApiKey, testLanguageModel, updateLanguageModels, type AiStatus } from "./ai";
import { openLink } from "./app-info";
import { CLOUD_NOTICE_SUMMARY, CLOUD_NOTICE_TEXT, PRIVACY_INTRO } from "./privacy-text";
import { onSnapshotHotkeyError } from "./recording";
import { updateSnapshotHotkeys } from "./processing";
import { usePlatformCapabilities } from "./platform-capabilities";

vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), firstRunInfo: vi.fn(), getSettings: vi.fn(), updateSettings: vi.fn(), setRecordPromptEnabled: vi.fn(), removeSilencedRecordPromptApp: vi.fn(), updateSnapshotHotkeys: vi.fn(), installedModels: vi.fn(), acknowledgeCloudNotice: vi.fn(), modelDownloadStatus: vi.fn(), onModelDownloadStatus: vi.fn() }));
vi.mock("./ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("./ai")>()), aiStatus: vi.fn(), listAiModels: vi.fn(), managedStatus: vi.fn(), managedAccount: vi.fn(), managedSendCode: vi.fn(), managedSignIn: vi.fn(), managedDisconnect: vi.fn(), managedRemoveKey: vi.fn(), managedSaveKey: vi.fn(), saveApiKey: vi.fn(), deleteApiKey: vi.fn(), updateLanguageModels: vi.fn(), testLanguageModel: vi.fn(), testCloudApiKey: vi.fn() }));
vi.mock("./app-info", () => ({ openLink: vi.fn() }));
vi.mock("./recording", async (importOriginal) => ({ ...(await importOriginal<typeof import("./recording")>()), onSnapshotHotkeyError: vi.fn() }));
vi.mock("./platform-capabilities", async (importOriginal) => ({ ...(await importOriginal<typeof import("./platform-capabilities")>()), usePlatformCapabilities: vi.fn() }));
afterEach(() => { cleanup(); vi.useRealTimers(); vi.resetAllMocks(); });
beforeEach(() => { vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: true, backgroundTranscription: true, snapshots: true }); });
beforeEach(() => { vi.mocked(firstRunInfo).mockResolvedValue({ discrete_gpu: false, library_location: "D:\\Meetings" }); vi.mocked(installedModels).mockResolvedValue([]); vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false }); vi.mocked(managedAccount).mockResolvedValue({ email: "person@example.com", state: "admitted", minutes_used: 0.5, minutes_remaining: 119.5, allowance_minutes: 120, resets_at: "2026-11-01T00:00:00.000Z" }); vi.mocked(modelDownloadStatus).mockResolvedValue([]); vi.mocked(onModelDownloadStatus).mockResolvedValue(() => {}); vi.mocked(onSnapshotHotkeyError).mockResolvedValue(() => {}); });

it("shows the reported Managed allowance without Account UI", async () => {
  vi.mocked(getSettings).mockResolvedValue(base); vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false, remaining_minutes: 119.5 });
  render(<SettingsView initialTab="speech-to-text" />);
  expect((await screen.findAllByText("119.5 minutes remaining this month.", {}, { timeout: 3_000 })).length).toBeGreaterThan(0);
  expect(screen.queryByRole("region", { name: "Account" })).toBeNull();
  expect(screen.queryByText(/Waitlisted|Delete account|Invite code/)).toBeNull();
  expect(managedAccount).not.toHaveBeenCalled();
});

const noAi: AiSettings = { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null };
const empty: AiStatus = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };

const base: Settings = { first_run_completed: true, recording_notice_acknowledged: true, cloud_notice_acknowledged: false, auto_process: true, model: "small", vocabulary: ["Nguyễn"], language: null, your_name: null, ai: noAi, language_models: { models: [], correction: null, summary: null, ask: null, confirmed_correction_providers: [], legacy_correction_pending: false } };

async function openTab(name: string) { fireEvent.mouseDown(await screen.findByRole("tab", { name }), { button: 0, ctrlKey: false }); }

it("turns the Record prompt off and removes a silenced app", async () => {
  const withSilencedApp = { ...base, record_prompt_enabled: true, silenced_record_prompt_apps: [{ id: "teams.exe", display_name: "Teams" }] };
  vi.mocked(getSettings).mockResolvedValue(withSilencedApp);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  const withNewApp = { ...withSilencedApp, record_prompt_enabled: false, silenced_record_prompt_apps: [...withSilencedApp.silenced_record_prompt_apps, { id: "zoom.exe", display_name: "Zoom" }] };
  vi.mocked(setRecordPromptEnabled).mockResolvedValue(withNewApp);
  vi.mocked(removeSilencedRecordPromptApp).mockResolvedValue({ ...withNewApp, silenced_record_prompt_apps: [{ id: "zoom.exe", display_name: "Zoom" }] });
  render(<SettingsView initialTab="general" />);
  fireEvent.click(await screen.findByRole("switch", { name: "Record prompt" }));
  await waitFor(() => expect(setRecordPromptEnabled).toHaveBeenCalledWith(false));
  expect(await screen.findByText("Zoom")).toBeTruthy();
  expect(screen.getByText("Teams")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Remove Teams" }));
  await waitFor(() => expect(removeSilencedRecordPromptApp).toHaveBeenCalledWith("teams.exe"));
  expect(screen.queryByText("Teams")).toBeNull();
  expect(screen.getByText("Zoom")).toBeTruthy();
});

it("hides unsupported Record prompt and Snapshot settings", async () => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: false, backgroundTranscription: false, snapshots: false });
  vi.mocked(getSettings).mockResolvedValue({ ...base, record_prompt_enabled: true, silenced_record_prompt_apps: [{ id: "teams.exe", display_name: "Teams" }] });
  vi.mocked(aiStatus).mockResolvedValue(empty);
  render(<SettingsView initialTab="general" />);
  await screen.findByRole("switch", { name: "Process after I stop" });
  expect(screen.queryByRole("switch", { name: "Record prompt" })).toBeNull();
  expect(screen.queryByText("Apps you silenced")).toBeNull();
  expect(screen.queryByRole("textbox", { name: "Take Snapshot" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "Draw new Capture region" })).toBeNull();
  expect(screen.getByText(/Record: Ctrl/)).toBeTruthy();
  expect(onSnapshotHotkeyError).not.toHaveBeenCalled();
});

it("uses macOS storage and computer wording in Settings", async () => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: false, backgroundTranscription: false, snapshots: false });
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  render(<SettingsView initialTab="keys" />);
  const keys = await screen.findByRole("tabpanel", { name: "Keys" });
  expect(within(keys).getByText("Keys stay in macOS Keychain.")).toBeTruthy();
  expect(within(keys).getByText("On this computer. No key needed.")).toBeTruthy();
  await openTab("Speech-to-text");
  const speech = screen.getByRole("tabpanel", { name: "Speech-to-text" });
  expect(within(speech).getByText("Use your own keys for cloud Processing. Keys stay in macOS Keychain.")).toBeTruthy();
  expect(within(speech).getByText(/Runs on this computer after cloud Providers/)).toBeTruthy();
});

it("retries a failed Record prompt switch with its attempted value", async () => {
  const withSilencedApp = { ...base, record_prompt_enabled: true, silenced_record_prompt_apps: [{ id: "teams.exe", display_name: "Teams" }] };
  vi.mocked(getSettings).mockResolvedValue(withSilencedApp);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(setRecordPromptEnabled).mockRejectedValueOnce("disk full").mockResolvedValue({ ...withSilencedApp, record_prompt_enabled: false });
  render(<SettingsView initialTab="general" />);
  fireEvent.click(await screen.findByRole("switch", { name: "Record prompt" }));
  expect((await screen.findByRole("alert")).textContent).toContain("disk full");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(setRecordPromptEnabled).toHaveBeenCalledTimes(2));
  expect(setRecordPromptEnabled).toHaveBeenNthCalledWith(2, false);
  expect(updateSettings).not.toHaveBeenCalled();
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
});

it("retries removing the same silenced app after a save failure", async () => {
  const withSilencedApp = { ...base, record_prompt_enabled: true, silenced_record_prompt_apps: [{ id: "teams.exe", display_name: "Teams" }] };
  vi.mocked(getSettings).mockResolvedValue(withSilencedApp);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(removeSilencedRecordPromptApp).mockRejectedValueOnce("disk full").mockResolvedValue({ ...withSilencedApp, silenced_record_prompt_apps: [] });
  render(<SettingsView initialTab="general" />);
  fireEvent.click(await screen.findByRole("button", { name: "Remove Teams" }));
  expect((await screen.findByRole("alert")).textContent).toContain("disk full");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(removeSilencedRecordPromptApp).toHaveBeenCalledTimes(2));
  expect(removeSilencedRecordPromptApp).toHaveBeenNthCalledWith(2, "teams.exe");
  expect(updateSettings).not.toHaveBeenCalled();
});

it("shows the resolved Library root in General", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  render(<SettingsView initialTab="general" />);
  expect(await screen.findByText("D:\\Meetings")).toBeTruthy();
  expect(firstRunInfo).toHaveBeenCalledOnce();
});

it("changes both Snapshot hotkeys and rejects a duplicate combination", async () => {
  vi.mocked(getSettings).mockResolvedValue({ ...base, snapshot_hotkey: "Ctrl+Alt+S", draw_snapshot_hotkey: "Ctrl+Alt+Shift+S" });
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(updateSnapshotHotkeys).mockImplementation(async (snapshotHotkey, drawSnapshotHotkey) => ({ ...base, snapshot_hotkey: snapshotHotkey, draw_snapshot_hotkey: drawSnapshotHotkey }));
  render(<SettingsView initialTab="general" />);
  const repeat = await screen.findByRole("textbox", { name: "Take Snapshot" });
  const draw = screen.getByRole("textbox", { name: "Draw new Capture region" });
  expect(repeat).toHaveProperty("value", "Ctrl+Alt+S");
  fireEvent.keyDown(repeat, { key: "D", ctrlKey: true, altKey: true });
  await waitFor(() => expect(updateSnapshotHotkeys).toHaveBeenCalledWith("Ctrl+Alt+D", "Ctrl+Alt+Shift+S"));
  fireEvent.keyDown(draw, { key: "F", ctrlKey: true, altKey: true, shiftKey: true });
  await waitFor(() => expect(updateSnapshotHotkeys).toHaveBeenLastCalledWith("Ctrl+Alt+D", "Ctrl+Alt+Shift+F"));
  fireEvent.keyDown(draw, { key: "D", ctrlKey: true, altKey: true });
  expect((await screen.findByRole("alert")).textContent).toContain("different combinations");
  expect(updateSnapshotHotkeys).toHaveBeenCalledTimes(2);
});

it("shows the named Snapshot hotkey when registration fails", async () => {
  vi.mocked(getSettings).mockResolvedValue({ ...base, snapshot_hotkey: "Ctrl+Alt+S", draw_snapshot_hotkey: "Ctrl+Alt+Shift+S" });
  vi.mocked(aiStatus).mockResolvedValue(empty);
  let notify: (message: string) => void = () => {};
  vi.mocked(onSnapshotHotkeyError).mockImplementation(async (handler) => { notify = handler; return () => {}; });
  render(<SettingsView initialTab="general" />);
  await screen.findByRole("textbox", { name: "Take Snapshot" });
  await waitFor(() => expect(onSnapshotHotkeyError).toHaveBeenCalled());
  act(() => notify("Could not register Ctrl+Alt+S: already in use"));
  expect(screen.getByRole("alert").textContent).toContain("Ctrl+Alt+S");
});

it("shows five tabs, remembers the last one, and lets an opening link choose another", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  const first = render(<SettingsView initialTab="general" />);
  expect((await screen.findByRole("tab", { name: "General" })).getAttribute("aria-selected")).toBe("true");
  expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["General", "Speech-to-text", "Language models", "Keys", "Privacy"]);
  await openTab("Privacy");
  expect(screen.getByText(PRIVACY_INTRO)).toBeTruthy();
  expect(screen.getByText(/Snapshots never leave this PC/)).toBeTruthy();
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
  expect(saved.closest("header")).toBe(screen.getByRole("heading", { name: "Settings" }).closest("header"));
  expect(screen.getByRole("tabpanel", { name: "General" }).parentElement?.className).toContain("motion-content-enter");
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Keys" }), { button: 0, ctrlKey: false });
  expect(screen.getByText("Saved")).toBe(saved);
  act(() => { vi.advanceTimersByTime(2000); });
  expect(saved.getAttribute("data-state")).toBe("leaving");
  act(() => { vi.advanceTimersByTime(120); });
  expect(screen.queryByText("Saved")).toBeNull();
});

it("saves and removes a Managed key in Provider keys", async () => {
  vi.mocked(getSettings).mockResolvedValue(base); vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(managedSaveKey).mockResolvedValue({ connected: true, revoked: false, remaining_minutes: 60 });
  vi.mocked(managedRemoveKey).mockResolvedValue({ connected: false, revoked: false });
  render(<SettingsView initialTab="keys" />);
  const keys = await screen.findByRole("tabpanel", { name: "Keys" });
  const managed = within(keys).getByRole("group", { name: "Managed transcription Provider" });
  expect(within(managed).getByRole("link", { name: "Request a key" }).getAttribute("href")).toBe("https://meeting-minutes.tech/request-a-key/");
  expect(within(managed).getByText(/Saving a key allows Meeting audio to be sent to the cloud/)).toBeTruthy();
  fireEvent.change(within(managed).getByLabelText("Managed key"), { target: { value: "  mm-secret  " } });
  fireEvent.click(within(managed).getByRole("button", { name: "Save Managed key" }));
  await waitFor(() => expect(managedSaveKey).toHaveBeenCalledWith("mm-secret"));
  expect(within(managed).getByText("60 minutes remaining this month.")).toBeTruthy();
  expect(within(managed).getByText("Key saved")).toBeTruthy();
  fireEvent.click(within(managed).getByRole("button", { name: "Remove Managed key" }));
  await waitFor(() => expect(managedRemoveKey).toHaveBeenCalledOnce());
  expect(within(managed).getByText("No key")).toBeTruthy();
});

it("shows an invalid or revoked Managed key and accepts a replacement", async () => {
  vi.mocked(getSettings).mockResolvedValue(base); vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: true });
  vi.mocked(managedSaveKey).mockResolvedValue({ connected: true, revoked: false });
  render(<SettingsView initialTab="keys" />);
  const keys = await screen.findByRole("tabpanel", { name: "Keys" });
  const managed = within(keys).getByRole("group", { name: "Managed transcription Provider" });
  expect(await within(managed).findByText("Key invalid or revoked")).toBeTruthy();
  fireEvent.change(within(managed).getByLabelText("Managed key"), { target: { value: "new-key" } });
  fireEvent.click(within(managed).getByRole("button", { name: "Save Managed key" }));
  await waitFor(() => expect(managedSaveKey).toHaveBeenCalledWith("new-key"));
  expect(within(managed).queryByText("Key invalid or revoked")).toBeNull();
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
  expect(await within(keys).findByText(CLOUD_NOTICE_SUMMARY)).toBeTruthy();
  const disclosure = within(within(keys).getByRole("alert")).getByRole("button", { name: "Details" });
  expect(disclosure.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(disclosure);
  expect(disclosure.getAttribute("aria-expanded")).toBe("true");
  expect(within(keys).getByText(CLOUD_NOTICE_TEXT)).toBeTruthy();
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
  expect(status(turbo)).toContain("Download required");
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

it("adds and tests an Ollama Language model", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(updateLanguageModels).mockImplementation(async (models) => ({ ...base, language_models: models }));
  vi.mocked(testLanguageModel).mockResolvedValue("ready");
  render(<SettingsView initialTab="language-models" />);
  await screen.findByRole("combobox", { name: "Provider" });
  fireEvent.change(screen.getByRole("textbox", { name: "Model ID" }), { target: { value: "llama3.2" } });
  fireEvent.click(screen.getByRole("button", { name: "Add model" }));
  await waitFor(() => expect(updateLanguageModels).toHaveBeenCalledWith(expect.objectContaining({ models: [{ provider: "ollama", model: "llama3.2" }] })));
  fireEvent.click(screen.getByRole("button", { name: "Test" }));
  await waitFor(() => expect(testLanguageModel).toHaveBeenCalledWith({ provider: "ollama", model: "llama3.2" }));
});

it("offers Gemini Language models with its saved key and links to Keys", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue({ ...empty, has_gemini_key: true });
  vi.mocked(listAiModels).mockResolvedValue(["gemini-2.5-flash"]);
  render(<SettingsView initialTab="language-models" />);
  const provider = await screen.findByRole("combobox", { name: "Provider" });
  fireEvent.change(provider, { target: { value: "gemini" } });
  fireEvent.click(screen.getByRole("button", { name: "Load models" }));
  await waitFor(() => expect(listAiModels).toHaveBeenCalledWith("gemini"));
  expect(screen.getByRole("combobox", { name: "Provider models" })).toBeTruthy();
  fireEvent.change(provider, { target: { value: "openai" } });
  fireEvent.click(screen.getByRole("button", { name: "Manage keys" }));
  expect(screen.getByRole("tab", { name: "Keys" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getAllByLabelText("Gemini API key")).toHaveLength(1);
});

it("routes a Language model with no key to the Keys tab", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  render(<SettingsView initialTab="language-models" />);
  const provider = await screen.findByRole("combobox", { name: "Provider" });
  fireEvent.change(provider, { target: { value: "openai" } });
  const manage = screen.getByRole("button", { name: "Manage keys" });
  expect(manage.parentElement?.textContent).toContain("No key");
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

it("shows model discovery errors", async () => {
  vi.mocked(getSettings).mockResolvedValue(base);
  vi.mocked(aiStatus).mockResolvedValue(empty);
  vi.mocked(listAiModels).mockRejectedValue("Ollama is not running at localhost:11434");
  render(<SettingsView initialTab="language-models" />);
  fireEvent.click(await screen.findByRole("button", { name: "Load models" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Ollama is not running");
});
