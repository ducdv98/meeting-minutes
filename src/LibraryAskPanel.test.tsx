import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import LibraryAskPanel from "./LibraryAskPanel";
import { libraryAskPlan, libraryAskStart, libraryAsks, onLibraryAskDelta, onLibraryAsksChanged, stopLibraryAsk, type LibraryAskEntry } from "./libraryAsk";
import type { AiStatus, LanguageModelSettings } from "./ai";

vi.mock("./libraryAsk", () => ({ libraryAskPlan: vi.fn(), libraryAskStart: vi.fn(), libraryAsks: vi.fn(), onLibraryAskDelta: vi.fn(() => Promise.resolve(() => {})), onLibraryAsksChanged: vi.fn(() => Promise.resolve(() => {})), stopLibraryAsk: vi.fn() }));
const models: LanguageModelSettings = { models: [{ provider: "ollama", model: "llama" }, { provider: "openai", model: "gpt-4o" }], correction: null, summary: null, ask: { provider: "ollama", model: "llama" }, confirmed_correction_providers: [], legacy_correction_pending: false };
const status: AiStatus = { provider: "ollama", model: "llama", configured: true, has_openai_key: true, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false };
const meetings = [{ id: "m1", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 10000 }, { id: "m2", title: "Review", started_at: "2026-10-13T09:00:00Z", duration_ms: 10000 }];
const empty = { asks: [], active: null };
const localPlan = { plan_id: "p1", provider: "ollama" as const, cloud: false, meetings: [{ meeting_id: "m1", title: "Planning", started_at: meetings[0].started_at, excerpts: 2 }] };
const cloudPlan = { ...localPlan, provider: "openai" as const, cloud: true, meetings: [...localPlan.meetings, { meeting_id: "m2", title: "Review", started_at: meetings[1].started_at, excerpts: 3 }] };
const props = { aiStatus: status, languageModels: models, onOpenCitation: vi.fn(), onOpenSettings: vi.fn() };

beforeEach(() => { vi.mocked(libraryAsks).mockResolvedValue(empty); vi.mocked(libraryAskPlan).mockResolvedValue(localPlan); vi.mocked(libraryAskStart).mockResolvedValue(empty); vi.mocked(stopLibraryAsk).mockResolvedValue(); });
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("opens with the search question without planning until submitted", async () => {
  render(<LibraryAskPanel {...props} initialQuestion="What was decided?" />);
  expect(screen.getByRole("textbox", { name: "Question" })).toHaveProperty("value", "What was decided?");
  await screen.findByRole("button", { name: "Library Ask" });
  expect(libraryAskPlan).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Library Ask" }));
  await waitFor(() => expect(libraryAskPlan).toHaveBeenCalledWith("What was decided?", "all", { provider: "ollama", model: "llama" }));
  await waitFor(() => expect(libraryAskStart).toHaveBeenCalledWith("p1", [], false));
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("passes the selected date range to planning", async () => {
  render(<LibraryAskPanel {...props} />);
  fireEvent.click(await screen.findByRole("button", { name: "Last 30 days" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "What changed?" } });
  fireEvent.click(screen.getByRole("button", { name: "Library Ask" }));
  await waitFor(() => expect(libraryAskPlan).toHaveBeenCalledWith("What changed?", "last_30_days", { provider: "ollama", model: "llama" }));
});

it("uses another Language model for one Library Ask and shows planning progress", async () => {
  let finishPlan!: (plan: typeof cloudPlan) => void;
  vi.mocked(libraryAskPlan).mockImplementation(() => new Promise((resolve) => { finishPlan = resolve; }));
  render(<LibraryAskPanel {...props} />);
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Choose Language model" }), { button: 0, ctrlKey: false });
  fireEvent.click(screen.getByRole("menuitem", { name: "OpenAI · gpt-4o" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Question" }), { target: { value: "What changed?" } });
  fireEvent.click(screen.getByRole("button", { name: "Library Ask" }));
  expect(screen.getByText("Finding relevant Meetings…")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  expect(stopLibraryAsk).toHaveBeenCalled();
  expect(libraryAskPlan).toHaveBeenCalledWith("What changed?", "all", { provider: "openai", model: "gpt-4o" });
  await act(async () => finishPlan(cloudPlan));
});

it("silently ends a cancelled plan", async () => {
  vi.mocked(libraryAskPlan).mockRejectedValue("Cancelled");
  render(<LibraryAskPanel {...props} initialQuestion="What changed?" />);
  fireEvent.click(screen.getByRole("button", { name: "Library Ask" }));
  await waitFor(() => expect(libraryAskPlan).toHaveBeenCalled());
  await waitFor(() => expect(screen.queryByText("Finding relevant Meetings…")).toBeNull());
  expect(screen.queryByRole("alert")).toBeNull();
});

it("shows cloud Meetings and excludes unticked Meetings only after Send", async () => {
  vi.mocked(libraryAskPlan).mockResolvedValue(cloudPlan);
  render(<LibraryAskPanel {...props} initialQuestion="What was decided?" />);
  fireEvent.click(screen.getByRole("button", { name: "Library Ask" }));
  const dialog = await screen.findByRole("dialog", { name: "Send to OpenAI?" });
  expect(dialog.textContent).toContain("Your question was sent to OpenAI to find relevant Meetings. Excerpts from the ticked Meetings will be sent next.");
  expect(dialog.textContent).toContain("Planning");
  expect(dialog.textContent).toContain("2 excerpts");
  expect(dialog.textContent).toContain("Review");
  expect(dialog.textContent).toContain("3 excerpts");
  expect(libraryAskStart).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("checkbox", { name: /Review/ }));
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(libraryAskStart).toHaveBeenCalledWith("p1", ["m2"], true));
});

it("disables Send when all Meetings are unticked, and Cancel starts no Answer", async () => {
  vi.mocked(libraryAskPlan).mockResolvedValue(cloudPlan);
  render(<LibraryAskPanel {...props} initialQuestion="What was decided?" />);
  fireEvent.click(screen.getByRole("button", { name: "Library Ask" }));
  await screen.findByRole("dialog");
  fireEvent.click(screen.getByRole("checkbox", { name: /Planning/ }));
  fireEvent.click(screen.getByRole("checkbox", { name: /Review/ }));
  expect(screen.getByRole("button", { name: "Send" }).hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(libraryAskStart).not.toHaveBeenCalled();
});

it("starts a zero Meeting plan without a cloud dialog", async () => {
  vi.mocked(libraryAskPlan).mockResolvedValue({ ...cloudPlan, meetings: [] });
  render(<LibraryAskPanel {...props} initialQuestion="Unknown?" />);
  fireEvent.click(screen.getByRole("button", { name: "Library Ask" }));
  await waitFor(() => expect(libraryAskStart).toHaveBeenCalledWith("p1", [], false));
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("streams deltas and stops the active Library Ask", async () => {
  vi.mocked(libraryAsks).mockResolvedValue({ asks: [], active: { ask_id: "la0001", partial: "First" } });
  render(<LibraryAskPanel {...props} />);
  expect(await screen.findByText("First")).toBeTruthy();
  await act(async () => { vi.mocked(onLibraryAskDelta).mock.calls[0][0]({ ask_id: "la0001", text: "First answer" }); });
  expect(screen.getByText("First answer")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  expect(stopLibraryAsk).toHaveBeenCalled();
});

it("shows recent history, Answer states, Citations, and Meeting navigation", async () => {
  const base: Omit<LibraryAskEntry, "id" | "question" | "state" | "answer" | "citations"> = { range: "all", asked_at: "2026-10-12T09:00:00Z", provider: "ollama", model: "llama", invalid_citations: [], error: null, meetings_read: ["m1"] };
  vi.mocked(libraryAsks).mockResolvedValue({ asks: [
    { ...base, id: "la0001", question: "Old question", state: "answered", answer: "An older Answer", citations: [{ meeting_id: "m1", title: "Original Planning", started_at: "2026-10-11T09:00:00Z", segment_id: "s1", revision: 2, start_ms: 1000, end_ms: 2000 }] },
    { ...base, id: "la0002", question: "New question", state: "insufficient_evidence", answer: "Your Library doesn't cover this.", citations: [] },
    { ...base, id: "la0003", question: "Third question", state: "unverified", answer: "Maybe", citations: [] },
    { ...base, id: "la0004", question: "Fourth question", state: "incomplete", answer: "Partial", citations: [] },
  ], active: null });
  const onOpenCitation = vi.fn();
  render(<LibraryAskPanel {...props} onOpenCitation={onOpenCitation} />);
  expect(await screen.findByText("Fourth question")).toBeTruthy();
  const items = screen.getAllByRole("listitem");
  expect(items[0].textContent).toContain("Fourth question");
  expect(screen.getByText("Insufficient evidence")).toBeTruthy();
  expect(screen.getByText("Unverified")).toBeTruthy();
  expect(screen.getByText("Incomplete")).toBeTruthy();
  expect(screen.getAllByText("Read 1 Meeting").length).toBeGreaterThan(0);
  fireEvent.click(screen.getByRole("button", { name: /Original Planning.*Segment/ }));
  expect(onOpenCitation).toHaveBeenCalledWith({ meeting_id: "m1", title: "Original Planning", started_at: "2026-10-11T09:00:00Z", segment_id: "s1", revision: 2, start_ms: 1000, end_ms: 2000 });
});

it("shows Library Ask setup guidance when no Language model is available", async () => {
  render(<LibraryAskPanel {...props} aiStatus={null} languageModels={{ ...models, models: [], ask: null }} />);
  expect(await screen.findByText(/No Language model is set up for Library Ask/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Set one up →" }));
  expect(props.onOpenSettings).toHaveBeenCalled();
});
