import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import RecordingBar from "./RecordingBar";
import {
  acknowledgeRecordingNotice,
  continueMeeting,
  dismissContinue,
  formatElapsed,
  onRecordingNoticeRequested,
  onRecordingStatus,
  pauseRecording,
  recordingStatus,
  retrySave,
  startRecording,
  stopRecording,
  type RecorderStatus,
} from "./recording";

vi.mock("./recording", async (importOriginal) => {
  const original = await importOriginal<typeof import("./recording")>();
  return {
    ...original,
    recordingStatus: vi.fn(),
    startRecording: vi.fn(),
    pauseRecording: vi.fn(),
    resumeRecording: vi.fn(),
    stopRecording: vi.fn(),
    retrySave: vi.fn(),
    acknowledgeRecordingNotice: vi.fn(),
    continueMeeting: vi.fn(),
    dismissContinue: vi.fn(),
    onRecordingStatus: vi.fn(),
    onRecordingNoticeRequested: vi.fn(),
  };
});

const idle: RecorderStatus = {
  phase: "idle",
  meeting_id: null,
  elapsed_ms: 0,
  notice_acknowledged: true,
  saves: [],
  continue_offers: [],
};
const recording: RecorderStatus = { ...idle, phase: "recording", meeting_id: "m1", elapsed_ms: 65_000 };

let pushStatus: (status: RecorderStatus) => void = () => {};
let requestNotice: () => void = () => {};

beforeEach(() => {
  vi.mocked(onRecordingStatus).mockImplementation((handler) => {
    pushStatus = handler;
    return Promise.resolve(() => {});
  });
  vi.mocked(onRecordingNoticeRequested).mockImplementation((handler) => {
    requestNotice = handler;
    return Promise.resolve(() => {});
  });
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("formats elapsed time as hours, minutes and seconds", () => {
  expect(formatElapsed(0)).toBe("00:00:00");
  expect(formatElapsed(3_723_999)).toBe("01:02:03");
});

it("starts a Recording when Record is clicked", async () => {
  vi.mocked(recordingStatus).mockResolvedValue(idle);
  vi.mocked(startRecording).mockResolvedValue(recording);
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Record" }));

  expect(await screen.findByRole("status")).toBeTruthy();
  expect(screen.getByRole("status").textContent).toContain("Recording 00:01:05");
  expect(startRecording).toHaveBeenCalledTimes(1);
});

it("shows the Recording notice before the first Recording ever", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, notice_acknowledged: false });
  vi.mocked(acknowledgeRecordingNotice).mockResolvedValue(idle);
  vi.mocked(startRecording).mockResolvedValue(recording);
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Record" }));

  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(startRecording).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Start recording" }));
  expect(await screen.findByText(/Recording/, { selector: "p" })).toBeTruthy();
  expect(acknowledgeRecordingNotice).toHaveBeenCalledTimes(1);
  expect(startRecording).toHaveBeenCalledTimes(1);
});

it("cancelling the Recording notice does not record", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, notice_acknowledged: false });
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Record" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

  expect(screen.queryByRole("dialog")).toBeNull();
  expect(startRecording).not.toHaveBeenCalled();
});

it("shows the Recording notice when the tray or hotkey asks for it", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, notice_acknowledged: false });
  render(<RecordingBar />);
  await screen.findByRole("button", { name: "Record" });

  act(() => requestNotice());

  expect(screen.getByRole("dialog")).toBeTruthy();
});

it("follows elapsed time and phase pushed from the app", async () => {
  vi.mocked(recordingStatus).mockResolvedValue(recording);
  render(<RecordingBar />);
  await screen.findByRole("status");

  act(() => pushStatus({ ...recording, phase: "paused", elapsed_ms: 90_000 }));

  expect(screen.getByRole("status").textContent).toContain("Paused 00:01:30");
  expect(screen.getByRole("button", { name: "Resume" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Record" })).toBeNull();
});

it("pauses and stops the active Recording", async () => {
  vi.mocked(recordingStatus).mockResolvedValue(recording);
  vi.mocked(pauseRecording).mockResolvedValue({ ...recording, phase: "paused" });
  vi.mocked(stopRecording).mockResolvedValue({
    ...idle,
    saves: [{ meeting_id: "m1", state: "saving", error: null, recovered: false }],
  });
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Pause" }));
  fireEvent.click(await screen.findByRole("button", { name: "Stop" }));

  expect(await screen.findByText("Saving m1…")).toBeTruthy();
  expect(screen.queryByRole("status")).toBeNull();
});

it("offers Retry save after a failed save", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({
    ...idle,
    saves: [{ meeting_id: "m1", state: "failed", error: "disk full", recovered: false }],
  });
  vi.mocked(retrySave).mockResolvedValue({
    ...idle,
    saves: [{ meeting_id: "m1", state: "saving", error: null, recovered: false }],
  });
  render(<RecordingBar />);

  expect(await screen.findByText(/Could not save m1: disk full/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Retry save" }));

  expect(await screen.findByText("Saving m1…")).toBeTruthy();
  expect(retrySave).toHaveBeenCalledWith("m1");
});

it("shows an error when a recording action fails", async () => {
  vi.mocked(recordingStatus).mockResolvedValue(idle);
  vi.mocked(startRecording).mockRejectedValue("A Recording is already active");
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Record" }));

  expect((await screen.findByRole("alert")).textContent).toBe("A Recording is already active");
});

it("labels a save recovered after a crash", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({
    ...idle,
    saves: [{ meeting_id: "m1", state: "saving", error: null, recovered: true }],
  });
  render(<RecordingBar />);

  expect(await screen.findByText("Recovering m1…")).toBeTruthy();
});

it("offers to continue a recovered Meeting with a new Recording", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, continue_offers: ["m1"] });
  vi.mocked(continueMeeting).mockResolvedValue(recording);
  render(<RecordingBar />);

  expect(await screen.findByText(/Recovered m1/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Continue meeting" }));

  expect(await screen.findByRole("status")).toBeTruthy();
  expect(continueMeeting).toHaveBeenCalledWith("m1");
  expect(screen.queryByText(/Recovered m1/)).toBeNull();
});

it("dismisses the offer to continue a recovered Meeting", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, continue_offers: ["m1"] });
  vi.mocked(dismissContinue).mockResolvedValue(idle);
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Dismiss" }));

  expect(dismissContinue).toHaveBeenCalledWith("m1");
  await vi.waitFor(() => expect(screen.queryByText(/Recovered m1/)).toBeNull());
  expect(continueMeeting).not.toHaveBeenCalled();
});

it("shows the Recording notice before continuing when it was never shown", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({
    ...idle,
    notice_acknowledged: false,
    continue_offers: ["m1"],
  });
  vi.mocked(acknowledgeRecordingNotice).mockResolvedValue(idle);
  vi.mocked(continueMeeting).mockResolvedValue(recording);
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Continue meeting" }));
  expect(continueMeeting).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Start recording" }));

  expect(await screen.findByRole("status")).toBeTruthy();
  expect(continueMeeting).toHaveBeenCalledWith("m1");
  expect(startRecording).not.toHaveBeenCalled();
});
