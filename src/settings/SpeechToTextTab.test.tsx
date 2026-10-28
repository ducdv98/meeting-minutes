import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SpeechToTextTab } from "./SpeechToTextTab";
import { managedAccount, managedDeleteAccount, managedDisconnect, managedSendCode, managedSetEnabled, managedSignIn, managedStatus } from "../ai";
import { cancelModelDownload, deleteModelDownload, modelDownloadStatus, onModelDownloadStatus, startModelDownload, testSpeechModel, type ModelDownloadStatus } from "../processing";
import { MANAGED_PRIVACY_STATEMENT, MANAGED_PRIVACY_SUMMARY } from "../privacy-text";
import type { Settings } from "../processing";
import { usePlatformCapabilities } from "../platform-capabilities";

vi.mock("../ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("../ai")>()), managedStatus: vi.fn(), managedAccount: vi.fn(), managedSendCode: vi.fn(), managedSignIn: vi.fn(), managedDisconnect: vi.fn(), managedSetEnabled: vi.fn(), managedDeleteAccount: vi.fn() }));
vi.mock("../processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("../processing")>()), testSpeechModel: vi.fn(), modelDownloadStatus: vi.fn(), onModelDownloadStatus: vi.fn(), startModelDownload: vi.fn(), cancelModelDownload: vi.fn(), deleteModelDownload: vi.fn() }));
vi.mock("../platform-capabilities", async (importOriginal) => ({ ...(await importOriginal<typeof import("../platform-capabilities")>()), usePlatformCapabilities: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
beforeEach(() => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: true, backgroundTranscription: true, snapshots: true });
  vi.mocked(managedAccount).mockResolvedValue({ email: "person@example.com", state: "admitted", minutes_used: 0, minutes_remaining: 120, allowance_minutes: 120, resets_at: "2026-11-01T00:00:00.000Z" });
  vi.mocked(modelDownloadStatus).mockResolvedValue([]);
  vi.mocked(onModelDownloadStatus).mockResolvedValue(() => {});
});

const settings: Settings = { first_run_completed: true, recording_notice_acknowledged: true, cloud_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null } };

function renderTab() {
  return render(<SpeechToTextTab settings={settings} save={vi.fn()} installed={[]} vocabulary="" setVocabulary={vi.fn()} saveVocabulary={vi.fn()} providerStatus={null} onManageKeys={vi.fn()} />);
}

it("shows the Managed model and keeps Account controls hidden", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false, remaining_minutes: 42.5 });
  renderTab();
  expect(await screen.findByText("New Meetings use: Managed transcription")).toBeTruthy();
  expect(screen.getAllByText("Managed transcription · mm-transcription-internal").length).toBeGreaterThan(0);
  expect(screen.getByText("42.5 minutes remaining this month.")).toBeTruthy();
  expect(screen.queryByRole("region", { name: "Account" })).toBeNull();
  expect(screen.queryByText(/Waitlisted|Delete account|Sign in/)).toBeNull();
  expect(managedAccount).not.toHaveBeenCalled();
});

it("shows an invalid Managed key as unavailable", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: true });
  renderTab();
  expect(await screen.findByText("New Meetings use: Local Whisper · small")).toBeTruthy();
  expect(screen.getAllByText("Key invalid or revoked").length).toBeGreaterThan(0);
});

it("tests and adds a Groq model before using it automatically", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  vi.mocked(testSpeechModel).mockResolvedValue();
  const save = vi.fn();
  const cloud = { ...settings, speech: { models: [{ provider: "groq" as const, model: "whisper-large-v3" }], automatic_groq: "whisper-large-v3", automatic_gemini: null, automatic_whisper: "small" } };
  const providerStatus = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: true, has_gemini_key: false };
  const view = render(<SpeechToTextTab settings={cloud} save={save} installed={["small"]} vocabulary="" setVocabulary={vi.fn()} saveVocabulary={vi.fn()} providerStatus={providerStatus} onManageKeys={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Add Speech-to-text model" }));
  fireEvent.change(screen.getByLabelText("Model ID"), { target: { value: "whisper-large-v3-turbo" } });
  expect(screen.getByRole("button", { name: "Add model" }).hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Test" }));
  await waitFor(() => expect(testSpeechModel).toHaveBeenCalledWith("groq", "whisper-large-v3-turbo"));
  fireEvent.click(screen.getByRole("button", { name: "Add model" }));
  const added = save.mock.lastCall?.[0] as Settings;
  expect(added.speech?.models).toContainEqual({ provider: "groq", model: "whisper-large-v3-turbo" });
  view.rerender(<SpeechToTextTab settings={added} save={save} installed={["small"]} vocabulary="" setVocabulary={vi.fn()} saveVocabulary={vi.fn()} providerStatus={providerStatus} onManageKeys={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Use automatically" }));
  expect((save.mock.lastCall?.[0] as Settings).speech?.automatic_groq).toBe("whisper-large-v3-turbo");
});

it("updates the Transcription order when a Managed key changes", async () => {
  vi.mocked(managedStatus).mockResolvedValueOnce({ connected: false, revoked: false }).mockResolvedValue({ connected: true, revoked: false });
  renderTab();
  expect(screen.getByText("New Meetings use: Local Whisper · small")).toBeTruthy();
  await waitFor(() => expect(managedStatus).toHaveBeenCalledOnce());
  act(() => window.dispatchEvent(new Event("managed-key-changed")));
  expect(await screen.findByText("New Meetings use: Managed transcription")).toBeTruthy();
});

it("downloads, cancels, and deletes a local model from Settings", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  const status = (state: ModelDownloadStatus["state"], progress: number): ModelDownloadStatus => ({ model_id: "base", state, progress, error: null });
  vi.mocked(startModelDownload).mockResolvedValue([status("downloading", 0)]);
  vi.mocked(cancelModelDownload).mockResolvedValue([status("cancelled", 0)]);
  vi.mocked(deleteModelDownload).mockResolvedValue([status("not_downloaded", 0)]);
  renderTab();
  fireEvent.click(screen.getByRole("button", { name: "Download Base" }));
  await waitFor(() => expect(startModelDownload).toHaveBeenCalledWith("base"));
  const event = vi.mocked(onModelDownloadStatus).mock.calls[0][0];
  event(status("downloading", 37));
  expect(await screen.findByText("37%")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel Base" }));
  await waitFor(() => expect(cancelModelDownload).toHaveBeenCalledWith("base"));
  event(status("downloaded", 100));
  expect(await screen.findByRole("button", { name: "Delete Base" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Delete Base" }));
  await waitFor(() => expect(deleteModelDownload).toHaveBeenCalledWith("base"));
  expect(await screen.findByRole("button", { name: "Download Base" })).toBeTruthy();
});

it("keeps a completed download when its start response arrives late", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  let finish!: (statuses: ModelDownloadStatus[]) => void;
  vi.mocked(startModelDownload).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  renderTab();
  await waitFor(() => expect(onModelDownloadStatus).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: "Download Base" }));
  await act(async () => vi.mocked(onModelDownloadStatus).mock.calls[0][0]({ model_id: "base", state: "downloaded", progress: 100, error: null }));
  await act(async () => finish([{ model_id: "base", state: "downloading", progress: 0, error: null }]));
  expect(screen.getByRole("button", { name: "Delete Base" })).toBeTruthy();
});
