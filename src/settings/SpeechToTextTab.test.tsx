import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SpeechToTextTab } from "./SpeechToTextTab";
import { managedConnect, managedDisconnect, managedStatus } from "../ai";
import { cancelModelDownload, deleteModelDownload, modelDownloadStatus, onModelDownloadStatus, startModelDownload, testSpeechModel, type ModelDownloadStatus } from "../processing";
import { MANAGED_PRIVACY_STATEMENT, MANAGED_PRIVACY_SUMMARY } from "../privacy-text";
import type { Settings } from "../processing";

vi.mock("../ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("../ai")>()), managedStatus: vi.fn(), managedConnect: vi.fn(), managedDisconnect: vi.fn() }));
vi.mock("../processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("../processing")>()), testSpeechModel: vi.fn(), modelDownloadStatus: vi.fn(), onModelDownloadStatus: vi.fn(), startModelDownload: vi.fn(), cancelModelDownload: vi.fn(), deleteModelDownload: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
beforeEach(() => {
  vi.mocked(modelDownloadStatus).mockResolvedValue([]);
  vi.mocked(onModelDownloadStatus).mockResolvedValue(() => {});
});

const settings: Settings = { first_run_completed: true, recording_notice_acknowledged: true, cloud_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null } };

function renderTab() {
  return render(<SpeechToTextTab settings={settings} save={vi.fn()} installed={[]} vocabulary="" setVocabulary={vi.fn()} saveVocabulary={vi.fn()} providerStatus={null} onManageKeys={vi.fn()} />);
}

it("shows the privacy statement before Connect, then connects after the invite link opens", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  vi.mocked(managedConnect).mockResolvedValue({ connected: true, revoked: false });
  renderTab();
  const section = screen.getByRole("region", { name: "Managed transcription (experimental)" });
  await waitFor(() => expect(managedStatus).toHaveBeenCalledOnce());
  expect(within(section).queryByLabelText("Invite code")).toBeNull();
  expect(within(section).getByText(MANAGED_PRIVACY_SUMMARY)).toBeTruthy();
  expect(MANAGED_PRIVACY_SUMMARY).toContain("text is kept up to 5 minutes");
  expect(MANAGED_PRIVACY_SUMMARY).not.toContain("nothing is stored");
  const details = within(section).getByRole("button", { name: "Details" });
  fireEvent.click(details);
  expect(details.getAttribute("aria-expanded")).toBe("true");
  expect(within(section).getByText(MANAGED_PRIVACY_STATEMENT)).toBeTruthy();
  fireEvent.click(within(section).getByRole("button", { name: "Have an invite code?" }));
  fireEvent.change(within(section).getByLabelText("Invite code"), { target: { value: " code " } });
  fireEvent.click(within(section).getByRole("button", { name: "Connect" }));
  await waitFor(() => expect(managedConnect).toHaveBeenCalledWith(" code "));
  expect(await within(section).findByText("Connected")).toBeTruthy();
});

it("opens the Invite code field when Managed transcription was revoked", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: true });
  renderTab();
  const section = screen.getByRole("region", { name: "Managed transcription (experimental)" });
  expect(await within(section).findByText("Managed transcription was disconnected.")).toBeTruthy();
  expect(within(section).getByLabelText("Invite code")).toBeTruthy();
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

it("updates the Transcription order when connection, key, and download state change", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  const speech = { models: [{ provider: "groq" as const, model: "whisper-large-v3" }], automatic_groq: "whisper-large-v3", automatic_gemini: null, automatic_whisper: "small" };
  const cloud = { ...settings, speech };
  const props = { settings: cloud, save: vi.fn(), vocabulary: "", setVocabulary: vi.fn(), saveVocabulary: vi.fn(), onManageKeys: vi.fn() };
  const status = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };
  const view = render(<SpeechToTextTab {...props} installed={[]} providerStatus={status} />);
  expect(screen.getByText("New Meetings use: Local Whisper · small")).toBeTruthy();
  const order = screen.getByRole("region", { name: "Transcription order" });
  expect(within(order).getByText("Download required")).toBeTruthy();
  view.rerender(<SpeechToTextTab {...props} installed={["small"]} providerStatus={{ ...status, has_groq_key: true }} />);
  expect(screen.getByText("New Meetings use: Groq · whisper-large-v3")).toBeTruthy();
  expect(within(order).getAllByText("Ready")).toHaveLength(2);
  view.rerender(<SpeechToTextTab {...props} installed={["small"]} providerStatus={status} />);
  expect(screen.getByText("New Meetings use: Local Whisper · small")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Have an invite code?" }));
  fireEvent.change(screen.getByLabelText("Invite code"), { target: { value: "code" } });
  vi.mocked(managedConnect).mockResolvedValue({ connected: true, revoked: false });
  await waitFor(() => expect(screen.getByRole("button", { name: "Connect" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "Connect" }));
  expect(await screen.findByText("New Meetings use: Managed transcription")).toBeTruthy();
  vi.mocked(managedDisconnect).mockResolvedValue({ connected: false, revoked: false });
  fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
  expect(await screen.findByText("New Meetings use: Local Whisper · small")).toBeTruthy();
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
