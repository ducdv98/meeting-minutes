import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SummaryPanel from "./SummaryPanel";
import { cancelSummary, dismissSummary, generateSummary, meetingSummary, onSummaryStatus, summaryParticipantsChanged, summarySendPlan, summaryStatus } from "./summary";
import type { Transcript } from "./processing";
import type { LanguageModelSettings } from "./ai";

vi.mock("./summary", () => ({ meetingSummary: vi.fn(), summaryStatus: vi.fn(), summaryParticipantsChanged: vi.fn(), summarySendPlan: vi.fn(), generateSummary: vi.fn(), cancelSummary: vi.fn(), dismissSummary: vi.fn(), onSummaryStatus: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("./recording", async (original) => ({ ...(await original<typeof import("./recording")>()), onLibraryChanged: vi.fn(() => Promise.resolve(() => {})) }));
const transcript: Transcript = { schema_version: 1, revision: 2, created_at: "now", model: ["small"], language: "en", language_detected: false, duration_ms: 30000, segments: [{ id: "s00001", start_ms: 1000, end_ms: 2500, text: "Budget", speaker: "me" }] };
const aiStatus = { provider: "ollama" as const, model: "llama", configured: true, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };
const citation = { segment_id: "s00001", start_ms: 1000, end_ms: 2500 };
const languageModels: LanguageModelSettings = { models: [{ provider: "ollama", model: "llama" }, { provider: "openai", model: "gpt-4o" }], correction: null, summary: { provider: "ollama", model: "llama" }, ask: null, confirmed_correction_providers: [], legacy_correction_pending: false };
beforeEach(() => { vi.mocked(meetingSummary).mockResolvedValue(null); vi.mocked(summaryParticipantsChanged).mockResolvedValue(false); vi.mocked(summaryStatus).mockResolvedValue({ jobs: [] }); vi.mocked(summarySendPlan).mockResolvedValue({ provider: "ollama", cloud: false, consented: true, disclosure: null }); vi.mocked(generateSummary).mockResolvedValue({ jobs: [] }); });

it("offers regeneration after Participants change without starting automatically", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "now", provider: "ollama", model: "llama", overview: [], key_points: [], decisions: [], action_items: [], dropped_items: 0, participants_fingerprint: "saved" });
  vi.mocked(summaryParticipantsChanged).mockResolvedValue(true);
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect(await screen.findByText("Participants changed: regenerate Summary?")).toBeTruthy();
  expect(generateSummary).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", false, { provider: "ollama", model: "llama" }));
});
it("shows the cloud disclosure before regenerating for changed Participants", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "now", provider: "ollama", model: "llama", overview: [], key_points: [], decisions: [], action_items: [], dropped_items: 0 });
  vi.mocked(summaryParticipantsChanged).mockResolvedValue(true);
  vi.mocked(summarySendPlan).mockResolvedValue({ provider: "openai", cloud: true, consented: false, disclosure: { provider: "openai", model: "gpt", endpoint: "https://example.com", system: "Summary instructions", messages: [{ role: "user", content: "Transcript" }], characters: 30 } });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, provider: "openai", has_openai_key: true }} />);
  fireEvent.click(await screen.findByRole("button", { name: "Regenerate" }));
  expect(await screen.findByRole("dialog", { name: "Send to OpenAI?" })).toBeTruthy();
  expect(generateSummary).not.toHaveBeenCalled();
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("shows cited Summary items and plays their spans", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", provider: "ollama", model: "llama", overview: [{ text: "Budget agreed", citations: [citation] }], key_points: [], decisions: [], action_items: [{ text: "Send report", owner: "Alex", due: "Friday", citations: [citation] }], dropped_items: 0 });
  const player = { seek: vi.fn(), play: vi.fn(), pause: vi.fn(), playSpan: vi.fn() };
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={player} aiStatus={aiStatus} />);
  expect(await screen.findByText("Budget agreed")).toBeTruthy();
  expect(screen.getByText("This Summary is from an earlier Transcript revision.")).toBeTruthy();
  expect(screen.getByText("Alex · Friday")).toBeTruthy();
  fireEvent.click(screen.getAllByRole("button", { name: /Play from 00:01/ })[0]);
  expect(player.playSpan).toHaveBeenCalledWith(1000, 2500);
});
it("renders Markdown in every Summary section and keeps Citations after the text", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "now", provider: "ollama", model: "llama", overview: [{ text: "# Overview text", citations: [citation] }], key_points: [{ text: "- **First**\n- Second", citations: [citation] }], decisions: [{ text: "Decision with `code`", citations: [citation] }], action_items: [{ text: "1. *Draft* 2. Send", owner: null, due: null, citations: [citation] }], dropped_items: 0 });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect((await screen.findByText("Overview text")).tagName).toBe("STRONG");
  expect(screen.getByText("First").tagName).toBe("STRONG");
  expect(screen.getByText("code").tagName).toBe("CODE");
  const actionList = screen.getByText("Draft").closest("ol");
  expect(actionList?.querySelectorAll("li")).toHaveLength(2);
  expect(actionList?.querySelectorAll("li")[1].querySelector("button")).toBeTruthy();
  expect(screen.getAllByRole("button", { name: /Play from 00:01/ })).toHaveLength(4);
});
it("generates locally without a disclosure", async () => {
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  fireEvent.click(await screen.findByRole("button", { name: "Generate with llama" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", false, { provider: "ollama", model: "llama" }));
});
it("requires Send after showing cloud disclosure", async () => {
  vi.mocked(summarySendPlan).mockResolvedValue({ provider: "openai", cloud: true, consented: false, disclosure: { provider: "openai", model: "gpt", endpoint: "https://example.com", system: "Summary instructions", messages: [{ role: "user", content: "Transcript" }], characters: 30 } });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, provider: "openai", has_openai_key: true }} />);
  fireEvent.click(await screen.findByRole("button", { name: "Generate with llama" }));
  expect(await screen.findByRole("dialog", { name: "Send to OpenAI?" })).toBeTruthy();
  expect(generateSummary).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(generateSummary).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Generate with llama" }));
  fireEvent.click(await screen.findByRole("button", { name: "Send" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", true, { provider: "openai", model: "llama" }));
});
it("shows progress and Stop for a running Summary", async () => {
  vi.mocked(summaryStatus).mockResolvedValue({ jobs: [{ meeting_id: "m1", state: "running", done: 1, total: 3, error: null }] });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect(await screen.findByText("Summarising… (1/3)")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  await waitFor(() => expect(cancelSummary).toHaveBeenCalledWith("m1"));
  expect(onSummaryStatus).toHaveBeenCalled();
});
it("shows the running Summary's progress label once, also after the loading delay", async () => {
  vi.mocked(summaryStatus).mockResolvedValue({ jobs: [{ meeting_id: "m1", state: "running", done: 1, total: 3, error: null }] });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  await screen.findByText("Summarising… (1/3)");
  await act(() => new Promise((resolve) => setTimeout(resolve, 1100)));
  expect(screen.getAllByText("Summarising… (1/3)")).toHaveLength(1);
});
it("shows a failed Summary and lets the user dismiss it", async () => {
  vi.mocked(summaryStatus).mockResolvedValue({ jobs: [{ meeting_id: "m1", state: "failed", done: 0, total: 1, error: "Ollama is unavailable" }] });
  vi.mocked(dismissSummary).mockResolvedValue({ jobs: [] });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect((await screen.findByRole("alert")).textContent).toContain("Ollama is unavailable");
  expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  await waitFor(() => expect(dismissSummary).toHaveBeenCalledWith("m1"));
});
it("generates with a chosen model for one run and then restores the Summary default", async () => {
  const settings = structuredClone(languageModels);
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, has_openai_key: true }} languageModels={settings} />);
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Choose Language model" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "OpenAI · gpt-4o" }));
  fireEvent.click(screen.getByRole("button", { name: "Generate with gpt-4o" }));
  await waitFor(() => expect(summarySendPlan).toHaveBeenCalledWith("m1", { provider: "openai", model: "gpt-4o" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", false, { provider: "openai", model: "gpt-4o" }));
  expect(await screen.findByRole("button", { name: "Generate with llama" })).toBeTruthy();
  expect(settings.summary).toEqual({ provider: "ollama", model: "llama" });
});
it("shows setup guidance when no Language model is ready", async () => {
  const onOpenSettings = vi.fn();
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} languageModels={{ ...languageModels, models: [], summary: null }} onOpenSettings={onOpenSettings} />);
  expect(await screen.findByText(/No Language model is set up for Summary/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: /Generate with/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Set one up →" }));
  expect(onOpenSettings).toHaveBeenCalledOnce();
});
it("keeps Generate disabled with no Summary default until a model is chosen", async () => {
  const onOpenSettings = vi.fn();
  const settings = { ...languageModels, summary: null };
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} languageModels={settings} onOpenSettings={onOpenSettings} />);
  const generate = await screen.findByRole("button", { name: "Generate with a model" });
  expect(generate.hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("button", { name: "Choose Language model" }).textContent).toContain("Choose a model");
  expect(screen.getByText(/No default Language model is set for Summary/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Language models in Settings →" }));
  expect(onOpenSettings).toHaveBeenCalledOnce();
  expect(generateSummary).not.toHaveBeenCalled();
  fireEvent.pointerDown(screen.getByRole("button", { name: "Choose Language model" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Ollama · llama" }));
  fireEvent.click(screen.getByRole("button", { name: "Generate with llama" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", false, { provider: "ollama", model: "llama" }));
  expect(settings.summary).toBeNull();
});
it("keeps Redo disabled with no Summary default", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "now", provider: "ollama", model: "llama", overview: [], key_points: [], decisions: [], action_items: [], dropped_items: 0 });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} languageModels={{ ...languageModels, summary: null }} />);
  expect((await screen.findByRole("button", { name: "Redo with a model" })).hasAttribute("disabled")).toBe(true);
});
