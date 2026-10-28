import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AskPanel from "./AskPanel";
import { askMeeting, askSendPlan, meetingAsks, onAskDelta, onAsksChanged, retryAsk, stopAsk } from "./ask";
import type { Transcript } from "./processing";
import type { LanguageModelSettings } from "./ai";
vi.mock("./ask", () => ({ askMeeting: vi.fn(), askSendPlan: vi.fn(), meetingAsks: vi.fn(), onAskDelta: vi.fn(() => Promise.resolve(() => {})), onAsksChanged: vi.fn(() => Promise.resolve(() => {})), retryAsk: vi.fn(), stopAsk: vi.fn(), answerLabel: (state: string) => ({ incomplete: "Incomplete", unverified: "Unverified", failed: "Failed", insufficient_evidence: "Insufficient evidence" })[state] ?? "" }));
const transcript: Transcript = { schema_version: 1, revision: 2, created_at: "now", model: ["small"], language: "en", language_detected: false, duration_ms: 30000, segments: [] };
const aiStatus = { provider: "ollama" as const, model: "llama", configured: true, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };
const empty = { asks: [], active: null, retryable_id: null };
const languageModels: LanguageModelSettings = { models: [{ provider: "ollama", model: "llama" }, { provider: "openai", model: "gpt-4o" }], correction: null, summary: { provider: "ollama", model: "llama" }, ask: { provider: "ollama", model: "llama" }, confirmed_correction_providers: [], legacy_correction_pending: false };
beforeEach(() => { vi.mocked(meetingAsks).mockResolvedValue(empty); vi.mocked(askSendPlan).mockResolvedValue({ provider: "ollama", cloud: false, consented: true, disclosure: null }); vi.mocked(askMeeting).mockResolvedValue(empty); vi.mocked(retryAsk).mockResolvedValue(empty); vi.mocked(stopAsk).mockResolvedValue(); });
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("shows cited history and offers retry only for the latest failed Ask", async () => {
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [
    { id: "a0001", question: "What budget?", asked_at: "now", revision: 1, provider: "ollama", model: "llama", state: "answered", answer: "$20", citations: [{ segment_id: "s00001", start_ms: 1000, end_ms: 2500 }], invalid_citations: [], error: null },
    { id: "a0002", question: "What date?", asked_at: "now", revision: 2, provider: "ollama", model: "llama", state: "unverified", answer: "Friday", citations: [], invalid_citations: ["s99999"], error: null },
    { id: "a0003", question: "Who?", asked_at: "now", revision: 2, provider: "ollama", model: "llama", state: "failed", answer: "", citations: [], invalid_citations: [], error: "Connection lost" },
  ], active: null, retryable_id: "a0003" });
  const player = { seek: vi.fn(), play: vi.fn(), pause: vi.fn(), playSpan: vi.fn() };
  render(<AskPanel meetingId="m1" transcript={transcript} player={player} aiStatus={aiStatus} />);
  expect(await screen.findByText("$20")).toBeTruthy();
  expect(screen.getByText("Asked about transcript revision 1")).toBeTruthy();
  expect(screen.getByText("Unverified")).toBeTruthy();
  expect(screen.getByText(/Some Citations could not be checked against this Meeting/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Play from 00:01: earlier Revision" }));
  expect(player.playSpan).toHaveBeenCalledWith(1000, 2500);
  expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(retryAsk).toHaveBeenCalledWith("m1", false, { provider: "ollama", model: "llama" }));
});
it("distinguishes answered, insufficient, incomplete, and off-topic Answers", async () => {
  const base = { asked_at: "now", revision: 2, provider: "ollama" as const, model: "llama", citations: [], invalid_citations: [], error: null };
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [
    { ...base, id: "a1", question: "What happened?", state: "answered", answer: "A decision was made." },
    { ...base, id: "a2", question: "What was the price?", state: "insufficient_evidence", answer: "The transcript of this Meeting does not cover this." },
    { ...base, id: "a3", question: "What else?", state: "incomplete", answer: "It began" },
    { ...base, id: "a4", question: "Tell me a joke", state: "off_topic", answer: "Ask something about this Meeting." },
  ], active: null, retryable_id: null });
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect(await screen.findByText("Answered")).toBeTruthy();
  expect(screen.getByText("The transcript of this Meeting does not cover this.")).toBeTruthy();
  expect(screen.getByText("The transcript of this Meeting does not cover this.").closest(".border-dashed")).toBeTruthy();
  expect(screen.getByText("Incomplete")).toBeTruthy();
  expect(screen.getByText("Off-topic")).toBeTruthy();
  expect(screen.getByText("Ask something about this Meeting.")).toBeTruthy();
  expect(screen.getByText("Ask something about this Meeting.").closest(".text-ink-muted")).toBeTruthy();
  expect(screen.getByText(/Answers come from this Meeting only, using Ollama on this computer/)).toBeTruthy();
});
it("shows streaming text and Stop", async () => {
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [], active: { ask_id: "a0001", partial: "The plan" }, retryable_id: null });
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect(await screen.findByText("The plan")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  expect(stopAsk).toHaveBeenCalledWith("m1");
  expect(onAskDelta).toHaveBeenCalled(); expect(onAsksChanged).toHaveBeenCalled();
  await act(async () => { vi.mocked(onAskDelta).mock.calls[0][0]({ meeting_id: "m1", ask_id: "a0001", text: "The plan is approved" }); });
  expect(screen.getByText("The plan is approved")).toBeTruthy();
});
it("renders numbered Answers with Citations after the final item", async () => {
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [{ id: "a1", question: "What next?", asked_at: "now", revision: 2, provider: "ollama", model: "llama", state: "answered", answer: "1. **Find information…** 2. **Clarify…**", citations: [{ segment_id: "s00001", start_ms: 1000, end_ms: 2500 }], invalid_citations: [], error: null }], active: null, retryable_id: null });
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  const list = (await screen.findByText("Find information…")).closest("ol");
  expect(list?.querySelectorAll("li")).toHaveLength(2);
  expect(list?.querySelectorAll("strong")).toHaveLength(2);
  expect(list?.querySelectorAll("li")[0].querySelector("button")).toBeNull();
  expect(list?.querySelectorAll("li")[1].querySelector("button")).toBeTruthy();
});
it("renders streaming Markdown after an unclosed bold marker is completed", async () => {
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [], active: { ask_id: "a1", partial: "The **decision" }, retryable_id: null });
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect(await screen.findByText("The **decision")).toBeTruthy();
  await act(async () => { vi.mocked(onAskDelta).mock.calls[0][0]({ meeting_id: "m1", ask_id: "a1", text: "The **decision**" }); });
  expect(screen.getByText("decision").tagName).toBe("STRONG");
  expect(screen.queryByText(/\*\*/)).toBeNull();
});
it("submits a local Ask without disclosure", async () => {
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "What was decided?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "What was decided?", false, { provider: "ollama", model: "llama" }));
});
it("shows the exact cloud disclosure before sending an Ask", async () => {
  vi.mocked(askSendPlan).mockResolvedValue({ provider: "anthropic", cloud: true, consented: false, disclosure: { provider: "anthropic", model: "claude", endpoint: "https://example.com", system: "Ask instructions", messages: [{ role: "user", content: "Meeting evidence" }], characters: 31 } });
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, provider: "anthropic", has_anthropic_key: true }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "When?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask Anthropic" }));
  expect(await screen.findByRole("dialog", { name: "Send to Anthropic?" })).toBeTruthy();
  expect(askMeeting).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "When?", true, { provider: "anthropic", model: "llama" }));
});
it("uses a chosen Language model for follow-up Asks without changing Settings", async () => {
  const settings = structuredClone(languageModels);
  const status = { ...aiStatus, has_openai_key: true };
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={status} languageModels={settings} />);
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Choose Language model" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "OpenAI · gpt-4o" }));
  expect(screen.getByRole("button", { name: "Choose Language model" }).textContent).toContain("OpenAI · gpt-4o");
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "First?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask OpenAI" }));
  await waitFor(() => expect(askSendPlan).toHaveBeenCalledWith("m1", "First?", { provider: "openai", model: "gpt-4o" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "First?", false, { provider: "openai", model: "gpt-4o" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "Follow-up?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask OpenAI" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "Follow-up?", false, { provider: "openai", model: "gpt-4o" }));
  expect(settings.ask).toEqual({ provider: "ollama", model: "llama" });
});
it("shows the chosen Provider and model in the cloud disclosure and saved Answer", async () => {
  vi.mocked(askSendPlan).mockResolvedValue({ provider: "openai", cloud: true, consented: false, disclosure: { provider: "openai", model: "gpt-4o", endpoint: "https://example.com", system: "Ask instructions", messages: [], characters: 16 } });
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [{ id: "a1", question: "What?", asked_at: "now", revision: 2, provider: "openai", model: "gpt-4o", state: "answered", answer: "Yes", citations: [], invalid_citations: [], error: null }], active: null, retryable_id: null });
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, has_openai_key: true }} languageModels={languageModels} />);
  expect(await screen.findByText("OpenAI · gpt-4o")).toBeTruthy();
  fireEvent.pointerDown(screen.getByRole("button", { name: "Choose Language model" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "OpenAI · gpt-4o" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "When?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask OpenAI" }));
  expect(await screen.findByRole("dialog", { name: "Send to OpenAI?" })).toBeTruthy();
  expect(screen.getByText("gpt-4o")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "When?", true, { provider: "openai", model: "gpt-4o" }));
});
it("shows setup guidance instead of the Ask input when no Language model is ready", async () => {
  const onOpenSettings = vi.fn();
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} languageModels={{ ...languageModels, models: [], ask: null }} onOpenSettings={onOpenSettings} />);
  expect(await screen.findByText(/No Language model is set up for Ask/)).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: "Question" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Set one up →" }));
  expect(onOpenSettings).toHaveBeenCalledOnce();
});
it("keeps Ask empty when its default is None until a model is chosen", async () => {
  const onOpenSettings = vi.fn();
  const settings = { ...languageModels, ask: null };
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} languageModels={settings} onOpenSettings={onOpenSettings} />);
  const picker = await screen.findByRole("button", { name: "Choose Language model" });
  expect(picker.textContent).toContain("Choose a model");
  expect(screen.getByRole("textbox", { name: "Question" }).hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("button", { name: "Ask" }).hasAttribute("disabled")).toBe(true);
  expect(screen.getByText(/No default Language model is set for Ask/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Language models in Settings →" }));
  expect(onOpenSettings).toHaveBeenCalledOnce();
  expect(askMeeting).not.toHaveBeenCalled();
  fireEvent.pointerDown(picker, { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Ollama · llama" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "What changed?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "What changed?", false, { provider: "ollama", model: "llama" }));
  expect(settings.ask).toBeNull();
});
