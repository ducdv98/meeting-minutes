import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import RecordingBar from "./RecordingBar";
import {
  acknowledgeRecordingNotice,
  continueMeeting,
  dismissContinue,
  dismissSourceWarning,
  formatElapsed,
  onRecordingNoticeRequested,
  onRecordingStatus,
  onSnapshotError,
  openSoundSettings,
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
    dismissSourceWarning: vi.fn(),
    openSoundSettings: vi.fn(),
    onRecordingStatus: vi.fn(),
    onSnapshotError: vi.fn(),
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
  device_switches: [],
};
const recording: RecorderStatus = { ...idle, phase: "recording", meeting_id: "m1", elapsed_ms: 65_000 };

it("shows the latest device switch until dismissed and shows a newer one", async () => {
  const first = { at_ms: 500, source: "microphone" as const, device: "Logi USB Headset" };
  const second = { at_ms: 800, source: "system_audio" as const, device: "Speakers" };
  vi.mocked(recordingStatus).mockResolvedValue({ ...recording, device_switches: [first] });
  render(<RecordingBar />);

  expect((await screen.findByText(/Microphone switched to Logi USB Headset/)).closest('[role="status"]')).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  expect(screen.queryByText(/Microphone switched to/)).toBeNull();

  act(() => pushStatus({ ...recording, device_switches: [first, second] }));
  expect(screen.getByText(/System audio switched to Speakers/)).toBeTruthy();

  act(() => pushStatus({ ...idle, device_switches: [first, second] }));
  expect(screen.queryByText(/switched to/)).toBeNull();
});

let pushStatus: (status: RecorderStatus) => void = () => {};
let requestNotice: () => void = () => {};
let reportSnapshotError: (error: string) => void = () => {};

beforeEach(() => {
  vi.mocked(onRecordingStatus).mockImplementation((handler) => {
    pushStatus = handler;
    return Promise.resolve(() => {});
  });
  vi.mocked(onRecordingNoticeRequested).mockImplementation((handler) => {
    requestNotice = handler;
    return Promise.resolve(() => {});
  });
  vi.mocked(onSnapshotError).mockImplementation((handler) => {
    reportSnapshotError = handler;
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

it("handles a sidebar Record request made before the Recording state loads", async () => {
  let resolveStatus!: (status: RecorderStatus) => void;
  vi.mocked(recordingStatus).mockReturnValue(new Promise((resolve) => { resolveStatus = resolve; }));
  vi.mocked(startRecording).mockResolvedValue(recording);
  const view = render(<RecordingBar recordRequest={0} hideIdleButton />);
  view.rerender(<RecordingBar recordRequest={1} hideIdleButton />);
  await act(async () => resolveStatus(idle));
  expect(startRecording).toHaveBeenCalledTimes(1);
  expect((await screen.findByRole("status")).textContent).toContain("Recording 00:01:05");
});

it("shows the Recording notice before the first Recording ever", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, notice_acknowledged: false });
  vi.mocked(acknowledgeRecordingNotice).mockResolvedValue(idle);
  vi.mocked(startRecording).mockResolvedValue(recording);
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Record" }));

  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(screen.getByRole("dialog").textContent).toMatch(/Snapshots may capture what others share/);
  expect(screen.getByRole("dialog").textContent).toMatch(/Each Snapshot is confirmed on screen/);
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

it("shows a Snapshot hotkey registration failure", async () => {
  vi.mocked(recordingStatus).mockResolvedValue(recording);
  render(<RecordingBar />);
  await screen.findByRole("status");

  act(() => reportSnapshotError("Could not register Ctrl+Alt+S"));

  expect(screen.getByRole("alert").textContent).toContain("Could not register Ctrl+Alt+S");
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

it("shows actual Source devices, levels and hotkeys", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({
    ...recording,
    source_devices: [
      { source: "microphone", device: "USB Microphone" },
      { source: "system_audio", device: "Speakers" },
    ],
    source_levels: [{ source: "microphone", level: 0.7, silent: false }, { source: "system_audio", level: 0, silent: true }],
  });
  render(<RecordingBar />);

  expect(await screen.findByText("USB Microphone")).toBeTruthy();
  expect(screen.getByText("Speakers")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Pause" }).getAttribute("title")).toContain("Ctrl+Alt+Shift+P");
  expect(screen.getByRole("button", { name: "Stop" }).getAttribute("title")).toContain("Ctrl+Alt+Shift+R");
  expect(screen.getAllByRole("meter")).toHaveLength(2);
  expect(screen.getByRole("meter", { name: "Microphone level" }).getAttribute("aria-valuenow")).toBe("70");
  expect(screen.getByText("Silent")).toBeTruthy();
});

it("shows the Microphone no signal warning and handles its actions", async () => {
  const warning: RecorderStatus = {
    ...recording,
    source_devices: [{ source: "microphone", device: "USB Microphone" }],
    source_health: [{ source: "microphone", warning: "no_signal" }],
  };
  vi.mocked(recordingStatus).mockResolvedValue(warning);
  vi.mocked(openSoundSettings).mockResolvedValue(undefined);
  vi.mocked(dismissSourceWarning).mockResolvedValue({ ...warning, source_health: [{ source: "microphone", warning: null }] });
  render(<RecordingBar />);

  expect((await screen.findByRole("alert")).textContent).toContain("Microphone has no signal");
  expect(screen.getByRole("alert").textContent).toContain("USB Microphone");
  fireEvent.click(screen.getByRole("button", { name: "Open sound settings" }));
  await vi.waitFor(() => expect(openSoundSettings).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  await vi.waitFor(() => expect(dismissSourceWarning).toHaveBeenCalledWith("microphone"));
  await vi.waitFor(() => expect(screen.queryByText(/Microphone has no signal/)).toBeNull());
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
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

  expect(await screen.findByText(/The audio for m1 is kept. disk full/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));

  expect(await screen.findByText("Saving m1…")).toBeTruthy();
  expect(retrySave).toHaveBeenCalledWith("m1");
});

it("shows an error when a recording action fails", async () => {
  vi.mocked(recordingStatus).mockResolvedValue(idle);
  vi.mocked(startRecording).mockRejectedValue("A Recording is already active");
  render(<RecordingBar />);

  fireEvent.click(await screen.findByRole("button", { name: "Record" }));

  expect((await screen.findByRole("alert")).textContent).toContain("A Recording is already active");
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
