import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import LibraryTable from "./LibraryTable";
import { meetingSummary, onSummaryStatus } from "./summary";
import type { MeetingSummary } from "./library";

vi.mock("./summary", () => ({ meetingSummary: vi.fn(), onSummaryStatus: vi.fn() }));

const meetings: MeetingSummary[] = [
  { id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 3_600_000 },
  { id: "two", title: "Review", started_at: "2026-10-13T09:00:00Z", duration_ms: 600_000 },
  { id: "three", title: "Budget", started_at: "2026-10-11T09:00:00Z", duration_ms: 1_800_000 },
];

const callbacks = { onOpen: vi.fn(), onRename: vi.fn(), onDelete: vi.fn(), onCancelProcessing: vi.fn() };

function renderTable(overrides: Partial<Parameters<typeof LibraryTable>[0]> = {}) {
  return render(<LibraryTable meetings={meetings} details={{}} jobs={[]} recordingMeetingId={null} {...callbacks} {...overrides} />);
}

function titles() {
  return within(screen.getByRole("table", { name: "Meetings in the Library" })).getAllByRole("row").slice(1).map((row) => within(row).getByRole("rowheader").textContent);
}
function row(title: string) { return screen.getByRole("button", { name: title }).closest("tr")!; }

beforeEach(() => {
  vi.mocked(meetingSummary).mockResolvedValue(null);
  vi.mocked(onSummaryStatus).mockResolvedValue(() => {});
});

afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("sorts Title, Date and Duration with keyboard operable headers and aria-sort", () => {
  renderTable();
  expect(titles()).toEqual(["Review", "Planning", "Budget"]);
  const title = screen.getByRole("columnheader", { name: "Title" });
  const date = screen.getByRole("columnheader", { name: "Date" });
  const duration = screen.getByRole("columnheader", { name: "Duration" });
  expect(date.getAttribute("aria-sort")).toBe("descending");
  fireEvent.click(within(title).getByRole("button"));
  expect(titles()).toEqual(["Budget", "Planning", "Review"]);
  expect(title.getAttribute("aria-sort")).toBe("ascending");
  fireEvent.click(within(title).getByRole("button"));
  expect(titles()).toEqual(["Review", "Planning", "Budget"]);
  expect(title.getAttribute("aria-sort")).toBe("descending");
  fireEvent.click(within(duration).getByRole("button"));
  expect(titles()).toEqual(["Review", "Budget", "Planning"]);
  expect(duration.getAttribute("aria-sort")).toBe("ascending");
  fireEvent.click(within(duration).getByRole("button"));
  expect(titles()).toEqual(["Planning", "Budget", "Review"]);
  fireEvent.click(within(date).getByRole("button"));
  expect(titles()).toEqual(["Budget", "Planning", "Review"]);
  expect(date.getAttribute("aria-sort")).toBe("ascending");
});

it("opens a Meeting through its Title button and retains mouse row clicks", () => {
  renderTable();
  const planning = row("Planning");
  expect(planning.hasAttribute("tabindex")).toBe(false);
  fireEvent.click(planning);
  fireEvent.click(screen.getByRole("button", { name: "Planning" }));
  expect(callbacks.onOpen).toHaveBeenNthCalledWith(1, "one");
  expect(callbacks.onOpen).toHaveBeenNthCalledWith(2, "one");
});

it("updates row data and shows processing, failure and completed Summary states", async () => {
  vi.mocked(meetingSummary).mockImplementation(async (id) => id === "three" ? { schema_version: 1, revision: 1, created_at: "2026-10-12T00:00:00Z", provider: "ollama", model: "test", overview: [], key_points: [], decisions: [], action_items: [], dropped_items: 0 } : null);
  const view = renderTable({ jobs: [{ meeting_id: "one", model: null, stage: "transcribing", progress: 0.42, error: null }] });
  expect(within(row("Planning")).getByText("Transcribing · 42%")).toBeTruthy();
  await waitFor(() => expect(within(row("Budget")).getByText("Summary made")).toBeTruthy());
  view.rerender(<LibraryTable meetings={meetings.slice(1)} details={{ two: { language: "vi", imported: false } }} jobs={[{ meeting_id: "two", model: null, stage: "failed", progress: 0, error: "network" }]} recordingMeetingId={null} {...callbacks} />);
  expect(screen.queryByRole("button", { name: "Planning" })).toBeNull();
  expect(within(row("Review")).getByText("Tiếng Việt")).toBeTruthy();
  expect(within(row("Review")).getByText("Failed")).toBeTruthy();
});

it("labels every Meeting language and auto-detect in the Library", () => {
  renderTable({ details: { one: { language: "es", imported: false }, two: { language: "ja", imported: false }, three: { language: null, imported: false } } });
  expect(within(row("Planning")).getByText("Spanish")).toBeTruthy();
  expect(within(row("Review")).getByText("Japanese")).toBeTruthy();
  expect(within(row("Budget")).getByText("Auto-detect")).toBeTruthy();
});

it("refreshes Summary made when a Summary status event arrives", async () => {
  vi.mocked(meetingSummary).mockResolvedValue(null);
  renderTable({ meetings: [meetings[0]] });
  await waitFor(() => expect(meetingSummary).toHaveBeenCalledWith("one"));
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "2026-10-12T00:00:00Z", provider: "ollama", model: "test", overview: [], key_points: [], decisions: [], action_items: [], dropped_items: 0 });
  await act(async () => vi.mocked(onSummaryStatus).mock.calls[0][0]({ jobs: [] }));
  expect(await screen.findByText("Summary made")).toBeTruthy();
});

it("reuses the Rename, Delete and Cancel processing actions", async () => {
  renderTable({ jobs: [{ meeting_id: "one", model: null, stage: "queued", progress: 0, error: null }] });
  fireEvent.pointerDown(screen.getByRole("button", { name: "Table options for Planning" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Cancel processing" }));
  expect(callbacks.onCancelProcessing).toHaveBeenCalledWith("one");
  expect(callbacks.onOpen).not.toHaveBeenCalled();
  fireEvent.pointerDown(screen.getByRole("button", { name: "Table options for Planning" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Rename Planning" }));
  expect(callbacks.onRename).toHaveBeenCalledWith(meetings[0]);
  fireEvent.pointerDown(screen.getByRole("button", { name: "Table options for Planning" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Delete Planning" }));
  expect(callbacks.onDelete).toHaveBeenCalledWith(meetings[0]);
});
