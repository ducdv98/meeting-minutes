import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import TranscribeDialog from "./TranscribeDialog";
import { aiStatus, managedStatus } from "./ai";
import { getSettings, installedModels } from "./processing";

vi.mock("./ai", async (original) => ({ ...(await original<typeof import("./ai")>()), aiStatus: vi.fn(), managedStatus: vi.fn() }));
vi.mock("./processing", async (original) => ({ ...(await original<typeof import("./processing")>()), getSettings: vi.fn(), installedModels: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
beforeEach(() => {
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, cloud_notice_acknowledged: true, auto_process: false, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null }, speech: { models: [{ provider: "groq", model: "whisper-large-v3" }, { provider: "gemini", model: "gemini-2.5-flash" }], automatic_groq: "whisper-large-v3", automatic_gemini: "gemini-2.5-flash", automatic_whisper: "small" } });
  vi.mocked(aiStatus).mockResolvedValue({ provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: true, has_gemini_key: false });
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false });
  vi.mocked(installedModels).mockResolvedValue(["small"]);
});

it("defaults to the Transcription order and groups local and cloud choices", async () => {
  const onTranscribe = vi.fn();
  render(<TranscribeDialog again={false} onClose={() => {}} onTranscribe={onTranscribe} onOpenSettings={() => {}} />);
  expect(screen.getByRole("button", { name: "Automatic (Transcription order)" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Choose Speech-to-text model" }), { button: 0, ctrlKey: false });
  expect(await screen.findByText("On this PC")).toBeTruthy();
  expect(screen.getByText("Cloud")).toBeTruthy();
  expect(within(screen.getByRole("menuitem", { name: "Local Whisper · Small" })).getByText("Downloaded")).toBeTruthy();
  const gemini = screen.getByRole("menuitem", { name: "Gemini · gemini-2.5-flash, set up in Settings" });
  expect(gemini.hasAttribute("data-disabled")).toBe(true);
  expect(within(gemini).getByText("No key")).toBeTruthy();
  expect(within(screen.getByRole("menuitem", { name: "Groq · whisper-large-v3" })).getByText("Ready · sends audio to Groq")).toBeTruthy();
  fireEvent.click(screen.getByRole("menuitem", { name: "Local Whisper · Large v3 turbo" }));
  expect(screen.getByText("Downloads when first used (574 MB)")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Transcribe" }));
  expect(onTranscribe).toHaveBeenCalledWith("local:large-v3-turbo-q5_0");
});

it("passes no model for Automatic and shows where a cloud choice sends audio", async () => {
  const onTranscribe = vi.fn();
  render(<TranscribeDialog again onClose={() => {}} onTranscribe={onTranscribe} onOpenSettings={() => {}} />);
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Choose Speech-to-text model" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Groq · whisper-large-v3" }));
  expect(screen.getByText("Ready · sends audio to Groq")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Transcribe" }));
  expect(screen.getByText("This Meeting's audio will be sent to Groq for transcription with whisper-large-v3.")).toBeTruthy();
  expect(onTranscribe).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Back" }));
  fireEvent.click(screen.getByRole("button", { name: "Automatic (Transcription order)" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Automatic (Transcription order)" }).getAttribute("aria-pressed")).toBe("true"));
  fireEvent.click(screen.getByRole("button", { name: "Transcribe" }));
  expect(onTranscribe).toHaveBeenCalledWith(undefined);
});
it("lists disconnected Managed transcription as unavailable", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  render(<TranscribeDialog again onClose={() => {}} onTranscribe={() => {}} onOpenSettings={() => {}} />);
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Choose Speech-to-text model" }), { button: 0, ctrlKey: false });
  const managed = await screen.findByRole("menuitem", { name: "Managed transcription · Fixed model, set up in Settings" });
  expect(managed.hasAttribute("data-disabled")).toBe(true);
  expect(within(managed).getByText("Not connected")).toBeTruthy();
});
