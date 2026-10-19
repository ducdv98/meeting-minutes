import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import RecordPromptWindow from "./RecordPromptWindow";
import { dismissRecordPrompt, getRecordPrompt, showRecordingNotice } from "./record-prompt";
import { recordingStatus, startRecording, type RecorderStatus } from "./recording";

vi.mock("./record-prompt", () => ({
  dismissRecordPrompt: vi.fn(), getRecordPrompt: vi.fn(), showRecordingNotice: vi.fn(),
}));
vi.mock("./recording", async (original) => ({
  ...await original<typeof import("./recording")>(), recordingStatus: vi.fn(), startRecording: vi.fn(),
}));

const idle: RecorderStatus = {
  phase: "idle", meeting_id: null, elapsed_ms: 0, notice_acknowledged: true,
  saves: [], continue_offers: [], device_switches: [],
};

beforeEach(() => {
  vi.mocked(getRecordPrompt).mockResolvedValue({ id: "teams", display_name: "Teams" });
  vi.mocked(recordingStatus).mockResolvedValue(idle);
  vi.mocked(startRecording).mockResolvedValue({ ...idle, phase: "recording", meeting_id: "m1" });
  vi.mocked(dismissRecordPrompt).mockResolvedValue(undefined);
  vi.mocked(showRecordingNotice).mockResolvedValue(undefined);
});

afterEach(() => { cleanup(); vi.useRealTimers(); vi.resetAllMocks(); });

it("names the app and offers Record and Not now", async () => {
  render(<RecordPromptWindow />);
  expect((await screen.findByText(/Teams/)).textContent).toContain("Teams");
  expect(screen.getByRole("button", { name: "Record" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Not now" }));
  await waitFor(() => expect(dismissRecordPrompt).toHaveBeenCalledOnce());
  expect(startRecording).not.toHaveBeenCalled();
});

it("Record uses the usual start command", async () => {
  render(<RecordPromptWindow />);
  fireEvent.click(await screen.findByRole("button", { name: "Record" }));
  await waitFor(() => expect(startRecording).toHaveBeenCalledOnce());
  expect(dismissRecordPrompt).toHaveBeenCalledOnce();
});

it("Record opens the main window's Recording notice when needed", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, notice_acknowledged: false });
  render(<RecordPromptWindow />);
  fireEvent.click(await screen.findByRole("button", { name: "Record" }));
  await waitFor(() => expect(showRecordingNotice).toHaveBeenCalledOnce());
  expect(startRecording).not.toHaveBeenCalled();
  expect(dismissRecordPrompt).toHaveBeenCalledOnce();
});

it("closes after about twenty seconds without starting a Recording", async () => {
  vi.useFakeTimers();
  render(<RecordPromptWindow />);
  await act(async () => {});
  expect(screen.getByText(/Teams/)).toBeTruthy();
  await act(async () => { await vi.advanceTimersByTimeAsync(20_000); });
  expect(dismissRecordPrompt).toHaveBeenCalledOnce();
  expect(startRecording).not.toHaveBeenCalled();
});
