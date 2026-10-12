import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "./App";
import { pickAudioFiles, importStatus, dismissImportErrors, onImportStatus, onImportHover } from "./importing";
import { rescanLibrary } from "./library";
import { cancelProcessing, onProcessingStatus, processingStatus, type ProcessingStatus } from "./processing";

vi.mock("./importing", () => ({ pickAudioFiles: vi.fn(), importStatus: vi.fn(), dismissImportErrors: vi.fn(), onImportStatus: vi.fn(), onImportHover: vi.fn() }));
vi.mock("./library", () => ({ rescanLibrary: vi.fn() }));
vi.mock("./RecordingBar", () => ({ default: () => null }));
vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), processingStatus: vi.fn(), onProcessingStatus: vi.fn(), cancelProcessing: vi.fn() }));
vi.mock("./MeetingView", () => ({ default: ({ id, onBack }: { id: string; onBack: () => void }) => (
  <section><h1>Meeting {id}</h1><button type="button" onClick={onBack}>Back to Library</button></section>
) }));
vi.mock("./recording", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./recording")>()),
  onLibraryChanged: () => Promise.resolve(() => {}),
}));

beforeEach(() => {
  vi.mocked(importStatus).mockResolvedValue({ current: null, queued: 0, errors: [] });
  vi.mocked(onImportStatus).mockResolvedValue(() => {});
  vi.mocked(onImportHover).mockResolvedValue(() => {});
  vi.mocked(dismissImportErrors).mockResolvedValue({ current: null, queued: 0, errors: [] });
  vi.mocked(pickAudioFiles).mockResolvedValue({ current: null, queued: 0, errors: [] });
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [] });
  vi.mocked(onProcessingStatus).mockResolvedValue(() => {});
  vi.mocked(cancelProcessing).mockResolvedValue({ jobs: [] });
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
  expect(screen.getByText("Downloading model 34%")).toBeTruthy();
  const handler = vi.mocked(onProcessingStatus).mock.calls[0][0];
  const live: ProcessingStatus = { jobs: [
    { meeting_id: "one", model: null, stage: "transcribing", progress: 0.61, error: null },
    { meeting_id: "two", model: null, stage: "failed", progress: 0.34, error: "network" },
  ] };
  await act(async () => handler(live));
  expect(screen.getByText("Transcribing 61%")).toBeTruthy();
  expect(screen.getByText("Transcription failed")).toBeTruthy();
  const row = screen.getByRole("button", { name: "Planning" }).closest("li")!;
  fireEvent.click(row.querySelector("button[aria-label='Cancel processing']")!);
  await waitFor(() => expect(cancelProcessing).toHaveBeenCalledWith("one"));
});

it("shows an empty Library when there are no Meetings", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [], malformed: [] });

  render(<App />);

  expect(screen.getByRole("heading", { name: "Library" })).toBeTruthy();
  expect(await screen.findByText("No meetings yet")).toBeTruthy();
});

it("shows the Meeting Minutes wordmark", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [], malformed: [] });

  render(<App />);

  expect(screen.getByRole("img", { name: "Meeting Minutes" })).toBeTruthy();
  await screen.findByText("No meetings yet");
});

it("lists Meeting titles in the Library", async () => {
  vi.mocked(rescanLibrary).mockResolvedValue({ meetings: [
    { id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", duration_ms: 0 },
    { id: "two", title: "Review", started_at: "2026-10-12T10:00:00Z", duration_ms: 1_830_000 },
  ], malformed: [] });

  render(<App />);

  expect(await screen.findByText("Planning")).toBeTruthy();
  expect(screen.getAllByRole("listitem")[1].textContent).toContain("Review · 00:30:30");
  expect(screen.getByText("2026-10-12 09:00").getAttribute("datetime")).toBe("2026-10-12T09:00:00Z");
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
  await screen.findByText("No meetings yet");
  fireEvent.click(screen.getByRole("button", { name: "Rescan" }));
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
  fireEvent.click(screen.getByRole("button", { name: "Import audio" }));
  await waitFor(() => expect(pickAudioFiles).toHaveBeenCalled());
  await act(async () => vi.mocked(onImportStatus).mock.calls[0][0]({ current: "Team.wav", queued: 2, errors: ["bad.txt: not an mp3, m4a, wav or flac file"] }));
  expect(screen.getByRole("status").textContent).toContain("Importing Team.wav… (+2 queued)");
  expect(screen.getByRole("alert").textContent).toContain("bad.txt");
  fireEvent.click(screen.getByRole("button", { name: "Dismiss import errors" }));
  await waitFor(() => expect(dismissImportErrors).toHaveBeenCalled());
  await act(async () => vi.mocked(onImportHover).mock.calls[0][0](true));
  expect(screen.getByText("Drop audio to import (mp3, m4a, wav, flac)")).toBeTruthy();
});
