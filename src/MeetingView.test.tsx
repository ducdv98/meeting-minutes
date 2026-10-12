import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import MeetingView from "./MeetingView";
import { meetingDetail, renameMeeting, type MeetingDetail } from "./library";
import { convertFileSrc } from "@tauri-apps/api/core";
import { onLibraryChanged } from "./recording";
import { processingStatus, meetingTranscript, enqueueTranscription, cancelProcessing, onProcessingStatus, setMeetingLanguage, type ProcessingStatus } from "./processing";
import { editSegment } from "./transcript";

vi.mock("./library", () => ({ meetingDetail: vi.fn(), renameMeeting: vi.fn() }));
vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), processingStatus: vi.fn(), meetingTranscript: vi.fn(), enqueueTranscription: vi.fn(), cancelProcessing: vi.fn(), dismissProcessing: vi.fn(), setMeetingLanguage: vi.fn(), onProcessingStatus: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("./recording", async (importOriginal) => ({ ...(await importOriginal<typeof import("./recording")>()), onLibraryChanged: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("./transcript", async (importOriginal) => ({ ...(await importOriginal<typeof import("./transcript")>()), editSegment: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: vi.fn((path: string) => `asset:${path}`) }));

const detail: MeetingDetail = {
  id: "old", title: "Planning", started_at: "2026-10-12T09:30:00Z",
  duration_ms: 125_000, language: "vi", audio_path: "C:\\Meetings\\meeting.mp3",
  markers: [
    { kind: "pause", at_ms: 10_000, paused_ms: 125_000 },
    { kind: "gap", at_ms: 20_000 },
    { kind: "device_switch", at_ms: 30_000, source: "microphone", device: "Headset" },
  ],
};

beforeEach(() => {
  vi.mocked(meetingDetail).mockResolvedValue(detail);
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [] });
  vi.mocked(meetingTranscript).mockResolvedValue(null);
  vi.mocked(enqueueTranscription).mockResolvedValue({ jobs: [{ meeting_id: "old", model: null, stage: "queued", progress: 0, error: null }] });
  vi.mocked(cancelProcessing).mockResolvedValue({ jobs: [] });
  vi.mocked(setMeetingLanguage).mockResolvedValue();
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.resetAllMocks(); });

it("shows the Meeting fields and Timeline markers", async () => {
  const { container } = render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByRole("heading", { name: "Planning" })).toBeTruthy();
  expect(screen.getByText("2026-10-12 09:30").getAttribute("datetime")).toBe(detail.started_at);
  expect(screen.getByText("Duration: 00:02:05")).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement).value).toBe("vi");
  expect(convertFileSrc).toHaveBeenCalledWith(detail.audio_path);
  expect(container.querySelector("audio")?.getAttribute("src")).toBe(`asset:${detail.audio_path}`);
  expect((screen.getByRole("slider", { name: "Position" }) as HTMLInputElement).max).toBe("125000");
  expect(screen.getByRole("button", { name: "Paused for 2m 05s at 00:00:10" }).getAttribute("data-kind")).toBe("pause");
  expect(screen.getByRole("button", { name: "Gap between Recordings at 00:00:20" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Microphone switched to Headset at 00:00:30" }).getAttribute("style")).toContain("24%");
  fireEvent.click(screen.getByRole("button", { name: "Gap between Recordings at 00:00:20" }));
  expect((screen.getByRole("slider", { name: "Position" }) as HTMLInputElement).value).toBe("20000");
});

it("renames the Meeting after releasing audio and follows the new id", async () => {
  const renamed: MeetingDetail = { ...detail, id: "new", title: "Review", audio_path: null, markers: [] };
  vi.mocked(meetingDetail).mockImplementation(async (id) => id === "new" ? renamed : detail);
  const onRenamed = vi.fn();
  vi.mocked(renameMeeting).mockImplementation(async () => {
    expect(document.querySelector("audio")).toBeNull();
    return { id: "new", title: "Review", started_at: detail.started_at, duration_ms: detail.duration_ms };
  });
  function Host() {
    const [id, setId] = useState("old");
    return <MeetingView id={id} onBack={() => {}} onRenamed={(next) => { onRenamed(next); setId(next); }} />;
  }
  render(<Host />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  const title = screen.getByRole("textbox", { name: "Title" });
  fireEvent.change(title, { target: { value: " Review " } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("heading", { name: "Review" })).toBeTruthy();
  expect(renameMeeting).toHaveBeenCalledWith("old", "Review");
  expect(onRenamed).toHaveBeenCalledWith("new");
  expect(meetingDetail).toHaveBeenCalledWith("new");
});

it("keeps the rename input open and shows an error on failure", async () => {
  vi.mocked(renameMeeting).mockRejectedValue("Folder is busy");
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Review" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Folder is busy");
  expect(screen.getByRole("textbox", { name: "Title" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
});

it("shows no audio, disables empty saves, and handles Escape", async () => {
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, language: null, audio_path: null });
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("No audio yet")).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement).value).toBe("");
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  const title = screen.getByRole("textbox", { name: "Title" });
  fireEvent.change(title, { target: { value: "   " } });
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.keyDown(title, { key: "Escape" });
  await waitFor(() => expect(screen.getByRole("heading", { name: "Planning" })).toBeTruthy());
});

it("enqueues transcription and can cancel a queued job", async () => {
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  fireEvent.click(await screen.findByRole("button", { name: "Transcribe" }));
  expect(enqueueTranscription).toHaveBeenCalledWith("old");
  expect(await screen.findByText("Waiting to transcribe")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel processing" }));
  await waitFor(() => expect(cancelProcessing).toHaveBeenCalledWith("old"));
});

it("shows download progress, failure actions, and a ready revision", async () => {
  const downloading: ProcessingStatus = { jobs: [{ meeting_id: "old", model: null, stage: "downloading", progress: 0.34, error: null }] };
  vi.mocked(processingStatus).mockResolvedValue(downloading);
  const { rerender } = render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Downloading model 34%")).toBeTruthy();
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: null, stage: "failed", progress: 0.34, error: "Model unavailable" }] });
  rerender(<MeetingView id="new" onBack={() => {}} onRenamed={() => {}} />);
  rerender(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect((await screen.findByRole("alert")).textContent).toContain("Model unavailable");
  expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Dismiss" })).toBeTruthy();
});

it("shows transcript metadata and changes the Meeting language", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "2026-10-12T10:00:00Z", model: "small", language: "vi", language_detected: true, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 0, end_ms: 1000, text: "Hello", speaker: "me" }] });
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Transcript revision 2 · model small · 1 segments · Vietnamese")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Re-transcribe" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));
  expect(enqueueTranscription).toHaveBeenCalledWith("old");
  fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "en" } });
  await waitFor(() => expect(setMeetingLanguage).toHaveBeenCalledWith("old", "en"));
});

it("re-transcribes with the chosen model and edits the current Revision", async () => {
  const original = { schema_version: 1, revision: 2, created_at: "2026-10-12T10:00:00Z", model: "small", language: "en" as const, language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 0, end_ms: 1000, text: "Hello", speaker: "me" as const }] };
  vi.mocked(meetingTranscript).mockResolvedValue(original);
  vi.mocked(editSegment).mockResolvedValue({ ...original, segments: [{ ...original.segments[0], text: "Updated" }] });
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Segment text" }), { target: { value: "Updated" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByText("Updated")).toBeTruthy();
  expect(editSegment).toHaveBeenCalledWith("old", 2, "s00001", "Updated");
  fireEvent.change(screen.getByRole("combobox", { name: "Model" }), { target: { value: "medium" } });
  fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));
  expect(enqueueTranscription).toHaveBeenCalledWith("old", "medium");
});

it("closes an open Segment editor and keeps the newer Revision when one arrives", async () => {
  const r1 = { schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: "small", language: "en" as const, language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 0, end_ms: 1000, text: "Hello", speaker: "me" as const }] };
  const r2 = { ...r1, revision: 2, segments: [{ ...r1.segments[0], text: "Hello again" }] };
  vi.mocked(meetingTranscript).mockResolvedValue(r1);
  let resolveEdit!: (transcript: typeof r1) => void;
  vi.mocked(editSegment).mockReturnValue(new Promise((resolve) => { resolveEdit = resolve; }));
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Segment text" }), { target: { value: "Edited" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  vi.mocked(meetingTranscript).mockResolvedValue(r2);
  await act(async () => { vi.mocked(onLibraryChanged).mock.calls[0][0](); });
  expect(await screen.findByText("Hello again")).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: "Segment text" })).toBeNull();
  await act(async () => resolveEdit({ ...r1, segments: [{ ...r1.segments[0], text: "Edited" }] }));
  expect(screen.getByText("Hello again")).toBeTruthy();
  expect(screen.queryByText("Edited")).toBeNull();
});

it("shows transcription progress and reloads a completed transcript", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: null, stage: "transcribing", progress: 0.61, error: null }] });
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Transcribing 61%")).toBeTruthy();
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: "small", language: "en", language_detected: false, duration_ms: 125000, segments: [] });
  await act(async () => { vi.mocked(onLibraryChanged).mock.calls[0][0](); });
  expect(await screen.findByText("Transcript revision 1 · model small · 0 segments · English")).toBeTruthy();
});

it("keeps a newer processing event when the initial snapshot arrives late", async () => {
  let resolveSnapshot!: (status: ProcessingStatus) => void;
  vi.mocked(processingStatus).mockReturnValue(new Promise((resolve) => { resolveSnapshot = resolve; }));
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  const event = vi.mocked(onProcessingStatus).mock.calls[0][0];
  await act(async () => event({ jobs: [{ meeting_id: "old", model: null, stage: "transcribing", progress: 0.61, error: null }] }));
  expect(screen.getByText("Transcribing 61%")).toBeTruthy();
  await act(async () => resolveSnapshot({ jobs: [{ meeting_id: "old", model: null, stage: "queued", progress: 0, error: null }] }));
  expect(screen.getByText("Transcribing 61%")).toBeTruthy();
  expect(screen.queryByText("Waiting to transcribe")).toBeNull();
});

it("follows the new id even when reloading the renamed Meeting fails", async () => {
  vi.mocked(meetingDetail).mockImplementation(async (id) => {
    if (id === "new") throw "Could not read meeting.json";
    return detail;
  });
  vi.mocked(renameMeeting).mockResolvedValue({ id: "new", title: "Review", started_at: detail.started_at, duration_ms: detail.duration_ms });
  const onRenamed = vi.fn();
  function Host() {
    const [id, setId] = useState("old");
    return <MeetingView id={id} onBack={() => {}} onRenamed={(next) => { onRenamed(next); setId(next); }} />;
  }
  render(<Host />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Review" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Could not read meeting.json");
  expect(onRenamed).toHaveBeenCalledWith("new");
  expect(screen.queryByRole("textbox", { name: "Title" })).toBeNull();
});
