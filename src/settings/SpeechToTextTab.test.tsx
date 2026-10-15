import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SpeechToTextTab } from "./SpeechToTextTab";
import { managedDeleteAccount, managedDisconnect, managedSendCode, managedSetEnabled, managedSignIn, managedStatus } from "../ai";
import { cancelModelDownload, deleteModelDownload, modelDownloadStatus, onModelDownloadStatus, startModelDownload, testSpeechModel, type ModelDownloadStatus } from "../processing";
import { MANAGED_PRIVACY_STATEMENT, MANAGED_PRIVACY_SUMMARY } from "../privacy-text";
import type { Settings } from "../processing";

vi.mock("../ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("../ai")>()), managedStatus: vi.fn(), managedSendCode: vi.fn(), managedSignIn: vi.fn(), managedDisconnect: vi.fn(), managedSetEnabled: vi.fn(), managedDeleteAccount: vi.fn() }));
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

it("signs in with an email code while Managed transcription stays off until consent", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  vi.mocked(managedSendCode).mockResolvedValue();
  vi.mocked(managedSignIn).mockResolvedValue({ connected: true, revoked: false, email: "person@example.com", enabled: false });
  vi.mocked(managedSetEnabled).mockResolvedValue({ connected: true, revoked: false, email: "person@example.com", enabled: true });
  renderTab();
  const account = screen.getByRole("region", { name: "Account" });
  const managed = screen.getByRole("region", { name: "Managed transcription" });
  expect(within(managed).getByText(MANAGED_PRIVACY_SUMMARY)).toBeTruthy();
  const details = within(managed).getByRole("button", { name: "Details" });
  fireEvent.click(details);
  expect(within(managed).getByText(MANAGED_PRIVACY_STATEMENT)).toBeTruthy();
  fireEvent.change(within(account).getByLabelText("Email address"), { target: { value: "person@example.com" } });
  fireEvent.click(within(account).getByRole("button", { name: "Send code" }));
  await waitFor(() => expect(managedSendCode).toHaveBeenCalledWith("person@example.com"));
  fireEvent.change(within(account).getByLabelText("Six-digit code"), { target: { value: "123456" } });
  fireEvent.click(within(account).getByRole("button", { name: "Sign in" }));
  await waitFor(() => expect(managedSignIn).toHaveBeenCalledWith("person@example.com", "123456"));
  expect(within(account).getByText("Signed in as person@example.com")).toBeTruthy();
  expect(screen.getByText("New Meetings use: Local Whisper · small")).toBeTruthy();
  fireEvent.click(within(managed).getByRole("switch", { name: "Enable Managed transcription" }));
  await waitFor(() => expect(managedSetEnabled).toHaveBeenCalledWith(true));
  expect(screen.getByText("New Meetings use: Managed transcription")).toBeTruthy();
});

it("shows a legacy connection and keeps its tokens available while signing in", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false, email: null, enabled: true });
  renderTab();
  const account = screen.getByRole("region", { name: "Account" });
  expect(await within(account).findByText("Connected (legacy invite)")).toBeTruthy();
  expect(screen.getByText("New Meetings use: Managed transcription")).toBeTruthy();
  fireEvent.click(within(account).getByRole("button", { name: "Sign in" }));
  expect(within(account).getByLabelText("Email address")).toBeTruthy();
});

it("deletes an Account only after confirmation and leaves local Meetings alone", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false, email: "person@example.com", enabled: false });
  vi.mocked(managedDeleteAccount).mockResolvedValue({ connected: false, revoked: false, email: null, enabled: false });
  renderTab();
  const account = screen.getByRole("region", { name: "Account" });
  fireEvent.click(await within(account).findByRole("button", { name: "Delete account" }));
  expect(screen.getByText(/email address and Managed transcription usage history/)).toBeTruthy();
  expect(screen.getByText(/All Meetings and Transcripts on this PC stay here/)).toBeTruthy();
  expect(managedDeleteAccount).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("dialog").querySelector("button:last-child")!);
  await waitFor(() => expect(managedDeleteAccount).toHaveBeenCalledOnce());
  expect(await within(account).findByLabelText("Email address")).toBeTruthy();
});

it("keeps the Account signed in when deletion fails", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false, email: "person@example.com", enabled: false });
  vi.mocked(managedDeleteAccount).mockRejectedValue("Could not reach the Account service. Check your connection and try again.");
  renderTab();
  fireEvent.click(await screen.findByRole("button", { name: "Delete account" }));
  fireEvent.click(screen.getByRole("dialog").querySelector("button:last-child")!);
  expect(await screen.findByText("Could not reach the Account service. Check your connection and try again.")).toBeTruthy();
  expect(screen.getByText("Signed in as person@example.com")).toBeTruthy();
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
  vi.mocked(managedSendCode).mockResolvedValue();
  vi.mocked(managedSignIn).mockResolvedValue({ connected: true, revoked: false, email: "person@example.com", enabled: false });
  vi.mocked(managedSetEnabled).mockResolvedValue({ connected: true, revoked: false, email: "person@example.com", enabled: true });
  vi.mocked(managedDisconnect).mockResolvedValue({ connected: false, revoked: false, email: null, enabled: false });
  fireEvent.change(screen.getByLabelText("Email address"), { target: { value: "person@example.com" } });
  fireEvent.click(screen.getByRole("button", { name: "Send code" }));
  fireEvent.change(await screen.findByLabelText("Six-digit code"), { target: { value: "123456" } });
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
  expect(await screen.findByText("Signed in as person@example.com")).toBeTruthy();
  expect(screen.getByText("New Meetings use: Local Whisper · small")).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "Enable Managed transcription" }));
  expect(await screen.findByText("New Meetings use: Managed transcription")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
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
