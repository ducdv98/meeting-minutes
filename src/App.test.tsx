import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "./App";
import { pickAudioFiles, importStatus, dismissImportErrors, onImportStatus, onImportHover } from "./importing";
import { deleteMeeting, meetingDetail, renameMeeting, rescanLibrary, searchLibrary } from "./library";
import { cancelProcessing, onProcessingStatus, processingStatus, type ProcessingStatus } from "./processing";
import { onRecordingStatus, recordingStatus } from "./recording";

vi.mock("./importing", () => ({ pickAudioFiles: vi.fn(), importStatus: vi.fn(), dismissImportErrors: vi.fn(), onImportStatus: vi.fn(), onImportHover: vi.fn() }));
vi.mock("./library", async (original) => ({ ...(await original<typeof import("./library")>()), rescanLibrary: vi.fn(), searchLibrary: vi.fn(), deleteMeeting: vi.fn(), renameMeeting: vi.fn(), meetingDetail: vi.fn() }));
vi.mock("./RecordingBar", () => ({ default: () => null }));
vi.mock("./SettingsView", () => ({ default: () => <section><h1>Settings pane</h1></section> }));
vi.mock("./HelpView", () => ({ default: () => <section><h1>Help pane</h1></section> }));
vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), processingStatus: vi.fn(), onProcessingStatus: vi.fn(), cancelProcessing: vi.fn() }));
vi.mock("./MeetingView", () => ({ default: ({ id, openAt, onBack }: { id: string; openAt?: { segment_id: string }; onBack: () => void }) => (
  <section><h1>Meeting {id}</h1>{openAt && <p>Segment {openAt.segment_id}</p>}<button type="button" onClick={onBack}>Back to Library</button></section>
) }));
vi.mock("./recording", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./recording")>()),
  onLibraryChanged: () => Promise.resolve(() => {}),
  onRecordingStatus: vi.fn(), recordingStatus: vi.fn(),
}));

function openRowMenu(title: string) {
  fireEvent.pointerDown(screen.getByRole("button", { name: `More options for ${title}` }), { button: 0, ctrlKey: false });
}

function openMoreMenu() {
  fireEvent.pointerDown(screen.getByRole("button", { name: "More" }), { button: 0, ctrlKey: false });
}

beforeEach(() => {
  vi.mocked(importStatus).mockResolvedValue({ current: null, queued: 0, errors: [] });
  vi.mocked(onImportStatus).mockResolvedValue(() => {});
  vi.mocked(onImportHover).mockResolvedValue(() => {});
  vi.mocked(dismissImportErrors).mockResolvedValue({ current: null, queued: 0, errors: [] });
  vi.mocked(pickAudioFiles).mockResolvedValue({ current: null, queued: 0, errors: [] });
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [] });
  vi.mocked(onProcessingStatus).mockResolvedValue(() => {});
  vi.mocked(cancelProcessing).mockResolvedValue({ jobs: [] });
  vi.mocked(recordingStatus).mockResolvedValue({ phase: "idle", meeting_id: null, elapsed_ms: 0, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] });
  vi.mocked(onRecordingStatus).mockResolvedValue(() => {});
  vi.mocked(searchLibrary).mockResolvedValue([]);
  vi.mocked(deleteMeeting).mockResolvedValue();
  vi.mocked(meetingDetail).mockImplementation(async (id) => ({ id, title: id, started_at: "2026-10-12T09:00:00Z", duration_ms: 0, language: null, audio_path: null, markers: [], imported: false }));
});

it("opens a Meeting from a search result at its Segment", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [], malformed: [] });
  vi.mocked(searchLibrary).mockResolvedValue([{ meeting_id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", segment_id: "s00001", revision: 1, start_ms: 1000, end_ms: 2000, snippet: "budget" }]);
  render(<App />);
  const input = screen.getByRole("searchbox", { name: "Search transcripts" });
  fireEvent.change(input, { target: { value: "budget" } });
  fireEvent.keyDown(input, { key: "Enter" });
  fireEvent.click(await screen.findByRole("button", { name: /Planning/ }));
  expect(screen.getByRole("heading", { name: "Meeting one" })).toBeTruthy();
  expect(screen.getByText("Segment s00001")).toBeTruthy();
});

it("confirms list deletion, allows Cancel, and shows backend errors", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }], malformed: [] });
  render(<App />);
  await screen.findByRole("button", { name: "Planning" });
  openRowMenu("Planning");
  fireEvent.click(await screen.findByRole("menuitem", { name: "Delete Planning" }));
  expect(screen.getByRole("dialog", { name: "Delete Planning?" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(deleteMeeting).not.toHaveBeenCalled();
  vi.mocked(deleteMeeting).mockRejectedValueOnce("Folder is busy").mockResolvedValueOnce();
  openRowMenu("Planning");
  fireEvent.click(await screen.findByRole("menuitem", { name: "Delete Planning" }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Folder is busy");
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));
  await waitFor(() => expect(deleteMeeting).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});

it("disables Delete for the Meeting being recorded", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }], malformed: [] });
  vi.mocked(recordingStatus).mockResolvedValue({ phase: "recording", meeting_id: "one", elapsed_ms: 1000, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] });
  render(<App />);
  await screen.findByRole("button", { name: "Planning" });
  openRowMenu("Planning");
  const item = await screen.findByRole("menuitem", { name: "Delete Planning" });
  expect(item.getAttribute("data-disabled")).toBe("");
});

it("renames a Meeting inline from its row menu while keeping the selected Meeting open", async () => {
  const planning = { id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 };
  const review = { ...planning, title: "Review" };
  vi.mocked(rescanLibrary).mockResolvedValueOnce({ meetings: [planning], malformed: [] }).mockResolvedValue({ meetings: [review], malformed: [] });
  vi.mocked(renameMeeting).mockResolvedValue(review);
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Planning" }));
  expect(screen.getByRole("heading", { name: "Meeting one" })).toBeTruthy();
  openRowMenu("Planning");
  fireEvent.click(await screen.findByRole("menuitem", { name: "Rename Planning" }));
  const field = screen.getByRole("textbox", { name: "Title for Planning" }) as HTMLInputElement;
  expect(field.value).toBe("Planning");
  fireEvent.change(field, { target: { value: " Review " } });
  fireEvent.keyDown(field, { key: "Enter" });
  await waitFor(() => expect(renameMeeting).toHaveBeenCalledWith("one", "Review"));
  expect(await screen.findByRole("heading", { name: "Meeting one" })).toBeTruthy();
  expect(await screen.findByRole("button", { name: "Review" })).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: /Title for/ })).toBeNull();
});

it("keeps the inline rename open on failure and cancels with Escape", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }], malformed: [] });
  vi.mocked(renameMeeting).mockRejectedValue("Could not save Title");
  render(<App />);
  await screen.findByRole("button", { name: "Planning" });
  openRowMenu("Planning");
  fireEvent.click(await screen.findByRole("menuitem", { name: "Rename Planning" }));
  const field = screen.getByRole("textbox", { name: "Title for Planning" });
  fireEvent.change(field, { target: { value: "Review" } });
  fireEvent.keyDown(field, { key: "Enter" });
  expect((await screen.findByRole("alert")).textContent).toContain("Could not save Title");
  expect(screen.getByRole("textbox", { name: "Title for Planning" })).toBeTruthy();
  fireEvent.keyDown(screen.getByRole("textbox", { name: "Title for Planning" }), { key: "Escape" });
  expect(screen.queryByRole("textbox", { name: "Title for Planning" })).toBeNull();
  expect(screen.getByRole("button", { name: "Planning" })).toBeTruthy();
});

it("allows Rename for the Meeting being recorded", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }], malformed: [] });
  vi.mocked(recordingStatus).mockResolvedValue({ phase: "recording", meeting_id: "one", elapsed_ms: 1000, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] });
  render(<App />);
  await screen.findByRole("button", { name: "Planning" });
  openRowMenu("Planning");
  const item = await screen.findByRole("menuitem", { name: "Rename Planning" });
  expect(item.getAttribute("data-disabled")).toBeNull();
});

it("edits the Title from the live Recording screen", async () => {
  const planning = { id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 };
  const review = { ...planning, title: "Review" };
  vi.mocked(rescanLibrary).mockResolvedValueOnce({ meetings: [planning], malformed: [] }).mockResolvedValue({ meetings: [review], malformed: [] });
  vi.mocked(recordingStatus).mockResolvedValue({ phase: "recording", meeting_id: "one", elapsed_ms: 1000, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] });
  vi.mocked(renameMeeting).mockResolvedValue(review);
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Live Meeting" }));
  fireEvent.click(screen.getByRole("button", { name: "Rename Planning" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Review" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(renameMeeting).toHaveBeenCalledWith("one", "Review"));
  expect(await screen.findByRole("button", { name: "Rename Review" })).toBeTruthy();
});

it("shows a red Recording indicator and opens the Recording card from the sidebar", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }], malformed: [] });
  vi.mocked(recordingStatus).mockResolvedValue({ phase: "recording", meeting_id: "one", elapsed_ms: 1000, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] });
  render(<App />);

  const live = await screen.findByRole("button", { name: "Live Meeting" });
  expect(screen.getByText("Recording").className).toContain("text-record");
  expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Planning" }));
  fireEvent.click(live);
  expect(screen.getByRole("region", { name: "Live Recording" })).toBeTruthy();
  expect(screen.getByText("Recording Planning")).toBeTruthy();
  expect(screen.getByText(/The Transcript appears after you stop/)).toBeTruthy();
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("shows each Meeting's live processing state and cancels its job", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [
    { id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 },
    { id: "two", title: "Review", started_at: "2026-10-12T10:00:00Z", duration_ms: 0 },
  ], malformed: [] });
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [
    { meeting_id: "one", model: null, stage: "queued", progress: 0, error: null },
    { meeting_id: "two", model: null, stage: "downloading", progress: 0.34, error: null },
  ] });
  render(<App />);
  expect(await screen.findByText("Waiting to transcribe")).toBeTruthy();
  expect(screen.getByText("Downloading model · 34%")).toBeTruthy();
  const handler = vi.mocked(onProcessingStatus).mock.calls[0][0];
  const live: ProcessingStatus = { jobs: [
    { meeting_id: "one", model: null, stage: "transcribing", progress: 0.61, error: null },
    { meeting_id: "two", model: null, stage: "failed", progress: 0.34, error: "network" },
  ] };
  await act(async () => handler(live));
  expect(screen.getByText("Transcribing · 61%")).toBeTruthy();
  expect(screen.getByText("Transcription failed")).toBeTruthy();
  openRowMenu("Planning");
  fireEvent.click(await screen.findByRole("menuitem", { name: "Cancel processing" }));
  await waitFor(() => expect(cancelProcessing).toHaveBeenCalledWith("one"));
});

it("shows an empty Library when there are no Meetings", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [], malformed: [] });

  render(<App />);

  expect(await screen.findByRole("heading", { name: "Your Library is empty" })).toBeTruthy();
  expect(screen.getByText(/Documents\\Meeting Minutes/)).toBeTruthy();
});

it("shows the Meeting Minutes wordmark", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [], malformed: [] });

  render(<App />);

  expect(screen.getByRole("img", { name: "Meeting Minutes" })).toBeTruthy();
  await screen.findByRole("heading", { name: "Your Library is empty" });
});

it("lists Meeting titles in the Library", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [
    { id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 },
    { id: "two", title: "Review", started_at: "2026-10-12T10:00:00Z", duration_ms: 1_830_000 },
  ], malformed: [] });

  render(<App />);

  expect(await screen.findByText("Planning")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Review" }).textContent).toContain("30 min");
  expect(screen.getByRole("button", { name: "Planning" }).querySelector("time")?.getAttribute("datetime")).toBe("2026-10-12T09:00:00Z");
});

it("shows the Meeting language with its duration", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 1_830_000 }], malformed: [] });
  vi.mocked(meetingDetail).mockResolvedValue({ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 1_830_000, language: "en", audio_path: null, markers: [], imported: false });
  render(<App />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Planning" }).textContent).toContain("30 min · English"));
});

it("shows an error when the Library cannot be loaded", async () => {
  vi.mocked(rescanLibrary).mockRejectedValue(new Error("boom"));

  render(<App />);

  expect(await screen.findByRole("alert")).toBeTruthy();
});

it("reports malformed folders while listing valid Meetings", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({
    meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }],
    malformed: [{ folder: "Broken folder", reason: "missing meeting.json" }],
  });
  render(<App />);
  expect(await screen.findByText("Planning")).toBeTruthy();
  expect(screen.getByRole("status").textContent).toContain("1 folder could not be read");
  expect(screen.getByRole("status").textContent).toContain("Broken folder: missing meeting.json");
});

it("rescans the Library when the Rescan button is clicked", async () => {
  vi.mocked(rescanLibrary)
    .mockResolvedValueOnce({ meetings: [], malformed: [] })
    .mockResolvedValueOnce({ meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }], malformed: [] });
  render(<App />);
  await screen.findByRole("heading", { name: "Your Library is empty" });
  openMoreMenu();
  fireEvent.click(await screen.findByRole("menuitem", { name: "Rescan Library" }));
  expect(await screen.findByText("Planning")).toBeTruthy();
  expect(rescanLibrary).toHaveBeenCalledTimes(2);
});

it("opens a Meeting from its title and returns to the Library", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({
    meetings: [{ id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 }],
    malformed: [],
  });
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Planning" }));
  expect(screen.getByRole("heading", { name: "Meeting one" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Back to Library" }));
  expect(screen.getByRole("heading", { name: "Library" })).toBeTruthy();
});

it("picks audio and shows import queue, errors, and drop hint", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [], malformed: [] });
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Import an audio file" }));
  await waitFor(() => expect(pickAudioFiles).toHaveBeenCalled());
  await act(async () => vi.mocked(onImportStatus).mock.calls[0][0]({ current: "Team.wav", queued: 2, errors: ["bad.txt: not an mp3, m4a, wav or flac file"] }));
  expect(screen.getByText("Importing Team.wav… (+2 queued)")).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toContain("bad.txt");
  fireEvent.click(screen.getByRole("button", { name: "Dismiss import errors" }));
  await waitFor(() => expect(dismissImportErrors).toHaveBeenCalled());
  await act(async () => vi.mocked(onImportHover).mock.calls[0][0](true));
  expect(screen.getByText("Drop audio to import (mp3, m4a, wav, flac)")).toBeTruthy();
});


it("focuses sidebar search with Ctrl K and shows results in the main pane", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [], malformed: [] });
  vi.mocked(searchLibrary).mockResolvedValue([{ meeting_id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", segment_id: "s00001", revision: 1, start_ms: 1000, end_ms: 2000, snippet: "The \u0002budget\u0003" }]);
  render(<App />);
  fireEvent.keyDown(window, { key: "k", ctrlKey: true });
  const sidebarSearch = screen.getByRole("searchbox", { name: "Search transcripts" });
  expect(document.activeElement).toBe(sidebarSearch);
  fireEvent.change(sidebarSearch, { target: { value: "budget" } });
  expect(screen.getByRole("searchbox", { name: "Search the Library" })).toBeTruthy();
  expect(await screen.findByRole("mark")).toBeTruthy();
  expect(screen.getByRole("list", { name: "Search results" }).textContent).toContain("Planning");
});

it("groups Meetings, marks the selected row, and opens Settings and Help in the main pane", async () => {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 9).toISOString();
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1, 9).toISOString();
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [
    { id: "one", title: "Today meeting", started_at: today, duration_ms: 60000 },
    { id: "two", title: "Yesterday meeting", started_at: yesterday, duration_ms: 60000 },
  ], malformed: [] });
  render(<App />);
  await screen.findByRole("button", { name: "Today meeting" });
  expect(screen.getByRole("heading", { name: "Today" })).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Yesterday" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Today meeting" }));
  expect(screen.getByRole("button", { name: "Today meeting" }).getAttribute("aria-current")).toBe("page");
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  expect(screen.getByRole("heading", { name: "Settings pane" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Today meeting" }).getAttribute("aria-current")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Help" }));
  expect(screen.getByRole("heading", { name: "Help pane" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Help" }).getAttribute("aria-current")).toBe("page");
  expect(screen.getByRole("button", { name: "Settings" }).getAttribute("aria-current")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Yesterday meeting" }));
  expect(screen.queryByRole("heading", { name: "Help pane" })).toBeNull();
});
