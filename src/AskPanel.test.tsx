import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AskPanel from "./AskPanel";
import { askMeeting, askSendPlan, meetingAsks, onAskDelta, onAsksChanged, retryAsk, stopAsk } from "./ask";
import type { Transcript } from "./processing";
vi.mock("./ask", () => ({ askMeeting: vi.fn(), askSendPlan: vi.fn(), meetingAsks: vi.fn(), onAskDelta: vi.fn(() => Promise.resolve(() => {})), onAsksChanged: vi.fn(() => Promise.resolve(() => {})), retryAsk: vi.fn(), stopAsk: vi.fn(), answerLabel: (state: string) => ({ incomplete: "Incomplete", unverified: "Unverified", failed: "Failed", insufficient_evidence: "Insufficient evidence" })[state] ?? "" }));
const transcript: Transcript = { schema_version: 1, revision: 2, created_at: "now", model: "small", language: "en", language_detected: false, duration_ms: 30000, segments: [] };
const aiStatus = { provider: "ollama" as const, model: "llama", configured: true, has_openai_key: false, has_anthropic_key: false };
const empty = { asks: [], active: null, retryable_id: null };
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
  expect(screen.getByText("Some Citations could not be checked against this Meeting.")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Play from 00:01" }));
  expect(player.playSpan).toHaveBeenCalledWith(1000, 2500);
  expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(retryAsk).toHaveBeenCalledWith("m1", false));
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
it("submits a local Ask without disclosure", async () => {
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={aiStatus} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "What was decided?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "What was decided?", false));
});
it("shows the exact cloud disclosure before sending an Ask", async () => {
  vi.mocked(askSendPlan).mockResolvedValue({ provider: "anthropic", cloud: true, consented: false, disclosure: { provider: "anthropic", model: "claude", endpoint: "https://example.com", system: "Ask instructions", messages: [{ role: "user", content: "Meeting evidence" }], characters: 31 } });
  render(<AskPanel meetingId="m1" transcript={transcript} player={null} aiStatus={{ ...aiStatus, provider: "anthropic" }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "When?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask Anthropic" }));
  expect(await screen.findByRole("dialog", { name: "Send to Anthropic?" })).toBeTruthy();
  expect(askMeeting).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(askMeeting).toHaveBeenCalledWith("m1", "When?", true));
});
