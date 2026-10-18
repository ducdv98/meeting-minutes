import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import CaptureBarWindow from "./CaptureBarWindow";
import {
  onRecordingStatus,
  onSnapshotError,
  pauseRecording,
  recordingStatus,
  resumeRecording,
  stopRecording,
  takeSnapshot,
  type RecorderStatus,
} from "./recording";
import { getCurrentWindow } from "@tauri-apps/api/window";

vi.mock("./recording", async (importOriginal) => ({
  ...await importOriginal<typeof import("./recording")>(),
  onRecordingStatus: vi.fn(),
  onSnapshotError: vi.fn(),
  recordingStatus: vi.fn(),
  pauseRecording: vi.fn(),
  resumeRecording: vi.fn(),
  stopRecording: vi.fn(),
  takeSnapshot: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(),
  LogicalSize: class {
    constructor(public width: number, public height: number) {}
  },
}));

const recording: RecorderStatus = {
  phase: "recording", meeting_id: "m1", elapsed_ms: 65_000,
  notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [],
};
let pushStatus: (status: RecorderStatus) => void = () => {};
const windowApi = { startDragging: vi.fn(), setSize: vi.fn() };

beforeEach(() => {
  vi.mocked(getCurrentWindow).mockReturnValue(windowApi as unknown as ReturnType<typeof getCurrentWindow>);
  windowApi.startDragging.mockResolvedValue(undefined);
  windowApi.setSize.mockResolvedValue(undefined);
  vi.mocked(onRecordingStatus).mockImplementation((handler) => {
    pushStatus = handler;
    return Promise.resolve(() => {});
  });
  vi.mocked(onSnapshotError).mockResolvedValue(() => {});
  vi.mocked(recordingStatus).mockResolvedValue(recording);
});

afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("follows the Recording and uses its pause, resume, stop and repeat Snapshot commands", async () => {
  vi.mocked(pauseRecording).mockResolvedValue({ ...recording, phase: "paused" });
  vi.mocked(resumeRecording).mockResolvedValue(recording);
  vi.mocked(stopRecording).mockResolvedValue({ ...recording, phase: "idle", meeting_id: null });
  vi.mocked(takeSnapshot).mockResolvedValue(undefined);
  render(<CaptureBarWindow />);

  expect((await screen.findByRole("status")).textContent).toContain("00:01:05");
  fireEvent.click(screen.getByRole("button", { name: "Take Snapshot" }));
  await waitFor(() => expect(takeSnapshot).toHaveBeenCalledWith(false));
  fireEvent.click(screen.getByRole("button", { name: "Pause" }));
  expect(await screen.findByRole("button", { name: "Resume" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Resume" }));
  expect(await screen.findByRole("button", { name: "Pause" })).toBeTruthy();
  act(() => pushStatus({ ...recording, elapsed_ms: 93_000 }));
  expect(screen.getByRole("status").textContent).toContain("00:01:33");
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  await waitFor(() => expect(stopRecording).toHaveBeenCalledTimes(1));
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("collapses to a draggable timer and expands back to the controls", async () => {
  render(<CaptureBarWindow />);
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Drag Capture bar" }));
  expect(windowApi.startDragging).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole("button", { name: "Collapse Capture bar" }));
  await waitFor(() => expect(windowApi.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 174, height: 58 })));
  expect(screen.queryByRole("button", { name: "Take Snapshot" })).toBeNull();
  expect(screen.getByRole("status").textContent).toContain("00:01:05");

  fireEvent.click(screen.getByRole("button", { name: "Expand Capture bar" }));
  await waitFor(() => expect(windowApi.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 354, height: 58 })));
  expect(screen.getByRole("button", { name: "Take Snapshot" })).toBeTruthy();
});
