import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { managedStatus, type AiStatus } from "../ai";
import { CLOUD_NOTICE_TEXT, MANAGED_PRIVACY_STATEMENT, PRIVACY_DESTINATIONS, PRIVACY_INTRO } from "../privacy-text";
import type { Settings } from "../processing";
import { PrivacyTab } from "./PrivacyTab";

vi.mock("../ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("../ai")>()), managedStatus: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });

const status: AiStatus = { provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };
const settings: Settings = { recording_notice_acknowledged: true, cloud_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null }, language_models: { models: [], correction: null, summary: null, ask: null, confirmed_correction_providers: [], legacy_correction_pending: false } };

it("lists every destination and shows all six off when no cloud option is active", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  render(<PrivacyTab settings={settings} providerStatus={status} />);
  expect(screen.getByText(PRIVACY_INTRO)).toBeTruthy();
  for (const name of ["Managed transcription", "Groq", "Gemini", "OpenAI", "Anthropic", "Correction model"]) {
    const row = screen.getByRole("region", { name });
    expect(within(row).getByText(/^What:/)).toBeTruthy();
    expect(within(row).getByText(/^When:/)).toBeTruthy();
    expect(within(row).getByRole("button", { name: "Details" })).toBeTruthy();
  }
  expect(await within(screen.getByRole("region", { name: "Managed transcription" })).findByText("Off · Not connected")).toBeTruthy();
  for (const name of ["Managed transcription", "Groq", "Gemini", "OpenAI", "Anthropic", "Correction model"]) expect(within(screen.getByRole("region", { name })).getByText(/^Off/)).toBeTruthy();
  expect(screen.getByText(CLOUD_NOTICE_TEXT)).toBeTruthy();
});

it("updates key and Correction states from live Settings props", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: true, revoked: false });
  const cloud = { ...status, has_groq_key: true, has_anthropic_key: true };
  const correction = { provider: "anthropic" as const, model: "claude-test" };
  const nextSettings = { ...settings, speech: { models: [{ provider: "groq" as const, model: "whisper-large-v3" }], automatic_groq: "whisper-large-v3", automatic_gemini: null, automatic_whisper: "small" }, language_models: { ...settings.language_models!, models: [correction], correction } };
  const view = render(<PrivacyTab settings={nextSettings} providerStatus={cloud} />);
  expect(await within(screen.getByRole("region", { name: "Managed transcription" })).findByText("On · Connected")).toBeTruthy();
  expect(within(screen.getByRole("region", { name: "Groq" })).getByText("On · Key saved")).toBeTruthy();
  expect(within(screen.getByRole("region", { name: "Correction model" })).getByText("On · Anthropic · claude-test")).toBeTruthy();
  expect(within(screen.getByRole("region", { name: "Anthropic" })).getByText("On · Key saved")).toBeTruthy();
  view.rerender(<PrivacyTab settings={settings} providerStatus={status} />);
  expect(within(screen.getByRole("region", { name: "Groq" })).getByText("Off · No key")).toBeTruthy();
  expect(within(screen.getByRole("region", { name: "Correction model" })).getByText("Off")).toBeTruthy();
});

it("shows saved keys as off until an automatic Speech-to-text model or Use for default is valid", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  const keys = { ...status, has_groq_key: true, has_gemini_key: true, has_openai_key: true };
  const speech = { models: [{ provider: "groq" as const, model: "whisper-large-v3" }, { provider: "gemini" as const, model: "gemini-test" }], automatic_groq: "missing", automatic_gemini: null, automatic_whisper: "small" };
  const openai = { provider: "openai" as const, model: "gpt-test" };
  const view = render(<PrivacyTab settings={{ ...settings, speech, language_models: { ...settings.language_models!, summary: openai } }} providerStatus={keys} />);
  for (const name of ["Groq", "Gemini", "OpenAI"]) expect(within(screen.getByRole("region", { name })).getByText("Off · Key saved, not used")).toBeTruthy();
  view.rerender(<PrivacyTab settings={{ ...settings, speech: { ...speech, automatic_groq: "whisper-large-v3", automatic_gemini: "gemini-test" }, language_models: { ...settings.language_models!, models: [openai], summary: openai } }} providerStatus={keys} />);
  for (const name of ["Groq", "Gemini", "OpenAI"]) expect(within(screen.getByRole("region", { name })).getByText("On · Key saved")).toBeTruthy();
});

it("refreshes Managed status on each visit and uses the shared disclosure text", async () => {
  vi.mocked(managedStatus).mockResolvedValueOnce({ connected: false, revoked: false }).mockResolvedValueOnce({ connected: true, revoked: false });
  const view = render(<PrivacyTab settings={settings} providerStatus={status} />);
  const managed = screen.getByRole("region", { name: "Managed transcription" });
  expect(await within(managed).findByText("Off · Not connected")).toBeTruthy();
  fireEvent.click(within(managed).getByRole("button", { name: "Details" }));
  expect(within(managed).getByText(MANAGED_PRIVACY_STATEMENT)).toBeTruthy();
  const groq = screen.getByRole("region", { name: "Groq" });
  fireEvent.click(within(groq).getByRole("button", { name: "Details" }));
  expect(within(groq).getByText(PRIVACY_DESTINATIONS.groq.details)).toBeTruthy();
  view.unmount();
  render(<PrivacyTab settings={settings} providerStatus={status} />);
  await waitFor(() => expect(within(screen.getByRole("region", { name: "Managed transcription" })).getByText("On · Connected")).toBeTruthy());
  expect(managedStatus).toHaveBeenCalledTimes(2);
});
