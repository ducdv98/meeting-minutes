import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SummaryPanel from "./SummaryPanel";
import { cancelSummary, dismissSummary, generateSummary, meetingSummary, onSummaryStatus, summaryParticipantsChanged, summarySendPlan, summaryStatus } from "./summary";
import type { Transcript } from "./processing";

vi.mock("./summary", () => ({ meetingSummary: vi.fn(), summaryStatus: vi.fn(), summaryParticipantsChanged: vi.fn(), summarySendPlan: vi.fn(), generateSummary: vi.fn(), cancelSummary: vi.fn(), dismissSummary: vi.fn(), onSummaryStatus: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("./recording", async (original) => ({ ...(await original<typeof import("./recording")>()), onLibraryChanged: vi.fn(() => Promise.resolve(() => {})) }));
const transcript: Transcript = { schema_version: 1, revision: 2, created_at: "now", model: "small", language: "en", language_detected: false, duration_ms: 30000, segments: [{ id: "s00001", start_ms: 1000, end_ms: 2500, text: "Budget", speaker: "me" }] };
const aiStatus = { provider: "ollama" as const, model: "llama", configured: true, has_openai_key: false, has_anthropic_key: false };
const citation = { segment_id: "s00001", start_ms: 1000, end_ms: 2500 };
beforeEach(() => { vi.mocked(meetingSummary).mockResolvedValue(null); vi.mocked(summaryParticipantsChanged).mockResolvedValue(false); vi.mocked(summaryStatus).mockResolvedValue({ jobs: [] }); vi.mocked(summarySendPlan).mockResolvedValue({ provider: "ollama", cloud: false, consented: true, disclosure: null }); vi.mocked(generateSummary).mockResolvedValue({ jobs: [] }); });

it("offers regeneration after Participants change without starting automatically", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "now", provider: "ollama", model: "llama", overview: [], key_points: [], decisions: [], action_items: [], dropped_items: 0, participants_fingerprint: "saved" });
  vi.mocked(summaryParticipantsChanged).mockResolvedValue(true);
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect(await screen.findByText("Participants changed: regenerate Summary?")).toBeTruthy();
  expect(generateSummary).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Regenerate" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", false));
});
it("shows the cloud disclosure before regenerating for changed Participants", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "now", provider: "ollama", model: "llama", overview: [], key_points: [], decisions: [], action_items: [], dropped_items: 0 });
  vi.mocked(summaryParticipantsChanged).mockResolvedValue(true);
  vi.mocked(summarySendPlan).mockResolvedValue({ provider: "openai", cloud: true, consented: false, disclosure: { provider: "openai", model: "gpt", endpoint: "https://example.com", system: "Summary instructions", messages: [{ role: "user", content: "Transcript" }], characters: 30 } });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, provider: "openai" }} />);
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
it("generates locally without a disclosure", async () => {
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  fireEvent.click(await screen.findByRole("button", { name: "Generate Summary" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", false));
});
it("requires Send after showing cloud disclosure", async () => {
  vi.mocked(summarySendPlan).mockResolvedValue({ provider: "openai", cloud: true, consented: false, disclosure: { provider: "openai", model: "gpt", endpoint: "https://example.com", system: "Summary instructions", messages: [{ role: "user", content: "Transcript" }], characters: 30 } });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, provider: "openai" }} />);
  fireEvent.click(await screen.findByRole("button", { name: "Generate Summary with OpenAI" }));
  expect(await screen.findByRole("dialog", { name: "Send to OpenAI?" })).toBeTruthy();
  expect(generateSummary).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(generateSummary).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Generate Summary with OpenAI" }));
  fireEvent.click(await screen.findByRole("button", { name: "Send" }));
  await waitFor(() => expect(generateSummary).toHaveBeenCalledWith("m1", true));
});
it("shows progress and Stop for a running Summary", async () => {
  vi.mocked(summaryStatus).mockResolvedValue({ jobs: [{ meeting_id: "m1", state: "running", done: 1, total: 3, error: null }] });
  render(<SummaryPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  expect(await screen.findByText("Summarising… (1/3)")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  await waitFor(() => expect(cancelSummary).toHaveBeenCalledWith("m1"));
  expect(onSummaryStatus).toHaveBeenCalled();
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
